//! A small terminal emulator for the panel's console: it turns what a shell writes
//! into lines of text with a cursor. It understands what line-based programs and
//! a shell's line editing use (CR, LF, backspace, tab, cursor moves, erasing), and
//! what full-screen programs such as `vi`, `less` and `top` need from a VT100:
//! scrolling regions, inserting and deleting lines, reverse video, saving the
//! cursor, application cursor keys and the alternate screen. Colors and other
//! attributes are dropped. Lines that scroll off the top of the main screen stay
//! as scrollback. Plain computation, tested on the host.

/// Lines kept in all, scrollback included.
const KEEP: usize = 3000;

/// One character on the screen, and whether it is drawn in reverse video.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub character: char,
    pub reverse: bool,
}

const BLANK: Cell = Cell { character: ' ', reverse: false };

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Ground,
    Escape,
    /// Inside `ESC [`, with the parameters read so far.
    Csi,
    /// Inside `ESC ]` (a title or similar), until BEL or `ESC \`.
    Osc,
    OscEscape,
    /// `ESC (` and the like take one more character.
    Charset,
}

/// A saved cursor: its row on the screen, its column and the reverse attribute.
#[derive(Clone, Copy)]
struct SavedCursor {
    row: usize,
    column: usize,
    reverse: bool,
}

/// The main screen, kept while the alternate screen is shown.
struct MainScreen {
    lines: Vec<Vec<Cell>>,
    row: usize,
    column: usize,
}

pub struct Terminal {
    lines: Vec<Vec<Cell>>,
    columns: usize,
    rows: usize,
    /// The cursor, as an index into `lines` and a column. The column can equal
    /// `columns`, which means the next character starts a new line.
    row: usize,
    column: usize,
    state: State,
    parameters: String,
    /// Bytes of a UTF-8 character still to skip.
    continuation: u8,
    /// Whether new characters are drawn in reverse video.
    reverse: bool,
    /// The scrolling region, as the first and last screen rows. `None` is the
    /// whole screen.
    region: Option<(usize, usize)>,
    saved: Option<SavedCursor>,
    /// Set while the alternate screen is shown, which has no scrollback.
    main: Option<MainScreen>,
    application_cursor: bool,
    cursor_visible: bool,
}

impl Terminal {
    pub fn new(columns: usize, rows: usize) -> Self {
        Terminal {
            lines: vec![Vec::new()],
            columns: columns.max(1),
            rows: rows.max(1),
            row: 0,
            column: 0,
            state: State::Ground,
            parameters: String::new(),
            continuation: 0,
            reverse: false,
            region: None,
            saved: None,
            main: None,
            application_cursor: false,
            cursor_visible: true,
        }
    }

    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    /// A new size, as the keyboard shows or hides. Lines of the main screen are
    /// kept as they are; programs are told through the pseudo-terminal and redraw.
    pub fn resize(&mut self, columns: usize, rows: usize) {
        self.columns = columns.max(1);
        self.rows = rows.max(1);
        self.column = self.column.min(self.columns);
        self.region = None;
        if self.main.is_some() {
            self.lines.resize_with(self.rows, Vec::new);
            self.row = self.row.min(self.rows - 1);
        }
    }

    pub fn lines(&self) -> &[Vec<Cell>] {
        &self.lines
    }

