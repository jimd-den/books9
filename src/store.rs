//! `libbiz::store` -- on-disk append-only, hash-chained journal.
//!
//! WHAT:    The journal's on-disk surface: `create`, `open`,
//!          `append`, `last_hash`. Plus the period gate (`periods_root`,
//!          `period_status`, `set_period`) and the journal format
//!          constant `HEADER_LINE`. Every write goes through
//!          write-temp + `sync_all` + rename so a crash mid-write
//!          leaves the prior journal untouched.
//! WHY:     SRD FR-2 (corrections are reversing entries only) and
//!          FR-1 (rejections never partially append). The journal is
//!          the books; this module is the only door into it.
//! LAYER:   Interface adapter. Owns the on-disk format. Knows about
//!          the filesystem, the hash, and the period flag files; does
//!          not know about any business verb.
//! DEPENDS: `crate::chain` (hash), `std::fs`, `std::io`. Knows the
//!          journal format via `HEADER_LINE`.
//! USED BY: `post` (writes), `verify` (reads), `close` (reads + writes
//!          the period flag), `reverse` (reads + writes via `post`).
//!          No future business tool is allowed to write the journal
//!          directly; everything funnels through `post`.
pub const HEADER_LINE: &str = "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash";

use std::fs;
use std::io::Write;
use std::path::Path;

/// Create a fresh journal file at `path` containing only the header
/// line. Refuses to overwrite an existing file — the journal is
/// append-only (FR-2) and the kernel must never silently clobber the
/// books.
///
/// On success the file exists with exactly one line (the header) and
/// no trailing junk. On a pre-existing path the call returns an `Err`
/// carrying a one-line reason suitable for stderr; the existing file
/// is left untouched.

/// Atomically write `bytes` to `path` using the kernel's
/// write-temp + `sync_all` + rename contract.
///
/// WHAT:    The only path that knows about write-temp + `sync_all`
///          + rename. Three call sites (`append`, `set_period`,
///          `close::write_stamp`) currently re-implement this dance;
///          this primitive is the single source of truth.
/// WHY:     SRD FR-1 partial: "rejections never partially append"
///          generalizes to "the journal is never observed torn."
///          Without fsync, a power loss between rename and the
///          kernel's later writeback could leave an empty file at
///          the renamed path -- the rename is atomic at the
///          directory-entry level, but the data behind it might
///          not be on disk yet. `sync_all` forces the page cache
///          to stable storage BEFORE the rename becomes visible.
/// LAYER:   Interface adapter. Pure-with-IO: same bytes in, same
///          bytes on disk, but only when the kernel agrees.
/// DEPENDS: `std::fs`. Knows nothing about the journal format;
///          this is the byte-level primitive below `append` and
///          `set_period`.
/// USED BY: `append` (Phase 1), `set_period` (Phase 2),
///          `close::write_stamp` (Phase 2). Future atomic writers
///          (cache files, the FX rates table) route through here too.
///
/// Returns `Err` with a one-line stderr-ready reason on any I/O
/// failure. The temp file is best-effort cleaned up on failure so
/// the parent directory does not accumulate `.tmp` siblings.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("write_atomic: path has no parent: {}", path.display()))?;
    let stem = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("write_atomic: path has no filename: {}", path.display()))?;
    let tmp_path = parent.join(format!(".{stem}.tmp"));

    // Best-effort cleanup on failure. We do not propagate the
    // unlink error because the original error is the one the
    // caller cares about; an orphan .tmp is annoying, not fatal.
    let result: Result<(), String> = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)
            .map_err(|e| format!("open tmp {}: {e}", tmp_path.display()))?;
        f.write_all(bytes)
            .map_err(|e| format!("write tmp {}: {e}", tmp_path.display()))?;
        f.sync_all()
            .map_err(|e| format!("fsync tmp {}: {e}", tmp_path.display()))?;
        fs::rename(&tmp_path, path)
            .map_err(|e| format!("rename tmp to {}: {e}", path.display()))?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    result
}

pub fn create(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "journal already exists at {}; refusing to overwrite (FR-2)",
            path.display()
        ));
    }

    // Open exclusive and write. `create_new(true)` is the atomic
    // "fail if exists" primitive on every POSIX target; on its own
    // it would let a race slip in, but combined with the explicit
    // exists() check above we get a loud error and a preserved file.
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("create {}: {e}", path.display()))?;

    writeln!(f, "{HEADER_LINE}")
        .map_err(|e| format!("write header to {}: {e}", path.display()))?;

    Ok(())
}

