//
//! `routing` -- process plan (routing) driver.
//!
//! WHAT:    `routing show --item ITEM` prints the routing
//!          for an item. Phase P2P ships an empty list
//!          (no steps); future cycles add multi-step routing.
//! WHY:     Spec Phase 4 includes routing. Phase P2P lands
//!          the read surface; the write surface (steps) is
//!          a future cycle.
//! LAYER:   Driver.

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
    if opts.sub != "show" {
        eprintln!("routing: unknown subcommand: {}", opts.sub);
        return ExitCode::from(2);
    }
    // Phase P2P: empty list. Future cycles read the routing
    // tree at /biz/routings/{item}/steps.tsv.
    println!("step\top\tdescription");
    ExitCode::from(0)
}

struct Opts {
    sub: String,
    #[allow(dead_code)]
    root: PathBuf,
    #[allow(dead_code)]
    item: String,
}

fn parse_args() -> Result<Opts, String> {
    let mut args = std::env::args().skip(1);
    let sub = args.next().ok_or_else(|| "routing: usage: routing show ...".to_string())?;
    let mut root: Option<PathBuf> = None;
    let mut item: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let p = args.next().ok_or_else(|| "--root requires PATH".to_string())?;
                root = Some(PathBuf::from(p));
            }
            "--item" => item = Some(args.next().ok_or_else(|| "--item requires ITEM".to_string())?),
            _ => return Err(format!("routing: unknown flag: {a}")),
        }
    }
    let root = root.ok_or_else(|| "--root DIR is required".to_string())?;
    let item = item.ok_or_else(|| "--item ITEM is required".to_string())?;
    Ok(Opts { sub, root, item })
}
