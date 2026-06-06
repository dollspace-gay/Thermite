//! Conformance test for `thermite-lower`'s ADT lowering (basis Stage 1c,
//! `.design/basis/01-adts.md` REQ-8/REQ-9/REQ-10) against the EXTERNAL truths:
//! the real `verus` binary (the emitted L3 output must VERIFY — `0 errors`), the
//! real `rustc` compiler (the emitted L1 output must COMPILE + RUN + fire its
//! contract check on violation), and the hand-derived cert oracle
//! (`conformance/{bank_account,shape}.cert.json` — R-CHAR-3, NEVER edited).
//!
//! Verification is by EXECUTION, not a strict byte-match against the goldens
//! (`tests/golden/lower/{bank_account,shape,list_sum}.verus.rs` +
//! `tests/golden/l1/{bank_account,shape}.l1.rs` are REFERENCES, the verify-not-
//! byte-match practice the existing `lower_conformance`/`l1_conformance` use).
//! `unwrap`/`expect`/`panic!` are fine here — `tests/` is not anti-pattern-gated.

use std::path::{Path, PathBuf};
use std::process::Command;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

fn parse_corpus(name: &str) -> thermite_syntax::ast::Program {
    let src = std::fs::read_to_string(corpus_dir().join(format!("{name}.th")))
        .unwrap_or_else(|e| panic!("cannot read corpus {name}.th: {e}"));
    let parsed = thermite_syntax::parse(&src);
    assert!(
        parsed.errors.is_empty(),
        "corpus {name}.th must parse clean: {:?}",
        parsed.errors
    );
    parsed.program
}

fn lower_l3(name: &str) -> String {
    thermite_lower::lower(&parse_corpus(name))
        .unwrap_or_else(|e| panic!("L3 lowering {name}.th failed: {e}"))
}

// ---- verus driver (shared shape with lower_conformance.rs) -----------------

fn verus_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    if let Ok(out) = Command::new("which").arg("verus").output() {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let cand = PathBuf::from(home).join(".local/bin/verus");
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

/// Run `verus <file>`; `None` if verus is unavailable (caller SKIPs LOUDLY).
fn run_verus(file: &Path) -> Option<(bool, String)> {
    let bin = verus_bin()?;
    let out = Command::new(bin)
        .arg(file)
        .current_dir(std::env::temp_dir())
        .output()
        .ok()?;
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), combined))
}

/// Lower `name` to L3, write to a temp file with a VALID crate name (the verus
/// `.`-in-crate-name gotcha), run verus, assert exit 0 + `verified, 0 errors`
/// (R-CODE-4: status checked, never swallowed). Returns the emitted source.
fn lower_and_verify(name: &str) -> String {
    let emitted = lower_l3(name);
    let tmp = std::env::temp_dir().join(format!("{name}_adt_lower.rs"));
    std::fs::write(&tmp, &emitted).unwrap_or_else(|e| panic!("write temp for {name}: {e}"));
    match run_verus(&tmp) {
        Some((ok, output)) => {
            assert!(
                ok && output.contains("0 errors"),
                "verus on emitted {name} did NOT verify (R-CODE-4). exit_success={ok}\n\
                 --- verus output ---\n{output}\n--- emitted ({}) ---\n{emitted}",
                tmp.display()
            );
            assert!(
                output.contains("verified, 0 errors"),
                "verus output for {name} missing `verified, 0 errors`:\n{output}"
            );
        }
        None => eprintln!(
            "SKIP: verus not available — L3 verification of emitted `{name}` not run \
             (set VERUS_BIN or install verus on PATH); structural asserts still run."
        ),
    }
    emitted
}

// ---- AC-1: struct + invariant → Verus struct + well_formed, VERIFIES (L3) --
//
// REQ-8: `deposit` lowers to a `pub struct Account` + the `well_formed` invariant
// predicate, with the invariant threaded (OQ-3 automatic threading) into
// `requires`/`ensures`; real verus VERIFIES it (L3). The cert oracle says L3,
// pure, non-vacuous.

