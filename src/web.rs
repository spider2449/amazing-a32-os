//! A read-only status page for the local network: what the panel's home page shows,
//! the end of the persistent log, and photos and live video from the cameras. The
//! status needs no login; the cameras need the web password (HTTP Basic, any user
//! name) and stay off until one is set on the console. Plain HTTP sends the
//! password readably, so this keeps out other people on the network only if they
//! do not watch its traffic. The firewall only lets private addresses reach it.
//! Everything here is text in and bytes out, so it runs on the host in unit tests.

use crate::bluetooth::{address_text, Device};
use crate::sensors::centi_text;
#[cfg(test)]
use crate::sensors::{Format, Reading};
use crate::ui::Snapshot;
use std::time::Duration;

pub const PORT: u16 = 80;
/// The same port, for firewall rules.
pub const PORT_TEXT: &str = "80";

/// A request line longer than this, or headers past `REQUEST_LIMIT`, are refused.
pub const REQUEST_LIMIT: usize = 4096;
// The page reloads itself this often, in seconds.
const REFRESH_SECONDS: u32 = 15;

#[derive(Debug, PartialEq)]
pub enum Route {
    Page,
    Json,
    Bluetooth,
    /// A new reading of the phone's own sensors.
    Sensors,
    /// A new photo from the named camera, "rear" or "front".
    Photo(&'static str),
    /// Live video from the named camera, as MJPEG.
    Live(&'static str),
    NotFound,
    MethodNotAllowed,
    BadRequest,
}

/// Reads the first line of a request, such as `GET / HTTP/1.1`.
pub fn route(request: &str) -> Route {
    let line = request.lines().next().unwrap_or_default();
    let mut parts = line.split(' ');
    let (Some(method), Some(target), Some(version)) = (parts.next(), parts.next(), parts.next()) else {
        return Route::BadRequest;
    };
    if parts.next().is_some() || !version.starts_with("HTTP/") {
        return Route::BadRequest;
    }
    if method != "GET" {
        return Route::MethodNotAllowed;
    }
    // A query string changes nothing, but browsers and scripts may add one.
    match target.split('?').next().unwrap_or_default() {
        "/" | "/index.html" => Route::Page,
        "/status.json" => Route::Json,
        "/bluetooth.json" => Route::Bluetooth,
        "/sensors.json" => Route::Sensors,
        "/camera/rear.jpg" => Route::Photo("rear"),
        "/camera/front.jpg" => Route::Photo("front"),
        "/camera/rear.mjpg" => Route::Live("rear"),
        "/camera/front.mjpg" => Route::Live("front"),
        _ => Route::NotFound,
    }
}

/// The rows both formats show, as (JSON key, label, value).
fn rows(status: &Snapshot) -> [(&'static str, &'static str, &str); 12] {
    [
        ("address", "Address", &status.address),
        ("network", "Network", &status.network),
        ("signal", "Signal", &status.signal),
        ("ssh", "SSH", &status.ssh),
        ("uptime", "Uptime", &status.uptime),
        ("load", "Load average", &status.load),
        ("cpu_temperature", "CPU temperature", &status.cpu_temperature),
        ("memory", "Memory", &status.memory),
        ("disk", "Disk /data", &status.disk),
        ("battery", "Battery", &status.battery),
        ("battery_temperature", "Battery temperature", &status.battery_temperature),
        ("bluetooth", "Bluetooth", &status.bluetooth),
    ]
}

/// A sensor's latest values in words, such as `23.40 C, 45.60%, battery 93%, 5 s ago`.
pub fn sensor_summary(device: &Device, now: Duration) -> String {
    let Some((reading, updated)) = &device.sensor else {
        return String::new();
    };
    let mut parts = Vec::new();
    if let Some(value) = reading.temperature_centi {
        parts.push(format!("{} C", centi_text(value.into())));
    }
    if let Some(value) = reading.humidity_centi {
        parts.push(format!("{}%", centi_text(value.into())));
    }
    if let Some(value) = reading.battery_percent {
        parts.push(format!("battery {value}%"));
    }
    parts.push(format!("{} s ago", now.saturating_sub(*updated).as_secs()));
    parts.join(", ")
}

/// The devices the BLE scan heard, strongest first, for `/bluetooth.json`.
pub fn bluetooth_json(state: &str, devices: &[&Device], now: Duration) -> String {
    let items: Vec<String> = devices
        .iter()
        .map(|device| {
            format!(
                "{{\"address\":\"{}\",\"random\":{},\"name\":{},\"kind\":{},\"rssi\":{},\"seconds_ago\":{},\"reports\":{},\"encrypted\":{},\"sensor\":{}}}",
                address_text(&device.address),
                device.random,
                device.name.as_deref().map_or("null".to_string(), escape_json),
                device.kind.as_deref().map_or("null".to_string(), escape_json),
                device.rssi,
                now.saturating_sub(device.last_seen).as_secs(),
                device.reports,
                device.encrypted,
                device.sensor.as_ref().map_or("null".to_string(), |(reading, updated)| {
                    let number = |value: Option<String>| value.unwrap_or_else(|| "null".to_string());
                    format!(
                        "{{\"format\":\"{}\",\"temperature_c\":{},\"humidity_percent\":{},\"battery_percent\":{},\"seconds_ago\":{}}}",
                        reading.format.name(),
                        number(reading.temperature_centi.map(|value| centi_text(value.into()))),
                        number(reading.humidity_centi.map(|value| centi_text(value.into()))),
                        number(reading.battery_percent.map(|value| value.to_string())),
                        now.saturating_sub(*updated).as_secs(),
                    )
                }),
            )
        })
        .collect();
    format!("{{\"state\":{},\"devices\":[{}]}}\n", escape_json(state), items.join(","))
}

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(character),
        }
    }
    out
}

