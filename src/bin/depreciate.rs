//
//! `depreciate` -- monthly depreciation amount driver.
//!
//! WHAT:    Reads an asset profile, calls
//!          `depreciate::straight_line`, prints the amount.
//! WHY:     "What is the depreciation for asset X in
//!          period Y?" is the controller's question.
//! LAYER:   Driver.
//! DEPENDS: `libbiz::asset` (read_profile),
//!          `libbiz::depreciate` (straight_line), stdlib.

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
    let profile = match new_project::asset::read_profile(
        &opts.root.join(&opts.asset).join("profile.tsv"),
    ) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("depreciate: {e}");
            return ExitCode::from(2);
        }
    };
    let amount = new_project::depreciate::straight_line(
        profile.cost_minor,
        profile.salvage_minor,
        profile.useful_life_months,
    );
    println!("{amount}");
    ExitCode::from(0)
}

struct Opts {
    root: PathBuf,
    asset: String,
    #[allow(dead_code)]
    period: String,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let mut root: Option<PathBuf> = None;
    let mut asset: Option<String> = None;
    let mut period: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--asset" => asset = Some(args.next().ok_or_else(|| "--asset requires ID".to_string())?),
            "--period" => period = Some(args.next().ok_or_else(|| "--period requires YYYY-MM".to_string())?),
            _ => return Err(format!("depreciate: unknown flag: {a}")),
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let asset = asset.ok_or_else(|| "--asset ID is required".to_string())?;
    let period = period.unwrap_or_else(|| "0000-00".to_string());
    Ok(Opts { root, asset, period })
}