    /// The cursor's line (an index into `lines`) and column.
    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.column.min(self.columns - 1))
    }

    /// Whether the program wants the cursor shown.
    pub fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }

    /// Whether cursor keys should send `ESC O` instead of `ESC [`, as a program
    /// asks for with `ESC [ ? 1 h`.
    pub fn application_cursor(&self) -> bool {
        self.application_cursor
    }

    /// Adds a note of this system's own, such as that the shell ended, on a line
    /// of its own. Modes a full-screen program left behind are reset first.
    pub fn note(&mut self, text: &str) {
        self.leave_alternate(false);
        self.reverse = false;
        self.region = None;
        self.application_cursor = false;
        self.cursor_visible = true;
        if self.column > 0 || !self.lines[self.row].is_empty() {
            self.new_line();
        }
        self.feed(text.as_bytes());
        self.new_line();
    }

    /// Takes output from the program. Returns how many lines were added, so that a
    /// view scrolled back can stay where it is.
    pub fn feed(&mut self, bytes: &[u8]) -> usize {
        let before = self.lines.len();
        let mut dropped = 0;
        for &byte in bytes {
            self.byte(byte);
            if self.lines.len() > KEEP {
                let excess = self.lines.len() - KEEP;
                self.lines.drain(..excess);
                self.row = self.row.saturating_sub(excess);
                dropped += excess;
            }
        }
        (self.lines.len() + dropped).saturating_sub(before)
    }

    fn byte(&mut self, byte: u8) {
        match self.state {
            State::Ground => self.ground(byte),
            State::Escape => {
                self.state = State::Ground;
                match byte {
                    b'[' => {
                        self.parameters.clear();
                        self.state = State::Csi;
                    }
                    b']' => self.state = State::Osc,
                    b'(' | b')' | b'*' | b'+' | b'#' => self.state = State::Charset,
                    // ESC D, E, M: index, next line, reverse index.
                    b'D' => self.line_feed(),
                    b'E' => self.new_line(),
                    b'M' => self.reverse_index(),
                    b'7' => self.save_cursor(),
                    b'8' => self.restore_cursor(),
                    _ => {}
                }
            }
            State::Csi => {
                if (0x40..=0x7E).contains(&byte) {
                    self.state = State::Ground;
                    self.csi(byte);
                } else if self.parameters.len() < 32 {
                    self.parameters.push(byte as char);
                }
            }
            State::Osc => match byte {
                0x07 => self.state = State::Ground,
                0x1B => self.state = State::OscEscape,
                _ => {}
            },
            State::OscEscape => self.state = State::Ground,
            State::Charset => self.state = State::Ground,
        }
    }

    fn ground(&mut self, byte: u8) {
        if self.continuation > 0 {
            if byte & 0xC0 == 0x80 {
                self.continuation -= 1;
                return;
            }
            self.continuation = 0;
        }
        match byte {
            0x1B => self.state = State::Escape,
            b'\r' => self.column = 0,
            b'\n' | 0x0B | 0x0C => self.line_feed(),
            0x08 => self.column = self.column.min(self.columns).saturating_sub(1),
            b'\t' => {
                let next = (self.column / 8 + 1) * 8;
                self.column = next.min(self.columns - 1).max(self.column.min(self.columns - 1));
            }
            0x20..=0x7E => self.put(byte as char),
            // The font has ASCII only: other characters show as one `?`.
            0xC0..=0xFF => {
                self.continuation = match byte {
                    0xF0..=0xFF => 3,
                    0xE0..=0xEF => 2,
                    _ => 1,
                };
                self.put('?');
            }
            _ => {}
        }
    }

    fn put(&mut self, character: char) {
        if self.column >= self.columns {
            self.column = 0;
            self.line_feed();
        }
        let cell = Cell { character, reverse: self.reverse };
        let line = &mut self.lines[self.row];
        if line.len() <= self.column {
            line.resize(self.column, BLANK);
            line.push(cell);
        } else {
            line[self.column] = cell;
        }
        self.column += 1;
    }

    fn new_line(&mut self) {
        self.column = 0;
        self.line_feed();
    }

    fn line_feed(&mut self) {
        if self.region.is_some() || self.main.is_some() {
            let (first, last) = self.margins();
            if self.row == last {
                self.scroll_up(first, last, 1);
            } else if self.row + 1 < self.lines.len() {
                self.row += 1;
            }
            return;
        }
        self.row += 1;
        if self.row == self.lines.len() {
            self.lines.push(Vec::new());
        }
    }

    /// Up one line, scrolling the region down at its top line.
    fn reverse_index(&mut self) {
        let (first, last) = self.margins();
        if self.row == first {
            self.scroll_down(first, last, 1);
        } else if self.row > self.screen_top() {
            self.row -= 1;
        }
    }

    /// The first line of the screen: cursor positions count from here.
    fn screen_top(&self) -> usize {
        self.lines.len().saturating_sub(self.rows)
    }

    /// Makes the screen full height, so that every screen row is a line.
    fn fill_screen(&mut self) {
        while self.lines.len() < self.rows {
            self.lines.push(Vec::new());
        }
    }

    /// The scrolling region's first and last lines, as indexes into `lines`.
    fn margins(&mut self) -> (usize, usize) {
        self.fill_screen();
        let top = self.screen_top();
        let (first, last) = self.region.unwrap_or((0, self.rows - 1));
        (top + first, top + last)
    }

    /// Moves lines `first..=last` up by `count`, with blank lines coming in at the
    /// bottom. On the main screen, lines leaving the top of the whole screen are
    /// kept as scrollback, and the cursor stays on the same screen row.
    fn scroll_up(&mut self, first: usize, last: usize, count: usize) {
        let count = count.min(last + 1 - first);
        if self.main.is_none() && first == self.screen_top() && last + 1 == self.lines.len() {
            self.lines.extend(std::iter::repeat_with(Vec::new).take(count));
            self.row += count;
            return;
        }
        for _ in 0..count {
            self.lines.remove(first);
            self.lines.insert(last, Vec::new());
        }
    }

    /// Moves lines `first..=last` down by `count`, with blank lines coming in at
    /// the top.
    fn scroll_down(&mut self, first: usize, last: usize, count: usize) {
        for _ in 0..count.min(last + 1 - first) {
            self.lines.remove(last);
            self.lines.insert(first, Vec::new());
        }
    }

    fn save_cursor(&mut self) {
        self.saved = Some(SavedCursor {
            row: self.row.saturating_sub(self.screen_top()),
            column: self.column,
            reverse: self.reverse,
        });
    }

    fn restore_cursor(&mut self) {
        let saved = self.saved.unwrap_or(SavedCursor { row: 0, column: 0, reverse: false });
        self.fill_screen();
        self.row = self.screen_top() + saved.row.min(self.rows - 1);
        self.column = saved.column.min(self.columns);
        self.reverse = saved.reverse;
    }

    fn enter_alternate(&mut self, save_cursor: bool) {
        if self.main.is_some() {
            return;
        }
        if save_cursor {
            self.save_cursor();
        }
        let screen_row = self.row.saturating_sub(self.screen_top()).min(self.rows - 1);
        let lines = std::mem::replace(&mut self.lines, vec![Vec::new(); self.rows]);
        self.main = Some(MainScreen { lines, row: self.row, column: self.column });
        self.row = screen_row;
    }

    fn leave_alternate(&mut self, restore_cursor: bool) {
        let Some(main) = self.main.take() else { return };
        self.lines = main.lines;
        self.row = main.row.min(self.lines.len() - 1);
        self.column = main.column.min(self.columns);
        if restore_cursor && self.saved.is_some() {
            self.restore_cursor();
        }
    }

    /// `ESC [ ? ... h` and `l`: modes that programs set and reset.
    fn private_mode(&mut self, last: u8) {
        let on = match last {
            b'h' => true,
            b'l' => false,
            _ => return,
        };
        let modes: Vec<usize> = self.parameters[1..].split(';').filter_map(|part| part.parse().ok()).collect();
        for mode in modes {
            match mode {
                1 => self.application_cursor = on,
                25 => self.cursor_visible = on,
                47 | 1047 | 1049 if on => self.enter_alternate(mode == 1049),
                47 | 1047 | 1049 => self.leave_alternate(mode == 1049),
                _ => {}
            }
        }
    }

    /// `ESC [ ... m`: of the attributes, only reverse video is shown. It is what
    /// the `vt100` terminfo entry uses for standout, so status lines and
    /// selections show.
    fn attributes(&mut self, numbers: &[usize]) {
        for &number in numbers {
            match number {
                0 => self.reverse = false,
                7 => self.reverse = true,
                27 => self.reverse = false,
                _ => {}
            }
        }
    }

    fn csi(&mut self, last: u8) {
        if self.parameters.starts_with('?') {
            self.private_mode(last);
            return;
        }
        // Other private sequences (`ESC [ > ...`, `ESC [ = ...`) change nothing here.
        if self.parameters.starts_with(['>', '=']) {
            return;
        }
        let numbers: Vec<usize> = self.parameters.split(';').map(|part| part.parse().unwrap_or(0)).collect();
        let first = numbers.first().copied().unwrap_or(0);
        let count = first.max(1);
        let column = self.column.min(self.columns - 1);
        match last {
            b'A' | b'F' => {
                let (top, _) = self.margins();
                let limit = if self.row >= top { top } else { self.screen_top() };
                self.row = self.row.saturating_sub(count).max(limit);
                if last == b'F' {
                    self.column = 0;
                }
            }
            b'B' | b'E' => {
                let (_, bottom) = self.margins();
                let limit = if self.row <= bottom { bottom } else { self.lines.len() - 1 };
                self.row = (self.row + count).min(limit);
                if last == b'E' {
                    self.column = 0;
                }
            }
            b'C' => self.column = (column + count).min(self.columns - 1),
            b'D' => self.column = column.saturating_sub(count),
            b'G' | b'`' => self.column = (count - 1).min(self.columns - 1),
            b'd' => {
                // Addressing a screen taller than the output so far adds lines.
                self.fill_screen();
                self.row = self.screen_top() + (count - 1).min(self.rows - 1);
            }
            b'H' | b'f' => {
                let row = numbers.first().copied().unwrap_or(1).max(1) - 1;
                let column = numbers.get(1).copied().unwrap_or(1).max(1) - 1;
                self.fill_screen();
                self.row = self.screen_top() + row.min(self.rows - 1);
                self.column = column.min(self.columns - 1);
            }
            b'K' => {
                let line = &mut self.lines[self.row];
                match first {
                    0 => line.truncate(column),
                    1 => line.iter_mut().take(column + 1).for_each(|cell| *cell = BLANK),
                    _ => line.clear(),
                }
            }
            b'J' => {
                let home = self.row == self.screen_top() && column == 0;
                match first {
                    // From the top left of the main screen, as `clear` does, this
                    // clears the whole screen, which keeps it as scrollback.
                    0 if !(home && self.main.is_none()) => {
                        self.lines[self.row].truncate(column);
                        for line in &mut self.lines[self.row + 1..] {
                            line.clear();
                        }
                    }
                    1 => {
                        for row in self.screen_top()..self.row {
                            self.lines[row].clear();
                        }
                        self.lines[self.row].iter_mut().take(column + 1).for_each(|cell| *cell = BLANK);
                    }
                    _ if self.main.is_some() => {
                        self.lines.iter_mut().for_each(Vec::clear);
                    }
                    // Clearing the screen keeps what was on it as scrollback.
                    _ => {
                        let keep = self.lines.len();
                        let screen_row = self.row - self.screen_top().min(self.row);
                        self.lines.extend(std::iter::repeat_with(Vec::new).take(self.rows));
                        self.row = (keep + screen_row.min(self.rows - 1)).min(self.lines.len() - 1);
                    }
                }
            }
            b'L' | b'M' => {
                let (top, bottom) = self.margins();
                if (top..=bottom).contains(&self.row) {
                    if last == b'L' {
                        self.scroll_down(self.row, bottom, count);
                    } else {
                        self.scroll_up(self.row, bottom, count);
                    }
                    self.column = 0;
                }
            }
            b'S' => {
                let (top, bottom) = self.margins();
                self.scroll_up(top, bottom, count);
            }
            b'T' => {
                let (top, bottom) = self.margins();
                self.scroll_down(top, bottom, count);
            }
            b'P' => {
                let line = &mut self.lines[self.row];
                if column < line.len() {
                    let end = (column + count).min(line.len());
                    line.drain(column..end);
                }
            }
            b'@' => {
                let line = &mut self.lines[self.row];
                if column < line.len() {
                    for _ in 0..count {
                        line.insert(column, BLANK);
                    }
                    line.truncate(self.columns);
                }
            }
            b'X' => {
                let line = &mut self.lines[self.row];
                for cell in line.iter_mut().skip(column).take(count) {
                    *cell = BLANK;
                }
            }
            b'r' => {
                let top = first.max(1) - 1;
                let bottom = numbers.get(1).copied().filter(|&row| row > 0).unwrap_or(self.rows).min(self.rows) - 1;
                if top < bottom {
                    self.region = if top == 0 && bottom == self.rows - 1 { None } else { Some((top, bottom)) };
                    self.fill_screen();
                    self.row = self.screen_top();
                    self.column = 0;
                }
            }
            b's' => self.save_cursor(),
            b'u' => self.restore_cursor(),
            b'm' => self.attributes(&numbers),
            // Everything else is dropped.
            _ => {}
        }
    }
}

