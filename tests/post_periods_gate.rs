//
// One behavior under test: `post --periods PATH` reads period files
// under PATH named `YYYY-MM`. Each file's content is either \"open\"
// or \"closed\". If the proposed entry's date falls in a closed
// period, post refuses to append.
//
// Phase 1 sets the API; Phase 2 owns the close tool itself (which
// flips the flag and emits a signed snapshot).
//
// Date detection: the validator already reads col 2 (date) as a
// string. We extract the leading \"YYYY-MM\" by splitting on '-'.
// Lines without a recognizable YYYY-MM prefix are skipped (they
// would have failed validate() with a malformed-row message
// anyway, so we don't double-report).
//
// Working rules: TDD, stdlib only, one behavior per commit.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_post"))
}

fn unique_dir(tag: &str) -> PathBuf {
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

fn header() -> &'static str {
    "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash"
}

fn run_post(
    journal: &PathBuf,
    periods: Option<&PathBuf>,
    proposed: &str,
) -> std::process::Output {
    let mut cmd = bin();
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
fn post_refuses_to_append_into_a_closed_period() {
    let journal = unique_dir("periods-journal");
    let periods = unique_dir("periods-closed");
    let _ = fs::remove_file(&journal);
    let _ = fs::remove_dir_all(&periods);
    new_project::store::create(&journal).expect("create journal");
    fs::create_dir_all(&periods).expect("mkdir periods");
    fs::write(periods.join("2026-01"), "closed\n").unwrap();
    fs::write(periods.join("2026-02"), "open\n").unwrap();

    // Entry dated 2026-01-15 — must be rejected because 2026-01 is closed.
    let proposed = format!(
        "{h}\n\
         e1\t1\t2026-01-15\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\t2026-01-15\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );
    let out = run_post(&journal, Some(&periods), &proposed);
    assert_eq!(
        out.status.code(),
        Some(2),
        "appending into a closed period must fail; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("2026-01"),
        "stderr must name the closed period; got: {stderr}"
    );

    // And nothing was appended.
    let n = new_project::store::open(&journal).unwrap();
    assert_eq!(n, 0, "rejection must not partially append (FR-1)");

    let _ = fs::remove_file(&journal);
    let _ = fs::remove_dir_all(&periods);
}

#[test]
fn post_accepts_an_entry_in_an_open_period() {
    let journal = unique_dir("periods-open");
    let periods = unique_dir("periods-open-dir");
    let _ = fs::remove_file(&journal);
    let _ = fs::remove_dir_all(&periods);
    new_project::store::create(&journal).expect("create journal");
    fs::create_dir_all(&periods).expect("mkdir periods");
    fs::write(periods.join("2026-02"), "open\n").unwrap();

    let proposed = format!(
        "{h}\n\
         e1\t1\t2026-02-15\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\t2026-02-15\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );
    let out = run_post(&journal, Some(&periods), &proposed);
    assert_eq!(
        out.status.code(),
        Some(0),
        "appending into an open period must succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let n = new_project::store::open(&journal).unwrap();
    assert_eq!(n, 2, "the entry must be appended");

    let _ = fs::remove_file(&journal);
    let _ = fs::remove_dir_all(&periods);
}

#[test]
fn post_accepts_when_no_period_file_exists_for_the_date() {
    // No flag file for the entry's date: today is treated as open
    // (Phase 2 will populate these files via `close`).
    let journal = unique_dir("periods-missing");
    let periods = unique_dir("periods-missing-dir");
    let _ = fs::remove_file(&journal);
    let _ = fs::remove_dir_all(&periods);
    new_project::store::create(&journal).expect("create journal");
    fs::create_dir_all(&periods).expect("mkdir periods");
    // Empty periods directory.
    assert!(fs::read_dir(&periods).unwrap().next().is_none());

    let proposed = format!(
        "{h}\n\
         e1\t1\t2026-03-15\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\t2026-03-15\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );
    let out = run_post(&journal, Some(&periods), &proposed);
    assert_eq!(
        out.status.code(),
        Some(0),
        "missing period file defaults to open; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = fs::remove_file(&journal);
    let _ = fs::remove_dir_all(&periods);
}

#[test]
fn post_skips_period_check_when_flag_is_absent() {
    // No --periods flag: post must not gate. This is the Phase 0
    // behavior preserved for backward compatibility.
    let journal = unique_dir("periods-skip");
    let _ = fs::remove_file(&journal);
    new_project::store::create(&journal).expect("create journal");

    let proposed = format!(
        "{h}\n\
         e1\t1\t2026-01-15\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\t2026-01-15\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );
    let out = run_post(&journal, None, &proposed);
    assert_eq!(
        out.status.code(),
        Some(0),
        "without --periods, post must not gate; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = fs::remove_file(&journal);
}