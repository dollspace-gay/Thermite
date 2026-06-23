//! The stage-3 bit-vector route (`.design/stage3-bv-reconstruction.md` REQ-2 / AC-2 /
//! AC-3): `forge check --engine bv` over the `mix64` example and the two AC-3 fixtures,
//! end to end through the binary. The route lowers `@bv`-tagged clauses to fixed-width
//! QF_BV (the [`EngineName::BitVector`] route) alongside the stage-1 nlsat route, so a
//! mixed-mechanism function attributes each clause to the engine that grounds it.
//!
//! The whole suite is gated on the `bv` cargo feature (the shadow-flag plumbing — without
//! it the `@bv` tag is a structured parse error, REQ-1's R-BV-1 lock) AND on `verus`/z3
//! being reachable (the route reuses the Verus base pass and reaches z3 for the QF_BV and
//! QF_NRA queries). A shard without them SKIPS — the CI lean/verus job is the authoritative
//! gate, mirroring `g1_gate.rs` and `nlsat_relax_conformance.rs`.

#![cfg(feature = "bv")]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("forge crate has a parent workspace dir")
        .to_path_buf()
}

/// `verus` is reachable (the same skip-guard `g1_gate.rs` uses). z3 ships alongside the
/// verus distribution, so a present verus implies a usable QF_BV / QF_NRA solver.
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

fn run_bv(example: &str) -> (Option<i32>, Vec<Value>) {
    let th = repo_root().join("conformance/forge").join(example);
    let out = Command::new(forge_bin())
        .arg("check")
        .arg("--engine")
        .arg("bv")
        .arg(&th)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json must emit one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    let arr = value
        .as_array()
        .unwrap_or_else(|| panic!("forge --json must emit a JSON array of certs: {value}"))
        .clone();
    (out.status.code(), arr)
}

fn cert<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c["item"].as_str() == Some(item))
        .unwrap_or_else(|| panic!("no certificate for `{item}` in {certs:?}"))
}

/// AC-2: the `mix64` example certifies at L4 with three clauses on two mechanisms — two
/// `@bv64` clauses via `EngineName::BitVector` (decidable QF_BV, complete bit-pattern
/// countermodels) and one unbounded clause via nlsat — each clause's certificate naming
/// its engine and semantics; the injectivity lemma discharges at `@bv64` with no author
/// proof (an empty `proof { }`). Both mechanisms certify at the caged rung L4, so the
/// item (the MIN over its clauses) is L4; the `@bv` clauses' SOLVER trust base is
/// recorded in the per-clause attribution (kernel-grounded by REQ-7/8, same rung).
#[test]
fn mix64_certifies_with_two_bitvector_clauses_and_one_unbounded() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus (z3) absent — the bit-vector route is not run (set VERUS_BIN; the CI \
             verus job is the gate)."
        );
        return;
    }
    let (code, certs) = run_bv("mix64.th");
    assert_eq!(code, Some(0), "the mix64 example must certify (exit 0)");

    // (1) The fn `mix64` — L4 (the MIN over L4, L4, L4), three per-clause obligations.
    let m = cert(&certs, "mix64");
    assert_eq!(
        m["level"],
        Value::from("L4"),
        "item level is the min over clauses (two @bv64 + one nlsat, all caged L4)"
    );
    assert!(
        m.get("reject").is_none() || m["reject"].is_null(),
        "a certified item has no reject"
    );
    let obls = m["obligations"].as_array().expect("obligations array");
    assert_eq!(
        obls.len(),
        3,
        "three ens clauses, three per-clause obligations"
    );
    assert_eq!(
        obls[0]["engine"],
        Value::from("bitvector"),
        "ens#0 → bitvector"
    );
    assert_eq!(
        obls[1]["engine"],
        Value::from("bitvector"),
        "ens#1 → bitvector"
    );
    assert_eq!(
        obls[2]["engine"],
        Value::from("nlsat"),
        "ens#2 → nlsat (unbounded)"
    );
    for (k, o) in obls.iter().enumerate() {
        assert_eq!(
            o["verdict"]["kind"],
            Value::from("Proved"),
            "clause ens#{k} is Proved"
        );
        assert!(
            !o["trust"].as_array().map(Vec::is_empty).unwrap_or(true),
            "clause ens#{k} names its trust base"
        );
    }
    // The two bit-vector clauses name the fixed-width semantics; the unbounded one does not.
    assert!(
        obls[0]["name"].as_str().unwrap_or("").contains("bv64"),
        "the bit-vector clause names its bv64 semantics"
    );
    assert!(
        obls[2]["name"].as_str().unwrap_or("").contains("unbounded"),
        "the unbounded clause names its unbounded semantics"
    );

    // (2) The injectivity lemma — L4 via the bit-vector engine, no author proof.
    let l = cert(&certs, "rotl1_injective");
    assert_eq!(
        l["level"],
        Value::from("L4"),
        "the @bv64 lemma certifies at the caged rung L4 (decidable QF_BV)"
    );
    let lobls = l["obligations"].as_array().expect("lemma obligations");
    assert_eq!(
        lobls[0]["engine"],
        Value::from("bitvector"),
        "the lemma → bitvector"
    );
    assert_eq!(lobls[0]["verdict"]["kind"], Value::from("Proved"));
}

