//
//! `ledgerd` -- ledger daemon.
//!
//! WHAT:    A Unix socket server that wraps the existing
//!          CLI tools. Clients connect, send a command
//!          (one per line), and get the tool's output
//!          back. Single-process, multi-threaded: one
//!          thread per connection.
//! WHY:     The spec's "ledgerd daemon owning /biz/ledger/
//!          journal" is the entry point for network access
//!          to the books. A client on the same machine uses
//!          the socket directly; a client on another
//!          machine can reach it via SSH forwarding. This is
//!          the socket-based alternative for Phase final
//!          (a full a thin protocol implementation is deferred).
//! LAYER:   Driver. Threaded socket server; spawns
//!          subprocesses per command.
//! DEPENDS: stdlib (`std::os::unix::net`, `std::thread`).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::sync::Arc;
use std::thread;

fn main() -> ExitCode {
    let opts = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    // Remove any stale socket file.
    let _ = std::fs::remove_file(&opts.socket);
    let listener = match UnixListener::bind(&opts.socket) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("ledgerd: bind {}: {e}", opts.socket.display());
            return ExitCode::from(2);
        }
    };
    // The handler closure: parse the command, run the tool,
    // write the output back. Cloned for each thread.
    let handler = Arc::new(move |line: String, mut stream: UnixStream| -> std::io::Result<()> {
        handle_command(&line, &opts.journal, &mut stream)
    });
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        let h = Arc::clone(&handler);
        thread::spawn(move || {
            let _ = handle_connection(stream, h);
        });
    }
    ExitCode::from(0)
}

fn handle_connection(
    mut stream: UnixStream,
    handler: Arc<dyn Fn(String, UnixStream) -> std::io::Result<()> + Send + Sync>,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    while reader.read_line(&mut line)? > 0 {
        let trimmed = line.trim().to_string();
        if !trimmed.is_empty() {
            handler(trimmed, stream.try_clone()?)?;
        }
        line.clear();
    }
    Ok(())
}

fn handle_command(
    line: &str,
    journal: &std::path::Path,
    stream: &mut UnixStream,
) -> std::io::Result<()> {
    // Parse the command. The first token is the tool name;
    // the rest are the args (key=value or --flag VALUE).
    let mut parts = line.split_whitespace();
    let tool = match parts.next() {
        Some(t) => t.to_string(),
        None => return Ok(()),
    };
    // The journal path is always passed as `--journal <path>`.
    // Build the arg list: <tool's own args> --journal <path>.
    let mut args: Vec<String> = Vec::new();
    for p in parts {
        args.push(p.to_string());
    }
    args.push("--journal".to_string());
    args.push(journal.to_string_lossy().to_string());

    let target_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/debug");
    let bin = target_dir.join(&tool);
    let output = match Command::new(&bin)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            let msg = format!("ledgerd: spawn {}: {e}\n", bin.display());
            stream.write_all(msg.as_bytes())?;
            return Ok(());
        }
    };
    if !output.status.success() {
        let msg = format!("ledgerd: {} failed\n", tool);
        stream.write_all(msg.as_bytes())?;
    }
    stream.write_all(&output.stdout)?;
    stream.write_all(b"\n")?;  // blank-line terminator
    // Half-close: tell the client we're done writing. This
    // matches the request/response shape: one verb in,
    // one response out, EOF on the wire.
    let _ = stream.shutdown(std::net::Shutdown::Write);
    Ok(())
}

struct Opts {
    socket: PathBuf,
    journal: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let mut socket: Option<PathBuf> = None;
    let mut journal: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--socket" => {
                let p = args.next().ok_or_else(|| "--socket requires PATH".to_string())?;
                socket = Some(PathBuf::from(p));
            }
            "--journal" => {
                let p = args.next().ok_or_else(|| "--journal requires PATH".to_string())?;
                journal = Some(PathBuf::from(p));
            }
            _ => return Err(format!("ledgerd: unknown flag: {a}")),
        }
    }
    let socket = socket.ok_or_else(|| "--socket PATH is required".to_string())?;
    let journal = journal.ok_or_else(|| "--journal PATH is required".to_string())?;
    Ok(Opts { socket, journal })
}
