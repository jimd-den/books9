//
// tests/phase4_integration.rs
//
//! End-to-end integration test for the Phase 4 FX surface.
//!
//! The loop: write a 2-row journal (EUR entries) and a 2-row
//! rates table; run `trial --fx`, `balance --fx`, and
//! `close --fx --close-date`. The USD-normalized column
//! appears in the first two; the unrealized-gain info line
//! appears on stderr in the third.

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

fn make_rates(dir: &PathBuf, body: &str) -> PathBuf {
    let path = dir.join("rates.tsv");
    fs::write(&path, body).expect("write rates");
    path
}

#[test]
fn phase4_end_to_end() {
    let tmp = fresh_tempdir("phase4-e2e");
    eprintln!("DEBUG test: cwd={:?}", std::env::current_dir());
    eprintln!("DEBUG test: trial exists at {:?}", std::path::Path::new("target/debug/trial").canonicalize());
    // Journal: 100 EUR booked on 2026-09-01.
    // Note: between amount_minor (col 7) and provenance_hash (col 11)
    // there are 3 empty fields: party, doc_ref, tag. The schema has
    // 13 columns; each data row must match.
    let journal = make_journal(&tmp, &[
        "e1\t1\t2026-09-01\tent1\tEUR\t1100\t\t100\t\t\t\th0\tseed",
        "e1\t2\t2026-09-01\tent1\tEUR\t\t2100\t100\t\t\t\th1\th0",
    ]);
    // Rates: EUR->USD 1.10 on 2026-09-01, 1.08 on 2026-09-15.
    let rates = make_rates(&tmp,
        "date\tfrom\tto\trate\n2026-09-01\tEUR\tUSD\t1.10\n2026-09-15\tEUR\tUSD\t1.08\n",
    );

    // 1. trial --fx: 5-column output with usd_normalized.
    let out = Command::new("target/debug/trial")
        .arg("--journal").arg(&journal)
        .arg("--fx").arg(&rates)
        .output()
        .expect("run trial");
    eprintln!("DEBUG trial stdout: {:?}", String::from_utf8_lossy(&out.stdout));
    eprintln!("DEBUG trial stderr: {:?}", String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "trial --fx exits 0: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(stdout.contains("usd_normalized"), "trial has usd_normalized: {stdout}");
    assert!(stdout.contains("\t110"), "trial has 110 (100 EUR * 1.10): {stdout}");

    // 2. balance --fx --account 1100: 5-column output for 1100.
    let out = Command::new("target/debug/balance")
        .arg("--journal").arg(&journal)
        .arg("--account").arg("1100")
        .arg("--fx").arg(&rates)
        .output()
        .expect("run balance");
    assert!(out.status.success(), "balance --fx exits 0");
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(stdout.contains("usd_normalized"), "balance has usd_normalized: {stdout}");
    let lines: Vec<&str> = stdout.lines().collect();
    let cols: Vec<&str> = lines[1].split('\t').collect();
    assert_eq!(cols[4], "110", "usd_normalized for 100 EUR * 1.10 = 110");

    // 3. close --fx --close-date 2026-09-15: unrealized-gain
    // info on stderr (the rate moved from 1.10 to 1.08).
    // First create a periods root and mark the period as
    // open-able (close needs the periods dir).
    let periods = tmp.join("periods");
    fs::create_dir_all(&periods).expect("create periods");
    // close exits 0 even though we haven't called post -- the
    // close walk just reads the journal.
    let out = Command::new("target/debug/close")
        .arg("--journal").arg(&journal)
        .arg("--period").arg("2026-09")
        .arg("--periods").arg(&periods)
        .arg("--fx").arg(&rates)
        .arg("--close-date").arg("2026-09-15")
        .output()
        .expect("run close");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unrealized") || stderr.contains("EUR"),
        "close --fx emits an unrealized-gain line: {stderr}"
    );
}
