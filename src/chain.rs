//! `libbiz::chain` -- the journal hash primitive.
//!
//! WHAT:    `next(prev_hash, row) -> String` produces the next
//!          provenance_hash as 16 lowercase hex chars. `link_rows`
//!          wires a list of raw 13-column rows into a chain.
//! WHY:     Tamper-evidence falls out of the data structure (SRD:
//!          "verify detects any single flipped byte in the journal").
//!          One linker for every writer so the on-disk chain shape
//!          cannot drift between `post` and `reverse`.
//! LAYER:   Interface adapter. Pure, but lives at the adapter layer
//!          because the hash function is a detail the kernel is
//!          allowed to swap without breaking call sites.
//! DEPENDS: `std::hash::DefaultHasher`. Wrapped so a future swap to
//!          a real cryptographic hash (BLAKE3, SHA-256) does not
//!          break call sites.
//! USED BY: `post` (via `link_rows`), `close` (snapshot rows), future
//!          tools that emit hash-chained output.
//!
//! SECURITY NOTE: tamper-evident, NOT cryptographic. DefaultHasher's
//! SipHash variant is not collision-resistant against an attacker.
//! Phase 3+ replaces this with a real hash. Until then the chain
//! catches accidental corruption (flipped bytes, partial writes,
//! mis-ordered rows), which is exactly what the SRD's Phase 1
//! acceptance criterion requires.
use std::hash::{DefaultHasher, Hasher};

/// Compute the next hash in the journal chain.
///
/// `prev_hash` is the provenance_hash of the prior line (16 hex
/// chars; "0000000000000000" for the very first line).
///
/// `row` is the line's bytes WITHOUT the two trailing hash columns
/// (provenance_hash and prev_hash). The contract is documented in
/// the journal.rs header and in tests/store_chain.rs.
///
/// Returns the new provenance_hash as 16 lowercase hex chars. The
/// format is fixed at 16 chars (64 bits) so on-disk rows are
/// predictable-width and tools can `cut -c` without surprises.
pub fn next(prev_hash: &str, row: &[u8]) -> String {
    let mut h = DefaultHasher::new();

    // Feed the prev_hash as bytes. We deliberately include the
    // raw bytes (not the parsed number) so a future swap to a
    // string-hashing hash function does not change behavior.
    h.write(prev_hash.as_bytes());
    h.write(row);
    let digest = h.finish();

    // Format as 16-char lowercase hex. 64 bits is enough for
    // tamper-evidence in the spike; a real hash will widen this.
    format!("{:016x}", digest)
}

/// Wire raw journal rows into a chained block.
///
/// `seed` is the `provenance_hash` the first row must point at (the
/// journal's last hash, or the zero sentinel for a fresh journal).
/// Each `row` is a full 13-column line; columns 11 and 12
/// (provenance_hash, prev_hash) are OVERWRITTEN — whatever the
/// proposer put there is not trusted, because the chain is the
/// kernel's statement, not the caller's.
///
/// This is the only place the row-linking loop lives. `post` and
/// `reverse` both funnel through it so the on-disk chain shape can
/// never drift between writers: one validator, one linker, one
/// truth.
///
/// Pure: same seed, same rows, same output, byte for byte. Returns
/// a one-line reason naming the column count if a row is malformed
/// — the caller has usually validated already, but the linker will
/// not be lied to.
pub fn link_rows(seed: &str, rows: &[&str]) -> Result<Vec<String>, String> {
    let mut prev = seed.to_string();
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let cols: Vec<&str> = row.split('\t').collect();
        if cols.len() != 13 {
            return Err(format!(
                "malformed row: expected 13 columns, got {}",
                cols.len()
            ));
        }
        let row_no_hash = cols[..11].join("\t");
        let prov = next(&prev, row_no_hash.as_bytes());
        out.push(format!("{row_no_hash}\t{prov}\t{prev}"));
        prev = prov;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_rows_chains_each_row_to_the_last() {
        // Two raw 13-column rows; the hash columns carry garbage and
        // must be overwritten. Seed is the journal's last hash.
        let seed = "abcd ef"; // any string: the function is content-agnostic
        let r1 = "e1\t1\t2026-01-01\tent:1\tUSD\t1100\t\t100\t\td:x\t\tGARBAGE\tGARBAGE";
        let r2 = "e1\t2\t2026-01-01\tent:1\tUSD\t\t2100\t100\t\td:x\t\tGARBAGE\tGARBAGE";
        let out = link_rows(seed, &[r1, r2]).expect("two well-formed rows must link");

        let c1: Vec<&str> = out[0].split('\t').collect();
        let c2: Vec<&str> = out[1].split('\t').collect();
        assert_eq!(c1.len(), 13);
        assert_eq!(c2.len(), 13);

        // Row 1 chains from the seed; row 2 chains from row 1's new
        // provenance. Recomputing proves it; garbage is gone.
        assert_eq!(c1[12], seed);
        let want_p1 = next(seed, r1.split('\t').take(11).collect::<Vec<_>>().join("\t").as_bytes());
        assert_eq!(c1[11], want_p1);
        assert_eq!(c2[12], want_p1);
        assert_ne!(c1[11], "GARBAGE");

        // Business columns pass through untouched.
        assert_eq!(&c1[..11], &r1.split('\t').take(11).collect::<Vec<_>>()[..]);
    }

    #[test]
    fn link_rows_is_pure_and_deterministic() {
        let rows = ["e1\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\t",
                     "e1\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\t"];
        let a = link_rows("0000000000000000", &rows).unwrap();
        let b = link_rows("0000000000000000", &rows).unwrap();
        assert_eq!(a, b, "same inputs, same outputs: the determinism pillar");
    }

    #[test]
    fn link_rows_rejects_a_malformed_row_with_one_line_reason() {
        let bad = "e1\t1\td\tent\tUSD\t1100"; // 6 columns
        let err = link_rows("seed", &[bad]).unwrap_err();
        assert_eq!(err.lines().count(), 1, "one-line reason: {err}");
        assert!(err.contains("6"), "reason names the column count: {err}");
    }

    #[test]
    fn next_is_deterministic() {
        let h1 = next("0000000000000000", b"hello");
        let h2 = next("0000000000000000", b"hello");
        assert_eq!(h1, h2);
    }

    #[test]
    fn next_changes_when_input_changes() {
        let h1 = next("0000000000000000", b"hello");
        let h2 = next("0000000000000000", b"hellp"); // last byte flipped
        assert_ne!(h1, h2);
    }

    #[test]
    fn next_changes_when_prev_changes() {
        let h1 = next("0000000000000000", b"hello");
        let h2 = next("ffffffffffffffff", b"hello");
        assert_ne!(h1, h2);
    }

    #[test]
    fn next_formats_as_sixteen_lowercase_hex_chars() {
        let h = next("0000000000000000", b"hello");
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
}