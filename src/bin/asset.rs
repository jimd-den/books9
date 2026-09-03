//
//! `asset` -- fixed-asset register driver.
//!
//! WHAT:    Two subcommands: new (create a leaf with
//!          profile.tsv) and ls (list every asset).
//! WHY:     The asset register is the input to depreciation.
//! LAYER:   Driver.
//! DEPENDS: `libbiz::asset` (walk, read_profile), stdlib.

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
        Sub::New { ref id, ref name, ref cost, ref acquired, ref life, ref salvage } => {
            cmd_new(&opts.root, id, name, cost, acquired, life, salvage)
        }
        Sub::Ls => cmd_ls(&opts.root),
    }
}

fn cmd_new(root: &PathBuf, id: &str, name: &str, cost: &str, acquired: &str, life: &str, salvage: &str) -> ExitCode {
    let leaf = root.join(id);
    if leaf.exists() {
        eprintln!("asset new: {id} already exists at {}", leaf.display());
        return ExitCode::from(2);
    }
    if let Err(e) = std::fs::create_dir_all(&leaf) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    let body = format!(
        "id\tname\tcost_minor\tacquired\tuseful_life_months\tsalvage_minor\n{id}\t{name}\t{cost}\t{acquired}\t{life}\t{salvage}\n"
    );
    let profile = leaf.join("profile.tsv");
    if let Err(e) = new_project::store::write_atomic(&profile, body.as_bytes()) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    ExitCode::from(0)
}

fn cmd_ls(root: &PathBuf) -> ExitCode {
    let assets = match new_project::asset::walk(root) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    println!("id");
    for a in assets {
        println!("{}", a.display());
    }
    ExitCode::from(0)
}

enum Sub {
    New { id: String, name: String, cost: String, acquired: String, life: String, salvage: String },
    Ls,
}

struct Opts {
    subcommand: Sub,
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub_name = args.next().ok_or_else(|| "usage: asset <new|ls> ...".to_string())?;
    let mut root: Option<PathBuf> = None;
    let mut id: Option<String> = None;
    let mut name: Option<String> = None;
    let mut cost: Option<String> = None;
    let mut acquired: Option<String> = None;
    let mut life: Option<String> = None;
    let mut salvage: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--id" => id = Some(args.next().ok_or_else(|| "--id requires ID".to_string())?),
            "--name" => name = Some(args.next().ok_or_else(|| "--name requires NAME".to_string())?),
            "--cost" => cost = Some(args.next().ok_or_else(|| "--cost requires COST".to_string())?),
            "--acquired" => acquired = Some(args.next().ok_or_else(|| "--acquired requires DATE".to_string())?),
            "--life" => life = Some(args.next().ok_or_else(|| "--life requires MONTHS".to_string())?),
            "--salvage" => salvage = Some(args.next().ok_or_else(|| "--salvage requires SALVAGE".to_string())?),
            _ => return Err(format!("asset: unknown flag: {a}")),
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let subcommand = match sub_name.as_str() {
        "new" => {
            let id = id.ok_or_else(|| "asset new: --id is required".to_string())?;
            let name = name.ok_or_else(|| "asset new: --name is required".to_string())?;
            let cost = cost.ok_or_else(|| "asset new: --cost is required".to_string())?;
            let acquired = acquired.ok_or_else(|| "asset new: --acquired is required".to_string())?;
            let life = life.ok_or_else(|| "asset new: --life is required".to_string())?;
            let salvage = salvage.ok_or_else(|| "asset new: --salvage is required".to_string())?;
            Sub::New { id, name, cost, acquired, life, salvage }
        }
        "ls" => Sub::Ls,
        _ => return Err(format!("unknown subcommand: {sub_name}")),
    };
    Ok(Opts { subcommand, root })
}
