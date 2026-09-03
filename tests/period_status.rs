//
// One behavior under test: `store::period_status(periods_root, period)`
// reads the period flag file under `periods_root/YYYY-MM` and returns
// a typed status:
//   - Open       if the file is absent (defaults to open)
//   - Closed     if the file's content is exactly "closed"
//   - Malformed  if the file exists but its content is anything else
//
// The function is read-only: it does not write to the disk, it does
// not create missing files, it does not normalize whitespace beyond
// a trim. Its job is to turn the on-disk layout into a Rust enum so
// callers (post, close) can match on the result instead of doing
// string compares inline.
//
// The SRD's per-period gate is: missing => open, "closed" => refuse.
// This commit widens the gate to admit a third state so a corrupted
// flag file (an editor crash, a half-written close) refuses to be
// silently treated as either open or closed.
//
// Working rules: TDD, stdlib only, one behavior per commit.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use new_project::store::{period_status, PeriodStatus};

fn unique_dir(tag: &str) -> PathBuf {
    use std::process;
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

#[test]
fn period_status_open_when_file_is_missing() {
    let dir = unique_dir("status-missing");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir periods");
    // No flag file written.
    let s = period_status(&dir, "2026-01").expect("must not error on missing file");
    assert!(matches!(s, PeriodStatus::Open), "missing => Open; got {s:?}");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn period_status_closed_when_file_says_closed() {
    let dir = unique_dir("status-closed");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir periods");
    fs::write(dir.join("2026-01"), "closed\n").expect("write closed");
    let s = period_status(&dir, "2026-01").expect("must not error on closed file");
    assert!(matches!(s, PeriodStatus::Closed), "got {s:?}");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn period_status_open_when_file_says_open() {
    // An explicit "open" file (rather than absent) also reads as Open.
    // The store treats anything other than "closed" as not-closed; the
    // file's presence is otherwise inert.
    let dir = unique_dir("status-open");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir periods");
    fs::write(dir.join("2026-02"), "open\n").expect("write open");
    let s = period_status(&dir, "2026-02").expect("must not error on open file");
    assert!(matches!(s, PeriodStatus::Open), "got {s:?}");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn period_status_malformed_when_file_content_is_unrecognized() {
    let dir = unique_dir("status-malformed");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir periods");
    fs::write(dir.join("2026-03"), "garbage that is not closed nor open\n")
        .expect("write garbage");
    let s = period_status(&dir, "2026-03").expect("read must succeed; classification fails");
    match s {
        PeriodStatus::Malformed(reason) => {
            assert!(!reason.is_empty(), "Malformed must carry a reason");
        }
        other => panic!("expected Malformed, got {other:?}"),
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn period_status_is_read_only() {
    // The function must not create the period file when it is
    // missing; it must not create the periods directory either. A
    // missing flag file is read as Open without any side effect.
    let dir = unique_dir("status-readonly");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir periods");
    assert!(!dir.join("2026-04").exists(), "precondition: no flag file");
    let _ = period_status(&dir, "2026-04").expect("must not error");
    assert!(
        !dir.join("2026-04").exists(),
        "period_status must not create the flag file as a side effect"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn period_status_handles_trailing_whitespace_in_closed_file() {
    // Editors and `echo` insert trailing newlines; we trim before
    // classifying so a "closed\n" file still reads as Closed.
    let dir = unique_dir("status-whitespace");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir periods");
    fs::write(dir.join("2026-05"), "closed\r\n").expect("write closed crlf");
    let s = period_status(&dir, "2026-05").expect("must not error");
    assert!(matches!(s, PeriodStatus::Closed), "got {s:?}");

    let _ = fs::remove_dir_all(&dir);
}