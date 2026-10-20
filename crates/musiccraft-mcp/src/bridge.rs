//! Live mode, client side: relays the stdio transport to an open MusicCraft
//! window's control channel (`musiccraft --control <port>`), so an agent works
//! in the window the user is looking at.

use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;

/// The line the window answers a correct token with.
pub const ACCEPTED: &str = "ok";

/// Connects to the window at `addr` with the token in `token_file`, then relays
/// stdin to the window and its replies to stdout until either side closes.
///
/// # Errors
///
/// When the token can't be read, the window can't be reached, or it refuses
/// the token.
pub fn relay(addr: &str, token_file: &Path) -> Result<(), String> {
    let token = std::fs::read_to_string(token_file)
        .map_err(|e| format!("can't read the token file {}: {e}", token_file.display()))?;
    let mut stream =
        TcpStream::connect(addr).map_err(|e| format!("can't reach MusicCraft at {addr}: {e}"))?;
    writeln!(stream, "{}", token.trim()).map_err(|e| e.to_string())?;
    let mut replies = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let mut first = String::new();
    replies.read_line(&mut first).map_err(|e| e.to_string())?;
    if first.trim() != ACCEPTED {
        return Err("the MusicCraft window refused the control token".to_owned());
    }
    std::thread::spawn(move || {
        let mut stdout = io::stdout().lock();
        for line in replies.lines() {
            let Ok(line) = line else {
                break;
            };
            if writeln!(stdout, "{line}")
                .and_then(|()| stdout.flush())
                .is_err()
            {
                break;
            }
        }
        // The window closed: nothing more to serve.
        std::process::exit(0);
    });
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else {
            break;
        };
        if writeln!(stream, "{line}")
            .and_then(|()| stream.flush())
            .is_err()
        {
            break;
        }
    }
    Ok(())
}
