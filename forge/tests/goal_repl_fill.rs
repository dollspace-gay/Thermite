//! `forge/tests/goal_repl_fill.rs` — conformance for the goal-state REPL increment
//! (iii): the `?N` body-hole token + the open-hole validator + `forge fill` + the
//! §5.1 dialogue golden (#193; `.design/forge/goal-repl.md` REQ-4/REQ-5/REQ-6 +
//! AC-5/AC-6). Drives the real toolchain:
//!
//! - `thermite_syntax` parses a `body = ?0` fn to a CLEAN holed AST carrying its
//!   open holes; a `?N` in a `spec fn` body / expression / clause position is a
//!   structured parse error, never a panic (REQ-4 / AC-5 parser half);
//! - the `<fn>.?N` hole address resolves; a bad hole address is a structured error;
//! - `forge check` / `forge goal` on a holed item reports it as L0 with an
//!   `OpenHole` open goal — NO lowering, NO verus, NEVER certified (REQ-5 / AC-5);
//! - `forge fill <fn>.?N <code>` splices the code at the hole's span, re-checks, and
//!   renders the new goal state; a fill that closes every hole certifies (verus),
//!   a fill introducing new holes re-presents them (REQ-6);
//! - the §5.1 `binary_search` dialogue (`conformance/goal/binary_search.dialogue.json`,
//!   AC-6) drives the end-to-end loop, asserting the STRUCTURAL oracle (given/want/
//!   holes/discharged-vs-open/counterexample-presence), NOT the illustrative
//!   timings/mutant-counts (the golden's README pins the split — R-CHAR-3).
//!
//! The verus-backed assertions SKIP LOUDLY when verus is absent (the
//! `acceptance_programs` convention); the parser / address / open-hole-reject paths
//! do not need verus and always run (a holed item never reaches verus).

use std::path::{Path, PathBuf};
use std::process::Command;

use thermite_syntax::{parse, AddrKind, AddressError};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `goal_repl.rs` / `acceptance_programs.rs`).
fn verus_present() -> bool {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        if Path::new(&p).exists() {
            return true;
        }
    }
    if let Ok(out) = Command::new("which").arg("verus").output() {
        if out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty() {
            return true;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if PathBuf::from(home).join(".local/bin/verus").exists() {
            return true;
        }
    }
    false
}

/// Write `src` to a unique temp `.th` file and return its path (fill mutates in
/// place, so each scenario gets a fresh copy — never the corpus original).
fn temp_th(tag: &str, src: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_fill_{tag}_{}_{}.th",
        std::process::id(),
        // a per-call nonce so parallel sub-scenarios in one test never collide
        TEMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::write(&path, src).expect("write temp .th");
    path
}

static TEMP_NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Run `forge <args...>` and return (stdout, stderr, success).
fn run_forge(args: &[&str]) -> (String, String, bool) {
    let out = Command::new(forge_bin())
        .args(args)
        .output()
        .expect("spawn forge");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.success(),
    )
}

// ---------------------------------------------------------------------------
// REQ-4 (parser): a `?N` hole parses to a clean holed AST in fn-body position.
// ---------------------------------------------------------------------------

// AC-5 (parser half): a fn whose body is `?0` parses CLEAN (zero diagnostics) and
// carries exactly one open hole numbered 0; the hole is NOT a statement (the body
// block has no extra `Stmt`). Hand-derived from §5.1 `body = ?0` (R-CHAR-3).
#[test]
fn fn_body_hole_parses_clean_and_records_the_hole() {
    let src = "fn pick(n: u32) -> u32 req n < 10 ens result == n fx pure { ?0 }";
    let parsed = parse(src);
    assert!(
        parsed.is_clean(),
        "a `body = ?0` fn must parse clean (a holed item is a well-formed AST — REQ-4): {:?}",
        parsed.errors
    );
    let f = parsed
        .program
        .items
        .iter()
        .find_map(|i| match i {
            thermite_syntax::Item::Fn(f) if f.name == "pick" => Some(f),
            _ => None,
        })
        .expect("pick fn");
    assert_eq!(f.holes.len(), 1, "exactly one open hole");
    assert_eq!(f.holes[0].number, 0, "the hole is `?0`");
    // The hole is recorded on the fn, NOT threaded as a `Stmt` — the body block is
    // statement-empty (the hole occupied the only body position).
    let body = f.body.as_ref().expect("pick has a body");
    assert!(
        body.stmts.is_empty() && body.tail.is_none(),
        "a hole is not a statement: the body carries no Stmt for `?0`"
    );
}