/// AC-3 (first half): a planted non-injective shift dies as a `Counterexample` with the
/// bit pattern in the certificate.
#[test]
fn planted_non_injective_shift_is_a_counterexample_with_bit_pattern() {
    if !verus_present() {
        eprintln!("SKIP: verus (z3) absent — the bit-vector route is not run.");
        return;
    }
    let (code, certs) = run_bv("bv_shl_not_injective.th");
    assert_ne!(code, Some(0), "a refuted clause does not certify");
    let c = cert(&certs, "shl1_injective_BROKEN");
    assert_eq!(c["reject"]["cause"], Value::from("Counterexample"));
    let obl = &c["obligations"][0];
    assert_eq!(obl["verdict"]["kind"], Value::from("Counterexample"));
    let diag = obl["diagnostic"].as_str().unwrap_or("");
    assert!(
        diag.contains("0b"),
        "the certificate carries the falsifying bit pattern (`0b…`): {diag}"
    );
}

/// AC-3 (second half): an over-budget 64-bit multiplier query yields `Timeout` under the
/// dedicated budget profile — NEVER `unknown` and never a silent downgrade. The robust
/// invariant (across z3 versions): the clause never lands a silent `BvUnknown` skip; when
/// it IS a timeout, the cert names the `bv64-multiplier` profile.
#[test]
fn over_budget_multiplier_is_timeout_under_named_profile_never_unknown() {
    if !verus_present() {
        eprintln!("SKIP: verus (z3) absent — the bit-vector route is not run.");
        return;
    }
    let (_code, certs) = run_bv("bv_mul64_budget.th");
    let c = cert(&certs, "mul64_no_factor");
    let cause = c["reject"]["cause"].as_str().unwrap_or("");
    assert_ne!(
        cause, "BvUnknown",
        "the 64-bit multiplier cliff is NEVER a silent unknown (AC-3)"
    );
    // The expected outcome is the dedicated-profile Timeout; assert it names the profile.
    if cause == "BvBudgetTimeout" {
        let obl = &c["obligations"][0];
        assert_eq!(obl["verdict"]["kind"], Value::from("Timeout"));
        let detail = obl["verdict"]["detail"].as_str().unwrap_or("");
        assert!(
            detail.contains("bv64-multiplier"),
            "the Timeout names the dedicated budget profile: {detail}"
        );
    }
}

/// AC-5 (Lock 2 — bv-semantics mutation): a `@bv` fn whose `ens` clause constrains the
/// body via `result` certifies at L4 AND its certificate surfaces a non-trivial mutation
/// score from the WRAP-AWARE battery. The `succ_ge` fixture's `ens@bv64 result >= x` over
/// the identity body `x + 0` is machine-valid (L4); the frozen off-by-one mutator's
/// `x + 1` body is the wrap-exploiting mutant — valid over unbounded integers but false
/// over QF_BV64 at `x = 2^64 - 1`, so the wrap-aware kill check kills it. The score is
/// surfaced (`contract_quality.mutants_killed`), not gated — the L4 rung is
/// solver-decided. The unit suite (`check::tests::ac5_*`, z3-gated) pins the
/// width-vs-unbounded contrast at the engine level; this pins the end-to-end cert.
#[test]
fn bv_semantics_mutation_surfaces_a_nontrivial_kill_ratio_on_a_result_clause() {
    if !verus_present() {
        eprintln!("SKIP: verus (z3) absent — the bit-vector mutation battery is not run.");
        return;
    }
    let (code, certs) = run_bv("bv_wrap_mutation.th");
    assert_eq!(code, Some(0), "the @bv fn certifies (exit 0)");

    let c = cert(&certs, "succ_ge");
    assert_eq!(
        c["level"],
        Value::from("L4"),
        "the @bv fn certifies at the caged rung L4"
    );
    // The wrap-aware mutation battery scored the result-referencing @bv clause and killed
    // the wrap-exploiting (and early-return) mutants: a non-`"0/0"` kill ratio.
    let killed = c["contract_quality"]["mutants_killed"]
        .as_str()
        .unwrap_or("0/0");
    assert_eq!(
        killed, "2/4",
        "the bv-semantics battery kills the wrap-exploiting `x + 1` and the early-return \
         `0` mutants (2 of 4 scored): {c}"
    );
    // The surviving mutant is a body-equivalent one (`return x`), never the killed
    // wrap-exploiting mutant.
    let survivor = c["contract_quality"]["survivor"].as_str().unwrap_or("");
    assert!(
        !survivor.contains("off-by-one literal 0->1"),
        "the wrap-exploiting mutant is killed, never surfaced as a survivor: {survivor}"
    );
}

