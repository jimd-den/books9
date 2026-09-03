//
// tests/coa_walks_directory_tree.rs
//
//! Pin the CoA-as-directory-tree contract.
//!
//! The CoA is a tree under /biz/ledger/accounts/. A leaf is a
//! directory containing profile.tsv. Walking the tree returns
//! every account path in sorted order. This is the only way
//! `post --coa` and `coa ls` know what accounts exist.
//!
//! The walk is pure: same root, same result. No state, no IO
//! beyond read_dir.
//!
//! Failing test for the first TDD commit of Phase 3. The
//! implementation lands in the next commit; this commit pins
//! the contract so the implementation has a target.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_tempdir(label: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("books9-{label}-{pid}-{n}"));
    fs::create_dir_all(&dir).expect("create tempdir");
    dir
}

/// Seed three accounts at three depths, each with a profile.tsv
/// so the walker knows it is a leaf (presence of profile.tsv is
/// the existence proof; see plans/phase3-coa-reports.md §5).
fn seed_three_accounts(root: &PathBuf) {
    // Cash at the root: the simplest possible account.
    fs::create_dir_all(root.join("1100")).unwrap();
    fs::write(
        root.join("1100/profile.tsv"),
        "code\tname\tkind\n1100\tCash\tasset\n",
    )
    .unwrap();
    // Sales nested one level under a revenue group.
    fs::create_dir_all(root.join("4000/sales")).unwrap();
    fs::write(
        root.join("4000/sales/profile.tsv"),
        "code\tname\tkind\n4000\tSales\trevenue\n",
    )
    .unwrap();
    // AP deeply nested under a vendor group.
    fs::create_dir_all(root.join("2100/ap/vendors")).unwrap();
    fs::write(
        root.join("2100/ap/vendors/profile.tsv"),
        "code\tname\tkind\n2100\tAP\tliability\n",
    )
    .unwrap();
}

#[test]
fn coa_walk_lists_every_account_under_root() {
    let tmp = fresh_tempdir("coa-walk");
    seed_three_accounts(&tmp);

    let accounts = new_project::coa::walk(&tmp)
        .expect("walk must succeed on a well-formed CoA tree");

    assert_eq!(
        accounts,
        vec![
            PathBuf::from("1100"),
            PathBuf::from("2100/ap/vendors"),
            PathBuf::from("4000/sales"),
        ],
        "walk returns every leaf account, in sorted path order"
    );
}

#[test]
fn coa_walk_on_empty_root_returns_empty_list() {
    let tmp = fresh_tempdir("coa-empty");
    // No accounts at all. The walker should not invent any.
    let accounts = new_project::coa::walk(&tmp)
        .expect("walk on an empty root must succeed");
    assert!(
        accounts.is_empty(),
        "no accounts means an empty result, not an error"
    );
}

#[test]
fn coa_walk_skips_directories_without_profile_tsv() {
    // A directory without profile.tsv is a group, not an account.
    // It must not appear in the result; only the leaf accounts do.
    let tmp = fresh_tempdir("coa-skip");
    fs::create_dir_all(tmp.join("1000/group")).unwrap(); // group, no profile
    fs::create_dir_all(tmp.join("1000/group/1100")).unwrap();
    fs::write(
        tmp.join("1000/group/1100/profile.tsv"),
        "code\tname\tkind\n1100\tCash\tasset\n",
    )
    .unwrap();

    let accounts = new_project::coa::walk(&tmp).expect("walk must succeed");
    assert_eq!(
        accounts,
        vec![PathBuf::from("1000/group/1100")],
        "groups (no profile.tsv) are skipped; only leaves appear"
    );
}


