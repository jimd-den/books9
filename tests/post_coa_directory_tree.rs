//
// tests/post_coa_directory_tree.rs
//
//! Pin the new contract: `post --coa PATH` walks a directory
//! tree (Phase 3 migration). The flat-file shape is gone; a
//! CoA is now a tree of accounts under a root, with each leaf
//! directory containing a profile.tsv.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_post"))
}

fn fresh_tempdir(label: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("books9-{label}-{pid}-{n}"));
    fs::create_dir_all(&dir).expect("create tempdir");
    dir
}

fn make_account(root: &PathBuf, path: &str) {
    let dir = root.join(path);
    fs::create_dir_all(&dir).expect("create account dir");
    fs::write(dir.join("profile.tsv"), "code\tname\tkind\n").expect("write profile");
}


fn header() -> &'static str {
    "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash"
}

fn run_post(journal: &PathBuf, coa: Option<&PathBuf>, proposed: &str) -> std::process::Output {
    let mut cmd = bin();
    cmd.arg("--journal").arg(journal);
    if let Some(c) = coa {
        cmd.arg("--coa").arg(c);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(proposed.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn post_accepts_when_accounts_are_in_a_directory_tree() {
    let tmp = fresh_tempdir("coa-tree-ok");
    let coa = tmp.join("accounts");
    make_account(&coa, "1100");
    make_account(&coa, "2100");
    let journal = tmp.join("journal.tsv");
    let _ = fs::remove_file(&journal);
    new_project::store::create(&journal).expect("create journal");

    let proposed = format!(
        "{h}
e1\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\th0
e1\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1
",
        h = header()
    );
    let out = run_post(&journal, Some(&coa), &proposed);
    assert_eq!(
        out.status.code(),
        Some(0),
        "balanced entry with all accounts in tree must be accepted; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let n = new_project::store::open(&journal).unwrap();
    assert_eq!(n, 2, "the journal must have 2 data lines after a successful append");
}

#[test]
fn post_rejects_when_an_account_is_not_in_the_tree() {
    let tmp = fresh_tempdir("coa-tree-missing");
    let coa = tmp.join("accounts");
    make_account(&coa, "2100");
    // 1100 is NOT in the tree.
    let journal = tmp.join("journal.tsv");
    let _ = fs::remove_file(&journal);
    new_project::store::create(&journal).expect("create journal");

    let proposed = format!(
        "{h}
e1\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\th0
e1\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1
",
        h = header()
    );
    let out = run_post(&journal, Some(&coa), &proposed);
    assert_eq!(
        out.status.code(),
        Some(2),
        "unbalanced-by-coa entry must be rejected; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("1100"),
        "stderr must name the missing account; got: {stderr}"
    );
    let n = new_project::store::open(&journal).unwrap();
    assert_eq!(n, 0, "rejection must not partially append (FR-1)");
}
