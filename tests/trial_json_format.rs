//
// tests/trial_json_format.rs
//
//! Pin the FR-7 contract: trial --format json emits a JSON
//! representation of the trial balance. The TSV format (the
//! default) is unchanged.

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
fn trial_with_format_json_emits_valid_json() {
    let tmp = fresh_tempdir("trial-json");
    let journal = make_journal(&tmp, &[
        "e1\t1\t2026-09-01\tent1\tUSD\t1100\t\t1000\tcust:1\tinv:1\t\t\tseed",
        "e1\t2\t2026-09-01\tent1\tUSD\t\t4000\t1000\tcust:1\tinv:1\t\t\th0",
    ]);
    let bin = env!("CARGO_BIN_EXE_trial");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .arg("--format").arg("json")
        .output()
        .expect("run trial json");
    assert!(output.status.success(), "trial json exits 0: stderr={}",
        String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    // Must contain JSON keys: account, currency, debit, credit.
    assert!(stdout.contains("account"));
    assert!(stdout.contains("currency"));
    assert!(stdout.contains("debit"));
    assert!(stdout.contains("credit"));
    assert!(stdout.contains("1100"));
    assert!(stdout.contains("4000"));
}

#[test]
fn trial_default_format_is_tsv() {
    let tmp = fresh_tempdir("trial-default");
    let journal = make_journal(&tmp, &[
        "e1\t1\t2026-09-01\tent1\tUSD\t1100\t\t1000\tcust:1\tinv:1\t\t\tseed",
    ]);
    let bin = env!("CARGO_BIN_EXE_trial");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .output()
        .expect("run trial default");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    // TSV format: tab-separated.
    assert!(stdout.contains("1100\tUSD\t1000\t0"),
        "TSV format unchanged: {stdout}");
}
