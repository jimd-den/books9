//
// One behavior under test: `post --coa PATH` reads a chart of accounts
// from PATH (one account per line) and rejects any line whose
// account_debit or account_credit is not in the set. A missing --coa
// flag skips the check entirely (Phase 1 partial FR-1; the full
// chart-of-accounts tooling is Phase 3).
//
// SRD: FR-1 (\"post rejects any entry whose ... accounts don't exist\").
// The Coa file is plain text: one account per line, comments start
// with '#', blank lines are ignored.
//
// Working rules: TDD, stdlib only, one behavior per commit.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_post"))
}

fn unique_path(tag: &str) -> PathBuf {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "books9-{}-{tag}-{pid}-{nanos}",
        process::id(),
        pid = process::id(),
        nanos = nanos
    ))
}

/// Seed a directory-tree CoA at `root`. Each item in `accounts`
/// is a relative path that becomes a leaf directory with a
/// `profile.tsv` inside. This is the Phase 3 CoA shape; the
/// flat-file form was retired in commit 16.
fn seed_coa_tree(root: &PathBuf, accounts: &[&str]) {
    fs::create_dir_all(root).expect("create coa root");
    for a in accounts {
        let dir = root.join(a);
        fs::create_dir_all(&dir).expect("create account dir");
        fs::write(dir.join("profile.tsv"), "code\tname\tkind\n").expect("write profile");
    }
}

fn unique_journal(tag: &str) -> PathBuf {
    let mut p = unique_path(tag);
    p.set_extension("tsv");
    p
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
fn post_accepts_when_all_accounts_are_in_the_coa() {
    let journal = unique_journal("coa-ok");
    let coa = unique_path("coa-ok");
    let _ = fs::remove_file(&journal);
    let _ = fs::remove_file(&coa);
    new_project::store::create(&journal).expect("create journal");
    seed_coa_tree(&coa, &["1100", "2100"]);

    let proposed = format!(
        "{h}\n\
         e1\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );
    let out = run_post(&journal, Some(&coa), &proposed);
    assert_eq!(
        out.status.code(),
        Some(0),
        "balanced entry with all accounts in coa must be accepted; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let n = new_project::store::open(&journal).unwrap();
    assert_eq!(n, 2, "the journal must have 2 data lines after a successful append");

    let _ = fs::remove_file(&journal);
    let _ = fs::remove_file(&coa);
}

#[test]
fn post_rejects_when_a_debit_account_is_not_in_the_coa() {
    let journal = unique_journal("coa-bad-debit");
    let coa = unique_path("coa-bad-debit");
    let _ = fs::remove_file(&journal);
    let _ = fs::remove_file(&coa);
    new_project::store::create(&journal).expect("create journal");
    // Only 2100 in the coa; the proposed debit account (1100) is missing.
    seed_coa_tree(&coa, &["2100"]);

    let proposed = format!(
        "{h}\n\
         e1\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1\n",
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
    // And: nothing was appended.
    let n = new_project::store::open(&journal).unwrap();
    assert_eq!(n, 0, "rejection must not partially append (FR-1)");

    let _ = fs::remove_file(&journal);
    let _ = fs::remove_file(&coa);
}

#[test]
fn post_rejects_when_a_credit_account_is_not_in_the_coa() {
    let journal = unique_journal("coa-bad-credit");
    let coa = unique_path("coa-bad-credit");
    let _ = fs::remove_file(&journal);
    let _ = fs::remove_file(&coa);
    new_project::store::create(&journal).expect("create journal");
    // Only 1100 in the coa; the proposed credit account (2100) is missing.
    seed_coa_tree(&coa, &["1100"]);

    let proposed = format!(
        "{h}\n\
         e1\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );
    let out = run_post(&journal, Some(&coa), &proposed);
    assert_eq!(
        out.status.code(),
        Some(2),
        "missing credit account must be rejected; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("2100"),
        "stderr must name the missing account; got: {stderr}"
    );

    let _ = fs::remove_file(&journal);
    let _ = fs::remove_file(&coa);
}

#[test]
fn post_skips_coa_check_when_flag_is_absent() {
    // The default (no --coa) means: don't check accounts. This
    // preserves the Phase 0/1 default where accounts weren't part
    // of the validation surface yet.
    let journal = unique_journal("coa-skip");
    let _ = fs::remove_file(&journal);
    new_project::store::create(&journal).expect("create journal");

    let proposed = format!(
        "{h}\n\
         e1\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );
    let out = run_post(&journal, None, &proposed);
    assert_eq!(
        out.status.code(),
        Some(0),
        "without --coa, post must not check accounts; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let n = new_project::store::open(&journal).unwrap();
    assert_eq!(n, 2, "the entry must be appended without the coa check");

    let _ = fs::remove_file(&journal);
}