/// What a key sends to the shell. `control` turns a letter into its control
/// character, as a keyboard's Ctrl key does.
pub fn key_bytes(character: char, control: bool) -> Vec<u8> {
    if control {
        let upper = character.to_ascii_uppercase();
        if ('@'..='_').contains(&upper) {
            return vec![upper as u8 - b'@'];
        }
        if character == '?' {
            return vec![0x7F];
        }
    }
    let mut buffer = [0u8; 4];
    character.encode_utf8(&mut buffer).as_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(terminal: &Terminal) -> Vec<String> {
        terminal.lines().iter().map(|line| line.iter().map(|cell| cell.character).collect()).collect()
    }

    /// The screen's lines, the last `rows` of all lines.
    fn screen(terminal: &Terminal) -> Vec<String> {
        let lines = text(terminal);
        lines[lines.len().saturating_sub(terminal.rows())..].to_vec()
    }

    #[test]
    fn writes_lines() {
        let mut terminal = Terminal::new(10, 5);
        assert_eq!(terminal.feed(b"hello\r\nworld\r\n"), 2);
        assert_eq!(text(&terminal), ["hello", "world", ""]);
        assert_eq!(terminal.cursor(), (2, 0));
    }

    #[test]
    fn wraps_long_lines() {
        let mut terminal = Terminal::new(4, 5);
        terminal.feed(b"abcdefg");
        assert_eq!(text(&terminal), ["abcd", "efg"]);
        // Exactly full: the next line starts only with the next character.
        let mut terminal = Terminal::new(4, 5);
        terminal.feed(b"abcd\r\n");
        assert_eq!(text(&terminal), ["abcd", ""]);
    }

    #[test]
    fn edits_like_a_shell_prompt() {
        let mut terminal = Terminal::new(20, 5);
        // Typing, a backspace and erase to the end, as busybox ash does.
        terminal.feed(b"# lsx\x08\x1b[K");
        assert_eq!(text(&terminal), ["# ls"]);
        // History recall: back to the start of the line, rewrite it, erase the rest.
        terminal.feed(b"\r# echo hi\x1b[K");
        assert_eq!(text(&terminal), ["# echo hi"]);
        terminal.feed(b"\x1b[2D\x1b[P");
        assert_eq!(text(&terminal), ["# echo i"]);
        terminal.feed(b"\x1b[@h");
        assert_eq!(text(&terminal), ["# echo hi"]);
    }

    #[test]
    fn drops_colors_and_titles() {
        let mut terminal = Terminal::new(20, 5);
        terminal.feed(b"\x1b[1;34mdir\x1b[0m file\x1b]0;title\x07!\x1b(B\x1b[?25l");
        assert_eq!(text(&terminal), ["dir file!"]);
        assert!(terminal.lines()[0].iter().all(|cell| !cell.reverse));
        assert!(!terminal.cursor_visible());
        terminal.feed(b"\x1b[?25h");
        assert!(terminal.cursor_visible());
    }

    #[test]
    fn non_ascii_shows_as_one_mark() {
        let mut terminal = Terminal::new(20, 5);
        terminal.feed("a\u{e9}b\u{4e2d}c\u{1F600}d".as_bytes());
        assert_eq!(text(&terminal), ["a?b?c?d"]);
    }

    #[test]
    fn clearing_the_screen_keeps_scrollback() {
        let mut terminal = Terminal::new(10, 3);
        terminal.feed(b"one\r\ntwo\r\n");
        terminal.feed(b"\x1b[H\x1b[2J$ ");
        let lines = text(&terminal);
        assert_eq!(&lines[..2], ["one", "two"]);
        // The prompt is on the top line of a fresh screen.
        assert_eq!(lines.len() - terminal.rows(), terminal.cursor().0);
        assert_eq!(lines[terminal.cursor().0], "$ ");
    }

    #[test]
    fn clear_from_home_keeps_scrollback() {
        // Busybox `clear` sends ESC [ H ESC [ J.
        let mut terminal = Terminal::new(10, 3);
        terminal.feed(b"a\r\nb\r\nc\r\nd\r\n$ ");
        terminal.feed(b"\x1b[H\x1b[J$ ");
        assert_eq!(screen(&terminal), ["$ ", "", ""]);
        assert_eq!(&text(&terminal)[..5], ["a", "b", "c", "d", "$ "]);
    }

    #[test]
    fn erasing_below_keeps_the_screen_in_place() {
        let mut terminal = Terminal::new(10, 3);
        terminal.feed(b"a\r\nb\r\nc\r\nd\r\ne");
        terminal.feed(b"\x1b[2;2H\x1b[J");
        assert_eq!(text(&terminal), ["a", "b", "c", "d", ""]);
        assert_eq!(terminal.cursor(), (3, 1));
    }

    #[test]
    fn moves_to_screen_positions() {
        let mut terminal = Terminal::new(10, 3);
        terminal.feed(b"a\r\nb\r\nc\r\nd");
        terminal.feed(b"\x1b[1;3HX");
        // The screen is the last three lines: b, c, d.
        assert_eq!(text(&terminal), ["a", "b X", "c", "d"]);
        terminal.feed(b"\x1b[3dY");
        assert_eq!(screen(&terminal), ["b X", "c", "d  Y"]);
    }

    #[test]
    fn keeps_a_limited_scrollback() {
        let mut terminal = Terminal::new(10, 3);
        let added = terminal.feed("x\r\n".repeat(KEEP + 10).as_bytes());
        assert_eq!(added, KEEP + 10);
        assert_eq!(terminal.lines().len(), KEEP);
        assert_eq!(terminal.cursor().0, KEEP - 1);
    }

    #[test]
    fn notes_go_on_their_own_line() {
        let mut terminal = Terminal::new(20, 3);
        terminal.feed(b"# ");
        terminal.note("[ended]");
        assert_eq!(text(&terminal), ["# ", "[ended]", ""]);
    }

    #[test]
    fn scrolls_inside_a_region() {
        let mut terminal = Terminal::new(10, 4);
        terminal.feed(b"\x1b[H1\r\n2\r\n3\r\n4");
        // Region rows 2 and 3; a line feed at its bottom scrolls only those rows.
        terminal.feed(b"\x1b[2;3r\x1b[3;1H\nX");
        assert_eq!(screen(&terminal), ["1", "3", "X", "4"]);
        // A reverse index at the region's top scrolls it down.
        terminal.feed(b"\x1b[2;1H\x1bMY");
        assert_eq!(screen(&terminal), ["1", "Y", "3", "4"]);
        // Nothing went to scrollback.
        assert_eq!(terminal.lines().len(), 4);
        // CSI S and T scroll the region too.
        terminal.feed(b"\x1b[S");
        assert_eq!(screen(&terminal), ["1", "3", "", "4"]);
        terminal.feed(b"\x1b[T");
        assert_eq!(screen(&terminal), ["1", "", "3", "4"]);
    }

    #[test]
    fn whole_screen_region_keeps_scrollback() {
        let mut terminal = Terminal::new(10, 2);
        terminal.feed(b"\x1b[1;2r\x1b[2;1Ha\r\nb\r\nc");
        assert_eq!(text(&terminal), ["", "a", "b", "c"]);
        // Scrolling the whole main screen keeps lines as scrollback, and the
        // cursor stays on its screen row.
        terminal.feed(b"\x1b[S");
        assert_eq!(text(&terminal), ["", "a", "b", "c", ""]);
        assert_eq!(terminal.cursor(), (4, 1));
    }

    #[test]
    fn cursor_moves_stop_at_the_region() {
        let mut terminal = Terminal::new(10, 5);
        terminal.feed(b"\x1b[2;4r\x1b[3;1H\x1b[9A");
        assert_eq!(terminal.cursor().0, 1);
        terminal.feed(b"\x1b[9B");
        assert_eq!(terminal.cursor().0, 3);
    }

    #[test]
    fn inserts_and_deletes_lines() {
        let mut terminal = Terminal::new(10, 4);
        terminal.feed(b"\x1b[H1\r\n2\r\n3\r\n4");
        terminal.feed(b"\x1b[2;1H\x1b[L");
        assert_eq!(screen(&terminal), ["1", "", "2", "3"]);
        terminal.feed(b"\x1b[2M");
        assert_eq!(screen(&terminal), ["1", "3", "", ""]);
        // Inside a region, lines below it stay.
        let mut terminal = Terminal::new(10, 4);
        terminal.feed(b"\x1b[H1\r\n2\r\n3\r\n4\x1b[1;3r\x1b[1;1H\x1b[L");
        assert_eq!(screen(&terminal), ["", "1", "2", "4"]);
    }

    #[test]
    fn shows_reverse_video() {
        let mut terminal = Terminal::new(10, 3);
        terminal.feed(b"a\x1b[7mbc\x1b[27md\x1b[7me\x1b[mf");
        let reverse: Vec<bool> = terminal.lines()[0].iter().map(|cell| cell.reverse).collect();
        assert_eq!(reverse, [false, true, true, false, true, false]);
    }

    #[test]
    fn saves_and_restores_the_cursor() {
        let mut terminal = Terminal::new(10, 3);
        terminal.feed(b"\x1b[2;3H\x1b7\x1b[H\x1b8X");
        assert_eq!(screen(&terminal), ["", "  X", ""]);
        terminal.feed(b"\x1b[1;5H\x1b[s\x1b[3;1H\x1b[uY");
        assert_eq!(screen(&terminal), ["    Y", "  X", ""]);
    }

    #[test]
    fn alternate_screen_restores_the_main_one() {
        let mut terminal = Terminal::new(10, 3);
        terminal.feed(b"one\r\ntwo\r\n$ vi");
        terminal.feed(b"\x1b[?1049h\x1b[H\x1b[2Jfile\r\n~\r\n~\r\n~");
        // No scrollback on the alternate screen, and nothing of the main one.
        assert_eq!(text(&terminal), ["~", "~", "~"]);
        terminal.feed(b"\x1b[?1049l");
        assert_eq!(text(&terminal), ["one", "two", "$ vi"]);
        assert_eq!(terminal.cursor(), (2, 4));
    }

    #[test]
    fn a_note_leaves_the_alternate_screen() {
        let mut terminal = Terminal::new(20, 3);
        terminal.feed(b"$ \x1b[?1049h\x1b[?1h\x1b[7mx");
        terminal.note("[ended]");
        assert!(!terminal.application_cursor());
        assert_eq!(text(&terminal), ["$ ", "[ended]", ""]);
        assert!(terminal.lines()[1].iter().all(|cell| !cell.reverse));
    }

    #[test]
    fn resizing_the_alternate_screen() {
        let mut terminal = Terminal::new(10, 3);
        terminal.feed(b"\x1b[?1049h\x1b[3;1Hx");
        terminal.resize(10, 5);
        assert_eq!(terminal.lines().len(), 5);
        terminal.resize(10, 2);
        assert_eq!(terminal.lines().len(), 2);
        assert_eq!(terminal.cursor().0, 1);
    }

    #[test]
    fn application_cursor_mode() {
        let mut terminal = Terminal::new(10, 3);
        assert!(!terminal.application_cursor());
        terminal.feed(b"\x1b[?1h\x1b=");
        assert!(terminal.application_cursor());
        terminal.feed(b"\x1b[?1l\x1b>");
        assert!(!terminal.application_cursor());
    }

    #[test]
    fn keys_with_control() {
        assert_eq!(key_bytes('c', true), [3]);
        assert_eq!(key_bytes('D', true), [4]);
        assert_eq!(key_bytes('[', true), [0x1B]);
        assert_eq!(key_bytes('c', false), b"c");
        assert_eq!(key_bytes('1', true), b"1");
    }
}
