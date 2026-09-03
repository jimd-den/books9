//
// FR-2 audit invariants, enforced on the source of the writers.
//
// tests/store_fr2.rs pins the store's surface: four functions, one
// of them the only door that appends. This file pins the OTHER end
// of the pen: the tools that hold it. bin/reverse answers a wrong
// entry, and the only way it may touch the books is the append
// door -- no direct OpenOptions on the journal, no rename onto it,
// no fs::write. If a future commit grows reverse a truncate, this
// test fails at review time, not at audit time.
//
// Why source text and not a mock: there is no mock layer in a
// stdlib-only crate, and none should exist. The dependency is
// static -- reverse.rs either calls store::append and nothing else
// that writes, or it does not. Reading the file is honest about
// that.
//
// close is deliberately NOT pinned by the append rule: close's
// writes are to the period flag and the close stamp, and it may
// never write the journal. Its own sentence here: no store::append
// call, and no OpenOptions against a journal file.
//
// Working rules: TDD, stdlib only, one behavior per commit.

const REVERSE_SRC: &str = include_str!("../src/bin/reverse.rs");
const CLOSE_SRC: &str = include_str!("../src/bin/close.rs");
const POST_SRC: &str = include_str!("../src/bin/post.rs");

/// Strip line comments and doc comments so a discussion of a
/// forbidden call in prose does not trip the lint. Blocks are not
/// tracked: none of the tools nest comments inside code today, and
/// the failure mode of over-stripping is a missed lint, never a
/// false alarm.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("//") {
            continue;
        }
        // trailing comment: ` //` outside of any string heuristics --
        // the tools keep their code strings comment-free at line end.
        if let Some(pos) = line.find(" //") {
            out.push_str(&line[..pos]);
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

#[test]
fn reverse_touches_the_books_only_through_the_append_door() {
    let code = strip_comments(REVERSE_SRC);
    assert!(
        code.contains("store::append"),
        "reverse must append through the kernel door"
    );
    for forbidden in ["fs::File", "OpenOptions", "fs::write", "fs::rename", "remove_file", "set_len", "truncate"] {
        assert!(
            !code.contains(forbidden),
            "reverse must not write the journal by hand: found {forbidden:?}"
        );
    }
}

#[test]
fn close_never_writes_the_journal_at_all() {
    let code = strip_comments(CLOSE_SRC);
    for forbidden in ["store::append", "store::create"] {
        assert!(
            !code.contains(forbidden),
            "close must not write the journal: found {forbidden:?}"
        );
    }
    // close writes flags and stamps only, and every write to the
    // periods namespace MUST go through `store`. The point of the
    // pin is not "close has exactly one OpenOptions call" (that
    // was a shape, not a behavior); the point is "close never
    // hand-rolls an OpenOptions or fs::write against anything."
    // After the F6 literate-programming refactor, close delegates
    // to `store::set_period` and `store::write_close_stamp`, so it
    // has zero direct OpenOptions / fs::write sites. A future
    // commit that re-introduces one of these is the bug we want
    // to catch at review time.
    for forbidden in ["OpenOptions", "fs::File", "fs::write", "fs::rename", "remove_file", "set_len", "truncate"] {
        assert!(
            !code.contains(forbidden),
            "close must not write anything directly; found {forbidden:?}"
        );
    }
    // And the two doors it is allowed to use:
    assert!(code.contains("store::set_period"), "close must mark periods through store");
    assert!(code.contains("store::write_close_stamp"), "close must write the stamp through store");
}

#[test]
fn post_writes_the_journal_only_through_the_append_door() {
    let code = strip_comments(POST_SRC);
    assert!(code.contains("store::append"), "post appends through the kernel door");
    for forbidden in ["fs::write", "fs::rename", "remove_file", "set_len", "truncate"] {
        assert!(
            !code.contains(forbidden),
            "post must not write the journal by hand: found {forbidden:?}"
        );
    }
}
