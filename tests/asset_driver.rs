//
// tests/asset_driver.rs
//
//! Pin the contract for the `asset` driver: new/ls.

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
fn asset_new_creates_a_leaf_with_profile_tsv() {
    let tmp = fresh_tempdir("asset-new");
    let bin = env!("CARGO_BIN_EXE_asset");
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--id").arg("ast:1")
        .arg("--name").arg("Forklift")
        .arg("--cost").arg("5000000")
        .arg("--acquired").arg("2024-01-15")
        .arg("--life").arg("60")
        .arg("--salvage").arg("500000")
        .output()
        .expect("run asset new");
    assert!(output.status.success(), "asset new exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let leaf = tmp.join("ast:1");
    assert!(leaf.is_dir());
    let profile = leaf.join("profile.tsv");
    let content = fs::read_to_string(&profile).expect("read profile");
    assert!(content.contains("ast:1\tForklift\t5000000"));
}

#[test]
fn asset_ls_lists_every_asset() {
    let tmp = fresh_tempdir("asset-ls");
    fs::create_dir_all(tmp.join("ast:1")).expect("create asset");
    fs::write(tmp.join("ast:1/profile.tsv"),
        "id\tname\tcost_minor\tacquired\tuseful_life_months\tsalvage_minor\n         ast:1\tForklift\t5000000\t2024-01-15\t60\t500000\n").expect("write");
    let bin = env!("CARGO_BIN_EXE_asset");
    let output = Command::new(bin)
        .arg("ls")
        .arg("--root").arg(&tmp)
        .output()
        .expect("run asset ls");
    assert!(output.status.success(), "asset ls exits 0");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("ast:1"));
}
