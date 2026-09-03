//
// tests/reports_fold_journal.rs
//
//! Pin the contract for `reports::fold_journal` -- the use-case
//! primitive that turns a journal into per-account per-currency
//! debit/credit totals.
//!
//! The fold is the heart of `trial`, `balance`, and `stock`. It
//! is pure: same journal, same totals. No I/O, no clocks. One
//! walk, one map, one return.

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

/// Build a minimal journal file with the given data rows.
/// Header is the SRD contract; each data row is a 13-column TSV
/// with the prev_hash column already filled in (a hash-chained
/// row is what post would write; the fold does not care about
/// hashes, it only reads cols 5/6 (account_debit/account_credit),
/// 4 (currency), 7 (amount_minor)).
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

#[test]
fn fold_journal_returns_per_account_per_currency_totals() {
    let tmp = fresh_tempdir("fold-totals");
    // Two entries, single currency, balanced: 1100 (Cash) debit 100,
    // 2100 (AP) credit 100.
    let path = make_journal(&tmp, &[
        "e1\t1\t2026-09-01\tent1\tUSD\t1100\t\t100\t\t\t\th0\tseed",
        "e1\t2\t2026-09-01\tent1\tUSD\t\t2100\t100\t\t\t\th1\th0",
    ]);
    let totals = new_project::reports::fold_journal(&path, &|_line| true)
        .expect("well-formed journal must fold");
    // One account per row: 1100 should be debit 100 USD,
    // 2100 should be credit 100 USD.
    assert_eq!(totals.len(), 2, "two accounts: {totals:?}");
    let cash = totals.get("1100").expect("1100 must appear");
    assert_eq!(cash.get("USD").copied(), Some((100, 0)),
        "1100 is debit 100 USD, credit 0: got {cash:?}");
    let ap = totals.get("2100").expect("2100 must appear");
    assert_eq!(ap.get("USD").copied(), Some((0, 100)),
        "2100 is credit 100 USD, debit 0: got {ap:?}");
}

#[test]
fn fold_journal_groups_multi_currency_rows_by_account_then_currency() {
    let tmp = fresh_tempdir("fold-multi-currency");
    // Same account (1100) appears in two currencies; the fold
    // groups by account first, then by currency.
    let path = make_journal(&tmp, &[
        "e1\t1\t2026-09-01\tent1\tUSD\t1100\t\t100\t\t\t\th0\tseed",
        "e1\t2\t2026-09-01\tent1\tUSD\t\t2100\t100\t\t\t\th1\th0",
        "e2\t1\t2026-09-02\tent1\tEUR\t1100\t\t200\t\t\t\th2\th1",
        "e2\t2\t2026-09-02\tent1\tEUR\t\t2100\t200\t\t\t\th3\th2",
    ]);
    let totals = new_project::reports::fold_journal(&path, &|_line| true)
        .expect("fold");
    let cash = totals.get("1100").expect("1100 must appear");
    assert_eq!(cash.get("USD").copied(), Some((100, 0)));
    assert_eq!(cash.get("EUR").copied(), Some((200, 0)));
    let ap = totals.get("2100").expect("2100 must appear");
    assert_eq!(ap.get("USD").copied(), Some((0, 100)));
    assert_eq!(ap.get("EUR").copied(), Some((0, 200)));
}

#[test]
fn fold_journal_respects_a_filter_predicate() {
    // A filter that rejects everything returns an empty map.
    let tmp = fresh_tempdir("fold-filter");
    let path = make_journal(&tmp, &[
        "e1\t1\t2026-09-01\tent1\tUSD\t1100\t\t100\t\t\t\th0\tseed",
    ]);
    let totals = new_project::reports::fold_journal(&path, &|_line| false)
        .expect("fold");
    assert!(totals.is_empty(), "filter that rejects all yields empty");
}

#[test]
fn fold_journal_on_empty_journal_returns_empty_map() {
    let tmp = fresh_tempdir("fold-empty");
    let path = make_journal(&tmp, &[]);
    let totals = new_project::reports::fold_journal(&path, &|_line| true)
        .expect("fold on empty journal");
    assert!(totals.is_empty());
}
