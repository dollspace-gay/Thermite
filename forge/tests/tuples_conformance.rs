//! Conformance for Cluster **C9-B** (crosslink **#109**): TUPLES — the
//! `Type::Tuple(Vec<Type>)` n-tuple type, the `Expr::Tuple(Vec<Expr>)`
//! construction, and the `Expr::TupleProj { receiver, index }` projection (`.0`/
//! `.1`/…), the v1 §2.3 "one way" tuple access (NOT destructuring — REQ-9
//! deferred). These run against the two EXTERNAL truths the toolchain does not
//! author for itself: the built `forge` binary's certificate ladder (`forge
//! check`, real verus) and the real `verus` binary on the EMITTED lowering.
//!
//! Pins the C9-B deliverables (`.design/basis/10-recursion-tuples.md`):
//!
//!   * `fn swap(a, b: u64) -> (u64, u64) ens result.0 == b && result.1 == a {
//!     (b, a) }` → L3 (AC-4 — the GROUNDED `2 verified, 0 errors`).
//!   * the SAME `swap` with body `(a, b)` → NOT L3 (AC-5 — the projection `ens`
//!     BITES; `postcondition not satisfied`; R-DEFER-9 non-vacuity).
//!   * a 3-tuple `(u64, u64, u64)` with `ens result.0 == 1 && result.1 == 2 &&
//!     result.2 == 3` → L3 (AC-6 — n-tuple arity ≥ 2; GROUNDED `3 verified`).
//!   * a tuple in a `let` + projection in exec position BUILDS+RUNS.
//!   * the disambiguation does not break `()` (unit type) or `(e)` (grouping):
//!     these are parse-level checks (no verus needed).
//!
//! The verus checks SKIP LOUDLY when verus is absent (the `option_result_
//! conformance.rs` precedent) — never panic on a missing solver. `tests/` is not
//! anti-pattern-gated, so `unwrap`/`expect`/`panic!` are fine (R-APG-2).
//!
//! R-CHAR-3: expected levels trace to `.design/basis/10-recursion-tuples.md`
//! AC-4/AC-5/AC-6 (the GROUNDED forms: `swap` `2 verified, 0 errors`; wrong body
//! `postcondition not satisfied`; 3-tuple `3 verified, 0 errors`) +
//! `thermite-design.md` §6 ladder semantics (L3 == a fully-discharged real-verus
//! proof), NEVER copied from the toolchain's own output.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use thermite_syntax::{Expr, Type};

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `option_result_conformance.rs`).
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
        "forge_tuples_{tag}_{}_{}.th",
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
// Parse-level / AST-shape pins (REQ-5/REQ-7; no verus). The disambiguation
// `()` (unit) vs `(e)` (grouping) vs `(a, b)` (tuple) and the projection node.
// ---------------------------------------------------------------------------

/// REQ-7 — `(u64, u64)` parses to `Type::Tuple` (arity 2); the unit `()` stays
/// `Type::Unit`; the parenthesised `(u64)` is the inner type (grouping, arity 1).
#[test]
fn tuple_type_disambiguation_unit_grouping_tuple() {
    let parsed = thermite_syntax::parse(
        "fn swap(a: u64, b: u64) -> (u64, u64)\n  req true\n  ens result.0 == b && result.1 == a\n  fx pure\n{ (b, a) }\n",
    );
    assert!(
        parsed.is_clean(),
        "DESIGN 10-recursion-tuples.md REQ-5/REQ-7: `(u64, u64)` must parse (it \
         was the #109 parse-fail `expected ) to close the unit type ()`). errors: {:?}",
        parsed.errors
    );
    let item = &parsed.program.items[0];
    let f = match item {
        thermite_syntax::Item::Fn(f) => f,
        other => panic!("expected an Item::Fn, got {other:?}"),
    };
    match &f.ret {
        Type::Tuple(elems) => assert_eq!(
            elems.len(),
            2,
            "DESIGN REQ-7: `(u64, u64)` is a 2-tuple `Type::Tuple` (arity 2)"
        ),
        other => panic!("DESIGN REQ-5: a `(u64, u64)` return must be `Type::Tuple`, got {other:?}"),
    }

    // The unit `()` return stays `Type::Unit` (the disambiguation did NOT break it).
    let unit = thermite_syntax::parse("fn log() -> ()\n  req true\n  ens true\n  fx pure\n{ }\n");
    assert!(
        unit.is_clean(),
        "`()` unit return still parses: {:?}",
        unit.errors
    );
    if let thermite_syntax::Item::Fn(f) = &unit.program.items[0] {
        assert_eq!(
            f.ret,
            Type::Unit,
            "DESIGN REQ-7: arity-0 `()` stays `Type::Unit` (UNCHANGED)"
        );
    } else {
        panic!("expected fn");
    }

    // The parenthesised `(u64)` is grouping → the inner type (arity 1, NOT a tuple).
    let grouped = thermite_syntax::parse(
        "fn id(a: u64) -> (u64)\n  req true\n  ens result == a\n  fx pure\n{ a }\n",
    );
    assert!(
        grouped.is_clean(),
        "`(u64)` grouping still parses: {:?}",
        grouped.errors
    );
    if let thermite_syntax::Item::Fn(f) = &grouped.program.items[0] {
        assert_eq!(
            f.ret,
            Type::Prim(thermite_syntax::PrimType::U64),
            "DESIGN REQ-7: arity-1 `(u64)` is grouping — the inner type, NOT a tuple"
        );
    } else {
        panic!("expected fn");
    }
}

