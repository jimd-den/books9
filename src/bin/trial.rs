//
//! `trial` -- trial balance (driver).
//!
//! WHAT:    Reads a journal and emits a TSV on stdout with one
//!          row per (account, currency) showing debit/credit
//!          totals, sorted by account path.
//! WHY:     The trial balance is the accountant's first stop at
//!          month-end: a one-glance picture of every account's
//!          debits and credits. It is a pure fold over the
//!          journal (FR-5), so any disagreement between this
//!          and the cache is a corruption signal, not a bug.
//! LAYER:   Driver. Argv parsing, file read, fold call, and
//!          print are kept thin and named.
//! DEPENDS: `libbiz::reports` (fold), `libbiz::store` (HEADER_LINE,
//!          the schema source of truth), stdlib.
//! USED BY: Accountants, audit, the loop plan's
//!          "playtest the role of a clerk at month-end" step.

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
    // FR-5: a fold over the journal. `trial` is the "all
    // accounts" view; the filter is `|_| true`. When --fx
    // is given, use the FX-aware fold; otherwise the plain
    // fold (output shape is byte-identical to Phase 3).
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
        emit_trial(&totals, &opts.format, true);
    } else {
        // fold_journal returns (i64, i64); convert to (i64, i64, i64)
        // with usd_norm=0 for the non-FX path.
        let totals_2 = match new_project::reports::fold_journal(
            &opts.journal, &|_line| true,
        ) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        };
        let totals: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, (i64, i64, i64)>,
        > = totals_2.into_iter()
            .map(|(acct, per_ccy)| {
                let per_ccy: std::collections::BTreeMap<String, (i64, i64, i64)> =
                    per_ccy.into_iter()
                        .map(|(ccy, (d, c))| (ccy, (d, c, 0)))
                        .collect();
                (acct, per_ccy)
            })
            .collect();
        emit_trial(&totals, &opts.format, false);
    }
    ExitCode::from(0)
}

/// Print the trial balance as a TSV: header + one row per
/// (account, currency), sorted by account then currency. Pure
/// output: same totals, same bytes.
fn emit_trial(
    totals: &std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, (i64, i64, i64)>,
    >,
    format: &str,
    with_usd: bool,
) {
    if format == "json" {
        // Minimal hand-rolled JSON (no serde dependency).
        // Schema: {"rows": [{"account": "...", "currency": "...", "debit": N, "credit": N, "usd_normalized": N}, ...]}
        print!("{{\"rows\": [");
        let mut first_row = true;
        for (account, per_ccy) in totals {
            for (ccy, (debit, credit, usd_norm)) in per_ccy {
                if !first_row { print!(","); }
                first_row = false;
                print!(
                    "{{\"account\":\"{}\",\"currency\":\"{}\",\"debit\":{},\"credit\":{},\"usd_normalized\":{}}}",
                    json_escape(account), json_escape(ccy), debit, credit,
                    if with_usd { *debit } else { 0 }
                );
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
        for (account, per_ccy) in totals {
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

/// Minimal JSON string escape (handles \ and " and control chars).
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

fn print_tsv_fx(
    totals: &std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, (i64, i64, i64)>,
    >,
) {
    println!("account\tcurrency\tdebit\tcredit\tusd_normalized");
    for (account, per_ccy) in totals {
        for (ccy, (debit, credit, usd_norm)) in per_ccy {
            println!("{account}\t{ccy}\t{debit}\t{credit}\t{usd_norm}");
        }
    }
}

struct Opts {
    journal: PathBuf,
    fx: Option<PathBuf>,
    format: String,
}

fn parse_args() -> Result<Opts, String> {
    let mut journal: Option<PathBuf> = None;
    let mut opts_fx: Option<PathBuf> = None;
    let mut format = "tsv".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--journal" => {
                let p = args.next().ok_or_else(|| "--journal requires PATH".to_string())?;
                journal = Some(PathBuf::from(p));
            }
            "--fx" => {
                let p = args.next().ok_or_else(|| "--fx requires PATH".to_string())?;
                opts_fx = Some(PathBuf::from(p));
            }
            "--format" => {
                let f = args.next().ok_or_else(|| "--format requires tsv|json".to_string())?;
                if f != "tsv" && f != "json" {
                    return Err(format!("--format must be tsv or json (got {f:?})"));
                }
                format = f.to_string();
            }
            _ => {
                // Tolerate unknown flags (matches post's
                // forward-compat behavior).
            }
        }
    }
    let journal = journal.ok_or_else(|| "trial: --journal PATH is required".to_string())?;
    Ok(Opts { journal, fx: opts_fx, format })
}
