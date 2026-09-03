//! `libbiz::mrp` -- material requirements planning.
//!
//! WHAT:    A pure compute function: given a list of demand
//!          (SOs with priced lines) and a BOM tree, return a
//!          map of component -> total qty (in the component's
//!          unit).
//! WHY:     SRD FR-3: same inputs, same output bytes, run
//!          twice. The compute is pure; the driver writes the
//!          result to stdout in a stable, sorted shape.
//! LAYER:   Entity. Pure: same inputs, same result.
//! DEPENDS: `libbiz::bom` (read_bom), stdlib.
//! USED BY: `bin/mrp.rs` (the driver; the byte-stable
//!          golden-file test in `tests/mrp_byte_stable.rs`).

use std::collections::BTreeMap;
use std::path::Path;

/// One demand line: an SO with a priced item line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemandLine {
    pub item: String,
    pub qty: i64,
}

/// One output row: a component and the total qty needed.
pub type MrpOutput = BTreeMap<String, (i64, String)>; // component -> (qty, uom)

/// Compute the components needed across all demand lines.
///
/// WHAT:    For each (item, qty) in `demand`, read the item's
///          BOM at `bom_root/{item}/bom.tsv` and add each
///          component `qty * qty_per_unit` to the running total.
///          Same component across different items is summed.
/// WHY:     The shop floor's "what do I need?" question.
/// LAYER:   Entity.
/// DEPENDS: `libbiz::bom` (read_bom), stdlib.
pub fn compute(
    demand: &[DemandLine],
    bom_root: &Path,
) -> Result<MrpOutput, String> {
    let mut totals: MrpOutput = BTreeMap::new();
    for line in demand {
        let bom_path = bom_root.join(&line.item).join("bom.tsv");
        if !bom_path.exists() {
            // No BOM for this item: skip (the item is a
            // finished good or a phantom; the caller decides
            // what to do with phantoms).
            continue;
        }
        let bom_lines = crate::bom::read_bom(&bom_path)?;
        for bl in bom_lines {
            // Skip lines that don't belong to this item.
            if bl.item != line.item {
                continue;
            }
            let needed = line.qty.checked_mul(bl.qty_per_unit)
                .ok_or_else(|| format!("mrp: overflow on {}*{}",
                    line.qty, bl.qty_per_unit))?;
            let entry = totals.entry(bl.component.clone())
                .or_insert((0, bl.uom.clone()));
            entry.0 += needed;
        }
    }
    Ok(totals)
}
