#!/usr/bin/env bash
#
# BOOKS/9 — per-phase audit script.
#
# Enforces the project's non-negotiables mechanically so neither the
# agent nor the reviewer has to remember every rule by hand. Run after
# every phase lands; the output is a single green light or a list of
# red lines naming the violation.
#
# Stays inside stdlib (no third-party deps introduced). Pure shell +
# ripgrep + cargo.

set -uo pipefail

# Resolve repo root even if the script is invoked from elsewhere.
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
cd "$ROOT" || exit 1

red()  { printf '\033[31m%s\033[0m\n' "$*"; }
green(){ printf '\033[32m%s\033[0m\n' "$*"; }
say()  { printf '%s\n' "$*"; }

fails=0

say "BOOKS/9 audit"
say "  root: $ROOT"
say ""

# 1. Cargo build must be green.
say "1. cargo build"
if cargo build --quiet 2>build.err; then
  green "  ok"
else
  red   "  FAIL: cargo build"; sed 's/^/    /' build.err
  fails=$((fails+1))
fi
rm -f build.err
say ""

# 2. Cargo test must be green (all binaries).
say "2. cargo test"
if cargo test --quiet 2>test.err; then
  green "  ok"
else
  red   "  FAIL: cargo test"; sed 's/^/    /' test.err
  fails=$((fails+1))
fi
rm -f test.err
say ""

# 3. No new third-party deps were introduced.
say "3. stdlib-only"
deps="$(awk '/^\[dependencies\]/{flag=1; next} /^\[/{flag=0} flag && NF' Cargo.toml | grep -v '^\s*#' || true)"
if [ -z "$deps" ]; then
  green "  ok (no third-party deps)"
else
  red   "  FAIL: third-party deps detected:"
  printf '    %s\n' $deps
  fails=$((fails+1))
fi
say ""

# 4. Protected files untouched.
say "4. protected files (must be unchanged by phase work)"
protected="prompt.txt CONVENTIONS.md AGENTS.md ROADMAP.md README.md prompts"
dirty="$(git status --porcelain $protected 2>/dev/null | grep -v '^??' || true)"
if [ -z "$dirty" ]; then
  green "  ok (none of: $protected)"
else
  red   "  FAIL: protected files modified:"
  printf '    %s\n' $dirty
  fails=$((fails+1))
fi
say ""

# 5. Binaries log to stderr only — quick heuristic: every bin/ tool's
#    main() must end without printing to stdout. We grep for println!
#    (stdout) anywhere under src/bin/.
say "5. tools log to stderr only"
if grep -rnE '[^a-z]println!\b' src/bin/ 2>/dev/null; then
  red   "  FAIL: println! found in src/bin/ (use eprintln!)"
  fails=$((fails+1))
else
  green "  ok"
fi
say ""

# 6. No floats at tool boundaries — heuristic: src/bin/ must not
#    mention f32/f64 type ascriptions.
say "6. no floats at tool boundaries"
if grep -rnE '\b(f32|f64)\b' src/bin/ 2>/dev/null; then
  red   "  FAIL: f32/f64 found in src/bin/"
  fails=$((fails+1))
else
  green "  ok"
fi
say ""

# 7. Dependency direction (Dependency Rule): src/bin/ may import from
#    lib.rs (new_project::*) but the reverse is forbidden. Heuristic:
#    src/lib.rs / src/<modules>.rs must not mention `crate::bin` or
#    import from src/bin/.
say "7. dependency direction (inward-only)"
if grep -rnE 'crate::bin|use crate::bin|new_project::bin' src/ 2>/dev/null \
   | grep -v 'src/bin/'; then
  red   "  FAIL: a non-bin module imports from src/bin/"
  fails=$((fails+1))
else
  green "  ok"
fi
say ""

if [ "$fails" -eq 0 ]; then
  green "ALL GREEN"
  exit 0
else
  red   "$fails violation(s)"
  exit 1
fi