//
// Phase 2 end-to-end: the whole kernel story in one pipe's length.
//
//   create -> post -> verify green -> close -> post refused (FR-1:
//   nothing appends) -> reverse (FR-2: the answer, not the eraser)
//   -> verify still green
//
// Every step uses the real binaries over real temp files. This is
// the acceptance criterion for the phase, not a unit in disguise:
// the gate, the close, the stamp, the refusal message, the mirror,
// and the chain have to agree with each other, and only a walk
// across all of them proves they do.
//
// Working rules: TDD, stdlib only, one behavior per commit.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn post_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_post"))
}
fn close_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_close"))
}
fn reverse_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_reverse"))
}
fn verify_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_verify"))
}

fn unique_dir(tag: &str) -> PathBuf {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "books9-{}-{tag}-{pid}-{nanos}",
        process::id(),
        pid = process::id(),
        nanos = nanos
    ));
    fs::create_dir_all(&dir).expect("create isolated temp dir");
    dir
}

fn header() -> &'static str {
    "entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash"
}

fn entry(id: &str, date: &str) -> String {
    format!(
        "{h}\n\
         {id}\t1\t{d}\tent:1\tUSD\t1100\t\t10000\tp:1\tdoc:{id}\t\t\th?\n\
         {id}\t2\t{d}\tent:1\tUSD\t\t2100\t10000\tp:1\tdoc:{id}\t\t\th?\n",
        h = header(),
        id = id,
        d = date
    )
}

fn run(cmd: &mut Command, stdin: Option<&str>) -> std::process::Output {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    match stdin {
        Some(s) => {
            child.stdin.as_mut().unwrap().write_all(s.as_bytes()).unwrap();
        }
        None => {
            drop(child.stdin.take());
        }
    }
    child.wait_with_output().unwrap()
}

fn verify_ok(journal: &Path) {
    let out = run(verify_bin().arg(journal), None);
    assert_eq!(
        out.status.code(),
        Some(0),
        "verify must pass; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn phase2_walk_close_then_refuse_then_reverse() {
    let dir = unique_dir("e2e-walk");
    let journal = dir.join("journal.tsv");

    // 1. create: one header, no lies.
    new_project::store::create(&journal).expect("create journal");
    assert_eq!(new_project::store::open(&journal).unwrap(), 0);

    // 2. post a balanced entry into 2026-01.
    let out = run(
        post_bin().arg("--journal").arg(&journal),
        Some(&entry("e1", "2026-01-15")),
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "post must succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(new_project::store::open(&journal).unwrap(), 2);
    verify_ok(&journal);

    // 3. close 2026-01. Snapshot on stdout; flag and stamp on disk.
    let out = run(
        close_bin()
            .arg("--journal")
            .arg(&journal)
            .arg("--period")
            .arg("2026-01"),
        None,
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "close must succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let snapshot = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(snapshot.starts_with("close_id\tperiod\t"), "snapshot header: {snapshot}");
    assert!(snapshot.contains("USD"), "snapshot names the currency");
    let periods_root = new_project::store::periods_root(&journal);
    assert_eq!(fs::read_to_string(periods_root.join("2026-01")).unwrap().trim(), "closed");
    assert!(periods_root.join(".2026-01.last_close").exists(), "close recorded the hour");

    // The journal is untouched by a close: it reads the books, it
    // does not write them.
    verify_ok(&journal);

    // 4. post into the closed period: refused loudly, named fully,
    //    and the books do not even feel it (FR-1).
    let before = fs::read_to_string(&journal).unwrap();
    let out = run(
        post_bin()
            .arg("--journal")
            .arg(&journal)
            .arg("--periods")
            .arg(&periods_root),
        Some(&entry("e2", "2026-01-20")),
    );
    assert_eq!(out.status.code(), Some(2), "closed period must refuse e2");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(stderr.lines().count(), 1, "one-line reason: {stderr}");
    assert!(stderr.contains("2026-01"), "names the period: {stderr}");
    assert!(stderr.contains("2026-01-20"), "names the entry date: {stderr}");
    let stamp = fs::read_to_string(periods_root.join(".2026-01.last_close")).unwrap();
    let stamp = stamp.trim().to_string();
    assert!(stderr.contains(&stamp), "names the hour the door shut: {stderr}");
    assert_eq!(fs::read_to_string(&journal).unwrap(), before, "FR-1: refusal changed nothing");

    // 5. e1 was wrong. Answer it with its mirror (FR-2).
    let out = run(
        reverse_bin()
            .arg("--journal")
            .arg(&journal)
            .arg("--entry-id")
            .arg("e1"),
        None,
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "reverse must succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let after = fs::read_to_string(&journal).unwrap();
    assert!(after.starts_with(&before), "FR-2: append only; the past is intact");
    assert_eq!(after.lines().count(), before.lines().count() + 2, "two mirror rows landed");
    verify_ok(&journal);

    // 6. The fold over the whole journal nets every account to
    //    zero: the mistake stands, the answer stands, the books
    //    agree.
    let mut net: std::collections::BTreeMap<String, i64> = Default::default();
    for line in after.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let c: Vec<&str> = line.split('\t').collect();
        let amt: i64 = c[7].parse().unwrap();
        if !c[5].is_empty() {
            *net.entry(c[5].to_string()).or_insert(0) += amt;
        }
        if !c[6].is_empty() {
            *net.entry(c[6].to_string()).or_insert(0) -= amt;
        }
    }
    assert!(!net.is_empty(), "the walk touched accounts");
    assert!(
        net.values().all(|v| *v == 0),
        "post + reverse must net every account to zero; got {net:?}"
    );

    let _ = fs::remove_dir_all(&dir);
}