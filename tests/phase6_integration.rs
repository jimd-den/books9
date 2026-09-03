//
// tests/phase6_integration.rs
//
//! End-to-end integration test for the Phase 6 surface:
//! bom, mrp, org, payroll, wo. The full O2C -> MRP ->
//! WO -> Payroll loop on a populated filesystem.

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

#[test]
fn phase6_end_to_end() {
    let tmp = fresh_tempdir("phase6-e2e");

    // 1. Register a department.
    let out = Command::new("target/debug/org")
        .arg("new")
        .arg("--root").arg(tmp.join("org"))
        .arg("--code").arg("d:1")
        .arg("--name").arg("Engineering")
        .arg("--parent").arg("")
        .output()
        .expect("org new");
    assert!(out.status.success(), "org new: stderr={}", String::from_utf8_lossy(&out.stderr));

    // 2. Register a BOM: widget needs 2 steel.
    let out = Command::new("target/debug/bom")
        .arg("new")
        .arg("--root").arg(&tmp)
        .arg("--item").arg("widget")
        .arg("--component").arg("steel")
        .arg("--qty").arg("2")
        .arg("--uom").arg("kg")
        .output()
        .expect("bom new");
    assert!(out.status.success(), "bom new: stderr={}", String::from_utf8_lossy(&out.stderr));

    // 3. Run mrp: same inputs produce same bytes (FR-3 golden file).
    let bom_root = tmp.clone();
    let so_root = tmp.join("biz");
    fs::create_dir_all(so_root.join("docs").join("so")).expect("create so dir");
    fs::write(so_root.join("docs/so/000421.priced.tsv"),
        "so_id\tparty\tdate\tcurrency\tterms\tsku\tqty\tunit_price_minor\tline_total_minor\n         000421\tcust:1\t2026-09-01\tUSD\tNet-30\twidget\t10\t100\t1000\n").expect("write");
    let out1 = Command::new("target/debug/mrp")
        .args(&["--demand", "so:000421", "--bom-root", bom_root.to_str().unwrap(),
               "--so-root", so_root.to_str().unwrap()])
        .output()
        .expect("first mrp run");
    let out2 = Command::new("target/debug/mrp")
        .args(&["--demand", "so:000421", "--bom-root", bom_root.to_str().unwrap(),
               "--so-root", so_root.to_str().unwrap()])
        .output()
        .expect("second mrp run");
    assert!(out1.status.success() && out2.status.success(), "both mrp runs exit 0");
    let s1 = String::from_utf8(out1.stdout).expect("utf8");
    let s2 = String::from_utf8(out2.stdout).expect("utf8");
    assert_eq!(s1, s2, "FR-3: same inputs, same output bytes");
    assert!(s1.contains("steel\t20\tkg"), "mrp says 20 steel: {s1}");

    // 4. Run wo to make 5 widgets: emits a balanced backflush.
    let out = Command::new("target/debug/wo")
        .args(&["new",
            "--root", tmp.to_str().unwrap(),
            "--wo-id", "000001",
            "--item", "widget",
            "--qty", "5",
            "--date", "2026-09-15",
        ])
        .output()
        .expect("wo new");
    assert!(out.status.success(), "wo new: stderr={}", String::from_utf8_lossy(&out.stderr));
    let wo_path = tmp.join("docs/wo/000001.tsv");
    assert!(wo_path.is_file(), "WO file exists");

    // 5. Run payroll: gross - deductions = net, journal lines balance.
    fs::write(tmp.join("hours.tsv"),
        "employee\thours\trate\tcost_center\nemp:1\t40\t25\td:1\n").expect("write");
    fs::write(tmp.join("deductions.tsv"),
        "employee\tdeduction\nemp:1\t200\n").expect("write");
    let out = Command::new("target/debug/payroll")
        .args(&["--hours", tmp.join("hours.tsv").to_str().unwrap(),
               "--deductions", tmp.join("deductions.tsv").to_str().unwrap()])
        .output()
        .expect("payroll");
    assert!(out.status.success(), "payroll: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 4, "header + 3 legs: {stdout}");
    // Balance check: total_dr == total_cr.
    let total_dr: i64 = lines[1..].iter()
        .filter_map(|l| {
            let cols: Vec<&str> = l.split('\t').collect();
            if !cols[5].is_empty() { cols[7].parse::<i64>().ok() } else { None }
        }).sum();
    let total_cr: i64 = lines[1..].iter()
        .filter_map(|l| {
            let cols: Vec<&str> = l.split('\t').collect();
            if !cols[6].is_empty() { cols[7].parse::<i64>().ok() } else { None }
        }).sum();
    assert_eq!(total_dr, total_cr, "FR-4: payroll journal lines balance");
    assert_eq!(total_dr, 1000, "gross = 1000");
}
