//
// One behavior under test: `store::set_period(periods_root, period,
// status)` writes a flag file under `periods_root/YYYY-MM` reflecting
// the requested `PeriodStatus`. The write is atomic: write-temp +
// fsync + rename, same shape as `store::append` but with a smaller
// payload (a few bytes). The function refuses to write to any path
// other than exactly `periods_root/<period>` — no parent traversal,
// no absolute filename, no dotfile escape.
//
// Phase 2's `close` tool uses this to mark a period Closed. Phase 2
// uses it again to refuse double-close (read current status, write
// new status only if it changed).
//
// Working rules: TDD, stdlib only, one behavior per commit.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use new_project::store::{set_period, PeriodStatus};

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
fn set_period_writes_closed_to_a_fresh_period_file() {
    let dir = unique_dir("set-closed");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir periods");

    set_period(&dir, "2026-01", PeriodStatus::Closed).expect("set must succeed");
    let content = fs::read_to_string(dir.join("2026-01")).expect("read flag");
    assert_eq!(content.trim(), "closed", "set_period(Closed) must write 'closed'");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn set_period_overwrites_a_prior_value() {
    // The function writes; it does not append. A prior "open" file is
    // replaced by the new status atomically.
    let dir = unique_dir("set-overwrite");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir periods");
    fs::write(dir.join("2026-02"), "open\n").expect("seed open");

    set_period(&dir, "2026-02", PeriodStatus::Closed).expect("set must succeed");
    let content = fs::read_to_string(dir.join("2026-02")).expect("read flag");
    assert_eq!(content.trim(), "closed");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn set_period_writes_open_when_status_is_open() {
    let dir = unique_dir("set-open");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir periods");
    fs::write(dir.join("2026-03"), "closed\n").expect("seed closed");

    set_period(&dir, "2026-03", PeriodStatus::Open).expect("set must succeed");
    let content = fs::read_to_string(dir.join("2026-03")).expect("read flag");
    assert_eq!(content.trim(), "open");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn set_period_leaves_no_temp_files_in_the_periods_directory() {
    // The atomic shape: write-temp + fsync + rename. After a
    // successful call, exactly one file exists under periods_root
    // (the flag file we asked for). No ".tmp" siblings.
    let dir = unique_dir("set-no-tmp");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir periods");

    set_period(&dir, "2026-04", PeriodStatus::Closed).expect("set must succeed");

    let entries: Vec<String> = fs::read_dir(&dir)
        .expect("read_dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        entries,
        vec!["2026-04".to_string()],
        "exactly one flag file expected; got {entries:?}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn set_period_refuses_parent_traversal_in_period_name() {
    // A malicious or careless period name must not let a caller escape
    // periods_root and overwrite an unrelated file. The function
    // refuses any period name containing a path separator.
    let dir = unique_dir("set-traversal");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir periods");
    // Outside file that must NOT be written.
    let outside = unique_dir("set-traversal-outside");
    let _ = fs::remove_dir_all(&outside);
    fs::create_dir_all(&outside).expect("mkdir outside");
    let outside_file = outside.join("do-not-touch.txt");
    fs::write(&outside_file, "untouched").expect("seed outside");

    let bad = format!("../{}", outside_file.file_name().unwrap().to_string_lossy());
    let res = set_period(&dir, &bad, PeriodStatus::Closed);
    assert!(res.is_err(), "set_period must refuse parent traversal");

    // The outside file is byte-identical.
    let after = fs::read_to_string(&outside_file).expect("read outside");
    assert_eq!(after, "untouched");

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&outside);
}

#[test]
fn set_period_refuses_absolute_filename_in_period_name() {
    let dir = unique_dir("set-absolute");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir periods");

    let res = set_period(&dir, "/etc/passwd", PeriodStatus::Closed);
    assert!(res.is_err(), "set_period must refuse absolute path in period name");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn set_period_refuses_dotfile_period_names() {
    // Hidden files (".something") under periods_root are reserved for
    // bookkeeping (e.g. ".last_close"). set_period writes flag files
    // only; hidden names are rejected so close's bookkeeping and the
    // flag files never share a namespace.
    let dir = unique_dir("set-dotfile");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir periods");

    let res = set_period(&dir, ".last_close", PeriodStatus::Closed);
    assert!(res.is_err(), "set_period must refuse dotfile names");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn set_period_creates_the_flag_file_when_periods_dir_exists() {
    // The contract is: periods_dir is assumed to exist (callers
    // create it via fs::create_dir_all when needed). set_period does
    // not need to mkdir periods_root itself, but it must write the
    // flag file inside it.
    let dir = unique_dir("set-create-flag");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir periods");

    set_period(&dir, "2026-05", PeriodStatus::Closed).expect("set must succeed");
    assert!(dir.join("2026-05").exists());

    let _ = fs::remove_dir_all(&dir);
}