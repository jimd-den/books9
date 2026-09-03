//
// tests/stock_cache_reconcile.rs
//
//! Pin the FR-5 contract: the on-hand cache must match the
//! fold, or be rebuilt and warned about. The cache file lives
//! at the path given by --cache; absent cache means a fresh
//! reconcile (build and exit 0).

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
fn stock_writes_cache_when_no_cache_exists() {
    let tmp = fresh_tempdir("stock-no-cache");
    let journal = make_journal(&tmp, &[]);
    let cache = tmp.join("onhand.tsv");
    assert!(!cache.exists());
    let bin = env!("CARGO_BIN_EXE_stock");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .arg("--cache").arg(&cache)
        .output()
        .expect("run stock");
    assert!(output.status.success(), "stock exits 0 on first run: stderr={}", String::from_utf8_lossy(&output.stderr));
    assert!(cache.exists(), "cache must be written on first run");
    // The cache content must equal the stdout content.
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let cache_content = fs::read_to_string(&cache).expect("read cache");
    assert_eq!(stdout, cache_content, "cache and stdout must match");
}

#[test]
fn stock_exits_0_silently_when_cache_matches_fold() {
    let tmp = fresh_tempdir("stock-match");
    let journal = make_journal(&tmp, &[]);
    let cache = tmp.join("onhand.tsv");
    // Seed the cache with the same TSV stock would emit on an
    // empty journal: just the header.
    fs::write(&cache, "account\ton_hand\tcurrency\n").expect("seed cache");
    let bin = env!("CARGO_BIN_EXE_stock");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .arg("--cache").arg(&cache)
        .output()
        .expect("run stock");
    assert!(output.status.success(), "cache matches: exit 0");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.trim_end().is_empty(), "cache matches: no warning on stderr: {stderr}");
}

#[test]
fn stock_warns_on_stderr_and_rebuilds_when_cache_disagrees() {
    let tmp = fresh_tempdir("stock-mismatch");
    let journal = make_journal(&tmp, &[]);
    let cache = tmp.join("onhand.tsv");
    // Seed the cache with content that disagrees with the fold.
    fs::write(&cache, "account\ton_hand\tcurrency\n1100\t999\tUSD\n").expect("seed cache");
    let bin = env!("CARGO_BIN_EXE_stock");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .arg("--cache").arg(&cache)
        .output()
        .expect("run stock");
    assert!(output.status.success(), "mismatch is non-fatal: exit 0, rebuild, warn");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("rebuilt") || stderr.contains("disagrees") || stderr.contains("mismatch"),
        "stderr warns about the rebuild: {stderr}"
    );
    // The cache must have been rewritten to the fold's output.
    let cache_content = fs::read_to_string(&cache).expect("read rebuilt cache");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert_eq!(cache_content, stdout, "cache must be rewritten to the fold's output");
}