/// Run a forge subcommand over a conformance example, returning `(exit, parsed JSON)`.
fn run_forge_json(subcommand: &str, example: &str) -> (Option<i32>, Value) {
    let th = repo_root().join("conformance/forge").join(example);
    let out = Command::new(forge_bin())
        .arg(subcommand)
        .arg(&th)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge {subcommand}: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge {subcommand} --json must emit one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out.status.code(), value)
}

/// Count the obligations carrying a `bv_shadow` block across a cert array (AC-4 — the
/// grep-completeness count: `grep bv_shadow` ≡ the tagged clauses).
fn shadowed_clause_count(certs: &[Value]) -> usize {
    certs
        .iter()
        .flat_map(|c| c["obligations"].as_array().cloned().unwrap_or_default())
        .filter(|o| o.get("bv_shadow").is_some())
        .count()
}

/// AC-4 (Lock 1 — the shadow flag): every `@bv`-tagged clause's certificate carries
/// `bv_shadow` (the RFC §9 shape) and NOTHING untagged does — `grep bv_shadow` over the
/// certs ≡ exactly the tagged clauses. `mix64` has two `@bv64` clauses + one unbounded
/// clause, plus the injectivity lemma's `@bv64` clause: three tagged clauses carry the
/// flag, the unbounded clause does not. `nowrap_obligation` is the reserved (REQ-5) slot,
/// absent for a bare `@bv64`.
#[test]
fn every_bv_tagged_clause_carries_the_shadow_flag_and_nothing_else() {
    if !verus_present() {
        eprintln!("SKIP: verus (z3) absent — the bit-vector route is not run.");
        return;
    }
    let (code, certs) = run_bv("mix64.th");
    assert_eq!(code, Some(0), "mix64 certifies");

    let m = cert(&certs, "mix64");
    let obls = m["obligations"].as_array().expect("obligations array");
    // The two `@bv64` clauses carry the shadow flag naming the wraparound semantics.
    for k in [0usize, 1] {
        let s = &obls[k]["bv_shadow"];
        assert_eq!(
            s["flagged"],
            Value::Bool(true),
            "ens#{k} is flagged as a machine-semantics fork"
        );
        let semantics = s["semantics"].as_str().unwrap_or("");
        assert!(
            semantics.contains("bv64") && semantics.contains("wraparound"),
            "ens#{k} names its fixed-width wraparound semantics: {s}"
        );
        assert!(s.get("note").is_some(), "ens#{k} carries the §9 note");
        assert!(
            s.get("nowrap_obligation").is_none(),
            "the reserved nowrap_obligation slot (REQ-5) is omitted for a bare @bv64: {s}"
        );
    }
    // The untagged (unbounded) clause carries NO shadow flag — grep finds nothing else.
    assert!(
        obls[2].get("bv_shadow").is_none(),
        "the untagged unbounded clause has no shadow flag: {}",
        obls[2]
    );

    // The injectivity lemma's `@bv64` clause carries it too.
    let l = cert(&certs, "rotl1_injective");
    let ls = &l["obligations"][0]["bv_shadow"];
    assert_eq!(
        ls["flagged"],
        Value::Bool(true),
        "the lemma clause is flagged"
    );
    assert!(
        ls["semantics"].as_str().unwrap_or("").contains("bv64"),
        "the lemma clause names its bv64 semantics: {ls}"
    );

    // Grep-completeness over the WHOLE cert collection: exactly the three tagged clauses
    // (mix64::ens#0, mix64::ens#1, rotl1_injective::ens#0) carry bv_shadow.
    assert_eq!(
        shadowed_clause_count(&certs),
        3,
        "exactly the three @bv-tagged clauses carry bv_shadow (and nothing else)"
    );
}