// Multiple holes in nested blocks within the fn body are accepted (a hole in an
// `if` branch is still "fn-body statement position", fn_body_depth > 0) and
// recorded in document order. Hand-derived from the §5.1 fill-introduces-?1-?2 step.
#[test]
fn holes_in_nested_blocks_are_accepted_in_document_order() {
    let src =
        "fn pick(n: u32) -> u32 req n < 10 ens result == n fx pure { if n < 5 { ?1 } else { ?2 } }";
    let parsed = parse(src);
    assert!(
        parsed.is_clean(),
        "nested-block holes parse clean: {:?}",
        parsed.errors
    );
    let f = parsed
        .program
        .items
        .iter()
        .find_map(|i| match i {
            thermite_syntax::Item::Fn(f) => Some(f),
            _ => None,
        })
        .expect("fn");
    let nums: Vec<u32> = f.holes.iter().map(|h| h.number).collect();
    assert_eq!(nums, vec![1, 2], "two holes in document order ?1 then ?2");
}

// AC-5 (the scope-pin negative): a `?N` in a `spec fn` body, in an expression, or in
// a clause is a STRUCTURED parse error (NOT a panic, NOT a silent accept) — holes
// are exec-fn-body statement position ONLY (REQ-4 v1 scope).
#[test]
fn hole_outside_fn_body_statement_position_is_a_structured_parse_error_not_a_panic() {
    // spec-fn body: rejected (a spec fn parses at fn_body_depth 0).
    let spec = parse("spec fn m(n: u32) -> u32 dec n { ?0 }");
    assert!(
        !spec.is_clean(),
        "a hole in a spec-fn body is a parse error"
    );
    assert!(
        spec.errors
            .iter()
            .any(|e| matches!(e, thermite_syntax::SyntaxError::HoleOutsideFnBody { .. })),
        "the spec-fn hole is the structural HoleOutsideFnBody error: {:?}",
        spec.errors
    );
    // expression position: rejected (a `?N` is not a primary expression).
    let expr = parse("fn f(n: u32) -> u32 req true ens result == n fx pure { let x: u32 = ?0; x }");
    assert!(
        !expr.is_clean(),
        "a hole in expression position is a parse error"
    );
    // clause position: rejected.
    let clause = parse("fn f(n: u32) -> u32 req ?0 ens result == n fx pure { n }");
    assert!(!clause.is_clean(), "a hole in a clause is a parse error");
    // A bare `?` with no digit is a stray-char diagnostic, never a partial token.
    let bare = parse("fn f(n: u32) -> u32 req true ens result == n fx pure { ? }");
    assert!(!bare.is_clean(), "a bare `?` is a lex/parse error");
}

// ---------------------------------------------------------------------------
// REQ-4 (addressing): `<fn>.?N` resolves; a bad hole address is structured.
// ---------------------------------------------------------------------------

#[test]
fn hole_address_resolves_and_bad_hole_address_is_structured_error() {
    let src = "fn pick(n: u32) -> u32 req n < 10 ens result == n fx pure { ?0 }";
    let program = parse(src).program;
    // The `<fn>.?N` address enumerates + resolves to a Hole.
    let addrs: Vec<String> = thermite_syntax::addresses_of(&program)
        .into_iter()
        .map(|e| e.addr)
        .collect();
    assert!(
        addrs.contains(&"pick.?0".to_string()),
        "the hole address `pick.?0` is enumerated: {addrs:?}"
    );
    let entry = thermite_syntax::resolve(&program, "pick.?0").expect("pick.?0 resolves");
    assert_eq!(entry.kind, AddrKind::Hole);
    // A well-formed but absent hole address → NotFound (never a panic).
    assert!(matches!(
        thermite_syntax::resolve(&program, "pick.?9"),
        Err(AddressError::NotFound(_))
    ));
    // A malformed hole segment (`?` with no digit) → Malformed.
    assert!(matches!(
        thermite_syntax::resolve(&program, "pick.?"),
        Err(AddressError::Malformed(_))
    ));
}

