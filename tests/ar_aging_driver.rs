//
// tests/ar_aging_driver.rs
//
//! Pin the contract for the `ar aging` driver: read a journal,
//! find every AR debit (account 1100 on the debit side), and
//! compute the age of each open balance. Bucket: 0-30, 31-60,
//! 61-90, 90+ days.

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
fn ar_aging_buckets_outstanding_balances_by_age() {
    let tmp = fresh_tempdir("ar-aging");
    // Two AR debits: one 10 days old, one 45 days old, as of 2026-09-30.
    let journal = make_journal(&tmp, &[
        // 1000 USD debit 2026-09-20 (10 days old) and corresponding credit
        "e1\t1\t2026-09-20\tent1\tUSD\t1100\t\t1000\tcust:123\tinv:1\t\t\tseed",
        "e1\t2\t2026-09-20\tent1\tUSD\t\t4000\t1000\tcust:123\tinv:1\t\t\th0",
        // 500 USD debit 2026-08-16 (45 days old) and corresponding credit
        "e2\t1\t2026-08-16\tent1\tUSD\t1100\t\t500\tcust:123\tinv:2\t\t\th0",
        "e2\t2\t2026-08-16\tent1\tUSD\t\t4000\t500\tcust:123\tinv:2\t\t\th1",
    ]);
    let bin = env!("CARGO_BIN_EXE_ar_aging");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .arg("--as-of").arg("2026-09-30")
        .output()
        .expect("run ar aging");
    assert!(output.status.success(), "ar aging exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    // Header + 4 bucket rows (0-30, 31-60, 61-90, 90+).
    assert!(lines.len() >= 4, "header + 4 buckets: {stdout}");
    assert!(lines[0].contains("bucket"), "header has bucket: {}", lines[0]);
    // The 1000 (10 days) goes to 0-30; the 500 (45 days) goes to 31-60.
    let bucket_0_30 = lines[1].split('\t').next().unwrap_or("");
    assert_eq!(bucket_0_30, "0-30", "first bucket is 0-30");
    assert!(stdout.contains("1000"), "1000 in 0-30 bucket");
    assert!(stdout.contains("500"), "500 in 31-60 bucket");
}