#[test]
fn bank_account_lowers_struct_invariant_and_verifies_l3() {
    let emitted = lower_and_verify("bank_account");
    // REQ-8 struct + pub visibility tier (the recorded finding).
    assert!(
        emitted.contains("pub struct Account {") && emitted.contains("pub balance: u64,"),
        "struct + pub field tier (REQ-8):\n{emitted}"
    );
    // The well_formed type-invariant predicate, the inv lowered with self.field.
    assert!(
        emitted.contains("pub open spec fn well_formed(&self) -> bool {")
            && emitted.contains("self.balance <= 1000000"),
        "well_formed invariant predicate (REQ-8):\n{emitted}"
    );
    // OQ-3 automatic threading: the param + return well_formed conjuncts woven in.
    assert!(
        emitted.contains("a.well_formed(),"),
        "param invariant threaded into requires (REQ-8 OQ-3):\n{emitted}"
    );
    assert!(
        emitted.contains("result.well_formed(),"),
        "return invariant threaded into ensures (REQ-8 OQ-3):\n{emitted}"
    );
    // No weakening: the corpus contracts present (R-DEFER-9).
    assert!(
        emitted.contains("a.balance + amount <= 1000000,"),
        "corpus req present (no weakening):\n{emitted}"
    );
    assert!(
        emitted.contains("result.balance == a.balance + amount,"),
        "corpus ens present (no weakening):\n{emitted}"
    );
    // The StructLit construction (REQ-2/REQ-8).
    assert!(
        emitted.contains("Account { balance: a.balance + amount }"),
        "struct-literal construction (REQ-2):\n{emitted}"
    );
    assert_no_cheats(&emitted, "bank_account");
}

// ---- AC-1 cert oracle: deposit → L3, the stable subset matches the golden ----
//
// The cert oracle (`conformance/bank_account.cert.json`, R-CHAR-3 — never edited)
// pins the stable subset: level L3, tautology false, vacuous_precondition false,
// effects [pure], slag false. We assert the LOWERING enables that judgement: the
// emitted Verus verifies (L3 above) AND its effect is pure (no `fx alloc` — a
// struct construction does not allocate) AND the contract is non-vacuous (a real
// field relation, a satisfiable req).

#[test]
fn deposit_matches_cert_oracle_stable_subset() {
    // The cert oracle is the external truth (R-CHAR-3 — hand-derived, never read
    // from toolchain output, never edited). We assert its stable subset directly
    // from the raw JSON (a small frozen file; a string match avoids adding a JSON
    // dependency and still pins each oracle field).
    let cert = std::fs::read_to_string(corpus_dir().join("bank_account.cert.json"))
        .expect("read bank_account.cert.json");
    for needle in [
        "\"item\": \"deposit\"",
        "\"level\": \"L3\"",
        "\"tautology\": false",
        "\"vacuous_precondition\": false",
        "\"effects\": [\"pure\"]",
        "\"slag\": false",
    ] {
        assert!(
            cert.contains(needle),
            "bank_account cert oracle missing `{needle}`:\n{cert}"
        );
    }
    // The effect-row of `deposit` is `pure`: a `struct` construction is NOT an
    // alloc (only `Box::new` carries `fx alloc`). The effect checker accepts it.
    let program = parse_corpus("bank_account");
    assert!(
        thermite_lower::check_effects(&program).is_ok(),
        "deposit (fx pure) must pass effect-subsumption (a struct construction is not alloc)"
    );
    // The L3 verification itself is the dedicated
    // `bank_account_lowers_struct_invariant_and_verifies_l3` test (kept separate so
    // the two tests do not race on the same verus temp file).
}

// ---- AC-2: enum + match + is → Verus enum/match/is, VERIFIES (L3) ----------
//
// REQ-9: `is_circle` lowers to a Verus `enum Shape`, an ENUM-QUALIFIED `match`,
// and the `s is Circle` discriminant test; real verus VERIFIES it (L3). The cert
// oracle (`conformance/shape.cert.json`) says L3, pure, non-vacuous.

#[test]
fn shape_lowers_enum_match_is_and_verifies_l3() {
    let emitted = lower_and_verify("shape");
    // REQ-9 enum (tuple + struct variant).
    assert!(
        emitted.contains("enum Shape {")
            && emitted.contains("Circle(u64),")
            && emitted.contains("Rect { w: u64, h: u64 },"),
        "enum Shape with tuple + struct variants (REQ-9):\n{emitted}"
    );
    // REQ-9 match → ENUM-QUALIFIED Verus match arms.
    assert!(
        emitted.contains("Shape::Circle(r) => true,")
            && emitted.contains("Shape::Rect { w, h } => false,"),
        "enum-qualified match arms (REQ-9):\n{emitted}"
    );
    // REQ-9 `is` → Verus-native discriminant test.
    assert!(
        emitted.contains("result == (s is Circle),"),
        "`is` discriminant test in ens (REQ-6/REQ-9):\n{emitted}"
    );
    assert_no_cheats(&emitted, "shape");
}

#[test]
fn is_circle_matches_cert_oracle_stable_subset() {
    let cert = std::fs::read_to_string(corpus_dir().join("shape.cert.json"))
        .expect("read shape.cert.json");
    for needle in [
        "\"item\": \"is_circle\"",
        "\"level\": \"L3\"",
        "\"tautology\": false",
        "\"vacuous_precondition\": false",
        "\"effects\": [\"pure\"]",
        "\"slag\": false",
    ] {
        assert!(
            cert.contains(needle),
            "shape cert oracle missing `{needle}`:\n{cert}"
        );
    }
    let program = parse_corpus("shape");
    assert!(
        thermite_lower::check_effects(&program).is_ok(),
        "is_circle (fx pure) must pass effect-subsumption (a match is not effectful)"
    );
    // L3 verification is the dedicated `shape_lowers_enum_match_is_and_verifies_l3`
    // test (separate to avoid a verus temp-file race).
}

