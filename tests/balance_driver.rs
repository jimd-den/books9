//
// tests/balance_driver.rs
//
//! Pin the contract for the `balance` driver: read a journal,
//! filter to a single account, and emit per-currency debit/credit
//! totals on stdout.

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

#[test]
fn balance_filters_to_one_account() {
    let tmp = fresh_tempdir("balance-one");
    let journal = make_journal(&tmp, &[
        // 1100 (Cash) debit 100 USD
        "e1\t1\t2026-09-01\tent1\tUSD\t1100\t\t100\t\t\t\th0\tseed",
        // 2100 (AP) credit 100 USD
        "e1\t2\t2026-09-01\tent1\tUSD\t\t2100\t100\t\t\t\th1\th0",
    ]);
    let bin = env!("CARGO_BIN_EXE_balance");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .arg("--account").arg("1100")
        .output()
        .expect("run balance");
    assert!(output.status.success(), "balance exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    // Header + one row: 1100 USD debit 100 credit 0.
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "header + 1 row: {stdout}");
    let cols: Vec<&str> = lines[1].split('\t').collect();
    assert_eq!(cols[0], "1100");
    assert_eq!(cols[1], "USD");
    assert_eq!(cols[2], "100");
    assert_eq!(cols[3], "0");
}

#[test]
fn balance_for_unknown_account_emits_just_the_header() {
    let tmp = fresh_tempdir("balance-unknown");
    let journal = make_journal(&tmp, &[
        "e1\t1\t2026-09-01\tent1\tUSD\t1100\t\t100\t\t\t\th0\tseed",
    ]);
    let bin = env!("CARGO_BIN_EXE_balance");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .arg("--account").arg("9999")
        .output()
        .expect("run balance");
    assert!(output.status.success(), "balance exits 0 even for unknown account");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "unknown account: header only: {stdout}");
}

#[test]
fn balance_rejects_missing_account() {
    // No --account flag: rejected, nonzero exit, one-line stderr.
    let tmp = fresh_tempdir("balance-no-acct");
    let journal = make_journal(&tmp, &[]);
    let bin = env!("CARGO_BIN_EXE_balance");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .output()
        .expect("run balance");
    assert_eq!(output.status.code(), Some(2), "missing --account is exit 2");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--account"), "stderr names the missing flag: {stderr}");
    assert!(!stderr.trim_end().contains('\n'), "stderr is one logical line: {stderr}");
}
