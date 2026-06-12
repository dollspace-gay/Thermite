//! Seam test for `thermite_lower::lower_equivalence_obligation`
//! (`.design/forge/equivalent-mutants.md` REQ-1, crosslink #101).
//!
//! Grounds the SEAM the equivalent-mutant exclusion lowers through: an exec body
//! rendered into a Verus EQUIVALENCE OBLIGATION that VERIFIES for the equivalent
//! case and FAILS (counterexample) for the distinguishing case. The decision is
//! the real `verus` verdict (R-DEFER-9 — exclude ONLY on a proof), so this test
//! shells the real binary. It SKIPS LOUDLY when verus is absent (mirroring
//! `lower_conformance.rs`), never panics.
//!
//! Expected verdicts are hand-derived from the design's *Ground the path*
//! (R-CHAR-3): `clamp_zero`'s `req x == 0` makes the early-`return 0` and the
//! `x - 0` flip observably equal to the real `x + 0`, but the `x + 1` off-by-one
//! and the `loose` (`req x <= 100`) early-`return 0` are distinguishing.

use std::path::{Path, PathBuf};
use std::process::Command;

use thermite_syntax::ast::{Block, Expr, Stmt};

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

fn verus_bin() -> String {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        if Path::new(&p).exists() {
            return p;
        }
    }
    "verus".to_string()
}

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

