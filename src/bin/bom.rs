//
//! `bom` -- bill of materials driver.
//!
//! WHAT:    Three subcommands on a BOMs root directory:
//!            bom new   -- create a leaf with bom.tsv
//!            bom ls    -- list every item with a BOM
//!            bom show  -- print one item's BOM
//! WHY:     The shop floor needs registered BOMs before
//!          MRP can compute component needs.

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
        Sub::New { ref item, ref component, ref qty, ref uom } => {
            cmd_new(&opts.root, item, component, qty, uom)
        }
        Sub::Ls => cmd_ls(&opts.root),
        Sub::Show(ref item) => cmd_show(&opts.root, item),
    }
}

fn cmd_new(root: &PathBuf, item: &str, component: &str, qty: &str, uom: &str) -> ExitCode {
    let leaf = root.join(item);
    if let Err(e) = std::fs::create_dir_all(&leaf) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    let bom = leaf.join("bom.tsv");
    // If the file doesn't exist yet, create it with the header.
    if !bom.exists() {
        let header = "item\tcomponent\tqty_per_unit\tuom\n";
        if let Err(e) = std::fs::write(&bom, header) {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    }
    // Append the new line.
    use std::io::Write;
    let mut f = match std::fs::OpenOptions::new().append(true).open(&bom) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = writeln!(f, "{item}\t{component}\t{qty}\t{uom}") {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    ExitCode::from(0)
}

fn cmd_ls(root: &PathBuf) -> ExitCode {
    let items = match new_project::bom::walk(root) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    println!("item");
    for i in items {
        println!("{}", i.display());
    }
    ExitCode::from(0)
}

fn cmd_show(root: &PathBuf, item: &str) -> ExitCode {
    let bom = root.join(item).join("bom.tsv");
    let lines = match new_project::bom::read_bom(&bom) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bom show: {e}");
            return ExitCode::from(2);
        }
    };
    println!("item\tcomponent\tqty_per_unit\tuom");
    for line in lines {
        println!("{}\t{}\t{}\t{}", line.item, line.component, line.qty_per_unit, line.uom);
    }
    ExitCode::from(0)
}

enum Sub {
    New { item: String, component: String, qty: String, uom: String },
    Ls,
    Show(String),
}

struct Opts {
    subcommand: Sub,
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub_name = args.next().ok_or_else(|| "usage: bom <new|ls|show> ...".to_string())?;
    let mut root: Option<PathBuf> = None;
    let mut item: Option<String> = None;
    let mut component: Option<String> = None;
    let mut qty: Option<String> = None;
    let mut uom: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--item" => item = Some(args.next().ok_or_else(|| "--item requires ITEM".to_string())?),
            "--component" => component = Some(args.next().ok_or_else(|| "--component requires COMP".to_string())?),
            "--qty" => qty = Some(args.next().ok_or_else(|| "--qty requires QTY".to_string())?),
            "--uom" => uom = Some(args.next().ok_or_else(|| "--uom requires UOM".to_string())?),
            _ => {
                if item.is_none() && matches!(sub_name.as_str(), "show") {
                    item = Some(a.to_string());
                }
            }
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let subcommand = match sub_name.as_str() {
        "new" => {
            let item = item.ok_or_else(|| "bom new: --item is required".to_string())?;
            let component = component.ok_or_else(|| "bom new: --component is required".to_string())?;
            let qty = qty.ok_or_else(|| "bom new: --qty is required".to_string())?;
            let uom = uom.ok_or_else(|| "bom new: --uom is required".to_string())?;
            Sub::New { item, component, qty, uom }
        }
        "ls" => Sub::Ls,
        "show" => {
            let item = item.ok_or_else(|| "bom show: item argument required".to_string())?;
            Sub::Show(item)
        }
        _ => return Err(format!("unknown subcommand: {sub_name}")),
    };
    Ok(Opts { subcommand, root })
}
