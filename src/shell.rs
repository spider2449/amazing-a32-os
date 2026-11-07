//! Minimal debug shell served over the USB serial console.

use crate::wifi::{self, Credentials};
use std::fs;
use std::io::{self, BufRead, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::thread;
use std::time::{Duration, Instant};

const TCP_TIMEOUT: Duration = Duration::from_secs(10);

const HELP: &str = "commands:
  help                         show this text
  info                         show kernel, SoC and memory
  ls PATH                      list a directory
  cat PATH                     print a file
  dmesg                        print the kernel log
  dump PATH OFFSET LENGTH      print a file range as base64
  reboot [bootloader|recovery] restart the phone
  reboot TARGET SECONDS        restart after a delay, to allow unplugging USB
  sh LINE                      run a line with the stock shell in /system
  alpine LINE                  run a line with the shell of Alpine in /data/alpine
  recv NEW_FILE PORT           save one connection's data; check it with dump
  dhcp INTERFACE               get an address for a network interface
  wifi                         show the Wi-Fi state
  wifi join SSID PASSPHRASE    join a WPA2 network and keep it for later starts;
                               use double quotes around a value with spaces
  wifi forget                  leave the network and erase its saved details
  tcp HOST:PORT                try a TCP connection
  bluetooth                    show the BLE scan and the devices heard
  bluetooth on|off             start or stop the BLE scan, kept for later starts
  bluetooth add|remove ADDRESS save a sensor to show and record, or remove it
  bench cpu|mem|disk PATH      measure performance
  bench write NEW_FILE         measure writes to a new file, then remove it
  bench display                measure drawing, with the screen off
  ui tap X Y                   act as if the screen were touched there
  bench net PORT [SECONDS]     measure a TCP client, one connection
  bench load SECONDS           load all CPUs and record temperatures
  camera rear|front NEW_FILE   take a photo and save it as a JPEG
  web password                 say whether the web cameras have a password
  web password NEW|off         set the password for the web cameras (10 to 64
                               printable characters, no spaces), or turn them off
  poweroff [SECONDS]           turn the phone off, optionally after a delay
";

/// Operations that need the real kernel, abstracted for host testing.
pub trait Platform {
    fn kernel_log(&self) -> io::Result<Vec<u8>>;
    /// Only returns if the restart request failed.
    fn reboot(&self, target: Option<&str>) -> io::Error;
    /// Only returns if the power off request failed.
    fn power_off(&self) -> io::Error;
    /// Runs a line with the phone's stock shell and returns its output.
    fn run_stock_shell(&self, line: &str) -> io::Result<Vec<u8>>;
    /// Runs a line with Alpine's shell and returns its output.
    fn run_alpine(&self, line: &str) -> io::Result<Vec<u8>>;
    /// Saves one incoming connection's data to a new file and returns its length.
    fn receive_file(&self, path: &str, port: u16) -> io::Result<u64>;
    /// Gets a DHCP lease on an interface, applies it and describes the result.
    fn dhcp(&self, interface: &str) -> io::Result<String>;
    /// Runs a performance measurement and returns its results.
    fn bench(&self, arguments: &[&str]) -> io::Result<String>;
    /// Acts as if the screen were touched at this position, then released.
    fn tap(&self, x: i32, y: i32) -> io::Result<()>;
    /// Describes the Wi-Fi state.
    fn wifi_status(&self) -> io::Result<String>;
    /// Joins a network, gets an address and describes the result.
    fn wifi_join(&self, credentials: &Credentials) -> io::Result<String>;
    /// Leaves the network and erases its saved details.
    fn wifi_forget(&self) -> io::Result<String>;
    /// Turns the BLE scan on or off and keeps that choice, or with `None` describes it.
    fn bluetooth(&self, switch: Option<bool>) -> io::Result<String>;
    /// Saves or removes a BLE sensor, by its address as text.
    fn bluetooth_sensor(&self, add: bool, address: &str) -> io::Result<String>;
    /// Takes a photo with the rear or front camera, saves it as a JPEG and
    /// describes it.
    fn photo(&self, camera: &str, path: &str) -> io::Result<String>;
    /// Sets the web password for the cameras, removes it with `Some("off")`, or
    /// with `None` says whether one is set. Never shows the password.
    fn web_password(&self, password: Option<&str>) -> io::Result<String>;
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

/// Runs one line, writing its output and any error to `out`. Fails only if `out` does.
pub fn execute(line: &str, out: &mut impl Write, platform: &impl Platform) -> io::Result<()> {
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
        ["sh", _, ..] => {
            let rest = line.strip_prefix("sh").unwrap_or(line).trim();
            platform.run_stock_shell(rest).and_then(|output| out.write_all(&output))
        }
        ["alpine", _, ..] => {
            let rest = line.strip_prefix("alpine").unwrap_or(line).trim();
            platform.run_alpine(rest).and_then(|output| out.write_all(&output))
        }
        ["recv", path, port] => match port.parse() {
            Ok(port) => platform
                .receive_file(path, port)
                .and_then(|bytes| writeln!(out, "received {bytes} bytes")),
            Err(_) => Err(io::Error::new(io::ErrorKind::InvalidInput, "bad number")),
        },
        ["dhcp", interface] => platform.dhcp(interface).and_then(|text| writeln!(out, "{text}")),
        ["wifi"] => platform.wifi_status().and_then(|text| writeln!(out, "{text}")),
        ["wifi", "forget"] => platform.wifi_forget().and_then(|text| writeln!(out, "{text}")),
        ["wifi", "join", ..] => {
            let rest = line.strip_prefix("wifi").unwrap_or(line).trim_start();
            let rest = rest.strip_prefix("join").unwrap_or(rest);
            wifi::parse_join(rest)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))
                .and_then(|credentials| platform.wifi_join(&credentials))
                .and_then(|text| writeln!(out, "{text}"))
        }
        ["bluetooth"] => platform.bluetooth(None).and_then(|text| writeln!(out, "{text}")),
        ["bluetooth", "on"] => platform.bluetooth(Some(true)).and_then(|text| writeln!(out, "{text}")),
        ["bluetooth", "off"] => platform.bluetooth(Some(false)).and_then(|text| writeln!(out, "{text}")),
        ["bluetooth", "add", address] => {
            platform.bluetooth_sensor(true, address).and_then(|text| writeln!(out, "{text}"))
        }
        ["bluetooth", "remove", address] => {
            platform.bluetooth_sensor(false, address).and_then(|text| writeln!(out, "{text}"))
        }
        ["camera", camera @ ("rear" | "front"), path] => {
            platform.photo(camera, path).and_then(|text| writeln!(out, "{text}"))
        }
        ["web", "password"] => platform.web_password(None).and_then(|text| writeln!(out, "{text}")),
        ["web", "password", password] => {
            platform.web_password(Some(password)).and_then(|text| writeln!(out, "{text}"))
        }
        ["tcp", address] => try_tcp(address, out),
        ["ui", "tap", x, y] => match (x.parse(), y.parse()) {
            (Ok(x), Ok(y)) => platform.tap(x, y),
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "bad number")),
        },
        ["bench", arguments @ ..] => {
            platform.bench(arguments).and_then(|text| out.write_all(text.as_bytes()))
        }
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

