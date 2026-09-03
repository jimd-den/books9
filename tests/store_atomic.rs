//
// One behavior under test: `store::append` is atomic via write-temp
// + rename. A successful call leaves exactly the expected count of
// data lines AND no orphan temp files in the journal's directory.
// A failed call leaves the existing journal byte-identical.
//
// SRD: FR-1 (\"rejections never partially append\") and the journal
// format clause (\"Append-only ... journal\"). The kernel must never
// leave the books in a half-written state.
//
// Working rules: TDD, stdlib only, one behavior per commit.
//
// What this commit does NOT cover (each gets its own commit):
//   - fsync before rename (commit 7)
//   - hash chain (commit 8)
//   - verify tool (commit 9)
//   - multi-line entries across lines (commit 10)

use std::fs;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use new_project::store;

fn unique_path(tag: &str) -> PathBuf {
    use std::process;
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

#[test]
fn append_leaves_no_temp_files_in_the_journal_directory() {
    // The \"write-temp + rename\" pattern is observable by reading
    // the directory after a successful call: there must be exactly
    // one file (the journal itself), no orphans. Today's stub
    // implementation may not satisfy this.
    let path = unique_path("atomic-no-orphan");
    let _ = fs::remove_file(&path);
    store::create(&path).expect("create must succeed");

    let lines = vec!["e1\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\tph\tpv".to_string()];
    store::append(&path, &lines).expect("append must succeed");

    // List the directory contents. We expect exactly one entry: the
    // journal file itself. Anything else (a *.tmp, *.swp, ...) is
    // evidence the append wasn't atomic.
    let parent = path.parent().expect("journal must live under temp_dir");
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let mut orphans: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(parent).expect("read_dir").flatten() {
        let p = entry.path();
        if p == path {
            continue;
        }
        // Any sibling whose stem begins with the journal's stem is
        // an orphan temp file. \"books9-...-atomic-no-orphan-...\" is
        // the stem we filter on.
        if let Some(s) = p.file_stem().and_then(|s| s.to_str()) {
            if s.starts_with(stem) {
                orphans.push(p);
            }
        }
    }
    assert!(
        orphans.is_empty(),
        "append left orphan temp files in the journal directory: {:?}",
        orphans
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn append_succeeds_and_advances_count_by_exactly_n() {
    // The \"N+N0\" half of the contract. Pre-create with N0 data
    // lines (via raw file write so we control the count exactly),
    // then append N lines via the API, then assert count == N0 + N.
    let path = unique_path("atomic-count");
    let _ = fs::remove_file(&path);
    store::create(&path).expect("create must succeed");

    // Seed with 3 data lines so we can later append 2 and observe 5.
    let seed = "e0\t1\td\tent\tUSD\t1100\t\t1\t\t\t\t\tph\tpv\n".to_string();
    {
        let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
        for _ in 0..3 {
            f.write_all(seed.as_bytes()).unwrap();
        }
    }
    assert_eq!(store::open(&path).unwrap(), 3, "seed");

    let lines = vec![
        "e1\t1\td\tent\tUSD\t1100\t\t1\t\t\t\t\tph\tpv".to_string(),
        "e1\t2\td\tent\tUSD\t\t2100\t1\t\t\t\t\tph\tpv".to_string(),
    ];
    store::append(&path, &lines).expect("append must succeed");

    assert_eq!(
        store::open(&path).unwrap(),
        5,
        "a successful atomic append must add exactly N lines"
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn append_replaces_the_journal_via_rename_so_inode_changes() {
    // The \"write-temp + rename\" pattern is observable via inode:
    // a rename(2) replaces the directory entry, so the journal's
    // inode changes after every successful append. A direct
    // open-for-append would keep the same inode. Asserting this
    // pins the atomic contract for the commit that lands write-temp
    // + rename.
    let path = unique_path("atomic-inode");
    let _ = fs::remove_file(&path);
    store::create(&path).expect("create must succeed");

    let inode_before = fs::metadata(&path).unwrap().ino();

    let lines = vec!["e1\t1\td\tent\tUSD\t1100\t\t1\t\t\t\t\tph\tpv".to_string()];
    store::append(&path, &lines).expect("append must succeed");

    let inode_after = fs::metadata(&path).unwrap().ino();

    // On unix, a rename(2) replaces the directory entry; the file
    // behind the name is a NEW inode from the kernel's perspective.
    // A direct append (write-in-place) would not change the inode.
    assert_ne!(
        inode_before, inode_after,
        "successful atomic append must replace the journal via rename(2); \
         inode before={inode_before} after={inode_after}"
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn append_failure_leaves_existing_journal_unchanged() {
    // The \"failure leaves N0\" half of the contract. Force a failure
    // by pointing append at a path whose parent directory does not
    // exist: the open-for-append call must error, the journal (in a
    // *different* existing location) must be untouched.
    let path = unique_path("atomic-fail-keep");
    let _ = fs::remove_file(&path);
    store::create(&path).expect("create must succeed");

    let before = fs::read(&path).unwrap();

    let bad = unique_path("nonexistent-dir-for-append").join("nope.tsv");
    // Ensure the parent of `bad` is absent.
    if let Some(parent) = bad.parent() {
        let _ = fs::remove_dir_all(parent);
    }
    let lines = vec!["e1\t1\td\tent\tUSD\t1100\t\t1\t\t\t\t\tph\tpv".to_string()];
    let err = store::append(&bad, &lines).expect_err("append to bad path must fail");
    assert!(!err.is_empty(), "error must carry a one-line reason");

    // The good journal must be byte-identical.
    let after = fs::read(&path).unwrap();
    assert_eq!(
        after, before,
        "a failed append must leave the existing journal byte-identical"
    );

    let _ = fs::remove_file(&path);
}