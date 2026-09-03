//
// One behavior per test (working rules §6).
//   test 1: post rejects an entry with one side missing
//   test 2: post rejects a multi-currency entry where one currency
//           does not balance, with a per-currency one-line reason
//
// SRD references: FR-1, "rejections never partially append".

use std::io::Write;
use std::process::{Command, Stdio};

#[allow(unused_imports)]
use Command as _Command;

fn bin() -> Command {
    // The `post` binary is the userland tool that funnels balanced
    // journal lines through the validator. Until Phase 0 lands the
    // daemon, the validator is the testable surface.
    Command::new(env!("CARGO_BIN_EXE_post"))
}

/// Helper: run `post --check` with `proposed` on stdin, return the
/// captured (status, stdout, stderr). Caller asserts on the pieces.
fn run_check(proposed: &str) -> std::process::Output {
    let mut child = bin()
        .arg("--check") // dry run: must not touch the journal
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn `post --check`");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(proposed.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn post_rejects_unbalanced_entry() {
    // Proposed entry: 100 debit to cash, 0 credit. Unbalanced.
    let proposed = "\
entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash
e1\t1\t2026-01-01\tent:1\tUSD\t1000\t\t100\t\t\t\t\t0000000000000000
";

    let out = run_check(proposed);

    // SRD: exits 0/nonzero with a one-line stderr reason.
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected nonzero exit; got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.lines().count() == 1 && stderr.contains("unbalanced"),
        "expected one-line stderr mentioning 'unbalanced'; got: {stderr}"
    );
    assert!(
        out.stdout.is_empty(),
        "stdout must stay clean for piping; got: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn post_rejects_per_currency_mismatch_in_multi_leg_entry() {
    // Four one-sided legs (validator requires a leg to be one
    // debit OR one credit, never both):
    //   line 1: USD, 100 debit 1100
    //   line 2: USD, 100 credit 2100           -> USD balanced
    //   line 3: EUR, 200 debit 1100
    //   line 4: EUR, 199 credit 2100           -> EUR off by 1
    // The validator must reject the whole entry, name the offending
    // currency (EUR), and never write anything.
    let proposed = "\
entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash
e1\t1\t2026-01-01\tent:1\tUSD\t1100\t\t100\t\t\t\t\t0000000000000000
e1\t2\t2026-01-01\tent:1\tUSD\t\t2100\t100\t\t\t\t\t0000000000000001
e1\t3\t2026-01-01\tent:1\tEUR\t1100\t\t200\t\t\t\t\t0000000000000002
e1\t4\t2026-01-01\tent:1\tEUR\t\t2100\t199\t\t\t\t\t0000000000000003
";

    let out = run_check(proposed);

    assert_eq!(
        out.status.code(),
        Some(2),
        "expected nonzero exit; got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.lines().count() == 1,
        "expected one-line stderr; got {stderr:?}"
    );
    assert!(
        stderr.contains("unbalanced") && stderr.contains("EUR"),
        "stderr must name the offending currency; got: {stderr}"
    );
    assert!(
        !stderr.contains("USD"),
        "stderr must not mention the currency that did balance; got: {stderr}"
    );
    assert!(
        out.stdout.is_empty(),
        "stdout must stay clean for piping; got: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}
