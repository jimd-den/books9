//
//! `party` -- master-data driver for customers and vendors.
//!
//! WHAT:    Three subcommands on a parties root directory:
//!            party new   -- create a leaf with profile.tsv
//!            party ls    -- list every party
//!            party show  -- print one party's profile
//! WHY:     The order-to-cash path needs registered
//!          customers before it can invoice. `party` is the
//!          master-data entry point.
//! LAYER:   Driver. Argv parsing, subcommand dispatch, and
//!          the three subcommand bodies are thin and named.
//! DEPENDS: `libbiz::party` (walk), stdlib.
//! USED BY: Sales clerks, the loop plan's "playtest the
//!          role of a clerk" step.

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
    match opts.subcommand {
        Sub::New { ref id, ref name, ref kind, ref terms } => {
            cmd_new(&opts.root, id, name, kind, terms)
        }
        Sub::Ls => cmd_ls(&opts.root),
        Sub::Show(ref id) => cmd_show(&opts.root, id),
    }
}

fn cmd_new(root: &PathBuf, id: &str, name: &str, kind: &str, terms: &str) -> ExitCode {
    let leaf = root.join(id);
    if leaf.exists() {
        eprintln!("party new: {id} already exists at {}", leaf.display());
        return ExitCode::from(2);
    }
    if let Err(e) = std::fs::create_dir_all(&leaf) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    let body = format!("id\tname\tkind\tterms\n{id}\t{name}\t{kind}\t{terms}\n");
    let profile = leaf.join("profile.tsv");
    if let Err(e) = new_project::store::write_atomic(&profile, body.as_bytes()) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    ExitCode::from(0)
}

fn cmd_ls(root: &PathBuf) -> ExitCode {
    let parties = match new_project::party::walk(root) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    println!("id\tname\tkind\tterms");
    for p in &parties {
        let profile_path = root.join(p).join("profile.tsv");
        let text = match std::fs::read_to_string(&profile_path) {
            Ok(t) => t,
            Err(_) => continue, // malformed profile: skip
        };
        // Skip the header; the data row is the second line.
        let data = text.lines().nth(1).unwrap_or("");
        let cols: Vec<&str> = data.split('\t').collect();
        if cols.len() >= 4 {
            println!("{}\t{}\t{}\t{}", cols[0], cols[1], cols[2], cols[3]);
        }
    }
    ExitCode::from(0)
}

fn cmd_show(root: &PathBuf, id: &str) -> ExitCode {
    let profile = root.join(id).join("profile.tsv");
    let text = match std::fs::read_to_string(&profile) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("party show: {e}");
            return ExitCode::from(2);
        }
    };
    let data = text.lines().nth(1).unwrap_or("");
    let cols: Vec<&str> = data.split('\t').collect();
    if cols.len() < 4 {
        eprintln!("party show: malformed profile");
        return ExitCode::from(2);
    }
    println!("field\tvalue");
    println!("id\t{}", cols[0]);
    println!("name\t{}", cols[1]);
    println!("kind\t{}", cols[2]);
    println!("terms\t{}", cols[3]);
    ExitCode::from(0)
}

enum Sub {
    New { id: String, name: String, kind: String, terms: String },
    Ls,
    Show(String),
}

struct Opts {
    subcommand: Sub,
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub_name = args.next().ok_or_else(|| "usage: party <new|ls|show> ...".to_string())?;
    let mut root: Option<PathBuf> = None;
    let mut id: Option<String> = None;
    let mut name: Option<String> = None;
    let mut kind: Option<String> = None;
    let mut terms: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--id" => id = Some(args.next().ok_or_else(|| "--id requires ID".to_string())?),
            "--name" => name = Some(args.next().ok_or_else(|| "--name requires NAME".to_string())?),
            "--kind" => kind = Some(args.next().ok_or_else(|| "--kind requires KIND".to_string())?),
            "--terms" => terms = Some(args.next().ok_or_else(|| "--terms requires TERMS".to_string())?),
            _ => {
                if id.is_none() && matches!(sub_name.as_str(), "show") {
                    id = Some(a.to_string());
                }
            }
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let subcommand = match sub_name.as_str() {
        "new" => {
            let id = id.ok_or_else(|| "party new: --id is required".to_string())?;
            let name = name.ok_or_else(|| "party new: --name is required".to_string())?;
            let kind = kind.ok_or_else(|| "party new: --kind is required".to_string())?;
            let terms = terms.ok_or_else(|| "party new: --terms is required".to_string())?;
            Sub::New { id, name, kind, terms }
        }
        "ls" => Sub::Ls,
        "show" => {
            let id = id.ok_or_else(|| "party show: id argument required".to_string())?;
            Sub::Show(id)
        }
        _ => return Err(format!("unknown subcommand: {sub_name}")),
    };
    Ok(Opts { subcommand, root })
}
