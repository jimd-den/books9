//
//! `wbs` -- work breakdown structure driver.
//!
//! WHAT:    `wbs show --code CODE` prints the project's
//!          profile (name, parent) on stdout.
//! WHY:     Spec Phase 5 includes WBS. Phase P2P ships the
//!          read surface; the write surface (creating wbs
//!          elements) reuses `proj new` with a --parent flag.

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
    if opts.sub != "show" {
        eprintln!("wbs: unknown subcommand: {}", opts.sub);
        return ExitCode::from(2);
    }
    let profile_path = opts.root.join(&opts.code).join("profile.tsv");
    let text = match std::fs::read_to_string(&profile_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("wbs show: {e}");
            return ExitCode::from(2);
        }
    };
    let data = text.lines().nth(1).unwrap_or("");
    let cols: Vec<&str> = data.split('\t').collect();
    if cols.len() < 3 {
        eprintln!("wbs show: malformed profile");
        return ExitCode::from(2);
    }
    println!("field\tvalue");
    println!("code\t{}", cols[0]);
    println!("name\t{}", cols[1]);
    println!("parent\t{}", cols[2]);
    ExitCode::from(0)
}

struct Opts {
    sub: String,
    code: String,
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub = args.next().ok_or_else(|| "wbs: usage: wbs show ...".to_string())?;
    let mut root: Option<PathBuf> = None;
    let mut code: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--code" => code = Some(args.next().ok_or_else(|| "--code requires CODE".to_string())?),
            _ => return Err(format!("wbs: unknown flag: {a}")),
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let code = code.ok_or_else(|| "--code CODE is required".to_string())?;
    Ok(Opts { sub, code, root })
}
