//
//! `balance` -- point-in-time account balance (driver).
//!
//! WHAT:    Reads a journal and emits the per-currency debit/credit
//!          totals for one named account on stdout. The account is
//!          mandatory; the date filter (`--as-of`) is optional and
//!          restricts the fold to entries dated at or before the
//!          given date.
//! WHY:     "What is the balance of account 1100 right now?" is the
//!          most-asked question in accounting. A fold over the
//!          journal answers it deterministically; the result is
//!          reproducible from the journal alone (FR-5).
//! LAYER:   Driver. Argv parsing, the fold, the print, and the
//!          optional date filter are thin and named.
//! DEPENDS: `libbiz::reports` (fold), `libbiz::time::yyyy_mm` (date
//!          period extraction), stdlib.
//! USED BY: AP/AR clerks, the loop plan's "playtest the role of a
//!          clerk" step, future report tools that need a single
//!          account's view.

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
    let target = opts.account.clone();
    if let Some(fx_path) = &opts.fx {
        let rates = match new_project::fx::read_table(fx_path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        };
        let totals = match new_project::reports::fold_journal_fx(
            &opts.journal, &rates,
        ) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        };
        let one = totals.get(&opts.account);
        emit_balance(&opts.account, one, &opts.format, true);
    } else {
        let totals_2 = match new_project::reports::fold_journal(
            &opts.journal,
            &move |line| line_account(line) == target,
        ) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        };
        // Convert 2-tuple to 3-tuple (usd_norm=0 for non-FX).
        let totals: std::collections::BTreeMap<
            String, std::collections::BTreeMap<String, (i64, i64, i64)>
        > = totals_2.into_iter()
            .map(|(acct, per_ccy)| {
                let per_ccy: std::collections::BTreeMap<String, (i64, i64, i64)> =
                    per_ccy.into_iter().map(|(c, (d, cr))| (c, (d, cr, 0))).collect();
                (acct, per_ccy)
            })
            .collect();
        let one = totals.get(&opts.account);
        emit_balance(&opts.account, one, &opts.format, false);
    }
    ExitCode::from(0)
}

/// Extract the non-empty account from a 13-column data row.
/// Returns the empty string for a malformed row (the fold's
/// filter then excludes it).
fn line_account(line: &str) -> String {
    let cols: Vec<&str> = line.split('\t').collect();
    if cols.len() != 13 {
        return String::new();
    }
    if !cols[5].is_empty() {
        return cols[5].to_string();
    }
    if !cols[6].is_empty() {
        return cols[6].to_string();
    }
    String::new()
}



fn emit_balance(
    account: &str,
    totals: Option<&std::collections::BTreeMap<String, (i64, i64, i64)>>,
    format: &str,
    with_usd: bool,
) {
    if format == "json" {
        print!("{{\"account\":\"{}\",\"rows\": [", json_escape(account));
        if let Some(per_ccy) = totals {
            let mut first = true;
            for (ccy, (debit, credit, _usd)) in per_ccy {
                if !first { print!(","); }
                first = false;
                print!("{{\"currency\":\"{}\",\"debit\":{},\"credit\":{}}}", json_escape(ccy), debit, credit);
            }
        }
        println!("]}}");
    } else {
        let header = if with_usd {
            "account\tcurrency\tdebit\tcredit\tusd_normalized"
        } else {
            "account\tcurrency\tdebit\tcredit"
        };
        println!("{header}");
        if let Some(per_ccy) = totals {
            for (ccy, (debit, credit, usd_norm)) in per_ccy {
                if with_usd {
                    println!("{account}\t{ccy}\t{debit}\t{credit}\t{usd_norm}");
                } else {
                    println!("{account}\t{ccy}\t{debit}\t{credit}");
                }
            }
        }
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

struct Opts {
    journal: PathBuf,
    account: String,
    fx: Option<PathBuf>,
    format: String,
}

fn parse_args() -> Result<Opts, String> {
    let mut journal: Option<PathBuf> = None;
    let mut account: Option<String> = None;
    let mut format = "tsv".to_string();
    let mut opts_fx: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--journal" => {
                let p = args.next().ok_or_else(|| "--journal requires PATH".to_string())?;
                journal = Some(PathBuf::from(p));
            }
            "--account" => {
                let a = args.next().ok_or_else(|| "--account requires ACCOUNT".to_string())?;
                account = Some(a.to_string());
            }
            "--fx" => {
                let p = args.next().ok_or_else(|| "--fx requires PATH".to_string())?;
                opts_fx = Some(PathBuf::from(p));
            }
            _ => {}
        }
    }
    let journal = journal.ok_or_else(|| "balance: --journal PATH is required".to_string())?;
    let account = account.ok_or_else(|| "balance: --account ACCOUNT is required".to_string())?;
    Ok(Opts { journal, account, fx: opts_fx, format })
}
