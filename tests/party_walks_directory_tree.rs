//
// tests/party_walks_directory_tree.rs
//
//! Pin the contract for `party::walk` -- the same shape as
//! `coa::walk`, applied to /biz/parties/{id}/. A leaf is a
//! directory containing profile.tsv. Sorted by path.

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

#[test]
fn party_walk_lists_every_party_under_root() {
    let tmp = fresh_tempdir("party-walk");
    fs::create_dir_all(tmp.join("cust:123")).unwrap();
    fs::write(
        tmp.join("cust:123/profile.tsv"),
        "id\tname\tkind\tterms\ncust:123\tAcme\tcustomer\tNet-30\n",
    ).unwrap();
    fs::create_dir_all(tmp.join("cust:456")).unwrap();
    fs::write(
        tmp.join("cust:456/profile.tsv"),
        "id\tname\tkind\tterms\ncust:456\tBeta\tcustomer\tNet-30\n",
    ).unwrap();
    let parties = new_project::party::walk(&tmp).expect("walk must succeed");
    assert_eq!(
        parties,
        vec![PathBuf::from("cust:123"), PathBuf::from("cust:456")],
        "walk returns every party, sorted"
    );
}

#[test]
fn party_walk_on_empty_root_returns_empty_list() {
    let tmp = fresh_tempdir("party-empty");
    let parties = new_project::party::walk(&tmp).expect("walk on empty");
    assert!(parties.is_empty());
}

#[test]
fn party_walk_skips_directories_without_profile_tsv() {
    // A directory without profile.tsv is a group, not a party.
    let tmp = fresh_tempdir("party-skip");
    fs::create_dir_all(tmp.join("group")).unwrap(); // group, no profile
    fs::create_dir_all(tmp.join("group/cust:123")).unwrap();
    fs::write(
        tmp.join("group/cust:123/profile.tsv"),
        "id\tname\tkind\tterms\ncust:123\tAcme\tcustomer\tNet-30\n",
    ).unwrap();
    let parties = new_project::party::walk(&tmp).expect("walk must succeed");
    assert_eq!(parties, vec![PathBuf::from("group/cust:123")]);
}
