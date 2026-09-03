//
// tests/ui_unix_way.rs
//
//! Pin the contract for the Unix-way UI:
//! - `ui --help` prints usage and exits 0.
//! - `ui --version` prints a version string and exits 0.
//! - `ui ls` lists commands on stdout, one per line.
//! - `ui ls --json` emits JSON for tooling consumers.
//! - `ui help [CMD]` shows usage for a command (or all).
//! - `ui run CMD [args...]` runs a command through ledgerd.
//! - `ui run CMD | grep ...` composes with downstream tools.
//! - `ui run BOGUS` exits 2 with the bad name on stderr.
//! - `ui` with no subcommand prints usage on stderr, exits 2.
//! - `ui` reads its socket from $BOOKS9_SOCKET if set.

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
    assert!(stdout.contains("trial"), "ui ls lists trial: {stdout}");
    assert!(stdout.contains("balance"), "ui ls lists balance: {stdout}");
    assert!(stdout.contains("stock"), "ui ls lists stock: {stdout}");
    assert!(stdout.contains("ar_aging"), "ui ls lists ar_aging: {stdout}");
    assert!(stdout.contains("ap_aging"), "ui ls lists ap_aging: {stdout}");
    assert!(stdout.contains("inquiry"), "ui ls lists inquiry: {stdout}");
}

#[test]
fn ui_ls_json_emits_parseable_json() {
    // The Unix-way: machine-readable output for tooling.
    let out = Command::new(bin())
        .arg("--socket").arg("/tmp/does-not-exist.sock")
        .arg("ls").arg("--json")
        .output()
        .expect("run ui ls --json");
    assert!(out.status.success(), "ui ls --json exits 0: stderr={}",
        String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Top-level object with a "commands" array.
    assert!(stdout.contains("\"commands\""), "ui ls --json has 'commands' key: {stdout}");
    assert!(stdout.contains("trial"), "ui ls --json contains trial: {stdout}");
    assert!(stdout.contains("balance"), "ui ls --json contains balance: {stdout}");
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
fn ui_help_flag_prints_usage_and_exits_0() {
    let out = Command::new(bin())
        .arg("--help")
        .output()
        .expect("run ui --help");
    assert!(out.status.success(), "ui --help exits 0: stderr={}",
        String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Usage") || stderr.contains("usage"),
        "ui --help prints usage: {stderr}");
    assert!(stderr.contains("ls") && stderr.contains("help") && stderr.contains("run"),
        "ui --help lists all subcommands: {stderr}");
}

#[test]
fn ui_version_flag_prints_version_and_exits_0() {
    let out = Command::new(bin())
        .arg("--version")
        .output()
        .expect("run ui --version");
    assert!(out.status.success(), "ui --version exits 0: stderr={}",
        String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("BOOKS/9") || stderr.contains("0."),
        "ui --version prints BOOKS/9: {stderr}");
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

#[test]
fn ui_run_with_passthrough_sends_arbitrary_verb() {
    // --passthrough: send a verb that isn't in the armory.
    // Useful for tools that ledgerd knows but ui hasn't been
    // updated for.
    let (socket, journal) = fresh_paths("ui-passthrough");
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

    // "verify" is a verb ledgerd can run, even if not in the
    // ui armory (ui is read-mostly; verify is read-only but
    // not in the operator's day-to-day set).
    let out = Command::new(bin())
        .arg("--socket").arg(&socket)
        .arg("run").arg("--passthrough").arg("verify")
        .output()
        .expect("run ui run --passthrough verify");
    assert!(out.status.success(), "ui run --passthrough verify exits 0: stderr={}",
        String::from_utf8_lossy(&out.stderr));

    let _ = daemon.kill();
    let _ = daemon.wait();
}

#[test]
fn ui_books9_socket_env_var_is_respected() {
    // When --socket is not given, ui reads $BOOKS9_SOCKET.
    // We override the env var; the socket the binary tries
    // must come from the env, not the default.
    let (socket, journal) = fresh_paths("ui-env-socket");
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
        .env("BOOKS9_SOCKET", &socket)
        .arg("run").arg("trial")
        .arg("--journal").arg(&journal)
        .output()
        .expect("run ui with BOOKS9_SOCKET env");
    assert!(out.status.success(),
        "ui with BOOKS9_SOCKET env exits 0: stderr={}",
        String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("1100\tUSD\t1000"),
        "ui via env socket got the trial balance: {stdout}");

    let _ = daemon.kill();
    let _ = daemon.wait();
}

#[test]
fn ui_repl_on_non_tty_falls_back_to_usage() {
    // When stdin is not a TTY (a pipe, a file, or CI), the
    // REPL is not appropriate. ui with no subcommand should
    // print usage on stderr and exit 2 -- this is what tests
    // and scripts see.
    let out = Command::new(bin())
        .arg("--socket").arg("/tmp/does-not-exist.sock")
        .stdin(Stdio::null())
        .output()
        .expect("run ui with no stdin");
    assert_eq!(out.status.code(), Some(2),
        "ui with no stdin (non-TTY) exits 2: stderr={}",
        String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Usage") || stderr.contains("ls"),
        "ui non-TTY prints usage: {stderr}");
}
