//! `libbiz::asset` -- fixed-asset register.
//!
//! WHAT:    A directory tree under /biz/assets/{id}/ where a
//!          leaf is a directory containing profile.tsv. The
//!          profile has 6 fields: id, name, cost_minor,
//!          acquired, useful_life_months, salvage_minor.
//! WHY:     Phase 7 ships the asset register; depreciation
//!          reads the cost / life / salvage to compute the
//!          monthly amount.
//! LAYER:   Entity. Pure: same path, same result.
//! DEPENDS: stdlib only.
//! USED BY: `bin/asset.rs` (the driver; new/ls) and
//!          `bin/depreciate.rs` (which reads the profile).

use std::path::Path;
use std::path::PathBuf;

/// Walk the assets directory tree under `root` and return every
/// leaf asset path, sorted.
pub fn walk(root: &Path) -> Result<Vec<PathBuf>, String> {
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

/// A single asset's typed profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub cost_minor: i64,
    pub acquired: String,
    pub useful_life_months: i64,
    pub salvage_minor: i64,
}

/// Read a `profile.tsv` file and return a typed `Profile`.
pub fn read_profile(path: &Path) -> Result<Profile, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("read asset profile {}: {e}", path.display()))?;
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let _header = lines.next().ok_or_else(|| "empty profile".to_string())?;
    let data = lines.next().ok_or_else(|| "profile has no data row".to_string())?;
    let cols: Vec<&str> = data.split('\t').collect();
    if cols.len() != 6 {
        return Err(format!("asset profile: expected 6 columns, got {}", cols.len()));
    }
    let cost_minor: i64 = cols[2].parse()
        .map_err(|e| format!("cost_minor not an integer: {e}"))?;
    let useful_life_months: i64 = cols[4].parse()
        .map_err(|e| format!("useful_life_months not an integer: {e}"))?;
    let salvage_minor: i64 = cols[5].parse()
        .map_err(|e| format!("salvage_minor not an integer: {e}"))?;
    Ok(Profile {
        id: cols[0].to_string(),
        name: cols[1].to_string(),
        cost_minor,
        acquired: cols[3].to_string(),
        useful_life_months,
        salvage_minor,
    })
}
