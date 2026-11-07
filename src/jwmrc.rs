//! Changes to Alpine's JWM settings that make the desktop work by touch. This only
//! edits text, so it runs on the host in unit tests.

/// The on-screen keyboard's program, and the script that shows or hides it. /run
/// is shared with the chroot. BusyBox's `pkill -x` matches this whole name, not
/// the 15 characters the kernel keeps.
pub const KEYBOARD: &str = "svkbd-mobile-intl";
pub const KEYBOARD_SCRIPT: &str = "/run/a32-keyboard";
const KEYBOARD_CLASS: &str = "svkbd";
const KEYBOARD_HEIGHT: u32 = 300;
/// JWM's tray at the bottom of the screen, which the keyboard sits above.
const TRAY_HEIGHT: u32 = 34;
const KEYBOARD_FONT: &str = "DejaVu Sans:bold:size=12";

/// Where the keyboard's top edge is on a screen this high.
fn keyboard_top(height: u32) -> u32 {
    height.saturating_sub(TRAY_HEIGHT + KEYBOARD_HEIGHT)
}

/// The script that shows the keyboard, or hides it if it shows.
pub fn keyboard_script(width: u32, height: u32) -> String {
    format!(
        "#!/bin/sh\n\
         # Shows the on-screen keyboard, or hides it if it shows.\n\
         pkill -x {KEYBOARD} && exit 0\n\
         exec {KEYBOARD} -g {width}x{KEYBOARD_HEIGHT}+0+{top} -fn '{KEYBOARD_FONT}'\n",
        top = keyboard_top(height)
    )
}

/// Alpine's settings with these changes, or `None` if they do not have the root
/// menu this expects:
///
/// - The menu on the empty desktop opens with the right button only. With the left
///   one too, a drag that started on the desktop opened the menu first. Tray
///   buttons and keys name the menu by a button, such as `root:1`, so they follow.
/// - Focus moves on a click. With focus following the pointer, a finger on the
///   keyboard took the focus away from the window being typed in.
/// - The keyboard stays above the tray, without a title, above other windows, and
///   never takes the focus. A KB button in the tray shows or hides it.
pub fn for_touch(text: &str, height: u32) -> Option<String> {
    const MENU: &str = "<RootMenu onroot=\"";
    let start = text.find(MENU)? + MENU.len();
    let end = start + text[start..].find('"')?;
    let mut changed = format!("{}3{}", &text[..start], &text[end..]);
    for button in text[start..end].chars() {
        changed = changed.replace(&format!(">root:{button}<"), ">root:3<");
    }

    const FOCUS_OPEN: &str = "<FocusModel>";
    const FOCUS_CLOSE: &str = "</FocusModel>";
    let focus = changed
        .find(FOCUS_OPEN)
        .and_then(|open| Some((open + FOCUS_OPEN.len(), open + changed[open..].find(FOCUS_CLOSE)?)));
    let added_focus = match focus {
        Some((open, close)) => {
            changed.replace_range(open..close, "click");
            ""
        }
        None => "<FocusModel>click</FocusModel>",
    };

    const TRAY_BUTTON_END: &str = "</TrayButton>";
    if let Some(at) = changed.find(TRAY_BUTTON_END) {
        let at = at + TRAY_BUTTON_END.len();
        changed.insert_str(at, &format!("<TrayButton label=\"KB\">exec:/bin/sh {KEYBOARD_SCRIPT}</TrayButton>"));
    }

    let options = [
        "notitle".to_string(),
        "noborder".to_string(),
        "nofocus".to_string(),
        "layer:above".to_string(),
        "sticky".to_string(),
        "nolist".to_string(),
        "x:0".to_string(),
        format!("y:{}", keyboard_top(height)),
    ];
    let options: String = options.iter().map(|option| format!("<Option>{option}</Option>")).collect();
    let group = format!("{added_focus}<Group><Class>{KEYBOARD_CLASS}</Class>{options}</Group>\n</JWM>");
    let close = changed.rfind("</JWM>")?;
    changed.replace_range(close..close + "</JWM>".len(), &group);
    Some(changed)
}

/// Alpine's settings without what starts a program that is not installed, as told
/// by `installed`. Alpine's menu lists Firefox, Gimp, xcalc and others, and its tray
/// holds xclock, none of which come with JWM. A program installed later shows again.
///
/// - Menu entries whose command, or any command in its pipeline, is missing.
/// - Submenus left empty, and separators left at an edge or next to another.
/// - Tray programs (`Swallow`) and clock buttons (`exec:`) that are missing.
pub fn without_missing(text: &str, installed: impl Fn(&str) -> bool) -> String {
    let runs = |command: &str| commands(command).iter().all(|name| installed(name));
    let mut text = remove_elements(text, "Program", |_, content| runs(content));
    text = remove_elements(&text, "Swallow", |_, content| runs(content));
    text = remove_elements(&text, "Button", |_, content| match content.trim().strip_prefix("exec:") {
        Some(command) => runs(command),
        None => true,
    });
    while let Some(range) = empty_menu(&text) {
        remove_line(&mut text, range);
    }
    while let Some(range) = extra_separator(&text) {
        remove_line(&mut text, range);
    }
    text
}

