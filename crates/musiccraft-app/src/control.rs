//! Live mode, window side: a localhost control channel that lets an MCP client
//! (`musiccraft-mcp --bridge`) drive this window's session, so the user sees an
//! agent's changes as they happen (`docs/spec/mcp.md`).
//!
//! Connections are served on their own threads; each JSON-RPC line is handed to
//! the interface thread, which runs it between frames and sends the reply back.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};

use eframe::egui;
use musiccraft_mcp::bridge::ACCEPTED;

/// The longest first line (the token) read before a client is trusted.
const MAX_TOKEN_LINE: u64 = 256;

/// One message from a client, waiting for the interface thread.
pub struct Request {
    pub line: String,
    /// Where the reply goes (`None`: a notification, nothing to send).
    pub reply: Sender<Option<String>>,
}

/// The listening channel; requests arrive through [`Control::pending`].
pub struct Control {
    requests: Receiver<Request>,
}

impl Control {
    /// Listens on `127.0.0.1:<port>` and writes a fresh token to `token_file`
    /// once listening, so the file existing means the channel is ready.
    pub fn start(port: u16, token_file: &Path, ctx: egui::Context) -> Result<Self, String> {
        let token = new_token()?;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .map_err(|e| format!("control channel on port {port}: {e}"))?;
        write_token(token_file, &token)
            .map_err(|e| format!("control token {}: {e}", token_file.display()))?;
        let (tx, requests) = channel();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (tx, token, ctx) = (tx.clone(), token.clone(), ctx.clone());
                std::thread::spawn(move || serve(stream, &token, &tx, &ctx));
            }
        });
        Ok(Self { requests })
    }

    /// Requests that arrived since the last call.
    pub fn pending(&self) -> Vec<Request> {
        self.requests.try_iter().collect()
    }
}

/// 128 random bits as 32 hex digits.
fn new_token() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| format!("no randomness for the control token: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Writes the token next to `path` first and renames it into place, readable
/// only by the user on Unix.
fn write_token(path: &Path, token: &str) -> std::io::Result<()> {
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = std::path::PathBuf::from(temp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&temp)?;
    file.write_all(token.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temp, path)
}

/// Compares without stopping at the first difference.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// Serves one client: the token first, then one request per line.
fn serve(stream: TcpStream, token: &str, tx: &Sender<Request>, ctx: &egui::Context) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut writer = stream;
    let mut reader = BufReader::new(read_half);
    let mut first = String::new();
    if (&mut reader)
        .take(MAX_TOKEN_LINE)
        .read_line(&mut first)
        .is_err()
        || !same(first.trim(), token)
    {
        return;
    }
    if writeln!(writer, "{ACCEPTED}").is_err() {
        return;
    }
    for line in reader.lines() {
        let Ok(line) = line else {
            break;
        };
        let (reply, answer) = channel();
        if tx.send(Request { line, reply }).is_err() {
            break;
        }
        ctx.request_repaint();
        match answer.recv() {
            Ok(Some(text)) => {
                if writeln!(writer, "{text}")
                    .and_then(|()| writer.flush())
                    .is_err()
                {
                    break;
                }
            }
            Ok(None) => {}
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_fresh_hex() {
        let (a, b) = (
            new_token().unwrap_or_default(),
            new_token().unwrap_or_default(),
        );
        assert_eq!(a.len(), 32);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn token_comparison() {
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("abc", "abcd"));
    }

    #[test]
    fn channel_needs_the_token_then_relays_lines() {
        let dir = std::env::temp_dir().join(format!("musiccraft-control-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let token_file = dir.join("t.token");
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .and_then(|l| l.local_addr())
            .map(|a| a.port())
            .unwrap_or(0);
        let control = match Control::start(port, &token_file, egui::Context::default()) {
            Ok(control) => control,
            Err(err) => panic!("{err}"),
        };
        let token = std::fs::read_to_string(&token_file).unwrap_or_default();
        assert_eq!(token.len(), 32);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&token_file).map(|m| m.permissions().mode() & 0o777);
            assert_eq!(mode.ok(), Some(0o600));
        }

        // A wrong token is dropped without an answer.
        let mut bad = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).ok();
        if let Some(bad) = &mut bad {
            let _ = writeln!(bad, "nope");
            let mut line = String::new();
            let read = BufReader::new(bad).read_line(&mut line).unwrap_or(0);
            assert_eq!(read, 0, "connection closed");
        }

        let Ok(stream) = TcpStream::connect((Ipv4Addr::LOCALHOST, port)) else {
            panic!("connect");
        };
        let mut writer = stream.try_clone().ok();
        let mut reader = BufReader::new(stream);
        if let Some(w) = &mut writer {
            let _ = writeln!(w, "{token}");
            let _ = writeln!(w, "ping");
        }
        let mut line = String::new();
        let _ = reader.read_line(&mut line);
        assert_eq!(line.trim(), ACCEPTED);
        let request = loop {
            if let Some(r) = control.pending().into_iter().next() {
                break r;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert_eq!(request.line, "ping");
        let _ = request.reply.send(Some("pong".to_owned()));
        line.clear();
        let _ = reader.read_line(&mut line);
        assert_eq!(line.trim(), "pong");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
