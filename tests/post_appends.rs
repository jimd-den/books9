//
// One behavior under test: `post --journal PATH` (no --check) appends
// a validated entry to the journal. Each appended data line carries
// the prior line's provenance_hash as its prev_hash; the very first
// data line has prev_hash = 0 (i64 zero, 16 hex zeros).
//
// SRD: \"Append-only, hash-chained text stream that is simultaneously
// the books, the audit trail, and the source of truth.\" This commit
// lands the append-only half; the chain link logic (commit 8) lands
// the tamper-evidence half.
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
        "books9-{}-{tag}-{pid}-{nanos}.tsv",
        process::id(),
        pid = process::id(),
        nanos = nanos
    ))
}

fn header() -> &'static str {
    "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash"
}

/// Run `post --journal PATH` with `proposed` on stdin, return Output.
fn run_post(journal: &PathBuf, proposed: &str) -> std::process::Output {
    let mut child = bin()
        .arg("--journal")
        .arg(journal)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn `post --journal PATH`");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(proposed.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn post_appends_validated_entry_to_the_journal() {
    let journal = unique_path("append-ok");
    let _ = fs::remove_file(&journal);

    new_project::store::create(&journal).expect("create must succeed");
    assert_eq!(new_project::store::open(&journal).unwrap(), 0);

    // A two-leg balanced USD entry. The first row's prev_hash is
    // 0000000000000000 (the sentinel for \"first line\"). The second
    // row's prev_hash equals the first row's provenance_hash; until
    // commit 8 lands the chain, we accept any 16-char hex in those
    // columns. Today's contract: the entry is appended in full.
    let proposed = format!(
        "{h}\n\
         e1\t1\t2026-01-01\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\t2026-01-01\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );

    let out = run_post(&journal, &proposed);
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 on balanced live append; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stdout.is_empty(),
        "stdout must stay clean for piping; got: {}",
        String::from_utf8_lossy(&out.stdout)
    );

    let n = new_project::store::open(&journal).unwrap();
    assert_eq!(
        n, 2,
        "after one live append of a 2-leg entry, the journal must hold 2 data lines"
    );

    // Read back and check shape: header + 2 data lines, each a 13-col TSV.
    let content = fs::read_to_string(&journal).unwrap();
    let mut lines = content.lines();
    assert_eq!(lines.next().unwrap(), header());
    let row1: Vec<&str> = lines.next().unwrap().split('\t').collect();
    assert_eq!(row1.len(), 13, "data row must have 13 cols");
    assert_eq!(row1[0], "e1");
    assert_eq!(row1[6], ""); // credit empty (debit leg)
    assert_eq!(row1[5], "1100");
    assert_eq!(row1[7], "100");
    assert_eq!(row1[12], "0000000000000000", "first row prev_hash is zero");

    let row2: Vec<&str> = lines.next().unwrap().split('\t').collect();
    assert_eq!(row2.len(), 13);
    assert_eq!(row2[0], "e1");
    assert_eq!(row2[5], "", "credit leg: debit empty");
    assert_eq!(row2[6], "2100");
    assert_eq!(row2[7], "100");
    // The second row's prev_hash must equal the first row's
    // provenance_hash. The hash format isn't pinned yet (commit 8
    // introduces the 16-hex format); here we only assert the chain
    // link is wired and the second row's prev_hash is the first
    // row's provenance_hash verbatim.
    let prev2 = row2[12];
    assert_ne!(
        prev2, "0000000000000000",
        "second row's prev_hash must link to the prior line; got the zero sentinel"
    );
    assert_eq!(
        prev2,
        row1[11],
        "second row's prev_hash must equal the first row's provenance_hash"
    );

    let _ = fs::remove_file(&journal);
}

#[test]
fn post_refuses_to_partially_append_on_validation_failure() {
    let journal = unique_path("append-bad");
    let _ = fs::remove_file(&journal);

    new_project::store::create(&journal).expect("create must succeed");
    assert_eq!(new_project::store::open(&journal).unwrap(), 0);

    // Unbalanced: 100 debit, 0 credit. Validator must reject; the
    // journal must remain at zero data lines.
    let proposed = format!(
        "{h}\n\
         e1\t1\t2026-01-01\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n",
        h = header()
    );

    let out = run_post(&journal, &proposed);
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected nonzero exit on unbalanced; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let n = new_project::store::open(&journal).unwrap();
    assert_eq!(
        n, 0,
        "rejected entries must never partially append (FR-1); got {n} data lines"
    );

    let _ = fs::remove_file(&journal);
}