//
//! `flat2tsv` -- vendor-flat flat-file to party profile TSV converter.
//!
//! WHAT: Reads an vendor-flat flat file on stdin and emits a
//! party profile TSV per data row on stdout. Each
//! emitted line is the file body for `party new` to
//! consume.
//! WHY: Legacy vendor customer masters come as flat files.
//! `flat2tsv` is the bridge from the legacy vendor
//! format into the BOOKS/9 party tree.
//! LAYER: Driver. stdin -> stdout; the conversion is pure
//! (no I/O of its own).
//! DEPENDS: stdlib.

use std::io::{self, Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
 let mut input = String::new();
 if io::stdin().read_to_string(&mut input).is_err() {
 eprintln!("flat2tsv: read stdin failed");
 return ExitCode::from(2);
 }
 let stdout = io::stdout();
 let mut out = stdout.lock();
 // Header.
 if writeln!(out, "id\tname\tkind\tterms").is_err() {
 return ExitCode::from(2);
 }
 for line in input.lines() {
 if line.trim().is_empty() {
 continue;
 }
 // First line is the file header (`VENDORHEADER`); skip.
 if line.starts_with("VENDORHEADER") {
 continue;
 }
 // Data lines: KIND;id;name;terms
 let cols: Vec<&str> = line.split(';').collect();
 if cols.len() != 4 {
 // Skip malformed lines; a future cycle can warn
 // on stderr.
 continue;
 }
 let kind = cols[0].to_lowercase();
 let id = cols[1];
 let name = cols[2];
 let terms = cols[3];
 if let Err(e) = writeln!(out, "{id}\t{name}\t{kind}\t{terms}") {
 eprintln!("flat2tsv: write stdout: {e}");
 return ExitCode::from(2);
 }
 }
 ExitCode::from(0)
}
