//
//! `item` -- master-data driver for SKUs.
//!
//! WHAT:    Three subcommands on an items root directory:
//!            item new   -- create a leaf with profile.tsv
//!            item ls    -- list every item
//!            item show  -- print one item's profile
//! WHY:     Sales orders and price lookups need registered
//!          SKUs. `item` is the master-data entry point for
//!          inventory.
//! LAYER:   Driver. Argv parsing, subcommand dispatch, and
//!          the three subcommand bodies are thin and named.
//! DEPENDS: `libbiz::item` (walk, profile_tsv), stdlib.
//! USED BY: Sales clerks, the O2C pipeline.

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
        Sub::New { ref id, ref name, ref uom, ref default_price } => {
            cmd_new(&opts.root, id, name, uom, default_price)
        }
        Sub::Ls => cmd_ls(&opts.root),
        Sub::Show(ref id) => cmd_show(&opts.root, id),
    }
}

fn cmd_new(root: &PathBuf, id: &str, name: &str, uom: &str, default_price: &str) -> ExitCode {
    let leaf = root.join(id);
    if leaf.exists() {
        eprintln!("item new: {id} already exists at {}", leaf.display());
        return ExitCode::from(2);
    }
    if let Err(e) = std::fs::create_dir_all(&leaf) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    let body = format!(
        "id\tname\tuom\tdefault_price\n{id}\t{name}\t{uom}\t{default_price}\n"
    );
    let profile = leaf.join("profile.tsv");
    if let Err(e) = new_project::store::write_atomic(&profile, body.as_bytes()) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    ExitCode::from(0)
}

fn cmd_ls(root: &PathBuf) -> ExitCode {
    let items = match new_project::item::walk(root) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    println!("id\tname\tuom\tdefault_price");
    for p in &items {
        let profile = root.join(p).join("profile.tsv");
        match new_project::item::profile_tsv(&profile) {
            Ok(p) => println!("{}\t{}\t{}\t{}", p.id, p.name, p.uom, p.default_price),
            Err(_) => continue, // malformed profile: skip
        }
    }
    ExitCode::from(0)
}

fn cmd_show(root: &PathBuf, id: &str) -> ExitCode {
    let profile = root.join(id).join("profile.tsv");
    let p = match new_project::item::profile_tsv(&profile) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("item show: {e}");
            return ExitCode::from(2);
        }
    };
    println!("field\tvalue");
    println!("id\t{}", p.id);
    println!("name\t{}", p.name);
    println!("uom\t{}", p.uom);
    println!("default_price\t{}", p.default_price);
    ExitCode::from(0)
}

enum Sub {
    New { id: String, name: String, uom: String, default_price: String },
    Ls,
    Show(String),
}

struct Opts {
    subcommand: Sub,
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub_name = args.next().ok_or_else(|| "usage: item <new|ls|show> ...".to_string())?;
    let mut root: Option<PathBuf> = None;
    let mut id: Option<String> = None;
    let mut name: Option<String> = None;
    let mut uom: Option<String> = None;
    let mut default_price: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--id" => id = Some(args.next().ok_or_else(|| "--id requires ID".to_string())?),
            "--name" => name = Some(args.next().ok_or_else(|| "--name requires NAME".to_string())?),
            "--uom" => uom = Some(args.next().ok_or_else(|| "--uom requires UOM".to_string())?),
            "--default-price" => {
                default_price = Some(args.next().ok_or_else(|| "--default-price requires PRICE".to_string())?);
            }
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
            let id = id.ok_or_else(|| "item new: --id is required".to_string())?;
            let name = name.ok_or_else(|| "item new: --name is required".to_string())?;
            let uom = uom.ok_or_else(|| "item new: --uom is required".to_string())?;
            let default_price = default_price.ok_or_else(|| "item new: --default-price is required".to_string())?;
            Sub::New { id, name, uom, default_price }
        }
        "ls" => Sub::Ls,
        "show" => {
            let id = id.ok_or_else(|| "item show: id argument required".to_string())?;
            Sub::Show(id)
        }
        _ => return Err(format!("unknown subcommand: {sub_name}")),
    };
    Ok(Opts { subcommand, root })
}
