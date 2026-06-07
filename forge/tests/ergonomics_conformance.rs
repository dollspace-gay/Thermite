//! Conformance for Cluster **C10** (crosslink **#112**): the binding /
//! control-flow ERGONOMICS — tuple destructuring (`let (x, y) = e`), `for i in
//! 0..n` loops, match guards (`x if cond =>`), or-patterns (`1 | 2 =>`), and
//! `if let` / `while let`. Each is SUGAR over the shipped, proven core
//! (`while`+`inv`/`dec`, `match`, `Expr::TupleProj`, `Expr::Is`) — no ergonomic
//! adds a proof rule, weakens an obligation, or launders a verification path
//! (R-DEFER-9). These run against the two EXTERNAL truths the toolchain does not
//! author for itself: the built `forge` binary's certificate ladder (`forge
//! check`, real verus) and the `thermite_spec::validate` exhaustiveness checker.
//!
//! Pins the C10 deliverables (`.design/basis/11-ergonomics.md`):
//!
//!   * REQ-1 `let (x, y) = swap(a, b);` using `y` → L3 (AC-1 — the temp +
//!     projection desugar verifies; `Expr::TupleProj` is the SHIPPED core).
//!   * REQ-2 `for i in 0..n inv … { acc = acc + 1; }` → L3 (AC-2 — the
//!     for→while desugar with AUTO-`dec n - i`); a BAD inv (`acc = acc + 2`,
//!     inv still `acc == i`) → L0 (AC-2b — the inv bites through the desugar).
//!   * REQ-3 a guarded match (`n if n < 10 => …`) → L3 (AC-3); a guarded-ONLY
//!     arm (`Yes(v) if v < 10 => v, No => 0`, no plain `Yes`) → NonExhaustive
//!     (AC-3b — a guard does NOT complete a match).
//!   * REQ-4 an or-pattern (`1 | 2 => …`) → L3 (AC-4); `Yes(_) | No` over an
//!     enum is EXHAUSTIVE (the union closes the match); a strict-subset `A | B`
//!     over `{A,B,C}` is still NonExhaustive.
//!   * REQ-5 `if let Some(v) = e { v } else { 0 }` → L3 (AC-5); `while let
//!     Some(_) = cur … { … }` → L3 via the canonical `while (cur is Some)` form
//!     (AC-6 — the `matches!` exec discriminant, NOT loop+break).
//!
//! The verus checks SKIP LOUDLY when verus is absent (the `tuples_conformance.rs`
//! precedent) — never panic on a missing solver. `tests/` is not anti-pattern-
//! gated, so `unwrap`/`expect`/`panic!` are fine (R-APG-2).
//!
//! R-CHAR-3: expected levels trace to `.design/basis/11-ergonomics.md`
//! AC-1..AC-6 (the GROUNDED forms: tuple `2 verified, 0 errors`; for `2
//! verified` / bad-inv `invariant not satisfied`; guarded-only `non-exhaustive
//! patterns`; or-pattern exhaustive; if-let/while-let L3) + `thermite-design.md`
//! §6 ladder semantics (L3 == a fully-discharged real-verus proof), NEVER copied
//! from the toolchain's own output. The validator-reject expectations
//! (`NonExhaustiveMatch { missing }`) are hand-derived from REQ-3/REQ-4.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use thermite_spec::{validate, SpecError};

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `tuples_conformance.rs`).
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

/// Write `program` to a unique temp `.th`, `forge check --json` it, return the
/// cert array. The temp file is removed before returning (scratch hygiene, #53).
fn check_program(tag: &str, program: &str) -> Vec<Value> {
    let fixture = std::env::temp_dir().join(format!(
        "forge_erg_{tag}_{}_{}.th",
        std::process::id(),
        tag.len()
    ));
    std::fs::write(&fixture, program).expect("write fixture");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&fixture)
        .arg("--json")
        .output()
        .expect("spawn forge");
    let _ = std::fs::remove_file(&fixture);
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<Value>(stdout.trim())
        .unwrap_or_else(|e| {
            panic!(
                "forge --json stdout must be one JSON doc: {e}\nstdout:\n{stdout}\nstderr:\n{}",
                String::from_utf8_lossy(&out.stderr)
            )
        })
        .as_array()
        .expect("array of certs")
        .clone()
}

fn cert_for<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no cert for `{item}` in {certs:?}"))
}

