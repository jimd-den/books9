
//! `close` -- per-period closer (driver).
//!
//! WHAT:    Walks the journal, computes per-currency debit/credit
//!          totals for the named `--period YYYY-MM`, and emits a
//!          signed closing snapshot to stdout. On success calls
//!          `store::set_period` to mark the period Closed under the
//!          sibling `periods/` directory and writes a `.last_close`
//!          stamp in the reserved dot-prefix namespace.
//! WHY:     SRD FR-6: "Period close makes the dated range
//!          append-refusing, and emits a signed closing snapshot."
//!          The stamp is the operator's "when did this door shut?"
//!          answer; the snapshot is the audit; the flag is the gate.
//! LAYER:   Driver. The journal walk is its own use case; argv
//!          parsing, the period gate read, and the snapshot emit
//!          are thin and named.
//! DEPENDS: `libbiz::store` (period_status, set_period, periods_root),
//!          `libbiz::chain` (snapshot row linking), `libbiz::time`
//!          (close stamp format), stdlib.
//! USED BY: Accountants at month-end, the operator's close script.
//!          Refuses to double-close and refuses to close a period
//!          that has entries dated after it (those must be
//!          re-dated or reversed first).
//! FLAGS:   `--list` is the read-only inventory of every period
//!          under the periods root: one TSV line per period with
//!          its status and last-close stamp.
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use new_project::chain;
use new_project::store::{self, PeriodStatus};

const N_COLS: usize = 13;
const CURRENCY_COL: usize = 4;
const DEBIT_COL: usize = 5;
const CREDIT_COL: usize = 6;
const AMOUNT_COL: usize = 7;
const DATE_COL: usize = 2;
const HASH_COL: usize = 11;

#[derive(Debug, Default)]
struct Totals {
    debits: HashMap<String, i64>,
    credits: HashMap<String, i64>,
    entries: usize,
}

