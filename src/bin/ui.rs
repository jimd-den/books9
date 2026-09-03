//
//! `ui` -- -mode terminal client for `ledgerd`.
//!
//! The Book of , applied to a TUI:
//! - **Ground**: the armory. Every command the read-mostly
//! operator can strike with is named up front.
//! - **Water**: rhythm. Each invocation is one strike; the
//! UI holds no state between invocations.
//! - **Fire**: timing. `ui run` is a single decisive
//! motion; `ui ls` is the menu; `ui help` is the stance.
//! - **Wind**: composability. The UI's stdout is data; pipe
//! it to `grep`, `awk`, `less`, or another BOOKS/9 tool.
//! - **Void**: empty mind. The UI does not interpret
//! ledgerd's output. It only displays it.
//!
//! Subcommands:
//! ui ls -- list available commands (the armory)
//! ui help [CMD] -- show usage for a command (or all)
//! ui run CMD [args] -- run one command through ledgerd
//!
//! Flags:
//! --socket PATH -- the ledgerd socket
//! (default: $BOOKS9_SOCKET,
//! then /tmp/books9-ledgerd.sock)
//! --journal PATH -- propagated to the underlying tool
//!
//! With no subcommand, `ui` prints usage on stderr and exits 2.
//! That refusal is the discipline: every command must be
//! intentional. did not believe in idle swings.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Clone)]
struct Cmd {
 name: &'static str,
 blurb: &'static str,
 usage: &'static str,
}

/// The armory. Read-mostly commands routed through ledgerd.
/// The list is the stable surface area of the read-mostly
/// operator: trial, balance, stock, ar/ap aging, inquiry.
const COMMANDS: &[Cmd] = &[
 Cmd {
 name: "trial",
 blurb: "Trial balance (per-account per-currency totals)",
 usage: "trial --journal PATH [--format tsv|json] [--entity ENT] [--period YYYY-MM]",
 },
 Cmd {
 name: "balance",
 blurb: "Balance for one account at one point in time",
 usage: "balance --journal PATH --account ACCT [--as-of DATE] [--format tsv|json]",
 },
 Cmd {
 name: "stock",
 blurb: "On-hand stock per SKU per warehouse (cache vs recompute)",
 usage: "stock --journal PATH [--item ID] [--warehouse WH] [--format tsv|json]",
 },
 Cmd {
 name: "ar_aging",
 blurb: "Open receivables by aging bucket",
 usage: "ar_aging --journal PATH --as-of DATE [--format tsv|json]",
 },
 Cmd {
 name: "ap_aging",
 blurb: "Open payables by aging bucket",
 usage: "ap_aging --journal PATH --as-of DATE [--format tsv|json]",
 },
 Cmd {
 name: "inquiry",
 blurb: "Read-only keyword router to the reports ( mode)",
 usage: "inquiry --journal PATH (reads QUESTION from stdin)",
 },
];

fn find(name: &str) -> Option<&'static Cmd> {
 COMMANDS.iter().find(|c| c.name == name)
}

fn default_socket() -> PathBuf {
 if let Ok(p) = std::env::var("BOOKS9_SOCKET") {
 return PathBuf::from(p);
 }
 PathBuf::from("/tmp/books9-ledgerd.sock")
}

fn main() -> ExitCode {
 let opts = match parse_args() {
 Ok(o) => o,
 Err(e) => {
 eprintln!("ui: {e}");
 return ExitCode::from(2);
 }
 };
 match opts.subcommand {
 Sub::Ls => ls(),
 Sub::Help(name) => help(name.as_deref()),
 Sub::Run(ref cmd) => run(&opts, cmd),
 Sub::None => {
 // No subcommand: print usage on stderr, exit 2.
 // Refusal is the discipline.
 print_usage();
 ExitCode::from(2)
 }
 }
}

fn ls() -> ExitCode {
 // The armory: every command on stdout, one per line.
 // Column-shaped for grep-friendliness.
 println!("{}", COMMANDS.iter().map(|c| c.name).collect::<Vec<_>>().join("\n"));
 ExitCode::from(0)
}

fn help(name: Option<&str>) -> ExitCode {
 match name {
 Some(n) => match find(n) {
 Some(c) => {
 println!("{} -- {}", c.name, c.blurb);
 println!(" usage: {}", c.usage);
 ExitCode::from(0)
 }
 None => {
 eprintln!("ui: unknown command: {n}");
 eprintln!("ui: try `ui ls` to list available commands");
 ExitCode::from(2)
 }
 },
 None => {
 // No command name: list every command's one-liner.
 // grep-friendly: each line is `name: blurb`.
 for c in COMMANDS {
 println!("{}: {}", c.name, c.blurb);
 }
 ExitCode::from(0)
 }
 }
}