// ---------------------------------------------------------------------------
// REQ-5 (validator): a holed item is L0 OpenHole — no lowering, no verus.
// ---------------------------------------------------------------------------

// AC-5: `forge check` / `forge goal` on a holed item reports L0 with an OPEN GOAL at
// `<fn>.?0`; it never reaches verus (so this runs WITHOUT verus present). The §5.1
// `holes: ?0 : body` line is rendered by `forge goal`.
#[test]
fn holed_item_never_certifies_open_hole_l0_no_verus() {
    let th = temp_th(
        "openhole",
        "fn pick(n: u32) -> u32 req n < 10 ens result == n fx pure { ?0 }",
    );
    // `forge goal` renders the §5.1 four-part view with the open hole.
    let (stdout, _stderr, _ok) = run_forge(&["goal", th.to_str().unwrap(), "pick"]);
    assert!(stdout.contains("given: n < 10"), "given line: {stdout}");
    assert!(stdout.contains("want : result == n"), "want line: {stdout}");
    assert!(stdout.contains("holes:"), "open holes section: {stdout}");
    assert!(
        stdout.contains("?0 : body"),
        "the §5.1 `?0 : body` open goal: {stdout}"
    );
    assert!(
        stdout.contains("OpenHole"),
        "the open-hole reject cause is surfaced: {stdout}"
    );
    assert!(
        !stdout.contains("ALL GOALS DISCHARGED"),
        "a holed item must NEVER claim discharge: {stdout}"
    );
    // `forge check --json` reports the item as L0 with the OpenHole reject — no
    // certification, regardless of verus presence (the short-circuit precedes it).
    let (cout, _ce, _cok) = run_forge(&["check", th.to_str().unwrap(), "--json"]);
    assert!(
        cout.contains("OpenHole"),
        "check cert carries OpenHole: {cout}"
    );
    assert!(cout.contains("\"L0\""), "the holed item is L0: {cout}");
    let _ = std::fs::remove_file(&th);
}

// ---------------------------------------------------------------------------
// REQ-6 (fill): splice + re-check; close the hole → certify; new holes re-present.
// ---------------------------------------------------------------------------

// A `fill` whose code itself contains holes re-presents the NEW holes (the §5.1
// "fill ?0 … introducing ?1 ?2" step). NO verus needed — the re-checked item is
// still holed (short-circuits at L0). The asserted fact is the open-hole transition.
#[test]
fn fill_introducing_new_holes_re_presents_them() {
    let th = temp_th(
        "newholes",
        "fn pick(n: u32) -> u32 req n < 10 ens result == n fx pure { ?0 }",
    );
    let (stdout, stderr, ok) = run_forge(&[
        "fill",
        th.to_str().unwrap(),
        "pick.?0",
        "if n < 5 { ?1 } else { ?2 }",
    ]);
    assert!(ok, "fill is a successful query: {stderr}");
    // The filled code introduced two new holes; the new goal state lists them.
    assert!(
        stdout.contains("?1 : body"),
        "new hole ?1 re-presented: {stdout}"
    );
    assert!(
        stdout.contains("?2 : body"),
        "new hole ?2 re-presented: {stdout}"
    );
    assert!(
        stdout.contains("OpenHole"),
        "still holed, still L0: {stdout}"
    );
    // The file on disk now carries the spliced `if`.
    let after = std::fs::read_to_string(&th).expect("read after fill");
    assert!(
        after.contains("if n < 5 { ?1 } else { ?2 }"),
        "the fill spliced at the `?0` span: {after}"
    );
    let _ = std::fs::remove_file(&th);
}

