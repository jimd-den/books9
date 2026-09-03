//
// tests/fx_converts_amounts.rs
//
//! Pin the contract for `fx::convert` -- the multiply-and-divide
//! that turns an amount in `from` minor units into an amount in
//! `to` minor units given a rate.
//!
//! Math: `to_amount = from_amount * rate / 10^8`. The
//! multiplication is checked for overflow; the division is
//! integer (truncates toward zero).



#[test]
fn convert_applies_a_rate_to_an_amount() {
    // 100 EUR * 1.10 (rate 110_000_000) / 10^8 = 110 USD.
    // Both EUR and USD are 2-decimal minor units, so 100 EUR minor
    // is 100 (1.00 EUR), and 110 USD minor is 110 (1.10 USD).
    // The rate 1.10 = 110_000_000 in 10^-8 of `to` per `from`.
    // So: 100 * 110_000_000 / 100_000_000 = 110. Exact.
    let got = new_project::fx::convert(100, 110_000_000);
    assert_eq!(got, 110);
}

#[test]
fn convert_handles_a_rate_with_fewer_than_eight_decimal_places() {
    // 0.91 -> 91_000_000. 200 * 91_000_000 / 100_000_000 = 182.
    let got = new_project::fx::convert(200, 91_000_000);
    assert_eq!(got, 182);
}

#[test]
fn convert_truncates_on_non_exact_division() {
    // 1 * 1.10 = 1.10 truncated to 1. The audit-friendly
    // choice: a penny is not invented; the report's column
    // total is the sum of the per-row truncated amounts.
    let got = new_project::fx::convert(1, 110_000_000);
    assert_eq!(got, 1, "truncation, not rounding (1.10 -> 1)");

    // 3 * 1.10 = 3.30 exact; passes through unchanged.
    let got = new_project::fx::convert(3, 110_000_000);
    assert_eq!(got, 3, "exact case passes through");

    // 4 * 1.10 = 4.40 truncated to 4.
    let got = new_project::fx::convert(4, 110_000_000);
    assert_eq!(got, 4, "truncation (4.40 -> 4)");
}

#[test]
fn convert_panics_on_overflow_instead_of_wrapping() {
    // i64::MAX * 110_000_000 overflows. The audit-friendly choice
    // is to panic with a labeled message, same as `money::add`.
    let result = std::panic::catch_unwind(|| {
        new_project::fx::convert(i64::MAX, 110_000_000)
    });
    assert!(result.is_err(), "overflow must panic, not return a wrapped value");
}
