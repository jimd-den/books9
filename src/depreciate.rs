//! `libbiz::depreciate` -- depreciation math.
//!
//! WHAT:    Pure compute: straight-line depreciation
//!          `(cost - salvage) / useful_life_months` per
//!          month. Same inputs, same output.
//! WHY:     Phase 7 ships the smallest depreciation surface;
//!          a future cycle can add other methods (declining
//!          balance, sum-of-years-digits) as sibling functions.
//! LAYER:   Entity. Pure: same inputs, same result.
//! DEPENDS: stdlib only.
//! USED BY: `bin/depreciate.rs` (the driver; --asset --period).

/// Compute the straight-line monthly depreciation amount
/// in minor units.
///
/// WHAT:    `(cost_minor - salvage_minor) / useful_life_months`
///          using integer division (truncates toward zero).
/// WHY:     The math is the contract. The same inputs produce
///          the same amount, run after run.
/// LAYER:   Entity.
/// DEPENDS: stdlib only.
///
/// Edge cases:
///  - `useful_life_months == 0` -> return 0 (don't panic).
///  - `salvage > cost` -> return 0 (clamped; negative
///    depreciation makes no sense).
///  - overflow: `cost - salvage` is always <= cost (i64),
///    so no checked_sub needed in this direction. The
///    intermediate subtraction is always non-negative.
pub fn straight_line(cost_minor: i64, salvage_minor: i64, useful_life_months: i64) -> i64 {
    if useful_life_months <= 0 {
        return 0;
    }
    if salvage_minor >= cost_minor {
        return 0;
    }
    let depreciable = cost_minor - salvage_minor;
    depreciable / useful_life_months
}
