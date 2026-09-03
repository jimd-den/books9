//
// tests/p2p_integration.rs
//
//! End-to-end integration test for the P2P pipeline.
//! The loop: create a PO, record a GRN, run ap match,
//! run ap aging, all on a populated filesystem.

use std::process::Command;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::fs;
use std::io::Write;

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
fn p2p_end_to_end() {
    let tmp = fresh_tempdir("p2p-e2e");

    // 1. Create a PO: 50 widgets at 100 = 5000 USD.
    let out = run("po", &["new",
        "--root", tmp.to_str().unwrap(),
        "--po-id", "po:1",
        "--vendor", "vend:1",
        "--date", "2026-09-01",
        "--line", "sku:77,50",
    ]);
    assert!(out.status.success(), "po new: stderr={}", String::from_utf8_lossy(&out.stderr));
    let po_path = tmp.join("docs/po/po:1.tsv");
    assert!(po_path.is_file(), "PO file exists");

    // 2. Record a GRN: 50 widgets received.
    let out = run("grn", &["new",
        "--root", tmp.to_str().unwrap(),
        "--grn-id", "grn:1",
        "--po", "po:1",
        "--date", "2026-09-15",
        "--received", "sku:77,50",
    ]);
    assert!(out.status.success(), "grn new: stderr={}", String::from_utf8_lossy(&out.stderr));

    // 3. ap match: should be ok.
    let out = run("grn", &["match",
        "--root", tmp.to_str().unwrap(),
        "--po", "po:1",
        "--grn", "grn:1",
    ]);
    assert!(out.status.success(), "ap match exits 0: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(stdout.contains("ok"), "ap match passed: {stdout}");

    // 4. ap aging on a journal with the AP credit.
    let journal = tmp.join("journal.tsv");
    let mut content = String::from("entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash");
    content.push('\n');
    // 5000 USD debit 1100, credit 2100 on 2026-09-15 (15 days old
    // as of 2026-09-30).
    content.push_str("e1\t1\t2026-09-15\tent1\tUSD\t1100\t\t5000\tvend:1\tpo:1\t\t\tseed\n");
    content.push_str("e1\t2\t2026-09-15\tent1\tUSD\t\t2100\t5000\tvend:1\tpo:1\t\t\th0\n");
    fs::write(&journal, content).expect("write journal");
    let out = run("ap", &["aging",
        "--journal", journal.to_str().unwrap(),
        "--as-of", "2026-09-30",
    ]);
    assert!(out.status.success(), "ap aging exits 0: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(stdout.contains("0-30\t5000"), "0-30 bucket has 5000: {stdout}");

    // 5. FR-7: trial --format json on this journal.
    let mut child = Command::new("target/debug/trial")
        .arg("--journal").arg(&journal)
        .arg("--format").arg("json")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "trial json exits 0: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(stdout.contains("account"));
    assert!(stdout.contains("1100"));
    assert!(stdout.contains("2100"));
}
