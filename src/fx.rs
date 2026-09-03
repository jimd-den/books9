//
//! `libbiz::fx` -- foreign-exchange rates and conversion.
//!
//! WHAT:    A TSV rates reader (`read_table`), an in-memory
//!          `RatesTable` for O(log n) lookup by (from, date),
//!          and a `convert` function that multiplies an
//!          amount by a rate with overflow checks.
//! WHY:     SRD: "FX realized/unrealized gains post automatically
//!          at settlement and close." Phase 4 ships the table
//!          and the conversion; the posting lands in commits 13-14.
//! LAYER:   Entity. Pure: same path, same table. No I/O, no
//!          clocks, no state beyond the read table.
//! DEPENDS: stdlib only.
//! USED BY: `bin/trial.rs` and `bin/balance.rs` (when given
//!          `--fx PATH`), `bin/close.rs` (unrealized gain at
//!          close), `bin/fx.rs` (the rates driver).

use std::collections::BTreeMap;
use std::path::Path;

/// One FX rate. `rate` is 10^-8 of `to` per unit of `from`.
/// E.g. 1.10 USD/EUR -> `rate: 110_000_000`. Inverse pairs are
/// explicit; the table does not auto-invert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rate {
    pub date: String,
    pub from: String,
    pub to: String,
    pub rate: i64,
}

/// In-memory rates table keyed by (from, date). The `to` side
/// is implicit in the lookup: a (from, date) -> rate entry
/// means "1 from = (rate / 10^8) to".
#[derive(Debug, Clone)]
pub struct RatesTable {
    inner: BTreeMap<(String, String), i64>,
}

impl RatesTable {
    /// Look up the rate for `from` (in 10^-8 of `to`) on `date`.
    /// Returns `None` if the (from, date) pair has no rate entry.
    pub fn lookup(&self, from: &str, date: &str) -> Option<i64> {
        self.inner.get(&(from.to_string(), date.to_string())).copied()
    }
}

/// Read a TSV rates file at `path` and return a `RatesTable`.
///
/// WHAT:    A pure reader. The file has a header
///          `date\tfrom\tto\trate` and one data row per rate.
/// WHY:     The TSV is the editor's surface (a clerk edits
///          `1.10` in their editor); the in-memory form is
///          i64 with 8 decimal places of precision (the SRD's
///          "no floats cross a tool boundary" rule).
/// LAYER:   Entity.
/// DEPENDS: stdlib only.
/// USED BY: every driver that gains an `--fx PATH` flag.
pub fn read_table(path: &Path) -> Result<RatesTable, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("read fx rates {}: {e}", path.display()))?;
    let mut inner: BTreeMap<(String, String), i64> = BTreeMap::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if i == 0 {
            // First non-blank line is the header; trust its
            // presence but do not assert on its values.
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 4 {
            return Err(format!(
                "fx rates row {}: expected 4 columns, got {}",
                i + 1,
                cols.len()
            ));
        }
        let rate = parse_rate(cols[3]).map_err(|e| {
            format!("fx rates row {}: {e} (rate was {:?})", i + 1, cols[3])
        })?;
        inner.insert((cols[1].to_string(), cols[0].to_string()), rate);
    }
    Ok(RatesTable { inner })
}

/// Parse a decimal rate string to i64 in 10^-8 of `to` per
/// unit of `from`. Accepts "1.10" -> 110_000_000; "1" -> 100_000_000;
/// "0.91" -> 91_000_000. Rejects more than 8 decimal places.
fn parse_rate(s: &str) -> Result<i64, String> {
    let s = s.trim();
    let (int_part, frac_part) = match s.split_once('.') {
        Some((i, f)) => (i, f),
        None => (s, ""),
    };
    if int_part.is_empty() || int_part == "-" {
        return Err("rate must have a leading digit".to_string());
    }
    if frac_part.len() > 8 {
        return Err("rate has more than 8 decimal places".to_string());
    }
    // Pad frac_part to 8 digits.
    let mut frac = frac_part.to_string();
    while frac.len() < 8 {
        frac.push('0');
    }
    let combined = format!("{int_part}{frac}");
    combined.parse::<i64>().map_err(|e| format!("rate not an integer: {e}"))
}


/// Convert an amount in `from` minor units to `to` minor units
/// using `rate` (in 10^-8 of `to` per unit of `from`).
///
/// WHAT:    `to_amount = from_amount * rate / 10^8` with
///          checked multiplication and integer division.
/// WHY:     The fold that produces the USD-normalized column
///          calls this for every non-USD row. The math is in
///          one place so the report can never disagree with
///          the close-time unrealized gain.
/// LAYER:   Entity. Pure: same inputs, same output.
/// DEPENDS: stdlib only.
/// USED BY: `trial --fx PATH`, `balance --fx PATH`,
///          `close --fx PATH` (unrealized gain).
///
/// Truncation: the division is integer (truncates toward
/// zero). A penny is not invented; the column total is the
/// sum of the per-row truncated amounts.
///
/// Panics on overflow with a labeled message. Same shape as
/// `money::add` and `journal::add`: silent wraparound would
/// corrupt the books; the panic is the audit-friendly signal.
pub fn convert(from_amount: i64, rate: i64) -> i64 {
    const SCALE: i64 = 100_000_000; // 10^8
    let product = from_amount
        .checked_mul(rate)
        .expect("overflow: libbiz::fx::convert");
    product / SCALE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rate_handles_one_point_one_zero() {
        assert_eq!(parse_rate("1.10").unwrap(), 110_000_000);
    }

    #[test]
    fn parse_rate_handles_one_point_zero_eight() {
        assert_eq!(parse_rate("1.08").unwrap(), 108_000_000);
    }

    #[test]
    fn parse_rate_handles_integer() {
        assert_eq!(parse_rate("1").unwrap(), 100_000_000);
    }

    #[test]
    fn parse_rate_handles_zero_point_nine_one() {
        assert_eq!(parse_rate("0.91").unwrap(), 91_000_000);
    }

    #[test]
    fn parse_rate_rejects_more_than_eight_decimal_places() {
        assert!(parse_rate("1.123456789").is_err());
    }
}
