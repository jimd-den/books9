//! `libbiz::journal` -- balance validator (use case).
//!
//! WHAT:    `validate(input: &str) -> Result<(), String>` parses a
//!          proposed entry (header + N data legs in the SRD's fixed
//!          13-column TSV shape) and accepts it iff every currency's
//!          debits equal its credits. Returns one-line stderr-ready
//!          reasons on rejection.
//! WHY:     SRD FR-1: "rejections never partially append." One gate,
//!          one answer, called identically by every driver that
//!          proposes an entry.
//! LAYER:   Use case. Pure: no I/O, no clocks, no globals.
//! DEPENDS: `std::collections` only. The on-disk append / hash chain
//!          lives in `crate::store`, which wraps this validator.
//! USED BY: `post` (driver) before any append; `close` (driver) for
//!          per-period totals; future reporting tools (Phase 3) for
//!          per-currency balance folds.
use std::collections::{BTreeSet, HashMap};

// Fixed column order from the SRD's journal line format.
// entry_id seq date entity currency account_debit account_credit
//   amount_minor party doc_ref tag provenance_hash prev_hash
const CURRENCY_COL: usize = 4;
const DEBIT_COL: usize = 5;
const CREDIT_COL: usize = 6;
const AMOUNT_COL: usize = 7;
const N_COLS: usize = 13;

/// Parse `input` as a proposed entry (header + N data legs), return
/// `Ok(())` if every currency balances, `Err(msg)` otherwise.
///
/// `msg` is a single line, suitable for stderr. On error the
/// message names the first mismatched currency so operators can
/// diagnose without re-parsing the input.
pub fn validate(input: &str) -> Result<(), String> {
    let mut lines = input.lines().filter(|l| !l.trim().is_empty());

    // Header: column names. Don't assert their values; we index by
    // position, per the SRD's fixed column order. This keeps the
    // validator robust to header reordering by future tools, as long
    // as they keep the SRD contract.
    let _header = lines.next().ok_or_else(|| "empty input".to_string())?;

    let mut debits: HashMap<String, i64> = HashMap::new();
    let mut credits: HashMap<String, i64> = HashMap::new();
    let mut saw_leg = false;

    for data in lines {
        saw_leg = true;
        let cols: Vec<&str> = data.split('\t').collect();
        if cols.len() != N_COLS {
            return Err(format!(
                "malformed entry: expected {N_COLS} columns, got {}",
                cols.len()
            ));
        }

        let currency = cols[CURRENCY_COL].to_string();
        let debit_acct = cols[DEBIT_COL];
        let credit_acct = cols[CREDIT_COL];
        let amount: i64 = cols[AMOUNT_COL]
            .parse()
            .map_err(|_| format!("amount_minor not an integer: {:?}", cols[AMOUNT_COL]))?;

        // Every leg is one debit + one credit with the same amount.
        // A posting builder that needs splits across multiple lines
        // per side is a separate tool, out of scope here.
        match (debit_acct.is_empty(), credit_acct.is_empty()) {
            (true, true) => {
                return Err(format!(
                    "unbalanced: leg has neither debit nor credit account (currency {currency})"
                ));
            }
            (false, true) => {
                *debits.entry(currency.clone()).or_insert(0) += amount;
            }
            (true, false) => {
                *credits.entry(currency).or_insert(0) += amount;
            }
            (false, false) => {
                return Err(format!(
                    "unbalanced: leg has both debit and credit accounts (currency {currency}); a leg must be one-sided"
                ));
            }
        }
    }

    if !saw_leg {
        return Err("no data line".to_string());
    }

    // Per-currency check. Iterate the union so each currency is
    // reported exactly once; pick the first mismatch by sorted key
    // for deterministic output (the SRD wants reproducible error
    // messages, not just reproducible totals).
    let mut ccys: BTreeSet<&String> = debits.keys().collect();
    ccys.extend(credits.keys());
    for ccy in ccys {
        let d = debits.get(ccy).copied().unwrap_or(0);
        let c = credits.get(ccy).copied().unwrap_or(0);
        if d != c {
            return Err(format!("unbalanced: currency {ccy} debits={d} credits={c}"));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> &'static str {
        "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash"
    }

    #[test]
    fn rejects_when_one_side_missing() {
        let s = format!("{h}\nid\t1\td\tent\tUSD\t1000\t\t100\t\t\t\t\th0\n", h = header());
        let err = validate(&s).unwrap_err();
        assert!(err.contains("unbalanced"), "got: {err}");
        assert!(err.contains("USD"), "got: {err}");
        assert!(!err.contains('\n'), "must be one line: {err}");
    }

    #[test]
    fn accepts_single_currency_two_leg_balanced() {
        let s = format!(
            "{h}\n\
             id\t1\td\tent\tUSD\t1000\t\t100\t\t\t\t\th0\n\
             id\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1\n",
            h = header()
        );
        assert!(validate(&s).is_ok());
    }

    #[test]
    fn rejects_multi_currency_off_by_one() {
        let s = format!(
            "{h}\n\
             id\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\th0\n\
             id\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1\n\
             id\t3\td\tent\tEUR\t1100\t\t200\t\t\t\t\th2\n\
             id\t4\td\tent\tEUR\t\t2100\t199\t\t\t\t\th3\n",
            h = header()
        );
        let err = validate(&s).unwrap_err();
        assert!(err.contains("unbalanced"), "got: {err}");
        assert!(err.contains("EUR"), "got: {err}");
        assert!(!err.contains("USD"), "got: {err}");
        assert!(!err.contains('\n'), "got: {err}");
    }

    #[test]
    fn accepts_multi_currency_all_balanced() {
        let s = format!(
            "{h}\n\
             id\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\th0\n\
             id\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1\n\
             id\t3\td\tent\tEUR\t1100\t\t200\t\t\t\t\th2\n\
             id\t4\td\tent\tEUR\t\t2100\t200\t\t\t\t\th3\n",
            h = header()
        );
        assert!(validate(&s).is_ok());
    }
}