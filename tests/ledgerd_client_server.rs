//
// tests/ledgerd_client_server.rs
//
//! Pin the contract for `ledgerd`: a client connects
//! over a Unix socket, sends a command, and gets the
//! tool's output back.

use std::process::Command;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::fs;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_socket_path(label: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("books9-ledgerd-{label}-{pid}-{n}.sock"))
}

fn seed_journal(dir: &PathBuf) -> PathBuf {
    let path = dir.join("journal.tsv");
    let mut content = String::from("entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash");
    content.push('\n');
    content.push_str("e1\t1\t2026-09-01\tent1\tUSD\t1100\t\t1000\tcust:1\tinv:1\t\t\tseed\n");
    content.push_str("e1\t2\t2026-09-01\tent1\tUSD\t\t4000\t1000\tcust:1\tinv:1\t\t\th0\n");
    fs::write(&path, content).expect("write journal");
    path
}

#[test]
#[ignore = "Phase final RED: pinned contract; implementation lands in the next commit"]
fn ledgerd_client_server_round_trip() {
    let tmp_dir = std::env::temp_dir().join(format!(
        "books9-ledgerd-{}",
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&tmp_dir).expect("create tmp");
    let journal = seed_journal(&tmp_dir);
    let socket = fresh_socket_path("client-server");
    let _ = fs::remove_file(&socket);

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
    assert!(socket.exists(), "ledgerd socket did not appear");

    let mut stream = UnixStream::connect(&socket).expect("connect");
    stream.write_all(b"trial\n").expect("write");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).expect("read");
    let response = String::from_utf8_lossy(&buf);
    assert!(response.contains("1100\tUSD\t1000"), "trial ran: {response}");

    drop(stream);
    let _ = daemon.kill();
    let _ = daemon.wait();
}
