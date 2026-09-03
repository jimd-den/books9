//
// One behavior under test: `store::open(path)` reads an existing
// journal and returns the count of *data* lines (header excluded).
// A missing file errors; the header line is not counted.
//
// SRD: Phase 1 (\"create-or-open an empty journal at PATH with the
// column header\"). Counting data lines is the cheap way to know
// how many entries the journal holds without re-parsing the chain.

use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use new_project::store;

fn unique_path(tag: &str) -> PathBuf {
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
fn open_returns_zero_data_lines_for_a_fresh_journal() {
    let path = unique_path("open-fresh");
    let _ = fs::remove_file(&path);

    store::create(&path).expect("create must succeed");
    let n = store::open(&path).expect("open must succeed on a created file");
    assert_eq!(n, 0, "a fresh journal holds zero data lines");

    let _ = fs::remove_file(&path);
}

#[test]
fn open_counts_data_lines_and_excludes_the_header() {
    let path = unique_path("open-count");
    let _ = fs::remove_file(&path);

    store::create(&path).expect("create must succeed");

    // Append three data rows manually (without going through `post`).
    // Hash and prev_hash are placeholder zeros; `open` must not
    // validate them — it just counts lines.
    let row = "e1\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\t0000000000000000\t0000000000000000\n";
    let mut appended = String::new();
    for _ in 0..3 {
        appended.push_str(row);
    }
    use std::io::Write;
    let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
    f.write_all(appended.as_bytes()).unwrap();

    let n = store::open(&path).expect("open must succeed");
    assert_eq!(n, 3, "header excluded, three data lines");

    let _ = fs::remove_file(&path);
}

#[test]
fn open_errors_on_a_missing_file() {
    let path = unique_path("open-missing");
    let _ = fs::remove_file(&path); // ensure absence
    assert!(
        !path.exists(),
        "precondition: path must not exist ({})",
        path.display()
    );

    let err = store::open(&path).expect_err("open must error on a missing file");
    assert!(
        !err.is_empty(),
        "the error must carry a one-line reason; got empty"
    );
}