//
// tests/invoice_driver.rs
//
//! Pin the contract for the `invoice` driver: read a priced
//! SO and emit a balanced journal-entry proposal on stdout
//! that `post` can consume directly.

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

fn seed_invoice_inputs(so_root: &PathBuf, coa_root: &PathBuf, so_id: &str) {
    // Seed the CoA: 1100 (Cash/AR), 4000 (Sales).
    for (acct, kind) in [("1100", "asset"), ("4000", "revenue")] {
        let dir = coa_root.join(acct);
        fs::create_dir_all(&dir).expect("create account dir");
        let body = format!("code\tname\tkind\n{acct}\t{acct}\t{kind}\n");
        fs::write(dir.join("profile.tsv"), body).expect("write profile");
    }
    // Seed a priced SO.
    let so_dir = so_root.join("docs").join("so");
    fs::create_dir_all(&so_dir).expect("create so dir");
    let priced_body = format!(
        "so_id\tparty\tdate\tcurrency\tterms\tsku\tqty\tunit_price_minor\tline_total_minor\n         {so_id}\tcust:123\t2026-09-01\tUSD\tNet-30\tsku:77\t40\t100\t4000\n         {so_id}\tcust:123\t2026-09-01\tUSD\tNet-30\tsku:88\t10\t250\t2500\n"
    );
    fs::write(so_dir.join(format!("{so_id}.priced.tsv")), priced_body).expect("write priced");
}

#[test]
fn invoice_emits_a_balanced_journal_proposal() {
    let tmp = fresh_tempdir("invoice-balanced");
    let coa = tmp.join("coa");
    let so_root = tmp.join("biz");
    seed_invoice_inputs(&so_root, &coa, "000421");
    // We also need a journal file (the proposal is post-compatible).
    let journal = tmp.join("journal.tsv");
    let bin = env!("CARGO_BIN_EXE_invoice");
    let output = Command::new(bin)
        .arg("--root").arg(&so_root)
        .arg("--so").arg("000421")
        .arg("--coa-root").arg(&coa)
        .arg("--journal").arg(&journal)
        .output()
        .expect("run invoice");
    assert!(output.status.success(), "invoice exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    // Header + 2 data rows (one debit, one credit).
    assert_eq!(lines.len(), 3, "header + 2 rows: {stdout}");
    // 1100 (AR) debit = 6500 (4000 + 2500).
    let r1: Vec<&str> = lines[1].split('\t').collect();
    assert_eq!(r1[5], "1100", "debit account");
    assert_eq!(r1[6], "", "no credit");
    assert_eq!(r1[7], "6500", "debit amount = 4000 + 2500");
    // 4000 (Sales) credit = 6500.
    let r2: Vec<&str> = lines[2].split('\t').collect();
    assert_eq!(r2[6], "4000", "credit account");
    assert_eq!(r2[5], "", "no debit");
    assert_eq!(r2[7], "6500", "credit amount = 4000 + 2500");
}
