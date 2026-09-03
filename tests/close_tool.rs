//
// One behavior under test: the `close` tool, written as `bin/close`,
// marks a period Closed and emits a signed closing snapshot to stdout.
//
// Surface (per Phase 2 spec):
//   close --journal PATH --period YYYY-MM [--reason \"...\"]
//
//   --journal PATH   journal file; close derives the sibling periods
//                    directory via store::periods_root(journal)
//   --period YYYY-MM period to close
//   --reason TEXT    human-readable reason carried in the snapshot
//
//   On success:
//     - stdout: a signed closing snapshot (TSV; the contract for what
//       it contains lives in this test, not in a separate spec doc)
//     - stderr: silent
//     - exit: 0
//     - the period flag file under periods_root/YYYY-MM is set to Closed
//     - the journal is unchanged (close is read-only with respect to
//       the journal; it walks it but never appends)
//
//   On rejection (one of three):
//     - journal missing           -> stderr one-liner; exit 2
//     - period already closed     -> stderr one-liner; exit 2
//     - open entries after close  -> stderr one-liner; exit 2
//       (any data line with date strictly after the closing period
//       would break the audit; close rejects pre-emptively)
//
// The snapshot format is one TSV header + one row per currency:
//   close_id  date  period  currency  debit_total  credit_total
//             reason  entries  last_hash  provenance_hash
//
// Working rules: TDD, stdlib only, one behavior per commit.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn post_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_post"))
}
fn close_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_close"))
}

fn unique_path(tag: &str, ext: &str) -> PathBuf {
    use std::process;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    // Place the journal inside a uniquely-named directory. close
    // derives its periods_root from the journal's parent, so a
    // unique parent gives a unique periods_root and lets parallel
    // tests share the filesystem without racing on the same flag
    // file. See tests/close_tool_isolated.rs for the pin.
    let dir = std::env::temp_dir().join(format!(
        "books9-{}-{tag}-{pid}-{nanos}",
        process::id(),
        tag = tag,
        pid = process::id(),
        nanos = nanos
    ));
    std::fs::create_dir_all(&dir).expect("create isolated temp dir");
    dir.join(format!("journal.{ext}"))
}

fn header() -> &'static str {
    "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash"
}

fn post(journal: &PathBuf, proposed: &str) -> std::process::Output {
    let mut child = post_bin()
        .arg("--journal")
        .arg(journal)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn post");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(proposed.as_bytes())
        .unwrap();
    child.wait_with_output().expect("wait post")
}

fn close(journal: &PathBuf, period: &str, reason: Option<&str>) -> std::process::Output {
    let mut cmd = close_bin();
    cmd.arg("--journal").arg(journal).arg("--period").arg(period);
    if let Some(r) = reason {
        cmd.arg("--reason").arg(r);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn close")
}

#[test]
fn close_writes_a_period_flag_and_emits_a_signed_snapshot() {
    let journal = unique_path("close-happy", "tsv");
    new_project::store::create(&journal).expect("create journal");

    // Post one balanced entry dated in 2026-01.
    let proposed = format!(
        "{h}\n\
         e1\t1\t2026-01-15\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\t2026-01-15\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );
    let post_out = post(&journal, &proposed);
    assert_eq!(post_out.status.code(), Some(0), "post must succeed");

    let out = close(&journal, "2026-01", Some("month-end"));
    assert_eq!(
        out.status.code(),
        Some(0),
        "close must succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // The period flag is now Closed under periods_root.
    let periods_dir = new_project::store::periods_root(&journal);
    let flag = periods_dir.join("2026-01");
    let content = fs::read_to_string(&flag).expect("read flag");
    assert_eq!(content.trim(), "closed");

    // Stdout: one snapshot, header + at least one data row.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines().filter(|l| !l.trim().is_empty());
    let header_line = lines.next().expect("snapshot header");
    let header_cols: Vec<&str> = header_line.split('\t').collect();
    assert!(header_cols.len() >= 5, "snapshot header must be TSV; got: {header_line}");
    // First data row: USD debit_total == credit_total == 100.
    let row = lines.next().expect("at least one data row");
    let cols: Vec<&str> = row.split('\t').collect();
    assert!(cols.len() >= 5);
    // Snapshot columns: close_id(0) period(1) currency(2)
    //                  debit_total(3) credit_total(4) ...
    assert_eq!(cols[2], "USD", "currency column");
    assert_eq!(cols[3], "100", "debit_total");
    assert_eq!(cols[4], "100", "credit_total");

    let _ = fs::remove_dir_all(journal.parent().unwrap());
}

#[test]
fn close_rejects_double_close() {
    let journal = unique_path("close-double", "tsv");
    new_project::store::create(&journal).expect("create journal");
    let proposed = format!(
        "{h}\n\
         e1\t1\t2026-02-15\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\t2026-02-15\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );
    assert_eq!(post(&journal, &proposed).status.code(), Some(0));

    let first = close(&journal, "2026-02", None);
    assert_eq!(first.status.code(), Some(0));

    let second = close(&journal, "2026-02", None);
    assert_eq!(
        second.status.code(),
        Some(2),
        "second close must reject; stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(
        stderr.contains("2026-02") || stderr.to_lowercase().contains("closed"),
        "stderr must name the period or the closure; got: {stderr}"
    );
    // FR-1: no partial state. The double-close does not append to the
    // journal (close never does), and the flag file remains 'closed'.
    let periods_dir = new_project::store::periods_root(&journal);
    let flag_content = fs::read_to_string(periods_dir.join("2026-02")).expect("read flag");
    assert_eq!(flag_content.trim(), "closed");

    let _ = fs::remove_dir_all(journal.parent().unwrap());
}

#[test]
fn close_rejects_when_journal_is_missing() {
    let journal = unique_path("close-nojournal", "tsv");
    // No create(): the journal does not exist.

    let out = close(&journal, "2026-03", None);
    assert_eq!(
        out.status.code(),
        Some(2),
        "close on missing journal must reject; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.trim().is_empty(), "stderr must carry a reason");

    let _ = fs::remove_dir_all(new_project::store::periods_root(&journal));
}

#[test]
fn close_rejects_when_open_entries_exist_after_the_closing_period() {
    // A journal with a 2026-04 entry posted, then a 2026-05 entry
    // posted, cannot be closed at 2026-04 because 2026-05 is
    // already open and would break the audit (a closed period must
    // contain all prior entries and admit no new ones after it).
    let journal = unique_path("close-after", "tsv");
    new_project::store::create(&journal).expect("create journal");

    let e_april = format!(
        "{h}\n\
         e1\t1\t2026-04-15\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\t2026-04-15\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );
    assert_eq!(post(&journal, &e_april).status.code(), Some(0));
    let e_may = format!(
        "{h}\n\
         e2\t1\t2026-05-15\tent:1\tUSD\t1100\t\t200\t\t\t\t\th0\n\
         e2\t2\t2026-05-15\tent:1\tUSD\t\t2100\t200\t\t\t\t\th1\n",
        h = header()
    );
    assert_eq!(post(&journal, &e_may).status.code(), Some(0));

    let out = close(&journal, "2026-04", None);
    assert_eq!(
        out.status.code(),
        Some(2),
        "close must reject when later entries exist; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("2026-04") || stderr.contains("after"),
        "stderr must name the open-after period; got: {stderr}"
    );

    // The flag file must NOT exist (no partial side effect).
    let periods_dir = new_project::store::periods_root(&journal);
    assert!(
        !periods_dir.join("2026-04").exists(),
        "rejected close must not leave a flag file behind"
    );

    let _ = fs::remove_dir_all(journal.parent().unwrap());
}