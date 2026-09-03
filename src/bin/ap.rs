//
//! `ap` -- accounts payable driver (aging).
//!
//! WHAT:    `ap aging` reads the journal, finds every AP
//!          credit (account 2100 on the credit side of a leg),
//!          and buckets by age (0-30, 31-60, 61-90, 90+).
//! WHY:     "What's outstanding in AP?" is the controller's
//!          question. Mirror of `ar_aging` (Phase 5).
//! LAYER:   Driver.

use std::path::PathBuf;
use std::process::ExitCode;

const AP_ACCOUNT: &str = "2100";

fn main() -> ExitCode {
    let opts = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    cmd_ap_aging(&opts)
}

fn cmd_ap_aging(opts: &Opts) -> ExitCode {
    let content = match std::fs::read_to_string(&opts.journal) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("ap aging: read journal {}: {e}", opts.journal.display());
            return ExitCode::from(2);
        }
    };
    let as_of = match parse_date(&opts.as_of) {
        Some(d) => d,
        None => {
            eprintln!("ap aging: --as-of must be YYYY-MM-DD: {}", opts.as_of);
            return ExitCode::from(2);
        }
    };
    let mut buckets: [i64; 4] = [0; 4];
    for line in content.lines() {
        if line.trim().is_empty() || line.starts_with("entry_id\t") {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 13 {
            continue;
        }
        // AP credits: account 2100 on the credit side.
        if cols[6] != AP_ACCOUNT {
            continue;
        }
        let date = match parse_date(cols[2]) {
            Some(d) => d,
            None => continue,
        };
        let amount: i64 = match cols[7].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let age_days = (as_of - date).max(0);
        let bucket = match age_days {
            0..=30 => 0,
            31..=60 => 1,
            61..=90 => 2,
            _ => 3,
        };
        buckets[bucket] += amount;
    }
    if opts.format == "json" {
        println!("{{\"buckets\": [{{\"bucket\":\"0-30\",\"total\":{}}}, {{\"bucket\":\"31-60\",\"total\":{}}}, {{\"bucket\":\"61-90\",\"total\":{}}}, {{\"bucket\":\"90+\",\"total\":{}}}]}}",
            buckets[0], buckets[1], buckets[2], buckets[3]);
    } else {
        println!("bucket\ttotal");
        println!("0-30\t{}", buckets[0]);
        println!("31-60\t{}", buckets[1]);
        println!("61-90\t{}", buckets[2]);
        println!("90+\t{}", buckets[3]);
    }
    ExitCode::from(0)
}

/// Parse YYYY-MM-DD to days-since-epoch. Same algorithm as
/// `ar_aging`.
fn parse_date(s: &str) -> Option<i64> {
    let bytes = s.as_bytes();
    if bytes.len() < 10 {
        return None;
    }
    if !bytes[..4].iter().all(|b| b.is_ascii_digit())
        || bytes[4] != b'-'
        || !bytes[5..7].iter().all(|b| b.is_ascii_digit())
        || bytes[7] != b'-'
        || !bytes[8..10].iter().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let year: i64 = std::str::from_utf8(&bytes[..4]).ok()?.parse().ok()?;
    let month: i64 = std::str::from_utf8(&bytes[5..7]).ok()?.parse().ok()?;
    let day: i64 = std::str::from_utf8(&bytes[8..10]).ok()?.parse().ok()?;
    Some(days_from_civil(year, month, day))
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = (y - era * 400) as i64;
    let m = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * m + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

struct Opts {
    journal: PathBuf,
    as_of: String,
    #[allow(dead_code)]
    aging: bool,
    format: String,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub = args.next().ok_or_else(|| "ap: usage: ap aging ...".to_string())?;
    if sub != "aging" {
        return Err(format!("ap: unknown subcommand: {sub}"));
    }
    let mut journal: Option<PathBuf> = None;
    let mut as_of: Option<String> = None;
    let mut format = "tsv".to_string();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--journal" => {
                let p = args.next().ok_or_else(|| "--journal requires PATH".to_string())?;
                journal = Some(PathBuf::from(p));
            }
            "--as-of" => {
                as_of = Some(args.next().ok_or_else(|| "--as-of requires DATE".to_string())?);
            }
            "--format" => {
                let f = args.next().ok_or_else(|| "--format requires tsv|json".to_string())?;
                if f != "tsv" && f != "json" { return Err(format!("--format must be tsv or json (got {f:?})")); }
                format = f.to_string();
            }
            _ => return Err(format!("ap: unknown flag: {a}")),
        }
    }
    let journal = journal.ok_or_else(|| "--journal PATH is required".to_string())?;
    let as_of = as_of.ok_or_else(|| "--as-of DATE is required".to_string())?;
    Ok(Opts { journal, as_of, aging: true, format })
}
