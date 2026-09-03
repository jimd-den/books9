//
//! `price` -- apply prices to a sales order.
//!
//! WHAT:    Reads an SO at /biz/docs/so/{ID}.tsv, looks up
//!          each SKU's default_price in the items profile
//!          at /biz/items/{SKU}/profile.tsv, and writes the
//!          priced SO to {ID}.priced.tsv with
//!          unit_price_minor and line_total_minor filled in.
//! WHY:     The O2C pipeline needs a priced SO before
//!          `invoice` can emit a balanced journal entry.
//! LAYER:   Driver. The arithmetic and the file write are
//!          thin and named.
//! DEPENDS: `libbiz::item` (profile_tsv), stdlib.
//! USED BY: The O2C pipeline, between `so new` and `invoice`.

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
    cmd_price(&opts)
}

fn cmd_price(opts: &Opts) -> ExitCode {
    let so_dir = opts.root.join("docs").join("so");
    let so_path = so_dir.join(format!("{}.tsv", opts.so_id));
    let text = match std::fs::read_to_string(&so_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("price: read {}: {e}", so_path.display());
            return ExitCode::from(2);
        }
    };
    let mut lines = text.lines();
    let header = match lines.next() {
        Some(h) => h,
        None => {
            eprintln!("price: empty SO file");
            return ExitCode::from(2);
        }
    };
    // Output buffer: header + priced rows.
    let mut out = String::from(header);
    out.push('\n');
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 9 {
            eprintln!("price: malformed SO row ({} columns, expected 9): {line}", cols.len());
            return ExitCode::from(2);
        }
        let sku = cols[5];
        let qty: i64 = match cols[6].parse() {
            Ok(n) => n,
            Err(e) => {
                eprintln!("price: qty not an integer ({e}): {line}");
                return ExitCode::from(2);
            }
        };
        // Look up the item profile.
        let profile_path = opts.items_root.join(sku).join("profile.tsv");
        let profile = match new_project::item::profile_tsv(&profile_path) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("price: {e}");
                return ExitCode::from(2);
            }
        };
        let unit = profile.default_price;
        let line_total = qty.checked_mul(unit).unwrap_or_else(|| {
            eprintln!("price: overflow on qty * unit for {sku}");
            std::process::exit(2);
        });
        // Build the priced row: replace the last two columns
        // (unit_price_minor, line_total_minor) with the
        // computed values. cols 0..6 are unchanged.
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{unit}\t{line_total}\n",
            cols[0], cols[1], cols[2], cols[3], cols[4], cols[5], cols[6],
        ));
    }
    let priced_path = so_dir.join(format!("{}.priced.tsv", opts.so_id));
    if let Err(e) = new_project::store::write_atomic(&priced_path, out.as_bytes()) {
        eprintln!("price: write {}: {e}", priced_path.display());
        return ExitCode::from(2);
    }
    ExitCode::from(0)
}

struct Opts {
    so_id: String,
    root: PathBuf,
    items_root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let mut root: Option<PathBuf> = None;
    let mut so_id: Option<String> = None;
    let mut opts_items_root: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--so" => {
                so_id = Some(args.next().ok_or_else(|| "--so requires ID".to_string())?);
            }
            "--items-root" => {
                let p = args.next().ok_or_else(|| "--items-root requires PATH".to_string())?;
                opts_items_root = Some(PathBuf::from(p));
            }
            _ => return Err(format!("price: unknown flag: {a}")),
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let so_id = so_id.ok_or_else(|| "--so ID is required".to_string())?;
    let items_root = opts_items_root.unwrap_or_else(|| root.clone());
    Ok(Opts { so_id, root, items_root })
}