fn escape_json(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if u32::from(control) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(control))),
            _ => out.push(character),
        }
    }
    out.push('"');
    out
}

/// The status page. `sensors` holds a label and a summary for each BLE sensor, and
/// the section is left out without any. Everything is shown after escaping.
pub fn page(status: &Snapshot, time: &str, sensors: &[(String, String)], log_tail: &str) -> String {
    let mut table = String::new();
    for (_, label, value) in rows(status) {
        table.push_str(&format!("<tr><th>{label}</th><td>{}</td></tr>\n", escape_html(value)));
    }
    let mut sensor_section = String::new();
    if !sensors.is_empty() {
        sensor_section.push_str("<h2>Sensors</h2>\n<table>\n");
        for (label, summary) in sensors {
            sensor_section
                .push_str(&format!("<tr><td>{}</td><td>{}</td></tr>\n", escape_html(label), escape_html(summary)));
        }
        sensor_section.push_str("</table>\n");
    }
    format!(
        "<!doctype html>
<html lang=\"en\">
<head>
<meta charset=\"utf-8\">
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
<meta http-equiv=\"refresh\" content=\"{REFRESH_SECONDS}\">
<title>A32 status</title>
<style>
:root {{ color-scheme: light dark; --bg: #f6f7f9; --fg: #1c2330; --muted: #5d6878; --line: #d9dee5; }}
@media (prefers-color-scheme: dark) {{ :root {{ --bg: #121722; --fg: #e4e8ee; --muted: #93a0b2; --line: #2a3344; }} }}
body {{ margin: 0; padding: 16px; background: var(--bg); color: var(--fg); font: 15px/1.45 system-ui, sans-serif; }}
main {{ max-width: 760px; margin: 0 auto; }}
h1 {{ font-size: 20px; margin: 0 0 4px; }}
h2 {{ font-size: 16px; margin: 24px 0 8px; }}
p {{ margin: 0; color: var(--muted); }}
table {{ border-collapse: collapse; width: 100%; margin-top: 12px; }}
th, td {{ text-align: left; padding: 6px 8px; border-bottom: 1px solid var(--line); vertical-align: top; }}
th {{ width: 40%; color: var(--muted); font-weight: 500; }}
pre {{ margin: 0; padding: 12px; border: 1px solid var(--line); border-radius: 6px; overflow-x: auto;
       font: 12px/1.4 ui-monospace, monospace; white-space: pre; }}
</style>
</head>
<body>
<main>
<h1>Amazing A32</h1>
<p>{} · refreshes every {REFRESH_SECONDS} s · <a href=\"/status.json\">JSON</a> · <a href=\"/bluetooth.json\">Bluetooth devices</a> · <a href=\"/sensors.json\">phone sensors</a> · photo: <a href=\"/camera/rear.jpg\">rear</a>, <a href=\"/camera/front.jpg\">front</a> · live: <a href=\"/camera/rear.mjpg\">rear</a>, <a href=\"/camera/front.mjpg\">front</a></p>
<table>
{table}</table>
{sensor_section}<h2>Log, newest last</h2>
<pre>{}</pre>
</main>
</body>
</html>
",
        escape_html(time),
        escape_html(log_tail),
    )
}

pub fn json(status: &Snapshot, time: &str) -> String {
    let mut fields = vec![format!("\"time\":{}", escape_json(time))];
    for (key, _, value) in rows(status) {
        fields.push(format!("\"{key}\":{}", escape_json(value)));
    }
    format!("{{{}}}\n", fields.join(","))
}

/// A whole response. The connection is closed after it, so no keep-alive is offered.
pub fn response(status: &str, content_type: &str, body: &str) -> Vec<u8> {
    response_bytes(status, content_type, body.as_bytes())
}

/// The same with a binary body, such as a photo.
pub fn response_bytes(status: &str, content_type: &str, body: &[u8]) -> Vec<u8> {
    let mut reply = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    reply.extend_from_slice(body);
    reply
}

/// The start of an MJPEG reply: each frame follows as one `live_part`.
pub fn live_header() -> &'static [u8] {
    b"HTTP/1.1 200 OK\r\nContent-Type: multipart/x-mixed-replace; boundary=frame\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n"
}

/// One frame of an MJPEG reply.
pub fn live_part(jpeg: &[u8]) -> Vec<u8> {
    let mut part = format!("--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n", jpeg.len()).into_bytes();
    part.extend_from_slice(jpeg);
    part.extend_from_slice(b"\r\n");
    part
}

/// The reply that asks a browser for the camera password.
pub fn unauthorized() -> Vec<u8> {
    let body = "the cameras need the web password\n";
    format!(
        "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"A32 cameras\", charset=\"UTF-8\"\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/// Shortest and longest password `password_problem` accepts.
pub const PASSWORD_MIN: usize = 10;
pub const PASSWORD_MAX: usize = 64;

/// Why `password` cannot be the web password, or `None` if it can: it must be
/// printable ASCII without spaces (so it can be typed on the console and sent in
/// a header) and from `PASSWORD_MIN` to `PASSWORD_MAX` characters.
pub fn password_problem(password: &str) -> Option<String> {
    if !(PASSWORD_MIN..=PASSWORD_MAX).contains(&password.len()) {
        return Some(format!("use {PASSWORD_MIN} to {PASSWORD_MAX} characters"));
    }
    if !password.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Some("use printable ASCII characters without spaces".to_string());
    }
    None
}

/// Whether the request's Basic credentials carry `password`, whatever the user
/// name. The comparison takes the same time wherever the first difference is.
pub fn authorized(request: &str, password: &str) -> bool {
    let Some(given) = basic_password(request) else { return false };
    let (given, expected) = (given.as_bytes(), password.as_bytes());
    let mut difference = given.len() ^ expected.len();
    for (index, &byte) in expected.iter().enumerate() {
        difference |= usize::from(byte ^ given.get(index).copied().unwrap_or(0));
    }
    difference == 0 && !expected.is_empty()
}

/// The password from an `Authorization: Basic` header.
fn basic_password(request: &str) -> Option<String> {
    let value = request.lines().skip(1).take_while(|line| !line.is_empty()).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim().eq_ignore_ascii_case("authorization").then_some(value.trim())
    })?;
    let (scheme, encoded) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("basic") {
        return None;
    }
    let decoded = String::from_utf8(base64_decode(encoded.trim())?).ok()?;
    decoded.split_once(':').map(|(_, password)| password.to_string())
}

/// Standard base64 with padding, as browsers send it.
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let text = text.trim_end_matches('=');
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut bits, mut count) = (0u32, 0);
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(value);
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
        }
    }
    Some(out)
}

