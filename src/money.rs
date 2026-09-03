//! `libbiz::money` -- i64 minor-unit arithmetic.
//!
//! WHAT:    One pure function: `add(a, b) -> i64`. Panics on overflow
//!          with a labeled message so callers can pattern-match it.
//! WHY:     SRD: "no floats ever cross a tool boundary." Every
//!          posting builder depends on this primitive; a single
//!          silent overflow would corrupt the books.
//! LAYER:   Entity. Pure values in, pure values out, no I/O.
//! DEPENDS: stdlib (`i64::checked_add`).
//! USED BY: `libbiz::journal` (indirectly, via call sites in tools
//!          that build posting lines) and any future money-arithmetic
//!          module that needs addition without silent wraparound.
pub fn add(a: i64, b: i64) -> i64 {
    a.checked_add(b).expect("overflow: libbiz::money::add")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_smoke() {
        assert_eq!(add(40, 60), 100);
    }

    #[test]
    fn add_handles_signed_legs() {
        // -40 (credit) + 40 (debit) must net to zero, not 80.
        assert_eq!(add(-40, 40), 0);
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn add_panics_on_overflow_instead_of_wrapping() {
        let _ = add(i64::MAX, 1);
    }
}