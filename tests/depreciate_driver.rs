//
// tests/depreciate_driver.rs
//
//! Pin the contract for the `depreciate` driver: reads an
//! asset profile and prints the monthly depreciation amount.

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
fn depreciate_driver_prints_the_monthly_amount() {
    let tmp = fresh_tempdir("depreciate-driver");
    fs::create_dir_all(tmp.join("ast:1")).expect("create asset");
    fs::write(
        tmp.join("ast:1/profile.tsv"),
        "id\tname\tcost_minor\tacquired\tuseful_life_months\tsalvage_minor\n         ast:1\tForklift\t5000000\t2024-01-15\t60\t500000\n",
    ).expect("write profile");
    let bin = env!("CARGO_BIN_EXE_depreciate");
    let output = Command::new(bin)
        .arg("--root").arg(&tmp)
        .arg("--asset").arg("ast:1")
        .arg("--period").arg("2026-09")
        .output()
        .expect("run depreciate");
    assert!(output.status.success(), "depreciate exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert_eq!(stdout.trim(), "75000", "monthly depreciation: {stdout}");
}
