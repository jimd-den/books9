//
// tests/inspect_sample.rs
//
//! Pin the contract for the `inspect sample` subcommand.

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
fn inspect_sample_records_a_sub_sample() {
    let tmp = fresh_tempdir("inspect-sample");
    let bin = env!("CARGO_BIN_EXE_inspect");
    let output = Command::new(bin)
        .arg("sample")
        .arg("--root").arg(&tmp)
        .arg("--lot-id").arg("lot:1")
        .arg("--qty").arg("5")
        .arg("--verdict").arg("pass")
        .output()
        .expect("run inspect sample");
    assert!(output.status.success(), "inspect sample exits 0: stderr={}",
        String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "header + 1 row: {stdout}");
    assert!(lines[0].contains("lot_id") || lines[0].contains("verdict"));
    assert!(stdout.contains("lot:1"));
    assert!(stdout.contains("pass"));
}
