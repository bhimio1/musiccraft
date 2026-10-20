//! Live mode: `musiccraft-mcp --bridge` relays to a window's control channel.
//! A stand-in window here checks the token and answers with a real server.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};

use musiccraft_mcp::Server;

/// A one-client window: token first, then JSON-RPC lines.
fn fake_window(listener: TcpListener, token: &'static str) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut writer = stream.try_clone().unwrap();
        let mut lines = BufReader::new(stream).lines();
        if lines.next().unwrap().unwrap() != token {
            return;
        }
        writeln!(writer, "{}", musiccraft_mcp::bridge::ACCEPTED).unwrap();
        let mut server = Server::new();
        for line in lines {
            let Ok(line) = line else { break };
            if let Some(reply) = server.handle_line(&line) {
                writeln!(writer, "{reply}").unwrap();
            }
        }
    })
}

fn token_file(name: &str, token: &str) -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("musiccraft-bridge-{name}-{}", std::process::id()));
    std::fs::write(&path, format!("{token}\n")).unwrap();
    path
}

#[test]
fn relays_to_the_window() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let window = fake_window(listener, "secret");
    let token = token_file("ok", "secret");
    let mut child = Command::new(env!("CARGO_BIN_EXE_musiccraft-mcp"))
        .args(["--bridge", &addr, "--control-token-file"])
        .arg(&token)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let input = concat!(
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"project_set_tempo","arguments":{"args":["99"]}}}"#,
        "\n",
    );
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(input.as_bytes()).unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    let reply: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(reply["id"], 7);
    assert_eq!(reply["result"]["content"][0]["text"], "tempo 99 BPM");
    drop(stdin);
    assert!(child.wait().unwrap().success());
    window.join().unwrap();
    let _ = std::fs::remove_file(token);
}

#[test]
fn a_refused_token_is_an_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let window = fake_window(listener, "secret");
    let token = token_file("bad", "wrong");
    let output = Command::new(env!("CARGO_BIN_EXE_musiccraft-mcp"))
        .args(["--bridge", &addr, "--control-token-file"])
        .arg(&token)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("refused"));
    window.join().unwrap();
    let _ = std::fs::remove_file(token);
}

#[test]
fn dir_sets_where_relative_paths_go() {
    let dir = std::env::temp_dir().join(format!("musiccraft-mcp-dir-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_musiccraft-mcp"))
        .arg("--dir")
        .arg(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"project_save","arguments":{"args":["song.mcproj"]}}}"#)
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(
        dir.join("song.mcproj").is_file(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
