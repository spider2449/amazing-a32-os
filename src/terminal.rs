//! A small terminal emulator for the panel's console: it turns what a shell writes
//! into lines of text with a cursor. It understands what line-based programs and
//! a shell's line editing use (CR, LF, backspace, tab, cursor moves, erasing) and
//! drops everything else, such as colors. Lines that scroll off the top stay as
//! scrollback. Plain computation, tested on the host.

/// Lines kept in all, scrollback included.
const KEEP: usize = 3000;

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

pub struct Terminal {
    lines: Vec<Vec<char>>,
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
        }
    }

    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    /// A new size, as the keyboard shows or hides. Existing lines are kept as
    /// they are; programs are told through the pseudo-terminal.
    pub fn resize(&mut self, columns: usize, rows: usize) {
        self.columns = columns.max(1);
        self.rows = rows.max(1);
        self.column = self.column.min(self.columns);
    }

    pub fn lines(&self) -> &[Vec<char>] {
        &self.lines
    }

    /// The cursor's line (an index into `lines`) and column.
    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.column.min(self.columns - 1))
    }

    /// Adds a note of this system's own, such as that the shell ended, on a line
    /// of its own.
    pub fn note(&mut self, text: &str) {
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
                self.state = match byte {
                    b'[' => {
                        self.parameters.clear();
                        State::Csi
                    }
                    b']' => State::Osc,
                    b'(' | b')' | b'*' | b'+' | b'#' => State::Charset,
                    // ESC D, E, M: index, next line, reverse index.
                    b'D' => {
                        self.line_feed();
                        State::Ground
                    }
                    b'E' => {
                        self.column = 0;
                        self.line_feed();
                        State::Ground
                    }
                    b'M' => {
                        self.row = self.row.max(self.screen_top() + 1) - 1;
                        State::Ground
                    }
                    _ => State::Ground,
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
            0x08 => self.column = self.column.min(self.columns) .saturating_sub(1),
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
        let line = &mut self.lines[self.row];
        if line.len() <= self.column {
            line.resize(self.column, ' ');
            line.push(character);
        } else {
            line[self.column] = character;
        }
        self.column += 1;
    }

    fn new_line(&mut self) {
        self.column = 0;
        self.line_feed();
    }

    fn line_feed(&mut self) {
        self.row += 1;
        if self.row == self.lines.len() {
            self.lines.push(Vec::new());
        }
    }

    /// The first line of the screen: cursor positions count from here.
    fn screen_top(&self) -> usize {
        self.lines.len().saturating_sub(self.rows)
    }

    fn csi(&mut self, last: u8) {
        // Private modes (`ESC [ ? ...`) change nothing here.
        if self.parameters.starts_with(['?', '>', '=']) {
            return;
        }
        let numbers: Vec<usize> = self.parameters.split(';').map(|part| part.parse().unwrap_or(0)).collect();
        let first = numbers.first().copied().unwrap_or(0);
        let count = first.max(1);
        let column = self.column.min(self.columns - 1);
        match last {
            b'A' => self.row = self.row.saturating_sub(count).max(self.screen_top().min(self.row)),
            b'B' => self.row = (self.row + count).min(self.lines.len() - 1),
            b'C' => self.column = (column + count).min(self.columns - 1),
            b'D' => self.column = column.saturating_sub(count),
            b'G' => self.column = (count - 1).min(self.columns - 1),
            b'H' | b'f' => {
                let row = numbers.first().copied().unwrap_or(1).max(1) - 1;
                let column = numbers.get(1).copied().unwrap_or(1).max(1) - 1;
                // Addressing a screen taller than the output so far adds lines.
                while self.lines.len() < self.rows {
                    self.lines.push(Vec::new());
                }
                self.row = self.screen_top() + row.min(self.rows - 1);
                self.column = column.min(self.columns - 1);
            }
            b'K' => {
                let line = &mut self.lines[self.row];
                match first {
                    0 => line.truncate(column),
                    1 => line.iter_mut().take(column + 1).for_each(|cell| *cell = ' '),
                    _ => line.clear(),
                }
            }
            b'J' => match first {
                0 => {
                    self.lines[self.row].truncate(column);
                    self.lines.truncate(self.row + 1);
                }
                1 => {
                    for row in self.screen_top()..self.row {
                        self.lines[row].clear();
                    }
                    self.lines[self.row].iter_mut().take(column + 1).for_each(|cell| *cell = ' ');
                }
                // Clearing the screen keeps what was on it as scrollback.
                _ => {
                    let keep = self.lines.len();
                    self.lines.extend(std::iter::repeat_with(Vec::new).take(self.rows));
                    self.row = keep + (self.row - self.screen_top().min(self.row)).min(self.rows - 1);
                    self.row = self.row.min(self.lines.len() - 1);
                }
            },
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
                        line.insert(column, ' ');
                    }
                    line.truncate(self.columns);
                }
            }
            b'X' => {
                let line = &mut self.lines[self.row];
                for cell in line.iter_mut().skip(column).take(count) {
                    *cell = ' ';
                }
            }
            // Colors and other attributes, and everything else, are dropped.
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
        terminal.lines().iter().map(|line| line.iter().collect()).collect()
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
    fn moves_to_screen_positions() {
        let mut terminal = Terminal::new(10, 3);
        terminal.feed(b"a\r\nb\r\nc\r\nd");
        terminal.feed(b"\x1b[1;3HX");
        // The screen is the last three lines: b, c, d.
        assert_eq!(text(&terminal), ["a", "b X", "c", "d"]);
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
    fn keys_with_control() {
        assert_eq!(key_bytes('c', true), [3]);
        assert_eq!(key_bytes('D', true), [4]);
        assert_eq!(key_bytes('[', true), [0x1B]);
        assert_eq!(key_bytes('c', false), b"c");
        assert_eq!(key_bytes('1', true), b"1");
    }
}