fn run(opts: &Opts, cmd: &str) -> ExitCode {
 // A single strike. The command must be in the armory,
 // unless the user passed --passthrough for an arbitrary
 // ledgerd-recognized verb.
 if !opts.passthrough && find(cmd).is_none() {
 eprintln!("ui: unknown command: {cmd}");
 eprintln!("ui: try `ui ls` to list available commands");
 return ExitCode::from(2);
 }
 // Connect to ledgerd.
 let mut stream = match UnixStream::connect(&opts.socket) {
 Ok(s) => s,
 Err(e) => {
 eprintln!("ui: connect {}: {e}", opts.socket.display());
 return ExitCode::from(2);
 }
 };
 // Send the command line. The shape is what ledgerd
 // expects: one verb, then its flags.
 let mut line = String::from(cmd);
 for a in &opts.tool_args {
 line.push(' ');
 line.push_str(a);
 }
 line.push('\n');
 if let Err(e) = stream.write_all(line.as_bytes()) {
 eprintln!("ui: write: {e}");
 return ExitCode::from(2);
 }
 // Read the response and write it to stdout verbatim.
 // The UI does not interpret; it only displays.
 let mut buf = Vec::new();
 if let Err(e) = stream.read_to_end(&mut buf) {
 eprintln!("ui: read: {e}");
 return ExitCode::from(2);
 }
 let _ = std::io::stdout().write_all(&buf);
 ExitCode::from(0)
}

fn print_usage() {
 eprintln!("ui -- -mode terminal client for ledgerd");
 eprintln!();
 eprintln!("Usage:");
 eprintln!(" ui [--socket PATH] ls");
 eprintln!(" ui [--socket PATH] help [CMD]");
 eprintln!(" ui [--socket PATH] run CMD [-- ARGS...]");
 eprintln!(" ui [--socket PATH] run --passthrough CMD [-- ARGS...]");
 eprintln!();
 eprintln!("Defaults:");
 eprintln!(" --socket $BOOKS9_SOCKET or /tmp/books9-ledgerd.sock");
 eprintln!();
 eprintln!("Examples:");
 eprintln!(" ui ls");
 eprintln!(" ui help trial");
 eprintln!(" ui run trial --journal ./journal.tsv");
 eprintln!(" ui run trial --journal ./journal.tsv | grep 1100");
}

#[derive(Default)]
enum Sub {
 #[default]
 None,
 Ls,
 Help(Option<String>),
 Run(String),
}

#[derive(Default)]
struct Opts {
 socket: PathBuf,
 passthrough: bool,
 subcommand: Sub,
 tool_args: Vec<String>,
}

fn parse_args() -> Result<Opts, String> {
 let mut args = std::env::args().skip(1);
 let mut socket: Option<PathBuf> = None;
 let mut passthrough = false;
 let mut subcommand = Sub::None;
 let mut tool_args: Vec<String> = Vec::new();
 // Position cursor.
 while let Some(a) = args.next() {
 match a.as_str() {
 "--socket" => {
 let p = args.next().ok_or_else(|| "--socket requires PATH".to_string())?;
 socket = Some(PathBuf::from(p));
 }
 "--passthrough" => passthrough = true,
 "--help" | "-h" => {
 subcommand = Sub::Help(None);
 }
 "ls" => subcommand = Sub::Ls,
 "help" => {
 // `help` with optional positional next.
 subcommand = Sub::Help(args.next());
 }
 "run" => {
 let cmd = args.next().ok_or_else(|| "ui run requires CMD".to_string())?;
 subcommand = Sub::Run(cmd);
 // Remaining args are forwarded to the tool verbatim.
 while let Some(rest) = args.next() {
 tool_args.push(rest);
 }
 }
 "--" => {
 // End of ui flags; the rest belong to the tool
 // (only meaningful after `run CMD --`).
 if let Sub::Run(_) = subcommand {
 while let Some(rest) = args.next() {
 tool_args.push(rest);
 }
 } else {
 tool_args.push(a);
 }
 }
 _ => {
 // Bare positional before any subcommand: it's
 // ambiguous. Refuse to guess.
 return Err(format!("unexpected positional: {a} (try `ui ls`, `ui help`, or `ui run`)"));
 }
 }
 }
 let socket = socket.unwrap_or_else(default_socket);
 Ok(Opts { socket, passthrough, subcommand, tool_args })
}
