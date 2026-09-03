//
// tests/maint_driver.rs
//
//! Pin the contract for the `maint` driver: emit a balanced
//! maintenance journal proposal (DR Maint Exp / CR Cash).

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

#[test]
fn maint_new_emits_a_balanced_journal_proposal() {
    let tmp = fresh_tempdir("maint-new");
    let bin = env!("CARGO_BIN_EXE_maint");
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--maint-id").arg("m:1")
        .arg("--asset").arg("ast:1")
        .arg("--date").arg("2026-09-15")
        .arg("--amount").arg("200")
        .output()
        .expect("run maint new");
    assert!(output.status.success(), "maint new exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 3, "header + 2 legs: {stdout}");
    // DR Maintenance Expense 200, CR Cash 200.
    assert!(stdout.contains("\t6400\t\t200\t"));
    assert!(stdout.contains("\t\t1000\t200\t"));
}
