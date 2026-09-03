//! `libbiz::bom` -- bill of materials.
//!
//! WHAT:    A TSV reader per item, and a `walk` that returns
//!          every BOM-bearing item path.
//! WHY:     The MRP driver (Phase 6) reads the BOM tree to
//!          compute the components needed for open SOs.
//! LAYER:   Entity. Pure: same path, same result.
//! DEPENDS: stdlib only.
//! USED BY: `bin/bom.rs` (the driver; new/ls/show) and
//!          `bin/mrp.rs` (which reads the BOM tree).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Walk the BOMs directory tree under `root` and return every
/// leaf item path (a leaf is a directory containing `bom.tsv`),
/// sorted.
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
        if path.join("bom.tsv").is_file() {
            let rel = path.strip_prefix(prefix)
                .map_err(|e| format!("strip_prefix {} -> {}: {e}", path.display(), prefix.display()))?
                .to_path_buf();
            out.push(rel);
        } else {
            walk_into(prefix, &path, out)?;
        }
    }
    Ok(())
}

/// A single line of a BOM: an item needs `qty_per_unit` of
/// `component` per unit of itself, measured in `uom`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BomLine {
    pub item: String,
    pub component: String,
    pub qty_per_unit: i64,
    pub uom: String,
}

/// Read a `bom.tsv` file and return its lines.
pub fn read_bom(path: &Path) -> Result<Vec<BomLine>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("read bom {}: {e}", path.display()))?;
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let _header = lines.next().ok_or_else(|| "empty bom.tsv".to_string())?;
    let mut out: Vec<BomLine> = Vec::new();
    for line in lines {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 4 {
            return Err(format!("bom.tsv: expected 4 columns, got {}", cols.len()));
        }
        let qty: i64 = cols[2].parse()
            .map_err(|e| format!("bom.tsv: qty_per_unit not an integer: {e}"))?;
        out.push(BomLine {
            item: cols[0].to_string(),
            component: cols[1].to_string(),
            qty_per_unit: qty,
            uom: cols[3].to_string(),
        });
    }
    Ok(out)
}

/// Suppress unused warning for BTreeSet; reserved for future
/// batched BOM readers.
#[allow(dead_code)]
fn _unused() -> BTreeSet<String> {
    BTreeSet::new()
}
