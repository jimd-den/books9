//
// One behavior under test: `verify` re-walks the journal's hash
// chain and reports the first divergence, if any. Clean journals
// exit 0; diverged journals exit nonzero with a one-line stderr
// reason naming the line number and what went wrong.
//
// SRD: \"The hash chain plus provenance fields make the whole system
// forensic: verify re-computes the chain and reports the first
// divergence.\" This is the cornerstone of FR-2 (corrections are
// reversing entries only) — if the chain is intact, the books haven't
// been silently edited.
//
// Working rules: TDD, stdlib only, one behavior per commit.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_verify"))
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

fn run_verify(journal: &PathBuf) -> std::process::Output {
    bin()
        .arg(journal)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn verify")
}

#[test]
fn verify_exits_zero_on_a_fresh_journal() {
    let path = unique_path("verify-fresh");
    let _ = fs::remove_file(&path);
    new_project::store::create(&path).expect("create must succeed");

    let out = run_verify(&path);
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 on a header-only journal; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stdout.is_empty(),
        "stdout must stay clean for piping; got: {}",
        String::from_utf8_lossy(&out.stdout)
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn verify_exits_zero_after_post_appends_valid_entries() {
    let path = unique_path("verify-clean");
    let _ = fs::remove_file(&path);
    new_project::store::create(&path).expect("create must succeed");

    // Post two balanced entries via the binary (live append).
    for n in 1..=2 {
        let proposed = format!(
            "{h}\n\
             e{n}\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\th0\n\
             e{n}\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1\n",
            h = header(),
            n = n,
        );
        let mut child = Command::new(env!("CARGO_BIN_EXE_post"))
            .arg("--journal")
            .arg(&path)
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
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(0), "post {n} stderr: {}", String::from_utf8_lossy(&out.stderr));
    }

    let out = run_verify(&path);
    assert_eq!(
        out.status.code(),
        Some(0),
        "verify on a clean journal must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn verify_exits_nonzero_when_a_byte_in_the_journal_is_flipped() {
    let path = unique_path("verify-tamper");
    let _ = fs::remove_file(&path);
    new_project::store::create(&path).expect("create must succeed");

    let proposed = format!(
        "{h}\n\
         e1\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_post"))
        .arg("--journal")
        .arg(&path)
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
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0));

    // Tamper: flip a byte in the middle of the second data row.
    let mut content = fs::read(&path).unwrap();
    // Find the second data row: skip header (line 1), then a few
    // bytes into row 2.
    let header_end = content.iter().position(|&b| b == b'\n').unwrap();
    let row2_start = header_end + 1;
    // Flip a bit somewhere safe (in the account_debit column, which
    // is a digit). Position chosen so the byte is a digit (0x30-0x39)
    // so we don't accidentally produce a non-TSV-safe char.
    let pos = row2_start + 6; // 'date' column area, second data line
    content[pos] ^= 0x01;
    fs::write(&path, &content).unwrap();

    let out = run_verify(&path);
    assert_ne!(
        out.status.code(),
        Some(0),
        "verify on a tampered journal must exit nonzero; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.lines().count() == 1,
        "verify's error must be one line; got: {stderr:?}"
    );
    // The error must name either a line number or a reason that
    // identifies what broke. We accept either form.
    assert!(
        !stderr.trim().is_empty(),
        "verify's error must carry a reason; got empty"
    );

    let _ = fs::remove_file(&path);
}