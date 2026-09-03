//
// One behavior under test: `store::periods_root(journal_path)` derives
// the sibling `periods/` directory that lives next to the journal file.
//
// The SRD's filesystem contract places `/biz/ledger/periods/` as a
// sibling of `/biz/ledger/journal`. Given a journal at
// `/biz/ledger/journal`, the periods root is `/biz/ledger/periods`.
//
// This is pure path arithmetic: no I/O, no filesystem check. The
// caller (post, close, future tools) decides whether the directory
// exists or needs to be created.
//
// Working rules: TDD, stdlib only, one behavior per commit.

use std::path::Path;

#[test]
fn periods_root_is_the_sibling_periods_directory_of_the_journal() {
    let journal = Path::new("/biz/ledger/journal");
    let root = new_project::store::periods_root(journal);
    assert_eq!(root, Path::new("/biz/ledger/periods"));
}

#[test]
fn periods_root_handles_a_relative_journal_path() {
    // The function is purely about path arithmetic; relative inputs
    // are accepted and produce a relative sibling.
    let journal = Path::new("var/journal.tsv");
    let root = new_project::store::periods_root(journal);
    assert_eq!(root, Path::new("var/periods"));
}

#[test]
fn periods_root_is_a_pure_function_no_io() {
    // The function MUST NOT touch the filesystem. We point it at a
    // path whose directory is absent and assert it returns cleanly.
    let journal = Path::new("/this/path/definitely/does/not/exist/journal");
    let root = new_project::store::periods_root(journal);
    assert_eq!(
        root,
        Path::new("/this/path/definitely/does/not/exist/periods")
    );
}