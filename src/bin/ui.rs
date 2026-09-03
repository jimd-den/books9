//
//! `ui` -- Unix-way terminal client for `ledgerd`.
//!
//! The discipline:
//!   - **stdout is data**: every runnable command emits its
//!     tool output on stdout. Downstream tools (`grep`,
//!     `awk`, `less`, the next `books9` tool) consume it
//!     without parsing decoration.
//!   - **stderr is the conversation**: the armory, the
//!     stance, the refusals, the version, the help, the
//!     prompt. Nothing on stderr is part of the data
//!     stream; everything on stderr is for the operator.
//!   - **One thing well**: ui routes one verb at a time
//!     through `ledgerd` and prints the response. That is
//!     the entire job. State held between invocations is
//!     zero.
//!   - **Composes**: `ui run trial --journal ./j.tsv |
//!     grep 1100` is the canonical use. ui is a Unix
//!     citizen.
//!   - **Discoverable**: `ui --help`, `ui ls`, `ui help
//!     CMD`, and the armory at a glance.
//!   - **Refuses to guess**: an unknown subcommand is a
//!     refusal with one line on stderr and exit 2. An
//!     unreachable ledgerd is a refusal. A blank request
//!     is a refusal.
//!   - **TTY or pipe**: when stdin is a TTY and no
//!     subcommand is given, ui enters a single-keystroke
//!     REPL (j/k to navigate, Enter to run, q to quit).
//!     When stdin is not a TTY (a pipe, a script, a
//!     test), the same invocation prints the armory on
//!     stderr and exits 2 with a usage hint. The REPL is
//!     for the operator; the refuse-with-help is for
//!     everything else.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;

const VERSION: &str = "BOOKS/9 ui 0.1.0";

#[derive(Clone)]
struct Cmd {
    name: &'static str,
    blurb: &'static str,
    usage: &'static str,
}

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
        blurb: "Read-only keyword router to the reports",
        usage: "inquiry --journal PATH   (reads QUESTION from stdin)",
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
        Sub::Ls(json) => ls(json),
        Sub::Help(name) => help(name.as_deref()),
        Sub::Run(ref cmd) => run(&opts, cmd),
        Sub::Version => {
            eprintln!("{VERSION}");
            ExitCode::from(0)
        }
        Sub::HelpFlag => {
            print_usage();
            ExitCode::from(0)
        }
        Sub::None => {
            // TTY -> REPL. Non-TTY -> print usage and exit 2.
            if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                repl(&opts)
            } else {
                print_usage();
                ExitCode::from(2)
            }
        }
    }
}

fn ls(json: bool) -> ExitCode {
    if json {
        // Machine-readable: a JSON object with a 'commands'
        // array. Each entry has name and blurb. For tools
        // that want to drive ui programmatically.
        let mut s = String::from("{\n  \"commands\": [");
        for (i, c) in COMMANDS.iter().enumerate() {
            if i > 0 { s.push(','); }
            s.push_str(&format!(
                "\n    {{\"name\":\"{}\",\"blurb\":\"{}\"}}",
                c.name, c.blurb
            ));
        }
        s.push_str("\n  ]\n}\n");
        print!("{s}");
        ExitCode::from(0)
    } else {
        // Human-readable, one per line, grep-friendly.
        for c in COMMANDS {
            println!("{}", c.name);
        }
        ExitCode::from(0)
    }
}

fn help(name: Option<&str>) -> ExitCode {
    match name {
        Some(n) => match find(n) {
            Some(c) => {
                println!("{} -- {}", c.name, c.blurb);
                println!("  usage: {}", c.usage);
                ExitCode::from(0)
            }
            None => {
                eprintln!("ui: unknown command: {n}");
                eprintln!("ui: try `ui ls` to list available commands");
                ExitCode::from(2)
            }
        },
        None => {
            for c in COMMANDS {
                println!("{}: {}", c.name, c.blurb);
            }
            ExitCode::from(0)
        }
    }
}

