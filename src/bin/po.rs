//
//! `po` -- purchase order driver.
//!
//! WHAT:    `po new` creates a purchase order at
//!          /biz/docs/po/{ID}.tsv with a header and one
//!          row per --line flag. `po ls` lists every PO.
//! WHY:     The P2P pipeline starts with the PO.
//! LAYER:   Driver.
//! DEPENDS: `libbiz::store` (write_atomic), stdlib.

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
        Sub::New { ref po_id, ref vendor, ref date, ref lines } => {
            cmd_new(&opts.root, po_id, vendor, date, lines)
        }
        Sub::Ls => cmd_ls(&opts.root),
    }
}

struct Line {
    sku: String,
    qty: i64,
}

fn cmd_new(root: &PathBuf, po_id: &str, vendor: &str, date: &str, lines: &[Line]) -> ExitCode {
    let po_dir = root.join("docs").join("po");
    if let Err(e) = std::fs::create_dir_all(&po_dir) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    let po_path = po_dir.join(format!("{po_id}.tsv"));
    if po_path.exists() {
        eprintln!("po new: {po_id} already exists at {}", po_path.display());
        return ExitCode::from(2);
    }
    let mut body = String::from(
        "po_id\tvendor\tdate\tcurrency\tterms\tsku\tqty\n",
    );
    for line in lines {
        body.push_str(&format!(
            "{po_id}\t{vendor}\t{date}\tUSD\tNet-30\t{sku}\t{qty}\n",
            po_id = po_id, vendor = vendor, date = date,
            sku = line.sku, qty = line.qty,
        ));
    }
    if let Err(e) = new_project::store::write_atomic(&po_path, body.as_bytes()) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    println!("{po_id}");
    ExitCode::from(0)
}

fn cmd_ls(root: &PathBuf) -> ExitCode {
    let po_dir = root.join("docs").join("po");
    let entries = match std::fs::read_dir(&po_dir) {
        Ok(e) => e,
        Err(_) => {
            // No POs yet: empty list, not an error.
            println!("po_id");
            return ExitCode::from(0);
        }
    };
    println!("po_id");
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        // Strip the .tsv suffix.
        if let Some(stripped) = name.strip_suffix(".tsv") {
            println!("{stripped}");
        }
    }
    ExitCode::from(0)
}

enum Sub {
    New { po_id: String, vendor: String, date: String, lines: Vec<Line> },
    Ls,
}

struct Opts {
    subcommand: Sub,
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub = args.next().ok_or_else(|| "po: usage: po <new|ls> ...".to_string())?;
    let mut root: Option<PathBuf> = None;
    let mut po_id: Option<String> = None;
    let mut vendor: Option<String> = None;
    let mut date: Option<String> = None;
    let mut lines: Vec<Line> = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--po-id" => po_id = Some(args.next().ok_or_else(|| "--po-id requires ID".to_string())?),
            "--vendor" => vendor = Some(args.next().ok_or_else(|| "--vendor requires VENDOR".to_string())?),
            "--date" => date = Some(args.next().ok_or_else(|| "--date requires DATE".to_string())?),
            "--line" => {
                let raw = args.next().ok_or_else(|| "--line requires SKU,QTY".to_string())?;
                let mut parts = raw.split(',');
                let sku = parts.next().ok_or_else(|| "--line: missing sku".to_string())?
                    .to_string();
                let qty_s = parts.next().ok_or_else(|| "--line: missing qty".to_string())?;
                let qty: i64 = qty_s.parse()
                    .map_err(|e| format!("--line: qty not an integer: {e}"))?;
                lines.push(Line { sku, qty });
            }
            _ => return Err(format!("po: unknown flag: {a}")),
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let subcommand = match sub.as_str() {
        "new" => {
            let po_id = po_id.ok_or_else(|| "po new: --po-id is required".to_string())?;
            let vendor = vendor.ok_or_else(|| "po new: --vendor is required".to_string())?;
            let date = date.ok_or_else(|| "po new: --date is required".to_string())?;
            if lines.is_empty() {
                return Err("po new: at least one --line is required".to_string());
            }
            Sub::New { po_id, vendor, date, lines }
        }
        "ls" => Sub::Ls,
        _ => return Err(format!("unknown subcommand: {sub}")),
    };
    Ok(Opts { subcommand, root })
}