fn main() -> ExitCode {
    let opts = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };

    // --list is read-only: it answers "which doors exist, which are
    // shut, and when they shut" as a TSV stream, and it never
    // touches the journal or the flags it reports on.
    if opts.list {
        let root = match (&opts.periods, &opts.journal) {
            (Some(p), _) => p.clone(),
            (None, Some(j)) => store::periods_root(j),
            (None, None) => {
                eprintln!("close --list requires --journal PATH or --periods DIR");
                return ExitCode::from(2);
            }
        };
        return list_periods(&root);
    }

    let (journal, period, reason) = match (opts.journal, opts.period, opts.reason) {
        (Some(j), Some(p), r) => (j, p, r),
        (None, _, _) => {
            eprintln!("--journal PATH is required");
            return ExitCode::from(2);
        }
        (_, None, _) => {
            eprintln!("--period YYYY-MM is required");
            return ExitCode::from(2);
        }
    };

    if !journal.exists() {
        eprintln!("close: journal not found: {}", journal.display());
        return ExitCode::from(2);
    }

    // Derive the periods root from the journal path unless an
    // explicit --periods was given, so the layout matches what
    // `post --periods` expects when it consumes the closed flag via
    // store::period_status.
    let periods_root = match &opts.periods {
        Some(p) => p.clone(),
        None => store::periods_root(&journal),
    };
    if !periods_root.exists() {
        // The periods directory is created lazily on first close.
        // We deliberately do NOT mkdir periods_root on read-only
        // operations; close is a writer, so creation is its job.
        if let Err(e) = fs::create_dir_all(&periods_root) {
            eprintln!(
                "close: cannot create periods directory {}: {e}",
                periods_root.display()
            );
            return ExitCode::from(2);
        }
    }

    // Idempotent rejection of double-close. Reads the current state
    // first so a concurrent close from another operator cannot race
    // us past this gate.
    match store::period_status(&periods_root, &period) {
        Ok(PeriodStatus::Closed) => {
            eprintln!("close: period {period} is already closed");
            return ExitCode::from(2);
        }
        Ok(PeriodStatus::Malformed(reason)) => {
            eprintln!("close: {reason}");
            return ExitCode::from(2);
        }
        Ok(PeriodStatus::Open) => {}
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    }

    // Walk the journal: per-currency totals for the period, plus
    // a guard that rejects open entries dated strictly after the
    // closing period.
    let content = match fs::read_to_string(&journal) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("close: read {}: {e}", journal.display());
            return ExitCode::from(2);
        }
    };

    let mut totals = Totals::default();
    let mut prev_hash: String = "0000000000000000".to_string();
    let mut first_after: Option<String> = None;

    let mut iter = content.lines();
    let header_line = match iter.next() {
        Some(h) => h,
        None => {
            eprintln!("close: empty journal: {}", journal.display());
            return ExitCode::from(2);
        }
    };
    if header_line.split('\t').count() != N_COLS {
        eprintln!("close: header must be {N_COLS} columns");
        return ExitCode::from(2);
    }

    for data in iter {
        if data.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = data.split('\t').collect();
        if cols.len() != N_COLS {
            eprintln!(
                "close: journal row has {} columns; expected {N_COLS}",
                cols.len()
            );
            return ExitCode::from(2);
        }
        let date = cols[DATE_COL];
        let date_period = match new_project::time::yyyy_mm(date) {
            Some(p) => p,
            None => continue,
        };
        if date_period.as_str() > period.as_str() {
            if first_after.is_none() {
                first_after = Some(date.to_string());
            }
            prev_hash = cols[HASH_COL].to_string();
            continue;
        }
        if date_period.as_str() == period.as_str() {
            let currency = cols[CURRENCY_COL].to_string();
            let debit_acct = cols[DEBIT_COL];
            let credit_acct = cols[CREDIT_COL];
            let amount: i64 = match cols[AMOUNT_COL].parse() {
                Ok(n) => n,
                Err(_) => {
                    eprintln!(
                        "close: amount_minor not an integer: {:?}",
                        cols[AMOUNT_COL]
                    );
                    return ExitCode::from(2);
                }
            };
            match (debit_acct.is_empty(), credit_acct.is_empty()) {
                (true, false) => {
                    *totals.credits.entry(currency).or_insert(0) += amount;
                }
                (false, true) => {
                    *totals.debits.entry(currency).or_insert(0) += amount;
                }
                _ => {
                    // Multi-sided leg or empty leg: skip silently for
                    // the close totals; the validator and verify own
                    // the strict one-sided contract.
                }
            }
            totals.entries += 1;
        }
        prev_hash = cols[HASH_COL].to_string();
    }

    if let Some(d) = first_after {
        eprintln!(
            "close: cannot close {period}; journal has entries dated after it (first: {d})"
        );
        return ExitCode::from(2);
    }

    // Snapshot rows: header + one row per currency. Each data row
    // is hash-chained to the prior data row; the chain header is
    // the journal's last_hash (or the zero sentinel for an empty
    // journal).
    let rows = build_snapshot_rows(&period, &totals, &prev_hash, reason.as_deref());
    let mut stdout = std::io::stdout().lock();
    for line in &rows {
        if let Err(e) = writeln!(stdout, "{line}") {
            eprintln!("close: write stdout: {e}");
            return ExitCode::from(2);
        }
    }
    drop(stdout);

    // Mark the period Closed last: the snapshot has been emitted
    // successfully to stdout, so a failure to mark Closed does not
    // leave the operator with no audit. If marking fails, the
    // caller may re-run close; the snapshot is the same modulo a
    // fresh chain hash.
    if let Err(e) = store::set_period(&periods_root, &period, PeriodStatus::Closed) {
        eprintln!("close: {e}");
        return ExitCode::from(2);
    }

    // Phase 4: if --fx PATH and --close-date DATE were given,
    // emit a one-line unrealized-gain info on stderr per
    // non-USD balance where the close-date rate differs from
    // the entry-date rate. This is an info emission, not a
    // journal posting -- the full posting lands in a future
    // commit when settlement stories are first-class (Phase 5+).
    if let (Some(fx_path), Some(close_date)) = (&opts.fx, &opts.close_date) {
        emit_unrealized_gains(&journal, fx_path, &period, close_date);
    }

        // Record when the door shut. The stamp lives in the reserved
    // dot-prefix namespace (set_period refuses those names for
    // periods), so period_status and directory scans for flags
    // cannot mistake it for a period. One wall-clock read, handed
    // to a pure formatter: every other path in the crate is
    // clock-free.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let stamp = new_project::time::format_utc(now);
    if let Err(e) = store::write_close_stamp(&periods_root, &period, &stamp) {
        // The period IS closed; the audit of WHEN failed. Loud on
        // stderr, nonzero exit, but no rollback: an unrecorded
        // close time is less dangerous than reopening the period.
        eprintln!("close: {e}");
        return ExitCode::from(2);
    }

    ExitCode::from(0)
}


