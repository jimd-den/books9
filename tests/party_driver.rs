//
// tests/party_driver.rs
//
//! Pin the contract for the `party` driver: new/ls/show on
//! the parties directory tree.

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

/// Write a profile.tsv with REAL tab and newline bytes.
/// The profile has 4 fields: id, name, kind, terms.
fn seed_party(tmp: &PathBuf, id: &str, name: &str, kind: &str, terms: &str) {
    let dir = tmp.join(id);
    fs::create_dir_all(&dir).expect("create party dir");
    let body = format!(
        "id\tname\tkind\tterms\n{id}\t{name}\t{kind}\t{terms}\n"
    );
    fs::write(dir.join("profile.tsv"), body).expect("write profile");
}

#[test]
fn party_new_creates_a_leaf_with_profile_tsv() {
    let tmp = fresh_tempdir("party-new");
    let bin = env!("CARGO_BIN_EXE_party");
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--id").arg("cust:123")
        .arg("--name").arg("Acme Co.")
        .arg("--kind").arg("customer")
        .arg("--terms").arg("Net-30")
        .output()
        .expect("run party new");
    assert!(output.status.success(), "party new exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let leaf = tmp.join("cust:123");
    assert!(leaf.is_dir(), "leaf directory must exist: {leaf:?}");
    let profile = leaf.join("profile.tsv");
    assert!(profile.is_file(), "profile.tsv must exist");
    let content = fs::read_to_string(&profile).expect("read profile");
    assert!(content.contains("cust:123"), "profile has id: {content}");
    assert!(content.contains("Acme"), "profile has name: {content}");
    assert!(content.contains("customer"), "profile has kind: {content}");
    assert!(content.contains("Net-30"), "profile has terms: {content}");
}

#[test]
fn party_ls_lists_every_party() {
    let tmp = fresh_tempdir("party-ls");
    seed_party(&tmp, "cust:123", "Acme", "customer", "Net-30");
    seed_party(&tmp, "vend:1", "AcmeSupply", "vendor", "Net-30");
    let bin = env!("CARGO_BIN_EXE_party");
    let output = Command::new(bin)
        .arg("ls")
        .arg("--root").arg(&tmp)
        .output()
        .expect("run party ls");
    assert!(output.status.success(), "party ls exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 3, "header + 2 rows: {stdout}");
    assert!(lines[0].contains("id"), "header has id");
    assert!(lines[1].starts_with("cust:123") || lines[2].starts_with("cust:123"), "cust:123 row present");
}

#[test]
fn party_show_prints_one_party() {
    let tmp = fresh_tempdir("party-show");
    seed_party(&tmp, "cust:123", "Acme", "customer", "Net-30");
    let bin = env!("CARGO_BIN_EXE_party");
    let output = Command::new(bin)
        .arg("show")
        .arg("--root").arg(&tmp)
        .arg("cust:123")
        .output()
        .expect("run party show");
    assert!(output.status.success(), "party show exits 0");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("cust:123"), "show has id");
    assert!(stdout.contains("Acme"), "show has name");
    assert!(stdout.contains("Net-30"), "show has terms");
}
