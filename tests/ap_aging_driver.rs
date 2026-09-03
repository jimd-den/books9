//
// tests/ap_aging_driver.rs
//
//! Pin the contract for the `ap` driver: aging buckets for
//! outstanding accounts payable (mirror of ar_aging).

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
fn ap_aging_buckets_outstanding_ap_credits() {
    // 2100 (AP) credit 2026-09-20 (10 days old).
    let tmp = fresh_tempdir("ap-aging");
    let journal = make_journal(&tmp, &[
        "e1\t1\t2026-09-20\tent1\tUSD\t1100\t\t1000\tcust:1\tinv:1\t\t\tseed",
        "e1\t2\t2026-09-20\tent1\tUSD\t\t2100\t1000\tcust:1\tinv:1\t\t\th0",
    ]);
    let bin = env!("CARGO_BIN_EXE_ap");
    let out = Command::new(bin)
        .arg("aging")
        .arg("--journal").arg(&journal)
        .arg("--as-of").arg("2026-09-30")
        .output()
        .expect("run ap aging");
    assert!(out.status.success(), "ap aging exits 0: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(stdout.contains("0-30\t1000"), "0-30 bucket has 1000: {stdout}");
}
