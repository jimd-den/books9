//
// tests/so_driver.rs
//
//! Pin the contract for the `so` driver: create a sales
//! order at /biz/docs/so/{NNNNNN}.tsv with header + one
//! row per line. The priced SO (with unit_price_minor) is
//! the input to `price` and then `invoice`; the unpriced
//! SO is the user's intent.

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
fn so_new_creates_an_so_file_with_header_and_lines() {
    let tmp = fresh_tempdir("so-new");
    let bin = env!("CARGO_BIN_EXE_so");
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--so-id").arg("000421")
        .arg("--party").arg("cust:123")
        .arg("--date").arg("2026-09-01")
        .arg("--currency").arg("USD")
        .arg("--terms").arg("Net-30")
        .arg("--line").arg("sku:77,40")
        .arg("--line").arg("sku:88,10")
        .output()
        .expect("run so new");
    assert!(output.status.success(), "so new exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    // The SO is at <root>/docs/so/000421.tsv.
    let so_path = tmp.join("docs").join("so").join("000421.tsv");
    assert!(so_path.is_file(), "SO file must exist: {so_path:?}");
    let content = fs::read_to_string(&so_path).expect("read SO");
    let lines: Vec<&str> = content.lines().collect();
    // Header + 2 lines.
    assert_eq!(lines.len(), 3, "header + 2 lines: {content}");
    assert_eq!(lines[0], "so_id\tparty\tdate\tcurrency\tterms\tsku\tqty\tunit_price_minor\tline_total_minor");
    assert!(lines[1].starts_with("000421\tcust:123\t2026-09-01\tUSD\tNet-30\tsku:77\t40"));
    assert!(lines[2].starts_with("000421\tcust:123\t2026-09-01\tUSD\tNet-30\tsku:88\t10"));
}

#[test]
fn so_new_rejects_a_duplicate_so_id() {
    let tmp = fresh_tempdir("so-dup");
    let bin = env!("CARGO_BIN_EXE_so");
    // First call: creates the SO.
    let _ = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--so-id").arg("000421")
        .arg("--party").arg("cust:123")
        .arg("--date").arg("2026-09-01")
        .arg("--currency").arg("USD")
        .arg("--terms").arg("Net-30")
        .arg("--line").arg("sku:77,40")
        .output()
        .expect("first so new");
    // Second call: same SO id, must reject.
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--so-id").arg("000421")
        .arg("--party").arg("cust:123")
        .arg("--date").arg("2026-09-01")
        .arg("--currency").arg("USD")
        .arg("--terms").arg("Net-30")
        .arg("--line").arg("sku:77,40")
        .output()
        .expect("second so new");
    assert_eq!(output.status.code(), Some(2), "duplicate so-id is exit 2");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("exists") || stderr.contains("000421"),
        "stderr names the conflict: {stderr}");
}
