//
// One behavior under test: `post --journal PATH --check` does not
// touch the journal. Live append lives in tests/post_appends.rs.
//
// History: an earlier commit in this stack carried a test that
// asserted `post --journal PATH` did NOT write to the journal
// (because persistence wasn't wired yet). Once commit 5 wires live
// append, that test's "no-write" claim is wrong by definition;
// the live-append assertion now lives in post_appends.rs and the
// --check dry-run fence stays here. The earlier test was removed,
// not silently relaxed, per CONVENTIONS.md "say so explicitly".
//
// SRD: \"The kernel of the system: take what `post --check` validates
// and actually append it\" — the live-append half is tested in
// post_appends.rs; the dry-run half is fenced here.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_post"))
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

#[test]
fn post_journal_with_check_does_not_touch_the_journal() {
    // This is the explicit dry-run contract: --check stays dry even
    // when --journal is given. A future commit will introduce live
    // append (no --check); THIS commit's regression fence is that
    // --check remains no-write.
    let journal = unique_path("flag-check");
    let _ = fs::remove_file(&journal);

    new_project::store::create(&journal).expect("create must succeed");

    // Note the BEFORE content byte-for-byte so we can prove no write.
    let before = fs::read(&journal).expect("read before");
    let before_len = before.len();

    let proposed = format!(
        "{h}\n\
         e1\t1\t2026-01-01\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\t2026-01-01\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );

    let mut child = bin()
        .arg("--journal")
        .arg(&journal)
        .arg("--check")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn `post --journal PATH --check`");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(proposed.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();

    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 on balanced input with --check; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let after = fs::read(&journal).expect("read after");
    assert_eq!(
        after, before,
        "--check must leave the journal byte-identical; before={before:?} after={after:?}"
    );
    assert_eq!(
        after.len(),
        before_len,
        "--check must not change the journal size"
    );

    let _ = fs::remove_file(&journal);
}