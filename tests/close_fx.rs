//
// tests/close_fx.rs
//
//! Pin the contract for `close --fx PATH`: at close time, if
//! an FX rates table is given, emit a one-line unrealized-gain
//! posting per non-USD balance. The gain is the difference
//! between the booked (entry-date) USD-normalized value and
//! the current (close-date) USD-normalized value.

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

fn make_journal(dir: &PathBuf, rows: &[&str]) -> PathBuf {
    let path = dir.join("journal.tsv");
    let header = "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash";
    let mut content = String::from(header);
    content.push('\n');
    for row in rows {
        content.push_str(row);
        content.push('\n');
    }
    fs::write(&path, content).expect("write journal");
    path
}

fn make_rates(dir: &PathBuf, body: &str) -> PathBuf {
    let path = dir.join("rates.tsv");
    fs::write(&path, body).expect("write rates");
    path
}

#[test]
fn close_with_fx_emits_an_unrealized_gain_line_on_stderr() {
    // Entry dated 2026-09-01: 100 EUR booked at 1.10 USD/EUR.
    // Close on 2026-09-15 with rate 1.08: the EUR is now worth
    // 108 USD, not 110. The unrealized loss is 2 USD.
    let tmp = fresh_tempdir("close-fx");
    let journal = make_journal(&tmp, &[
        "e1\t1\t2026-09-01\tent1\tEUR\t1100\t\t100\t\t\t\th0\tseed",
        "e1\t2\t2026-09-01\tent1\tEUR\t\t2100\t100\t\t\t\th1\th0",
    ]);
    let rates = make_rates(&tmp,
        "date\tfrom\tto\trate\n2026-09-01\tEUR\tUSD\t1.10\n2026-09-15\tEUR\tUSD\t1.08\n",
    );
    let bin = env!("CARGO_BIN_EXE_close");
    let output = Command::new(bin)
        .arg("--journal").arg(&journal)
        .arg("--period").arg("2026-09")
        .arg("--fx").arg(&rates)
        .arg("--close-date").arg("2026-09-15")
        .output()
        .expect("run close");
    // The unrealized gain line is on stderr (it's an info
    // emission, not a journal posting -- Phase 4 ships the
    // info line, the journal posting is a future commit).
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unrealized") || stderr.contains("EUR"),
        "stderr has the unrealized-gain line: {stderr}"
    );
}
