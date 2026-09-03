//
// tests/phase8_integration.rs
//
//! End-to-end integration test for the Phase 8 surface:
//! flat2tsv (legacy vendor importer) and inquiry (read-only agent).
//! The full Phase 8 also includes the ledgerd daemon, the network
//! export, and TUI -- those are deferred to a future cycle.

use std::process::Command;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::fs;
use std::io::Write;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_tempdir(label: &str) -> PathBuf {
 let n = COUNTER.fetch_add(1, Ordering::Relaxed);
 let pid = std::process::id();
 let dir = std::env::temp_dir().join(format!("books9-{label}-{pid}-{n}"));
 fs::create_dir_all(&dir).expect("create tempdir");
 dir
}

fn make_journal(dir: &PathBuf) -> PathBuf {
 let path = dir.join("journal.tsv");
 let header = "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash";
 let mut content = String::from(header);
 content.push('\n');
 content.push_str("e1\t1\t2026-09-01\tent1\tUSD\t1100\t\t1000\tcust:1\tinv:1\t\t\tseed\n");
 content.push_str("e1\t2\t2026-09-01\tent1\tUSD\t\t4000\t1000\tcust:1\tinv:1\t\t\th0\n");
 fs::write(&path, content).expect("write journal");
 path
}

#[test]
fn phase8_end_to_end() {
 let tmp = fresh_tempdir("phase8-e2e");

 // 1. flat2tsv: legacy customer master -> party profile TSVs.
 use std::process::Stdio;
 let idoc = b"VENDORHEADER\nCUSTOMER;cust:1;Acme;Net-30\nCUSTOMER;cust:2;Beta;Net-30\n";
 let mut child = Command::new("target/debug/flat2tsv")
 .stdin(Stdio::piped())
 .stdout(Stdio::piped())
 .stderr(Stdio::piped())
 .spawn()
 .unwrap();
 child.stdin.as_mut().unwrap().write_all(idoc).unwrap();
 let out = child.wait_with_output().unwrap();
 assert!(out.status.success(), "flat2tsv exits 0: stderr={}",
 String::from_utf8_lossy(&out.stderr));
 let stdout = String::from_utf8(out.stdout).unwrap();
 let lines: Vec<&str> = stdout.lines().collect();
 assert!(lines.len() >= 3, "header + 2 rows: {stdout}");
 assert!(stdout.contains("cust:1\tAcme\tcustomer\tNet-30"));
 assert!(stdout.contains("cust:2\tBeta\tcustomer\tNet-30"));

 // 2. inquiry: "what is the cash balance today?" -> trial.
 let journal = make_journal(&tmp);
 let target_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug");
 let mut cmd = Command::new(env!("CARGO_BIN_EXE_inquiry"));
 cmd.arg("--journal").arg(&journal);
 cmd.env("INQUIRY_TOOL_TRIAL", target_dir.join("trial"));
 cmd.env("INQUIRY_TOOL_BALANCE", target_dir.join("balance"));
 cmd.env("INQUIRY_TOOL_STOCK", target_dir.join("stock"));
 cmd.env("INQUIRY_TOOL_AR_AGING", target_dir.join("ar_aging"));
 cmd.stdin(std::process::Stdio::piped());
 cmd.stdout(std::process::Stdio::piped());
 cmd.stderr(std::process::Stdio::piped());
 let mut child = cmd.spawn().unwrap();
 child.stdin.as_mut().unwrap().write_all(b"what is the cash balance today?").unwrap();
 let out = child.wait_with_output().unwrap();
 assert!(out.status.success(), "inquiry exits 0: stderr={}",
 String::from_utf8_lossy(&out.stderr));
 let stdout = String::from_utf8(out.stdout).unwrap();
 assert!(stdout.contains("1100\tUSD\t1000"),
 "inquiry routed to trial and the cash row is in the output: {stdout}");
}