// ---------------------------------------------------------------------------
// REQ-1 — tuple destructuring `let (x, y) = e;`
// ---------------------------------------------------------------------------

/// REQ-1 / AC-1 — `let (x, y) = swap(a, b);` then returning `y` certifies L3.
/// The destructure desugars (in the parser) to a temp + per-element projection
/// `let`s reusing the SHIPPED `Expr::TupleProj`. `swap` carries `ens result.0 ==
/// b, result.1 == a`, so `y == result.1 == a` — the consumer's `ens result == a`
/// holds. AUTHORITY: `.design/basis/11-ergonomics.md` AC-1 (GROUNDED `2
/// verified, 0 errors`); §6 (L3 == a fully-discharged verus proof).
#[test]
fn req1_tuple_destructuring_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — tuple destructuring L3 not exercised.");
        return;
    }
    let certs = check_program(
        "destr",
        "fn swap(a: u64, b: u64) -> (u64, u64)\n  req true\n  ens result.0 == b\n  ens result.1 == a\n  fx pure\n{ (b, a) }\nfn use_it(a: u64, b: u64) -> u64\n  req true\n  ens result == a\n  fx pure\n{ let (x, y) = swap(a, b);\n y }\n",
    );
    let use_it = cert_for(&certs, "use_it");
    assert_eq!(
        use_it["level"], "L3",
        "DESIGN 11-ergonomics.md AC-1: `let (x, y) = swap(a, b);` returning `y` \
         (== result.1 == a) certifies L3 — the destructure desugars to a temp + \
         `Expr::TupleProj` `let`s. forge reports: {}",
        use_it["level"]
    );
}

/// REQ-1 (parse) — `let (x, y) = e;` desugars to a temp + two projection `let`s
/// (NO new AST node; the surface tuple-pattern is gone before lowering). A
/// `_`-element drops its `let`. Pure AST-shape pin (no verus).
#[test]
fn req1_tuple_destructure_desugars_to_temp_plus_projections() {
    use thermite_syntax::{Expr, Item, Stmt};
    let parsed = thermite_syntax::parse(
        "fn f(a: u64, b: u64) -> u64\n  req true\n  ens result == a\n  fx pure\n{ let (x, _) = g(a, b);\n x }\n",
    );
    assert!(parsed.is_clean(), "must parse: {:?}", parsed.errors);
    let Item::Fn(f) = &parsed.program.items[0] else {
        panic!("expected fn");
    };
    let body = f.body.as_ref().expect("body");
    // The `_` element is dropped, so the destructure expands to exactly: the
    // temp `let __td<n> = g(a, b);` + one projection `let x = __td<n>.0;`.
    assert_eq!(
        body.stmts.len(),
        2,
        "DESIGN AC-1: `let (x, _) = g(a, b);` desugars to a temp + ONE projection \
         `let` (the `_` element is dropped). stmts: {:?}",
        body.stmts
    );
    // The first stmt is the temp init; the second binds `x` to a TupleProj index 0.
    match &body.stmts[1] {
        Stmt::Let {
            name,
            init: Expr::TupleProj { index, .. },
            ..
        } => {
            assert_eq!(name, "x");
            assert_eq!(*index, 0, "the first element binds `.0`");
        }
        other => panic!("DESIGN AC-1: element binding must be a projection `let`, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// REQ-2 — `for i in 0..n` loops
// ---------------------------------------------------------------------------

/// REQ-2 / AC-2 — `for i in 0..n inv acc == i inv i <= n { acc = acc + 1; }` with
/// `ens result == n` certifies L3. The for→while desugar synthesizes the AUTO
/// `dec n - i` (the user writes only the `inv`). AUTHORITY:
/// `.design/basis/11-ergonomics.md` AC-2 (GROUNDED `2 verified, 0 errors`); §6.
#[test]
fn req2_for_range_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — for-range L3 not exercised.");
        return;
    }
    let certs = check_program(
        "for",
        "fn count(n: u64) -> u64\n  req true\n  ens result == n\n  fx pure\n{ let mut acc: u64 = 0;\n for i in 0..n inv acc == i inv i <= n { acc = acc + 1; }\n acc }\n",
    );
    let count = cert_for(&certs, "count");
    assert_eq!(
        count["level"], "L3",
        "DESIGN 11-ergonomics.md AC-2: `for i in 0..n inv acc == i inv i <= n {{ \
         acc = acc + 1; }}` with `ens result == n` certifies L3 — the for desugars \
         to a `while i < n` with the AUTO `dec n - i`. forge reports: {}",
        count["level"]
    );
}

/// REQ-2 / AC-2b (the inv BITES through the desugar — R-DEFER-9) — the SAME loop
/// whose body steps `acc = acc + 2` while the inv still claims `acc == i` is NOT
/// L3 (`invariant not satisfied`). The for desugar does not launder the
/// obligation. AUTHORITY: `.design/basis/11-ergonomics.md` AC-2b (GROUNDED `1
/// verified, 1 errors`); §7 (the battery bites).
#[test]
fn req2_bad_for_inv_is_l0() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — for bad-inv non-vacuity not exercised.");
        return;
    }
    let certs = check_program(
        "forbad",
        "fn count(n: u64) -> u64\n  req true\n  ens result == n\n  fx pure\n{ let mut acc: u64 = 0;\n for i in 0..n inv acc == i inv i <= n { acc = acc + 2; }\n acc }\n",
    );
    let count = cert_for(&certs, "count");
    assert_ne!(
        count["level"], "L3",
        "R-DEFER-9: a `for` whose body steps `acc = acc + 2` while the inv claims \
         `acc == i` must be REJECTED (the inv obligation bites through the desugar, \
         not laundered). forge reports: {}",
        count["level"]
    );
}