/// Run verus on `source`; return `true` iff it VERIFIES (`0 errors`).
fn verus_verifies(source: &str, label: &str) -> bool {
    let dir = std::env::temp_dir().join(format!(
        "thermite_equiv_{}_{}_{label}",
        std::process::id(),
        unique()
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let stem = format!("{label}_check");
    let path = dir.join(format!("{stem}.rs"));
    std::fs::write(&path, source).expect("write source");
    let out = Command::new(verus_bin())
        .arg(&path)
        .current_dir(&dir)
        .output()
        .expect("spawn verus");
    let _ = std::fs::remove_dir_all(&dir);
    // The grounded summary line is `verification results:: N verified, 0 errors`
    // (emitted to stderr without `--output-json`). A VERIFIED run exits 0 AND
    // reports `, 0 errors`; a counterexample exits non-zero with `, 1 errors`.
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out.status.success() && combined.contains(", 0 errors")
}

/// Parse a single-fn program and return its `FnItem`.
fn parse_fn(src: &str) -> thermite_syntax::ast::FnItem {
    let prog = thermite_syntax::parse(src).program;
    prog.items
        .into_iter()
        .find_map(|i| match i {
            thermite_syntax::ast::Item::Fn(f) => Some(f),
            _ => None,
        })
        .expect("a fn item")
}

/// An early-`return <lit>` mutant body for a scalar fn (the `mutation::generate`
/// early-return family): a leading `return lit;` with the real tail kept after.
fn early_return_body(real: &Block, lit: u128) -> Block {
    let mut body = real.clone();
    body.stmts.insert(
        0,
        Stmt::Return(Some(Expr::IntLit {
            value: lit,
            raw: lit.to_string(),
        })),
    );
    body
}

const CLAMP_ZERO: &str = "fn clamp_zero(x: u64) -> u64\n    req x == 0\n    ens result == 0\n    fx pure\n{\n    let y: u64 = x + 0;\n    y\n}\n";

const LOOSE: &str = "fn loose(x: u64) -> u64\n    req x <= 100\n    ens result <= 1000\n    fx pure\n{\n    let y: u64 = x + 0;\n    y\n}\n";

#[test]
fn equivalent_early_return_verifies() {
    // `clamp_zero`'s early-`return 0` IS observably equal to `x + 0` under
    // `req x == 0` (design: `2 verified, 0 errors`). The obligation VERIFIES.
    if !verus_present() {
        eprintln!("SKIP equivalent_early_return_verifies: verus absent");
        return;
    }
    let f = parse_fn(CLAMP_ZERO);
    let mutant = early_return_body(f.body.as_ref().unwrap(), 0);
    let obligation = thermite_lower::lower_equivalence_obligation(&f, &mutant, &[])
        .expect("scalar obligation lowers");
    assert!(
        verus_verifies(&obligation, "clamp_equiv"),
        "the early-return-0 mutant is PROVED equivalent to `x + 0` under x == 0; \
         obligation must VERIFY (REQ-2).\n--- obligation ---\n{obligation}"
    );
}

#[test]
fn distinguishing_offbyone_fails() {
    // The off-by-one `return 1` (the killed-class witness) is NOT equivalent to
    // `x + 0` under `req x == 0` (design: `0 verified, 1 errors`). The obligation
    // FAILS — so the survivor would STAY counted (the soundness line, REQ-3).
    if !verus_present() {
        eprintln!("SKIP distinguishing_offbyone_fails: verus absent");
        return;
    }
    let f = parse_fn(CLAMP_ZERO);
    let mutant = early_return_body(f.body.as_ref().unwrap(), 1);
    let obligation = thermite_lower::lower_equivalence_obligation(&f, &mutant, &[])
        .expect("scalar obligation lowers");
    assert!(
        !verus_verifies(&obligation, "clamp_distinguish"),
        "the early-return-1 mutant DIFFERS from `x + 0` under x == 0; the \
         equivalence obligation must FAIL — never launder a distinguishing \
         mutant (REQ-3).\n--- obligation ---\n{obligation}"
    );
}

#[test]
fn loose_early_return_stays_distinguishing() {
    // Under the LOOSER `req x <= 100` the SAME early-`return 0` mutant is NOT
    // equivalent (x = 5 distinguishes), so the obligation FAILS — the decision is
    // the verus verdict, NOT a syntactic shape match (AC-3 / AC-2 soundness).
    if !verus_present() {
        eprintln!("SKIP loose_early_return_stays_distinguishing: verus absent");
        return;
    }
    let f = parse_fn(LOOSE);
    let mutant = early_return_body(f.body.as_ref().unwrap(), 0);
    let obligation = thermite_lower::lower_equivalence_obligation(&f, &mutant, &[])
        .expect("scalar obligation lowers");
    assert!(
        !verus_verifies(&obligation, "loose_distinguish"),
        "under req x <= 100 the early-return-0 mutant is distinguishing (x = 5); \
         the obligation must FAIL (the verdict is verus's, not syntactic).\n\
         --- obligation ---\n{obligation}"
    );
}

#[test]
fn non_scalar_return_is_unsupported() {
    // A non-scalar (slice) return is out of the OQ-1 scalar scope: the seam
    // returns `Unsupported` so the caller leaves the survivor COUNTED (the
    // sound-but-incomplete fallback) — never a panic, never a spurious exclusion.
    let src = "fn head(xs: &[u32]) -> &[u32]\n    req true\n    ens true\n    fx pure\n{\n    &xs[..0]\n}\n";
    let f = parse_fn(src);
    let body = f.body.clone().unwrap();
    let res = thermite_lower::lower_equivalence_obligation(&f, &body, &[]);
    assert!(
        matches!(res, Err(thermite_lower::LowerError::Unsupported { .. })),
        "a non-scalar return must be Unsupported (OQ-1), got {res:?}"
    );
}

// ---------------------------------------------------------------------------
// #269 REQ-7: the CALL-BEARING equivalence obligation — the exec harness with
// the callee closure woven (modulo callee contracts, §9).
// ---------------------------------------------------------------------------

/// The §9 direct-composition fixture verbatim from `conformance/composition/
/// cases.json` (`verifies_to_boundary`): a `#[boundary]` `ext_id` whose contract
/// PINS its result, and `caller` whose body is `{ ext_id(x) }`.
const DIRECT_COMPOSITION: &str = "#[boundary(\"ext::ext_id\")] fn ext_id(x: u32) -> u32 req x < 100 ens result == x fx pure ; fn caller(x: u32) -> u32 req x < 100 ens result == x fx pure { ext_id(x) }";

/// The AC-8 WEAK-callee fixture: `ext_weak`'s `ens` does NOT pin its result
/// (`result <= 100`), so the identity mutant of `wcaller` is UNPROVABLE.
const WEAK_COMPOSITION: &str = "#[boundary(\"ext::ext_weak\")] fn ext_weak(x: u32) -> u32 req x < 100 ens result <= 100 fx pure ; fn wcaller(x: u32) -> u32 req x < 100 ens result <= 100 fx pure { ext_weak(x) }";

/// Parse `src` and return `(the named fn, every OTHER fn as the woven closure)`.
/// The closure mirrors `forge::check::reachable_fn_deps` (every in-file fn the
/// named fn references); for these single-caller fixtures it is the boundary
/// callee.
fn parse_caller_and_deps(
    src: &str,
    name: &str,
) -> (
    thermite_syntax::ast::FnItem,
    Vec<thermite_syntax::ast::Item>,
) {
    let prog = thermite_syntax::parse(src).program;
    let mut caller = None;
    let mut deps = Vec::new();
    for item in prog.items {
        match &item {
            thermite_syntax::ast::Item::Fn(f) if f.name == name => caller = Some(f.clone()),
            thermite_syntax::ast::Item::Fn(_) => deps.push(item),
            _ => {}
        }
    }
    (caller.expect("the caller fn"), deps)
}

/// The F-IDENT identity-return mutant body for `caller`: a leading `return x;`
/// (the identity of param `x`) ahead of the real tail.
fn identity_return_body(real: &Block, param: &str) -> Block {
    let mut body = real.clone();
    body.stmts
        .insert(0, Stmt::Return(Some(Expr::Path(vec![param.to_string()]))));
    body
}

#[test]
fn call_bearing_obligation_emits_the_woven_exec_harness() {
    // REQ-7 STRUCTURE (hand-derived to the design template, NOT pinned from the
    // tool's own output): the call-bearing obligation must be the EXEC harness
    // with the boundary callee woven as an external_body signature and the two
    // compared bodies in the `let real = { .. }; let mutant = { .. }` slots.
    let (caller, deps) = parse_caller_and_deps(DIRECT_COMPOSITION, "caller");
    let mutant = identity_return_body(caller.body.as_ref().unwrap(), "x");
    let obligation = thermite_lower::lower_equivalence_obligation(&caller, &mutant, &deps)
        .expect("the call-bearing obligation lowers (REQ-7)");

    // The woven callee: ext_id's external_body assumable signature (the SAME
    // `lower_external_body_fn` arm `item_subprogram` weaves), carrying its
    // unweakened contract.
    assert!(
        obligation.contains("#[verifier::external_body]"),
        "the boundary callee is woven as an external_body signature (REQ-7).\n{obligation}"
    );
    assert!(
        obligation.contains("ensures result == x")
            || obligation.contains("ensures\n        result == x")
            || obligation.contains("result == x"),
        "ext_id's PINNING `ens result == x` is woven verbatim (modulo-contract \
         equivalence rests on it).\n{obligation}"
    );
    // The harness form: an exec `equiv_check_caller -> (eq: bool)` with `ensures
    // eq` over the two block-value comparands.
    assert!(
        obligation.contains("fn equiv_check_caller(x: u32) -> (eq: bool)"),
        "the harness is an EXEC fn returning `eq: bool` (REQ-7).\n{obligation}"
    );
    assert!(
        obligation.contains("requires x < 100,"),
        "the harness carries the caller's `req` (REQ-7).\n{obligation}"
    );
    assert!(
        obligation.contains("ensures eq,"),
        "the harness's obligation is `ensures eq` (REQ-7).\n{obligation}"
    );
    assert!(
        obligation.contains("let real_v: u32 = { ext_id(x) };"),
        "the real body renders as the woven call `ext_id(x)` in EXEC position \
         (REQ-7 — a call is legal here).\n{obligation}"
    );
    assert!(
        obligation.contains("let mutant_v: u32 = { x };"),
        "the identity mutant renders as `x` (the early-return value).\n{obligation}"
    );
    assert!(
        obligation.contains("real_v == mutant_v"),
        "the harness compares the two block values (REQ-7).\n{obligation}"
    );
}

#[test]
fn call_bearing_identity_through_strong_contract_verifies() {
    // REQ-7 GROUNDING: ext_id's assumed `ens result == x` pins `real == x` at the
    // call site, so `caller`'s identity mutant `return x` IS a true equivalent
    // MODULO the contract — the harness `ensures eq` PROVES → excludable (REQ-2).
    if !verus_present() {
        eprintln!("SKIP call_bearing_identity_through_strong_contract_verifies: verus absent");
        return;
    }
    let (caller, deps) = parse_caller_and_deps(DIRECT_COMPOSITION, "caller");
    let mutant = identity_return_body(caller.body.as_ref().unwrap(), "x");
    let obligation = thermite_lower::lower_equivalence_obligation(&caller, &mutant, &deps)
        .expect("the call-bearing obligation lowers");
    assert!(
        verus_verifies(&obligation, "caller_modulo_contract"),
        "the identity mutant is PROVED equivalent THROUGH ext_id's contract \
         (REQ-7); the harness must VERIFY.\n--- obligation ---\n{obligation}"
    );
}

#[test]
fn call_bearing_identity_through_weak_contract_fails() {
    // REQ-8 CONSERVATISM: ext_weak's `ens result <= 100` does NOT pin `real == x`,
    // so the harness `eq` is UNPROVABLE → the survivor STAYS counted, the item
    // gates. Never a false exclusion (the decision is verus's, not syntactic).
    if !verus_present() {
        eprintln!("SKIP call_bearing_identity_through_weak_contract_fails: verus absent");
        return;
    }
    let (wcaller, deps) = parse_caller_and_deps(WEAK_COMPOSITION, "wcaller");
    let mutant = identity_return_body(wcaller.body.as_ref().unwrap(), "x");
    let obligation = thermite_lower::lower_equivalence_obligation(&wcaller, &mutant, &deps)
        .expect("the weak-callee obligation lowers");
    assert!(
        !verus_verifies(&obligation, "wcaller_weak_contract"),
        "ext_weak's weak `ens result <= 100` cannot pin `real == x`; the harness \
         must FAIL → the survivor stays counted (REQ-8).\n--- obligation ---\n{obligation}"
    );
}