/// REQ-5 — the body `(b, a)` parses to `Expr::Tuple` (arity 2) and the `ens`
/// `result.0` / `result.1` parse to `Expr::TupleProj` (the dedicated node, OQ-1).
#[test]
fn tuple_expr_and_projection_nodes() {
    let parsed = thermite_syntax::parse(
        "fn swap(a: u64, b: u64) -> (u64, u64)\n  req true\n  ens result.0 == b && result.1 == a\n  fx pure\n{ (b, a) }\n",
    );
    assert!(parsed.is_clean(), "must parse: {:?}", parsed.errors);
    let f = match &parsed.program.items[0] {
        thermite_syntax::Item::Fn(f) => f,
        other => panic!("expected fn, got {other:?}"),
    };

    // The body tail is `(b, a)` → `Expr::Tuple` of 2.
    let tail = f
        .body
        .as_ref()
        .and_then(|b| b.tail.as_ref())
        .expect("a tail expr");
    match tail.as_ref() {
        Expr::Tuple(elems) => assert_eq!(
            elems.len(),
            2,
            "DESIGN REQ-5: `(b, a)` is `Expr::Tuple` (arity 2)"
        ),
        other => panic!("DESIGN REQ-5: `(b, a)` must be `Expr::Tuple`, got {other:?}"),
    }

    // The `ens` `result.0 == b` — its lhs is `Expr::TupleProj { index: 0 }`.
    let ens0 = &f.contract.ens[0].expr;
    let lhs = match ens0 {
        Expr::Binary { lhs, .. } => lhs.as_ref(),
        other => panic!("ens is `result.0 == b` (a Binary), got {other:?}"),
    };
    match lhs {
        // The ens is `result.0 == b && result.1 == a` — a single `ens` clause whose
        // top is the `&&`. Its lhs is `result.0 == b`, whose lhs is `result.0`.
        Expr::Binary { lhs: inner, .. } => match inner.as_ref() {
            Expr::TupleProj { index, .. } => assert_eq!(
                *index, 0,
                "DESIGN REQ-5/OQ-1: `result.0` is `Expr::TupleProj {{ index: 0 }}` (the \
                 DEDICATED projection node, NOT a string-named `Field`)"
            ),
            other => panic!("`result.0` must be `Expr::TupleProj`, got {other:?}"),
        },
        Expr::TupleProj { index, .. } => assert_eq!(*index, 0),
        other => panic!("expected a projection under the ens, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// End-to-end ladder pins (REQ-8; real verus via `forge check`).
// ---------------------------------------------------------------------------

const SWAP_L3: &str = "fn swap(a: u64, b: u64) -> (u64, u64)\n  req true\n  ens result.0 == b && result.1 == a\n  fx pure\n{ (b, a) }\n";

/// AC-4 — `swap(a, b) -> (u64, u64) ens result.0 == b && result.1 == a { (b, a) }`
/// certifies L3.
///
/// AUTHORITY: `.design/basis/10-recursion-tuples.md` AC-4 — the GROUNDED form
/// (`2 verified, 0 errors`): `(u64, u64)` lowers to a Verus tuple type, `(b, a)`
/// to a Verus tuple, `result.0`/`result.1` to native Verus projections.
/// `thermite-design.md` §6: a fully-discharged verus proof is L3.
#[test]
fn ac4_swap_tuple_projection_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — swap tuple L3 not exercised.");
        return;
    }
    let certs = check_program("swapl3", SWAP_L3);
    let swap = cert_for(&certs, "swap");
    assert_eq!(
        swap["level"], "L3",
        "DESIGN 10-recursion-tuples.md AC-4: `swap` returning `(u64, u64)` with \
         `ens result.0 == b && result.1 == a` and body `(b, a)` certifies L3 (the \
         tuple lowers to a Verus tuple, the projection `ens` to `r.0`/`r.1`). forge \
         reports: {}",
        swap["level"]
    );
}

/// AC-5 (the projection `ens` BITES — non-vacuity, R-DEFER-9) — the SAME `swap`
/// with body `(a, b)` is NOT L3.
///
/// AUTHORITY: `.design/basis/10-recursion-tuples.md` AC-5 — `(a, b)` under the
/// projection `ens` (`result.0 == b`) FAILS verus (`postcondition not satisfied`)
/// — the projection contract is real, not vacuous. `thermite-design.md` §7: the
/// battery catches the false claim. The §7 vacuity gate is respected.
#[test]
fn ac5_wrong_body_under_projection_ens_is_rejected() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — tuple non-vacuity not exercised.");
        return;
    }
    let certs = check_program(
        "swapwrong",
        "fn swap(a: u64, b: u64) -> (u64, u64)\n  req true\n  ens result.0 == b && result.1 == a\n  fx pure\n{ (a, b) }\n",
    );
    let swap = cert_for(&certs, "swap");
    assert_ne!(
        swap["level"], "L3",
        "R-DEFER-9 non-vacuity: a wrong body `(a, b)` under the projection `ens` \
         (`result.0 == b && result.1 == a`) must be REJECTED — the projection `ens` \
         is a REAL constraint, not a vacuous `true`. forge reports: {}",
        swap["level"]
    );
}

