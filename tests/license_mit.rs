//
// tests/license_mit.rs
//
//! Pin the project's license: a `LICENSE` file at the repo
//! root, holding the canonical MIT license text. This is
//! the green test the project is currently missing; the
//! next commit writes the file.

use std::fs;
use std::path::Path;

const EXPECTED_LICENSE: &str = include_str!("../LICENSE");

#[test]
fn license_file_exists_at_repo_root() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let license = manifest_dir.join("LICENSE");
    assert!(
        license.exists(),
        "LICENSE file missing at repo root: {}",
        license.display()
    );
}

#[test]
fn license_is_mit_text() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let license = manifest_dir.join("LICENSE");
    let actual = fs::read_to_string(&license).expect("read LICENSE");
    assert_eq!(
        actual, EXPECTED_LICENSE,
        "LICENSE contents drifted from the canonical MIT text"
    );
}

#[test]
fn license_text_contains_mit_markers() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let license = manifest_dir.join("LICENSE");
    let actual = fs::read_to_string(&license).expect("read LICENSE");
    assert!(actual.contains("MIT License"), "LICENSE missing 'MIT License'");
    assert!(actual.contains("Permission is hereby granted"), "LICENSE missing grant clause");
    assert!(actual.contains("THE SOFTWARE IS PROVIDED"), "LICENSE missing disclaimer");
}
