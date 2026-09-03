//
// tests/trial_driver.rs
//
//! Pin the contract for the `trial` driver: reads a journal and
//! emits a TSV on stdout with one row per (account, currency)
//! showing debit/credit totals.

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
fn trial_emits_one_row_per_account_per_currency() {
    let tmp = fresh_tempdir("trial-rows");
    let journal = make_journal(&tmp, &[
        "e1\t1\t2026-09-01\tent1\tUSD\t1100\t\t100\t\t\t\th0\tseed",
        "e1\t2\t2026-09-01\tent1\tUSD\t\t2100\t100\t\t\t\th1\th0",
    ]);
    let bin = env!("CARGO_BIN_EXE_trial");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .output()
        .expect("run trial");
    assert!(output.status.success(), "trial exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    // Header + 2 data rows (one per account, single currency).
    assert!(lines.len() >= 3, "expected header + >= 2 rows; got {}: {stdout}", lines.len());
    // Header: account\tcurrency\tdebit\tcredit
    assert!(lines[0].contains("account"), "header has account: {}", lines[0]);
    assert!(lines[0].contains("currency"), "header has currency: {}", lines[0]);
    assert!(lines[0].contains("debit"), "header has debit: {}", lines[0]);
    assert!(lines[0].contains("credit"), "header has credit: {}", lines[0]);
    // Find the 1100 USD row
    let cash = lines.iter().find(|l| l.starts_with("1100\t")).expect("1100 row");
    assert!(cash.contains("USD"), "cash is USD: {cash}");
    let cash_cols: Vec<&str> = cash.split('\t').collect();
    assert_eq!(cash_cols[2], "100");
    assert_eq!(cash_cols[3], "0");
    // Find the 2100 USD row
    let ap = lines.iter().find(|l| l.starts_with("2100\t")).expect("2100 row");
    let ap_cols: Vec<&str> = ap.split('\t').collect();
    assert_eq!(ap_cols[2], "0");
    assert_eq!(ap_cols[3], "100");
}

#[test]
fn trial_on_empty_journal_emits_only_header() {
    let tmp = fresh_tempdir("trial-empty");
    let journal = make_journal(&tmp, &[]);
    let bin = env!("CARGO_BIN_EXE_trial");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .output()
        .expect("run trial");
    assert!(output.status.success(), "trial exits 0");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    // Just the header.
    assert_eq!(lines.len(), 1, "empty journal: header only; got: {stdout}");
    assert!(lines[0].contains("account"), "header has account: {}", lines[0]);
}

#[test]
fn trial_stderr_is_clean_for_piping() {
    // SRD Unix constitution: "all tools log to stderr only; stdout
    // stays clean for piping." On a successful run, stderr must be
    // empty (or near-empty -- only a literal newline is allowed).
    let tmp = fresh_tempdir("trial-stderr");
    let journal = make_journal(&tmp, &[]);
    let bin = env!("CARGO_BIN_EXE_trial");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .output()
        .expect("run trial");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.trim().is_empty(),
        "successful trial must have empty stderr: {stderr}"
    );
}