/// AC-6 — an n-tuple (a 3-tuple `(u64, u64, u64)`) with a projection `ens`
/// certifies L3 (arity ≥ 2, not pairs-only).
///
/// AUTHORITY: `.design/basis/10-recursion-tuples.md` AC-6 — the GROUNDED 3-tuple
/// (`3 verified, 0 errors`): `Type::Tuple`/`Expr::Tuple` carry any arity ≥ 2.
#[test]
fn ac6_three_tuple_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — 3-tuple L3 not exercised.");
        return;
    }
    let certs = check_program(
        "triple",
        "fn triple() -> (u64, u64, u64)\n  req true\n  ens result.0 == 1 && result.1 == 2 && result.2 == 3\n  fx pure\n{ (1, 2, 3) }\n",
    );
    let triple = cert_for(&certs, "triple");
    assert_eq!(
        triple["level"], "L3",
        "DESIGN 10-recursion-tuples.md AC-6: a 3-tuple `(u64, u64, u64)` with `ens \
         result.0 == 1 && result.1 == 2 && result.2 == 3` certifies L3 — n-tuples \
         (arity >= 2), not pairs-only. forge reports: {}",
        triple["level"]
    );
}

/// REQ-8 (build+run) — a tuple in a `let` + a projection in EXEC position lowers
/// and verifies: a fn that constructs `(a, b)`, binds it, and reads `.0`.
///
/// AUTHORITY: `.design/basis/10-recursion-tuples.md` REQ-8 — the tuple
/// construction + projection lower to native Verus tuple ops in BOTH exec and
/// contract position. L3 == a fully-discharged verus proof (§6).
#[test]
fn req8_tuple_let_and_exec_projection_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — tuple let/exec-projection L3 not exercised.");
        return;
    }
    let certs = check_program(
        "letproj",
        "fn first(a: u64, b: u64) -> u64\n  req true\n  ens result == a\n  fx pure\n{ let p: (u64, u64) = (a, b); p.0 }\n",
    );
    let first = cert_for(&certs, "first");
    assert_eq!(
        first["level"], "L3",
        "DESIGN 10-recursion-tuples.md REQ-8: a tuple `let p: (u64, u64) = (a, b);` \
         bound + an EXEC projection `p.0` returning `a` (the `ens result == a`) \
         certifies L3. forge reports: {}",
        first["level"]
    );
}
