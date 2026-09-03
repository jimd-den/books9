//! `post` -- journal entry validator and appender (driver).
//!
//! WHAT:    Reads a proposed entry from stdin, runs it through
//!          `libbiz::journal::validate` (and the optional `--coa`
//!          and `--periods` gates), then either exits 0 in dry-run
//!          (`--check`) mode or atomically appends the entry to
//!          `--journal PATH` with the prev_hash column rewritten
//!          to chain to the prior line.
//! WHY:     This is the one and only door into the journal. SRD
//!          FR-1: "rejections never partially append." A driver is
//!          the right shape because parsing, validation, and
//!          atomic write are three different actors.
//! LAYER:   Driver. No business logic in `main`; argv parsing,
//!          stdin read, gate calls, and `store::append` are kept
//!          thin and named.
//! DEPENDS: `libbiz::journal` (validate), `libbiz::store` (append,
//!          last_hash, period_status, periods_root), `libbiz::chain`
//!          (link_rows), stdlib.
//! USED BY: Accountants, AP/AR clerks, future O2C/P2P tools (Phase
//!          5) that emit postings. The proposed entry arrives on
//!          stdin so the call shape stays pipelineable.
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    // Parse flags minimally. Unknown flags and the order of flags
    // vs. stdin are kept loose on purpose: the legacy `post --check`
    // form must keep working.
    //
    // Flags today:
    //   --journal PATH     journal file for live append
    //   --check            dry-run only; never writes
    //   --coa PATH         chart-of-accounts file; rejects entries
    //                      with unknown accounts (FR-1 partial;
    //                      full coa tooling is Phase 3)
    //   --periods PATH     directory of YYYY-MM period files;
    //                      entries into a closed period are rejected
    //                      (Phase 1 sets the API; Phase 2 owns the
    //                      close tool that writes these files)
    let mut args = std::env::args().skip(1);
    let mut journal: Option<PathBuf> = None;
    let mut check = false;
    let mut coa: Option<PathBuf> = None;
    let mut periods: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--journal" => {
                let p = args.next().expect("--journal requires a PATH argument");
                journal = Some(PathBuf::from(p));
            }
            "--check" => {
                check = true;
            }
            "--coa" => {
                let p = args.next().expect("--coa requires a PATH argument");
                coa = Some(PathBuf::from(p));
            }
            "--periods" => {
                let p = args.next().expect("--periods requires a PATH argument");
                periods = Some(PathBuf::from(p));
            }
            // Unknown flags remain tolerated no-ops (legacy Phase 0
            // behavior).
            _ => {}
        }
    }
    // Live-append mode: --journal PATH was given AND --check was not.
    // Otherwise stay in validate-only mode (Phase 0 default).
    let dry_run = check || journal.is_none();

    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .expect("read stdin");

    // Always validate first. FR-1: rejections never partially append.
    // The validator is the same gate for dry-run and live-append
    // modes — only the side effect differs.
    if let Err(reason) = new_project::journal::validate(&buf) {
        eprintln!("{reason}");
        return ExitCode::from(2);
    }

    // FR-1 partial: account-existence check against the chart of
    // accounts file. If --coa PATH was given, every account_debit
    // and account_credit in the proposed entry must appear in PATH.
    // The check is independent of dry-run vs. live-append: it's a
    // validation gate, not a side effect.
    if let Some(coa_path) = &coa {
        match check_coa(coa_path, &buf) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        }
    }

    // Per-period gate (Phase 1 sets the API; Phase 2 owns the
    // close tool itself). If --periods DIR was given, every
    // proposed date must either have no flag file (defaults to
    // open) or have an \"open\" file. A \"closed\" file rejects.
    if let Some(p) = &periods {
        match check_periods(p, &buf) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        }
    }

    if dry_run {
        return ExitCode::from(0);
    }

    // Live append path. --journal PATH is required; without it we
    // have nowhere to write.
    let path = match journal {
        Some(p) => p,
        None => {
            eprintln!("--journal PATH is required for live append");
            return ExitCode::from(2);
        }
    };

    // Extract the proposed data lines (skip header) and rewrite
    // each row's prev_hash column to the prior row's provenance_hash.
    // The starting prev_hash is read from the journal: if the journal
    // already holds N data lines, we read the N-th line's
    // provenance_hash and use it as the seed. An empty journal starts
    // from the zero sentinel. This makes the chain CONTINUOUS across
    // entries (commit 8's spec: "second entry's hash depends on the
    // first").
    //
    // The linking loop itself lives in chain::link_rows -- one
    // linker for every writer (post, reverse) so the on-disk chain
    // shape cannot drift between tools.
    let seed: String = match new_project::store::last_hash(&path) {
        Ok(Some(h)) => h,
        Ok(None) => "0000000000000000".to_string(),
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let mut raw_rows: Vec<&str> = Vec::new();
    let mut lines = buf.lines().filter(|l| !l.trim().is_empty());
    let _header_line = lines.next().expect("validate already rejected empty");
    for data in lines {
        raw_rows.push(data);
    }
    let out_lines = match new_project::chain::link_rows(&seed, &raw_rows) {
        Ok(l) => l,
        Err(reason) => {
            eprintln!("{reason}");
            return ExitCode::from(2);
        }
    };

    if let Err(reason) = new_project::store::append(&path, &out_lines) {
        eprintln!("{reason}");
        return ExitCode::from(2);
    }

    ExitCode::from(0)
}