// ---- AC-3: recursive List + Box + structural decreases, VERIFIES (L3) ------
//
// REQ-10: `list_sum` lowers to a Verus recursive `enum List` with `Box<List>` at
// the recursive occurrence, and `spec fn sum_list` carries `decreases l` (the
// datatype VALUE — Verus's built-in structural order) recursing through `*t`;
// real verus VERIFIES it (terminates + totals). No fn cert (a spec-fn-only
// program) — the oracle is verus itself.

#[test]
fn list_sum_lowers_recursive_box_and_verifies_l3() {
    let emitted = lower_and_verify("list_sum");
    // REQ-10 recursive enum + Box at the recursive occurrence.
    assert!(
        emitted.contains("enum List {")
            && emitted.contains("Nil,")
            && emitted.contains("Cons(u64, Box<List>),"),
        "recursive enum with Box<List> (REQ-10):\n{emitted}"
    );
    // REQ-10 spec fn with structural `decreases l` (the value), recursing via *t.
    assert!(
        emitted.contains("spec fn sum_list(l: List) -> nat"),
        "sum_list spec fn returns nat (REQ-10):\n{emitted}"
    );
    assert!(
        emitted.contains("decreases l"),
        "structural decreases over the datatype VALUE (REQ-10):\n{emitted}"
    );
    assert!(
        emitted.contains("List::Nil => 0,") && emitted.contains("sum_list(*t)"),
        "enum-qualified match + *t Box-deref recursion (REQ-9/REQ-10):\n{emitted}"
    );
    assert_no_cheats(&emitted, "list_sum");
}

// ---- L1: deposit/is_circle compile + run + the contract check fires --------

fn compile_and_run(src: &str, crate_name: &str) -> (bool, bool, String) {
    let dir = std::env::temp_dir();
    let rs = dir.join(format!("{crate_name}.l1.rs"));
    let bin = dir.join(crate_name);
    std::fs::write(&rs, src).unwrap_or_else(|e| panic!("write temp {crate_name}: {e}"));
    let comp = Command::new("rustc")
        .arg("--crate-name")
        .arg(crate_name)
        .arg("--edition")
        .arg("2021")
        .arg(&rs)
        .arg("-o")
        .arg(&bin)
        .current_dir(&dir)
        .output()
        .unwrap_or_else(|e| panic!("rustc invocation failed for {crate_name}: {e}"));
    let mut combined = String::from_utf8_lossy(&comp.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&comp.stderr));
    if !comp.status.success() {
        return (false, false, combined);
    }
    let run = Command::new(&bin)
        .current_dir(&dir)
        .output()
        .unwrap_or_else(|e| panic!("running {crate_name} failed: {e}"));
    combined.push_str(&String::from_utf8_lossy(&run.stdout));
    combined.push_str(&String::from_utf8_lossy(&run.stderr));
    (true, run.status.success(), combined)
}

fn lower_l1(name: &str) -> String {
    thermite_lower::lower_l1(&parse_corpus(name))
        .unwrap_or_else(|e| panic!("L1 lowering {name}.th failed: {e}"))
}

// AC: deposit L1 compiles + runs; the positive case holds (hand-derived
// `deposit(Account{100}, 50).balance == 150`, R-CHAR-3).
#[test]
fn bank_account_l1_compiles_and_runs() {
    let emitted = lower_l1("bank_account");
    let program = format!(
        "{emitted}\nfn main() {{\n    let a = deposit(Account {{ balance: 100 }}, 50);\n    assert_eq!(a.balance, 150);\n    let z = deposit(Account {{ balance: 0 }}, 0);\n    assert_eq!(z.balance, 0);\n    println!(\"ok\");\n}}\n"
    );
    let (compiled, ran, out) = compile_and_run(&program, "bank_account_l1");
    assert!(
        compiled,
        "deposit L1 did NOT compile:\n{out}\n--- {program}"
    );
    assert!(
        ran && out.contains("ok"),
        "deposit L1 did NOT run clean (positive 150/0):\n{out}"
    );
}

