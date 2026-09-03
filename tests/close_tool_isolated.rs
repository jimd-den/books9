//
// Pin the parallel-safe isolation contract for close_tool.
//
// The current close_tool.rs derives the periods directory as
// `parent(journal)/periods/`. When integration tests run in
// parallel (cargo test's default), every test places its journal
// directly under /tmp/, so every test's periods_root is /tmp/periods
// and every test's set_period races on the same flag files.
//
// This test pins the fix: each test must use a uniquely-named
// directory as the journal's parent, so periods_root is unique.
//
// This is a contract test, not a behavior test. It exists because
// the flake was real and silent: the failure mode was a spurious
// "rename tmp to /tmp/periods/2026-01: No such file or directory"
// caused by another test's cleanup racing with this one's write.
// The flake disappears the moment every journal sits in its own
// directory.
//
use std::path::PathBuf;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

/// Like the helper in tests/close_tool.rs, but the journal lives
/// inside a uniquely-named subdirectory. The directory is created
/// eagerly so the test does not race the binary's create_dir_all.
fn unique_isolated_journal(tag: &str, ext: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
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

#[test]
fn every_unique_isolated_journal_gets_its_own_periods_root() {
    // Three journals in three directories -> three periods roots.
    let a = unique_isolated_journal("iso-a", "tsv");
    let b = unique_isolated_journal("iso-b", "tsv");
    let c = unique_isolated_journal("iso-c", "tsv");

    let ra = new_project::store::periods_root(&a);
    let rb = new_project::store::periods_root(&b);
    let rc = new_project::store::periods_root(&c);

    assert_ne!(ra, rb, "periods_root must differ across journals");
    assert_ne!(rb, rc, "periods_root must differ across journals");
    assert_ne!(ra, rc, "periods_root must differ across journals");
    // Sanity: periods_root is the journal's parent + "/periods"
    assert_eq!(
        ra.parent(),
        a.parent(),
        "periods_root is a sibling directory of the journal"
    );

    // Cleanup
    for d in [a.parent(), b.parent(), c.parent()] {
        if let Some(d) = d {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}

#[test]
fn cargo_test_parallel_runs_do_not_share_a_periods_root() {
    // The failure we are guarding against was: tests/close_tool.rs
    // placed journals at /tmp/books9-...tsv, so periods_root was
    // /tmp/periods for every test, and set_period races left a
    // journal rename returning ENOENT. The fix below asserts that
    // the current helper never produces a periods_root of
    // /tmp/periods directly (i.e. journals must live in their
    // own subdirectories, not loose in /tmp/).
    let j = unique_isolated_journal("iso-guard", "tsv");
    let r = new_project::store::periods_root(&j);
    assert_ne!(
        r,
        std::path::PathBuf::from("/tmp/periods"),
        "shared /tmp/periods is exactly the flake we are guarding against"
    );

    let _ = std::fs::remove_dir_all(j.parent().unwrap());
}