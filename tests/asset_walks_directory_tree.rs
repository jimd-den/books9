//
// tests/asset_walks_directory_tree.rs
//
//! Pin the contract for `asset::walk` -- the same shape as
//! `coa::walk`, applied to /biz/assets/{id}/. A leaf is a
//! directory containing profile.tsv.

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
fn asset_walk_lists_every_asset() {
    let tmp = fresh_tempdir("asset-walk");
    for id in &["ast:1", "ast:2"] {
        let dir = tmp.join(id);
        fs::create_dir_all(&dir).expect("create asset dir");
        fs::write(dir.join("profile.tsv"),
            "id\tname\tcost_minor\tacquired\tuseful_life_months\tsalvage_minor\n             {id}\tAsset\t1000000\t2024-01-15\t60\t100000\n").expect("write");
    }
    let assets = new_project::asset::walk(&tmp).expect("walk");
    assert_eq!(assets, vec![PathBuf::from("ast:1"), PathBuf::from("ast:2")]);
}
