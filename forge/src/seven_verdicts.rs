//! `forge/src/seven_verdicts.rs` — the seven-verdict hermetic suite (`.design/
//! stage1-forge-tier.md` REQ-10 / AC-14: "a hermetic test per verdict (seven tests named
//! for their verdict)"). One `#[test]` per cert-level verdict
//! ([`crate::verdict::CertVerdict`]), each named for its verdict and each exercising the
//! verdict at the boundary that PRODUCES it, so the gate's "seven hermetic tests" claim is
//! literally met. The suite runs under `cargo test -p forge` — the F4 lean job runs it with
//! the built spine, and the z3-gated producers run there (z3 ships with verus).
//!
//! Why the production boundary (not always the full binary): the relax route's
//! `Proved`/`Counterexample`/`RealWitness` are produced by [`crate::engine::NlsatEngine`]
//! (z3-gated — they run in CI where z3 is present); `CovenantRefuted` is produced by the
//! verus-FREE covenant engine (ungated); `Stuck`/`KernelBudget`/`Timeout` are produced
//! UPSTREAM by [`crate::verdict::cert_verdict_for_lean`] from the lean transcript (a LIVE
//! kernel-budget exhaustion is inherently non-deterministic, so the hermetic exercise is
//! the deterministic transcript→verdict→cert production the live path funnels through —
//! `KernelBudget` additionally drives the real cert builder [`crate::check::
//! lean_unverifiable_cert`]). Together: every one of the seven verdicts has a named,
//! hermetic, CI-run test. The closure-instrumented never-degrades tests
//! (`crate::degrade`) are unchanged.
#![cfg(test)]

use crate::engine::{NlsatEngine, NlsatOutcome, Reason, Verdict};
use crate::manifest::{Certificate, Level};
use crate::verdict::cert_verdict_for_lean;
use thermite_syntax::{FnItem, Item, Program};

/// Parse a single-`fn` source and return the program + the named `fn`.
fn parse_fn(src: &str, name: &str) -> (Program, FnItem) {
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
    let f = parsed
        .program
        .items
        .iter()
        .find_map(|i| match i {
            Item::Fn(f) if f.name == name => Some(f.clone()),
            _ => None,
        })
        .expect("the named fn is present");
    (parsed.program, f)
}

/// VERDICT 1/7 — **Proved**: a relaxable polynomial side-condition the nlsat real
/// relaxation decides valid (QF_NRA-unsat over the negation) certifies at L4 with the
/// `Proved` verdict. The producer is [`NlsatEngine::discharge_relax`]; z3-gated, so it runs
/// in the CI lean job (z3 ships with verus).
#[test]
fn verdict_proved() {
    if !NlsatEngine::z3_present() {
        eprintln!("SKIP verdict_proved: z3 absent (runs in the CI lean job where z3 is present)");
        return;
    }
    let (program, f) = parse_fn(
        "fn p(lo: u64, hi: u64) -> u64\n  req lo <= hi\n  ens lo + 1 <= hi + 1\n  fx pure\n{ lo }\n",
        "p",
    );
    let outcome = NlsatEngine::new(program).discharge_relax(&f);
    assert!(
        matches!(outcome, NlsatOutcome::Proved),
        "a valid relaxable side-condition is Proved (L4); got {outcome:?}"
    );
}

/// VERDICT 2/7 — **Counterexample**: a relaxable clause with a genuine INTEGER falsifier
/// (the real relaxation is `sat` and an integer point in the radius-2 box falsifies it) is
/// a `Counterexample` carrying the integer witness — never escalated. z3-gated.
#[test]
fn verdict_counterexample() {
    if !NlsatEngine::z3_present() {
        eprintln!("SKIP verdict_counterexample: z3 absent (runs in the CI lean job)");
        return;
    }
    // `∀ n, n + 1 <= n` is false at every integer — the nlsat route finds an integer
    // counterexample (not a real-only witness).
    let (program, f) = parse_fn(
        "fn c(n: u64) -> u64\n  req true\n  ens n + 1 <= n\n  fx pure\n{ n }\n",
        "c",
    );
    let outcome = NlsatEngine::new(program).discharge_relax(&f);
    assert!(
        matches!(outcome, NlsatOutcome::Counterexample { .. }),
        "an integer-falsifiable clause is a Counterexample; got {outcome:?}"
    );
}

/// VERDICT 3/7 — **RealWitness**: a clause true over ℤ but false over ℝ (`∀ n, n·n ≠ 2`)
/// yields a `RealWitness` carrying the raw real point (√2), escalated UP to the forge —
/// NEVER a `Counterexample`. This is the relax PRODUCER path the AC-14 audit flagged; it is
/// z3-gated, so it runs in the CI lean job (z3 ships with verus). The matching UNGATED
/// structural producer test lives in `engine.rs`
/// (`classify_sat_real_only_model_is_real_witness`).
#[test]
fn verdict_real_witness() {
    if !NlsatEngine::z3_present() {
        eprintln!("SKIP verdict_real_witness: z3 absent (runs in the CI lean job)");
        return;
    }
    let (program, f) = parse_fn(
        "fn w(n: u64) -> u64\n  req true\n  ens n * n != 2\n  fx pure\n{ n }\n",
        "w",
    );
    let outcome = NlsatEngine::new(program).discharge_relax(&f);
    match outcome {
        NlsatOutcome::RealWitness { point } => {
            assert!(
                !point.assignment.is_empty(),
                "the RealWitness carries the raw real point"
            );
        }
        other => panic!("`∀ n. n·n ≠ 2` must be a RealWitness, never {other:?}"),
    }
}

