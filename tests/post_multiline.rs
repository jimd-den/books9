//
// One behavior under test: a single logical entry that spans N data
// lines produces a continuous chain within the entry. The entry_id
// stays the same across lines; line k's prev_hash equals line k-1's
// provenance_hash; verify walks the chain without noticing where
// entry boundaries are.
//
// SRD: \"Multi-line entries: one logical posting spanning N data
// lines\" (Phase 1 acceptance criterion). The chain is the same
// primitive whether the next row belongs to the same entry or the
// next one; this commit pins that.
//
// Working rules: TDD, stdlib only, one behavior per commit.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn post_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_post"))
}

fn verify_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_verify"))
}

fn unique_path(tag: &str) -> PathBuf {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};
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

fn header() -> &'static str {
    "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash"
}

fn post(journal: &PathBuf, proposed: &str) -> std::process::Output {
    let mut child = post_bin()
        .arg("--journal")
        .arg(journal)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(proposed.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn multi_line_entry_chains_continuously_within_the_entry() {
    let path = unique_path("multiline-chain");
    let _ = fs::remove_file(&path);
    new_project::store::create(&path).expect("create must succeed");

    // A 4-leg USD entry: 1 debit, 3 credits. Same entry_id (e1)
    // across all four rows. The chain must link rows 1→2→3→4.
    let proposed = format!(
        "{h}\n\
         e1\t1\td\tent\tUSD\t1100\t\t300\t\t\t\t\th0\n\
         e1\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1\n\
         e1\t3\td\tent\tUSD\t\t2101\t100\t\t\t\t\th2\n\
         e1\t4\td\tent\tUSD\t\t2102\t100\t\t\t\t\th3\n",
        h = header()
    );

    let out = post(&path, &proposed);
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 on balanced 4-leg entry; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Read back: 4 data lines, same entry_id, prov/prev chain within
    // the entry is continuous.
    let content = fs::read_to_string(&path).unwrap();
    let mut lines = content.lines();
    lines.next(); // header
    let rows: Vec<Vec<String>> = lines.map(|l| l.split('\t').map(String::from).collect()).collect();
    assert_eq!(rows.len(), 4, "must be 4 data lines");

    // Same entry_id across all four rows.
    for row in &rows {
        assert_eq!(row[0], "e1", "all rows share the entry_id 'e1'");
    }

    // Row 0 prev_hash is zero.
    assert_eq!(rows[0][12], "0000000000000000", "row 0 prev_hash is zero");

    // Row k prev_hash == row k-1 provenance_hash, for k >= 1.
    for k in 1..rows.len() {
        assert_eq!(
            rows[k][12], rows[k - 1][11],
            "row {k} prev_hash must equal row {} provenance_hash",
            k - 1
        );
    }

    let _ = fs::remove_file(&path);
}

#[test]
fn verify_walks_multi_line_entries_indistinguishably_from_separate_entries() {
    // Two cases that should produce an identical chain from verify's
    // perspective: (a) one 4-leg entry (4 lines, same entry_id),
    // (b) two 2-leg entries (4 lines, two entry_ids). Both have the
    // same chain semantics: row k's prev_hash = row k-1's prov. We
    // append a (a) journal, run verify (clean), then a (b) journal,
    // run verify (clean). Both must exit 0.
    let path_a = unique_path("multiline-a");
    let path_b = unique_path("multiline-b");
    let _ = fs::remove_file(&path_a);
    let _ = fs::remove_file(&path_b);
    new_project::store::create(&path_a).expect("create a");
    new_project::store::create(&path_b).expect("create b");

    // (a) single 4-leg entry
    let a = format!(
        "{h}\n\
         e1\t1\td\tent\tUSD\t1100\t\t300\t\t\t\t\th0\n\
         e1\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1\n\
         e1\t3\td\tent\tUSD\t\t2101\t100\t\t\t\t\th2\n\
         e1\t4\td\tent\tUSD\t\t2102\t100\t\t\t\t\th3\n",
        h = header()
    );
    let out = post(&path_a, &a);
    assert_eq!(out.status.code(), Some(0), "post a: {}", String::from_utf8_lossy(&out.stderr));

    // (b) two 2-leg entries (entry_ids e1 and e2)
    let b1 = format!(
        "{h}\n\
         e1\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );
    let out = post(&path_b, &b1);
    assert_eq!(out.status.code(), Some(0), "post b1: {}", String::from_utf8_lossy(&out.stderr));
    let b2 = format!(
        "{h}\n\
         e2\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e2\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );
    let out = post(&path_b, &b2);
    assert_eq!(out.status.code(), Some(0), "post b2: {}", String::from_utf8_lossy(&out.stderr));

    // Verify both journals.
    let verify_a = verify_bin()
        .arg(&path_a)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(
        verify_a.status.code(),
        Some(0),
        "verify on multi-line single-entry journal must pass; stderr: {}",
        String::from_utf8_lossy(&verify_a.stderr)
    );

    let verify_b = verify_bin()
        .arg(&path_b)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(
        verify_b.status.code(),
        Some(0),
        "verify on multi-entry journal must pass; stderr: {}",
        String::from_utf8_lossy(&verify_b.stderr)
    );

    // Both journals hold 4 data lines. Read back and compare the
    // chain structure: row k's prev_hash equals row k-1's prov_hash
    // in both cases.
    for path in [&path_a, &path_b] {
        let content = fs::read_to_string(path).unwrap();
        let rows: Vec<Vec<String>> = content
            .lines()
            .skip(1) // header
            .map(|l| l.split('\t').map(String::from).collect())
            .collect();
        assert_eq!(rows.len(), 4, "{}: must be 4 data lines", path.display());
        assert_eq!(rows[0][12], "0000000000000000", "{}: row 0 prev zero", path.display());
        for k in 1..rows.len() {
            assert_eq!(
                rows[k][12], rows[k - 1][11],
                "{}: chain link broken at row {k}", path.display()
            );
        }
    }

    let _ = fs::remove_file(&path_a);
    let _ = fs::remove_file(&path_b);
}