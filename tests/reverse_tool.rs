//
// One behavior under test: `reverse` answers a wrong entry with its
// mirror, never with an eraser.
//
//   reverse --journal PATH --entry-id ID
//
// The tool reads the journal, finds every line of the named entry,
// and appends a new entry in which every leg returns on the
// opposite SIDE of the same account: a source debit leg is answered
// by a mirror credit leg on that identical account, and a source
// credit leg by a mirror debit. Accounts do not move between
// columns; sides flip, amounts stay positive minor units. Folding
// source and mirror leaves every account net zero — that is what
// "reversed" means, and the fold is asserted, not assumed.
//
// The surface the journal cares about:
//   - the original lines are untouched, byte for byte (FR-2: there
//     is no edit path to take; reverse only appends)
//   - new entry_id is "rev-{original}"; seq restarts at 1
//   - doc_ref becomes "rev:{original doc_ref}"; an original with no
//     doc_ref gets "rev:" so the provenance still says what it is
//   - the chain is wired exactly as post wires it: seed from the
//     journal's last hash, one link per appended row, so bin/verify
//     stays green across the seam
//   - the mirror passes the same journal::validate gate before a
//     single byte moves — every tool funnels through one validator
//
// Rejections (one-line stderr, exit 2, nothing appended):
//   - unknown entry_id: the thing you want to undo is not here
//   - a source line whose doc_ref already starts "rev:": a mirror
//     of a mirror is the mistake again, wearing a different hat
//   - no --journal, no --entry-id: usage
//
// Working rules: TDD, stdlib only, one behavior per commit.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn post_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_post"))
}
fn reverse_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_reverse"))
}
fn verify_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_verify"))
}

fn unique_dir(tag: &str) -> PathBuf {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "books9-{}-{tag}-{pid}-{nanos}",
        process::id(),
        pid = process::id(),
        nanos = nanos
    ));
    fs::create_dir_all(&dir).expect("create isolated temp dir");
    dir
}

fn header() -> &'static str {
    "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash"
}

