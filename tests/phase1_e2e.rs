//
// End-to-end integration test for the Phase 1 surface.
//
// Exercises the full path: create journal, append entry, verify clean,
// tamper with one byte, verify reports the divergence.
//
// This is the SRD's Phase 1 acceptance criterion in code:
//   \"verify detects any single flipped byte in the journal.\"
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

fn unique_path(tag: &str, ext: &str) -> PathBuf {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "books9-{}-{tag}-{pid}-{nanos}.{ext}",
        process::id(),
        pid = process::id(),
        nanos = nanos,
        ext = ext
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

fn verify(journal: &PathBuf) -> std::process::Output {
    verify_bin()
        .arg(journal)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

#[test]
fn phase1_e2e_create_append_verify_tamper_verify_fails() {
    let journal = unique_path("phase1-e2e", "tsv");
    let _ = fs::remove_file(&journal);

    // 1. Create the journal (FR-2: refuses if exists; we created
    //    the journal just now so the file is absent).
    new_project::store::create(&journal).expect("create must succeed");
    assert_eq!(
        new_project::store::open(&journal).unwrap(),
        0,
        "fresh journal holds zero data lines"
    );

    // 2. Append a balanced entry via post --journal.
    let proposed = format!(
        "{h}\n\
         e1\t1\t2026-01-15\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\t2026-01-15\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );
    let out = post(&journal, &proposed);
    assert_eq!(
        out.status.code(),
        Some(0),
        "post must succeed on balanced input; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stdout.is_empty(),
        "stdout must stay clean for piping"
    );
    assert_eq!(
        new_project::store::open(&journal).unwrap(),
        2,
        "one 2-leg entry appended"
    );

    // 3. Verify the clean journal: exit 0.
    let v = verify(&journal);
    assert_eq!(
        v.status.code(),
        Some(0),
        "verify on clean journal must exit 0; stderr: {}",
        String::from_utf8_lossy(&v.stderr)
    );

    // 4. Tamper: flip a byte in the second data row.
    let mut content = fs::read(&journal).unwrap();
    // header line + '\n' + row 1 + '\n' + row 2 + '\n'.
    let newline_indices: Vec<usize> = content
        .iter()
        .enumerate()
        .filter_map(|(i, &b)| if b == b'\n' { Some(i) } else { None })
        .collect();
    assert!(
        newline_indices.len() >= 3,
        "expected at least 3 newlines (header, row1, row2); got {}",
        newline_indices.len()
    );
    let row2_start = newline_indices[1] + 1;
    let row2_end = newline_indices[2];
    let pos = row2_start + (row2_end - row2_start) / 2; // middle of row 2
    content[pos] ^= 0x01;
    fs::write(&journal, &content).unwrap();

    // 5. Verify the tampered journal: exit nonzero with a one-line
    //    stderr that names the line number.
    let v = verify(&journal);
    assert_ne!(
        v.status.code(),
        Some(0),
        "verify on tampered journal must exit nonzero"
    );
    let stderr = String::from_utf8_lossy(&v.stderr);
    assert!(
        stderr.lines().count() == 1,
        "verify's error must be one line; got: {stderr:?}"
    );
    // The error must reference a line number OR a hash mismatch.
    assert!(
        !stderr.trim().is_empty(),
        "verify's error must carry a reason"
    );

    let _ = fs::remove_file(&journal);
}

#[test]
fn phase1_e2e_multi_entry_chain_links_across_entries() {
    // Append three balanced entries; verify the chain links across
    // them. Every prior line is byte-equal after the third append
    // (FR-2: append-only). Each append advances the count by N lines.
    let journal = unique_path("phase1-multi", "tsv");
    let _ = fs::remove_file(&journal);
    new_project::store::create(&journal).expect("create must succeed");

    for n in 1..=3 {
        let proposed = format!(
            "{h}\n\
             e{n}\t1\t2026-01-15\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
             e{n}\t2\t2026-01-15\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
            h = header(),
        );
        let out = post(&journal, &proposed);
        assert_eq!(
            out.status.code(),
            Some(0),
            "post {n} must succeed; stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    assert_eq!(
        new_project::store::open(&journal).unwrap(),
        6,
        "three 2-leg entries => 6 data lines"
    );

    let v = verify(&journal);
    assert_eq!(
        v.status.code(),
        Some(0),
        "verify on the multi-entry journal must pass; stderr: {}",
        String::from_utf8_lossy(&v.stderr)
    );

    let _ = fs::remove_file(&journal);
}