/// The programs a shell command line runs: the first word of each part of a
/// pipeline or list.
fn commands(line: &str) -> Vec<&str> {
    line.split(['|', ';', '&'])
        .filter_map(|part| part.split_whitespace().next())
        .collect()
}

/// Where the element named `tag`, with its content, starts and ends in `text`,
/// searched from `from`. Only elements with content are found, not `<tag/>`.
fn find_element(text: &str, tag: &str, from: usize) -> Option<(usize, usize, usize, usize)> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut at = from;
    loop {
        let start = at + text[at..].find(&open)?;
        let after = start + open.len();
        // `<Program` must not match `<ProgramX`.
        if !text[after..].starts_with([' ', '>', '\t', '\n']) {
            at = after;
            continue;
        }
        let content_start = after + text[after..].find('>')? + 1;
        if text[..content_start].ends_with("/>") {
            at = content_start;
            continue;
        }
        let content_end = content_start + text[content_start..].find(&close)?;
        return Some((start, content_start, content_end, content_end + close.len()));
    }
}

/// `text` without each element named `tag` for which `keep(attributes, content)`
/// is false.
fn remove_elements(text: &str, tag: &str, keep: impl Fn(&str, &str) -> bool) -> String {
    let mut text = text.to_string();
    let mut at = 0;
    while let Some((start, content_start, content_end, end)) = find_element(&text, tag, at) {
        let attributes = &text[start + tag.len() + 1..content_start - 1];
        if keep(attributes, &text[content_start..content_end]) {
            at = end;
        } else {
            at = remove_line(&mut text, start..end);
        }
    }
    text
}

/// A submenu with no entries, only separators, if there is one. Only the
/// innermost submenus are checked, so an outer one is found once its last inner
/// one is removed.
fn empty_menu(text: &str) -> Option<std::ops::Range<usize>> {
    let mut at = 0;
    while let Some((start, content_start, content_end, end)) = find_element(text, "Menu", at) {
        let content = &text[content_start..content_end];
        if !content.contains("<Menu") && content.replace("<Separator/>", "").trim().is_empty() {
            return Some(start..end);
        }
        at = content_start;
    }
    None
}

/// A separator first or last in a menu, or right after another, if there is one.
fn extra_separator(text: &str) -> Option<std::ops::Range<usize>> {
    const SEPARATOR: &str = "<Separator/>";
    let mut at = 0;
    while let Some(found) = text[at..].find(SEPARATOR) {
        let start = at + found;
        let end = start + SEPARATOR.len();
        let before = text[..start].trim_end();
        let last_tag = &before[before.rfind('<').unwrap_or(0)..];
        let first = last_tag.starts_with("<Menu") || last_tag.starts_with("<RootMenu");
        let after = text[end..].trim_start();
        let last = after.starts_with("</Menu>") || after.starts_with("</RootMenu>");
        if first || last || before.ends_with(SEPARATOR) {
            return Some(start..end);
        }
        at = end;
    }
    None
}

