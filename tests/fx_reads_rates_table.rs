//
// tests/fx_reads_rates_table.rs
//
//! Pin the contract for `fx::read_table` -- the reader that
//! turns a TSV rates file at /biz/fx/rates into a typed
//! `RatesTable` for O(log n) lookup by (from, date).
//!
//! Rates are quoted as decimal in the TSV (1.10 USD/EUR) and
//! stored as i64 in 10^-8 of `to` per unit of `from` (1.10 ->
//! 110_000_000). The SRD's "no floats cross a tool boundary"
//! rule holds at the in-memory boundary; the TSV is the
//! editor's surface.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_tempdir(label: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("books9-{label}-{pid}-{n}"));
    fs::create_dir_all(&dir).expect("create tempdir");
    dir
}

#[test]
fn fx_table_parses_a_simple_two_row_table() {
    let tmp = fresh_tempdir("fx-table");
    let path = tmp.join("rates.tsv");
    fs::write(
        &path,
        "date\tfrom\tto\trate\n2026-09-01\tEUR\tUSD\t1.10\n2026-09-15\tEUR\tUSD\t1.08\n",
    )
    .expect("write rates");
    let table = new_project::fx::read_table(&path)
        .expect("well-formed table must read");
    let r1 = table
        .lookup("EUR", "2026-09-01")
        .expect("rate on 2026-09-01");
    let r2 = table
        .lookup("EUR", "2026-09-15")
        .expect("rate on 2026-09-15");
    // 1.10 -> 110_000_000; 1.08 -> 108_000_000.
    assert_eq!(r1, 110_000_000, "1.10 rate");
    assert_eq!(r2, 108_000_000, "1.08 rate");
}

#[test]
fn fx_table_lookup_for_unknown_pair_returns_none() {
    let tmp = fresh_tempdir("fx-table-miss");
    let path = tmp.join("rates.tsv");
    fs::write(
        &path,
        "date\tfrom\tto\trate\n2026-09-01\tEUR\tUSD\t1.10\n",
    )
    .expect("write rates");
    let table = new_project::fx::read_table(&path).expect("read");
    // JPY has no rate on 2026-09-01.
    assert!(
        table.lookup("JPY", "2026-09-01").is_none(),
        "unknown pair: None, not a panic"
    );
    // EUR has a rate on 2026-09-01 but not on 2026-12-31.
    assert!(
        table.lookup("EUR", "2026-12-31").is_none(),
        "wrong date: None, not a panic"
    );
}

#[test]
fn fx_table_rejects_malformed_rate_value() {
    // The rate column is "abc" (not a number). The reader must
    // reject the whole file with a one-line reason.
    let tmp = fresh_tempdir("fx-table-bad");
    let path = tmp.join("rates.tsv");
    fs::write(
        &path,
        "date\tfrom\tto\trate\n2026-09-01\tEUR\tUSD\tabc\n",
    )
    .expect("write rates");
    let err = new_project::fx::read_table(&path)
        .expect_err("malformed rate value must reject");
    assert!(
        err.contains("rate") || err.contains("abc"),
        "reason names the offending column or value: {err}"
    );
}
