//
// tests/no_sap_in_docs.rs
//
//! Pin the scrub: user-facing documentation must not
//! mention "SAP" by name. The project is its own thing.
//!
//! Scope of the scrub:
//! - The four top-level prose files: AGENTS.md,
//! README.md, ROADMAP.md, prompt.txt, SRD.md.
//! - The one Phase 8 plan that mentions the migration
//! tool by SAP name (plans/phase8-inquiry-flat2tsv.md).
//! - The two integration tests that describe the
//! migration tool in comments.
//! - The migration tool's source comments.
//!
//! Out of scope (kept on purpose, not user-facing):
//! - The flat2tsv binary name (it is the tool's job;
//! renaming it is a separate, larger piece of work).

use std::fs;
use std::path::{Path, PathBuf};

const SCRUB_SCOPE: &[&str] = &[
 "AGENTS.md",
 "README.md",
 "ROADMAP.md",
 "prompt.txt",
 "SRD.md",
 "plans/phase8-inquiry-flat2tsv.md",
 "tests/phase8_integration.rs",
 "tests/flat2tsv_emits_party_profiles.rs",
 "src/bin/flat2tsv.rs",
];

fn read_text(path: &Path) -> String {
 match fs::read_to_string(path) {
 Ok(s) => s,
 Err(_) => String::new(),
 }
}

fn first_hit(text: &str) -> Option<(usize, String)> {
 // Word-boundary search; case-sensitive. The auditor
 // hits (Stable Abstractions Principle) live in
 // prompts/, which is not in SCRUB_SCOPE.
 let needle = "SAP";
 text.match_indices(needle).next().map(|(i, _)| {
 let s = i.saturating_sub(40);
 let e = (i + 80).min(text.len());
 (i, text[s..e].replace('\n', " "))
 })
}

#[test]
fn no_sap_in_user_facing_docs() {
 let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
 let mut offenders: Vec<String> = Vec::new();
 for rel in SCRUB_SCOPE {
 let path = manifest.join(rel);
 if !path.exists() {
 continue;
 }
 let text = read_text(&path);
 // Strip lines that are unambiguously Rust attribute
 // comments naming the audit-stable-abstractions-
 // principle acronym, should any leak in. There are
 // none today; the check is a safety net.
 if let Some((_, snippet)) = first_hit(&text) {
 offenders.push(format!("{rel}: ...{snippet}..."));
 }
 }
 assert!(
 offenders.is_empty(),
 "Scrub violation: 'SAP' found in user-facing docs:\n {}",
 offenders.join("\n ")
 );
}

#[test]
fn no_s4hana_in_user_facing_docs() {
 let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
 let mut offenders: Vec<String> = Vec::new();
 for rel in SCRUB_SCOPE {
 let path = manifest.join(rel);
 if !path.exists() { continue; }
 let text = read_text(&path);
 if let Some(idx) = text.find("S/4HANA") {
 let s = idx.saturating_sub(40);
 let e = (idx + 60).min(text.len());
 offenders.push(format!("{rel}: ...{}...", text[s..e].replace('\n', " ")));
 }
 if let Some(idx) = text.find("SAP S/4") {
 let s = idx.saturating_sub(40);
 let e = (idx + 60).min(text.len());
 offenders.push(format!("{rel}: ...{}...", text[s..e].replace('\n', " ")));
 }
 }
 assert!(
 offenders.is_empty(),
 "Scrub violation: 'S/4HANA' / 'SAP S/4' in docs:\n {}",
 offenders.join("\n ")
 );
}
