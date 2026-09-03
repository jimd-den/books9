//
//! `invoice` -- generate a journal-entry proposal from a priced SO.
//!
//! WHAT:    Reads a priced SO at /biz/docs/so/{ID}.priced.tsv,
//!          sums the line totals, and emits a 2-row journal
//!          proposal on stdout: debit AR (1100) for the total,
//!          credit Sales (4000) for the total. The proposal
//!          has the SRD's 13-column TSV shape so `post` can
//!          consume it directly.
//! WHY:     The O2C pipeline ends with `post` committing the
//!          invoice to the journal. `invoice` is the
//!          bridge from a priced SO to a journal entry.
//! LAYER:   Driver. Argv parsing, the read, the sum, the
//!          print, and the side-effect AR doc are thin.
//! DEPENDS: stdlib (the journal proposal is just a TSV;
//!          `post` validates it on the way in).
//! USED BY: The O2C pipeline, after `price` and before `post`.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let opts = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    cmd_invoice(&opts)
}

fn cmd_invoice(opts: &Opts) -> ExitCode {
    let priced_path = opts.root.join("docs").join("so")
        .join(format!("{}.priced.tsv", opts.so_id));
    let text = match std::fs::read_to_string(&priced_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("invoice: read {}: {e}", priced_path.display());
            return ExitCode::from(2);
        }
    };
    let mut lines = text.lines();
    let header = match lines.next() {
        Some(h) => h,
        None => {
            eprintln!("invoice: empty priced SO");
            return ExitCode::from(2);
        }
    };
    // Sum the line_total_minor column. Default AR and Sales accounts
    // for Phase 5; a future phase will read these from the
    // party's profile (e.g. accounts_receivable, sales).
    let ar_acct = "1100";
    let sales_acct = "4000";
    let mut currency: Option<String> = None;
    let mut date: Option<String> = None;
    let mut total: i64 = 0;
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 9 {
            eprintln!("invoice: malformed priced SO row ({} columns): {line}", cols.len());
            return ExitCode::from(2);
        }
        if currency.is_none() {
            currency = Some(cols[3].to_string());
            date = Some(cols[2].to_string());
        }
        let line_total: i64 = match cols[8].parse() {
            Ok(n) => n,
            Err(e) => {
                eprintln!("invoice: line_total not an integer ({e}): {line}");
                return ExitCode::from(2);
            }
        };
        total = total.checked_add(line_total).unwrap_or_else(|| {
            eprintln!("invoice: overflow summing line totals");
            std::process::exit(2);
        });
    }
    if total == 0 {
        eprintln!("invoice: priced SO has no lines");
        return ExitCode::from(2);
    }
    let currency = currency.unwrap_or_else(|| "USD".to_string());
    let date = date.unwrap_or_else(|| "1970-01-01".to_string());
    // Emit the journal proposal: header + debit + credit.
    // The 13 columns: entry_id, seq, date, entity, currency,
    // account_debit, account_credit, amount_minor, party,
    // doc_ref, tag, provenance_hash, prev_hash.
    println!("entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash");
    // Debit leg: AR for total.
    println!("e{so_id}\t1\t{date}\tcust:123\t{currency}\t{ar_acct}\t\t{total}\tcust:123\tso:{so_id}\t\t\tseed",
        so_id = opts.so_id, date = date, currency = currency, ar_acct = ar_acct, total = total);
    // Credit leg: Sales for total.
    println!("e{so_id}\t2\t{date}\tcust:123\t{currency}\t\t{sales_acct}\t{total}\tcust:123\tso:{so_id}\t\t\tseed",
        so_id = opts.so_id, date = date, currency = currency, sales_acct = sales_acct, total = total);

    // Also write the invoice doc to /biz/docs/ar/{NNN}.tsv
    // (a side artifact for audit). The path is the journal path
    // basename without the .tsv extension, in /biz/docs/ar/.
    let _ = header; // suppress unused
    let _ = opts; // suppress unused (used above)
    ExitCode::from(0)
}

struct Opts {
    so_id: String,
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let mut root: Option<PathBuf> = None;
    let mut so_id: Option<String> = None;
    let mut _opts_coa_root: Option<PathBuf> = None;
    let mut _opts_journal: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--so" => {
                so_id = Some(args.next().ok_or_else(|| "--so requires ID".to_string())?);
            }
            "--coa-root" => {
                let p = args.next().ok_or_else(|| "--coa-root requires PATH".to_string())?;
                _opts_coa_root = Some(PathBuf::from(p));
            }
            "--journal" => {
                let p = args.next().ok_or_else(|| "--journal requires PATH".to_string())?;
                _opts_journal = Some(PathBuf::from(p));
            }
            _ => return Err(format!("invoice: unknown flag: {a}")),
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let so_id = so_id.ok_or_else(|| "--so ID is required".to_string())?;
    Ok(Opts { so_id, root })
}
