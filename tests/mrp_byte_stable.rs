//
// tests/mrp_byte_stable.rs
//
//! Pin the FR-3 contract: same inputs produce same bytes.
//! The MRP driver reads open SOs and a BOM tree, computes
//! the components needed, and emits a TSV on stdout. The
//! output is byte-stable: given the same SO content and
//! the same BOM, the output bytes are identical across runs.

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

fn seed_bom(root: &PathBuf, item: &str, lines: &[&str]) {
    let dir = root.join(item);
    fs::create_dir_all(&dir).expect("create bom dir");
    let body = "item\tcomponent\tqty_per_unit\tuom\n".to_string()
        + &lines.join("\n")
        + "\n";
    fs::write(dir.join("bom.tsv"), body).expect("write bom");
}

fn seed_priced_so(root: &PathBuf, so_id: &str, rows: &[&str]) {
    let so_dir = root.join("docs").join("so");
    fs::create_dir_all(&so_dir).expect("create so dir");
    let body = "so_id\tparty\tdate\tcurrency\tterms\tsku\tqty\tunit_price_minor\tline_total_minor\n".to_string()
        + &rows.join("\n")
        + "\n";
    fs::write(so_dir.join(format!("{so_id}.priced.tsv")), body).expect("write priced SO");
}

#[test]
fn mrp_same_inputs_produce_same_bytes() {
    let tmp = fresh_tempdir("mrp-stable");
    let bom_root = tmp.join("bom-root");
    let so_root = tmp.join("biz");
    seed_bom(&bom_root, "widget", &[
        "widget\tsteel\t2\tkg",
        "widget\tscrew\t1\teach",
    ]);
    seed_bom(&bom_root, "gadget", &[
        "gadget\tsteel\t3\tkg",
    ]);
    seed_priced_so(&so_root, "000421", &[
        "000421\tcust:1\t2026-09-01\tUSD\tNet-30\twidget\t10\t100\t1000",
        "000421\tcust:1\t2026-09-01\tUSD\tNet-30\tgadget\t5\t50\t250",
    ]);

    let bin = env!("CARGO_BIN_EXE_mrp");
    let out1 = Command::new(bin)
        .arg("--demand").arg("so:000421")
        .arg("--bom-root").arg(&bom_root)
        .arg("--so-root").arg(&so_root)
        .output()
        .expect("first run");
    let out2 = Command::new(bin)
        .arg("--demand").arg("so:000421")
        .arg("--bom-root").arg(&bom_root)
        .arg("--so-root").arg(&so_root)
        .output()
        .expect("second run");
    assert!(out1.status.success() && out2.status.success(), "both runs exit 0");
    let s1 = String::from_utf8(out1.stdout).expect("utf8");
    let s2 = String::from_utf8(out2.stdout).expect("utf8");
    assert_eq!(s1, s2, "FR-3: same inputs, same output bytes");
    assert!(s1.contains("steel\t35\tkg"), "35 steel needed: {s1}");
    assert!(s1.contains("screw\t10\teach"), "10 screw needed: {s1}");
}
