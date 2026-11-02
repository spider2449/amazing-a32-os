//! Minimal debug shell served over the USB serial console.

use std::fs;
use std::io::{self, BufRead, Write};
use std::thread;
use std::time::Duration;

const HELP: &str = "commands:
  help                         show this text
  info                         show kernel, SoC and memory
  ls PATH                      list a directory
  cat PATH                     print a file
  dmesg                        print the kernel log
  dump PATH OFFSET LENGTH      print a file range as base64
  reboot [bootloader|recovery] restart the phone
  reboot TARGET SECONDS        restart after a delay, to allow unplugging USB
  poweroff [SECONDS]           turn the phone off, optionally after a delay
";

/// Operations that need the real kernel, abstracted for host testing.
pub trait Platform {
    fn kernel_log(&self) -> io::Result<Vec<u8>>;
    /// Only returns if the restart request failed.
    fn reboot(&self, target: Option<&str>) -> io::Error;
    /// Only returns if the power off request failed.
    fn power_off(&self) -> io::Error;
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
    let words: Vec<&str> = line.split_whitespace().collect();
    let result = match words.as_slice() {
        [] => Ok(()),
        ["help"] => out.write_all(HELP.as_bytes()),
        ["info"] => out.write_all(crate::probe::report().as_bytes()),
        ["ls", path] => list(path, out),
        ["cat", path] => fs::read(path).and_then(|data| out.write_all(&data)),
        ["dmesg"] => platform.kernel_log().and_then(|log| out.write_all(&log)),
        ["dump", path, offset, length] => match (offset.parse(), length.parse()) {
            (Ok(offset), Ok(length)) => crate::dump::dump(path, offset, length, out),
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "bad number")),
        },
        ["reboot"] => Err(platform.reboot(None)),
        ["reboot", target @ ("bootloader" | "recovery")] => Err(platform.reboot(Some(target))),
        ["reboot", target @ ("bootloader" | "recovery"), delay] => match delay.parse() {
            // Blocking the console is fine: the delay exists to unplug USB.
            Ok(seconds) => {
                thread::sleep(Duration::from_secs(seconds));
                Err(platform.reboot(Some(target)))
            }
            Err(_) => Err(io::Error::new(io::ErrorKind::InvalidInput, "bad number")),
        },
        ["poweroff"] => Err(platform.power_off()),
        ["poweroff", delay] => match delay.parse() {
            // Blocking the console is fine: the delay exists to unplug USB.
            Ok(seconds) => {
                thread::sleep(Duration::from_secs(seconds));
                Err(platform.power_off())
            }
            Err(_) => Err(io::Error::new(io::ErrorKind::InvalidInput, "bad number")),
        },
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

        fn power_off(&self) -> io::Error {
            io::Error::other("fake power off")
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
    fn poweroff_takes_an_optional_delay() {
        assert!(session("poweroff\n").0.contains("fake power off"));
        assert!(session("poweroff 0\n").0.contains("fake power off"));
        assert!(session("poweroff now\n").0.contains("error: bad number"));
        assert!(session("poweroff 0 extra\n").0.contains("unknown command"));
    }

    #[test]
    fn dump_reports_bad_arguments_and_keeps_the_session() {
        let (out, _) = session("dump /no/such/file x 1\ndump /no/such/file 0 1\nhelp\n");
        assert!(out.contains("error: bad number"));
        assert!(out.contains("commands:"));
    }

    #[test]
    fn reboot_accepts_only_known_targets() {
        let (_, reboots) = session("reboot\nreboot bootloader\nreboot recovery\n");
        assert_eq!(
            reboots,
            vec![None, Some("bootloader".into()), Some("recovery".into())]
        );

        let (out, reboots) = session("reboot preloader\nreboot preloader 1\n");
        assert!(reboots.is_empty());
        assert!(out.contains("unknown command"));
    }

    #[test]
    fn delayed_reboot_needs_a_valid_delay() {
        let (_, reboots) = session("reboot recovery 0\n");
        assert_eq!(reboots, vec![Some("recovery".into())]);

        let (out, reboots) = session("reboot recovery soon\n");
        assert!(reboots.is_empty());
        assert!(out.contains("error: bad number"));
    }
}
