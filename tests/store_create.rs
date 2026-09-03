//
// One behavior under test: `store::create` writes the SRD's 13-column
// header line to a fresh file at PATH and refuses to overwrite an
// existing one.
//
// SRD: FR-2 ("corrections are reversing entries only") and the journal
// format clause ("Append-only ... journal"). The kernel never silently
// overwrites the books; create must error loudly if the file exists.
//
// Working rules: TDD, stdlib only. No `tempfile` crate; we synthesize
// unique paths under std::env::temp_dir() using pid + nanos.

use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use new_project::store;

fn unique_path(tag: &str) -> PathBuf {
    // pid + nanos keeps parallel test runs from colliding; both are
    // available without pulling in `rand` or `uuid`.
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

#[test]
fn create_writes_header_line_to_a_fresh_file() {
    let path = unique_path("create-fresh");
    let _ = fs::remove_file(&path); // ensure no leftover from a prior run

    store::create(&path).expect("create must succeed on a new path");

    let content = fs::read_to_string(&path).expect("file must exist after create");
    // Header is the only line; the journal starts empty.
    assert_eq!(content.lines().count(), 1, "got: {content:?}");
    assert_eq!(
        content.trim_end(),
        store::HEADER_LINE,
        "the on-disk header must match the SRD contract"
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn create_refuses_to_overwrite_an_existing_file() {
    let path = unique_path("create-exists");
    let _ = fs::remove_file(&path);

    store::create(&path).expect("first create must succeed");

    // Pre-populate with a sentinel byte; if create silently truncates,
    // we'd lose this and the test would fail.
    fs::write(&path, "PRE-EXISTING-DATA\n").unwrap();

    let err = store::create(&path).expect_err("second create must error");
    assert!(
        !err.is_empty(),
        "the error must carry a one-line reason operators can grep; got empty"
    );

    // Sentinel must survive — proving we did not silently overwrite.
    let content = fs::read_to_string(&path).unwrap();
    assert!(
        content.starts_with("PRE-EXISTING-DATA"),
        "existing data must not be clobbered; got: {content:?}"
    );

    let _ = fs::remove_file(&path);
}