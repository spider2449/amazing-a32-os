//! Minimal debug shell served over the USB serial console.

use std::fs;
use std::io::{self, BufRead, Write};

const HELP: &str = "commands:
  help                         show this text
  info                         show kernel, SoC and memory
  ls PATH                      list a directory
  cat PATH                     print a file
  dmesg                        print the kernel log
  reboot [bootloader|recovery] restart the phone
";

/// Operations that need the real kernel, abstracted for host testing.
pub trait Platform {
    fn kernel_log(&self) -> io::Result<Vec<u8>>;
    /// Only returns if the restart request failed.
    fn reboot(&self, target: Option<&str>) -> io::Error;
}

/// Serves commands until the input ends.
pub fn run(input: impl BufRead, mut out: impl Write, platform: &impl Platform) -> io::Result<()> {
    write!(out, "a32> ")?;
    out.flush()?;
    for line in input.lines() {
        execute(line?.trim(), &mut out, platform)?;
        write!(out, "a32> ")?;
        out.flush()?;
    }
    Ok(())
}

fn execute(line: &str, out: &mut impl Write, platform: &impl Platform) -> io::Result<()> {
    let mut words = line.split_whitespace();
    let command = words.next();
    let argument = words.next();
    let result = match (command, argument, words.next()) {
        (None, ..) => Ok(()),
        (Some("help"), None, None) => out.write_all(HELP.as_bytes()),
        (Some("info"), None, None) => out.write_all(crate::probe::report().as_bytes()),
        (Some("ls"), Some(path), None) => list(path, out),
        (Some("cat"), Some(path), None) => fs::read(path).and_then(|data| out.write_all(&data)),
        (Some("dmesg"), None, None) => platform.kernel_log().and_then(|log| out.write_all(&log)),
        (Some("reboot"), target @ (None | Some("bootloader" | "recovery")), None) => {
            Err(platform.reboot(target))
        }
        _ => {
            writeln!(out, "unknown command or arguments, try: help")?;
            Ok(())
        }
    };
    if let Err(err) = result {
        writeln!(out, "error: {err}")?;
    }
    Ok(())
}

fn list(path: &str, out: &mut impl Write) -> io::Result<()> {
    let mut names: Vec<String> = fs::read_dir(path)?
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for name in names {
        writeln!(out, "{name}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Fake {
        reboots: RefCell<Vec<Option<String>>>,
    }

    impl Platform for Fake {
        fn kernel_log(&self) -> io::Result<Vec<u8>> {
            Ok(b"kernel line\n".to_vec())
        }

        fn reboot(&self, target: Option<&str>) -> io::Error {
            self.reboots.borrow_mut().push(target.map(String::from));
            io::Error::other("fake reboot")
        }
    }

    fn session(input: &str) -> (String, Vec<Option<String>>) {
        let fake = Fake::default();
        let mut out = Vec::new();
        run(input.as_bytes(), &mut out, &fake).unwrap();
        (String::from_utf8(out).unwrap(), fake.reboots.into_inner())
    }

    #[test]
    fn prints_prompt_and_returns_at_end_of_input() {
        assert_eq!(session("").0, "a32> ");
        assert_eq!(session("\n").0, "a32> a32> ");
    }

    #[test]
    fn runs_basic_commands() {
        assert!(session("help\n").0.contains("reboot"));
        assert!(session("dmesg\n").0.contains("kernel line"));
        assert!(session("info\n").0.contains("a32-init"));
    }

    #[test]
    fn file_errors_do_not_end_the_session() {
        let (out, _) = session("cat /no/such/file\nhelp\n");
        assert!(out.contains("error:"));
        assert!(out.contains("commands:"));
    }

    #[test]
    fn reboot_accepts_only_known_targets() {
        let (_, reboots) = session("reboot\nreboot bootloader\nreboot recovery\n");
        assert_eq!(
            reboots,
            vec![None, Some("bootloader".into()), Some("recovery".into())]
        );

        let (out, reboots) = session("reboot preloader\nreboot bootloader extra\n");
        assert!(reboots.is_empty());
        assert!(out.contains("unknown command"));
    }
}
