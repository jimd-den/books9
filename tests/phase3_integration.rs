//
// tests/phase3_integration.rs
//
//! End-to-end integration test for the Phase 3 surface.
//!
//! The loop: create three accounts with `coa new`, post a balanced
//! entry with `post --coa`, then read the books with `trial`,
//! `balance`, and `stock`. Every tool is a separate process;
//! the only state they share is the filesystem (the CoA tree,
//! the journal, and the stock cache). If any step fails, the
//! next step sees the failure and the test fails with a clear
//! reason. This is the acceptance test for Phase 3.

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

fn make_data_row(entry_id: &str, seq: &str, date: &str, debit: &str, credit: &str, amount: &str, h: &str, prev: &str) -> String {
    format!(
        "{entry_id}\t{seq}\t{date}\tent1\tUSD\t{debit}\t{credit}\t{amount}\t\t\t\t{h}\t{prev}"
    )
}

#[test]
fn phase3_end_to_end() {
    let tmp = fresh_tempdir("phase3-e2e");
    let coa_root = tmp.join("accounts");
    let journal = tmp.join("journal.tsv");
    let stock_cache = tmp.join("onhand.tsv");

    // 1. Create three accounts with `coa new`.
    for (acct, name, kind, side) in [
        ("1100", "Cash", "asset", "debit"),
        ("2100", "AP", "liability", "credit"),
        ("4000", "Sales", "revenue", "credit"),
    ] {
        let out = run("coa", &["new",
            "--root", coa_root.to_str().unwrap(),
            acct,
            "--name", name,
            "--kind", kind,
            "--normal-side", side,
        ]);
        assert!(out.status.success(), "coa new {acct}: stderr={}", String::from_utf8_lossy(&out.stderr));
    }

    // 2. Create a fresh journal.
    let out = run("post", &["--journal", journal.to_str().unwrap(), "--check"]);
    // Empty input -> reject (Phase 0 behavior).
    assert_eq!(out.status.code(), Some(2));
    // Create via the post helper -- there's no `post init`; we
    // bootstrap by writing a header-only journal file.
    fs::write(&journal, "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash\n").unwrap();

    // 3. Post a balanced entry that uses all three accounts.
    let header = "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash";
    let row1 = make_data_row("e1", "1", "2026-09-01", "1100", "", "100", "h0", "0000000000000000");
    let row2 = make_data_row("e1", "2", "2026-09-01", "", "2100", "100", "h1", "h0");
    let proposed = format!("{header}\n{row1}\n{row2}\n");
    // Use stdin
    use std::io::Write;
    let mut child = Command::new("target/debug/post")
        .args(&["--journal", journal.to_str().unwrap(), "--coa", coa_root.to_str().unwrap()])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(proposed.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "post live append: stderr={}", String::from_utf8_lossy(&out.stderr));

    // 4. Trial balance: 1100 debit 100, 2100 credit 100.
    let out = run("trial", &["--journal", journal.to_str().unwrap()]);
    assert!(out.status.success(), "trial: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("1100") && stdout.contains("100"), "trial has 1100/100: {stdout}");
    assert!(stdout.contains("2100") && stdout.contains("100"), "trial has 2100/100: {stdout}");

    // 5. Balance for 1100: debit 100, credit 0.
    let out = run("balance", &[
        "--journal", journal.to_str().unwrap(),
        "--account", "1100",
    ]);
    assert!(out.status.success(), "balance: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 2, "balance has header + row: {stdout}");
    let cols: Vec<&str> = lines[1].split('\t').collect();
    assert_eq!(cols[0], "1100");
    assert_eq!(cols[2], "100");
    assert_eq!(cols[3], "0");

    // 6. Stock on the populated journal: cache reconcile writes the cache.
    let out = run("stock", &[
        "--journal", journal.to_str().unwrap(),
        "--cache", stock_cache.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "stock: stderr={}", String::from_utf8_lossy(&out.stderr));
    assert!(stock_cache.exists(), "stock wrote the cache");

    // 7. Re-run stock with the same cache: silent happy path.
    let out = run("stock", &[
        "--journal", journal.to_str().unwrap(),
        "--cache", stock_cache.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "stock (cached): exit 0");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.trim_end().is_empty(), "stock (cached) is silent: {stderr}");
}

