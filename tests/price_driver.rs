//
// tests/price_driver.rs
//
//! Pin the contract for the `price` driver: read an SO,
//! look up each SKU's default_price in the items profile,
//! and write the priced SO (with unit_price_minor and
//! line_total_minor filled in).

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

fn seed_item(root: &PathBuf, id: &str, default_price: &str) {
    let dir = root.join(id);
    fs::create_dir_all(&dir).expect("create item dir");
    let body = format!(
        "id\tname\tuom\tdefault_price\n{id}\tWidget\teach\t{default_price}\n"
    );
    fs::write(dir.join("profile.tsv"), body).expect("write profile");
}

/// Create the SO file (as if `so new` had been called).
fn seed_so(so_root: &PathBuf, so_id: &str) {
    let so_dir = so_root.join("docs").join("so");
    fs::create_dir_all(&so_dir).expect("create so dir");
    let body = format!(
        "so_id\tparty\tdate\tcurrency\tterms\tsku\tqty\tunit_price_minor\tline_total_minor\n         {so_id}\tcust:123\t2026-09-01\tUSD\tNet-30\tsku:77\t40\t\t\n         {so_id}\tcust:123\t2026-09-01\tUSD\tNet-30\tsku:88\t10\t\t\n"
    );
    fs::write(so_dir.join(format!("{so_id}.tsv")), body).expect("write SO");
}

#[test]
fn price_fills_unit_price_minor_and_line_total_minor() {
    let tmp = fresh_tempdir("price-fill");
    seed_item(&tmp, "sku:77", "100");
    seed_item(&tmp, "sku:88", "250");
    seed_so(&tmp, "000421");
    let bin = env!("CARGO_BIN_EXE_price");
    let output = Command::new(bin)
        .arg("--root").arg(&tmp)
        .arg("--so").arg("000421")
        .output()
        .expect("run price");
    assert!(output.status.success(), "price exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    // The priced SO is at /biz/docs/so/000421.priced.tsv
    let priced = tmp.join("docs").join("so").join("000421.priced.tsv");
    assert!(priced.is_file(), "priced SO must exist: {priced:?}");
    let content = fs::read_to_string(&priced).expect("read priced SO");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 3, "header + 2 lines");
    // sku:77 qty=40 unit=100 -> line_total=4000
    let cols77: Vec<&str> = lines[1].split('\t').collect();
    assert_eq!(cols77[6], "40", "qty");
    assert_eq!(cols77[7], "100", "unit_price_minor");
    assert_eq!(cols77[8], "4000", "line_total_minor = 40 * 100");
    // sku:88 qty=10 unit=250 -> line_total=2500
    let cols88: Vec<&str> = lines[2].split('\t').collect();
    assert_eq!(cols88[7], "250", "unit_price_minor");
    assert_eq!(cols88[8], "2500", "line_total_minor = 10 * 250");
}

#[test]
fn price_rejects_an_unknown_sku() {
    let tmp = fresh_tempdir("price-unknown");
    // Only sku:77 has a profile; sku:99 is unknown.
    seed_item(&tmp, "sku:77", "100");
    // Create an SO with sku:99
    let so_dir = tmp.join("docs").join("so");
    fs::create_dir_all(&so_dir).expect("create so dir");
    let body = "so_id\tparty\tdate\tcurrency\tterms\tsku\tqty\tunit_price_minor\tline_total_minor\n                000421\tcust:123\t2026-09-01\tUSD\tNet-30\tsku:99\t40\t\t\n";
    fs::write(so_dir.join("000421.tsv"), body).expect("write SO");
    let bin = env!("CARGO_BIN_EXE_price");
    let output = Command::new(bin)
        .arg("--root").arg(&tmp)
        .arg("--so").arg("000421")
        .output()
        .expect("run price");
    assert_eq!(output.status.code(), Some(2), "unknown sku is exit 2");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("sku:99"), "stderr names the unknown sku: {stderr}");
}
