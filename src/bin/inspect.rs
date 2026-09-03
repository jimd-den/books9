//
//! `inspect` -- inspection lot driver.
//!
//! WHAT:    Two subcommands:
//!            inspect new   -- record an inspection lot
//!            inspect sample -- record a sub-sample
//! WHY:     Phase 7 ships the inspector's verdict (new);
//!          Phase P2P adds the sub-sample verb. Gating
//!          receipts on the verdict is a future cycle
//!          (depends on P2P grn).
//! LAYER:   Driver.

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
        Sub::New { lot_id, sku, qty, verdict, inspector, date } => {
            print_lot_record(&lot_id, &sku, qty, &verdict, &inspector, &date)
        }
        Sub::Sample { lot_id, qty, verdict } => {
            print_sample_record(&lot_id, qty, &verdict)
        }
    }
}

fn print_sample_record(lot_id: &str, qty: i64, verdict: &str) -> ExitCode {
    println!("lot_id\tqty\tverdict");
    println!("{}\t{}\t{}", lot_id, qty, verdict);
    ExitCode::from(0)
}

fn print_lot_record(lot_id: &str, sku: &str, qty: i64, verdict: &str, inspector: &str, date: &str) -> ExitCode {
    println!("lot_id\tsku\tqty\tverdict\tinspector\tdate");
    println!("{}\t{}\t{}\t{}\t{}\t{}", lot_id, sku, qty, verdict, inspector, date);
    ExitCode::from(0)
}

enum Sub {
    New { lot_id: String, sku: String, qty: i64, verdict: String, inspector: String, date: String },
    Sample { lot_id: String, qty: i64, verdict: String },
}

struct Opts {
    subcommand: Sub,
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub = args.next().ok_or_else(|| "inspect: usage: inspect <new|sample> ...".to_string())?;
    let mut root: Option<PathBuf> = None;
    let subcommand = match sub.as_str() {
        "new" => {
            let mut lot_id: Option<String> = None;
            let mut sku: Option<String> = None;
            let mut qty_str: Option<String> = None;
            let mut verdict: Option<String> = None;
            let mut inspector: Option<String> = None;
            let mut date: Option<String> = None;
            while let Some(a) = args.next() {
                match a.as_str() {
                    "--root" => {
                        let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                        root = Some(PathBuf::from(p));
                    }
                    "--lot-id" => lot_id = Some(args.next().ok_or_else(|| "--lot-id requires ID".to_string())?),
                    "--sku" => sku = Some(args.next().ok_or_else(|| "--sku requires SKU".to_string())?),
                    "--qty" => qty_str = Some(args.next().ok_or_else(|| "--qty requires QTY".to_string())?),
                    "--verdict" => verdict = Some(args.next().ok_or_else(|| "--verdict requires pass|fail".to_string())?),
                    "--inspector" => inspector = Some(args.next().ok_or_else(|| "--inspector requires NAME".to_string())?),
                    "--date" => date = Some(args.next().ok_or_else(|| "--date requires DATE".to_string())?),
                    _ => return Err(format!("inspect new: unknown flag: {a}")),
                }
            }
            let lot_id = lot_id.ok_or_else(|| "--lot-id is required".to_string())?;
            let sku = sku.ok_or_else(|| "--sku is required".to_string())?;
            let qty_str = qty_str.ok_or_else(|| "--qty is required".to_string())?;
            let qty: i64 = qty_str.parse().map_err(|e| format!("--qty: not an integer: {e}"))?;
            let verdict = verdict.ok_or_else(|| "--verdict is required".to_string())?;
            let inspector = inspector.ok_or_else(|| "--inspector is required".to_string())?;
            let date = date.ok_or_else(|| "--date is required".to_string())?;
            Sub::New { lot_id, sku, qty, verdict, inspector, date }
        }
        "sample" => {
            let mut lot_id: Option<String> = None;
            let mut qty_str: Option<String> = None;
            let mut verdict: Option<String> = None;
            while let Some(a) = args.next() {
                match a.as_str() {
                    "--root" => {
                        let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                        root = Some(PathBuf::from(p));
                    }
                    "--lot-id" => lot_id = Some(args.next().ok_or_else(|| "--lot-id requires ID".to_string())?),
                    "--qty" => qty_str = Some(args.next().ok_or_else(|| "--qty requires QTY".to_string())?),
                    "--verdict" => verdict = Some(args.next().ok_or_else(|| "--verdict requires pass|fail".to_string())?),
                    _ => return Err(format!("inspect sample: unknown flag: {a}")),
                }
            }
            let lot_id = lot_id.ok_or_else(|| "--lot-id is required".to_string())?;
            let qty_str = qty_str.ok_or_else(|| "--qty is required".to_string())?;
            let qty: i64 = qty_str.parse().map_err(|e| format!("--qty: not an integer: {e}"))?;
            let verdict = verdict.ok_or_else(|| "--verdict is required".to_string())?;
            Sub::Sample { lot_id, qty, verdict }
        }
        _ => return Err(format!("inspect: unknown subcommand: {sub}")),
    };
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    Ok(Opts { subcommand, root })
}
