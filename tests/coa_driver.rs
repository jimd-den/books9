//
// tests/coa_driver.rs
//
//! Pin the contract for the `coa` driver: list/show/new on the
//! CoA directory tree. Phase 3 ships the basic surface; future
//! phases add `coa import` (flat-file -> tree migration) and
//! `coa rm` (with the FR-2 reversing-entry correction story).

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
fn coa_ls_lists_every_account_under_root() {
    let tmp = fresh_tempdir("coa-driver-ls");
    // Seed two accounts.
    fs::create_dir_all(tmp.join("1100")).unwrap();
    fs::write(tmp.join("1100/profile.tsv"), "code\tname\tkind\tnormal_side\tparent\tstatus\n1100\tCash\tasset\tdebit\t\tactive\n").unwrap();
    fs::create_dir_all(tmp.join("2100")).unwrap();
    fs::write(tmp.join("2100/profile.tsv"), "code\tname\tkind\tnormal_side\tparent\tstatus\n2100\tAP\tliability\tcredit\t\tactive\n").unwrap();
    let bin = env!("CARGO_BIN_EXE_coa");
    let output = Command::new(bin)
        .arg("ls")
        .arg("--root").arg(&tmp)
        .output()
        .expect("run coa ls");
    assert!(output.status.success(), "coa ls exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    // Header + 2 accounts.
    assert_eq!(lines.len(), 3, "header + 2 rows: {stdout}");
    assert!(lines[0].contains("account"), "header has account: {}", lines[0]);
    // Sorted by path: 1100 first, 2100 second.
    assert!(lines[1].starts_with("1100\t"), "1100 first: {}", lines[1]);
    assert!(lines[2].starts_with("2100\t"), "2100 second: {}", lines[2]);
}

#[test]
fn coa_show_prints_a_single_account_profile() {
    let tmp = fresh_tempdir("coa-driver-show");
    fs::create_dir_all(tmp.join("1100")).unwrap();
    fs::write(
        tmp.join("1100/profile.tsv"),
        "code\tname\tkind\tnormal_side\tparent\tstatus\n1100\tCash\tasset\tdebit\t\tactive\n",
    ).unwrap();
    let bin = env!("CARGO_BIN_EXE_coa");
    let output = Command::new(bin)
        .arg("show")
        .arg("--root").arg(&tmp)
        .arg("1100")
        .output()
        .expect("run coa show");
    assert!(output.status.success(), "coa show exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    // The output is the profile as a key\tvalue TSV.
    assert!(stdout.contains("1100"), "output has code: {stdout}");
    assert!(stdout.contains("Cash"), "output has name: {stdout}");
    assert!(stdout.contains("asset"), "output has kind: {stdout}");
    assert!(stdout.contains("debit"), "output has normal_side: {stdout}");
    assert!(stdout.contains("active"), "output has status: {stdout}");
}

#[test]
fn coa_new_creates_a_leaf_with_profile_tsv() {
    let tmp = fresh_tempdir("coa-driver-new");
    let bin = env!("CARGO_BIN_EXE_coa");
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("1100")
        .arg("--name").arg("Cash")
        .arg("--kind").arg("asset")
        .arg("--normal-side").arg("debit")
        .output()
        .expect("run coa new");
    assert!(output.status.success(), "coa new exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    // The leaf must exist with profile.tsv.
    let leaf = tmp.join("1100");
    assert!(leaf.is_dir(), "leaf directory must exist: {leaf:?}");
    let profile = leaf.join("profile.tsv");
    assert!(profile.is_file(), "profile.tsv must exist");
    let content = fs::read_to_string(&profile).expect("read profile");
    assert!(content.contains("1100"), "profile has code: {content}");
    assert!(content.contains("Cash"), "profile has name: {content}");
    assert!(content.contains("asset"), "profile has kind: {content}");
    assert!(content.contains("debit"), "profile has normal_side: {content}");
    assert!(content.contains("active"), "profile has default status: {content}");
}

#[test]
fn coa_new_refuses_to_overwrite_an_existing_account() {
    // FR-2 spirit: the CoA is part of the audit trail; `coa new`
    // must not silently overwrite an existing account. Use
    // `coa rm` (future) to remove an account first.
    let tmp = fresh_tempdir("coa-driver-new-dup");
    fs::create_dir_all(tmp.join("1100")).unwrap();
    fs::write(tmp.join("1100/profile.tsv"), "code\tname\tkind\n").unwrap();
    let bin = env!("CARGO_BIN_EXE_coa");
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("1100")
        .arg("--name").arg("Cash")
        .arg("--kind").arg("asset")
        .arg("--normal-side").arg("debit")
        .output()
        .expect("run coa new");
    assert_eq!(output.status.code(), Some(2), "duplicate --account is exit 2");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("exists") || stderr.contains("already"),
        "stderr names the conflict: {stderr}");
}
