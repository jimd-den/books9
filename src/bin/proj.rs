//
//! `proj` -- project driver.
//!
//! WHAT:    `proj new` creates a project directory at
//!          /biz/org/{code}/profile.tsv (same shape as the
//!          org tree; projects ARE orgs for Phase P2P).
//! WHY:     Spec Phase 5 includes projects. Phase P2P ships
//!          the read+write surface; routing payroll to a
//!          project is a future cycle.

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
    let leaf = opts.root.join(&opts.code);
    if leaf.exists() {
        eprintln!("proj new: {} already exists at {}", opts.code, leaf.display());
        return ExitCode::from(2);
    }
    if let Err(e) = std::fs::create_dir_all(&leaf) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    let body = format!("code\tname\tparent\n{}\t{}\t{}\n",
        opts.code, opts.name, opts.parent);
    let profile = leaf.join("profile.tsv");
    if let Err(e) = new_project::store::write_atomic(&profile, body.as_bytes()) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    ExitCode::from(0)
}

struct Opts {
    code: String,
    name: String,
    parent: String,
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub = args.next().ok_or_else(|| "proj: usage: proj new ...".to_string())?;
    if sub != "new" {
        return Err(format!("proj: unknown subcommand: {sub}"));
    }
    let mut root: Option<PathBuf> = None;
    let mut code: Option<String> = None;
    let mut name: Option<String> = None;
    let mut parent: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--code" => code = Some(args.next().ok_or_else(|| "--code requires CODE".to_string())?),
            "--name" => name = Some(args.next().ok_or_else(|| "--name requires NAME".to_string())?),
            "--parent" => parent = Some(args.next().ok_or_else(|| "--parent requires PARENT".to_string())?),
            _ => return Err(format!("proj: unknown flag: {a}")),
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let code = code.ok_or_else(|| "--code is required".to_string())?;
    let name = name.ok_or_else(|| "--name is required".to_string())?;
    let parent = parent.ok_or_else(|| "--parent is required".to_string())?;
    Ok(Opts { code, name, parent, root })
}
