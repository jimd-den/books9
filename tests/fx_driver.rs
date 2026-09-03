//
// tests/fx_driver.rs
//
//! Pin the contract for the `fx` driver: one verb (list).
//! Reads the rates table at --path PATH and emits it to stdout
//! as TSV. Read-only; no I/O beyond the input file.

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
fn fx_emits_the_table_on_stdout() {
    let tmp = fresh_tempdir("fx-driver-list");
    let path = tmp.join("rates.tsv");
    fs::write(
        &path,
        "date\tfrom\tto\trate\n2026-09-01\tEUR\tUSD\t1.10\n2026-09-15\tEUR\tUSD\t1.08\n",
    ).expect("write rates");
    let bin = env!("CARGO_BIN_EXE_fx");
    let output = Command::new(bin)
        .arg("--path").arg(&path)
        .output()
        .expect("run fx");
    assert!(output.status.success(), "fx exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    // Header + 2 data rows.
    assert_eq!(lines.len(), 3, "header + 2 rows: {stdout}");
    assert_eq!(lines[0], "date\tfrom\tto\trate");
    assert!(lines[1].contains("2026-09-01") && lines[1].contains("EUR") && lines[1].contains("USD"));
    assert!(lines[2].contains("2026-09-15"));
}

#[test]
fn fx_stderr_is_clean_for_piping_on_success() {
    let tmp = fresh_tempdir("fx-driver-stderr");
    let path = tmp.join("rates.tsv");
    fs::write(&path, "date\tfrom\tto\trate\n").expect("write empty rates");
    let bin = env!("CARGO_BIN_EXE_fx");
    let output = Command::new(bin)
        .arg("--path").arg(&path)
        .output()
        .expect("run fx");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.trim_end().is_empty(),
        "successful fx must have empty stderr: {stderr}");
}

#[test]
fn fx_rejects_missing_path() {
    // No --path: exit 2, one-line stderr.
    let bin = env!("CARGO_BIN_EXE_fx");
    let output = Command::new(bin)
        .output()
        .expect("run fx");
    assert_eq!(output.status.code(), Some(2), "missing --path is exit 2");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--path"), "stderr names the missing flag: {stderr}");
}
