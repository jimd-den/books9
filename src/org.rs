//! `libbiz::org` -- the organization tree.
//!
//! WHAT:    A directory tree under /biz/org/{code}/ where a
//!          leaf is a directory containing profile.tsv. The
//!          profile has 4 fields: code, name, parent, cost_center.
//! WHY:     The org tree is the seed of the cost-center
//!          hierarchy. Phase 6 ships it so `payroll` can
//!          route wages expense to the right cost-center.
//! LAYER:   Entity. Pure: same path, same result.
//! DEPENDS: stdlib only.
//! USED BY: `bin/org.rs` (the driver; new/ls) and
//!          `bin/payroll.rs` (which reads each employee's
//!          cost_center).

use std::path::Path;
use std::path::PathBuf;

/// Walk the orgs directory tree under `root` and return every
/// leaf department path, sorted.
pub fn walk(root: &Path) -> Result<Vec<PathBuf>, String> {
    // Same shape as coa::walk; separate because the leaf
    // file is profile.tsv, not whatever coa uses.
    let mut out: Vec<PathBuf> = Vec::new();
    walk_into(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk_into(prefix: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("read_dir {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read_dir entry: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join("profile.tsv").is_file() {
            let rel = path.strip_prefix(prefix)
                .map_err(|e| format!("strip_prefix {} -> {}: {e}",
                    path.display(), prefix.display()))?
                .to_path_buf();
            out.push(rel);
        } else {
            walk_into(prefix, &path, out)?;
        }
    }
    Ok(())
}