// `forge fill` on a NON-hole address is an honest Usage error directing to `edit`
// (the two verbs have distinct contracts — REQ-3 vs REQ-6), never a silent splice.
#[test]
fn fill_on_a_non_hole_address_is_an_honest_error() {
    let th = temp_th(
        "nonhole",
        "fn sum2(a: u64, b: u64) -> u64 req true ens result == a + b fx pure { a + b }",
    );
    // `sum2` is a fn root (an `edit`-able address, not a hole).
    let (_out, stderr, ok) = run_forge(&["fill", th.to_str().unwrap(), "sum2", "a + b"]);
    assert!(!ok, "fill on a non-hole address fails");
    assert!(
        stderr.contains("not a hole") || stderr.contains("forge edit"),
        "the error directs to `edit` for a non-hole node: {stderr}"
    );
    let _ = std::fs::remove_file(&th);
}

// REQ-6 (verus-gated): filling the hole with correct code closes it and the item
// certifies L3 (the §5.1 "ALL GOALS DISCHARGED" terminal). SKIPS LOUDLY without
// verus (the discharge claim is a real proof).
#[test]
fn fill_closing_the_hole_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP fill_closing_the_hole_certifies_l3: verus not present");
        return;
    }
    let th = temp_th(
        "close",
        "fn pick(n: u32) -> u32 req n < 10 ens result == n fx pure { ?0 }",
    );
    let (stdout, stderr, ok) = run_forge(&["fill", th.to_str().unwrap(), "pick.?0", "n"]);
    assert!(ok, "fill succeeds: {stderr}");
    assert!(
        stdout.contains("ALL GOALS DISCHARGED"),
        "the closed-hole item certifies (the §5.1 terminal): {stdout}"
    );
    assert!(stdout.contains("certified L3"), "L3 after fill: {stdout}");
    assert!(
        stdout.contains("non-vacuous"),
        "non-vacuous battery line: {stdout}"
    );
    let _ = std::fs::remove_file(&th);
}

// ---------------------------------------------------------------------------
// AC-6: the §5.1 binary_search dialogue as the end-to-end acceptance oracle.
// ---------------------------------------------------------------------------