/// Removes `range` from `text`, with its line when nothing else is on it. Returns
/// where the text after it now starts.
fn remove_line(text: &mut String, range: std::ops::Range<usize>) -> usize {
    let line_start = text[..range.start].rfind('\n').map_or(0, |at| at + 1);
    let alone_before = text[line_start..range.start].trim().is_empty();
    let line_end = text[range.end..].find('\n').map_or(text.len(), |at| range.end + at + 1);
    let alone_after = text[range.end..line_end].trim().is_empty();
    let range = if alone_before && alone_after { line_start..line_end } else { range };
    let start = range.start;
    text.replace_range(range, "");
    start
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Alpine's menu and tray as they come, shortened.
    const MENUS: &str = r#"<JWM>
    <RootMenu onroot="12">
        <Program icon="utilities-terminal" label="Terminal">xterm</Program>
        <Menu icon="folder" label="Applications">
            <Program icon="calc" label="Calculator">xcalc</Program>
            <Program icon="system-file-manager" label="ROX">rox ~</Program>
        </Menu>
        <Menu icon="folder" label="Utilities">
            <Program icon="info" label="Window Properties">
                xprop | xmessage -file -
            </Program>
        </Menu>
        <Separator/>
        <Program icon="lock" label="Lock">
           xscreensaver-command -lock
        </Program>
        <Separator/>
        <Restart label="Restart" icon="reload"/>
        <Exit label="Exit" confirm="true" icon="exit"/>
    </RootMenu>
    <Tray x="0" y="-1">
        <Swallow width="32" height="32" name="xclock">xclock</Swallow>
        <Clock format="%l:%M %p"><Button mask="123">exec:xclock</Button></Clock>
    </Tray>
</JWM>
"#;

    #[test]
    fn missing_programs_leave_the_menu_and_tray() {
        let changed = without_missing(MENUS, |name| name == "xterm");
        assert_eq!(
            changed,
            r#"<JWM>
    <RootMenu onroot="12">
        <Program icon="utilities-terminal" label="Terminal">xterm</Program>
        <Separator/>
        <Restart label="Restart" icon="reload"/>
        <Exit label="Exit" confirm="true" icon="exit"/>
    </RootMenu>
    <Tray x="0" y="-1">
        <Clock format="%l:%M %p"></Clock>
    </Tray>
</JWM>
"#
        );
    }

    #[test]
    fn installed_programs_stay() {
        let all = without_missing(MENUS, |_| true);
        assert_eq!(all, MENUS);
        // A pipeline shows only when every program in it is installed.
        let some = without_missing(MENUS, |name| ["xterm", "xcalc", "xprop"].contains(&name));
        assert!(some.contains(">xcalc</Program>"));
        assert!(!some.contains("rox ~"));
        assert!(!some.contains("xprop"));
        assert!(!some.contains("Utilities"));
        assert!(some.contains("Applications"));
    }

    #[test]
    fn nested_menus_go_when_empty() {
        let nested = "<RootMenu onroot=\"3\">\n<Menu label=\"A\">\n<Menu label=\"B\">\n<Program>gone</Program>\n</Menu>\n<Separator/>\n</Menu>\n<Program>xterm</Program>\n</RootMenu>\n";
        assert_eq!(
            without_missing(nested, |name| name == "xterm"),
            "<RootMenu onroot=\"3\">\n<Program>xterm</Program>\n</RootMenu>\n"
        );
    }

    const ALPINE: &str = r#"<?xml version="1.0"?>
<JWM>
    <RootMenu onroot="12">
        <Program label="Terminal">xterm</Program>
    </RootMenu>
    <Tray x="0" y="-1" autohide="off">
        <TrayButton label="JWM">root:1</TrayButton>
        <Spacer width="2"/>
        <TrayButton label="_">showdesktop</TrayButton>
    </Tray>
    <FocusModel>sloppy</FocusModel>
    <Key mask="A" key="F1">root:1</Key>
    <Key mask="A" key="F2">root:2</Key>
</JWM>
"#;

    #[test]
    fn menu_opens_with_the_right_button_everywhere() {
        let changed = for_touch(ALPINE, 854).unwrap();
        assert!(changed.contains(r#"<RootMenu onroot="3">"#));
        assert!(changed.contains(r#"<TrayButton label="JWM">root:3</TrayButton>"#));
        assert!(changed.contains(r#"key="F1">root:3</Key>"#));
        assert!(changed.contains(r#"key="F2">root:3</Key>"#));
        assert!(!changed.contains("root:1"));
    }

    #[test]
    fn focus_moves_on_click() {
        let changed = for_touch(ALPINE, 854).unwrap();
        assert!(changed.contains("<FocusModel>click</FocusModel>"));
        assert!(!changed.contains("sloppy"));
        // Settings without a focus model get one.
        let without = ALPINE.replace("    <FocusModel>sloppy</FocusModel>\n", "");
        assert_eq!(for_touch(&without, 854).unwrap().matches("<FocusModel>click</FocusModel>").count(), 1);
    }

    #[test]
    fn keyboard_sits_above_the_tray_with_a_button() {
        let changed = for_touch(ALPINE, 854).unwrap();
        assert!(changed.contains("<Class>svkbd</Class>"));
        assert!(changed.contains("<Option>nofocus</Option><Option>layer:above</Option>"));
        assert!(changed.contains("<Option>y:520</Option></Group>\n</JWM>"));
        assert!(changed.contains(
            r#"root:3</TrayButton><TrayButton label="KB">exec:/bin/sh /run/a32-keyboard</TrayButton>"#
        ));
        assert_eq!(changed.matches("</JWM>").count(), 1);
        assert!(keyboard_script(480, 854).contains("-g 480x300+0+520 -fn 'DejaVu Sans:bold:size=12'"));
    }

    #[test]
    fn unknown_settings_are_left_alone() {
        assert_eq!(for_touch("<JWM></JWM>", 854), None);
        assert_eq!(for_touch(r#"<RootMenu onroot="1">"#, 854), None);
    }
}
