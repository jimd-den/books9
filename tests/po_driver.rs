//
// tests/po_driver.rs
//
//! Pin the contract for the `po` driver: new/ls/show on
//! the POs directory tree at /biz/docs/po/.

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

#[test]
fn po_new_creates_a_po_file() {
    let tmp = fresh_tempdir("po-new");
    let bin = env!("CARGO_BIN_EXE_po");
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--po-id").arg("po:1")
        .arg("--vendor").arg("vend:1")
        .arg("--date").arg("2026-09-01")
        .arg("--line").arg("sku:77,50")
        .output()
        .expect("run po new");
    assert!(output.status.success(), "po new exits 0: stderr={}",
        String::from_utf8_lossy(&output.stderr));
    let po_path = tmp.join("docs").join("po").join("po:1.tsv");
    assert!(po_path.is_file(), "PO file exists: {po_path:?}");
    let content = fs::read_to_string(&po_path).expect("read PO");
    let lines: Vec<&str> = content.lines().collect();
    assert!(lines.len() >= 2, "header + 1 line: {content}");
    assert!(lines[0].starts_with("po_id\tvendor\tdate\tcurrency\tterms\tsku\tqty"));
    assert!(lines[1].starts_with("po:1\tvend:1\t2026-09-01"));
}

#[test]
fn po_ls_lists_every_purchase_order() {
    let tmp = fresh_tempdir("po-ls");
    fs::create_dir_all(tmp.join("docs/po")).expect("create po dir");
    fs::write(tmp.join("docs/po/po:1.tsv"),
        "po_id\tvendor\tdate\tcurrency\tterms\tsku\tqty\npo:1\tvend:1\t2026-09-01\tUSD\tNet-30\tsku:77\t50\n").expect("write");
    let bin = env!("CARGO_BIN_EXE_po");
    let output = Command::new(bin)
        .arg("ls")
        .arg("--root").arg(&tmp)
        .output()
        .expect("run po ls");
    assert!(output.status.success(), "po ls exits 0");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("po:1"));
}