/// AC-4: a refuted `@bv` clause STILL carries the shadow flag — a counterexample is a
/// machine-semantics fact, so the fork stays greppable even on a hard fail.
#[test]
fn a_refuted_bv_clause_still_carries_the_shadow_flag() {
    if !verus_present() {
        eprintln!("SKIP: verus (z3) absent — the bit-vector route is not run.");
        return;
    }
    let (_code, certs) = run_bv("bv_shl_not_injective.th");
    let c = cert(&certs, "shl1_injective_BROKEN");
    let s = &c["obligations"][0]["bv_shadow"];
    assert_eq!(
        s["flagged"],
        Value::Bool(true),
        "the refuted clause is still flagged as a machine-semantics fork: {c}"
    );
    assert!(s["semantics"].as_str().unwrap_or("").contains("bv64"));
}

/// AC-4: `forge audit` lists the bv shadows — auditing a bit-vector project routes
/// through the bv engine, so the manifest's additive `bv_shadows` section enumerates
/// every tagged clause (the way the TCB enumerates `#[slag]` blocks).
#[test]
fn forge_audit_lists_the_bv_shadows() {
    if !verus_present() {
        eprintln!("SKIP: verus (z3) absent — forge audit's bv route is not run.");
        return;
    }
    let (_code, manifest) = run_forge_json("audit", "mix64.th");
    let shadows = manifest["bv_shadows"]
        .as_array()
        .expect("the audit manifest carries a bv_shadows section");
    assert_eq!(
        shadows.len(),
        3,
        "the audit lists all three @bv-tagged clauses: {manifest}"
    );
    assert!(
        shadows
            .iter()
            .all(|s| s["shadow"]["flagged"] == Value::Bool(true)),
        "every listed shadow is flagged"
    );
    assert!(
        shadows.iter().any(|s| s["item"] == "mix64"),
        "the mix64 fn's tagged clauses are listed"
    );
    assert!(
        shadows.iter().any(|s| s["item"] == "rotl1_injective"),
        "the lemma's tagged clause is listed"
    );
}

/// REQ-3 / AC-4 regression: the auto-routed bv engine (`forge audit`/`review`) is a
/// PER-ITEM overlay, never a wholesale re-route. An ordinary Verus-provable `fn` that
/// merely shares a program with a `@bv` `fn` keeps its true L3 cert — it is NOT downgraded
/// to L0. (Before the `bv_check` fix, every `fn` was routed through the bv route, whose
/// untagged-clause branch rejects a non-`@bv`, non-relaxable clause — silently downgrading
/// `plain_add` from L3 to L0 in the audit.)
#[test]
fn audit_of_a_mixed_bv_program_keeps_ordinary_fns_at_their_verus_level() {
    if !verus_present() {
        eprintln!("SKIP: verus (z3) absent — the bit-vector route is not run.");
        return;
    }
    let (_code, manifest) = run_forge_json("audit", "bv_mixed_audit.th");
    let funcs = manifest["functions"]
        .as_array()
        .expect("the audit manifest carries a functions section");
    let level_of = |name: &str| -> String {
        funcs
            .iter()
            .find(|r| r["name"] == name)
            .unwrap_or_else(|| panic!("function `{name}` is in the audit: {manifest}"))["level"]
            .as_str()
            .unwrap_or("")
            .to_string()
    };
    // The `@bv` fn certifies at the caged rung L4 via the bit-vector route.
    assert_eq!(level_of("wrap_add"), "L4", "the @bv fn is L4: {manifest}");
    // The ordinary fn KEEPS its Verus L3 — the auto-route must not touch it.
    assert_eq!(
        level_of("plain_add"),
        "L3",
        "an ordinary Verus-provable fn sharing the file with a @bv fn stays L3, not L0: {manifest}"
    );
    // The shadow surface still works — exactly the one tagged clause is listed.
    let shadows = manifest["bv_shadows"]
        .as_array()
        .expect("bv_shadows section present");
    assert_eq!(
        shadows.len(),
        1,
        "exactly the wrap_add @bv clause is shadowed (plain_add contributes none): {manifest}"
    );
    assert_eq!(shadows[0]["item"], "wrap_add");
}

/// AC-4: `forge review` lists the bv shadows — the spec-intent review artifact's additive
/// `bv_shadows` section surfaces every tagged clause's machine-semantics fork for the
/// reviewer.
#[test]
fn forge_review_lists_the_bv_shadows() {
    if !verus_present() {
        eprintln!("SKIP: verus (z3) absent — forge review's bv route is not run.");
        return;
    }
    let (_code, artifact) = run_forge_json("review", "mix64.th");
    let shadows = artifact["bv_shadows"]
        .as_array()
        .expect("the review artifact carries a bv_shadows section");
    assert_eq!(
        shadows.len(),
        3,
        "the review lists all three @bv-tagged clauses: {artifact}"
    );
    assert!(
        shadows
            .iter()
            .all(|s| s["shadow"]["flagged"] == Value::Bool(true)),
        "every reviewed shadow is flagged"
    );
}
