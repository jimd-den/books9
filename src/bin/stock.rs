//
//! `stock` -- on-hand view, derived from the journal (driver).
//!
//! WHAT:    Reads a journal and emits a TSV with one row per
//!          (inventory account, currency) showing on-hand quantity.
//!          The fold is the inventory-specific view of the same
//!          per-account-per-currency totals the report suite uses.
//! WHY:     SRD FR-5: "stock recomputes on-hand from journal
//!          history and must equal the cached view or the cache
//!          is rebuilt and a warning is emitted." The fold is
//!          the source of truth; the cache is a disposable
//!          optimization.
//! LAYER:   Driver. Argv parsing, the fold call, the print, and
//!          the cache-reconcile path are kept thin and named.
//! DEPENDS: `libbiz::reports` (fold), stdlib.
//! USED BY: Warehouse operators, AR clerks, the loop plan's
//!          "playtest the role of an operator" step.
//!
//! Phase 3 status: SCAFFOLD. The fold shape, the cache check,
//! and the empty-journal behavior are all wired. Real inventory
//! postings arrive in Phase 5 (O2C/P2P); the fold is built so
//! those postings will be picked up automatically. An empty
//! journal today emits a header-only TSV; an inventory-coded
//! journal in Phase 5 will emit per-account on-hand rows.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let opts = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    // Fold: include every line. The inventory filter (which
    // account codes count as inventory accounts) is a Phase 5
    // concern; for now the fold is identity and the per-account
    // totals are the scaffold's output.
    let totals = match new_project::reports::fold_journal(
        &opts.journal,
        &|_line| true,
    ) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    // Compose the output as a String so we can compare against
    // the cache without printing twice.
    let mut out = String::new();
    out.push_str("account\ton_hand\tcurrency\n");
    for (account, per_ccy) in &totals {
        for (currency, (debit, credit)) in per_ccy {
            out.push_str(&format!("{account}\t{}\t{currency}\n", debit - credit));
        }
    }

    // FR-5 cache reconcile. If --cache is given:
    //   - if the cache file does not exist, write the fold's
    //     output as the new cache;
    //   - if the cache file exists and matches, exit silently;
    //   - if it disagrees, rebuild and emit a one-line stderr
    //     warning naming the disagreement.
    if let Some(cache_path) = &opts.cache {
        match std::fs::read_to_string(cache_path) {
            Ok(existing) if existing == out => {
                // Cache matches: silent.
            }
            Ok(_existing) => {
                eprintln!("stock: cache at {} disagrees with fold; rebuilt", cache_path.display());
                if let Err(e) = new_project::store::write_atomic(cache_path, out.as_bytes()) {
                    eprintln!("{e}");
                    return ExitCode::from(2);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // First run: write the cache.
                if let Err(e) = new_project::store::write_atomic(cache_path, out.as_bytes()) {
                    eprintln!("{e}");
                    return ExitCode::from(2);
                }
            }
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        }
    }

    print!("{out}");
    ExitCode::from(0)
}

struct Opts {
    journal: PathBuf,
    cache: Option<PathBuf>,
    format: String,
}

fn parse_args() -> Result<Opts, String> {
    let mut journal: Option<PathBuf> = None;
    let mut opts_cache: Option<PathBuf> = None;
    let mut format = "tsv".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--journal" => {
                let p = args.next().ok_or_else(|| "--journal requires PATH".to_string())?;
                journal = Some(PathBuf::from(p));
            }
            "--cache" => {
                let p = args.next().ok_or_else(|| "--cache requires PATH".to_string())?;
                opts_cache = Some(PathBuf::from(p));
            }
            "--format" => {
                let f = args.next().ok_or_else(|| "--format requires tsv|json".to_string())?;
                if f != "tsv" && f != "json" { return Err(format!("--format must be tsv or json (got {f:?})")); }
                format = f.to_string();
            }
            _ => {}
        }
    }
    let journal = journal.ok_or_else(|| "stock: --journal PATH is required".to_string())?;
    Ok(Opts { journal, cache: opts_cache, format })
}
