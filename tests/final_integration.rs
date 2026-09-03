//
// tests/final_integration.rs
//
//! End-to-end integration test for the final cycle.
//! The loop: seed a journal, run trial (TSV + JSON), start
//! ledgerd, connect via ui, run a command through the
//! daemon, verify the response.

use std::process::Command;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::fs;
use std::os::unix::net::UnixStream;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_paths(label: &str) -> (PathBuf, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("books9-final-{label}-{pid}-{n}"));
    fs::create_dir_all(&dir).expect("create tmp");
    let socket = dir.join("ledgerd.sock");
    let journal = dir.join("journal.tsv");
    (socket, journal)
}

fn seed_journal(journal: &PathBuf) {
    let mut content = String::from("entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash");
    content.push('\n');
    content.push_str("e1\t1\t2026-09-01\tent1\tUSD\t1100\t\t1000\tcust:1\tinv:1\t\t\tseed\n");
    content.push_str("e1\t2\t2026-09-01\tent1\tUSD\t\t4000\t1000\tcust:1\tinv:1\t\t\th0\n");
    fs::write(journal, content).expect("write journal");
}

fn run(bin: &str, args: &[&str]) -> std::process::Output {
    let path = format!("target/debug/{bin}");
    Command::new(&path)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {bin}: {e}"))
}

#[test]
fn final_end_to_end() {
    let (socket, journal) = fresh_paths("final-e2e");
    let _ = fs::remove_file(&socket);
    seed_journal(&journal);

    // 1. trial TSV (the Phase 3 surface).
    let out = run("trial", &["--journal", journal.to_str().unwrap()]);
    assert!(out.status.success(), "trial exits 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("1100\tUSD\t1000"), "trial TSV: {stdout}");

    // 2. trial JSON (FR-7).
    let out = run("trial", &["--journal", journal.to_str().unwrap(),
        "--format", "json"]);
    assert!(out.status.success(), "trial json exits 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("\"account\""), "trial JSON has account key: {stdout}");
    assert!(stdout.contains("1100"), "trial JSON has 1100: {stdout}");

    // 3. Start ledgerd, connect, run trial via the daemon.
    let _ = fs::remove_file(&socket);
    let mut daemon = Command::new("target/debug/ledgerd")
        .arg("--socket").arg(&socket)
        .arg("--journal").arg(&journal)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("start ledgerd");
    for _ in 0..50 {
        if socket.exists() { break; }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(socket.exists(), "ledgerd socket did not appear");

    let mut stream = UnixStream::connect(&socket).expect("connect to ledgerd");
    stream.write_all(b"trial\n").expect("write to ledgerd");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).expect("read from ledgerd");
    let response = String::from_utf8_lossy(&buf);
    assert!(response.contains("1100\tUSD\t1000"),
        "ledgerd routed trial: {response}");
    drop(stream);

    // 4. ui connects to ledgerd and runs a command.
    // The interface is `ui --socket PATH run CMD [args...]`.
    let out = run("ui", &["--socket", socket.to_str().unwrap(),
        "run", "trial"]);
    assert!(out.status.success(), "ui exits 0: stderr={}",
        String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("1100\tUSD\t1000"), "ui ran trial: {stdout}");

    // Cleanup.
    let _ = daemon.kill();
    let _ = daemon.wait();
}