/// Walk the CoA directory tree at `coa_path` and verify every
/// account in `proposed` (cols 5 and 6 of each data row) appears
/// in the walked set. Returns Ok(()) or Err with a one-line
/// reason naming the missing account.
///
/// Phase 3 migration: the CoA is now a directory tree. A leaf is
/// a directory containing `profile.tsv`; the path RELATIVE to
/// the root is the account code (e.g. "1100" for the cash account,
/// "4000/sales" for a nested one). The flat-file shape is gone.
fn check_coa(coa_path: &std::path::Path, proposed: &str) -> Result<(), String> {
    let accounts = new_project::coa::walk(coa_path)
        .map_err(|e| format!("walk coa {}: {e}", coa_path.display()))?;
    let set: std::collections::HashSet<String> = accounts
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let mut lines = proposed.lines().filter(|l| !l.trim().is_empty());
    let _header = lines.next().ok_or_else(|| "empty input".to_string())?;
    for data in lines {
        let cols: Vec<&str> = data.split('\t').collect();
        if cols.len() != 13 {
            continue;
        }
        for col_idx in [5usize, 6] {
            let acct = cols[col_idx].trim();
            if acct.is_empty() {
                continue;
            }
            if !set.contains(acct) {
                return Err(format!(
                    "unknown account: {acct} not in {}",
                    coa_path.display()
                ));
            }
        }
    }
    Ok(())
}

/// Check every period implied by the proposed entry's dates
/// against the period gate directory `periods_dir`. The on-disk
/// status of each period is read via `store::period_status`, which
/// returns a typed `PeriodStatus` (Open | Closed | Malformed). A
/// Closed period rejects the append; an Open period allows it; a
/// Malformed period rejects with the file's reason.
///
/// The rejection line carries everything the operator needs to act
/// without re-deriving it by hand: the period that refused, when the
/// tool recorded the closing (when a stamp exists), and the entry's
/// own date. One line, always — the stderr discipline holds even
/// when the message grows.
fn check_periods(periods_dir: &std::path::Path, proposed: &str) -> Result<(), String> {
    use std::collections::HashSet;
    use new_project::store::PeriodStatus;
    let mut lines = proposed.lines().filter(|l| !l.trim().is_empty());
    let _header = lines.next().ok_or_else(|| "empty input".to_string())?;
    let mut seen: HashSet<String> = HashSet::new();
    for data in lines {
        let cols: Vec<&str> = data.split('\t').collect();
        if cols.len() != 13 {
            continue;
        }
        let date = cols[2].trim();
        // Extract the leading "YYYY-MM". Anything else (or anything
        // without two '-' segments) we skip — the validator doesn't
        // impose a date format in Phase 1.
        let period = match new_project::time::yyyy_mm(date) {
            Some(p) => p,
            None => continue,
        };
        if !seen.insert(period.clone()) {
            continue; // already checked this period
        }
        match new_project::store::period_status(periods_dir, &period)? {
            PeriodStatus::Closed => {
                // Name the time when the tool knows it. A stampless
                // close (hand-written flag, or a close predating the
                // stamp commit) has no time to claim, and we do not
                // invent one.
                return Err(match new_project::store::read_close_stamp(periods_dir, &period) {
                    Some(stamp) => format!(
                        "period {period} closed at {stamp}; entry dated {date} refused"
                    ),
                    None => format!("period {period} closed; entry dated {date} refused"),
                });
            }
            PeriodStatus::Open => {}
            PeriodStatus::Malformed(reason) => {
                return Err(reason);
            }
        }
    }
    Ok(())
}