/// Reports whether a TCP connection opens. No data is sent.
fn try_tcp(address: &str, out: &mut impl Write) -> io::Result<()> {
    // Names are resolved first, so the time below only covers the connection.
    let target = address
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| io::Error::other("the name has no address"))?;
    let start = Instant::now();
    TcpStream::connect_timeout(&target, TCP_TIMEOUT)?;
    writeln!(out, "connected to {target} in {} ms", start.elapsed().as_millis())
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

        fn run_stock_shell(&self, line: &str) -> io::Result<Vec<u8>> {
            Ok(format!("ran [{line}]\n").into_bytes())
        }

        fn bench(&self, arguments: &[&str]) -> io::Result<String> {
            Ok(format!("bench {arguments:?}
"))
        }

        fn tap(&self, x: i32, y: i32) -> io::Result<()> {
            assert_eq!((x, y), (10, 20));
            Ok(())
        }

        fn run_alpine(&self, line: &str) -> io::Result<Vec<u8>> {
            Ok(format!("alpine ran [{line}]\n").into_bytes())
        }

        fn receive_file(&self, path: &str, port: u16) -> io::Result<u64> {
            assert_eq!((path, port), ("/new", 5202));
            Ok(7)
        }

        fn wifi_status(&self) -> io::Result<String> {
            Ok("wifi state".into())
        }

        fn wifi_join(&self, credentials: &Credentials) -> io::Result<String> {
            Ok(format!("joined {}", credentials.ssid))
        }

        fn wifi_forget(&self) -> io::Result<String> {
            Ok("forgotten".into())
        }

        fn bluetooth(&self, switch: Option<bool>) -> io::Result<String> {
            Ok(format!("bluetooth {switch:?}"))
        }

        fn bluetooth_sensor(&self, add: bool, address: &str) -> io::Result<String> {
            Ok(format!("sensor add={add} {address}"))
        }

        fn dhcp(&self, interface: &str) -> io::Result<String> {
            Ok(format!("lease on {interface}"))
        }

        fn photo(&self, camera: &str, path: &str) -> io::Result<String> {
            Ok(format!("photo {camera} {path}"))
        }

        fn web_password(&self, password: Option<&str>) -> io::Result<String> {
            Ok(format!("web password {password:?}"))
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
    fn sh_passes_the_rest_of_the_line_unchanged() {
        assert!(session("sh echo  'a b' > /x\n").0.contains("ran [echo  'a b' > /x]"));
        assert!(session("sh\n").0.contains("unknown command"));
    }

    #[test]
    fn dhcp_needs_one_interface() {
        assert!(session("dhcp wlan0\n").0.contains("lease on wlan0"));
        assert!(session("dhcp\n").0.contains("unknown command"));
    }

    #[test]
    fn wifi_join_checks_its_arguments() {
        assert!(session("wifi\n").0.contains("wifi state"));
        assert!(session("wifi join net password1\n").0.contains("joined 6e6574"));
        assert!(session("wifi  join  \"a b\"  password1\n").0.contains("joined 612062"));
        assert!(session("wifi join net short\n").0.contains("error: the passphrase"));
        assert!(session("wifi join\n").0.contains("error: usage"));
        assert!(session("wifi forget\n").0.contains("forgotten"));
        assert!(session("wifi leave\n").0.contains("unknown command"));
    }

    #[test]
    fn bluetooth_takes_on_or_off() {
        assert!(session("bluetooth\n").0.contains("bluetooth None"));
        assert!(session("bluetooth on\n").0.contains("bluetooth Some(true)"));
        assert!(session("bluetooth off\n").0.contains("bluetooth Some(false)"));
        assert!(session("bluetooth maybe\n").0.contains("unknown command"));
        assert!(session("bluetooth add 58:2d:34:36:74:93\n").0.contains("sensor add=true 58:2d:34:36:74:93"));
        assert!(session("bluetooth remove 58:2d:34:36:74:93\n").0.contains("sensor add=false 58:2d:34:36:74:93"));
        assert!(session("bluetooth add\n").0.contains("unknown command"));
    }

    #[test]
    fn web_password_takes_one_word() {
        assert!(session("web password\n").0.contains("web password None"));
        assert!(session("web password s3cret-value\n").0.contains("web password Some(\"s3cret-value\")"));
        assert!(session("web password off\n").0.contains("web password Some(\"off\")"));
        assert!(session("web password two words\n").0.contains("unknown command"));
    }

    #[test]
    fn camera_takes_a_side_and_a_file() {
        assert!(session("camera rear /data/a.jpg
").0.contains("photo rear /data/a.jpg"));
        assert!(session("camera front /data/b.jpg
").0.contains("photo front /data/b.jpg"));
        assert!(session("camera back /data/a.jpg
").0.contains("unknown command"));
        assert!(session("camera rear
").0.contains("unknown command"));
    }

    #[test]
    fn bench_passes_its_arguments() {
        assert!(session("bench disk /dev/system\n").0.contains(r#"["disk", "/dev/system"]"#));
    }

    #[test]
    fn alpine_passes_the_rest_of_the_line_unchanged() {
        assert!(session("alpine echo  'a b'\n").0.contains("alpine ran [echo  'a b']"));
        assert!(session("alpine\n").0.contains("unknown command"));
    }

    #[test]
    fn tap_needs_a_position() {
        assert_eq!(session("ui tap 10 20\n").0, "a32> a32> ");
        assert!(session("ui tap 10\n").0.contains("unknown command"));
        assert!(session("ui tap x 20\n").0.contains("error: bad number"));
    }

    #[test]
    fn recv_needs_a_valid_port() {
        assert!(session("recv /new 5202\n").0.contains("received 7 bytes"));
        assert!(session("recv /new soon\n").0.contains("error: bad number"));
        assert!(session("recv /new\n").0.contains("unknown command"));
    }

    #[test]
    fn tcp_reports_the_connection() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        assert!(session(&format!("tcp {address}\n")).0.contains("connected to"));
        drop(listener);
        assert!(session(&format!("tcp {address}\n")).0.contains("error:"));
        assert!(session("tcp nowhere\n").0.contains("error:"));
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
