//
// libbiz::coa -- the chart of accounts as a directory tree.
//
// Phase 3 lands the full surface. This commit ships a stub so the
// pinned tests in `tests/coa_walks_directory_tree.rs` can compile
// and FAIL at runtime; the next commit replaces the body with the
// real `walk` implementation. Per the TDD kickoff in
// `prompts/CONFIG.md` §3.1, "RED" means a real test failure with
// the right reason -- not a compile error.

use std::path::{Path, PathBuf};

/// Walk the CoA directory tree under `root` and return every leaf
/// account path, sorted.
///
/// WHAT:    A leaf account is a directory containing `profile.tsv`.
///          Groups (directories without `profile.tsv`) are skipped.
///          The result is sorted by path so callers get a stable
///          order for `trial` and `coa ls`.
/// WHY:     The CoA IS the tree; the only way to enumerate accounts
///          is to walk it. This is the single source of truth for
///          "which accounts exist?" and is shared by `coa ls`,
///          `post --coa`, `coa new`, and the future report tools.
/// LAYER:   Entity. Pure: same root, same result. The implementation
///          uses only `std::fs::read_dir`; no clock, no env, no
///          state.
/// DEPENDS: stdlib only.
/// USED BY: `post --coa PATH` (Phase 3 migration: PATH is now a
///          directory), `bin/coa.rs` (new tool: list/show/new),
///          future report tools that need the set of accounts.
///
/// Stub behavior: returns `unimplemented!()`. Replaced in the next
/// commit. The pinned tests in `tests/coa_walks_directory_tree.rs`
/// exercise the real contract; the stub makes them compile and
/// fail with a clear panic rather than a compile error.
/// Walk the CoA directory tree under `root` and return every leaf
/// account path, sorted.
///
/// WHAT:    A leaf account is a directory containing `profile.tsv`.
///          Groups (directories without `profile.tsv`) are skipped.
///          The result is sorted by path so callers get a stable
///          order for `trial` and `coa ls`.
/// WHY:     The CoA IS the tree; the only way to enumerate accounts
///          is to walk it. This is the single source of truth for
///          "which accounts exist?" and is shared by `coa ls`,
///          `post --coa`, `coa new`, and the future report tools.
/// LAYER:   Entity. Pure: same root, same result. The implementation
///          uses only `std::fs::read_dir`; no clock, no env, no
///          state.
/// DEPENDS: stdlib only.
/// USED BY: `post --coa PATH` (Phase 3 migration: PATH is now a
///          directory), `bin/coa.rs` (new tool: list/show/new),
///          future report tools that need the set of accounts.
///
/// Returns: `Ok(Vec<PathBuf>)` of account paths relative to
/// `root`, sorted. `Err(String)` on any I/O failure that prevents
/// a complete enumeration (the first failure stops the walk; the
/// caller has the partial result via the Err).
pub fn walk(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out: Vec<PathBuf> = Vec::new();
    walk_into(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

/// Recursive worker for `walk`. `prefix` is the original root; we
/// track depth from it so we never recurse past `root` (defense
/// against symlink loops). The account path stored in `out` is
/// the path relative to `prefix`.
fn walk_into(
    prefix: &Path,
    dir: &Path,
    out: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("read_dir {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read_dir entry: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue; // a stray file is not an account
        }
        if path.join("profile.tsv").is_file() {
            // Leaf account: store the path relative to the root.
            let rel = path
                .strip_prefix(prefix)
                .map_err(|e| format!("strip_prefix {} -> {}: {e}", path.display(), prefix.display()))?
                .to_path_buf();
            out.push(rel);
        } else {
            // Group: recurse into it. The group is not a leaf; do
            // not record it.
            walk_into(prefix, &path, out)?;
        }
    }
    Ok(())
}


/// A single account's typed profile, as read from `profile.tsv`.
///
/// WHAT:    A struct with six required string fields. Empty
///          strings are not allowed for `code`, `name`, `kind`,
///          `normal_side`, or `status`; `parent` may be empty
///          (top-level accounts have no parent).
/// WHY:     Reports and `coa show` need a typed view of an
///          account; the raw TSV is the storage, the struct is
///          the use case shape.
/// LAYER:   Entity. Plain data; no methods other than the
///          constructor (the reader) and Display (for the
///          future `coa show` driver).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub code: String,
    pub name: String,
    pub kind: String,
    pub normal_side: String,
    pub parent: String,
    pub status: String,
}

/// Read a `profile.tsv` file and return a typed `Profile`.
///
/// WHAT:    A pure reader: same file, same Profile. The file is
///          a TSV with one header row and one data row.
/// WHY:     Reports and `coa show` need a typed view; the raw
///          TSV is the storage.
/// LAYER:   Entity. Pure: same path, same result.
/// DEPENDS: stdlib only.
/// USED BY: `coa::walk`'s callers (e.g. `coa ls`, `post --coa`)
///          that want the typed view, not just the path.
///
/// Returns: `Ok(Profile)` on a well-formed file, `Err(String)`
/// with a one-line reason on a malformed file or a missing
/// required field.
pub fn profile_tsv(path: &Path) -> Result<Profile, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("read profile {}: {e}", path.display()))?;
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let _header = lines.next().ok_or_else(|| "empty profile.tsv".to_string())?;
    let data = lines.next().ok_or_else(|| "profile has no data row".to_string())?;
    if lines.next().is_some() {
        return Err("profile has more than one data row".to_string());
    }
    let cols: Vec<&str> = data.split('\t').collect();
    if cols.len() != 6 {
        return Err(format!("profile.tsv: expected 6 columns, got {}", cols.len()));
    }
    let code = cols[0].to_string();
    let name = cols[1].to_string();
    let kind = cols[2].to_string();
    let normal_side = cols[3].to_string();
    let parent = cols[4].to_string();
    let status = cols[5].to_string();
    // Required fields: code, name, kind, normal_side, status.
    // parent is allowed to be empty (top-level account).
    for (label, value) in [
        ("code", &code),
        ("name", &name),
        ("kind", &kind),
        ("normal_side", &normal_side),
        ("status", &status),
    ] {
        if value.is_empty() {
            return Err(format!("profile.tsv: required field {label} is empty"));
        }
    }
    Ok(Profile { code, name, kind, normal_side, parent, status })
}
