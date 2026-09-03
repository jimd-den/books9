//
//! `grn` -- goods received note driver (with ap match).
//!
//! WHAT:    Two subcommands:
//!            grn new   -- record a goods received note
//!            grn match --po ID --grn ID
//!                      -- three-way match (po + grn)
//! WHY:     The P2P pipeline continues from po to grn to
//!          ap. `grn match` is the controller's check that
//!          the receipt matches the order.

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
        Sub::New { ref grn_id, ref po, ref date, ref received } => {
            cmd_new(&opts.root, grn_id, po, date, received)
        }
        Sub::Match { ref po, ref grn } => cmd_match(&opts.root, po, grn),
    }
}

struct Line {
    sku: String,
    qty: i64,
}

fn cmd_new(root: &PathBuf, grn_id: &str, po: &str, date: &str, lines: &[Line]) -> ExitCode {
    let grn_dir = root.join("docs").join("grn");
    if let Err(e) = std::fs::create_dir_all(&grn_dir) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    let grn_path = grn_dir.join(format!("{grn_id}.tsv"));
    if grn_path.exists() {
        eprintln!("grn new: {grn_id} already exists at {}", grn_path.display());
        return ExitCode::from(2);
    }
    let vendor = match read_po_vendor(root, po) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let mut body = String::from(
        "grn_id\tpo_id\tvendor\tdate\tsku\tqty\n",
    );
    for line in lines {
        body.push_str(&format!(
            "{grn_id}\t{po}\t{vendor}\t{date}\t{sku}\t{qty}\n",
            grn_id = grn_id, po = po, vendor = vendor,
            date = date, sku = line.sku, qty = line.qty,
        ));
    }
    if let Err(e) = new_project::store::write_atomic(&grn_path, body.as_bytes()) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    println!("{grn_id}");
    ExitCode::from(0)
}

fn cmd_match(root: &PathBuf, po: &str, grn: &str) -> ExitCode {
    let po_lines = match read_po_lines(root, po) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let grn_lines = match read_grn_lines(root, grn) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let mut mismatches: Vec<String> = Vec::new();
    for pl in &po_lines {
        match grn_lines.iter().find(|gl| gl.sku == pl.sku) {
            Some(gl) => {
                if gl.qty != pl.qty {
                    mismatches.push(format!(
                        "mismatch: sku {} expected {} got {}",
                        pl.sku, pl.qty, gl.qty
                    ));
                }
            }
            None => {
                mismatches.push(format!(
                    "missing: sku {} ordered {} but not received",
                    pl.sku, pl.qty
                ));
            }
        }
    }
    if mismatches.is_empty() {
        println!("ok");
        ExitCode::from(0)
    } else {
        for m in &mismatches {
            println!("{m}");
        }
        ExitCode::from(1)
    }
}

fn read_po_vendor(root: &PathBuf, po: &str) -> Result<String, String> {
    let path = root.join("docs").join("po").join(format!("{po}.tsv"));
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("read po {}: {e}", path.display()))?;
    let data = text.lines().nth(1)
        .ok_or_else(|| format!("po {po} has no data row"))?;
    let cols: Vec<&str> = data.split('\t').collect();
    if cols.len() < 2 {
        return Err(format!("po {po}: malformed header"));
    }
    Ok(cols[1].to_string())
}

fn read_po_lines(root: &PathBuf, po: &str) -> Result<Vec<Line>, String> {
    let path = root.join("docs").join("po").join(format!("{po}.tsv"));
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("read po {}: {e}", path.display()))?;
    let mut out: Vec<Line> = Vec::new();
    for line in text.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 7 {
            continue;
        }
        let qty: i64 = cols[6].parse()
            .map_err(|e| format!("po: qty not an integer: {e}"))?;
        out.push(Line { sku: cols[5].to_string(), qty });
    }
    Ok(out)
}

fn read_grn_lines(root: &PathBuf, grn: &str) -> Result<Vec<Line>, String> {
    let path = root.join("docs").join("grn").join(format!("{grn}.tsv"));
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("read grn {}: {e}", path.display()))?;
    let mut out: Vec<Line> = Vec::new();
    for line in text.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 6 {
            continue;
        }
        let qty: i64 = cols[5].parse()
            .map_err(|e| format!("grn: qty not an integer: {e}"))?;
        out.push(Line { sku: cols[4].to_string(), qty });
    }
    Ok(out)
}

enum Sub {
    New { grn_id: String, po: String, date: String, received: Vec<Line> },
    Match { po: String, grn: String },
}

struct Opts {
    subcommand: Sub,
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub = args.next().ok_or_else(|| "grn: usage: grn <new|match> ...".to_string())?;
    let mut root: Option<PathBuf> = None;
    let mut grn_id: Option<String> = None;
    let mut po: Option<String> = None;
    let mut date: Option<String> = None;
    let mut received: Vec<Line> = Vec::new();
    let subcommand = if sub == "new" {
        while let Some(a) = args.next() {
            match a.as_str() {
                "--root" => {
                    let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                    root = Some(PathBuf::from(p));
                }
                "--grn-id" => grn_id = Some(args.next().ok_or_else(|| "--grn-id requires ID".to_string())?),
                "--po" => po = Some(args.next().ok_or_else(|| "--po requires ID".to_string())?),
                "--date" => date = Some(args.next().ok_or_else(|| "--date requires DATE".to_string())?),
                "--received" => {
                    let raw = args.next().ok_or_else(|| "--received requires SKU,QTY".to_string())?;
                    let mut parts = raw.split(',');
                    let sku = parts.next().ok_or_else(|| "--received: missing sku".to_string())?
                        .to_string();
                    let qty_s = parts.next().ok_or_else(|| "--received: missing qty".to_string())?;
                    let qty: i64 = qty_s.parse()
                        .map_err(|e| format!("--received: qty not an integer: {e}"))?;
                    received.push(Line { sku, qty });
                }
                _ => return Err(format!("grn new: unknown flag: {a}")),
            }
        }
        let grn_id = grn_id.ok_or_else(|| "--grn-id is required".to_string())?;
        let po = po.ok_or_else(|| "--po is required".to_string())?;
        let date = date.ok_or_else(|| "--date is required".to_string())?;
        if received.is_empty() {
            return Err("at least one --received is required".to_string());
        }
        Sub::New { grn_id, po, date, received }
    } else if sub == "match" {
        while let Some(a) = args.next() {
            match a.as_str() {
                "--root" => {
                    let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                    root = Some(PathBuf::from(p));
                }
                "--po" => po = Some(args.next().ok_or_else(|| "--po requires ID".to_string())?),
                "--grn" => grn_id = Some(args.next().ok_or_else(|| "--grn requires ID".to_string())?),
                _ => return Err(format!("grn match: unknown flag: {a}")),
            }
        }
        let po = po.ok_or_else(|| "--po is required".to_string())?;
        let grn = grn_id.ok_or_else(|| "--grn is required".to_string())?;
        Sub::Match { po, grn }
    } else {
        return Err(format!("unknown subcommand: {sub}"));
    };
    Ok(Opts { subcommand, root: root.clone().unwrap() })
}
