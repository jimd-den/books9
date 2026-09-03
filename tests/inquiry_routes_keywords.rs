//
// tests/inquiry_routes_keywords.rs
//
//! Pin the contract for the `inquiry` driver: a small
//! keyword router that maps a natural-language question
//! to a deterministic `trial`/`balance`/`stock`/
//! `ar aging` invocation. Read-only: the agent never calls
//! `post` or any mutating tool.

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

fn run_inquiry(journal: &PathBuf, question: &str) -> std::process::Output {
    // The inquiry driver spawns sibling tools. Point it at the
    // same cargo target dir the test framework used for the
    // inquiry binary itself.
    let target_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_inquiry"));
    cmd.arg("--journal").arg(journal);
    cmd.env("INQUIRY_TOOL_TRIAL", target_dir.join("trial"));
    cmd.env("INQUIRY_TOOL_BALANCE", target_dir.join("balance"));
    cmd.env("INQUIRY_TOOL_STOCK", target_dir.join("stock"));
    cmd.env("INQUIRY_TOOL_AR_AGING", target_dir.join("ar_aging"));
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    child.stdin.as_mut().unwrap().write_all(question.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn inquiry_cash_question_routes_to_trial() {
    let tmp = fresh_tempdir("inquiry-cash");
    let journal = make_journal(&tmp);
    let out = run_inquiry(&journal, "what is the cash balance today?");
    assert!(out.status.success(), "inquiry exits 0: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    // trial output has the per-account rows; the cash account
    // (1100) is in the output.
    assert!(stdout.contains("1100\tUSD\t1000"), "trial ran: {stdout}");
}

#[test]
fn inquiry_ar_question_routes_to_ar_aging() {
    let tmp = fresh_tempdir("inquiry-ar");
    let journal = make_journal(&tmp);
    let out = run_inquiry(&journal, "what is the ar aging?");
    assert!(out.status.success(), "inquiry exits 0: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    // ar_aging output has bucket/total rows.
    assert!(stdout.contains("0-30\t1000"), "ar aging ran: {stdout}");
}

#[test]
fn inquiry_default_routes_to_trial() {
    let tmp = fresh_tempdir("inquiry-default");
    let journal = make_journal(&tmp);
    let out = run_inquiry(&journal, "what is the total?");
    assert!(out.status.success(), "inquiry exits 0: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(stdout.contains("1100\tUSD\t1000"), "trial ran as default: {stdout}");
}