/// VERDICT 4/7 — **CovenantRefuted**: a planted-bug `fn` whose body violates its `ens` on a
/// `req`-satisfying input is refuted by the verus-FREE covenant `falsify` run (a hard fail,
/// never degraded). Ungated — the covenant engine is pure executable evaluation, no prover.
#[test]
fn verdict_covenant_refuted() {
    use crate::covenant_engine::{analyze_covenant, covenant_gate, witness_bindings, CovenantGate};
    // `bad` claims `result >= x && result >= y` but returns the SMALLER — refuted.
    let (program, f) = parse_fn(
        "fn bad(x: u64, y: u64) -> u64\n  req true\n  ens result >= x && result >= y\n  fx pure\n\
         { if x > y { y } else { x } }\n\nwitness { inhabit (5, 5); inhabit (9, 2); falsify 2000; }\n",
        "bad",
    );
    let witness = witness_bindings(&program)
        .get("bad")
        .cloned()
        .expect("the witness block covenants `bad`");
    match covenant_gate(analyze_covenant(&f, &witness), |_record| {}) {
        CovenantGate::Refuted { evidence, .. } => {
            assert_eq!(
                evidence.falsify_refuted, 1,
                "the covenant was refuted (a falsify hit)"
            );
        }
        CovenantGate::Burned { .. } | CovenantGate::Refused { .. } => {
            panic!("a planted-bug covenant must be CovenantRefuted")
        }
    }
}

/// VERDICT 5/7 — **Stuck**: a lean proof that ELABORATED but left a residual goal
/// ("unsolved goals") is `Stuck` (the frozen-battery residual + missing-bridge hint),
/// produced UPSTREAM by [`cert_verdict_for_lean`] — never silently `Proved` and never the
/// solver `Timeout` the 3-arm engine map would assign. Ungated (a transcript→verdict
/// producer test; the live discharge that emits such a transcript is exercised by
/// `battery_conformance.rs`).
#[test]
fn verdict_stuck() {
    let residual = "error: unsolved goals\n  ⊢ i + 1 ≤ n";
    let v = cert_verdict_for_lean(
        residual,
        &Verdict::Unknown(Reason::IncompleteUnknown("unsolved goals".to_string())),
    );
    assert_eq!(
        v.kind(),
        "Stuck",
        "a residual-goal transcript is Stuck, got {v:?}"
    );
}

/// VERDICT 6/7 — **KernelBudget**: a lean elaboration/kernel-budget exhaustion (the
/// textually-distinct `(deterministic) timeout … maximum number of heartbeats` signal) is
/// `KernelBudget`, produced UPSTREAM — NEVER mis-mapped to the solver `Timeout`. This is the
/// AC-14 e2e gap: a LIVE budget exhaustion is non-deterministic, so the hermetic exercise
/// drives the deterministic production path the live discharge funnels through — both the
/// verdict producer ([`cert_verdict_for_lean`]) AND the real cert builder
/// ([`crate::check::lean_unverifiable_cert`]), asserting the produced certificate is
/// classified `KernelBudget`.
#[test]
fn verdict_kernel_budget() {
    let budget_out = "error: (deterministic) timeout at `isDefEq`, maximum number of \
                      heartbeats (200000) has been reached";
    // (a) the verdict producer classifies it KernelBudget (not Timeout).
    let v = cert_verdict_for_lean(
        budget_out,
        &Verdict::Unknown(Reason::IncompleteUnknown("lake nonzero".to_string())),
    );
    assert_eq!(
        v.kind(),
        "KernelBudget",
        "a heartbeat-timeout transcript is KernelBudget"
    );
    // (b) the real cert builder produces a certificate that honestly attributes the
    // budget exhaustion (end-to-end through the production cert path).
    let base = Certificate::new(
        "kb_item",
        Level::L0,
        vec!["pure".to_string()],
        0,
        Vec::new(),
    );
    let cert = crate::check::lean_unverifiable_cert(
        &base,
        &Reason::IncompleteUnknown(budget_out.to_string()),
    );
    let detail = cert
        .reject
        .as_ref()
        .map(|r| r.detail.clone())
        .unwrap_or_default();
    assert!(
        detail.contains("KernelBudget"),
        "the produced cert attributes the budget exhaustion as KernelBudget: {detail}"
    );
}

/// VERDICT 7/7 — **Timeout**: a solver resource-limit (rlimit) exhaustion is the
/// `Verdict::Unknown` image under the TOTAL engine map — a `Timeout`, the only non-kernel,
/// non-residual incompleteness. Ungated (the engine-map producer; the live forced-timeout
/// run is exercised by `profile_conformance.rs`).
#[test]
fn verdict_timeout() {
    let rlimit = "error: rlimit exceeded; resource limit reached";
    let v = cert_verdict_for_lean(
        rlimit,
        &Verdict::Unknown(Reason::VerusTimeout("rlimit exceeded".to_string())),
    );
    assert_eq!(
        v.kind(),
        "Timeout",
        "an rlimit transcript is Timeout, got {v:?}"
    );
}
