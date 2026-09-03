//
// tests/flat2tsv_emits_party_profiles.rs
//
//! Pin the contract for `flat2tsv`: read an vendor-flat flat
//! file on stdin, emit a party profile TSV per data row
//! on stdout. Each emitted line is the file body for
//! `party new --root DIR --id ID --name NAME --kind KIND
//! --terms TERMS` to consume.

use std::process::Command;
use std::path::PathBuf;
use std::io::Write;
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
fn flat2tsv_emits_one_party_profile_per_row() {
 let tmp = fresh_tempdir("flat2tsv-emit");
 // b"..." with \n (real newlines in the file). One line
 // per data row, no leading spaces (the Python source
 // would otherwise leave indentation bytes in the
 // multiline b-string).
 let idoc = b"VENDORHEADER\nCUSTOMER;cust:1;Acme;Net-30\nCUSTOMER;cust:2;Beta;Net-30\n";
 let bin = env!("CARGO_BIN_EXE_flat2tsv");
 let mut child = Command::new(bin)
 .stdin(std::process::Stdio::piped())
 .stdout(std::process::Stdio::piped())
 .stderr(std::process::Stdio::piped())
 .spawn()
 .unwrap();
 child.stdin.as_mut().unwrap().write_all(idoc).unwrap();
 let out = child.wait_with_output().unwrap();
 assert!(out.status.success(), "flat2tsv exits 0: stderr={}",
 String::from_utf8_lossy(&out.stderr));
 let stdout = String::from_utf8(out.stdout).unwrap();
 let lines: Vec<&str> = stdout.lines().collect();
 assert_eq!(lines.len(), 3, "header + 2 rows: {stdout}");
 assert!(lines[0].starts_with("id\tname\tkind\tterms"));
 assert!(lines[1].contains("cust:1\tAcme\tcustomer\tNet-30"));
 assert!(lines[2].contains("cust:2\tBeta\tcustomer\tNet-30"));
}
