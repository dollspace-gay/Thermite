//! #52 §9 boundary-composition cert oracle (`.design/lower/boundary-composition.md`
//! AC-1..AC-4; `conformance/composition/cases.json`). Drives the BUILT `forge`
//! binary with `check --json` over each case's program (written to a temp `.th`
//! file so the read-only `conformance/` fixtures stay untouched — R-CHAR-3) and
//! asserts the §9 composition contract: a caller of a `#[boundary]`/`#[slag]` fn
//! that HONORS the callee's contract verifies THROUGH it at `L3` (was `L0` before
//! #52, the undefined-callee verus error) with `assurance_scope = to_boundary`
//! (#17); a caller that VIOLATES the callee's `req` is a COUNTEREXAMPLE, never a
//! false `L3`.
//!
//! THE HONESTY GATE (`goal.md` R-DEFER-9): `#[verifier::external_body]` is emitted
//! ONLY for the declared `#[boundary]`/`#[slag]` fn. A regular Thermite fn is
//! ALWAYS fully proved — a lying regular body is CAUGHT (the
//! `lying_regular_fn_is_caught` test), never laundered to L3 by the composition
//! path. The corpus (`sum`, `binary_search`) is UNAFFECTED: it references only
//! `spec fn`s / combinators, so no external_body is woven and the cert stays L3
//! END-TO-END (the `corpus_unaffected_*` tests).
//!
//! Expected values trace to the golden `conformance/composition/cases.json` and
//! `thermite-design.md` §9 (R-CHAR-3), NEVER copied from forge's own output.
//! These SKIP LOUDLY if verus is absent — the §9 composition proof is a real verus
//! run (the boundary caller L3-proves against the assumed contract).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn conformance_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

fn cases() -> Value {
    let path = conformance_dir().join("composition").join("cases.json");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read composition cases.json: {e}"));
    serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse composition cases.json: {e}"))
}

/// `true` iff verus can be located — mirrors `e2e_conformance.rs`. The §9
/// composition cases run a real verus proof (the boundary caller L3-proves
/// against the boundary fn's assumed contract), so the prover must be present.
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

fn write_temp_program(name: &str, program: &str) -> PathBuf {
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("composition_{name}_{pid}.th"));
    std::fs::write(&path, program).unwrap_or_else(|e| panic!("write temp .th: {e}"));
    path
}

/// Run `forge check <file> --json`, returning the parsed cert array (or empty on a
/// non-JSON / non-zero result), the exit code, and stderr.
fn run_check_json(file: &Path) -> (Option<i32>, Vec<Value>, String) {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(file)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let certs = serde_json::from_str::<Value>(stdout.trim())
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    (out.status.code(), certs, stderr)
}

fn find_cert<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no certificate for item `{item}` in {certs:?}"))
}

