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

#[cfg(test)]
mod tests {
    use super::*;

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
