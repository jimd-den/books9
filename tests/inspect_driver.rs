//
// tests/inspect_driver.rs
//
//! Pin the contract for the `inspect` driver: record an
//! inspection lot and print the record to stdout.

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
fn inspect_new_prints_a_lot_record() {
    let tmp = fresh_tempdir("inspect-new");
    let bin = env!("CARGO_BIN_EXE_inspect");
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--lot-id").arg("lot:1")
        .arg("--sku").arg("sku:77")
        .arg("--qty").arg("40")
        .arg("--verdict").arg("pass")
        .arg("--inspector").arg("alice")
        .arg("--date").arg("2026-09-15")
        .output()
        .expect("run inspect new");
    assert!(output.status.success(), "inspect new exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "header + 1 row: {stdout}");
    assert!(stdout.contains("lot:1\tsku:77\t40\tpass\talice\t2026-09-15"));
}
