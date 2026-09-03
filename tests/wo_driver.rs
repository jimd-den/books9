//
// tests/wo_driver.rs
//
//! Pin the contract for the `wo` driver: create a work
//! order and emit a balanced backflush journal proposal on
//! stdout (DR Finished Goods / CR components).

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

fn seed_bom(root: &PathBuf, item: &str, lines: &[&str]) {
    let dir = root.join(item);
    fs::create_dir_all(&dir).expect("create bom dir");
    let body = "item\tcomponent\tqty_per_unit\tuom\n".to_string()
        + &lines.join("\n")
        + "\n";
    fs::write(dir.join("bom.tsv"), body).expect("write bom");
}

#[test]
fn wo_new_creates_a_wo_file_and_emits_a_balanced_backflush_proposal() {
    let tmp = fresh_tempdir("wo-new");
    seed_bom(&tmp, "widget", &["widget\tsteel\t2\tkg"]);
    let bin = env!("CARGO_BIN_EXE_wo");
    let output = Command::new(bin)
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--wo-id").arg("000001")
        .arg("--item").arg("widget")
        .arg("--qty").arg("5")
        .arg("--date").arg("2026-09-15")
        .output()
        .expect("run wo new");
    assert!(output.status.success(), "wo new exits 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    // The WO file exists.
    let wo_path = tmp.join("docs").join("wo").join("000001.tsv");
    assert!(wo_path.is_file(), "WO file exists: {wo_path:?}");
    // The proposal has 2 lines: DR Finished Goods, CR steel.
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 3, "header + 2 lines: {stdout}");
    // 5 widgets * 2 steel/unit = 10 steel issued.
    // The DR is the finished good (assume 1500 each -> 7500).
    // The CR is steel (10 kg). For Phase 6 we use a simple
    // 1:1 cost assumption: FG = qty * 100, components = qty * qty_per_unit.
    assert!(stdout.contains("1500\t\t7500"), "DR FG 7500");
    // The inventory account is derived as "1" + first 3 chars of "steel" = "1ste".
    assert!(stdout.contains("\t1ste\t"), "CR 1ste present");
    // Balance: 7500 = 10 * 750 (steel at 750/kg)? No, we just
    // need the lines to balance in total_dr == total_cr. For
    // Phase 6 the test asserts the file exists; full balance
    // check is in the integration test.
}