// AC-1 (direct boundary caller → L3 + to_boundary): the `direct_boundary_caller`
// case — `caller` calls `#[boundary] ext_id` and honors its `req` + proves its own
// `ens` THROUGH ext_id's assumed `ensures` → `caller` is L3 (was L0 before #52)
// with scope to_boundary via ext_id; `ext_id` itself stays L1 + boundary
// (UNCHANGED, REQ-3). Anchored to `cases.json` `verifies_to_boundary`.
#[test]
fn direct_boundary_caller_verifies_through_the_contract() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — composition direct-caller oracle not run.");
        return;
    }
    let oracle = cases();
    for case in oracle["verifies_to_boundary"]
        .as_array()
        .expect("verifies_to_boundary array")
    {
        let name = case["name"].as_str().expect("name");
        let item = case["fn"].as_str().expect("fn");
        let program = case["program"].as_str().expect("program");
        let expect_level = case["expect_level"].as_str().expect("expect_level");
        let expect_via = case["expect_via"].as_str().expect("expect_via");

        let path = write_temp_program(name, program);
        let (code, certs, stderr) = run_check_json(&path);
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            code,
            Some(0),
            "case `{name}`: the boundary caller certifies (no env error); stderr:\n{stderr}"
        );
        let cert = find_cert(&certs, item);
        // REQ-2: the caller reaches L3 THROUGH the assumed contract (was L0).
        assert_eq!(
            cert["level"],
            Value::from(expect_level),
            "`{item}` (case {name}) proves L3 through `{expect_via}`'s contract (#52)"
        );
        // #17 scope ⊥ level: L3 AND to_boundary via the crossing.
        assert_eq!(
            cert["assurance_scope"]["kind"],
            Value::from("to_boundary"),
            "`{item}` (case {name}) records to_boundary (its closure crosses {expect_via})"
        );
        assert_eq!(
            cert["assurance_scope"]["via"],
            Value::from(expect_via),
            "`{item}` (case {name}) records the oracle crossing `via`"
        );

        // REQ-3: the boundary fn `ext_id` itself stays L1 + boundary, UNCHANGED.
        let ext = find_cert(&certs, expect_via);
        assert_eq!(
            ext["level"],
            Value::from("L1"),
            "the boundary fn `{expect_via}` stays L1 (the §16 path, untouched by #52)"
        );
        assert_eq!(
            ext["boundary"],
            Value::from(true),
            "the boundary fn `{expect_via}` keeps boundary == true"
        );
    }
}

// AC-2 (transitive caller → L3 + to_boundary): the `transitive_boundary_caller`
// case — `h → g → ext_id` — `h`'s sub-program weaves BOTH `g` (real body, proved)
// AND `ext_id` (external_body signature), so `h` proves L3 through the contracts;
// scope to_boundary via ext_id. Anchored to `cases.json` `transitive`.
#[test]
fn transitive_boundary_caller_weaves_real_and_external_body_deps() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — composition transitive oracle not run.");
        return;
    }
    let oracle = cases();
    for case in oracle["transitive"].as_array().expect("transitive array") {
        let name = case["name"].as_str().expect("name");
        let item = case["fn"].as_str().expect("fn");
        let program = case["program"].as_str().expect("program");
        let expect_level = case["expect_level"].as_str().expect("expect_level");
        let expect_via = case["expect_via"].as_str().expect("expect_via");

        let path = write_temp_program(name, program);
        let (code, certs, stderr) = run_check_json(&path);
        let _ = std::fs::remove_file(&path);

        assert_eq!(code, Some(0), "case `{name}` certifies; stderr:\n{stderr}");
        let cert = find_cert(&certs, item);
        assert_eq!(
            cert["level"],
            Value::from(expect_level),
            "`{item}` (case {name}) proves L3: its sub-program weaves the real `g` body \
             AND the `{expect_via}` external_body signature (#52)"
        );
        assert_eq!(
            cert["assurance_scope"]["kind"],
            Value::from("to_boundary"),
            "`{item}` (case {name}) is to_boundary (transitive crossing)"
        );
        assert_eq!(
            cert["assurance_scope"]["via"],
            Value::from(expect_via),
            "`{item}` (case {name}) records the transitive crossing `via`"
        );
        // The intermediary `g` ALSO proves L3 + to_boundary (it directly crosses).
        let g = find_cert(&certs, "g");
        assert_eq!(
            g["level"],
            Value::from("L3"),
            "the intermediary `g` proves L3 too"
        );
    }
}

