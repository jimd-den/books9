//
//! `so` -- sales order driver.
//!
//! WHAT:    One subcommand: `so new`. Creates a sales order
//!          at /biz/docs/so/{NNNNNN}.tsv with a header row
//!          and one data row per --line flag.
//! WHY:     The O2C pipeline starts with the SO. The priced
//!          SO (with unit_price_minor) is the input to
//!          `price` and then `invoice`; the unpriced SO is
//!          the user's intent.
//! LAYER:   Driver. Argv parsing and the file write are
//!          thin and named.
//! DEPENDS: `libbiz::store` (write_atomic), stdlib.
//! USED BY: Sales clerks, the O2C pipeline (`price`,
//!          `invoice`).

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
    cmd_new(&opts)
}

/// A single line on the SO, as entered on the command line.
#[derive(Debug, Clone)]
struct Line {
    sku: String,
    qty: i64,
}

fn cmd_new(opts: &Opts) -> ExitCode {
    // The SO lives at <root>/docs/so/<so_id>.tsv.
    let so_dir = opts.root.join("docs").join("so");
    if let Err(e) = std::fs::create_dir_all(&so_dir) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    let so_path = so_dir.join(format!("{}.tsv", opts.so_id));
    if so_path.exists() {
        eprintln!("so new: {} already exists at {}", opts.so_id, so_path.display());
        return ExitCode::from(2);
    }
    // Header + one row per line. unit_price_minor and
    // line_total_minor are blank in the unpriced SO; the
    // `price` step fills them in.
    let mut body = String::from(
        "so_id\tparty\tdate\tcurrency\tterms\tsku\tqty\tunit_price_minor\tline_total_minor\n",
    );
    for line in &opts.lines {
        body.push_str(&format!(
            "{so_id}\t{party}\t{date}\t{currency}\t{terms}\t{sku}\t{qty}\t\t\n",
            so_id = opts.so_id,
            party = opts.party,
            date = opts.date,
            currency = opts.currency,
            terms = opts.terms,
            sku = line.sku,
            qty = line.qty,
        ));
    }
    if let Err(e) = new_project::store::write_atomic(&so_path, body.as_bytes()) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    // Echo the SO id on stdout so the operator can pipe.
    println!("{}", opts.so_id);
    ExitCode::from(0)
}

struct Opts {
    so_id: String,
    party: String,
    date: String,
    currency: String,
    terms: String,
    lines: Vec<Line>,
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub = args.next().ok_or_else(|| "so: usage: so new --root DIR ...".to_string())?;
    if sub != "new" {
        return Err(format!("so: unknown subcommand: {sub}"));
    }
    let mut root: Option<PathBuf> = None;
    let mut so_id: Option<String> = None;
    let mut party: Option<String> = None;
    let mut date: Option<String> = None;
    let mut currency: Option<String> = None;
    let mut terms: Option<String> = None;
    let mut lines: Vec<Line> = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--so-id" => {
                so_id = Some(args.next().ok_or_else(|| "--so-id requires ID".to_string())?);
            }
            "--party" => {
                party = Some(args.next().ok_or_else(|| "--party requires PARTY".to_string())?);
            }
            "--date" => {
                date = Some(args.next().ok_or_else(|| "--date requires DATE".to_string())?);
            }
            "--currency" => {
                currency = Some(args.next().ok_or_else(|| "--currency requires CCY".to_string())?);
            }
            "--terms" => {
                terms = Some(args.next().ok_or_else(|| "--terms requires TERMS".to_string())?);
            }
            "--line" => {
                let raw = args.next().ok_or_else(|| "--line requires SKU,QTY".to_string())?;
                let mut parts = raw.split(',');
                let sku = parts.next().ok_or_else(|| "--line: missing sku".to_string())?
                    .to_string();
                let qty_s = parts.next().ok_or_else(|| "--line: missing qty".to_string())?;
                let qty: i64 = qty_s.parse()
                    .map_err(|e| format!("--line: qty not an integer: {e}"))?;
                if parts.next().is_some() {
                    return Err("--line: too many commas (expected SKU,QTY)".to_string());
                }
                lines.push(Line { sku, qty });
            }
            _ => return Err(format!("so new: unknown flag: {a}")),
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let so_id = so_id.ok_or_else(|| "--so-id is required".to_string())?;
    let party = party.ok_or_else(|| "--party is required".to_string())?;
    let date = date.ok_or_else(|| "--date is required".to_string())?;
    let currency = currency.ok_or_else(|| "--currency is required".to_string())?;
    let terms = terms.ok_or_else(|| "--terms is required".to_string())?;
    if lines.is_empty() {
        return Err("at least one --line is required".to_string());
    }
    Ok(Opts { so_id, party, date, currency, terms, lines, root })
}
