//
// tests/balance_fx.rs
//
//! Pin the contract for `balance --fx PATH`: the per-account
//! balance output gains a `usd_normalized` column when an FX
//! rates table is given. Without --fx, the output is unchanged
//! from Phase 3.

use std::process::Command;
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
fn balance_with_fx_adds_a_usd_normalized_column() {
    let tmp = fresh_tempdir("balance-fx-col");
    let journal = make_journal(&tmp, &[
        "e1\t1\t2026-09-01\tent1\tEUR\t1100\t\t100\t\t\t\th0\tseed",
        "e1\t2\t2026-09-01\tent1\tEUR\t\t2100\t100\t\t\t\th1\th0",
    ]);
    let rates = make_rates(&tmp, "date\tfrom\tto\trate\n2026-09-01\tEUR\tUSD\t1.10\n");
    let bin = env!("CARGO_BIN_EXE_balance");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .arg("--account").arg("1100")
        .arg("--fx").arg(&rates)
        .output()
        .expect("run balance");
    assert!(output.status.success(), "balance --fx exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "header + 1 row: {stdout}");
    assert!(lines[0].contains("usd_normalized"), "header has usd_normalized: {}", lines[0]);
    let cols: Vec<&str> = lines[1].split('\t').collect();
    assert_eq!(cols.len(), 5, "row has 5 cols");
    assert_eq!(cols[4], "110", "usd_normalized for 100 EUR * 1.10 = 110");
}

#[test]
fn balance_without_fx_is_unchanged_from_phase_3() {
    let tmp = fresh_tempdir("balance-fx-none");
    let journal = make_journal(&tmp, &[
        "e1\t1\t2026-09-01\tent1\tUSD\t1100\t\t100\t\t\t\th0\tseed",
    ]);
    let bin = env!("CARGO_BIN_EXE_balance");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .arg("--account").arg("1100")
        .output()
        .expect("run balance");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    let cols: Vec<&str> = lines[0].split('\t').collect();
    assert_eq!(cols.len(), 4, "without --fx: 4-column header: {}", lines[0]);
}