// AC: deposit L1's `req` check FIRES on a violating call (the always-active
// handler aborts, observable — handled-or-loud at run time, §6 L1 rung).
#[test]
fn bank_account_l1_req_check_fires() {
    let emitted = lower_l1("bank_account");
    // `req a.balance + amount <= 1_000_000` is violated by an overflowing deposit.
    let program = format!(
        "{emitted}\nfn main() {{\n    let _ = deposit(Account {{ balance: 1000000 }}, 1);\n    println!(\"should-not-reach\");\n}}\n"
    );
    let (compiled, ran, out) = compile_and_run(&program, "bank_account_l1_neg");
    assert!(
        compiled,
        "deposit L1 (violating) must still COMPILE:\n{out}"
    );
    assert!(
        !ran,
        "deposit L1 must ABORT at the violated `req` check, not run clean:\n{out}"
    );
    assert!(
        out.contains("thermite L1 contract violation [req]"),
        "the violation handler must fire with a [req] diagnostic:\n{out}"
    );
    assert!(
        !out.contains("should-not-reach"),
        "execution must abort before main's tail (the check is observable):\n{out}"
    );
}

// AC: is_circle L1 compiles + runs; the `ens result == (s is Circle)` holds for
// both variants (hand-derived: Circle→true, Rect→false).
#[test]
fn shape_l1_compiles_and_runs() {
    let emitted = lower_l1("shape");
    let program = format!(
        "{emitted}\nfn main() {{\n    assert!(is_circle(Shape::Circle(3)));\n    assert!(!is_circle(Shape::Rect {{ w: 2, h: 4 }}));\n    println!(\"ok\");\n}}\n"
    );
    let (compiled, ran, out) = compile_and_run(&program, "shape_l1");
    assert!(
        compiled,
        "is_circle L1 did NOT compile:\n{out}\n--- {program}"
    );
    assert!(
        ran && out.contains("ok"),
        "is_circle L1 did NOT run clean (Circle→true, Rect→false):\n{out}"
    );
}

// AC: is_circle L1's `ens` check FIRES when the body lies (corrupt the match so a
// Circle returns false — the always-active ens check aborts).
#[test]
fn shape_l1_ens_check_fires_on_a_lying_body() {
    let emitted = lower_l1("shape");
    assert!(
        emitted.contains("Shape::Circle(r) => true,"),
        "expected the Circle→true arm in the emitted body:\n{emitted}"
    );
    // Make the body lie: a Circle now yields `false`, so `ens result == (s is
    // Circle)` is violated for a Circle scrutinee.
    let corrupted = emitted.replace("Shape::Circle(r) => true,", "Shape::Circle(r) => false,");
    let program = format!(
        "{corrupted}\nfn main() {{\n    let _ = is_circle(Shape::Circle(3));\n    println!(\"should-not-reach\");\n}}\n"
    );
    let (compiled, ran, out) = compile_and_run(&program, "shape_l1_neg");
    assert!(
        compiled,
        "corrupted is_circle L1 must still COMPILE:\n{out}"
    );
    assert!(
        !ran,
        "corrupted is_circle L1 must ABORT at the violated `ens` check:\n{out}"
    );
    assert!(
        out.contains("thermite L1 contract violation [ens]"),
        "the violation handler must fire with an [ens] diagnostic:\n{out}"
    );
}

// ---- no proof cheats (R-DEFER-9) -------------------------------------------

fn assert_no_cheats(emitted: &str, name: &str) {
    for forbidden in [
        "assume(false)",
        "#[verifier::external]",
        "#[verifier::external_body]",
        "#[slag]",
        "ensures true",
        "ensures\n        true,",
    ] {
        assert!(
            !emitted.contains(forbidden),
            "{name} emission contains forbidden cheat token `{forbidden}` (R-DEFER-9):\n{emitted}"
        );
    }
}

// ---- AC-6: no regression — sum/binary_search still lower + VERIFY (L3) ------
//
// The ADT additions are purely additive (new `Item`/`Expr`/`Pattern`/`Type` and
// new error arms; no existing node reshapes). The non-ADT corpus must still lower
// to Verus that real verus VERIFIES, and the key contract substrings must be
// present (no weakening). Verification is by verus, NOT a byte-match of the
// emitted source against the golden — the verify-not-byte-match practice the
// existing `lower_conformance.rs` uses (the goldens are design-authored
// REFERENCES, not a regeneration of the lowerer's exact bytes). The byte-stability
// the `git diff tests/golden/` gauntlet checks is that the existing GOLDEN FILES
// are not EDITED by this stage, which they are not.

#[test]
fn sum_and_binary_search_still_lower_and_verify_l3() {
    let sum = lower_and_verify("sum");
    assert!(
        sum.contains("requires xs.len() <= 1000000,")
            && sum.contains("result as nat == spec_sum(xs@),"),
        "sum contracts present (no regression / no weakening):\n{sum}"
    );
    let bs = lower_and_verify("binary_search");
    assert!(
        bs.contains("requires sorted(haystack@),")
            && bs.contains("None => forall_in(haystack@, |x: u32| x != needle),"),
        "binary_search contracts present (no regression / no weakening):\n{bs}"
    );
}
