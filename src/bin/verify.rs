//! `verify` -- journal chain auditor (driver).
//!
//! WHAT:    Reads the journal at PATH, re-walks the hash chain from
//!          the zero sentinel, and reports the first divergence on
//!          stderr. Exits 0 on a clean journal, nonzero on divergence
//!          or unreadable input.
//! WHY:     SRD: "`verify` re-computes the chain and reports the
//!          first divergence." Cornerstone of FR-2 (corrections are
//!          reversing entries only): a flipped byte anywhere in the
//!          journal is caught at the first line that doesn't recompute.
//! LAYER:   Driver. The chain walk is small enough to live here
//!          directly while in Phase 0/1; Phase 8 wraps it in `ledgerd`.
//! DEPENDS: `libbiz::chain` (`next` for re-derivation), stdlib.
//! USED BY: Auditors, the operator's nightly cron, future the network inquiry
//!          tools that want a quick "is this journal intact?" answer.
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

const N_COLS: usize = 13;
const HASH_COL: usize = 11;
const PREV_COL: usize = 12;
const ZERO_HASH: &str = "0000000000000000";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let path = match args.next() {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("usage: verify PATH");
            return ExitCode::from(2);
        }
    };

    let content = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("read {}: {e}", path.display());
            return ExitCode::from(2);
        }
    };

    let mut iter = content.lines();
    let header_line = match iter.next() {
        Some(h) => h,
        None => {
            eprintln!("empty journal: {} has no header line", path.display());
            return ExitCode::from(2);
        }
    };
    // Sanity: the first line must look like the SRD header (13 cols).
    // If it doesn't, we treat it as an unreadable journal rather than
    // a chain divergence — the divergence meaning depends on the
    // header being present.
    if header_line.split('\t').count() != N_COLS {
        eprintln!(
            "header must be {N_COLS} columns; got {}",
            header_line.split('\t').count()
        );
        return ExitCode::from(2);
    }

    // Walk the chain. `prev_hash` is the hash carried by the previous
    // data row's prev_hash column; for the very first data row it
    // must be the zero sentinel. `prov_expected` is the hash we
    // re-derive from the row content (cols 0..11).
    let mut prev_hash: String = ZERO_HASH.to_string();
    let mut line_no: usize = 1; // 1-based, counting the header as line 1
    for data in iter {
        line_no += 1;
        if data.trim().is_empty() {
            continue; // tolerate blank lines (none in our writer, but cheap)
        }
        let cols: Vec<&str> = data.split('\t').collect();
        if cols.len() != N_COLS {
            eprintln!(
                "line {line_no}: expected {N_COLS} columns, got {}",
                cols.len()
            );
            return ExitCode::from(1);
        }
        let prov_expected = new_project::chain::next(&prev_hash, cols[..11].join("\t").as_bytes());
        let prov_got = cols[HASH_COL];
        let prev_got = cols[PREV_COL];
        if prov_got != prov_expected {
            eprintln!(
                "line {line_no}: provenance_hash mismatch (expected {prov_expected}, got {prov_got})"
            );
            return ExitCode::from(1);
        }
        if prev_got != prev_hash {
            eprintln!(
                "line {line_no}: prev_hash mismatch (expected {prev_hash}, got {prev_got})"
            );
            return ExitCode::from(1);
        }
        // Advance. The current row's provenance_hash becomes the
        // prev_hash the next row must carry.
        prev_hash = prov_expected;
    }

    ExitCode::from(0)
}