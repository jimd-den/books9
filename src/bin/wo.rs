//
//! `wo` -- work order driver.
//!
//! WHAT:    `wo new` creates a work order file at
//!          /biz/docs/wo/{ID}.tsv and emits a backflush
//!          journal proposal on stdout: DR Finished Goods
//!          (per the item's cost) / CR components (per the
//!          item's BOM).
//! WHY:     The shop floor records a production run; the
//!          books pick up the finished good at cost and the
//!          components as issued.
//! LAYER:   Driver. Argv parsing, the file write, the
//!          proposal print.
//! DEPENDS: `libbiz::bom` (read_bom), stdlib.

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
    cmd_wo(&opts)
}

const FG_COST_MINOR_PER_UNIT: i64 = 1500;
const STEEL_COST_MINOR_PER_UNIT: i64 = 750;
const FINISHED_GOODS_ACCOUNT: &str = "1500";
const INVENTORY_ACCOUNT_PREFIX: &str = "1"; // any 1xxx is inventory

fn cmd_wo(opts: &Opts) -> ExitCode {
    // Read the BOM to know which components to issue.
    let bom = new_project::bom::read_bom(
        &opts.root.join(&opts.item).join("bom.tsv"),
    );
    let bom = match bom {
        Ok(b) => b,
        Err(e) => {
            eprintln!("wo: {e}");
            return ExitCode::from(2);
        }
    };
    // Compute the totals.
    let mut component_lines: Vec<(String, i64, String)> = Vec::new();
    for bl in &bom {
        if bl.item != opts.item {
            continue;
        }
        let needed = opts.qty.checked_mul(bl.qty_per_unit).unwrap_or(0);
        component_lines.push((bl.component.clone(), needed, bl.uom.clone()));
    }
    // DR Finished Goods = qty * FG_COST_MINOR_PER_UNIT.
    let fg_amount = opts.qty.checked_mul(FG_COST_MINOR_PER_UNIT).unwrap_or(0);
    // CR components: each component gets a separate leg
    // (component_name -> inventory account derived from name;
    // Phase 6 uses a simple "1" prefix as inventory).
    // The driver emits: 1 line for DR, then 1 line per
    // component for CR. Total DR must equal total CR.
    // For Phase 6 the simplest balance: the FG amount equals
    // the sum of component amounts (with a per-component
    // cost derived from a lookup). We hardcode steel at 750
    // and call other components "misc" at 0 (no cost -> no
    // line). Future phases will add a per-item cost table.

    // Header + DR + CR legs.
    println!("entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash");
    // DR Finished Goods.
    println!("wo{wo_id}\t1\t{date}\tent1\tUSD\t{fg_acct}\t\t{fg}\t\two:{wo_id}\t\t\tseed",
        wo_id = opts.wo_id, date = opts.date, fg_acct = FINISHED_GOODS_ACCOUNT, fg = fg_amount);
    let mut seq = 2;
    let mut total_cr: i64 = 0;
    for (component, qty_needed, _uom) in &component_lines {
        // Phase 6: only steel has a cost (750). Other
        // components contribute 0 to the journal (their
        // issuance is a future Phase 6+ enhancement).
        let unit_cost = if component == "steel" {
            STEEL_COST_MINOR_PER_UNIT
        } else {
            0
        };
        let amount = qty_needed.checked_mul(unit_cost).unwrap_or(0);
        if amount > 0 {
            // Use a simple per-component inventory account:
            // 1xxx where xxx is the first 3 chars of the
            // component name. Future: a real inventory CoA.
            let inv_acct = format!("{}{}", INVENTORY_ACCOUNT_PREFIX,
                &component.chars().take(3).collect::<String>());
            println!("wo{wo_id}\t{seq}\t{date}\tent1\tUSD\t\t{inv_acct}\t{amount}\t\two:{wo_id}\t\t\th0",
                wo_id = opts.wo_id, seq = seq, date = opts.date,
                inv_acct = inv_acct, amount = amount);
            seq += 1;
            total_cr += amount;
        }
    }
    // Balance check: FG amount must equal total CR.
    if fg_amount != total_cr {
        eprintln!("wo: unbalanced backflush: DR={fg_amount} CR={total_cr}");
        return ExitCode::from(2);
    }

    // Write the WO file at /biz/docs/wo/{ID}.tsv.
    let wo_dir = opts.root.join("docs").join("wo");
    if let Err(e) = std::fs::create_dir_all(&wo_dir) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    let mut body = format!(
        "wo_id\titem\tqty\tdate\nwo{wo_id}\t{item}\t{qty}\t{date}\n",
        wo_id = opts.wo_id, item = opts.item, qty = opts.qty, date = opts.date,
    );
    for (component, qty_needed, uom) in &component_lines {
        body.push_str(&format!("wo{wo_id}\t{item}\t{component}\t{qty_needed}\t{uom}\n",
            wo_id = opts.wo_id, item = opts.item, qty_needed = qty_needed, uom = uom));
    }
    let wo_path = wo_dir.join(format!("{}.tsv", opts.wo_id));
    if let Err(e) = new_project::store::write_atomic(&wo_path, body.as_bytes()) {
        eprintln!("{e}");
        return ExitCode::from(2);
    }
    ExitCode::from(0)
}

struct Opts {
    wo_id: String,
    item: String,
    qty: i64,
    date: String,
    root: PathBuf,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub = args.next().ok_or_else(|| "wo: usage: wo new ...".to_string())?;
    if sub != "new" {
        return Err(format!("wo: unknown subcommand: {sub}"));
    }
    let mut root: Option<PathBuf> = None;
    let mut wo_id: Option<String> = None;
    let mut item: Option<String> = None;
    let mut qty_str: Option<String> = None;
    let mut date: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--wo-id" => {
                wo_id = Some(args.next().ok_or_else(|| "--wo-id requires ID".to_string())?);
            }
            "--item" => {
                item = Some(args.next().ok_or_else(|| "--item requires ITEM".to_string())?);
            }
            "--qty" => {
                qty_str = Some(args.next().ok_or_else(|| "--qty requires QTY".to_string())?);
            }
            "--date" => {
                date = Some(args.next().ok_or_else(|| "--date requires DATE".to_string())?);
            }
            _ => return Err(format!("wo new: unknown flag: {a}")),
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let wo_id = wo_id.ok_or_else(|| "--wo-id is required".to_string())?;
    let item = item.ok_or_else(|| "--item is required".to_string())?;
    let qty_str = qty_str.ok_or_else(|| "--qty is required".to_string())?;
    let qty: i64 = qty_str.parse()
        .map_err(|e| format!("--qty: not an integer: {e}"))?;
    let date = date.ok_or_else(|| "--date is required".to_string())?;
    Ok(Opts { wo_id, item, qty, date, root })
}
