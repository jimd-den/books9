//
//! `org` -- organization tree driver.
//!
//! WHAT:    Two subcommands on an orgs root directory:
//!            org new  -- create a leaf with profile.tsv
//!            org ls   -- list every department
//! WHY:     The org tree is the seed of the cost-center
//!          hierarchy. Phase 6 ships it; payroll reads it.

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
        Sub::New { ref code, ref name, ref parent } => {
            cmd_new(&opts.root, code, name, parent)
        }
        Sub::Ls => cmd_ls(&opts.root),
    }
}

fn cmd_new(root: &PathBuf, code: &str, name: &str, parent: &str) -> ExitCode {
    let leaf = root.join(code);
    if leaf.exists() {
        eprintln!("org new: {code} already exists at {}", leaf.display());
        return ExitCode::from(2);
    }
    if let Err(e) = std::fs::create_dir_all(&leaf) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    let body = format!("code\tname\tparent\tcost_center\n{code}\t{name}\t{parent}\t{code}\n");
    let profile = leaf.join("profile.tsv");
    if let Err(e) = new_project::store::write_atomic(&profile, body.as_bytes()) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    ExitCode::from(0)
}

fn cmd_ls(root: &PathBuf) -> ExitCode {
    let depts = match new_project::org::walk(root) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    println!("code");
    for d in depts {
        println!("{}", d.display());
    }
    ExitCode::from(0)
}

enum Sub {
    New { code: String, name: String, parent: String },
    Ls,
}

struct Opts {
    subcommand: Sub,
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub_name = args.next().ok_or_else(|| "usage: org <new|ls> ...".to_string())?;
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
            _ => return Err(format!("org: unknown flag: {a}")),
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let subcommand = match sub_name.as_str() {
        "new" => {
            let code = code.ok_or_else(|| "org new: --code is required".to_string())?;
            let name = name.ok_or_else(|| "org new: --name is required".to_string())?;
            let parent = parent.ok_or_else(|| "org new: --parent is required".to_string())?;
            Sub::New { code, name, parent }
        }
        "ls" => Sub::Ls,
        _ => return Err(format!("unknown subcommand: {sub_name}")),
    };
    Ok(Opts { subcommand, root })
}
