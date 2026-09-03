//
// tests/bom_walks_directory_tree.rs
//
//! Pin the contract for `bom::walk` -- the same shape as
//! `coa::walk`, applied to /biz/boms/{item}/. A leaf is a
//! directory containing bom.tsv.

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
fn bom_walk_lists_every_bom_bearing_item() {
    let tmp = fresh_tempdir("bom-walk");
    for item in &["widget", "gadget"] {
        let dir = tmp.join("boms").join(item);
        fs::create_dir_all(&dir).expect("create bom dir");
        fs::write(dir.join("bom.tsv"),
            "item\tcomponent\tqty_per_unit\tuom\nwidget\tsteel\t2\tkg\n").expect("write");
    }
    let boms = new_project::bom::walk(&tmp.join("boms")).expect("walk");
    assert_eq!(boms, vec![PathBuf::from("gadget"), PathBuf::from("widget")]);
}
