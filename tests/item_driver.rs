//
// tests/item_driver.rs
//
//! Pin the contract for the `item` driver: new/ls/show on
//! the items directory tree. Same shape as `party` driver.

use std::process::Command;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::fs;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_tempdir(label: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("books9-{label}-{pid}-{n}"));
    fs::create_dir_all(&dir).expect("create tempdir");
    dir
}

fn seed_item(tmp: &PathBuf, id: &str, name: &str, uom: &str, default_price: &str) {
    let dir = tmp.join(id);
    fs::create_dir_all(&dir).expect("create item dir");
    let body = format!(
        "id\tname\tuom\tdefault_price\n{id}\t{name}\t{uom}\t{default_price}\n"
    );
    fs::write(dir.join("profile.tsv"), body).expect("write profile");
}

#[test]
fn item_new_creates_a_leaf_with_profile_tsv() {
    let tmp = fresh_tempdir("item-new");
    let bin = env!("CARGO_BIN_EXE_item");
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--id").arg("sku:77")
        .arg("--name").arg("Widget")
        .arg("--uom").arg("each")
        .arg("--default-price").arg("100")
        .output()
        .expect("run item new");
    assert!(output.status.success(), "item new exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let leaf = tmp.join("sku:77");
    assert!(leaf.is_dir(), "leaf directory must exist: {leaf:?}");
    let profile = leaf.join("profile.tsv");
    let content = fs::read_to_string(&profile).expect("read profile");
    assert!(content.contains("sku:77"), "profile has id: {content}");
    assert!(content.contains("Widget"), "profile has name: {content}");
    assert!(content.contains("each"), "profile has uom: {content}");
    assert!(content.contains("100"), "profile has default_price: {content}");
}

#[test]
fn item_ls_lists_every_item() {
    let tmp = fresh_tempdir("item-ls");
    seed_item(&tmp, "sku:77", "Widget", "each", "100");
    seed_item(&tmp, "sku:88", "Gadget", "each", "200");
    let bin = env!("CARGO_BIN_EXE_item");
    let output = Command::new(bin)
        .arg("ls")
        .arg("--root").arg(&tmp)
        .output()
        .expect("run item ls");
    assert!(output.status.success(), "item ls exits 0");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 3, "header + 2 rows: {stdout}");
    assert!(lines[0].contains("id"), "header has id");
    assert!(lines[1].starts_with("sku:77") || lines[2].starts_with("sku:77"), "sku:77 row present");
}

#[test]
fn item_show_prints_one_item() {
    let tmp = fresh_tempdir("item-show");
    seed_item(&tmp, "sku:77", "Widget", "each", "100");
    let bin = env!("CARGO_BIN_EXE_item");
    let output = Command::new(bin)
        .arg("show")
        .arg("--root").arg(&tmp)
        .arg("sku:77")
        .output()
        .expect("run item show");
    assert!(output.status.success(), "item show exits 0");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("sku:77"), "show has id");
    assert!(stdout.contains("Widget"), "show has name");
    assert!(stdout.contains("each"), "show has uom");
    assert!(stdout.contains("100"), "show has default_price");
}
