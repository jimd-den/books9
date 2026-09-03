//
// tests/proj_wbs_driver.rs
//
//! Pin the contract for the `proj new` and `wbs show` subcommands.

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
fn proj_new_creates_a_project_directory() {
    let tmp = fresh_tempdir("proj-new");
    let bin = env!("CARGO_BIN_EXE_proj");
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--code").arg("p:1")
        .arg("--name").arg("Apollo")
        .arg("--parent").arg("")
        .output()
        .expect("run proj new");
    assert!(output.status.success(), "proj new exits 0: stderr={}",
        String::from_utf8_lossy(&output.stderr));
    let leaf = tmp.join("p:1");
    assert!(leaf.is_dir());
    let profile = leaf.join("profile.tsv");
    let content = fs::read_to_string(&profile).expect("read profile");
    assert!(content.contains("p:1"));
    assert!(content.contains("Apollo"));
}

#[test]
fn wbs_show_prints_a_projects_profile() {
    let tmp = fresh_tempdir("wbs-show");
    fs::create_dir_all(tmp.join("p:1")).expect("create project");
    fs::write(tmp.join("p:1/profile.tsv"),
        "code\tname\tparent\np:1\tApollo\t\n").expect("write");
    let bin = env!("CARGO_BIN_EXE_wbs");
    let output = Command::new(bin)
        .arg("show")
        .arg("--root").arg(&tmp)
        .arg("--code").arg("p:1")
        .output()
        .expect("run wbs show");
    assert!(output.status.success(), "wbs show exits 0: stderr={}",
        String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("p:1"));
    assert!(stdout.contains("Apollo"));
}
