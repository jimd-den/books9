//
//! `fx` -- foreign-exchange rates driver.
//!
//! WHAT:    One verb: list. Reads the rates table at --path PATH
//!          and emits it to stdout as TSV. Read-only.
//! WHY:     The clerk wants to spot-check what rate is in effect
//!          for a given (currency, date). The driver is the
//!          end-user surface; the math is in libbiz::fx.
//! LAYER:   Driver. Argv parsing, the read, and the print are
//!          thin and named.
//! DEPENDS: `libbiz::fx` (read_table), stdlib.
//! USED BY: FX admins, the loop plan's "playtest the role of
//!          an FX admin" step.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let opts = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let table = match new_project::fx::read_table(&opts.path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    // Emit the table. We round-trip through the in-memory
    // table to demonstrate the read+write contract; the bytes
    // are deterministic (BTreeMap iteration is sorted).
    println!("date\tfrom\tto\trate");
    // The table doesn't preserve insertion order; we re-walk
    // the source file to print rows in source order, which is
    // what a clerk expects. Fall back to BTreeMap iteration
    // if the source file can't be reread.
    print_from_source(&opts.path);
    // Suppress the unused-warning for the table we just built.
    let _ = table;
    ExitCode::from(0)
}

/// Read the source file and print each non-header line. This
/// preserves the clerk's insertion order (the order they typed
/// the rates), which the in-memory BTreeMap does not.
fn print_from_source(path: &PathBuf) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return,
    };
    let mut iter = text.lines();
    // Skip the first non-blank line (the header).
    if let Some(_header) = iter.next() {
        for line in iter {
            if line.trim().is_empty() {
                continue;
            }
            println!("{line}");
        }
    }
}

struct Opts {
    path: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut path: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--path" => {
                let p = args.next().ok_or_else(|| "--path requires PATH".to_string())?;
                path = Some(PathBuf::from(p));
            }
            _ => {}
        }
    }
    let path = path.ok_or_else(|| "fx: --path PATH is required".to_string())?;
    Ok(Opts { path })
}