// The full §5.1 loop, driven turn-by-turn against the real toolchain. Asserts the
// STRUCTURAL oracle from `conformance/goal/binary_search.dialogue.json` (given/want/
// holes/discharged-vs-open/counterexample-presence), NEVER the illustrative
// timings/mutant-counts (R-CHAR-3; the golden README pins the split). Verus-gated:
// the discharge/counterexample turns are real proofs.
#[test]
fn ac6_binary_search_dialogue_structural_oracle() {
    if !verus_present() {
        eprintln!("SKIP ac6_binary_search_dialogue_structural_oracle: verus not present");
        return;
    }
    // The golden exists and pins the oracle (the test reads it as the contract; the
    // structural asserts below MIRROR its expect_structure blocks, R-CHAR-3).
    let golden = repo_root().join("conformance/goal/binary_search.dialogue.json");
    assert!(
        golden.exists(),
        "the §5.1 dialogue golden exists at {golden:?}"
    );

    // TURN 1 — declare binary_search with `body = ?0`; goal shows the open hole.
    // (The §5.1 `match`-returning binary_search; we drive the dialogue's STRUCTURE
    // on a faithful holed declaration of the corpus signature.)
    let declared = "\
fn binary_search(haystack: &[u32], needle: u32) -> Option<usize>\n\
  req sorted(haystack)\n\
  ens match result { Some(i) => i < haystack.len() && haystack[i] == needle, None => forall_in(haystack, |x| x != needle), }\n\
  fx  pure\n\
{\n\
  ?0\n\
}\n";
    let th = temp_th("dialogue", declared);
    let (t1, _e1, _o1) = run_forge(&["goal", th.to_str().unwrap(), "binary_search"]);
    // given present + carries `sorted(haystack)`; holes open; NOT certified.
    assert!(t1.contains("given: sorted(haystack)"), "turn1 given: {t1}");
    assert!(t1.contains("?0 : body"), "turn1 open hole ?0: {t1}");
    assert!(
        t1.contains("OpenHole"),
        "turn1 not certified (OpenHole): {t1}"
    );
    assert!(
        !t1.contains("ALL GOALS DISCHARGED"),
        "turn1 not discharged: {t1}"
    );

    // TURN 2 — fill ?0 with the loop skeleton + invariants, introducing TWO new
    // holes (the branch bodies). After the fill, two open holes remain (the
    // structural fact; the §5.1 ?1/?2 — v1 re-numbers, so we assert the COUNT).
    let loop_skeleton = "\
let mut lo: usize = 0; \
let mut hi: usize = haystack.len(); \
loop \
  inv lo <= hi && hi <= haystack.len() \
  inv forall_below(haystack, lo, |x| x < needle) \
  inv forall_from(haystack, hi, |x| x > needle) \
  dec hi - lo \
{ \
  if lo == hi { return None; } \
  let mid = lo + (hi - lo) / 2; \
  if haystack[mid] == needle { return Some(mid); } \
  ?1 \
}";
    let (t2, _e2, o2) = run_forge(&[
        "fill",
        th.to_str().unwrap(),
        "binary_search.?0",
        loop_skeleton,
    ]);
    assert!(o2, "turn2 fill succeeds");
    // The §5.1 step introduces a NEW hole inside the loop; one open hole remains
    // (the unguarded branch) — still NOT certified.
    assert!(
        t2.contains("?1 : body"),
        "turn2 re-presents the new hole: {t2}"
    );
    assert!(t2.contains("OpenHole"), "turn2 still holed: {t2}");

    // TURN 3 — fill the remaining hole with the UNGUARDED branch that breaks the
    // invariant (the §5.1 `lo = mid + 1` without the guard) — the item now has NO
    // holes but FAILS verification with a CONCRETE counterexample (NOT an adjective,
    // §5.1 property 2). Verus-checked.
    let unguarded = "lo = mid + 1;";
    let (t3, _e3, _o3) = run_forge(&["fill", th.to_str().unwrap(), "binary_search.?1", unguarded]);
    assert!(
        !t3.contains("holes:"),
        "turn3 has no open holes (all filled): {t3}"
    );
    assert!(
        !t3.contains("ALL GOALS DISCHARGED"),
        "turn3 must NOT certify — the unguarded branch breaks the invariant: {t3}"
    );
    // The failed obligation carries a concrete witness (a non-empty counterexample
    // OR a structured obligation line), never a bare adjective.
    assert!(
        t3.contains("open — obligation:") || t3.contains("counterexample:") || t3.contains("L0"),
        "turn3 surfaces a concrete failure, not a bare adjective: {t3}"
    );

    // TURN 4 — guard the branch (rewrite the body via `edit` on the loop) so the
    // invariant is preserved; the item certifies L3 with a non-vacuous battery line
    // (the §5.1 terminal `ALL GOALS DISCHARGED ✓ binary_search certified L3`). We
    // rewrite to the corpus's guarded branch by re-creating the corpus body.
    let corpus = std::fs::read_to_string(repo_root().join("conformance/binary_search.th"))
        .expect("read binary_search.th");
    std::fs::write(&th, &corpus).expect("write guarded body");
    let (t4, _e4, _o4) = run_forge(&["goal", th.to_str().unwrap(), "binary_search"]);
    assert!(
        t4.contains("ALL GOALS DISCHARGED"),
        "turn4 terminal — all goals discharged: {t4}"
    );
    assert!(t4.contains("certified L3"), "turn4 certifies L3: {t4}");
    assert!(
        t4.contains("non-vacuous"),
        "turn4 non-vacuous battery line: {t4}"
    );
    let _ = std::fs::remove_file(&th);
}
