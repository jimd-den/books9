//
//! `libbiz::item` -- master data for SKUs.
//!
//! WHAT:    A TSV reader (`profile_tsv`), a `Profile` struct,
//!          and a `walk` that returns every leaf item path.
//! WHY:     The O2C path needs registered SKUs before it can
//!          build a sales order. `item` is the master-data
//!          entry point for inventory.
//! LAYER:   Entity. Pure: same path, same result.
//! DEPENDS: stdlib only.
//! USED BY: `bin/item.rs` (the driver; new/ls/show),
//!          `bin/so.rs` (the SO line's sku field), and
//!          `bin/price.rs` (which looks up the default price).

use std::collections::BTreeMap;
use std::path::Path;

/// A single SKU's typed profile, as read from `profile.tsv`.
///
/// WHAT:    A struct with four required string fields.
///          `default_price` is parsed to i64 (minor units per
///          unit; the price table can override it).
/// WHY:     Reports and the price tool need a typed view of
///          an item; the raw TSV is the storage.
/// LAYER:   Entity. Plain data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub uom: String,
    pub default_price: i64,
}

/// Walk the items directory tree under `root` and return every
/// leaf item path, sorted.
pub fn walk(root: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    // Same shape as coa::walk and party::walk. Re-exporting
    // coa::walk keeps the three reads in lockstep.
    crate::coa::walk(root)
}

/// Read a `profile.tsv` file and return a typed `Profile`.
///
/// WHAT:    A pure reader: same file, same Profile.
/// WHY:     The item driver and the price tool need a typed
///          view; the raw TSV is the storage.
pub fn profile_tsv(path: &Path) -> Result<Profile, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("read item profile {}: {e}", path.display()))?;
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let _header = lines.next().ok_or_else(|| "empty profile.tsv".to_string())?;
    let data = lines.next().ok_or_else(|| "profile has no data row".to_string())?;
    if lines.next().is_some() {
        return Err("profile has more than one data row".to_string());
    }
    let cols: Vec<&str> = data.split('\t').collect();
    if cols.len() != 4 {
        return Err(format!("profile.tsv: expected 4 columns, got {}", cols.len()));
    }
    let id = cols[0].to_string();
    let name = cols[1].to_string();
    let uom = cols[2].to_string();
    let default_price: i64 = cols[3].parse()
        .map_err(|e| format!("default_price not an integer: {e}"))?;
    for (label, value) in [("id", &id), ("name", &name), ("uom", &uom)] {
        if value.is_empty() {
            return Err(format!("profile.tsv: required field {label} is empty"));
        }
    }
    Ok(Profile { id, name, uom, default_price })
}

/// Suppress unused warning for BTreeMap; reserved for future
/// batched profile readers.
#[allow(dead_code)]
fn _unused() -> BTreeMap<String, Profile> {
    BTreeMap::new()
}