/// REQ-2 (the AUTO `dec` is a real measure) — a user `dec` on a `for` is a parse
/// error (the `dec` is automatic, `hi - i`). Pins that the for-loop owns its
/// decreases — the agent cannot get it wrong (§2.3 one-way).
#[test]
fn req2_user_dec_on_for_is_rejected() {
    let parsed = thermite_syntax::parse(
        "fn count(n: u64) -> u64\n  req true\n  ens result == n\n  fx pure\n{ let mut acc: u64 = 0;\n for i in 0..n inv acc == i dec n - i { acc = acc + 1; }\n acc }\n",
    );
    assert!(
        !parsed.is_clean(),
        "DESIGN AC-2: a user-written `dec` on a `for` must be a parse error — the \
         `dec` is AUTOMATIC (`hi - i`), so the user writes none."
    );
}

// ---------------------------------------------------------------------------
// REQ-3 — match guards `x if cond =>`
// ---------------------------------------------------------------------------

/// REQ-3 / AC-3 — a guarded match `match x { n if n < 10 => true, _ => false }`
/// with a non-vacuous `ens result == (x < 10)` certifies L3. AUTHORITY:
/// `.design/basis/11-ergonomics.md` AC-3 (the guard lowers to the Verus-native
/// guarded arm); §6.
#[test]
fn req3_guarded_match_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — guarded match L3 not exercised.");
        return;
    }
    let certs = check_program(
        "guard",
        "fn small(x: u64) -> bool\n  req true\n  ens result == (x < 10)\n  fx pure\n{ match x { n if n < 10 => true, _ => false } }\n",
    );
    let small = cert_for(&certs, "small");
    assert_eq!(
        small["level"], "L3",
        "DESIGN 11-ergonomics.md AC-3: a guarded match `n if n < 10 => true, _ => \
         false` with `ens result == (x < 10)` certifies L3 — the guard lowers to \
         the Verus-native `pat if cond => body`. forge reports: {}",
        small["level"]
    );
}

/// REQ-3 / AC-3b (a guard does NOT complete a match) — `match m { Yes(v) if v <
/// 10 => v, No => 0 }` over `enum Maybe { Yes(u64), No }` is NON-exhaustive: the
/// guarded `Yes` arm does not cover `Yes(_)`. The validator MUST reject it with
/// `NonExhaustiveMatch { missing: ["Yes"] }`. AUTHORITY:
/// `.design/basis/11-ergonomics.md` AC-3b (GROUNDED: Verus rejects a guarded-only
/// `Some` arm). Hand-derived expectation (R-CHAR-3).
#[test]
fn req3_guarded_only_arm_is_non_exhaustive() {
    let parsed = thermite_syntax::parse(
        "enum Maybe { Yes(u64), No } fn f(m: Maybe) -> u64 req true ens result == result fx pure { match m { Yes(v) if v < 10 => v, No => 0 } }",
    );
    assert!(parsed.is_clean(), "must parse: {:?}", parsed.errors);
    let errors = match validate(&parsed.program) {
        Ok(()) => panic!(
            "DESIGN AC-3b: a guarded-ONLY `Yes(v) if v < 10` arm does NOT cover \
             `Yes(_)` (the guard may fail), so the match is NON-exhaustive — \
             expected NonExhaustiveMatch{{missing:[Yes]}}, got Ok(())."
        ),
        Err(e) => e,
    };
    let found = errors.iter().any(|e| {
        matches!(e, SpecError::NonExhaustiveMatch { missing, .. } if missing.iter().any(|m| m == "Yes"))
    });
    assert!(
        found,
        "DESIGN AC-3b: expected NonExhaustiveMatch {{ missing: [Yes] }} (a guard does \
         NOT complete a match); got {errors:?}"
    );
}