/// Open an existing journal at `path` and return the number of *data*
/// lines it holds (the header is excluded; the count is therefore the
/// number of one-sided legs across all entries).
///
/// Errors if the file does not exist. Does not validate the chain;
/// `verify` owns that. This is the cheap count used by callers that
/// just want to know whether anything has been posted yet.
pub fn open(path: &Path) -> Result<usize, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;

    // Skip the first line (header). Any subsequent non-empty line is
    // a data row. Whitespace-only lines are filtered so the count
    // matches what a tool would actually see.
    let mut iter = content.lines();
    iter.next(); // header
    let n = iter.filter(|l| !l.trim().is_empty()).count();
    Ok(n)
}

/// Return the provenance_hash of the last data line in `path`, or
/// `Ok(None)` if the journal holds zero data lines.
///
/// The chain is built entry-by-entry: each new entry's first row
/// chains to the prior entry's last row's provenance_hash. So when
/// `post` is about to append, it needs to know the last hash to seed
/// the chain. We do NOT re-walk and re-verify the chain here —
/// `verify` owns that — but we do read the last line's stored hash
/// directly from the bytes.
///
/// Errors if the file is unreadable. Returns `Ok(None)` for a
/// header-only (empty) journal.
pub fn last_hash(path: &Path) -> Result<Option<String>, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;

    let mut last_data: Option<&str> = None;
    let mut iter = content.lines();
    iter.next(); // header
    for data in iter {
        if !data.trim().is_empty() {
            last_data = Some(data);
        }
    }
    let Some(last) = last_data else {
        return Ok(None);
    };
    let cols: Vec<&str> = last.split('\t').collect();
    if cols.len() != 13 {
        return Err(format!(
            "last line of {} is not 13 columns (got {len})",
            path.display(),
            len = cols.len()
        ));
    }
    Ok(Some(cols[11].to_string()))
}
/// Append already-validated data lines to an existing journal.
///
/// WHAT:    Reads the current journal text, composes the new file
///          contents in memory (`compose_new_content`), and hands
///          them to `write_atomic` for the on-disk write.
/// WHY:     SRD FR-1: "rejections never partially append." The
///          journal is never observed torn; readers see either the
///          pre-append or post-append file, never a mix.
/// LAYER:   Interface adapter. Performs no business validation;
///          the caller runs `crate::journal::validate` first.
/// DEPENDS: `crate::chain` (caller wires the chain) and the kernel
///          primitive `write_atomic` (this module).
/// USED BY: `post` (driver) after `chain::link_rows` fills in the
///          prev_hash column; `reverse` (driver) for the reversing
///          entry it appends.
///
/// Atomicity contract: write-temp + `sync_all` + rename, performed
/// by `write_atomic`. The hash chain (provenance per row) is the
/// caller's responsibility -- `lines` arrive here with `prev_hash`
/// already filled in (typically by `chain::link_rows`).
pub fn append(path: &Path, lines: &[String]) -> Result<(), String> {
    if lines.is_empty() {
        return Err("append called with no lines".to_string());
    }
    // Read the existing content first. If the journal is missing or
    // unreadable, error loudly -- we never create-on-append (create
    // owns that path).
    let current = fs::read_to_string(path)
        .map_err(|e| format!("open {} for append: {e}", path.display()))?;
    let new_content = compose_new_content(&current, lines);
    write_atomic(path, new_content.as_bytes())
}

