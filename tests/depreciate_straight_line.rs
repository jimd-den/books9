//
// tests/depreciate_straight_line.rs
//
//! Pin the contract for `depreciate::straight_line`:
//! the monthly depreciation amount is (cost - salvage)
//! / useful_life_months. Pure: same inputs, same output.

#[test]
fn straight_line_depreciation_basic() {
    // 5,000,000 cost, 500,000 salvage, 60 months life.
    // Monthly = (5,000,000 - 500,000) / 60 = 75,000.
    let amount = new_project::depreciate::straight_line(5_000_000, 500_000, 60);
    assert_eq!(amount, 75_000);
}

#[test]
fn straight_line_with_zero_life_returns_zero() {
    // Edge case: useful_life_months = 0 -> division by zero.
    // The contract: return 0 (don't panic; let the validator
    // flag the asset as misconfigured).
    let amount = new_project::depreciate::straight_line(1_000_000, 0, 0);
    assert_eq!(amount, 0);
}

#[test]
fn straight_line_with_zero_salvage() {
    // No salvage value: full cost depreciated over life.
    let amount = new_project::depreciate::straight_line(1_200_000, 0, 12);
    assert_eq!(amount, 100_000);
}
