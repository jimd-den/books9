//
// tests/grn_driver.rs
//
//! Pin the contract for the `grn` driver: new + match.
//! `grn new` records a goods received note against a PO.
//! `ap match` (separate driver) reads both files and
//! asserts (sku, qty) match.

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

fn seed_po(root: &PathBuf, po_id: &str, lines: &[&str]) {
    let po_dir = root.join("docs").join("po");
    fs::create_dir_all(&po_dir).expect("create po dir");
    let mut body = "po_id\tvendor\tdate\tcurrency\tterms\tsku\tqty\n".to_string();
    for l in lines {
        body.push_str(&format!("{po_id}\tvend:1\t2026-09-01\tUSD\tNet-30\t{l}\n"));
    }
    fs::write(po_dir.join(format!("{po_id}.tsv")), body).expect("write po");
}

#[test]
fn grn_new_creates_a_grn_file() {
    let tmp = fresh_tempdir("grn-new");
    // Seed a PO so grn new can read the vendor.
    seed_po(&tmp, "po:1", &["sku:77\t50"]);
    let bin = env!("CARGO_BIN_EXE_grn");
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--grn-id").arg("grn:1")
        .arg("--po").arg("po:1")
        .arg("--date").arg("2026-09-15")
        .arg("--received").arg("sku:77,50")
        .output()
        .expect("run grn new");
    assert!(output.status.success(), "grn new exits 0: stderr={}",
        String::from_utf8_lossy(&output.stderr));
    let grn_path = tmp.join("docs").join("grn").join("grn:1.tsv");
    assert!(grn_path.is_file(), "GRN file exists: {grn_path:?}");
    let content = fs::read_to_string(&grn_path).expect("read grn");
    let lines: Vec<&str> = content.lines().collect();
    assert!(lines.len() >= 2, "header + 1 line: {content}");
    assert!(lines[0].starts_with("grn_id\tpo_id\tvendor\tdate\tsku\tqty"));
    assert!(lines[1].starts_with("grn:1\tpo:1\t"));
    assert!(lines[1].contains("sku:77\t50"));
}

#[test]
fn grn_match_passes_when_po_and_grn_agree() {
    let tmp = fresh_tempdir("grn-match-ok");
    seed_po(&tmp, "po:1", &["sku:77\t50"]);
    // First record the GRN.
    let bin = env!("CARGO_BIN_EXE_grn");
    let _ = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--grn-id").arg("grn:1")
        .arg("--po").arg("po:1")
        .arg("--date").arg("2026-09-15")
        .arg("--received").arg("sku:77,50")
        .output()
        .expect("grn new");
    // Now run ap match (which is a subcommand of grn for Phase P2P).
    let out = Command::new(bin)
        .arg("match")
        .arg("--root").arg(&tmp)
        .arg("--po").arg("po:1")
        .arg("--grn").arg("grn:1")
        .output()
        .expect("grn match");
    assert!(out.status.success(), "grn match exits 0: stderr={}",
        String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(stdout.contains("ok"), "match passed: {stdout}");
}

#[test]
fn grn_match_fails_on_qty_mismatch() {
    let tmp = fresh_tempdir("grn-match-bad");
    seed_po(&tmp, "po:1", &["sku:77\t50"]);
    let bin = env!("CARGO_BIN_EXE_grn");
    // GRN has qty=40 but PO has qty=50.
    let _ = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--grn-id").arg("grn:1")
        .arg("--po").arg("po:1")
        .arg("--date").arg("2026-09-15")
        .arg("--received").arg("sku:77,40")
        .output()
        .expect("grn new");
    let out = Command::new(bin)
        .arg("match")
        .arg("--root").arg(&tmp)
        .arg("--po").arg("po:1")
        .arg("--grn").arg("grn:1")
        .output()
        .expect("grn match");
    // Match fails -> exit nonzero.
    assert_ne!(out.status.code(), Some(0), "mismatch is nonzero");
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(stdout.contains("mismatch") || stdout.contains("40") || stdout.contains("50"),
        "match reports the delta: {stdout}");
}