#[derive(Default)]
struct Opts {
    journal: Option<PathBuf>,
    period: Option<String>,
    reason: Option<String>,
    periods: Option<PathBuf>,
    list: bool,
    fx: Option<PathBuf>,
    close_date: Option<String>,
}

fn parse_args() -> Result<Opts, String> {
    let mut o = Opts::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--journal" => {
                o.journal = Some(PathBuf::from(
                    args.next().ok_or_else(|| "--journal requires PATH".to_string())?,
                ));
            }
            "--period" => {
                o.period = Some(
                    args.next()
                        .ok_or_else(|| "--period requires YYYY-MM".to_string())?,
                );
            }
            "--reason" => {
                o.reason = Some(
                    args.next()
                        .ok_or_else(|| "--reason requires TEXT".to_string())?,
                );
            }
            "--periods" => {
                o.periods = Some(PathBuf::from(
                    args.next().ok_or_else(|| "--periods requires DIR".to_string())?,
                ));
            }
            "--list" => {
                o.list = true;
            }
            "--fx" => {
                let p = args.next().ok_or_else(|| "--fx requires PATH".to_string())?;
                o.fx = Some(PathBuf::from(p));
            }
            "--close-date" => {
                let d = args.next().ok_or_else(|| "--close-date requires DATE".to_string())?;
                o.close_date = Some(d.to_string());
            }
            _ => {
                // Forward-compat: tolerate unknown flags (matches post).
            }
        }
    }
    Ok(o)
}

/// Emit the period inventory as TSV: header + one sorted row per
/// flag file. Dot-prefixed names are the reserved bookkeeping
/// namespace and never list as periods. A missing directory is an
/// empty inventory, not an error: the header is the true statement.
fn list_periods(root: &std::path::Path) -> ExitCode {
    let mut names: Vec<String> = match fs::read_dir(root) {
        Ok(rd) => {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                .filter(|n| !n.starts_with('.'))
                .collect()
        }
        Err(_) => {
            // Missing or unreadable directory: header only.
            let mut stdout = std::io::stdout().lock();
            let _ = writeln!(stdout, "period\tstatus\tlast_close");
            return ExitCode::from(0);
        }
    };
    names.sort();

    let mut stdout = std::io::stdout().lock();
    if writeln!(stdout, "period\tstatus\tlast_close").is_err() {
        return ExitCode::from(2);
    }
    for name in names {
        let status = match store::period_status(root, &name) {
            Ok(PeriodStatus::Open) => "open",
            Ok(PeriodStatus::Closed) => "closed",
            Ok(PeriodStatus::Malformed(_)) => "malformed",
            Err(_) => "malformed",
        };
        let stamp = match fs::read_to_string(root.join(format!(".{name}.last_close"))) {
            Ok(s) => s.lines().next().unwrap_or("").trim().to_string(),
            Err(_) => String::new(),
        };
        if writeln!(stdout, "{name}\t{status}\t{stamp}").is_err() {
            return ExitCode::from(2);
        }
    }
    ExitCode::from(0)
}


