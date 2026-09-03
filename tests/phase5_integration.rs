//
// tests/phase5_integration.rs
//
//! End-to-end integration test for the Phase 5 O2C surface.
//!
//! The loop: create a CoA, register a customer and an item,
//! create a sales order, price it, invoice it, post it,
//! and read the AR aging. Every tool is a separate process;
//! the only state they share is the filesystem.

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

fn run(bin: &str, args: &[&str]) -> std::process::Output {
    let path = format!("target/debug/{bin}");
    Command::new(&path)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {bin}: {e}"))
}

#[test]
fn phase5_end_to_end() {
    let tmp = fresh_tempdir("phase5-e2e");
    let coa = tmp.join("coa");
    let biz = tmp.join("biz");
    let journal = tmp.join("journal.tsv");

    // 1. Create the CoA: 1100 (AR) and 4000 (Sales).
    for (acct, name, kind) in [("1100", "AR", "asset"), ("4000", "Sales", "revenue")] {
        let out = run("coa", &["new",
            "--root", coa.to_str().unwrap(),
            acct,
            "--name", name,
            "--kind", kind,
            "--normal-side", if acct == "1100" { "debit" } else { "credit" },
        ]);
        assert!(out.status.success(), "coa new {acct}: stderr={}", String::from_utf8_lossy(&out.stderr));
    }

    // 2. Register a customer.
    let out = run("party", &["new",
        "--root", biz.join("parties").to_str().unwrap(),
        "--id", "cust:123",
        "--name", "Acme Co.",
        "--kind", "customer",
        "--terms", "Net-30",
    ]);
    assert!(out.status.success(), "party new: stderr={}", String::from_utf8_lossy(&out.stderr));

    // 3. Register two items.
    for (id, name, price) in [("sku:77", "Widget", "100"), ("sku:88", "Gadget", "250")] {
        let out = run("item", &["new",
            "--root", biz.join("items").to_str().unwrap(),
            "--id", id,
            "--name", name,
            "--uom", "each",
            "--default-price", price,
        ]);
        assert!(out.status.success(), "item new {id}: stderr={}", String::from_utf8_lossy(&out.stderr));
    }

    // 3.5. Bootstrap the journal with a header-only TSV.
    fs::write(&journal, "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash\n").expect("bootstrap journal");

    // 4. Create a sales order.
    let out = run("so", &["new",
        "--root", biz.to_str().unwrap(),
        "--so-id", "000421",
        "--party", "cust:123",
        "--date", "2026-09-01",
        "--currency", "USD",
        "--terms", "Net-30",
        "--line", "sku:77,40",
        "--line", "sku:88,10",
    ]);
    assert!(out.status.success(), "so new: stderr={}", String::from_utf8_lossy(&out.stderr));

    // 5. Price the SO.
    let out = run("price", &["--root", biz.to_str().unwrap(), "--items-root", biz.join("items").to_str().unwrap(), "--so", "000421"]);
    assert!(out.status.success(), "price: stderr={}", String::from_utf8_lossy(&out.stderr));

    // 6. Generate the invoice.
    let out = run("invoice", &[
        "--root", biz.to_str().unwrap(),
        "--so", "000421",
        "--coa-root", coa.to_str().unwrap(),
        "--journal", journal.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "invoice: stderr={}", String::from_utf8_lossy(&out.stderr));
    let invoice_stdout = String::from_utf8(out.stdout).expect("utf8");
    let invoice_lines: Vec<&str> = invoice_stdout.lines().collect();
    assert_eq!(invoice_lines.len(), 3, "header + 2 rows: {invoice_stdout}");

    // 7. Post the invoice (use the proposal as stdin).
    use std::io::Write;
    let mut child = Command::new("target/debug/post")
        .args(&["--journal", journal.to_str().unwrap(), "--coa", coa.to_str().unwrap()])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(invoice_stdout.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "post: stderr={}", String::from_utf8_lossy(&out.stderr));

    // 8. AR aging on the populated journal.
    let out = run("ar_aging", &[
        "--journal", journal.to_str().unwrap(),
        "--as-of", "2026-09-30",
    ]);
    assert!(out.status.success(), "ar aging: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(stdout.contains("0-30"), "ar aging has 0-30 bucket: {stdout}");
    assert!(stdout.contains("6500"), "ar aging has 6500 (4000 + 2500): {stdout}");
}
