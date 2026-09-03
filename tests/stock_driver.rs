//
// tests/stock_driver.rs
//
//! Pin the contract for the `stock` driver: on-hand view, derived
//! from a journal fold. Phase 3 ships the SCAFFOLD -- the fold
//! shape, the cache-vs-recompute check, and the empty-journal
//! behavior. Real inventory postings arrive in Phase 5 (O2C/P2P).

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
fn stock_on_empty_journal_emits_only_header() {
    let tmp = fresh_tempdir("stock-empty");
    let journal = make_journal(&tmp, &[]);
    let bin = env!("CARGO_BIN_EXE_stock");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .output()
        .expect("run stock");
    assert!(output.status.success(), "stock exits 0 on empty journal");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "empty journal: header only: {stdout}");
    // Header: account\ton_hand\tcurrency
    assert!(lines[0].contains("account"), "header has account: {}", lines[0]);
    assert!(lines[0].contains("on_hand"), "header has on_hand: {}", lines[0]);
    assert!(lines[0].contains("currency"), "header has currency: {}", lines[0]);
}

#[test]
fn stock_stderr_is_clean_for_piping_on_success() {
    let tmp = fresh_tempdir("stock-stderr");
    let journal = make_journal(&tmp, &[]);
    let bin = env!("CARGO_BIN_EXE_stock");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .output()
        .expect("run stock");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.trim_end().is_empty(),
        "successful stock must have empty stderr: {stderr}"
    );
}

#[test]
fn stock_rejects_missing_journal() {
    // No --journal flag: rejected, exit 2, one-line stderr.
    let bin = env!("CARGO_BIN_EXE_stock");
    let output = Command::new(bin)
        .output()
        .expect("run stock");
    assert_eq!(output.status.code(), Some(2), "missing --journal is exit 2");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--journal"), "stderr names the missing flag: {stderr}");
}