// ---------------------------------------------------------------------------
// REQ-4 — or-patterns `1 | 2 =>`
// ---------------------------------------------------------------------------

/// REQ-4 / AC-4 — `match x { 1 | 2 => true, _ => false }` with `ens result ==
/// (x == 1 || x == 2)` certifies L3. AUTHORITY:
/// `.design/basis/11-ergonomics.md` AC-4 (the or-pattern lowers to the
/// Verus-native `p0 | p1 | …`); §6.
#[test]
fn req4_or_pattern_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — or-pattern L3 not exercised.");
        return;
    }
    let certs = check_program(
        "orpat",
        "fn is12(x: u64) -> bool\n  req true\n  ens result == (x == 1 || x == 2)\n  fx pure\n{ match x { 1 | 2 => true, _ => false } }\n",
    );
    let is12 = cert_for(&certs, "is12");
    assert_eq!(
        is12["level"], "L3",
        "DESIGN 11-ergonomics.md AC-4: `match x {{ 1 | 2 => true, _ => false }}` with \
         `ens result == (x == 1 || x == 2)` certifies L3 — the or-pattern lowers to \
         the Verus-native `1 | 2`. forge reports: {}",
        is12["level"]
    );
}

/// REQ-4 / AC-4 (exhaustive via the union) — `match m { Yes(_) | No => 0 }` over
/// `enum Maybe { Yes(u64), No }` is EXHAUSTIVE: the or-pattern covers BOTH
/// variants (the union closes the match), so the validator ACCEPTS it. A
/// strict-subset `A | B` over `enum Tri { A, B, C }` is still NON-exhaustive
/// (`missing: [C]`). AUTHORITY: `.design/basis/11-ergonomics.md` AC-4 +
/// Architecture ("an or-pattern covers the UNION; a strict subset leaves the
/// rest uncovered"). Hand-derived (R-CHAR-3).
#[test]
fn req4_or_pattern_exhaustive_via_union() {
    // The union `Yes(_) | No` closes the match → validates clean.
    let exhaustive = thermite_syntax::parse(
        "enum Maybe { Yes(u64), No } fn f(m: Maybe) -> u64 req true ens result == result fx pure { match m { Yes(_) | No => 0 } }",
    );
    assert!(exhaustive.is_clean(), "must parse: {:?}", exhaustive.errors);
    assert!(
        validate(&exhaustive.program).is_ok(),
        "DESIGN AC-4: `Yes(_) | No` covers the UNION of both variants → the match \
         is EXHAUSTIVE (validates clean). got {:?}",
        validate(&exhaustive.program)
    );

    // A strict subset `A | B` over `{A, B, C}` still leaves `C` uncovered.
    let subset = thermite_syntax::parse(
        "enum Tri { A, B, C } fn f(t: Tri) -> u64 req true ens result == result fx pure { match t { A | B => 0 } }",
    );
    assert!(subset.is_clean(), "must parse: {:?}", subset.errors);
    let errors = validate(&subset.program).expect_err("A | B over {A,B,C} is non-exhaustive");
    assert!(
        errors.iter().any(|e| matches!(
            e,
            SpecError::NonExhaustiveMatch { missing, .. } if missing.iter().any(|m| m == "C")
        )),
        "DESIGN AC-4: an or-pattern over a STRICT SUBSET (`A | B` of `{{A,B,C}}`) \
         still leaves the rest uncovered — expected NonExhaustiveMatch{{missing:[C]}}, \
         got {errors:?}"
    );
}

// ---------------------------------------------------------------------------
// REQ-5 — `if let` / `while let`
// ---------------------------------------------------------------------------

