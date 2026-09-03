//
// One behavior under test: the journal validation surface is reachable
// from the library, not only from the `post` binary.
//
// SRD reference: FR-1, "rejections never partially append" — the rule
// lives in the kernel module so every tool that funnels entries (post,
// close, import, ...) uses the same gate.
//
// Working rule: stdlib only; minimal diff; one behavior per commit.
//
// What this commit introduces:
//   - libbiz::journal::validate(&str) -> Result<(), String>
//     Same contract as the current bin/post::validate: parse header +
//     N data legs, reject if any currency does not balance.
//
// What this commit deliberately defers (each gets its own test):
//   - hash chain, on-disk persistence, periods, account existence,
//     multi-line entries, --journal flag parsing.

use new_project::journal;

fn header() -> &'static str {
    "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash"
}

#[test]
fn journal_validate_accepts_two_lided_balanced() {
    let s = format!(
        "{h}\n\
         e1\t1\t2026-01-01\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\t2026-01-01\tent:1\tUSD\t\t2100\t100\t\t\t\t\th1\n",
        h = header()
    );
    assert!(journal::validate(&s).is_ok(), "balanced entry must validate");
}

#[test]
fn journal_validate_rejects_when_one_side_missing() {
    let s = format!(
        "{h}\n\
         e1\t1\t2026-01-01\tent:1\tUSD\t1100\t\t100\t\t\t\t\th0\n",
        h = header()
    );
    let err = journal::validate(&s).unwrap_err();
    assert!(err.contains("unbalanced"), "got: {err}");
    assert!(err.contains("USD"), "got: {err}");
    assert!(!err.contains('\n'), "must be one line: {err}");
}

#[test]
fn journal_validate_rejects_per_currency_off_by_one() {
    // EUR off by 1; USD balanced. Validator must name EUR and not USD.
    let s = format!(
        "{h}\n\
         e1\t1\td\tent\tUSD\t1100\t\t100\t\t\t\t\th0\n\
         e1\t2\td\tent\tUSD\t\t2100\t100\t\t\t\t\th1\n\
         e1\t3\td\tent\tEUR\t1100\t\t200\t\t\t\t\th2\n\
         e1\t4\td\tent\tEUR\t\t2100\t199\t\t\t\t\th3\n",
        h = header()
    );
    let err = journal::validate(&s).unwrap_err();
    assert!(err.contains("EUR"), "got: {err}");
    assert!(!err.contains("USD"), "got: {err}");
}