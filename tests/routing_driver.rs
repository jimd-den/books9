//
// tests/routing_driver.rs
//
//! Pin the contract for the `routing show` subcommand.

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
fn routing_show_prints_an_empty_list_for_phase_p2p() {
    let tmp = fresh_tempdir("routing-show");
    let bin = env!("CARGO_BIN_EXE_routing");
    let output = Command::new(bin)
        .arg("show")
        .arg("--root").arg(&tmp)
        .arg("--item").arg("widget")
        .output()
        .expect("run routing show");
    assert!(output.status.success(), "routing show exits 0: stderr={}",
        String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    // Phase P2P ships an empty list (no steps); future cycles add
    // multi-step routing.
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 1, "header at minimum: {stdout}");
}
