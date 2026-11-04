//! Moves a file to the phone over the network, before anything else can do that.
//!
//! There is no authentication and no encryption, and the port is open during the
//! call, so a sender must check the result by reading the file back with `dump`.

use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

const ACCEPT_WAIT: Duration = Duration::from_secs(60);
const IDLE_WAIT: Duration = Duration::from_secs(30);
const BUFFER: usize = 64 * 1024;

/// Waits for one connection, then stops listening.
pub fn accept_one(port: u16) -> io::Result<TcpStream> {
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))?;
    listener.set_nonblocking(true)?;
    let start = Instant::now();
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(err) if err.kind() != io::ErrorKind::WouldBlock => return Err(err),
            Err(_) if start.elapsed() > ACCEPT_WAIT => {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "no client connected"));
            }
            Err(_) => thread::sleep(Duration::from_millis(50)),
        }
    };
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(IDLE_WAIT))?;
    stream.set_write_timeout(Some(IDLE_WAIT))?;
    Ok(stream)
}

/// Writes one connection's data to a new file. It refuses an existing file, and
/// removes a partial one on failure. Returns the number of bytes.
pub fn receive(path: &str, port: u16) -> io::Result<u64> {
    let mut stream = accept_one(port)?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let result = (|| {
        let mut buffer = vec![0u8; BUFFER];
        let mut total = 0;
        loop {
            match stream.read(&mut buffer)? {
                0 => break,
                length => {
                    file.write_all(&buffer[..length])?;
                    total += length as u64;
                }
            }
        }
        file.sync_all()?;
        Ok(total)
    })();
    if result.is_err() {
        drop(file);
        let _ = std::fs::remove_file(path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn free_port() -> u16 {
        TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
    }

    fn send(port: u16, data: &[u8]) {
        let mut client = loop {
            match TcpStream::connect(("127.0.0.1", port)) {
                Ok(client) => break client,
                Err(_) => thread::sleep(Duration::from_millis(10)),
            }
        };
        client.write_all(data).unwrap();
    }

    fn temporary(name: &str) -> String {
        let path = std::env::temp_dir().join(format!("a32-{name}-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn saves_what_it_receives() {
        let (path, port) = (temporary("receive"), free_port());
        let saved = path.clone();
        let server = thread::spawn(move || receive(&saved, port));
        send(port, b"file contents");
        assert_eq!(server.join().unwrap().unwrap(), 13);
        assert_eq!(std::fs::read(&path).unwrap(), b"file contents");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn refuses_an_existing_file() {
        let (path, port) = (temporary("existing"), free_port());
        std::fs::write(&path, b"keep").unwrap();
        let saved = path.clone();
        let server = thread::spawn(move || receive(&saved, port));
        send(port, b"replacement");
        assert!(server.join().unwrap().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"keep");
        std::fs::remove_file(&path).unwrap();
    }
}