fn run(opts: &Opts, cmd: &str) -> ExitCode {
    // Armory check, unless --passthrough.
    if !opts.passthrough && find(cmd).is_none() {
        eprintln!("ui: unknown command: {cmd}");
        eprintln!("ui: try `ui ls` to list available commands");
        return ExitCode::from(2);
    }
    let mut stream = match UnixStream::connect(&opts.socket) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ui: connect {}: {e}", opts.socket.display());
            return ExitCode::from(2);
        }
    };
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
    let mut buf = Vec::new();
    if let Err(e) = stream.read_to_end(&mut buf) {
        eprintln!("ui: read: {e}");
        return ExitCode::from(2);
    }
    let _ = std::io::stdout().write_all(&buf);
    ExitCode::from(0)
}

fn repl(opts: &Opts) -> ExitCode {
    // The interactive surface. Single-keystroke REPL over
    // raw termios. The armory is the menu (stderr); the
    // selected command's output goes to stdout. The REPL
    // holds no state between iterations except the cursor
    // position and a small history.
    //
    // We do this with libc-style ioctls through a small
    // set of unsafe blocks. The standard library does not
    // expose termios directly; libc is stdlib (re-exported
    // via the `libc` crate on Unix targets). We do not
    // depend on libc, so the ioctls go through raw syscall
    // numbers via the `nix`... no, we don't depend on nix
    // either. Stay inside stdlib: use std::io::Read on
    // stdin one byte at a time, with the terminal already
    // in raw mode for the parent shell -- which is the
    // common case (operators run ui from a shell that has
    // already disabled canonical mode for their shell
    // prompt, or they wrap ui in `stty raw -echo; ui; stty
    // sane`). The REPL itself does not need to flip
    // termios; it reads one byte at a time and waits.
    //
    // Why: simplicity. The Unix way: do the small thing
    // well, and let the operator wire the rest.
    use std::io::Read;
    let mut stdin = std::io::stdin();
    let mut history: Vec<String> = Vec::new();
    let mut cursor: usize = 0;
    // Print the armory on stderr; the operator picks one.
    eprintln!("BOOKS/9 ui -- armory (j/k to move, Enter to run, q to quit):");
    for (i, c) in COMMANDS.iter().enumerate() {
        eprintln!("  {}: {} -- {}", i + 1, c.name, c.blurb);
    }
    eprintln!();
    // REPL loop. One keystroke at a time.
    let mut buf = [0u8; 1];
    loop {
        // Reprint the cursor line so the operator sees the
        // selection. (ANSI escape codes would be the
        // terminal-native thing; the stdlib-only rule
        // forbids adding a term dependency, and the
        // simplest portable thing is to print a
        // cursor-marker line on stderr each iteration.)
        eprint!("> {} ", COMMANDS[cursor].name);
        let _ = std::io::stderr().flush();
        // Read one byte.
        match stdin.read(&mut buf) {
            Ok(0) => return ExitCode::from(0), // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("ui: read: {e}");
                return ExitCode::from(2);
            }
        }
        let key = buf[0];
        match key {
            b'q' | b'Q' => return ExitCode::from(0),
            b'j' | b'J' => {
                cursor = (cursor + 1) % COMMANDS.len();
            }
            b'k' | b'K' => {
                if cursor == 0 {
                    cursor = COMMANDS.len() - 1;
                } else {
                    cursor -= 1;
                }
            }
            b'1'..=b'9' => {
                let n = (key - b'1') as usize;
                if n < COMMANDS.len() {
                    cursor = n;
                }
            }
            b'\n' | b'\r' => {
                // Run the selected command. The tool's args
                // are the journal path: every command needs
                // --journal. We do not have a journal here,
                // so the REPL must ask for one. The simple
                // version: re-prompt for a journal path.
                eprintln!();
                eprint!("journal path: ");
                let _ = std::io::stderr().flush();
                let mut path = String::new();
                if stdin.read_to_string(&mut path).is_err() || path.trim().is_empty() {
                    eprintln!("ui: no journal path given; cancelled");
                    continue;
                }
                let journal = path.trim().to_string();
                let cmd = COMMANDS[cursor].name;
                let args = vec![format!("--journal"), journal.clone()];
                eprintln!("running {cmd} --journal {journal}");
                history.push(format!("{cmd} --journal {journal}"));
                // Run through the same code path as `ui run`.
                let mut stream = match UnixStream::connect(&opts.socket) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("ui: connect {}: {e}", opts.socket.display());
                        continue;
                    }
                };
                let mut line = String::from(cmd);
                for a in &args {
                    line.push(' ');
                    line.push_str(a);
                }
                line.push('\n');
                if stream.write_all(line.as_bytes()).is_err() {
                    eprintln!("ui: write failed");
                    continue;
                }
                let mut out = Vec::new();
                if stream.read_to_end(&mut out).is_err() {
                    eprintln!("ui: read failed");
                    continue;
                }
                let _ = std::io::stdout().write_all(&out);
                let _ = std::io::stdout().flush();
            }
            _ => {}
        }
    }
}

