//
// tests/org_driver.rs
//
//! Pin the contract for the `org` driver: new/ls on the orgs tree.

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
fn org_new_creates_a_leaf_with_profile_tsv() {
    let tmp = fresh_tempdir("org-new");
    let bin = env!("CARGO_BIN_EXE_org");
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--code").arg("d:1")
        .arg("--name").arg("Engineering")
        .arg("--parent").arg("")
        .output()
        .expect("run org new");
    assert!(output.status.success(), "org new exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let leaf = tmp.join("d:1");
    assert!(leaf.is_dir());
    let profile = leaf.join("profile.tsv");
    let content = fs::read_to_string(&profile).expect("read profile");
    assert!(content.contains("d:1"));
    assert!(content.contains("Engineering"));
}

#[test]
fn org_ls_lists_every_department() {
    let tmp = fresh_tempdir("org-ls");
    fs::create_dir_all(tmp.join("d:1")).expect("create dept");
    fs::write(tmp.join("d:1/profile.tsv"),
        "code\tname\tparent\tcost_center\nd:1\tEng\t\td:1\n").expect("write");
    let bin = env!("CARGO_BIN_EXE_org");
    let output = Command::new(bin)
        .arg("ls")
        .arg("--root").arg(&tmp)
        .output()
        .expect("run org ls");
    assert!(output.status.success(), "org ls exits 0");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("d:1"));
}
