//
// Pins FR-2: the journal is never edited or deleted; corrections are
// reversing entries only. The way this surfaces in code is that the
// `store` module exposes exactly four public functions and no others:
//   - create(path)
//   - append(path, lines)        -- the only write path
//   - open(path)                -- read-only
//   - last_hash(path)           -- read-only
//
// There is no `truncate`, no `edit`, no `rewrite`, no `delete`. If a
// future commit adds one, this test fails loudly — which is the point.
//
// This is a contract test, not a behavior test. It exists because
// FR-2 is the rule that the whole system rests on; if it ever
// silently regresses, every audit property (tamper-evidence, the
// "the audit trail is the data" pillar, replayability) falls with it.

use std::collections::BTreeSet;

#[test]
fn store_exposes_only_the_four_fr2_safe_functions() {
    let mut found: BTreeSet<&'static str> = BTreeSet::new();
    for item in new_project::store::__inventory() {
        found.insert(item);
    }
    let want: BTreeSet<&'static str> = ["create", "open", "append", "last_hash"]
        .iter()
        .copied()
        .collect();
    assert_eq!(found, want, "FR-2 surface changed: {found:?}");
}