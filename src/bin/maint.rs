//
//! `maint` -- maintenance journal driver.
//!
//! WHAT:    `maint new` emits a balanced journal proposal:
//!          DR Maintenance Expense (6400) / CR Cash (1000)
//!          for the given amount.
//! WHY:     "Book a maintenance cost" is the maintenance
//!          lead's question.
//! LAYER:   Driver.
//! DEPENDS: stdlib.

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
    // Phase 7: hardcoded account numbers. A future cycle
    // adds an account-mapping config.
    println!("entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash");
    println!("{maint_id}\t1\t{date}\tent1\tUSD\t6400\t\t{amount}\t\tmaint:{maint_id}\t\t\tseed",
        maint_id = opts.maint_id, date = opts.date, amount = opts.amount);
    println!("{maint_id}\t2\t{date}\tent1\tUSD\t\t1000\t{amount}\t\tmaint:{maint_id}\t\t\th0",
        maint_id = opts.maint_id, date = opts.date, amount = opts.amount);
    ExitCode::from(0)
}

struct Opts {
    maint_id: String,
    asset: String,
    date: String,
    amount: i64,
    #[allow(dead_code)]
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub = args.next().ok_or_else(|| "maint: usage: maint new ...".to_string())?;
    if sub != "new" {
        return Err(format!("maint: unknown subcommand: {sub}"));
    }
    let mut root: Option<PathBuf> = None;
    let mut maint_id: Option<String> = None;
    let mut asset: Option<String> = None;
    let mut date: Option<String> = None;
    let mut amount_str: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--maint-id" => {
                maint_id = Some(args.next().ok_or_else(|| "--maint-id requires ID".to_string())?);
            }
            "--asset" => {
                asset = Some(args.next().ok_or_else(|| "--asset requires ID".to_string())?);
            }
            "--date" => {
                date = Some(args.next().ok_or_else(|| "--date requires DATE".to_string())?);
            }
            "--amount" => {
                amount_str = Some(args.next().ok_or_else(|| "--amount requires MINOR".to_string())?);
            }
            _ => return Err(format!("maint new: unknown flag: {a}")),
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let maint_id = maint_id.ok_or_else(|| "--maint-id is required".to_string())?;
    let asset = asset.ok_or_else(|| "--asset is required".to_string())?;
    let date = date.ok_or_else(|| "--date is required".to_string())?;
    let amount_str = amount_str.ok_or_else(|| "--amount is required".to_string())?;
    let amount: i64 = amount_str.parse()
        .map_err(|e| format!("--amount: not an integer: {e}"))?;
    Ok(Opts { maint_id, asset, date, amount, root })
}
