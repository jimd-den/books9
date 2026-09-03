//
// One behavior under test: `post --periods` rejection line names the
// period, the close stamp when the tool wrote one, and the proposed
// entry's date.
//
// Before Phase 2 the gate said "period 2026-01 is closed" and stopped
// there. An operator staring at that line still has to answer two
// questions by hand: which entry did the tool catch, and when did
// the door shut? The information is on disk; the message should
// carry it.
//
// New shape:
//   period {YYYY-MM} closed at {stamp}; entry dated {date} refused
//
// When no stamp exists (a hand-written flag, or a close predating
// this commit), the segment collapses to:
//   period {YYYY-MM} closed; entry dated {date} refused
//
// Two tests: the quoted stamp (close ran through the tool, stamp on
// disk), and the stampless path (operator hand-wrote the flag). Both
// demand one line on stderr and the entry's own date named in it.
//
// Working rules: TDD, stdlib only, one behavior per commit.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn post_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_post"))
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

fn entry(date: &str) -> String {
    format!(
        "{h}\n\
         e1\t1\t{d}\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\t{d}\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header(),
        d = date
    )
}

fn post(journal: &Path, periods: Option<&Path>, proposed: &str) -> std::process::Output {
    let mut cmd = post_bin();
    cmd.arg("--journal").arg(journal);
    if let Some(p) = periods {
        cmd.arg("--periods").arg(p);
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
fn rejection_names_period_stamp_and_entry_date() {
    let dir = unique_dir("msg-stamped");
    let journal = dir.join("journal.tsv");
    let periods = new_project::store::periods_root(&journal);
    fs::create_dir_all(&periods).unwrap();
    fs::write(periods.join("2026-01"), "closed\n").unwrap();
    let stamp = "2026-02-01T00:00:00Z";
    fs::write(periods.join(".2026-01.last_close"), format!("{stamp}\n")).unwrap();
    new_project::store::create(&journal).expect("create journal");

    let out = post(&journal, Some(&periods), &entry("2026-01-31"));
    assert_eq!(out.status.code(), Some(2), "closed period must reject");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(stderr.lines().count(), 1, "one-line reason: {stderr}");
    assert!(stderr.contains("2026-01"), "stderr: {stderr}");
    assert!(stderr.contains(stamp), "stderr must quote the stamp; got: {stderr}");
    assert!(
        stderr.contains("2026-01-31"),
        "stderr must name the entry date; got: {stderr}"
    );
    // Nothing appended (FR-1).
    assert_eq!(new_project::store::open(&journal).unwrap(), 0);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rejection_without_a_stamp_still_names_period_and_date() {
    // Hand-written flag, no stamp anywhere: the message keeps the
    // period and the date and simply has no time to quote. The
    // absence must not degrade into the old bare message.
    let dir = unique_dir("msg-unstamped");
    let journal = dir.join("journal.tsv");
    let periods = dir.join("periods");
    fs::create_dir_all(&periods).unwrap();
    fs::write(periods.join("2026-04"), "closed\n").unwrap();
    new_project::store::create(&journal).expect("create journal");

    let out = post(&journal, Some(&periods), &entry("2026-04-05"));
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(stderr.lines().count(), 1, "one-line reason: {stderr}");
    assert!(stderr.contains("2026-04"), "stderr: {stderr}");
    assert!(
        stderr.contains("2026-04-05"),
        "stderr must name the entry date; got: {stderr}"
    );
    // The stampless phrasing must not claim a time it does not have.
    assert!(
        !stderr.contains("closed at"),
        "no stamp means no claimed time; got: {stderr}"
    );

    let _ = fs::remove_dir_all(&dir);
}