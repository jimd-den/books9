//
// libbiz::reports -- the use-case fold over the journal.
//
// Phase 3 lands the full surface: `fold_journal` is the single
// primitive shared by `trial`, `balance`, and `stock`. Pure:
// same journal, same totals, no I/O beyond read.

use std::collections::BTreeMap;
use std::path::Path;

/// Per-currency debit/credit totals for one account.
/// First element is total debits (minor units), second is credits.
pub type AccountTotals = BTreeMap<String, (i64, i64)>;


/// Per-currency debit/credit/usd-normalized totals for one
/// account (the FX-aware shape). The third field is the
/// contribution to the USD-normalized column.
pub type FxAccountTotals = BTreeMap<String, (i64, i64, i64)>;



/// A line predicate used by callers to filter which lines the
/// fold considers. Reports pass closures like `|_| true` (for
/// `trial`) or `|line| line.account == requested_account` (for
/// `balance`).
pub type LineFilter = dyn Fn(&str) -> bool;

/// Fold every line of the journal at `path` into per-account
/// per-currency debit/credit totals. The fold is the heart of
/// the report suite (`trial`, `balance`, `stock`); a single
/// primitive that every report reuses so the math is in one
/// place.
///
/// WHAT:    One walk over the journal, one map per account,
///          one (debit, credit) tuple per currency.
/// WHY:     Reports are pure folds over the journal (SRD FR-5).
///          Putting the fold in one place means `trial` and
///          `balance` can never disagree on the math.
/// LAYER:   Use case. Pure: same path, same result, no I/O
///          beyond reading the journal file once.
/// DEPENDS: stdlib only.
/// USED BY: `bin/trial.rs`, `bin/balance.rs`, `bin/stock.rs`.
///
/// `filter` is a closure that takes one line (the data row, NOT
/// the header) and returns true to include it in the fold. `trial`
/// uses `|_| true`; `balance` uses a per-account match; future
/// reports (cash-flow, period-cut) supply their own.
///
/// Returns the per-account totals map. The map is BTreeMap so the
/// iteration order is stable; reports get a sorted output for free.
pub fn fold_journal(
    path: &Path,
    filter: &LineFilter,
) -> Result<BTreeMap<String, AccountTotals>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("read journal {}: {e}", path.display()))?;
    let mut out: BTreeMap<String, AccountTotals> = BTreeMap::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        // Skip the header (the first non-blank line is the
        // 13-column SRD header; data rows also have 13 columns,
        // so we cannot use column count to distinguish). The
        // header is the one line whose first column is
        // "entry_id".
        if line.starts_with("entry_id\t") {
            continue;
        }
        if !filter(line) {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 13 {
            // A malformed row: skip rather than reject, so a
            // bad line in the journal does not break every
            // report that ever runs. Future Phase: log it to
            // stderr so the operator knows.
            continue;
        }
        let currency = cols[4].to_string();
        let debit_acct = cols[5];
        let credit_acct = cols[6];
        let amount: i64 = match cols[7].parse() {
            Ok(n) => n,
            Err(_) => continue, // bad amount: skip
        };
        // Each leg is one-sided; only one of debit/credit is
        // non-empty. If both are empty or both are non-empty,
        // skip (malformed leg).
        match (debit_acct.is_empty(), credit_acct.is_empty()) {
            (false, true) => {
                let entry = out.entry(debit_acct.to_string()).or_default();
                let cell = entry.entry(currency).or_insert((0, 0));
                cell.0 += amount;
            }
            (true, false) => {
                let entry = out.entry(credit_acct.to_string()).or_default();
                let cell = entry.entry(currency).or_insert((0, 0));
                cell.1 += amount;
            }
            _ => continue, // neither or both: skip
        }
    }
    Ok(out)
}


/// Fold every line of the journal at `path` into per-account
/// per-currency debit/credit totals, with a `usd_normalized`
/// column populated by converting each non-USD amount via
/// `fx` (using the rate in effect on the row's date).
///
/// WHAT:    Same shape as `fold_journal`, but with a USD-
///          normalized cell per (account, currency) pair.
///          USD is native (no conversion); missing rates for
///          non-USD rows contribute 0 to the USD total.
/// WHY:     The SRD: "multi-currency first-class on every
///          line." Phase 4 ships the USD-normalized column;
///          per-currency minor-unit precision is a future
///          phase (see plans/phase4-fx.md §7).
/// LAYER:   Use case. Pure: same inputs, same result.
/// DEPENDS: `libbiz::fx` (read_table, lookup, convert).
/// USED BY: `trial --fx PATH`, `balance --fx PATH`,
///          `close --fx PATH` (unrealized gain).
///
/// The per-row rate is the rate in effect on the row's
/// `date` column (column index 2 of the 13-column TSV). All
/// legs of one entry share the same date, so the rate is
/// the same for the whole entry.
pub fn fold_journal_fx(
    path: &Path,
    rates: &crate::fx::RatesTable,
) -> Result<BTreeMap<String, FxAccountTotals>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("read journal {}: {e}", path.display()))?;
    let mut out: BTreeMap<String, FxAccountTotals> = BTreeMap::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with("entry_id\t") {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 13 {
            continue;
        }
        let date = cols[2];
        let currency = cols[4].to_string();
        let debit_acct = cols[5];
        let credit_acct = cols[6];
        let amount: i64 = match cols[7].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        // USD-normalized: USD is native; otherwise convert via
        // the rate for (currency, date). Missing rate -> 0.
        let usd_norm = if currency == "USD" {
            amount
        } else if let Some(rate) = rates.lookup(&currency, date) {
            crate::fx::convert(amount, rate)
        } else {
            0
        };
        match (debit_acct.is_empty(), credit_acct.is_empty()) {
            (false, true) => {
                let entry = out.entry(debit_acct.to_string()).or_default();
                let cell = entry.entry(currency).or_insert((0, 0, 0));
                cell.0 += amount;
                cell.2 += usd_norm;
            }
            (true, false) => {
                let entry = out.entry(credit_acct.to_string()).or_default();
                let cell = entry.entry(currency).or_insert((0, 0, 0));
                cell.1 += amount;
                cell.2 += usd_norm;
            }
            _ => {
                                continue;
            }
        }
    }
    Ok(out)
}


