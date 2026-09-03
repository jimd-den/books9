//! `libbiz` -- the BOOKS/9 kernel-in-userland.
//!
//! WHAT:    The crate root. Holds the layer map: money, time, journal,
//!          store, chain. Each lives in its own module with its own doc.
//! WHY:     This is the smallest thing every other crate in BOOKS/9
//!          depends on. If a behavior lives in BOOKS/9 at all, it is
//!          because something in here made it possible.
//! LAYER:   Crate root -- not an entity, use case, adapter, or driver.
//!          Each submodule declares its own layer.
//! DEPENDS: stdlib only. No third-party crates. See Cargo.toml.
//! USED BY: Every `src/bin/*.rs` driver via the `new_project::` path.
pub mod chain;
pub mod coa;
pub mod reports;
pub mod fx;
pub mod party;
pub mod item;
pub mod mrp;
pub mod payroll;
pub mod org;
pub mod bom;
pub mod inspect;
pub mod maint;
pub mod depreciate;
pub mod asset;
pub mod money;
pub mod journal;
pub mod store;
pub mod time;