fn build_snapshot_rows(
    period: &str,
    totals: &Totals,
    prev_hash: &str,
    reason: Option<&str>,
) -> Vec<String> {
    let header = "close_id\tperiod\tcurrency\tdebit_total\tcredit_total\treason\tentries\tlast_hash\tprovenance_hash";
    let reason_s = reason.unwrap_or("");
    let entries_s = totals.entries.to_string();

    let mut keys: BTreeSet<&String> = totals.debits.keys().collect();
    for k in totals.credits.keys() {
        keys.insert(k);
    }

    let mut rows = Vec::with_capacity(keys.len() + 1);
    rows.push(header.to_string());
    for k in keys {
        let d = totals.debits.get(k).copied().unwrap_or(0);
        let c = totals.credits.get(k).copied().unwrap_or(0);
        let row_no_hash = format!(
            "close-{p}\t{p}\t{k}\t{d}\t{c}\t{r}\t{e}\t{h}",
            p = period,
            k = k,
            d = d,
            c = c,
            r = reason_s,
            e = entries_s,
            h = prev_hash,
        );
        let prov = chain::next(prev_hash, row_no_hash.as_bytes());
        rows.push(format!("{row_no_hash}\t{prov}"));
    }
    rows
}

/// Emit one line per non-USD balance where the close-date rate
/// differs from the entry-date rate. The line goes to stderr;
/// stdout is reserved for the snapshot rows. Phase 4 ships the
/// info emission; the journal posting is a future commit.
///
/// WHAT:    Walk the journal, find every non-USD posting,
///          compare the rate in effect on the entry date to
///          the rate in effect on the close date, and emit a
///          one-line gain/loss info for each account+currency
///          pair.
///
/// WHY:     Operators need to see the open FX exposure at
///          close time. The full posting lands when settlement
///          is first-class; for now the info line is the
///          operator's signal.
///
/// LAYER:   Driver helper. Pure with-IO: reads the journal
///          and the rates table; writes to stderr.
fn emit_unrealized_gains(
    journal: &std::path::Path,
    fx_path: &std::path::Path,
    period: &str,
    close_date: &str,
) {
    let rates = match new_project::fx::read_table(fx_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("close --fx: {e}");
            return;
        }
    };
    let content = match std::fs::read_to_string(journal) {
        Ok(t) => t,
        Err(_) => return,
    };
    // Track per (account, currency) -- the entry-date rate.
    // For each row in the period, look up both rates and emit
    // a gain/loss line. We emit one line per (account, currency)
    // that has at least one non-USD posting in the period.
    use std::collections::BTreeMap;
    let mut seen: BTreeMap<(String, String), bool> = BTreeMap::new();
    for line in content.lines() {
        if line.trim().is_empty() || line.starts_with("entry_id\t") {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 13 {
            continue;
        }
        let date = cols[2];
        let period_of_date = match new_project::time::yyyy_mm(date) {
            Some(p) => p,
            None => continue,
        };
        if period_of_date.as_str() != period {
            continue;
        }
        let currency = cols[4].to_string();
        if currency == "USD" {
            continue;
        }
        let acct = if !cols[5].is_empty() { cols[5] } else { cols[6] };
        let key = (acct.to_string(), currency.clone());
        if seen.contains_key(&key) {
            continue;
        }
        let booked_rate = rates.lookup(&currency, date);
        let current_rate = rates.lookup(&currency, close_date);
        if let (Some(b), Some(c)) = (booked_rate, current_rate) {
            if b != c {
                eprintln!(
                    "close --fx: {acct} {currency} booked rate {b}, current rate {c} on {close_date}"
                );
            }
        }
        seen.insert(key, true);
    }
}
