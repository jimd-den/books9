//
// tests/ui_driver.rs
//
//! Pin the contract for the `ui` driver: connects to
//! ledgerd and runs a command. Menu-driven: the user picks
//! a command from a numbered list.

use std::process::Command;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::fs;
use std::os::unix::net::UnixListener;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_paths(label: &str) -> (PathBuf, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("books9-ui-{label}-{pid}-{n}"));
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

#[test]
#[ignore = "Phase final RED: pinned contract; implementation lands in the next commit"]
fn ui_connects_to_ledgerd_and_runs_trial() {
    let (socket, journal) = fresh_paths("ui-trial");
    let _ = fs::remove_file(&socket);
    seed_journal(&journal);

    // Start ledgerd.
    let target_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug");
    let mut daemon = Command::new(target_dir.join("ledgerd"))
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

    // Run the UI with --command trial. It should connect to
    // ledgerd, send "trial", get the output, print it.
    let mut child = Command::new(target_dir.join("ui"))
        .arg("--socket").arg(&socket)
        .arg("--command").arg("trial")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("start ui");
    let out = child.wait_with_output().unwrap();
    let _ = daemon.kill();
    let _ = daemon.wait();

    assert!(out.status.success(), "ui exits 0: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("1100\tUSD\t1000"), "trial ran via ui: {stdout}");
}
