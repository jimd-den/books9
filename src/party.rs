//
// libbiz::party -- master data for customers and vendors.
//
// Phase 5 lands the full surface. The directory-as-existence
// shape is the same as `coa`: a leaf is a directory containing
// profile.tsv. Phase 5 ships a `walk` that returns every
// party path; the rest (profile_tsv reader, party struct,
// party driver) follows in the next commits.
//
// This stub ships so the test file in
// `tests/party_walks_directory_tree.rs` can compile. The
// body lands in the next commit.

use std::path::{Path, PathBuf};

/// Walk the parties directory tree under `root` and return
/// every leaf party path, sorted.
///
/// WHAT:    A leaf is a directory containing `profile.tsv`.
///          Groups (directories without `profile.tsv`) are
///          recursed into, not recorded.
/// WHY:     Master data IS the tree. Same shape as the CoA.
/// LAYER:   Entity. Pure: same root, same result.
/// DEPENDS: stdlib only.
/// USED BY: `bin/party.rs` (the driver; new/ls/show), and
///          `bin/invoice.rs` (which translates an SO into
///          journal lines using the party's terms).
pub fn walk(root: &Path) -> Result<Vec<PathBuf>, String> {
    // The directory-as-existence shape is identical to CoA's.
    // Re-exporting coa::walk keeps the two reads in lockstep:
    // a future change to the walk algorithm (e.g., supporting
    // symlink resolution) lands in one place.
    crate::coa::walk(root)
}
