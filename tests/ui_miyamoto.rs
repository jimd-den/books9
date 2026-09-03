//
// tests/ui_miyamoto.rs
//
//! Pin the contract for the Miyamoto-mode UI:
//! - `ui ls` lists available commands (the armory).
//! - `ui help [CMD]` explains a command (the stance).
//! - `ui run CMD [args...]` runs a command via ledgerd (a single strike).
//! - The interactive REPL (no subcommand) is selectable.
//! - All stdout is the data; all diagnostics are on stderr.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::fs;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn bin() -> String {
    format!("{}/target/debug/ui", env!("CARGO_MANIFEST_DIR"))
}

fn fresh_paths(label: &str) -> (PathBuf, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!(
        "books9-ui-{label}-{pid}-{n}"
    ));
    fs::create_dir_all(&dir).expect("create tmp");
    let socket = dir.join("ledgerd.sock");
    let journal = dir.join("journal.tsv");
    (socket, journal)
}

fn seed_journal(journal: &PathBuf) {
    let mut s = String::from("entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash");
    s.push('\n');
    s.push_str("e1\t1\t2026-09-01\tent1\tUSD\t1100\t\t1000\tcust:1\tinv:1\t\t\tseed\n");
    s.push_str("e1\t2\t2026-09-01\tent1\tUSD\t\t4000\t1000\tcust:1\tinv:1\t\t\th0\n");
    fs::write(journal, s).expect("write journal");
}

#[test]
fn ui_ls_lists_commands() {
    let out = Command::new(bin())
        .arg("--socket").arg("/tmp/does-not-exist.sock")
        .arg("ls")
        .output()
        .expect("run ui ls");
    assert!(out.status.success(), "ui ls exits 0: stderr={}",
        String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Miyamoto armory: every read-mostly command is present.
    assert!(stdout.contains("trial"), "ui ls lists trial: {stdout}");
    assert!(stdout.contains("balance"), "ui ls lists balance: {stdout}");
    assert!(stdout.contains("stock"), "ui ls lists stock: {stdout}");
    assert!(stdout.contains("ar_aging"), "ui ls lists ar_aging: {stdout}");
    assert!(stdout.contains("ap_aging"), "ui ls lists ap_aging: {stdout}");
    assert!(stdout.contains("inquiry"), "ui ls lists inquiry: {stdout}");
}

#[test]
fn ui_help_explains_command() {
    let out = Command::new(bin())
        .arg("--socket").arg("/tmp/does-not-exist.sock")
        .arg("help").arg("trial")
        .output()
        .expect("run ui help trial");
    assert!(out.status.success(), "ui help exits 0: stderr={}",
        String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    // The stance for the cut: usage and one-line description.
    assert!(stdout.contains("trial"), "ui help trial mentions trial: {stdout}");
    assert!(stdout.contains("--journal"), "ui help trial explains the journal flag: {stdout}");
}

#[test]
fn ui_help_lists_commands_when_no_arg() {
    let out = Command::new(bin())
        .arg("--socket").arg("/tmp/does-not-exist.sock")
        .arg("help")
        .output()
        .expect("run ui help");
    assert!(out.status.success(), "ui help exits 0: stderr={}",
        String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("trial"), "ui help lists trial: {stdout}");
    assert!(stdout.contains("balance"), "ui help lists balance: {stdout}");
}

#[test]
fn ui_run_trial_routes_through_ledgerd() {
    let (socket, journal) = fresh_paths("ui-run-trial");
    let _ = fs::remove_file(&socket);
    seed_journal(&journal);

    let mut daemon = Command::new(format!("{}/target/debug/ledgerd", env!("CARGO_MANIFEST_DIR")))
        .arg("--socket").arg(&socket)
        .arg("--journal").arg(&journal)
        .stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().expect("start ledgerd");
    for _ in 0..50 {
        if socket.exists() { break; }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let out = Command::new(bin())
        .arg("--socket").arg(&socket)
        .arg("run").arg("trial")
        .arg("--journal").arg(&journal)
        .output()
        .expect("run ui run trial");
    assert!(out.status.success(), "ui run trial exits 0: stderr={}",
        String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("1100\tUSD\t1000"),
        "ui run trial got the trial balance: {stdout}");

    let _ = daemon.kill();
    let _ = daemon.wait();
}

#[test]
fn ui_run_pipes_to_grep() {
    // The Unix-way: ui's stdout is data; downstream tools consume it.
    let (socket, journal) = fresh_paths("ui-run-pipe");
    let _ = fs::remove_file(&socket);
    seed_journal(&journal);

    let mut daemon = Command::new(format!("{}/target/debug/ledgerd", env!("CARGO_MANIFEST_DIR")))
        .arg("--socket").arg(&socket)
        .arg("--journal").arg(&journal)
        .stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().expect("start ledgerd");
    for _ in 0..50 {
        if socket.exists() { break; }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    // ui run trial | grep 1100 -- should match the trial balance row.
    let mut ui = Command::new(bin())
        .arg("--socket").arg(&socket)
        .arg("run").arg("trial")
        .arg("--journal").arg(&journal)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn().expect("spawn ui");
    let ui_stdout = ui.stdout.take().expect("ui stdout");
    let mut grep = Command::new("grep")
        .arg("1100")
        .stdin(Stdio::from(ui_stdout))
        .stdout(Stdio::piped())
        .spawn().expect("spawn grep");
    let grep_out = grep.wait_with_output().expect("grep out");
    let ui_status = ui.wait().expect("ui wait");
    assert!(ui_status.success(), "ui exits 0");
    assert!(grep_out.status.success(), "grep finds 1100");
    let s = String::from_utf8_lossy(&grep_out.stdout);
    assert!(s.contains("1100\tUSD\t1000"),
        "grep on ui output sees 1100: {s}");

    let _ = daemon.kill();
    let _ = daemon.wait();
}

#[test]
fn ui_run_with_unknown_command_exits_2() {
    let out = Command::new(bin())
        .arg("--socket").arg("/tmp/does-not-exist.sock")
        .arg("run").arg("not_a_real_command")
        .output()
        .expect("run ui");
    assert_eq!(out.status.code(), Some(2),
        "ui run with unknown command exits 2: stderr={}",
        String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not_a_real_command") || stderr.contains("unknown"),
        "ui names the bad command on stderr: {stderr}");
}

#[test]
fn ui_run_with_no_command_lists_help() {
    // No subcommand at all: print help on stderr, exit 2.
    let out = Command::new(bin())
        .arg("--socket").arg("/tmp/does-not-exist.sock")
        .output()
        .expect("run ui");
    assert_eq!(out.status.code(), Some(2),
        "ui with no subcommand exits 2: stderr={}",
        String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Usage") || stderr.contains("usage") || stderr.contains("ls"),
        "ui prints usage: {stderr}");
}