/// The last `count` lines of `text`.
pub fn last_lines(text: &str, count: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(count)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status() -> Snapshot {
        Snapshot {
            address: "192.168.1.104".to_string(),
            network: "Home <\"Wi-Fi\"> & more".to_string(),
            battery: "100% Full".to_string(),
            ..Snapshot::default()
        }
    }

    #[test]
    fn routes_requests() {
        assert_eq!(route("GET / HTTP/1.1\r\nHost: x\r\n\r\n"), Route::Page);
        assert_eq!(route("GET /index.html HTTP/1.0\r\n\r\n"), Route::Page);
        assert_eq!(route("GET /status.json?x=1 HTTP/1.1\r\n\r\n"), Route::Json);
        assert_eq!(route("GET /sensors.json HTTP/1.1\r\n\r\n"), Route::Sensors);
        assert_eq!(route("GET /camera/rear.jpg HTTP/1.1\r\n\r\n"), Route::Photo("rear"));
        assert_eq!(route("GET /camera/front.jpg?t=1 HTTP/1.1\r\n\r\n"), Route::Photo("front"));
        assert_eq!(route("GET /camera/side.jpg HTTP/1.1\r\n\r\n"), Route::NotFound);
        assert_eq!(route("GET /camera/rear.mjpg HTTP/1.1\r\n\r\n"), Route::Live("rear"));
        assert_eq!(route("GET /camera/front.mjpg HTTP/1.1\r\n\r\n"), Route::Live("front"));
        assert_eq!(route("GET /etc/passwd HTTP/1.1\r\n\r\n"), Route::NotFound);
        assert_eq!(route("GET /../ HTTP/1.1\r\n\r\n"), Route::NotFound);
        assert_eq!(route("POST / HTTP/1.1\r\n\r\n"), Route::MethodNotAllowed);
        assert_eq!(route("GET /\r\n\r\n"), Route::BadRequest);
        assert_eq!(route("GET / HTTP/1.1 extra\r\n\r\n"), Route::BadRequest);
        assert_eq!(route("GET / SMTP\r\n\r\n"), Route::BadRequest);
        assert_eq!(route(""), Route::BadRequest);
    }

    #[test]
    fn page_escapes_values_and_the_log() {
        let html = page(&status(), "2026-11-06T12:00:00Z", &[], "[1.0] <script>alert(1)</script>");
        assert!(!html.contains("<h2>Sensors</h2>"));
        assert!(html.contains("Home &lt;&quot;Wi-Fi&quot;&gt; &amp; more"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("<td>192.168.1.104</td>"));
    }

    #[test]
    fn json_is_escaped_and_has_every_field() {
        let mut status = status();
        status.memory = "line\nbreak \\ tab\t\u{1}".to_string();
        let text = json(&status, "now");
        assert!(text.starts_with("{\"time\":\"now\",\"address\":\"192.168.1.104\""));
        assert!(text.contains("\"network\":\"Home <\\\"Wi-Fi\\\"> & more\""));
        assert!(text.contains("\"memory\":\"line\\nbreak \\\\ tab\\t\\u0001\""));
        for key in ["signal", "ssh", "uptime", "load", "cpu_temperature", "disk", "battery", "battery_temperature"] {
            assert!(text.contains(&format!("\"{key}\":")), "{key}");
        }
        assert!(text.ends_with("}\n"));
    }

    #[test]
    fn response_counts_bytes_not_characters() {
        let bytes = response("200 OK", "text/plain; charset=utf-8", "°C");
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Content-Length: 3\r\n"));
        assert!(text.ends_with("\r\n\r\n°C"));
    }

    #[test]
    fn lists_bluetooth_devices_with_escaped_names() {
        let named = Device {
            address: [0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
            random: false,
            name: Some("Room \"A\"".to_string()),
            kind: Some("Xiaomi".to_string()),
            rssi: -65,
            last_seen: Duration::from_secs(90),
            reports: 41,
            sensor: Some((
                Reading { format: Format::MiBeacon, temperature_centi: Some(-520), humidity_centi: Some(4560), battery_percent: Some(93) },
                Duration::from_secs(95),
            )),
            encrypted: false,
        };
        let unnamed = Device { address: [1, 2, 3, 4, 5, 6], random: true, name: None, kind: None, rssi: -80, last_seen: Duration::from_secs(99), reports: 1, sensor: None, encrypted: true };
        let text = bluetooth_json("scanning", &[&named, &unnamed], Duration::from_secs(100));
        assert_eq!(
            text,
            "{\"state\":\"scanning\",\"devices\":[\
             {\"address\":\"11:22:33:44:55:66\",\"random\":false,\"name\":\"Room \\\"A\\\"\",\"kind\":\"Xiaomi\",\"rssi\":-65,\"seconds_ago\":10,\"reports\":41,\"encrypted\":false,\
             \"sensor\":{\"format\":\"mibeacon\",\"temperature_c\":-5.20,\"humidity_percent\":45.60,\"battery_percent\":93,\"seconds_ago\":5}},\
             {\"address\":\"06:05:04:03:02:01\",\"random\":true,\"name\":null,\"kind\":null,\"rssi\":-80,\"seconds_ago\":1,\"reports\":1,\"encrypted\":true,\"sensor\":null}]}\n"
        );
        assert_eq!(sensor_summary(&named, Duration::from_secs(100)), "-5.20 C, 45.60%, battery 93%, 5 s ago");
        let rows = [("Room \"A\" 11:22:33:44:55:66".to_string(), sensor_summary(&named, Duration::from_secs(100)))];
        let html = page(&status(), "now", &rows, "");
        assert!(html.contains("<h2>Sensors</h2>"));
        assert!(html.contains("<td>Room &quot;A&quot; 11:22:33:44:55:66</td><td>-5.20 C, 45.60%, battery 93%, 5 s ago</td>"));
        assert_eq!(bluetooth_json("off", &[], Duration::ZERO), "{\"state\":\"off\",\"devices\":[]}\n");
        assert_eq!(route("GET /bluetooth.json HTTP/1.1\r\n\r\n"), Route::Bluetooth);
    }

    #[test]
    fn checks_the_basic_password() {
        // "viewer:correct horse" and ":correct horse" (no user name).
        let with_user = "GET /camera/rear.jpg HTTP/1.1\r\nHost: x\r\nAuthorization: Basic dmlld2VyOmNvcnJlY3QgaG9yc2U=\r\n\r\n";
        let no_user = "GET / HTTP/1.1\r\nauthorization:  basic OmNvcnJlY3QgaG9yc2U=\r\n\r\n";
        assert!(authorized(with_user, "correct horse"));
        assert!(authorized(no_user, "correct horse"));
        assert!(!authorized(with_user, "correct horsf"));
        assert!(!authorized(with_user, "correct hors"));
        assert!(!authorized(with_user, "correct horse!"));
        assert!(!authorized(with_user, ""));
        assert!(!authorized("GET / HTTP/1.1\r\n\r\n", "correct horse"));
        assert!(!authorized("GET / HTTP/1.1\r\nAuthorization: Bearer abc\r\n\r\n", "correct horse"));
        assert!(!authorized("GET / HTTP/1.1\r\nAuthorization: Basic %%%\r\n\r\n", "correct horse"));
        // A header after the blank line is body, not a header.
        assert!(!authorized("GET / HTTP/1.1\r\n\r\nAuthorization: Basic OmNvcnJlY3QgaG9yc2U=\r\n", "correct horse"));
        assert_eq!(base64_decode("YQ==").unwrap(), b"a");
        assert_eq!(base64_decode("YWI=").unwrap(), b"ab");
        assert_eq!(base64_decode("YWJj").unwrap(), b"abc");
        let reply = String::from_utf8(unauthorized()).unwrap();
        assert!(reply.starts_with("HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm="));
    }

    #[test]
    fn password_rules() {
        assert_eq!(password_problem("Tr0ub4dor&3x"), None);
        assert!(password_problem("short").is_some());
        assert!(password_problem("has a space in it").is_some());
        assert!(password_problem("non-ascii-p\u{e4}ssword").is_some());
        assert!(password_problem(&"x".repeat(65)).is_some());
    }

    #[test]
    fn frames_live_video() {
        assert!(String::from_utf8_lossy(live_header()).contains("multipart/x-mixed-replace; boundary=frame\r\n"));
        assert_eq!(live_part(b"JPG"), b"--frame\r\nContent-Type: image/jpeg\r\nContent-Length: 3\r\n\r\nJPG\r\n");
    }

    #[test]
    fn the_firewall_port_matches() {
        assert_eq!(PORT_TEXT, PORT.to_string());
    }

    #[test]
    fn keeps_the_last_lines() {
        assert_eq!(last_lines("a\nb\nc\n", 2), "b\nc");
        assert_eq!(last_lines("a\nb", 5), "a\nb");
        assert_eq!(last_lines("", 3), "");
    }
}
