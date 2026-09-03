//
//! `mrp` -- material requirements planning driver.
//!
//! WHAT:    Reads SOs at /biz/docs/so/{ID}.priced.tsv, looks
//!          up each item's BOM, and emits a TSV on stdout with
//!          the total qty needed per component.
//! WHY:     "What do I need to buy?" is the shop floor's first
//!          question. The output is byte-stable (FR-3).
//! LAYER:   Driver. Argv parsing, the compute, and the print
//!          are thin and named.
//! DEPENDS: `libbiz::mrp` (compute, DemandLine), stdlib.
//! USED BY: Shop floor lead, the integration test in
//!          `tests/mrp_byte_stable.rs`.

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
    cmd_mrp(&opts)
}

fn cmd_mrp(opts: &Opts) -> ExitCode {
    let mut demand: Vec<new_project::mrp::DemandLine> = Vec::new();
    for raw_so in &opts.demand {
        // raw_so is "so:NNNNNN"; strip the prefix.
        let so_id = raw_so.strip_prefix("so:")
            .ok_or_else(|| format!("mrp: --demand must be so:ID, got {raw_so}"))
            .unwrap_or("");
        let so_path = opts.so_root.join("docs").join("so")
            .join(format!("{so_id}.priced.tsv"));
        let text = match std::fs::read_to_string(&so_path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("mrp: read {}: {e}", so_path.display());
                return ExitCode::from(2);
            }
        };
        let mut lines = text.lines();
        let _header = match lines.next() {
            Some(h) => h,
            None => {
                eprintln!("mrp: empty SO file {}", so_path.display());
                return ExitCode::from(2);
            }
        };
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() != 9 {
                continue;
            }
            let qty: i64 = match cols[6].parse() {
                Ok(n) => n,
                Err(_) => continue,
            };
            demand.push(new_project::mrp::DemandLine {
                item: cols[5].to_string(),
                qty,
            });
        }
    }
    let totals = match new_project::mrp::compute(&demand, &opts.bom_root) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("mrp: {e}");
            return ExitCode::from(2);
        }
    };
    // FR-3 byte-stability: the output is BTreeMap iteration
    // (sorted by component) + a single println per row.
    // No wall-clock, no random, no env reads.
    println!("component\tqty\tuom");
    for (component, (qty, uom)) in &totals {
        println!("{component}\t{qty}\t{uom}");
    }
    ExitCode::from(0)
}

struct Opts {
    demand: Vec<String>,
    bom_root: PathBuf,
    so_root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let mut demand: Vec<String> = Vec::new();
    let mut bom_root: Option<PathBuf> = None;
    let mut so_root: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--demand" => {
                let s = args.next().ok_or_else(|| "--demand requires so:ID".to_string())?;
                demand.push(s.to_string());
            }
            "--bom-root" => {
                let p = args.next().ok_or_else(|| "--bom-root requires PATH".to_string())?;
                bom_root = Some(PathBuf::from(p));
            }
            "--so-root" => {
                let p = args.next().ok_or_else(|| "--so-root requires PATH".to_string())?;
                so_root = Some(PathBuf::from(p));
            }
            _ => return Err(format!("mrp: unknown flag: {a}")),
        }
    }
    let bom_root = bom_root.ok_or_else(|| "--bom-root DIR is required".to_string())?;
    let so_root = so_root.ok_or_else(|| "--so-root DIR is required".to_string())?;
    if demand.is_empty() {
        return Err("at least one --demand so:ID is required".to_string());
    }
    Ok(Opts { demand, bom_root, so_root })
}
