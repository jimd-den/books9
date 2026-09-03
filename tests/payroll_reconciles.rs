//
// tests/payroll_reconciles.rs
//
//! Pin the FR-4 contract: gross - deductions = net, and the
//! journal lines balance. The math is in `payroll::compute`;
//! the driver emits a balanced journal proposal.

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

fn seed_org(root: &PathBuf, code: &str) {
    let dir = root.join(code);
    fs::create_dir_all(&dir).expect("create dept");
    fs::write(dir.join("profile.tsv"),
        "code\tname\tparent\tcost_center\n{code}\tDept\t\t{code}\n").expect("write");
}

#[test]
fn payroll_gross_minus_deductions_equals_net() {
    let tmp = fresh_tempdir("payroll-reconcile");
    // One employee: 40 hours at 25 = 1000 gross; 200 deductions;
    // net = 800.
    let hours = tmp.join("hours.tsv");
    fs::write(&hours, "employee\thours\trate\tcost_center\nemp:1\t40\t25\td:1\n").expect("write");
    let deductions = tmp.join("deductions.tsv");
    fs::write(&deductions, "employee\tdeduction\nemp:1\t200\n").expect("write");
    // Compute directly via the library fn to test the
    // reconciliation without the journal noise.
    let hrs = new_project::payroll::read_hours(&hours).expect("hours");
    let mut deds = std::collections::HashMap::new();
    deds.insert("emp:1".to_string(), 200);
    let lines = new_project::payroll::compute(&hrs, &deds).expect("compute");
    assert_eq!(lines.len(), 1);
    let l = &lines[0];
    assert_eq!(l.gross, 1000, "40 * 25 = 1000");
    assert_eq!(l.deductions, 200);
    assert_eq!(l.net, 800, "1000 - 200 = 800");
    // FR-4 reconciliation: gross = net + deductions.
    assert_eq!(l.gross, l.net + l.deductions, "FR-4 reconciliation");
}

#[test]
fn payroll_driver_emits_a_balanced_journal_proposal() {
    let tmp = fresh_tempdir("payroll-driver");
    seed_org(&tmp, "d:1");
    let hours = tmp.join("hours.tsv");
    fs::write(&hours, "employee\thours\trate\tcost_center\nemp:1\t40\t25\td:1\n").expect("write");
    let deductions = tmp.join("deductions.tsv");
    fs::write(&deductions, "employee\tdeduction\nemp:1\t200\n").expect("write");
    let bin = env!("CARGO_BIN_EXE_payroll");
    let output = Command::new(bin)
        .arg("--hours").arg(&hours)
        .arg("--deductions").arg(&deductions)
        .output()
        .expect("run payroll");
    assert!(output.status.success(), "payroll exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    // Header + 3 legs per employee.
    assert_eq!(lines.len(), 4, "header + 3 legs: {stdout}");
    // 1000 (DR Wages), 800 (CR Cash), 200 (CR Wages Payable).
    assert!(stdout.contains("\t6000\t\t1000\t"), "DR Wages 1000");
    assert!(stdout.contains("\t\t1000\t800\t"), "CR Cash 800");
    assert!(stdout.contains("\t\t2100\t200\t"), "CR Wages Payable 200");
    // Balance: 1000 = 800 + 200.
    let total_dr: i64 = lines[1..].iter()
        .filter_map(|l| {
            let cols: Vec<&str> = l.split('\t').collect();
            if !cols[5].is_empty() {
                cols[7].parse::<i64>().ok()
            } else { None }
        }).sum();
    let total_cr: i64 = lines[1..].iter()
        .filter_map(|l| {
            let cols: Vec<&str> = l.split('\t').collect();
            if !cols[6].is_empty() {
                cols[7].parse::<i64>().ok()
            } else { None }
        }).sum();
    assert_eq!(total_dr, total_cr, "journal lines balance: {lines:?}");
}