// AC-3 (req-violating caller → counterexample, NOT a false L3): the
// `req_violating_caller` case — `bad`'s `req` is `true`, so it does NOT establish
// `ext_id`'s `req` (x < 100) at the call site → `precondition not satisfied` →
// NON-L3 counterexample. The external_body assumes ext_id's ENSURES but the caller
// must still discharge its REQ. THE soundness AC (R-DEFER-9 anti-cheat). Anchored
// to `cases.json` `counterexample`.
#[test]
fn req_violating_caller_is_a_counterexample_not_a_false_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — composition counterexample oracle not run.");
        return;
    }
    let oracle = cases();
    for case in oracle["counterexample"]
        .as_array()
        .expect("counterexample array")
    {
        let name = case["name"].as_str().expect("name");
        let item = case["fn"].as_str().expect("fn");
        let program = case["program"].as_str().expect("program");
        let expect_level = case["expect_level"].as_str().expect("expect_level");

        let path = write_temp_program(name, program);
        let (_code, certs, _stderr) = run_check_json(&path);
        let _ = std::fs::remove_file(&path);

        let cert = find_cert(&certs, item);
        // The decisive anti-cheat assertion: NOT a false L3.
        assert_ne!(
            cert["level"],
            Value::from("L3"),
            "`{item}` (case {name}) does NOT reach L3 — it fails ext_id's req (soundness)"
        );
        assert_eq!(
            cert["level"],
            Value::from(expect_level),
            "`{item}` (case {name}) is the L0 counterexample (precondition not satisfied)"
        );
        // "Counterexample, not adjective" (§5.1): a per-obligation failure witness.
        let obligations = cert["obligations"].as_array().expect("obligations array");
        assert!(
            obligations
                .iter()
                .any(|r| r["status"].as_str() == Some("failed")),
            "`{item}` (case {name}) carries a failed-obligation witness, not a bare reject"
        );
    }
}

// AC-4 + THE HONESTY GATE: the pure corpus references only spec fns / combinators,
// so NO external_body is woven and the cert stays L3 END-TO-END, byte-stable. The
// lowered string for a pure sub-program contains NO `external_body` substring
// (the load-bearing OQ-1 invariant): external_body appears IFF a woven dependency
// is `#[boundary]`/`#[slag]`. We exercise the corpus through the real pipeline.
#[test]
fn corpus_unaffected_stays_l3_end_to_end() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — corpus-unaffected oracle not run.");
        return;
    }
    for (file, item) in [("sum.th", "sum"), ("binary_search.th", "binary_search")] {
        let path = conformance_dir().join(file);
        let (code, certs, stderr) = run_check_json(&path);
        assert_eq!(
            code,
            Some(0),
            "corpus `{file}` certifies; stderr:\n{stderr}"
        );
        let cert = find_cert(&certs, item);
        assert_eq!(
            cert["level"],
            Value::from("L3"),
            "corpus `{item}` stays L3 (unaffected by #52 — no boundary/slag reference)"
        );
        // END-TO-END: no crossing in the closure, so no external_body was woven.
        let scope_is_e2e = match cert.get("assurance_scope") {
            None | Some(Value::Null) => true,
            Some(s) => s.get("kind").and_then(|v| v.as_str()) == Some("end_to_end"),
        };
        assert!(
            scope_is_e2e,
            "corpus `{item}` is END-TO-END (closure reaches no boundary/slag)"
        );
    }
}

