//
// tests/coa_profile_tsv.rs
//
//! Pin the contract for `coa::profile_tsv` -- the reader that turns
//! a leaf account's `profile.tsv` into a typed `Profile` struct.
//!
//! A profile.tsv is a TSV with a header row and one data row:
//! `code\tname\tkind\tnormal_side\tparent\tstatus`. All six
//! fields are required; any missing column or empty value is a
//! malformed profile. The reader is pure: same file, same result.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_tempdir(label: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("books9-{label}-{pid}-{n}"));
    fs::create_dir_all(&dir).expect("create tempdir");
    dir
}

fn write_profile(dir: &PathBuf, body: &str) -> PathBuf {
    let path = dir.join("profile.tsv");
    fs::write(&path, body).expect("write profile");
    path
}

#[test]
fn profile_tsv_reads_a_well_formed_profile() {
    let tmp = fresh_tempdir("profile-good");
    let path = write_profile(
        &tmp,
        "code\tname\tkind\tnormal_side\tparent\tstatus\n1100\tCash\tasset\tdebit\t\tactive\n",
    );
    let profile = new_project::coa::profile_tsv(&path)
        .expect("well-formed profile must read");
    assert_eq!(profile.code, "1100");
    assert_eq!(profile.name, "Cash");
    assert_eq!(profile.kind, "asset");
    assert_eq!(profile.normal_side, "debit");
    assert_eq!(profile.parent, "");  // top-level account has no parent
    assert_eq!(profile.status, "active");
}

#[test]
fn profile_tsv_rejects_missing_column() {
    // Only 4 columns instead of 6 -- malformed.
    let tmp = fresh_tempdir("profile-short");
    let path = write_profile(
        &tmp,
        "code\tname\tkind\tnormal_side\n1100\tCash\tasset\tdebit\n",
    );
    let err = new_project::coa::profile_tsv(&path)
        .expect_err("missing column must reject");
    assert!(err.contains("expected 6"), "reason names the column count: {err}");
}

#[test]
fn profile_tsv_rejects_empty_required_field() {
    // 6 columns but the `name` is empty.
    let tmp = fresh_tempdir("profile-empty-name");
    let path = write_profile(
        &tmp,
        "code\tname\tkind\tnormal_side\tparent\tstatus\n1100\t\tasset\tdebit\t\tactive\n",
    );
    let err = new_project::coa::profile_tsv(&path)
        .expect_err("empty name must reject");
    assert!(err.contains("name"), "reason names the offending field: {err}");
}
