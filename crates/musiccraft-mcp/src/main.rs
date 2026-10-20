//! `musiccraft-mcp`: MCP server over stdio. Configure an MCP client to launch this
//! binary; each line on stdin is one JSON-RPC message.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use musiccraft_mcp::Server;

const USAGE: &str = "\
usage: musiccraft-mcp [--dir <folder>] [--bridge <host:port> --control-token-file <file>]
  --dir <folder>              read and write relative paths in this folder
  --bridge <host:port>        work in an open MusicCraft window (musiccraft --control <port>)
  --control-token-file <file> the window's control token";

#[derive(Default)]
struct Options {
    dir: Option<PathBuf>,
    bridge: Option<String>,
    token_file: Option<PathBuf>,
}

fn parse_args() -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| USAGE.to_owned());
        match arg.as_str() {
            "--dir" => options.dir = Some(value()?.into()),
            "--bridge" => options.bridge = Some(value()?),
            "--control-token-file" => options.token_file = Some(value()?.into()),
            "-h" | "--help" => return Err(USAGE.to_owned()),
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    Ok(options)
}

fn serve() {
    let mut server = Server::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        if let Some(response) = server.handle_line(&line) {
            if writeln!(stdout, "{response}")
                .and_then(|()| stdout.flush())
                .is_err()
            {
                break;
            }
        }
    }
}

fn main() {
    let options = parse_args().unwrap_or_else(|message| {
        eprintln!("{message}");
        std::process::exit(2);
    });
    if let Some(dir) = &options.dir {
        if let Err(err) = std::env::set_current_dir(dir) {
            eprintln!("musiccraft-mcp: can't use {}: {err}", dir.display());
            std::process::exit(2);
        }
    }
    match (&options.bridge, &options.token_file) {
        (Some(addr), Some(token)) => {
            if let Err(err) = musiccraft_mcp::bridge::relay(addr, token) {
                eprintln!("musiccraft-mcp: {err}");
                std::process::exit(1);
            }
        }
        (None, None) => serve(),
        _ => {
            eprintln!("--bridge and --control-token-file go together\n{USAGE}");
            std::process::exit(2);
        }
    }
}
