//
// One behavior under test: `close` records when it closed a period.
//
// Immediately after store::set_period lands the `closed` flag, close
// writes a sibling `periods/.{YYYY-MM}.last_close` holding one
// ISO-8601 UTC timestamp (YYYY-MM-DDTHH:MM:SSZ) and nothing else.
//
// Why the dot-prefix: the flag files themselves are named by period
// (`2026-01`), and store::set_period refuses dot-prefixed names so
// the bookkeeping namespace is reserved. The stamp lives in that
// reserved namespace; period_status never looks at it, and a
// directory scan for flags cannot mistake it for a period.
//
// The clock is read exactly once, in main, and handed to a pure
// formatter (libbiz::time::format_utc). The formatter is unit-tested
// against known epochs; this test pins only the shape and the
// placement, not the value.
//
// Working rules: TDD, stdlib only, one behavior per commit.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn close_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_close"))
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

fn post(journal: &Path, date: &str) {
    let proposed = format!(
        "{h}\n\
         e1\t1\t{d}\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\t{d}\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header(),
        d = date
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_post"))
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
    let st = child.wait_with_output().unwrap();
    assert_eq!(st.status.code(), Some(0), "fixture post must succeed");
}

fn close(journal: &Path, period: &str) -> std::process::Output {
    close_bin()
        .arg("--journal")
        .arg(journal)
        .arg("--period")
        .arg(period)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

#[test]
fn close_records_an_iso8601_utc_stamp_next_to_the_flag() {
    let dir = unique_dir("stamp-shape");
    let journal = dir.join("journal.tsv");
    new_project::store::create(&journal).expect("create journal");
    post(&journal, "2026-01-10");

    let out = close(&journal, "2026-01");
    assert_eq!(
        out.status.code(),
        Some(0),
        "close must succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let periods_root = new_project::store::periods_root(&journal);
    let flag = periods_root.join("2026-01");
    assert_eq!(fs::read_to_string(&flag).unwrap().trim(), "closed");

    let stamp_path = periods_root.join(".2026-01.last_close");
    let raw = fs::read_to_string(&stamp_path)
        .unwrap_or_else(|e| panic!("close must record {stamp_path:?}: {e}"));

    // Exactly one timestamp, optionally newline-terminated.
    let stamp = raw.trim();
    assert_eq!(
        stamp,
        raw.lines().next().unwrap_or("").trim(),
        "the stamp file holds one line: {raw:?}"
    );
    assert_eq!(stamp.len(), 20, "YYYY-MM-DDTHH:MM:SSZ is 20 chars: {stamp:?}");
    assert_eq!(&stamp[4..5], "-");
    assert_eq!(&stamp[7..8], "-");
    assert_eq!(&stamp[10..11], "T");
    assert_eq!(&stamp[13..14], ":");
    assert_eq!(&stamp[16..17], ":");
    assert_eq!(&stamp[19..20], "Z");
    assert!(
        stamp[..19].bytes().enumerate().all(|(i, b)| if i == 10 {
            b == b'T'
        } else if matches!(i, 4 | 7) {
            b == b'-'
        } else if matches!(i, 13 | 16) {
            b == b':'
        } else {
            b.is_ascii_digit()
        }),
        "every non-separator byte must be a digit: {stamp:?}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_rejected_close_records_no_stamp() {
    // FR-1 applies to the gate's bookkeeping too: if close refuses,
    // neither the flag nor the stamp may appear.
    let dir = unique_dir("stamp-reject");
    let journal = dir.join("journal.tsv");
    // No journal on disk: close must reject before writing anything.
    let out = close(&journal, "2026-06");
    assert_eq!(out.status.code(), Some(2), "missing journal must reject");

    let periods_root = new_project::store::periods_root(&journal);
    assert!(
        !periods_root.join("2026-06").exists(),
        "rejected close leaves no flag"
    );
    assert!(
        !periods_root.join(".2026-06.last_close").exists(),
        "rejected close leaves no stamp"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn double_close_writes_no_second_stamp() {
    // The second close is refused, so the stamp from the first close
    // survives untouched: the recorded time is the time the door
    // actually shut, not the time someone last knocked on it.
    let dir = unique_dir("stamp-twice");
    let journal = dir.join("journal.tsv");
    new_project::store::create(&journal).expect("create journal");
    post(&journal, "2026-07-10");

    assert_eq!(close(&journal, "2026-07").status.code(), Some(0));
    let periods_root = new_project::store::periods_root(&journal);
    let stamp_path = periods_root.join(".2026-07.last_close");
    let first = fs::read_to_string(&stamp_path).unwrap();

    let second = close(&journal, "2026-07");
    assert_eq!(second.status.code(), Some(2), "double close must reject");
    let now = fs::read_to_string(&stamp_path).unwrap();
    assert_eq!(first, now, "a refused close must not move the stamp");

    let _ = fs::remove_dir_all(&dir);
}