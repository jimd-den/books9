//
// One behavior under test: `close --list` tells you which periods
// exist, which are shut, and when they shut -- as a TSV stream a
// downstream program can fold.
//
//   close --list --journal PATH      (root derived via store::periods_root)
//   close --list --periods DIR       (root explicit; for mounted trees)
//
// Output contract (stdout, clean):
//   header:  period<TAB>status<TAB>last_close
//   one row per flag file, sorted by period ascending (determinism:
//   the same directory always yields the same bytes)
//   a dot-prefixed file is the bookkeeping namespace, never a period
//   closed flag + stamp   -> status closed, last_close the stamp
//   closed flag, no stamp -> status closed, last_close empty
//   open flag / missing   -> status open (missing is never listed)
//   anything else         -> status malformed
//
// Missing periods directory: header only, exit 0. An empty list is
// a true statement; a refusal would be a lie about the pipe.
//
// Working rules: TDD, stdlib only, one behavior per commit.

use std::fs;
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

fn list(args: &[&str]) -> std::process::Output {
    close_bin()
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

#[test]
fn list_emits_header_and_sorted_rows_with_stamps() {
    let dir = unique_dir("list-rows");
    let periods = dir.join("periods");
    fs::create_dir_all(&periods).unwrap();
    fs::write(periods.join("2026-03"), "closed\n").unwrap();
    fs::write(periods.join(".2026-03.last_close"), "2026-04-01T00:00:00Z\n").unwrap();
    fs::write(periods.join("2026-01"), "closed\n").unwrap(); // no stamp
    fs::write(periods.join("2026-02"), "open\n").unwrap();
    // Reserved namespace must never appear as its own row -- but the
    // stamp for 2026-01 is real bookkeeping and must resolve:
    fs::write(periods.join(".2026-01.last_close"), "2026-02-01T00:00:00Z\n").unwrap();

    let out = list(&["--list", "--periods", periods.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "list must succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    assert_eq!(lines.next().unwrap(), "period\tstatus\tlast_close");
    // Sorted, dot files skipped, stamp shown when known:
    assert_eq!(lines.next().unwrap(), "2026-01\tclosed\t2026-02-01T00:00:00Z");
    assert_eq!(lines.next().unwrap(), "2026-02\topen\t");
    assert_eq!(lines.next().unwrap(), "2026-03\tclosed\t2026-04-01T00:00:00Z");
    assert!(lines.next().is_none(), "exactly three rows");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn list_marks_unrecognized_flags_malformed() {
    let dir = unique_dir("list-malformed");
    let periods = dir.join("periods");
    fs::create_dir_all(&periods).unwrap();
    fs::write(periods.join("2026-05"), "who knows\n").unwrap();

    let out = list(&["--list", "--periods", periods.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("2026-05\tmalformed\t"),
        "malformed rows are listed as such; got: {stdout}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn list_of_a_missing_directory_is_the_header_alone() {
    let dir = unique_dir("list-empty");
    let missing = dir.join("periods"); // never created

    let out = list(&["--list", "--periods", missing.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "empty list is a true statement");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout, "period\tstatus\tlast_close\n");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn list_accepts_the_journal_form_and_derives_the_root() {
    let dir = unique_dir("list-journal");
    let journal = dir.join("journal.tsv");
    new_project::store::create(&journal).expect("create journal");
    let periods = new_project::store::periods_root(&journal);
    fs::create_dir_all(&periods).unwrap();
    fs::write(periods.join("2026-07"), "closed\n").unwrap();

    let out = list(&["--list", "--journal", journal.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("2026-07\tclosed\t"), "got: {stdout}");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn list_requires_one_of_the_two_roots() {
    let out = list(&["--list"]);
    assert_eq!(out.status.code(), Some(2), "no root is a usage error");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(stderr.lines().count(), 1, "one-line reason: {stderr}");
    assert!(stderr.contains("--journal") || stderr.contains("--periods"), "got: {stderr}");
}