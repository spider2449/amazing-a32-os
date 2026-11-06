//! A read-only status page for the local network: what the panel's home page shows,
//! and the end of the persistent log. It takes no input beyond the path and changes
//! nothing, so it needs no login. The firewall only lets private addresses reach it.
//! Everything here is text in and bytes out, so it runs on the host in unit tests.

use crate::ui::Snapshot;

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
        _ => Route::NotFound,
    }
}

/// The rows both formats show, as (JSON key, label, value).
fn rows(status: &Snapshot) -> [(&'static str, &'static str, &str); 11] {
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
    ]
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

/// The status page. `log_tail` is shown as it is, after escaping.
pub fn page(status: &Snapshot, time: &str, log_tail: &str) -> String {
    let mut table = String::new();
    for (_, label, value) in rows(status) {
        table.push_str(&format!("<tr><th>{label}</th><td>{}</td></tr>\n", escape_html(value)));
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
<p>{} · refreshes every {REFRESH_SECONDS} s · <a href=\"/status.json\">JSON</a></p>
<table>
{table}</table>
<h2>Log, newest last</h2>
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
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
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
        let html = page(&status(), "2026-11-06T12:00:00Z", "[1.0] <script>alert(1)</script>");
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
