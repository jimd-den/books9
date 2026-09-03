//
// tests/bom_driver.rs
//
//! Pin the contract for the `bom` driver: new/ls/show on
//! the BOMs directory tree.

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
fn bom_new_creates_a_leaf_with_bom_tsv() {
    let tmp = fresh_tempdir("bom-new");
    let bin = env!("CARGO_BIN_EXE_bom");
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--item").arg("widget")
        .arg("--component").arg("steel")
        .arg("--qty").arg("2")
        .arg("--uom").arg("kg")
        .output()
        .expect("run bom new");
    assert!(output.status.success(), "bom new exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let leaf = tmp.join("widget");
    assert!(leaf.is_dir());
    let bom = leaf.join("bom.tsv");
    let content = fs::read_to_string(&bom).expect("read bom");
    assert!(content.contains("widget\tsteel\t2\tkg"));
}

#[test]
fn bom_show_prints_a_single_bom() {
    let tmp = fresh_tempdir("bom-show");
    fs::create_dir_all(tmp.join("widget")).expect("create bom dir");
    fs::write(
        tmp.join("widget/bom.tsv"),
        "item\tcomponent\tqty_per_unit\tuom\nwidget\tsteel\t2\tkg\nwidget\tscrew\t1\teach\n",
    ).expect("write bom");
    let bin = env!("CARGO_BIN_EXE_bom");
    let output = Command::new(bin)
        .arg("show")
        .arg("--root").arg(&tmp)
        .arg("widget")
        .output()
        .expect("run bom show");
    assert!(output.status.success(), "bom show exits 0");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("steel"));
    assert!(stdout.contains("screw"));
}