fn post(journal: &Path, proposed: &str) {
    let mut child = post_bin()
        .arg("--journal")
        .arg(journal)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(proposed.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "fixture post must succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn reverse(journal: &Path, entry_id: &str) -> std::process::Output {
    reverse_bin()
        .arg("--journal")
        .arg(journal)
        .arg("--entry-id")
        .arg(entry_id)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

fn data_lines(journal: &Path) -> Vec<Vec<String>> {
    let content = fs::read_to_string(journal).unwrap();
    content
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split('\t').map(|s| s.to_string()).collect())
        .collect()
}

#[test]
fn reverse_appends_the_mirror_entry_and_keeps_the_chain_green() {
    let dir = unique_dir("rev-happy");
    let journal = dir.join("journal.tsv");
    new_project::store::create(&journal).expect("create journal");

    post(
        &journal,
        &format!(
            "{h}\n\
             e1\t1\t2026-01-10\tent:1\tUSD\t1100\t\t10000\tp:1\tso:42\t\t\th0\n\
             e1\t2\t2026-01-10\tent:1\tUSD\t\t2100\t10000\tp:1\tso:42\t\t\th1\n",
            h = header()
        ),
    );
    let before = fs::read_to_string(&journal).unwrap();

    let out = reverse(&journal, "e1");
    assert_eq!(
        out.status.code(),
        Some(0),
        "reverse must succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stdout.is_empty(), "stdout stays clean for piping");

    let rows = data_lines(&journal);
    assert_eq!(rows.len(), 4, "mirror adds two rows to the two originals");

    // The originals are untouched, byte for byte (FR-2).
    let now = fs::read_to_string(&journal).unwrap();
    assert!(
        now.starts_with(&before),
        "reverse may only ever add; it edited the past"
    );

    // Mirror rows: each leg returns on the opposite side of the SAME
    // account, in source order. Folding source and mirror leaves
    // every account net zero (asserted below).
    let (m1, m2) = (&rows[2], &rows[3]);
    assert_eq!(m1[0], "rev-e1", "reversal carries its own entry id");
    assert_eq!(m2[0], "rev-e1");
    assert_eq!(m1[1], "1", "seq restarts inside the new entry");
    assert_eq!(m2[1], "2");
    assert_eq!(m1[5], "", "a source debit leg returns as a credit leg");
    assert_eq!(m1[6], "1100", "the account does not move; the side flips");
    assert_eq!(m1[7], "10000", "minor units do not change sign; sides do");
    assert_eq!(m2[5], "2100", "the source credit leg returns as a debit");
    assert_eq!(m2[6], "", "a leg is still one-sided");
    assert_eq!(m2[7], "10000");
    assert_eq!(m1[9], "rev:so:42", "doc_ref names what it reverses");
    assert_eq!(m2[9], "rev:so:42");

    // The fold that makes this a reversal at all: per-account debit
    // minus credit across all four rows is zero.
    let mut net: std::collections::BTreeMap<&str, i64> = Default::default();
    for r in &rows {
        let amt: i64 = r[7].parse().unwrap();
        if !r[5].is_empty() {
            *net.entry(r[5].as_str()).or_insert(0) += amt;
        }
        if !r[6].is_empty() {
            *net.entry(r[6].as_str()).or_insert(0) -= amt;
        }
    }
    assert!(
        net.values().all(|v| *v == 0),
        "source + mirror must net every account to zero; got {net:?}"
    );

    // Chain wired through the seam: verify re-walks and finds no lie.
    let v = verify_bin()
        .arg(&journal)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(
        v.status.code(),
        Some(0),
        "chain must stay continuous across a reversal; stderr: {}",
        String::from_utf8_lossy(&v.stderr)
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn reverse_refuses_an_unknown_entry_id_without_writing() {
    let dir = unique_dir("rev-ghost");
    let journal = dir.join("journal.tsv");
    new_project::store::create(&journal).expect("create journal");
    post(
        &journal,
        &format!(
            "{h}\n\
             e1\t1\t2026-01-10\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
             e1\t2\t2026-01-10\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
            h = header()
        ),
    );
    let before = fs::read_to_string(&journal).unwrap();

    let out = reverse(&journal, "nope");
    assert_eq!(out.status.code(), Some(2), "ghost entry must reject");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(stderr.lines().count(), 1, "one-line reason: {stderr}");
    assert!(stderr.contains("nope"), "stderr names the entry; got {stderr}");
    assert_eq!(
        fs::read_to_string(&journal).unwrap(),
        before,
        "a refusal may not even change the bytes it was reading"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn reverse_refuses_to_mirror_a_mirror() {
    let dir = unique_dir("rev-twice");
    let journal = dir.join("journal.tsv");
    new_project::store::create(&journal).expect("create journal");
    post(
        &journal,
        &format!(
            "{h}\n\
             e1\t1\t2026-01-10\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
             e1\t2\t2026-01-10\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
            h = header()
        ),
    );
    assert_eq!(reverse(&journal, "e1").status.code(), Some(0));
    let before = fs::read_to_string(&journal).unwrap();

    // Reversing the reversal would restore the mistake; the tool
    // refuses to be tricked back into being wrong.
    let out = reverse(&journal, "rev-e1");
    assert_eq!(out.status.code(), Some(2), "double reversal must reject");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(stderr.lines().count(), 1, "one-line reason: {stderr}");
    assert!(
        stderr.contains("rev"),
        "stderr should say why: already a reversal; got {stderr}"
    );
    assert_eq!(fs::read_to_string(&journal).unwrap(), before);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn reverse_requires_the_two_flags() {
    let dir = unique_dir("rev-usage");
    let journal = dir.join("journal.tsv");
    new_project::store::create(&journal).expect("create journal");

    let out = Command::new(env!("CARGO_BIN_EXE_reverse"))
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "no flags must reject");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--journal"), "usage names the flag: {stderr}");

    let _ = fs::remove_dir_all(&dir);
}