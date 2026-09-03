//
//! `ship` -- O2C shipping driver.
//!
//! WHAT:    `ship new` emits a journal proposal on stdout:
//!          DR COGS / CR Inventory at the shipped qty.
//! WHY:     The deferred O2C piece from Phase 5. The
//!          accounting shape is DR COGS / CR Inventory; a
//!          future cycle reads the standard cost from the
//!          item's profile (Phase 7 takes it as a flag for
//!          simplicity).
//! LAYER:   Driver.

use std::path::PathBuf;
use std::process::ExitCode;

const COGS_ACCOUNT: &str = "5000";
const INVENTORY_ACCOUNT: &str = "1300";

fn main() -> ExitCode {
    let opts = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let amount = opts.qty.checked_mul(opts.cogs_per_unit)
        .unwrap_or_else(|| {
            eprintln!("ship: overflow on {}*{}", opts.qty, opts.cogs_per_unit);
            std::process::exit(2);
        });
    println!("entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash");
    println!("sh{so_id}\t1\t{date}\tent1\tUSD\t{cogs}\t\t{amt}\t\tsh:{so_id}\t\t\tseed",
        so_id = opts.so_id, date = opts.date, cogs = COGS_ACCOUNT, amt = amount);
    println!("sh{so_id}\t2\t{date}\tent1\tUSD\t\t{inv}\t{amt}\t\tsh:{so_id}\t\t\th0",
        so_id = opts.so_id, date = opts.date, inv = INVENTORY_ACCOUNT, amt = amount);
    ExitCode::from(0)
}

struct Opts {
    so_id: String,
    date: String,
    qty: i64,
    cogs_per_unit: i64,
    #[allow(dead_code)]
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub = args.next().ok_or_else(|| "ship: usage: ship new ...".to_string())?;
    if sub != "new" {
        return Err(format!("ship: unknown subcommand: {sub}"));
    }
    let mut root: Option<PathBuf> = None;
    let mut so_id: Option<String> = None;
    let mut date: Option<String> = None;
    let mut qty_str: Option<String> = None;
    let mut cogs_str: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--so-id" => so_id = Some(args.next().ok_or_else(|| "--so-id requires ID".to_string())?),
            "--date" => date = Some(args.next().ok_or_else(|| "--date requires DATE".to_string())?),
            "--qty" => qty_str = Some(args.next().ok_or_else(|| "--qty requires QTY".to_string())?),
            "--cogs-per-unit" => {
                cogs_str = Some(args.next().ok_or_else(|| "--cogs-per-unit requires COST".to_string())?);
            }
            _ => return Err(format!("ship new: unknown flag: {a}")),
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let so_id = so_id.ok_or_else(|| "--so-id is required".to_string())?;
    let date = date.ok_or_else(|| "--date is required".to_string())?;
    let qty_str = qty_str.ok_or_else(|| "--qty is required".to_string())?;
    let cogs_str = cogs_str.ok_or_else(|| "--cogs-per-unit is required".to_string())?;
    let qty: i64 = qty_str.parse()
        .map_err(|e| format!("--qty: not an integer: {e}"))?;
    let cogs_per_unit: i64 = cogs_str.parse()
        .map_err(|e| format!("--cogs-per-unit: not an integer: {e}"))?;
    Ok(Opts { so_id, date, qty, cogs_per_unit, root })
}
