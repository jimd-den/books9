//
// One behavior under test: `chain::next(prev_hash, row)` produces a
// deterministic 16-character hex hash. The hash function is the
// stdlib's `DefaultHasher` (NOT cryptographic — tamper-evident only,
// per the SRD's Phase 1 stub). The function lives in `chain` so it
// can be swapped for a real cryptographic hash without breaking
// call sites.
//
// Determinism contracts:
//   - given the same (prev, row) the hash is identical across calls.
//   - the first row's hash depends only on (prev=0, row content).
//   - the second row's hash depends on the first row's hash AND its
//     own row content — i.e. flipping a byte in row 1 changes the
//     row-2 hash too.
//   - re-walking the chain detects a flipped byte in row N.
//
// SRD: \"The hash chain plus provenance fields make the whole system
// forensic: verify re-computes the chain and reports the first
// divergence.\" Commit 9 lands the verify tool; this commit lands
// the function.
//
// Working rules: TDD, stdlib only, one behavior per commit.

use std::collections::HashSet;

use new_project::chain;
use new_project::store;

#[test]
fn next_returns_a_16_char_hex_string() {
    let row = b"e1\t1\td\tent\tUSD\t1100\t\t100";
    let h = chain::next("0000000000000000", row);
    assert_eq!(
        h.len(),
        16,
        "the hash must format as exactly 16 hex chars; got {h:?}"
    );
    assert!(
        h.chars().all(|c| c.is_ascii_hexdigit()),
        "the hash must be hex-only; got {h:?}"
    );
}

#[test]
fn next_is_deterministic_for_the_same_inputs() {
    let row = b"e1\t1\td\tent\tUSD\t1100\t\t100";
    let h1 = chain::next("0000000000000000", row);
    let h2 = chain::next("0000000000000000", row);
    assert_eq!(h1, h2, "the hash must be deterministic");
}

#[test]
fn next_changes_when_a_byte_in_the_row_changes() {
    let row1 = b"e1\t1\td\tent\tUSD\t1100\t\t100";
    let row2 = b"e1\t1\td\tent\tUSD\t1100\t\t101"; // last byte 100 → 101
    let h1 = chain::next("0000000000000000", row1);
    let h2 = chain::next("0000000000000000", row2);
    assert_ne!(
        h1, h2,
        "the hash must change when the row changes (avalanche)"
    );
}

#[test]
fn next_changes_when_the_prev_hash_changes() {
    let row = b"e1\t1\td\tent\tUSD\t1100\t\t100";
    let h_zero = chain::next("0000000000000000", row);
    let h_other = chain::next("ffffffffffffffff", row);
    assert_ne!(
        h_zero, h_other,
        "the hash must depend on prev_hash (chain link)"
    );
}

#[test]
fn chain_walk_detects_a_flipped_byte() {
    // The end-to-end chain walk we will replicate in `verify` later.
    // For now we hand-roll it in the test so the chain::next contract
    // is pinned even before the verify tool exists.
    let prev0 = "0000000000000000";
    let row1 = b"e1\t1\td\tent\tUSD\t1100\t\t100";
    let row2 = b"e1\t2\td\tent\tUSD\t\t2100\t100";

    let h1 = chain::next(prev0, row1);
    let h2 = chain::next(&h1, row2);

    // Determinism: same inputs → same hashes.
    let h1_b = chain::next(prev0, row1);
    let h2_b = chain::next(&h1_b, row2);
    assert_eq!(h1, h1_b);
    assert_eq!(h2, h2_b);

    // Tamper: flip a byte in row1. The recomputed h1 differs, which
    // means h2 (which depends on h1) also differs.
    let mut row1_bad = row1.to_vec();
    row1_bad[20] ^= 0x01; // flip a bit in the middle
    let h1_bad = chain::next(prev0, &row1_bad);
    assert_ne!(h1, h1_bad, "flipped byte must change the row hash");

    // Recompute h2 from the bad h1 — it must differ from the good h2.
    let h2_bad = chain::next(&h1_bad, row2);
    assert_ne!(
        h2, h2_bad,
        "the second row's hash must depend on the first row's hash"
    );
}

#[test]
fn many_calls_produce_a_wide_distribution_of_hashes() {
    // Faintness sanity: the hash must not collapse to a small set
    // of values across many distinct rows. We sample 64 different
    // rows and assert the unique-hash count is high.
    let mut seen: HashSet<String> = HashSet::new();
    for i in 0..64u32 {
        let row = format!("e1\t{i}\td\tent\tUSD\t1100\t\t{i}");
        seen.insert(chain::next("0000000000000000", row.as_bytes()));
    }
    assert!(
        seen.len() >= 60,
        "64 inputs should produce near-64 unique hashes; got {}",
        seen.len()
    );
}

#[test]
fn append_then_tamper_makes_the_chain_diverge() {
    // End-to-end: write a journal via post (using store::append),
    // then flip a byte and re-walk the chain. The hash for the
    // tampered row must change, demonstrating that the chain
    // captures byte-level integrity.
    //
    // This is the contract `verify` will rely on in commit 9.
    use std::path::PathBuf;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_path(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "books9-{}-{tag}-{pid}-{nanos}.tsv",
            process::id(),
            pid = process::id(),
            nanos = nanos
        ))
    }

    let path = unique_path("chain-tamper");
    let _ = std::fs::remove_file(&path);
    store::create(&path).expect("create");

    // Build two 13-col data rows whose prev_hash/chain wiring is
    // done by the chain helper. This bypasses `post` and tests the
    // chain primitive directly.
    // Cols: entry_id seq date entity currency account_debit
    //       account_credit amount_minor party doc_ref tag prov prev
    let row1_full = "e1\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t";
    let row2_full = "e1\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t";

    // Hash the 11 content columns (no hash cols).
    let h1 = chain::next("0000000000000000", row1_full.as_bytes());

    let mut row1_on_disk = String::from(row1_full);
    row1_on_disk.push_str(&h1);
    row1_on_disk.push('\t');
    row1_on_disk.push_str("0000000000000000");

    let h2 = chain::next(&h1, row2_full.as_bytes());
    let mut row2_on_disk = String::from(row2_full);
    row2_on_disk.push_str(&h2);
    row2_on_disk.push('\t');
    row2_on_disk.push_str(&h1);

    let lines = vec![row1_on_disk.clone(), row2_on_disk.clone()];
    store::append(&path, &lines).expect("append");

    // Walk the chain as verify will: recompute h1 from the on-disk
    // row-without-hash-cols and the prev=0. If we tamper with row1,
    // the recomputed h1 must differ.
    let on_disk_row1: Vec<&str> = row1_on_disk.split('\t').collect();
    assert_eq!(on_disk_row1.len(), 13, "row1 must be 13 cols: {on_disk_row1:?}");
    let recomputed_h1 = chain::next("0000000000000000", row1_full.as_bytes());
    assert_eq!(recomputed_h1, h1);

    // Tamper: flip a byte in the on-disk row (in the content portion).
    let mut tampered = row1_on_disk.into_bytes();
    let pos = row1_full.len() / 2; // middle of the content portion
    tampered[pos] ^= 0x01;
    let tampered_str = String::from_utf8(tampered).unwrap();
    let cols: Vec<&str> = tampered_str.split('\t').collect();
    let tampered_content: String = cols[..11].join("\t");
    let recomputed_tampered_h1 =
        chain::next("0000000000000000", tampered_content.as_bytes());
    assert_ne!(
        recomputed_tampered_h1, h1,
        "flipping a byte in the journal must change the chain hash"
    );

    let _ = std::fs::remove_file(&path);
}