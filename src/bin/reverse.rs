//! `reverse` -- the FR-2 correction tool (driver).
//!
//! WHAT:    `reverse --journal PATH --entry-id ID` reads the
//!          journal, finds every line of the named entry, builds a
//!          new entry whose legs are the mirror image (debit
//!          account and credit account swap; amount stays a positive
//!          minor-unit integer), and appends it through the same
//!          `store::append` path `post` uses so the chain stays
//!          continuous.
//! WHY:     SRD FR-2: "Corrections are reversing entries only; the
//!          journal is never edited or deleted." A wrong entry is
//!          answered, never erased. The reversing entry's doc_ref
//!          points at the original so a reader can follow the
//!          trail.
//! LAYER:   Driver. Mirror construction is its own use case; argv
//!          parsing, the journal read, and the append are thin.
//! DEPENDS: `libbiz::store` (last_hash, append), `libbiz::chain`
//!          (link_rows), stdlib.
//! USED BY: Accountants who posted a wrong number and need the
//!          books to remain auditable. The tool never edits the
//!          journal; it only appends a new entry that nets the
//!          original to zero.
use std::path::PathBuf;
use std::process::ExitCode;

use new_project::{chain, journal, store};

const N_COLS: usize = 13;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut journal_path: Option<PathBuf> = None;
    let mut entry_id: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--journal" => {
                let p = args.next().expect("--journal requires a PATH argument");
                journal_path = Some(PathBuf::from(p));
            }
            "--entry-id" => {
                let id = args.next().expect("--entry-id requires an ID argument");
                entry_id = Some(id);
            }
            _ => {} // tolerate unknown flags (matches post)
        }
    }
    let path = match journal_path {
        Some(p) => p,
        None => {
            eprintln!("reverse: --journal PATH is required");
            return ExitCode::from(2);
        }
    };
    let id = match entry_id {
        Some(i) => i,
        None => {
            eprintln!("reverse: --entry-id ID is required");
            return ExitCode::from(2);
        }
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("reverse: read {}: {e}", path.display());
            return ExitCode::from(2);
        }
    };

    let mirror = match build_mirror(&id, &content) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("reverse: {e}");
            return ExitCode::from(2);
        }
    };

    // The mirror funnels through the same validator as every other
    // proposal. A well-formed source always yields a well-formed
    // mirror, but the kernel does not assume; it checks.
    let mut proposal = String::from(store::HEADER_LINE);
    proposal.push('\n');
    for row in &mirror {
        proposal.push_str(row);
        proposal.push('\n');
    }
    if let Err(reason) = journal::validate(&proposal) {
        eprintln!("reverse: {reason}");
        return ExitCode::from(2);
    }

    // Chain seed: continue the journal's chain, exactly as post does.
    let seed = match store::last_hash(&path) {
        Ok(Some(h)) => h,
        Ok(None) => "0000000000000000".to_string(),
        Err(e) => {
            eprintln!("reverse: {e}");
            return ExitCode::from(2);
        }
    };
    let linked = match chain::link_rows(&seed, &mirror.iter().map(|s| s.as_str()).collect::<Vec<_>>())
    {
        Ok(l) => l,
        Err(e) => {
            eprintln!("reverse: {e}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = store::append(&path, &linked) {
        eprintln!("reverse: {e}");
        return ExitCode::from(2);
    }

    ExitCode::from(0)
}

/// Build the mirror entry's raw rows (hash columns blank; the caller
/// links them). Returns a one-line Err reason for the two business
/// refusals: unknown entry id, and a source that is already a
/// reversal.
///
/// The cancel rule, one leg at a time: every source debit leg is
/// cancelled by a mirror CREDIT leg on the SAME account, and every
/// source credit leg by a mirror DEBIT on the same account. The
/// accounts do not move between columns; the SIDE flips. Folding
/// the source and the mirror leaves every account net zero — which
/// is what "reversed" must mean, and what a swap of accounts (not
/// sides) would break.
fn build_mirror(id: &str, content: &str) -> Result<Vec<String>, String> {
    let mut lines = content.lines().filter(|l| !l.trim().is_empty());
    let _header = lines.next().ok_or("empty journal")?;

    let mut src_rows: Vec<Vec<String>> = Vec::new();
    for data in lines {
        let cols: Vec<&str> = data.split('\t').collect();
        if cols.len() != N_COLS || cols[0] != id {
            continue;
        }
        src_rows.push(cols.iter().map(|s| s.to_string()).collect());
    }

    if src_rows.is_empty() {
        return Err(format!("entry {id} not found in journal"));
    }

    // A mirror of a mirror restores the mistake. Refuse the trick.
    for r in &src_rows {
        if r[9].starts_with("rev:") {
            return Err(format!(
                "entry {id} is already a reversal; refusing to reverse a reversal"
            ));
        }
    }

    let mut out = Vec::with_capacity(src_rows.len());
    for (n, r) in src_rows.iter().enumerate() {
        // One leg in, one mirrored leg out: the side flips, the
        // account stays, the amount stays a positive minor-unit
        // integer. A source debit leg (d, "") becomes a mirror
        // credit leg ("", d); a source credit leg (c, "") becomes a
        // mirror debit leg ("", c) on c. A malformed both-or-neither
        // leg never reaches here: the source was validated on its
        // way in, and the validate gate below re-checks the mirror.
        let (d, c) = (&r[5], &r[6]);
        let (md, mc) = (c, d);
        out.push(format!(
            "rev-{id}\t{n}\t{date}\t{entity}\t{ccy}\t{md}\t{mc}\t{amt}\t{party}\trev:{doc}\t{tag}\t\t",
            id = id,
            n = n + 1,
            date = r[2],
            entity = r[3],
            ccy = r[4],
            md = md,
            mc = mc,
            amt = r[7],
            party = r[8],
            doc = r[9],
            tag = r[10],
        ));
    }
    Ok(out)
}