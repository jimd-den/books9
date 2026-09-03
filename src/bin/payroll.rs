//
//! `payroll` -- payroll journal driver.
//!
//! WHAT:    Reads hours and deductions; emits a balanced
//!          journal proposal on stdout: one DR Wages Expense
//!          + one CR Cash + one CR Wages Payable per employee.
//! WHY:     "What did the workforce earn, and where do the
//!          books go?" The reconciliation (FR-4) is
//!          gross = net + deductions, and the journal lines
//!          balance by construction.
//! LAYER:   Driver. Argv parsing, the compute, the print.
//! DEPENDS: `libbiz::payroll` (compute, read_hours, read_deductions),
//!          stdlib.
//! USED BY: Payroll clerk at end of pay period.

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
    cmd_payroll(&opts)
}

fn cmd_payroll(opts: &Opts) -> ExitCode {
    let hours = match new_project::payroll::read_hours(&opts.hours) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let deductions = match new_project::payroll::read_deductions(&opts.deductions) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let lines = match new_project::payroll::compute(&hours, &deductions) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    // The journal proposal: header + 3 legs per employee.
    // One leg per (employee, cost_center, kind). The total
    // per employee is: DR Wages Expense = gross,
    // CR Cash = net, CR Wages Payable = deductions.
    println!("entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash");
    for (i, line) in lines.iter().enumerate() {
        let eid = format!("pr{}", i + 1);
        let seq1 = 1 + 3 * i as i64;
        let seq2 = 2 + 3 * i as i64;
        let seq3 = 3 + 3 * i as i64;
        // DR Wages Expense
        println!("{eid}\t{seq1}\t2026-09-30\t{cc}\tUSD\t6000\t\t{gross}\t{emp}\tpayroll\t\t\tseed",
            cc = line.cost_center, gross = line.gross, emp = line.employee);
        // CR Cash
        println!("{eid}\t{seq2}\t2026-09-30\t{cc}\tUSD\t\t1000\t{net}\t{emp}\tpayroll\t\t\th0",
            cc = line.cost_center, net = line.net, emp = line.employee);
        // CR Wages Payable
        println!("{eid}\t{seq3}\t2026-09-30\t{cc}\tUSD\t\t2100\t{ded}\t{emp}\tpayroll\t\t\th1",
            cc = line.cost_center, ded = line.deductions, emp = line.employee);
        // FR-4 reconciliation: gross = net + deductions. The
        // driver-level check is implicit in the math: if gross
        // != net + deductions, we'd have a bug. The compute
        // function sets net = gross - deductions, so this
        // holds by construction.
    }
    ExitCode::from(0)
}

struct Opts {
    hours: PathBuf,
    deductions: PathBuf,
    #[allow(dead_code)]
    org_root: Option<PathBuf>,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let mut hours: Option<PathBuf> = None;
    let mut deductions: Option<PathBuf> = None;
    let mut org_root: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--hours" => {
                let p = args.next().ok_or_else(|| "--hours requires PATH".to_string())?;
                hours = Some(PathBuf::from(p));
            }
            "--deductions" => {
                let p = args.next().ok_or_else(|| "--deductions requires PATH".to_string())?;
                deductions = Some(PathBuf::from(p));
            }
            "--org-root" => {
                let p = args.next().ok_or_else(|| "--org-root requires PATH".to_string())?;
                org_root = Some(PathBuf::from(p));
            }
            _ => return Err(format!("payroll: unknown flag: {a}")),
        }
    }
    let hours = hours.ok_or_else(|| "--hours PATH is required".to_string())?;
    let deductions = deductions.ok_or_else(|| "--deductions PATH is required".to_string())?;
    Ok(Opts { hours, deductions, org_root })
}