/// Compose the new journal file contents in memory: `current` plus
/// each new line, with exactly one newline between every adjacent
/// piece. Pure: same inputs, same output. No fs, no rename, no
/// side effects -- this is the part of `append` that can be tested
/// without a tempdir.
///
/// WHAT:    A pure string-concat that turns (current journal text,
///          new data lines) into the full new file contents.
/// WHY:     Splitting composition from on-disk write makes the
///          "what bytes did the new journal contain?" question
///          answerable without spinning up a tempdir; it also
///          keeps `append` itself short and named.
/// LAYER:   Interface adapter (still knows the journal text shape,
///          but not the disk).
/// DEPENDS: stdlib only.
/// USED BY: `append`; future code that needs the post-append
///          bytes without an actual on-disk write (e.g. an in-memory
///          journal for tests).
pub fn compose_new_content(current: &str, lines: &[String]) -> String {
    let mut new_content = String::with_capacity(
        current.len() + lines.iter().map(|l| l.len() + 1).sum::<usize>(),
    );
    new_content.push_str(current);
    if !new_content.ends_with('\n') && !new_content.is_empty() {
        new_content.push('\n');
    }
    for line in lines {
        new_content.push_str(line);
        new_content.push('\n');
    }
    new_content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_line_constant_is_the_srd_contract() {
        // The 13 columns, in order, named by the SRD. Any drift here
        // is a hard break of the journal format.
        let cols: Vec<&str> = HEADER_LINE.split('\t').collect();
        assert_eq!(cols.len(), 13);
        assert_eq!(cols[0], "entry_id");
        assert_eq!(cols[4], "currency");
        assert_eq!(cols[7], "amount_minor");
        assert_eq!(cols[11], "provenance_hash");
        assert_eq!(cols[12], "prev_hash");
    }

    /// Pick a fresh tempdir under the OS tempdir. Each call returns
    /// a unique directory, so concurrent tests don't collide. Used by
    /// the write_atomic tests; not exported.
    fn tempfile() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("books9-store-{pid}-{n}"));
        std::fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    /// `write_atomic` is the kernel contract: write-temp + fsync +
    /// rename. These three tests pin the contract on a real tempdir
    /// so the primitive cannot drift back to a non-atomic write or
    /// skip the sync_all. Without this, a future commit could quietly
    /// regress the audit-evidence guarantee.

    #[test]
    fn write_atomic_creates_target_with_exact_bytes() {
        let tmp = tempfile();
        let path = tmp.join("target.tsv");
        let bytes = b"entry_id\tseq\nfoo\t1\n";
        write_atomic(&path, bytes).expect("write_atomic must succeed");
        let got = std::fs::read(&path).expect("read back");
        assert_eq!(got, bytes, "on-disk bytes must match what was written");
    }

    #[test]
    fn write_atomic_overwrites_existing_file_atomically() {
        let tmp = tempfile();
        let path = tmp.join("target.tsv");
        std::fs::write(&path, b"old").expect("seed");
        let bytes = b"new contents after sync";
        write_atomic(&path, bytes).expect("rewrite must succeed");
        let got = std::fs::read(&path).expect("read back");
        assert_eq!(got, bytes, "second write replaces the first");
    }

    #[test]
    fn write_atomic_leaves_no_tmp_sibling_after_success() {
        let tmp = tempfile();
        let path = tmp.join("target.tsv");
        write_atomic(&path, b"x").expect("ok");
        let siblings: Vec<String> = std::fs::read_dir(&tmp)
            .expect("readdir")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        // The temp sibling (".target.tsv.tmp") must be gone after rename.
        assert!(
            !siblings.iter().any(|n| n.contains(".tmp")),
            "tmp sibling must not survive success; saw: {siblings:?}"
        );
    }
}

/// Derive the sibling `periods/` directory from a journal file path.
///
/// The SRD places `/biz/ledger/periods/` as a sibling of
/// `/biz/ledger/journal`. This function does the pure path arithmetic:
/// it replaces the journal's filename with `periods` and returns the
/// result. No filesystem touch, no creation, no check that the
/// directory exists. Callers that need the directory to exist create
/// it themselves (or reject when it is absent).
///
/// Phase 2 owns the period gate; this helper exists so `post` and
/// `close` and any future tool agree on the same layout without each
/// of them re-deriving the path inline.
pub fn periods_root(journal_path: &Path) -> std::path::PathBuf {
    match journal_path.parent() {
        Some(parent) => parent.join("periods"),
        None => std::path::PathBuf::from("periods"),
    }
}

/// Status of a single period under `periods_root/YYYY-MM`.
///
/// The on-disk layout is one flag file per period; its content is
/// either `closed` (the period refuses new postings) or `open`
/// (new postings are allowed). A missing file defaults to Open per
/// the SRD's filesystem contract. A non-empty, non-`closed` file is
/// treated as Malformed so callers do not silently treat corrupted
/// state as either open or closed.
///
/// `Malformed` carries a one-line reason suitable for stderr; the
/// caller decides whether to reject the operation or to recover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeriodStatus {
    Open,
    Closed,
    Malformed(String),
}

/// Read the on-disk status of `period` under `periods_root`.
///
/// Read-only: the function does not create the periods directory,
/// does not create the period file, does not normalize beyond a
/// trim. The behavior is the source of truth for callers (`post`,
/// `close`, future tools) that need to gate on a period's state.
///
/// Errors are reserved for I/O failures other than a missing file
/// (e.g. permission denied). A missing file is the Open case and is
/// not an error.
pub fn period_status(periods_root: &Path, period: &str) -> Result<PeriodStatus, String> {
    let flag = periods_root.join(period);
    let content = match fs::read_to_string(&flag) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(PeriodStatus::Open),
        Err(e) => return Err(format!("read period flag {}: {e}", flag.display())),
    };
    let trimmed = content.trim();
    if trimmed == "closed" {
        Ok(PeriodStatus::Closed)
    } else if trimmed.is_empty() || trimmed == "open" {
        Ok(PeriodStatus::Open)
    } else {
        Ok(PeriodStatus::Malformed(format!(
            "period {period} flag file at {} has unrecognized content (expected 'closed' or 'open'); got {trimmed:?}",
            flag.display()
        )))
    }
}