/// REQ-5 / AC-5 — `if let Some(v) = o { v } else { 0 }` (the desugar of which is
/// `match o { Some(v) => v, _ => 0 }`) with `ens result == match o { Some(v) =>
/// v, None => 0 }` certifies L3. AUTHORITY: `.design/basis/11-ergonomics.md`
/// AC-5 (the SHIPPED `Expr::Match` core); §6.
#[test]
fn req5_if_let_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — if-let L3 not exercised.");
        return;
    }
    let certs = check_program(
        "iflet",
        "fn unwrap_or(o: Option<u64>) -> u64\n  req true\n  ens result == match o { Some(v) => v, None => 0 }\n  fx pure\n{ if let Some(v) = o { v } else { 0 } }\n",
    );
    let f = cert_for(&certs, "unwrap_or");
    assert_eq!(
        f["level"], "L3",
        "DESIGN 11-ergonomics.md AC-5: `if let Some(v) = o {{ v }} else {{ 0 }}` \
         desugars to `match o {{ Some(v) => v, _ => 0 }}` and certifies L3 against \
         a matching `ens`. forge reports: {}",
        f["level"]
    );
}

/// REQ-5 / AC-6 — `while let Some(_) = cur … { … }` certifies L3 via the
/// canonical `while (cur is Some)` desugar (the `matches!` exec discriminant,
/// NOT loop+break). The body sets `cur = None` to exit; the loop `inv`/`dec` are
/// written by the user as for any `while` (the implication invariant
/// `!(cur is Some) || (c == 0)` ties the discriminant to the decreasing measure
/// `1 - c`). AUTHORITY: `.design/basis/11-ergonomics.md` AC-6 (the `while
/// (cond)` form is L3 — the loop+break alternative is L0); §6.
#[test]
fn req5_while_let_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — while-let L3 not exercised.");
        return;
    }
    let certs = check_program(
        "whilelet",
        "fn drain(start: Option<u64>) -> u64\n  req true\n  ens result <= 1\n  fx pure\n{ let mut cur: Option<u64> = start;\n let mut c: u64 = 0;\n while let Some(_) = cur inv c <= 1 inv !(cur is Some) || (c == 0) dec 1 - c { c = 1; cur = None; }\n c }\n",
    );
    let drain = cert_for(&certs, "drain");
    assert_eq!(
        drain["level"], "L3",
        "DESIGN 11-ergonomics.md AC-6: `while let Some(_) = cur … {{ … }}` certifies \
         L3 via the canonical `while (cur is Some)` form (the `matches!` exec \
         discriminant, NOT loop+break). forge reports: {}",
        drain["level"]
    );
}

/// REQ-5 (parse) — `while let` desugars to a `while` whose condition is the
/// SHIPPED `Expr::Is` discriminant (NOT a `loop`+`break`). Pure AST-shape pin.
#[test]
fn req5_while_let_desugars_to_while_is_variant() {
    use thermite_syntax::{Expr, Item, LoopKind, Stmt};
    let parsed = thermite_syntax::parse(
        "fn drain(start: Option<u64>) -> u64\n  req true\n  ens result <= 1\n  fx pure\n{ let mut cur: Option<u64> = start;\n let mut c: u64 = 0;\n while let Some(_) = cur inv c <= 1 dec 1 - c { cur = None; }\n c }\n",
    );
    assert!(parsed.is_clean(), "must parse: {:?}", parsed.errors);
    let Item::Fn(f) = &parsed.program.items[0] else {
        panic!("expected fn");
    };
    let body = f.body.as_ref().expect("body");
    // The `while let` lowered to a `Stmt::Loop` whose kind is `While(e is Some)`.
    let loop_node = body
        .stmts
        .iter()
        .find_map(|s| match s {
            Stmt::Loop(l) => Some(l),
            _ => None,
        })
        .expect("a Stmt::Loop from the while-let desugar");
    match &loop_node.kind {
        LoopKind::While(cond) => match cond.as_ref() {
            Expr::Is { variant, .. } => assert_eq!(
                variant.last().map(|s| s.as_str()),
                Some("Some"),
                "DESIGN AC-6: the `while let Some(_) = cur` desugars to `while (cur \
                 is Some)` — the condition is `Expr::Is {{ variant: Some }}`."
            ),
            other => panic!("DESIGN AC-6: while-let condition must be `Expr::Is`, got {other:?}"),
        },
        other => panic!("DESIGN AC-6: while-let must desugar to a `While` loop, got {other:?}"),
    }
}