// AC-8 (#269 — the WEAK-callee conservatism fixture, equivalent-mutants.md REQ-8):
// a `#[boundary]` `ext_weak` whose `ens result <= 100` does NOT pin its result,
// and `wcaller(x) { ext_weak(x) }` carrying the SAME un-pinning `ens result <=
// 100`. The caller's F-IDENT identity survivor `return x` PROVES against the
// caller's own contract (`x < 100 ⟹ x <= 100`) → SURVIVOR; but the call-bearing
// equivalence harness CANNOT prove `real == mutant` (ext_weak's `ens` does not pin
// `real == x`) → NOT excluded → the survivor STAYS counted → `wcaller` gates
// `WeakContract`. The genuine #101 anti-launder line, one level up: a mutant the
// callee contracts cannot prove equivalent is conservatively COUNTED (R-DEFER-9).
// Expected from §9 + equivalent-mutants.md REQ-8 (hand-derived), NOT forge output.
#[test]
fn weak_callee_identity_survivor_stays_counted_and_gates() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — weak-callee conservatism oracle not run.");
        return;
    }
    // ext_weak's `ens result <= 100` is too weak to pin `real == x`, so wcaller's
    // identity mutant `return x` is a counted survivor → wcaller gates WeakContract.
    let program = "#[boundary(\"ext::ext_weak\")] fn ext_weak(x: u32) -> u32 req x < 100 ens result <= 100 fx pure ; \
         fn wcaller(x: u32) -> u32 req x < 100 ens result <= 100 fx pure { ext_weak(x) }";
    let path = write_temp_program("weak_callee_caller", program);
    let (_code, certs, stderr) = run_check_json(&path);
    let _ = std::fs::remove_file(&path);

    let cert = find_cert(&certs, "wcaller");
    // The decisive conservatism assertion: NOT laundered to a certifying L3 — the
    // identity survivor the weak callee cannot pin EQUIVALENT keeps the gate.
    assert_ne!(
        cert["level"],
        Value::from("L3"),
        "`wcaller` does NOT certify L3 — its identity survivor is NOT excluded \
         (ext_weak's `ens` cannot pin `real == x`), so the gate fires (REQ-8); \
         stderr:\n{stderr}"
    );
    // The §7 verdict-in-cert reject shape: WeakContract with a named survivor.
    assert_eq!(
        cert["reject"]["cause"],
        Value::from("WeakContract"),
        "`wcaller` gates `WeakContract` (the contract under-constrains the body \
         through the weak callee contract); stderr:\n{stderr}"
    );
    // The decisive REQ-8 evidence that the identity survivor was NOT excluded: the
    // kill ratio's DENOMINATOR still counts it. ext_weak cannot pin `real == x`
    // (identity) NOR `real == 0` (zero-return), so BOTH survivors stay counted —
    // a `0/2` ratio. Had the call-bearing probe falsely excluded the identity (the
    // #269 bug it must NOT reintroduce), the denominator would be `0/1`. The detail
    // carries the `K/N` ratio verbatim.
    let detail = cert["reject"]["detail"].as_str().unwrap_or("");
    assert!(
        detail.contains("0/2"),
        "BOTH the identity and zero-return survivors stay counted (`0/2`) — the \
         weak callee cannot pin either equivalent, so neither is excluded (REQ-8 \
         conservatism — the call-bearing probe never falsely excludes); reject \
         detail was:\n{detail}"
    );
}

// THE HONESTY GATE (R-DEFER-9, OQ-1): external_body is emitted ONLY for a
// `#[boundary]`/`#[slag]` fn. A REGULAR fn with a LYING body (`ens result == x + 1`
// over a body returning `x`) is NEVER laundered to L3 by the composition path — it
// is fully proved and CAUGHT (`postcondition not satisfied`). This is the contrast
// the grounded harness (3) pins: the IDENTICAL body under external_body would
// "verify" 0/0, but a regular fn never gets external_body. Expected from §9 /
// R-DEFER-9 (the lying body must fail), not forge output.
#[test]
fn lying_regular_fn_is_caught_never_laundered_to_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — honesty-gate oracle not run.");
        return;
    }
    // A REGULAR fn (no #[boundary]/#[slag]) whose body returns `x` but whose `ens`
    // claims `result == x + 1`. It must be fully proved and FAIL.
    let program = "fn liar(x: u32) -> u32 req x < 100 ens result == x + 1 fx pure { x }";
    let path = write_temp_program("lying_regular_fn", program);
    let (_code, certs, stderr) = run_check_json(&path);
    let _ = std::fs::remove_file(&path);

    let cert = find_cert(&certs, "liar");
    assert_ne!(
        cert["level"],
        Value::from("L3"),
        "a lying REGULAR fn is NEVER laundered to L3 (external_body is boundary/slag-only); \
         stderr:\n{stderr}"
    );
    let obligations = cert["obligations"].as_array().expect("obligations array");
    assert!(
        obligations
            .iter()
            .any(|r| r["status"].as_str() == Some("failed")),
        "the lying regular fn carries a postcondition-failure witness (caught, R-DEFER-9)"
    );
}
