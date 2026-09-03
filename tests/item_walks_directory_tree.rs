//
// tests/item_walks_directory_tree.rs
//
//! Pin the contract for `item::walk` -- the same shape as
//! `coa::walk` and `party::walk`, applied to /biz/items/{id}/.

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
fn item_walk_lists_every_item_under_root() {
    let tmp = fresh_tempdir("item-walk");
    fs::create_dir_all(tmp.join("sku:77")).unwrap();
    fs::write(
        tmp.join("sku:77/profile.tsv"),
        "id\tname\tuom\tdefault_price\nsku:77\tWidget\teach\t100\n",
    ).unwrap();
    fs::create_dir_all(tmp.join("sku:88")).unwrap();
    fs::write(
        tmp.join("sku:88/profile.tsv"),
        "id\tname\tuom\tdefault_price\nsku:88\tGadget\teach\t200\n",
    ).unwrap();
    let items = new_project::item::walk(&tmp).expect("walk must succeed");
    assert_eq!(items, vec![PathBuf::from("sku:77"), PathBuf::from("sku:88")]);
}

#[test]
fn item_walk_skips_directories_without_profile_tsv() {
    let tmp = fresh_tempdir("item-skip");
    fs::create_dir_all(tmp.join("group")).unwrap();
    fs::create_dir_all(tmp.join("group/sku:77")).unwrap();
    fs::write(
        tmp.join("group/sku:77/profile.tsv"),
        "id\tname\tuom\tdefault_price\nsku:77\tWidget\teach\t100\n",
    ).unwrap();
    let items = new_project::item::walk(&tmp).expect("walk");
    assert_eq!(items, vec![PathBuf::from("group/sku:77")]);
}
