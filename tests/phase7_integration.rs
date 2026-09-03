//
// tests/phase7_integration.rs
//
//! End-to-end integration test for the Phase 7 surface:
//! asset, depreciate, maint, inspect. Every tool is a
//! separate process; the only state they share is the
//! filesystem.

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

fn run(bin: &str, args: &[&str]) -> std::process::Output {
    let path = format!("target/debug/{bin}");
    Command::new(&path)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {bin}: {e}"))
}

#[test]
fn phase7_end_to_end() {
    let tmp = fresh_tempdir("phase7-e2e");

    // 1. Register a forklift.
    let out = run("asset", &["new",
        "--root", tmp.to_str().unwrap(),
        "--id", "ast:1",
        "--name", "Forklift",
        "--cost", "5000000",
        "--acquired", "2024-01-15",
        "--life", "60",
        "--salvage", "500000",
    ]);
    assert!(out.status.success(), "asset new: stderr={}", String::from_utf8_lossy(&out.stderr));

    // 2. Depreciate for September 2026: 75000.
    let out = run("depreciate", &[
        "--root", tmp.to_str().unwrap(),
        "--asset", "ast:1",
        "--period", "2026-09",
    ]);
    assert!(out.status.success(), "depreciate: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(stdout.trim(), "75000", "monthly depreciation: {stdout}");

    // 3. Book a maintenance order for 200.
    let out = run("maint", &["new",
        "--root", tmp.to_str().unwrap(),
        "--maint-id", "m:1",
        "--asset", "ast:1",
        "--date", "2026-09-15",
        "--amount", "200",
    ]);
    assert!(out.status.success(), "maint: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 3, "header + 2 legs: {stdout}");
    // Balance check.
    let total_dr: i64 = lines[1..].iter()
        .filter_map(|l| {
            let cols: Vec<&str> = l.split('\t').collect();
            if !cols[5].is_empty() { cols[7].parse::<i64>().ok() } else { None }
        }).sum();
    let total_cr: i64 = lines[1..].iter()
        .filter_map(|l| {
            let cols: Vec<&str> = l.split('\t').collect();
            if !cols[6].is_empty() { cols[7].parse::<i64>().ok() } else { None }
        }).sum();
    assert_eq!(total_dr, total_cr, "maint journal lines balance");
    assert_eq!(total_dr, 200);

    // 4. Record an inspection lot.
    let out = run("inspect", &["new",
        "--root", tmp.to_str().unwrap(),
        "--lot-id", "lot:1",
        "--sku", "sku:77",
        "--qty", "40",
        "--verdict", "pass",
        "--inspector", "alice",
        "--date", "2026-09-15",
    ]);
    assert!(out.status.success(), "inspect: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(stdout.contains("lot:1\tsku:77\t40\tpass\talice\t2026-09-15"));
}