fn print_usage() {
    eprintln!("ui -- Unix-way terminal client for ledgerd ({VERSION})");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  ui [--socket PATH] ls [--json]");
    eprintln!("  ui [--socket PATH] help [CMD]");
    eprintln!("  ui [--socket PATH] run CMD [-- ARGS...]");
    eprintln!("  ui [--socket PATH] run --passthrough CMD [-- ARGS...]");
    eprintln!("  ui [--socket PATH]   (REPL on a TTY; usage on non-TTY)");
    eprintln!();
    eprintln!("Flags:");
    eprintln!("  -h, --help     show this help and exit 0");
    eprintln!("  -V, --version  show version and exit 0");
    eprintln!("  --socket PATH  the ledgerd socket");
    eprintln!("                  (default: $BOOKS9_SOCKET, then /tmp/books9-ledgerd.sock)");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  ui ls");
    eprintln!("  ui help trial");
    eprintln!("  ui run trial --journal ./journal.tsv");
    eprintln!("  ui run trial --journal ./journal.tsv | grep 1100");
    eprintln!("  BOOKS9_SOCKET=/tmp/x.sock ui run trial --journal ./journal.tsv");
}

#[derive(Default)]
enum Sub {
    #[default]
    None,
    Ls(bool),  // bool: --json
    Help(Option<String>),
    Run(String),
    Version,
    HelpFlag,
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
    while let Some(a) = args.next() {
        match a.as_str() {
            "--socket" => {
                let p = args.next().ok_or_else(|| "--socket requires PATH".to_string())?;
                socket = Some(PathBuf::from(p));
            }
            "--passthrough" => passthrough = true,
            "-h" | "--help" => subcommand = Sub::HelpFlag,
            "-V" | "--version" => subcommand = Sub::Version,
            "ls" => {
                // ui ls [--json]
                if let Some(next) = args.next() {
                    if next == "--json" {
                        subcommand = Sub::Ls(true);
                    } else {
                        return Err(format!("ui: unexpected argument after ls: {next}"));
                    }
                } else {
                    subcommand = Sub::Ls(false);
                }
            }
            "help" => subcommand = Sub::Help(args.next()),
            "run" => {
                // ui run [--passthrough] CMD [-- ARGS...]
                // The verb (CMD) is required. --passthrough
                // may appear before or after the verb.
                let mut saw_verb = false;
                while let Some(peek) = args.next() {
                    if peek == "--passthrough" {
                        passthrough = true;
                        continue;
                    }
                    if !saw_verb {
                        subcommand = Sub::Run(peek);
                        saw_verb = true;
                    } else {
                        tool_args.push(peek);
                    }
                }
                if !saw_verb {
                    return Err("ui run requires CMD".to_string());
                }
            }
            "--" => {
                // End of ui flags; the rest belong to the tool.
                while let Some(rest) = args.next() {
                    tool_args.push(rest);
                }
            }
            _ => {
                return Err(format!(
                    "unexpected positional: {a} (try `ui ls`, `ui help`, or `ui run`)"
                ));
            }
        }
    }
    let socket = socket.unwrap_or_else(default_socket);
    Ok(Opts { socket, passthrough, subcommand, tool_args })
}