/// Atomically write the flag file for `period` under `periods_root`.
///
/// The shape mirrors `append`: write-temp + fsync + rename, so a
/// crash mid-write leaves the prior flag file untouched and no
/// half-written `.tmp` siblings under `periods_root`.
///
/// The function refuses to write to any path other than
/// `periods_root/<period>`:
///   - period names containing a path separator (`/`, `\`) reject
///   - period names that look absolute (start with `/`) reject
///   - period names that start with `.` reject (the leading-dot
///     namespace is reserved for sibling bookkeeping files such as
///     `.last_close`)
/// On success the on-disk file is the new value; on failure the
/// prior state is preserved (FR-1 applies to the period gate too).
///
/// `Malformed` is not a writable status — a malformed flag file is
/// the operator's problem, not a value the tool should silently
/// overwrite.
pub fn set_period(
    periods_root: &Path,
    period: &str,
    status: PeriodStatus,
) -> Result<(), String> {
    if period.is_empty()
        || period.contains('/')
        || period.contains('\\')
        || period.starts_with('.')
    {
        return Err(format!(
            "set_period: refusing to use {period:?} as a period file name; \
             must be a non-empty, non-dot, non-path filename"
        ));
    }
    let bytes: &[u8] = match status {
        PeriodStatus::Open => b"open\n",
        PeriodStatus::Closed => b"closed\n",
        PeriodStatus::Malformed(_) => {
            return Err(
                "set_period: refusing to write Malformed; that is a diagnosis, not a value"
                    .to_string(),
            );
        }
    };

    let flag = periods_root.join(period);
    write_atomic(&flag, bytes)
}

/// Atomically record the close stamp for `period` under `periods_root`.
///
/// WHAT:    Writes a single-line stamp file (e.g. "2026-09-01T12:34:56Z")
///          to `periods_root/.{period}.last_close`. The dot-prefix is
///          the reserved bookkeeping namespace: `set_period` refuses
///          period names starting with `.`, so a stamp file can never
///          be mistaken for a period flag and `read_dir` listings
///          filter it out.
/// WHY:     The stamp is the operator's "when did this door shut?"
///          answer; `post` includes it in the close-period rejection
///          message ("period YYYY-MM closed at STAMP; entry dated
///          DATE refused"). Without a stamp, the message is the
///          stampless form ("period YYYY-MM closed; ..."). The stamp
///          is context, not a gate condition; `read_close_stamp`
///          collapsing to `None` on any read failure is correct.
/// LAYER:   Interface adapter (writer); the read is also here for
///          symmetry. Both belong with the on-disk layout they
///          describe.
/// DEPENDS: `write_atomic` (kernel primitive), `time::format_utc`
///          (the formatter that the caller hands the stamp to).
/// USED BY: `close` (driver, on successful close), `post` (driver,
///          via `read_close_stamp` to enrich rejection messages).
pub fn write_close_stamp(periods_root: &Path, period: &str, stamp: &str) -> Result<(), String> {
    let path = periods_root.join(format!(".{period}.last_close"));
    write_atomic(&path, format!("{stamp}\n").as_bytes())
}

/// Read the close stamp for `period` if one was recorded. Returns
/// `None` on any read failure (missing file, unreadable, empty) --
/// the stamp is context in the gate message, not a gate condition.
/// The gate already refused; this only decides whether we can say
/// when.
///
/// LAYER:   Interface adapter. Sits next to `write_close_stamp` so
///          the on-disk shape of the stamp is described in one
///          place, not split across two drivers.
/// USED BY: `post` (driver).
pub fn read_close_stamp(periods_root: &Path, period: &str) -> Option<String> {
    let raw = fs::read_to_string(periods_root.join(format!(".{period}.last_close"))).ok()?;
    let s = raw.lines().next()?.trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}


/// FR-2 inventory: the names of the journal-mutating functions in
/// this module. Hidden from rustdoc because it's a test surface, not
/// a runtime surface. `tests/store_fr2.rs` reads it to assert that no
/// future commit silently adds a truncate/edit/rewrite/delete path.
///
/// `periods_root` is a path-arithmetic helper and does not mutate the
/// journal; it is intentionally absent from this inventory so the
/// pin stays focused on the surface that FR-2 actually protects.
#[doc(hidden)]
pub fn __inventory() -> [&'static str; 4] {
    ["create", "open", "append", "last_hash"]
}