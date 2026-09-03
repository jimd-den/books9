//
//! `coa` -- chart-of-accounts driver.
//!
//! WHAT:    Three subcommands on a CoA root directory:
//!            coa ls    -- list every account
//!            coa show  -- print one account's profile
//!            coa new   -- create a leaf with profile.tsv
//! WHY:     The CoA is a directory tree; this tool is the
//!          end-user surface for inspecting and extending it.
//!          A future `coa rm` will land alongside the FR-2
//!          reversing-entry correction story (Phase 3 deferred).
//! LAYER:   Driver. Argv parsing, subcommand dispatch, and the
//!          three subcommand bodies are thin and named.
//! DEPENDS: `libbiz::coa` (walk, profile_tsv), stdlib.
//! USED BY: CoA admins, the loop plan's "playtest the role
//!          of an admin" step.

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
        Sub::Ls => cmd_ls(&opts.root),
        Sub::Show(ref acct) => cmd_show(&opts.root, acct),
        Sub::New {
            ref account,
            ref name,
            ref kind,
            ref normal_side,
        } => cmd_new(
            &opts.root,
            account,
            name,
            kind,
            normal_side,
        ),
    }
}

fn cmd_ls(root: &PathBuf) -> ExitCode {
    let accounts = match new_project::coa::walk(root) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    println!("account\tname\tkind\tnormal_side\tstatus");
    for path in &accounts {
        let profile_path = root.join(path).join("profile.tsv");
        let profile = match new_project::coa::profile_tsv(&profile_path) {
            Ok(p) => p,
            Err(_) => continue, // a malformed profile does not stop the listing
        };
        let acct = path.to_string_lossy();
        println!("{acct}\t{}\t{}\t{}\t{}",
            profile.name, profile.kind, profile.normal_side, profile.status);
    }
    ExitCode::from(0)
}

fn cmd_show(root: &PathBuf, account: &str) -> ExitCode {
    let path = root.join(account).join("profile.tsv");
    let profile = match new_project::coa::profile_tsv(&path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    // Emit the profile as a key\tvalue TSV. The header is the
    // field name; consumers (e.g. `coa show 1100 | post --coa
    // <tmp>`) can grep by field.
    println!("field\tvalue");
    println!("code\t{}", profile.code);
    println!("name\t{}", profile.name);
    println!("kind\t{}", profile.kind);
    println!("normal_side\t{}", profile.normal_side);
    println!("parent\t{}", profile.parent);
    println!("status\t{}", profile.status);
    ExitCode::from(0)
}

fn cmd_new(
    root: &PathBuf,
    account: &str,
    name: &str,
    kind: &str,
    normal_side: &str,
) -> ExitCode {
    let leaf = root.join(account);
    if leaf.exists() {
        eprintln!("coa new: account {account} already exists at {}", leaf.display());
        return ExitCode::from(2);
    }
    if let Err(e) = std::fs::create_dir_all(&leaf) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    // Build the profile.tsv: header + one data row. status
    // defaults to "active" for new accounts; the admin can
    // edit it directly for "inactive" or "archived".
    let body = format!(
        "code\tname\tkind\tnormal_side\tparent\tstatus\n{account}\t{name}\t{kind}\t{normal_side}\t\tactive\n"
    );
    let profile_path = leaf.join("profile.tsv");
    if let Err(e) = new_project::store::write_atomic(&profile_path, body.as_bytes()) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    ExitCode::from(0)
}

enum Sub {
    Ls,
    Show(String),
    New {
        account: String,
        name: String,
        kind: String,
        normal_side: String,
    },
}

struct Opts {
    subcommand: Sub,
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub_name = args.next().ok_or_else(|| "usage: coa <ls|show|new> ...".to_string())?;
    let mut root: Option<PathBuf> = None;
    let mut account: Option<String> = None;
    let mut name: Option<String> = None;
    let mut kind: Option<String> = None;
    let mut normal_side: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--name" => {
                name = Some(args.next().ok_or_else(|| "--name requires NAME".to_string())?);
            }
            "--kind" => {
                kind = Some(args.next().ok_or_else(|| "--kind requires KIND".to_string())?);
            }
            "--normal-side" => {
                normal_side = Some(args.next().ok_or_else(|| "--normal-side requires SIDE".to_string())?);
            }
            _ => {
                // Positional arg: the account name (show/new).
                if account.is_none() {
                    account = Some(a.to_string());
                }
            }
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let subcommand = match sub_name.as_str() {
        "ls" => Sub::Ls,
        "show" => {
            let acct = account.ok_or_else(|| "coa show: account argument required".to_string())?;
            Sub::Show(acct)
        }
        "new" => {
            let account = account.ok_or_else(|| "coa new: account argument required".to_string())?;
            let name = name.ok_or_else(|| "coa new: --name is required".to_string())?;
            let kind = kind.ok_or_else(|| "coa new: --kind is required".to_string())?;
            let normal_side = normal_side.ok_or_else(|| "coa new: --normal-side is required".to_string())?;
            Sub::New { account, name, kind, normal_side }
        }
        _ => return Err(format!("unknown subcommand: {sub_name}")),
    };
    Ok(Opts { subcommand, root })
}
