//
// tests/ship_driver.rs
//
//! Pin the contract for the `ship` driver: emit a journal
//! proposal (DR COGS / CR Inventory) at the shipped qty.

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
fn ship_new_emits_a_balanced_journal_proposal() {
    let tmp = fresh_tempdir("ship-new");
    let bin = env!("CARGO_BIN_EXE_ship");
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--so-id").arg("000421")
        .arg("--date").arg("2026-09-15")
        .arg("--qty").arg("10")
        .arg("--cogs-per-unit").arg("100")
        .output()
        .expect("run ship new");
    assert!(output.status.success(), "ship new exits 0: stderr={}",
        String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    // Header + 2 legs (DR COGS / CR Inventory).
    assert_eq!(lines.len(), 3, "header + 2 legs: {stdout}");
    // COGS = 10 * 100 = 1000.
    assert!(stdout.contains("	5000		1000	"), "DR COGS 1000");
    assert!(stdout.contains("	1300	1000	"), "CR Inventory 1000");
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
    assert_eq!(total_dr, total_cr, "ship journal lines balance");
}
