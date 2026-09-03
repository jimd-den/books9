//! `libbiz::time` -- the wall-clock seam and a pure UTC formatter.
//!
//! WHAT:    `format_utc(epoch_secs: i64) -> String` renders Unix
//!          epoch seconds as `YYYY-MM-DDTHH:MM:SSZ`, fixed width 20
//!          characters, no fractional part. The wall clock is read
//!          in a tool's `main()` once and handed in as a plain
//!          integer; this module never reads it.
//! WHY:     SRD's determinism pillar says reports must not read
//!          clocks. The one exception is recording WHEN a mutation
//!          happened (a close stamp). The clock read lives in a
//!          driver's main and never leaks into the use case layer.
//! LAYER:   Entity. Pure: same seconds in, same string out, always.
//! DEPENDS: stdlib. Algorithm: Howard Hinnant's civil_from_days
//!          shifted to Unix epoch; no tables, no deps.
//! USED BY: `close` (records the close stamp); future tools that
//!          need a UTC stamp (Phase 3 reports, Phase 5 invoices).
pub fn format_utc(epoch_secs: i64) -> String {
    let days = epoch_secs.div_euclid(86_400);
    let secs_of_day = epoch_secs.rem_euclid(86_400);

    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
    );

    // A single allocation is overkill for a 20-byte stamp, but this
    // runs once per close; the readable format! beats hand-rolled
    // digit math. Determinism is untouched either way.
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Civil date from days since 1970-01-01. Howard Hinnant's
/// `civil_from_days`, verbatim in spirit: move the epoch from
/// 0000-03-01 (where the arithmetic is clean) to 1970-01-01.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe =
        (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11], March-based
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}


/// Extract the leading "YYYY-MM" from a date string.
///
/// WHAT:    Pure parser: returns `Some("YYYY-MM")` iff the input
///          starts with a 4-digit year, a separator (either `-` or
///          `/`), and a 2-digit month. Otherwise `None`.
/// WHY:     SRD: the period gate is by `YYYY-MM`, not by full date.
///          Two drivers need this; the helper lives here so the
///          drivers do not drift apart.
/// LAYER:   Entity. Pure, no I/O, no clocks.
/// DEPENDS: stdlib only.
/// USED BY: `post` (periods gate), `close` (periods_root derivation,
///          per-period totals), and any future driver that needs
///          the period a date belongs to.
///
/// Accepts loose date shapes so callers do not have to normalize:
///   * "YYYY-MM-DD"  -> Some("YYYY-MM")
///   * "YYYY-MM"     -> Some("YYYY-MM")
///   * "YYYY/MM/DD"  -> Some("YYYY-MM")
///
/// Rejects anything shorter than 7 bytes, with a non-digit year
/// or month, or with a separator that is not `-` or `/`.
pub fn yyyy_mm(date: &str) -> Option<String> {
    let bytes = date.as_bytes();
    if bytes.len() < 7 {
        return None;
    }
    if !bytes[..4].iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if bytes[4] != b'-' && bytes[4] != b'/' {
        return None;
    }
    if !bytes[5..7].iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut out = String::with_capacity(7);
    out.push_str(std::str::from_utf8(&bytes[..4]).unwrap());
    out.push('-');
    out.push_str(std::str::from_utf8(&bytes[5..7]).unwrap());
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-computed anchors. Each of these was derived by counting
    /// days on a calendar, not by calling any library: if the
    /// algorithm drifts a leap day, one of these rows catches it.
    const ANCHORS: &[(i64, &str)] = &[
        (0, "1970-01-01T00:00:00Z"),
        // 2000 was a leap year (divisible by 400): 10957 days in.
        (1_095_7 * 86_400, "2000-01-01T00:00:00Z"),
        // 1900 would not have been (x - 146096): 25567 days before
        // the epoch, so 1900-01-01 is 25567 days before 1970-01-01.
        (-25_567 * 86_400, "1900-01-01T00:00:00Z"),
        // 2024-02-29 exists: 1970..2023 = 54 years, 13 leap days,
        // = 19723 days to 2024-01-01; +59 days lands on Feb 29.
        ((19_723 + 59) * 86_400, "2024-02-29T00:00:00Z"),
        ((19_723 + 60) * 86_400, "2024-03-01T00:00:00Z"),
        // 2100 will NOT be a leap year: 2100-02-28 is followed by
        // 2100-03-01. Days 1970->2000 = 10957; 2000->2100 = 36525
        // (25 leap days, 2000..2096). 2100-01-01 is day 47482.
        ((47_482 + 58) * 86_400, "2100-02-28T00:00:00Z"),
        ((47_482 + 59) * 86_400, "2100-03-01T00:00:00Z"),
        // A mid-day timestamp with all six fields exercised.
        // 2026-09-01 is day 20697 (1788220800 at midnight); +3h25m24s.
        (1_788_233_124, "2026-09-01T03:25:24Z"),
    ];

    #[test]
    fn format_utc_matches_hand_computed_anchors() {
        for (secs, want) in ANCHORS {
            assert_eq!(&format_utc(*secs), want, "for secs={secs}");
        }
    }

    #[test]
    fn format_utc_is_pure_and_fixed_width() {
        // Same input, same output: the determinism pillar, tested.
        let a = format_utc(1_788_264_879);
        let b = format_utc(1_788_264_879);
        assert_eq!(a, b);
        assert_eq!(a.len(), 20);
        assert!(a.ends_with('Z'));
    }
    /// `yyyy_mm` extracts the leading "YYYY-MM" from a date string.
    /// The function is the single shared primitive that `post` and
    /// `close` both call; pinning its contract here keeps the two
    /// drivers from drifting. Accepts "YYYY-MM-DD", "YYYY-MM",
    /// "YYYY/MM/DD"; rejects anything shorter, non-digit, or with
    /// a separator that is not '-' or '/'.
    #[test]
    fn yyyy_mm_extracts_year_and_month() {
        assert_eq!(yyyy_mm("2026-09-01"), Some("2026-09".to_string()));
        assert_eq!(yyyy_mm("2026-09"), Some("2026-09".to_string()));
        assert_eq!(yyyy_mm("2026/09/01"), Some("2026-09".to_string()));
    }

    #[test]
    fn yyyy_mm_rejects_too_short() {
        assert_eq!(yyyy_mm(""), None);
        assert_eq!(yyyy_mm("2026"), None);
        assert_eq!(yyyy_mm("2026-"), None);
    }

    #[test]
    fn yyyy_mm_rejects_non_digit_year_or_month() {
        assert_eq!(yyyy_mm("abcd-09"), None);
        assert_eq!(yyyy_mm("2026-ab"), None);
    }

    #[test]
    fn yyyy_mm_rejects_unknown_separator() {
        // A separator that is not '-' or '/' means the input is
        // not the loose date shape we accept.
        assert_eq!(yyyy_mm("2026.09.01"), None);
    }

}