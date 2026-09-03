//
// tests/reports_fold_journal_fx.rs
//
//! Pin the contract for `reports::fold_journal_fx` -- the
//! use case for USD-normalized totals. Given a journal and a
//! rates table, the fold returns per-account totals per
//! currency AND a `usd_normalized` column.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::fs;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_tempdir(label: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("books9-{label}-{pid}-{n}"));
    fs::create_dir_all(&dir).expect("create tempdir");
    dir
}

fn make_journal(dir: &PathBuf, rows: &[&str]) -> PathBuf {
    let path = dir.join("journal.tsv");
    let header = "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash";
    let mut content = String::from(header);
    content.push('\n');
    for row in rows {
        content.push_str(row);
        content.push('\n');
    }
    fs::write(&path, content).expect("write journal");
    path
}

fn make_rates(dir: &PathBuf, body: &str) -> PathBuf {
    let path = dir.join("rates.tsv");
    fs::write(&path, body).expect("write rates");
    path
}

#[test]
fn fold_journal_fx_normalizes_eur_to_usd() {
    let tmp = fresh_tempdir("fx-fold-normalize");
    // Two-line journal, EUR entries on 2026-09-01: 1100 (Cash)
    // debit 100 EUR, 2100 (AP) credit 100 EUR.
    let journal = make_journal(&tmp, &[
        "e1\t1\t2026-09-01\tent1\tEUR\t1100\t\t100\t\t\t\th0\tseed",
        "e1\t2\t2026-09-01\tent1\tEUR\t\t2100\t100\t\t\t\th1\th0",
    ]);
    // Rate table: EUR->USD 1.10 on 2026-09-01.
    let rates = make_rates(&tmp, "date\tfrom\tto\trate\n2026-09-01\tEUR\tUSD\t1.10\n");
    let table = new_project::fx::read_table(&rates).expect("rates");
    let totals = new_project::reports::fold_journal_fx(&journal, &table)
        .expect("fold with fx");
    // 1100 EUR debit 100, USD-normalized 110 (= 100 * 1.10).
    let cash = totals.get("1100").expect("1100");
    let (eur, _credit, usd) = cash.get("EUR").copied().unwrap_or((0, 0, 0));
    assert_eq!(eur, 100, "1100 has 100 EUR debit");
    assert_eq!(usd, 110, "1100 has 110 USD normalized");
    // 2100 EUR credit 100, USD-normalized 110.
    let ap = totals.get("2100").expect("2100");
    let (debit, credit, usd) = ap.get("EUR").copied().unwrap_or((0, 0, 0));
    assert_eq!(credit, 100, "2100 has 100 EUR credit (debit 0)");
    assert_eq!(debit, 0, "2100 has 0 EUR debit");
    assert_eq!(usd, 110, "2100 has 110 USD normalized");
}

#[test]
fn fold_journal_fx_treats_usd_as_native() {
    let tmp = fresh_tempdir("fx-fold-usd");
    // USD entry: 1100 debit 100 USD, 2100 credit 100 USD.
    // The rates table has no USD->USD entry; the fold must
    // still produce the right usd_normalized (100 USD == 100 USD).
    let journal = make_journal(&tmp, &[
        "e1\t1\t2026-09-01\tent1\tUSD\t1100\t\t100\t\t\t\th0\tseed",
        "e1\t2\t2026-09-01\tent1\tUSD\t\t2100\t100\t\t\t\th1\th0",
    ]);
    let rates = make_rates(&tmp, "date\tfrom\tto\trate\n"); // empty rates
    let table = new_project::fx::read_table(&rates).expect("rates");
    let totals = new_project::reports::fold_journal_fx(&journal, &table)
        .expect("fold with fx");
    let cash = totals.get("1100").expect("1100");
    let (usd, _credit, usd_norm) = cash.get("USD").copied().unwrap_or((0, 0, 0));
    assert_eq!(usd, 100, "1100 has 100 USD debit");
    assert_eq!(usd_norm, 100, "1100 has 100 USD normalized (USD is native)");
}

#[test]
fn fold_journal_fx_skips_rows_with_no_rate() {
    // An entry on 2026-09-01 in JPY; the rates table has no
    // JPY entry. The row contributes 0 to USD (no error).
    let tmp = fresh_tempdir("fx-fold-no-rate");
    let journal = make_journal(&tmp, &[
        "e1\t1\t2026-09-01\tent1\tJPY\t1100\t\t100\t\t\t\th0\tseed",
    ]);
    let rates = make_rates(&tmp, "date\tfrom\tto\trate\n2026-09-01\tEUR\tUSD\t1.10\n");
    let table = new_project::fx::read_table(&rates).expect("rates");
    let totals = new_project::reports::fold_journal_fx(&journal, &table)
        .expect("fold with fx");
    // 1100 JPY: the JPY cell has the original (debit 100), but
    // the USD-normalized contribution is 0 (no rate).
    let cash = totals.get("1100").expect("1100");
    let (jpy, _credit, _jpy_usd) = cash.get("JPY").copied().unwrap_or((0, 0, 0));
    assert_eq!(jpy, 100, "JPY debit 100 preserved");
    // No USD entry for 1100.
    assert!(cash.get("USD").is_none(), "no USD entry: rate is missing");
}
