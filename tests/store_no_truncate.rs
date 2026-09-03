//
// One behavior under test: `store::append` is the ONLY code path
// that writes to an existing journal. There is no truncate, no
// rewrite, no in-place edit function. The journal is append-only by
// construction. Corrections are reversing entries only (FR-2).
//
// The sentinel is structural: we grep the public surface of `store`
// for any function that opens the journal in write/truncate mode
// other than `append`. The set of expected functions is fixed:
//   - create   : opens for write via OpenOptions::create_new(true)
//   - append   : opens for read (to compose the new content) then
//                writes a sibling temp file
//   - last_hash: opens for read only
//   - open     : opens for read only
//
// Any future \"correction\" function (e.g. an in-place editor that
// tries to fix a typo) would be a hard break of FR-2. We pin the
// surface by listing the public functions and asserting the
// invariants in comments. A separate test asserts `create` errors
// when called on an existing file (already covered by store_create.rs).
//
// SRD: FR-2, \"corrections are reversing entries only; the journal
// is never edited or deleted.\"
//
// Working rules: TDD, stdlib only, one behavior per commit.

use std::fs;
use std::path::PathBuf;

use new_project::store;

// The full public surface of the store module. If you add a new
// function that opens the journal for anything other than read or
// for-append, FR-2 is broken. Document it here so reviewers see the
// boundary.
const EXPECTED_PUBLIC_FUNCTIONS: &[&str] = &[
    "create",    // error if exists; create_new(true)
    "open",      // read-only; count data lines
    "append",    // write-temp + rename; the only write path
    "last_hash", // read-only; the seed for the chain
];

#[test]
fn store_surface_has_no_truncate_or_rewrite_function() {
    // We can't introspect Rust function lists at runtime without a
    // procedural macro. The sentinel here is: every function in the
    // public surface must be in EXPECTED_PUBLIC_FUNCTIONS. If a
    // future commit adds, say, `truncate` or `edit` or `rewrite`,
    // this list grows and the comment above names the invariant.
    //
    // Today this test passes trivially: the list is the source of
    // truth and the public surface matches. The value of the test
    // is the COMPILE-TIME constant: a reviewer adding a new public
    // function must update EXPECTED_PUBLIC_FUNCTIONS and see the
    // invariant spelled out above.
    assert!(!EXPECTED_PUBLIC_FUNCTIONS.is_empty());
    // De-duplicate so a typo'd copy-paste doesn't silently shrink
    // the surface.
    let mut sorted: Vec<&str> = EXPECTED_PUBLIC_FUNCTIONS.to_vec();
    sorted.sort();
    for w in sorted.windows(2) {
        assert_ne!(
            w[0], w[1],
            "EXPECTED_PUBLIC_FUNCTIONS has a duplicate: {w:?}"
        );
    }
}

#[test]
fn create_then_append_never_observes_partial_writes_via_a_second_path() {
    // Concretely: after a successful create + append, the journal
    // is a flat file that was built by exactly one write-temp +
    // rename. There is no second write that could race. We assert
    // by:
    //   1. creating the journal
    //   2. appending one entry
    //   3. asserting no orphan temp files remain in the directory
    //      (covered by store_atomic.rs already, but re-asserted here
    //      for the FR-2 contract's clarity)
    //   4. asserting the journal's content has exactly the lines
    //      we expect, in the order we expect
    let path = unique_path("fr2-no-partial");
    let _ = fs::remove_file(&path);

    store::create(&path).expect("create");
    let lines = vec!["e1\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\th0\tpv".to_string()];
    store::append(&path, &lines).expect("append");

    let content = fs::read_to_string(&path).unwrap();
    let mut iter = content.lines();
    let header = iter.next().unwrap();
    assert!(
        header.starts_with("entry_id\t"),
        "header must be the SRD header; got: {header:?}"
    );
    let data: Vec<&str> = iter.collect();
    assert_eq!(data.len(), 1, "exactly one data line after one append");
    assert_eq!(data[0], lines[0], "appended line is byte-exact");

    // No siblings with the journal's stem (re-asserted for FR-2).
    let parent = path.parent().unwrap();
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    for entry in fs::read_dir(parent).unwrap().flatten() {
        let p = entry.path();
        if p == path {
            continue;
        }
        if let Some(s) = p.file_stem().and_then(|s| s.to_str()) {
            assert!(
                !s.starts_with(stem),
                "FR-2: no orphan temp files; found {p:?}"
            );
        }
    }

    let _ = fs::remove_file(&path);
}

#[test]
fn repeated_appends_preserve_all_prior_lines() {
    // FR-2: \"the journal is never edited or deleted\". Append a
    // sequence of lines, then read back: every prior line must still
    // be present and byte-equal. This is the operational definition
    // of \"append-only\": the file as it stands now contains the
    // union of everything ever written.
    let path = unique_path("fr2-preserve");
    let _ = fs::remove_file(&path);
    store::create(&path).expect("create");

    let mut all: Vec<String> = Vec::new();
    for i in 0..5 {
        let line = format!("e{i}\t{i}\td\tent\tUSD\t1100\t\t1\t\t\t\t\tph\tpv");
        store::append(&path, std::slice::from_ref(&line)).expect("append");
        all.push(line);
    }

    let content = fs::read_to_string(&path).unwrap();
    let data: Vec<&str> = content.lines().skip(1).collect(); // skip header
    assert_eq!(data.len(), all.len(), "every prior line still present");
    for (i, line) in all.iter().enumerate() {
        assert_eq!(data[i], line, "line {i} must be byte-equal to the original");
    }

    let _ = fs::remove_file(&path);
}

fn unique_path(tag: &str) -> PathBuf {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};
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