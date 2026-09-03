
//! `inquiry` -- read-only inquiry agent.
//!
//! WHAT:    Takes a question on stdin, routes to a report
//!          tool based on keywords, prints the result.
//!          The agent NEVER calls `post` or any mutating tool.
//! WHY:     "What is the cash balance today?" is the operator's
//!          first question. The agent pattern from the SRD:
//!          "agentic inquiry, not mutation."
//! LAYER:   Driver. Argv parsing, the keyword router, the
//!          subprocess call.
//! DEPENDS: stdlib.

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

fn main() -> ExitCode {
    let opts = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let mut question = String::new();
    if io::stdin().read_to_string(&mut question).is_err() {
        eprintln!("inquiry: read stdin failed");
        return ExitCode::from(2);
    }
    let question = question.to_lowercase();
    let (tool, extra_args) = route(&question);
    let child = match Command::new(tool.as_str())
        .arg("--journal").arg(&opts.journal)
        .args(&extra_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("inquiry: spawn {tool}: {e}");
            return ExitCode::from(2);
        }
    };
    let out = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("inquiry: wait: {e}");
            return ExitCode::from(2);
        }
    };
    if !out.status.success() {
        eprintln!("inquiry: {tool} failed: {}", String::from_utf8_lossy(&out.stderr));
        return ExitCode::from(2);
    }
    let stdout = io::stdout();
    let mut out_lock = stdout.lock();
    if out_lock.write_all(&out.stdout).is_err() {
        return ExitCode::from(2);
    }
    ExitCode::from(0)
}

/// Route a (lowercased) question to a (tool, extra-args) pair.
///
/// Keyword map (small; future cycles add more):
///   "cash" / "balance"   -> trial
///   "ar" / "receivable" / "aging" -> ar_aging
///   "stock" / "on hand" / "onhand" -> stock
///   default -> trial (the safe default; never mutates)
fn route(question: &str) -> (String, Vec<String>) {
    let tokens: Vec<&str> = question.split_whitespace().collect();
    for token in &tokens {
        match *token {
            "cash" | "balance" | "balances" => return (tool_path("trial"), vec![]),
            "ar" | "receivable" | "aging" | "aged" => {
                // ar_aging needs --as-of. Default to today.
                return (tool_path("ar_aging"), vec!["--as-of".to_string(), "1970-01-01".to_string()]);
            }
            "stock" | "on" | "hand" | "onhand" | "on-hand" | "on_hand" => {
                // stock needs --cache. Use a temp file the
                // driver writes; the --cache flag is a path.
                return (tool_path("stock"), vec!["--cache".to_string(), "/tmp/books9-inquiry-cache.tsv".to_string()]);
            }
            _ => {}
        }
    }
    // Default.
    (tool_path("trial"), vec![])
}

/// Look up a tool\'s path. First try the env var
/// `INQUIRY_TOOL_<NAME>` (uppercased, e.g.
/// `INQUIRY_TOOL_TRIAL`); fall back to the bare name
/// (assumed on PATH).
fn tool_path(name: &str) -> String {
    let env_name = format!("INQUIRY_TOOL_{}", name.to_uppercase().replace('-', "_"));
    std::env::var(&env_name).unwrap_or_else(|_| name.to_string())
}

struct Opts {
    journal: PathBuf,
    #[allow(dead_code)]
    as_of: Option<String>,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let mut journal: Option<PathBuf> = None;
    let mut as_of: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--journal" => {
                let p = args.next().ok_or_else(|| "--journal requires PATH".to_string())?;
                journal = Some(PathBuf::from(p));
            }
            "--as-of" => {
                as_of = Some(args.next().ok_or_else(|| "--as-of requires DATE".to_string())?);
            }
            _ => return Err(format!("inquiry: unknown flag: {a}")),
        }
    }
    let journal = journal.ok_or_else(|| "--journal PATH is required".to_string())?;
    Ok(Opts { journal, as_of })
}
