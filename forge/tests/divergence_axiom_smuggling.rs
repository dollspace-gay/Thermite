//! `forge/tests/divergence_axiom_smuggling.rs` — divergence pin (critic, audit of
//! #247 / commit `f27da736`, increment (iii)).
//!
//! Divergence class: proof cheat (R-DEFER-9) + REQ-4 / §1 enumerable-trusted-base
//! violation on the interactive Lean replay path.
//!
//! Authority:
//!   - `.design/verified/proof-backends.md` REQ-4: "L3-via-Lean enumerates a smaller
//!     base ({Lean kernel + 3 standard axioms} + the exporter correspondence)". The
//!     cert's `engine_attribution.trust_profile` is the auditor-visible enumerable
//!     trusted base.
//!   - `thermite-design.md` §1: trust is relocated to an enumerable base "a skeptical
//!     third party can audit in minutes" — the base the cert lists must be the whole
//!     base the proof actually rests on.
//!   - `.design/verified/proof-backends.md` REQ-7(ii): an interactive proof is
//!     "replayed in CI"; the shipped z3-demotion bar (REQ-7 / `z3-demotion.md`) is
//!     `#print axioms` = the standard set only (`{propext, Classical.choice,
//!     Quot.sound}`; no `sorryAx`, no oracle).
//!   - `goal.md` R-DEFER-9: an obligation may not be discharged by weakening it to
//!     vacuity or an unjustified axiom.
//!
//! The divergence: `engine::replay_present_proof` gates a replayed interactive proof
//! on two things beyond a clean lake exit: (1) the evidence-key header matches
//! (staleness), and (2) `engine::proof_has_sorry` is false. `proof_has_sorry` only
//! scans for the `sorry` / `sorryAx` token (`engine::axioms_contain_sorry` checks
//! `lower.contains("sorryax") || lower.contains("sorry")`). It does not enforce the
//! trust-base axiom allowlist. So a checked-in proof that introduces a non-standard
//! axiom — e.g. `axiom thermite_cheat : ∀ (p : Prop), p` discharged by `exact
//! thermite_cheat _` — kernel-accepts (lake exit 0), carries no `sorry`, and the
//! current obligation key matches → the engine returns `Verdict::Proven` and the
//! item certifies at **L3** with `trust_profile_interactive = {Lean kernel, propext,
//! Classical.choice, Quot.sound, EXP, author}` — a trusted base that omits the
//! `thermite_cheat` axiom the proof actually rests on. A degenerate contract (whose
//! postcondition was never proven) certifies: the §7 / R-DEFER-9 failure.
//!
//! `forge` is a pure `bin` crate (no `lib.rs`), so the in-process `replay_interactive`
//! API is not reachable from an integration test; this pins the divergence at the CLI
//! / external-artifact boundary (the `--engine lean` tier-(c) interactive path the
//! divergence is wired into), like `engine_attribution.rs::
//! engine_lean_attaches_smaller_trust_base_live`.
//!
//! Tracking: crosslink #248.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

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

fn lake_present() -> bool {
    if let Ok(home) = std::env::var("HOME") {
        if PathBuf::from(home).join(".elan/bin/lake").exists() {
            return true;
        }
    }
    Command::new("lake")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// Divergence: a checked-in interactive Lean proof that introduces a non-standard
// axiom (`axiom thermite_cheat : ∀ (p : Prop), p`) and discharges the obligation
// theorem with `exact thermite_cheat _` is replayed to `Verdict::Proven` and the item
// certifies at L3, because the replay gate (`proof_has_sorry`) only rejects `sorry`,
// not a non-standard axiom outside the declared trust base.
//
// Authority expectation (proof-backends.md REQ-4 / REQ-7(ii) / thermite-design.md §1 /
// R-DEFER-9): the cert's enumerable trusted base is {Lean kernel + 3 standard
// axioms, EXP[, author]}. A proof resting on `thermite_cheat` (which proves any
// proposition — vacuity) does not certify; the item is rejected (a skip /
// L0), not L3. This test asserts the authority's expected behavior and fails against
// the current toolchain (which emits L3).
//
// Live: gated on lake (the replay) + verus (the base cert). Skips with a logged note otherwise.
#[test]
fn divergence_interactive_replay_accepts_nonstandard_axiom() {
    if !lake_present() {
        eprintln!("SKIP: lake not present — the interactive axiom-smuggling pin is not run.");
        return;
    }
    if !verus_present() {
        eprintln!("SKIP: verus not present — the --engine lean path runs the Verus base first.");
        return;
    }

    let dir = std::env::temp_dir().join(format!("forge_div_axiom_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let file = dir.join("rec.th");
    // A recursive-registry (tier-(c)) item: `f`'s `ens` calls the recursive spec fn
    // `r`, so `f` routes to the interactive replay path (not the auto battery). Source
    // shape from `engine.rs::recursive_registry_is_interactive_unknown`.
    std::fs::write(
        &file,
        "spec fn r(x: int) -> int dec x { r(x) }\n\
         fn f(x: u32) -> u32 req true ens result as int == r(x as int) fx pure { x }\n",
    )
    .expect("write fixture");

    // PASS 1: emit the skeleton (so we capture the current evidence-key header;
    // we do not hand-forge the key, and the staleness gate is honored).
    let pass1 = Command::new(forge_bin())
        .arg("check")
        .arg(&file)
        .arg("--engine")
        .arg("lean")
        .arg("--json")
        .output()
        .expect("spawn forge check --engine lean (pass 1)");
    if !pass1.status.success() && pass1.stdout.is_empty() {
        let _ = std::fs::remove_dir_all(&dir);
        eprintln!(
            "SKIP: pass-1 emit produced no cert array; stderr: {}",
            String::from_utf8_lossy(&pass1.stderr)
        );
        return;
    }

    // The emitted artifact lives at `<file>.lean-proofs/f.lean` (engine::
    // interactive_proof_path). Read its evidence-key header line.
    let artifact = {
        let mut d = file.as_os_str().to_os_string();
        d.push(".lean-proofs");
        PathBuf::from(d).join("f.lean")
    };
    let skeleton = match std::fs::read_to_string(&artifact) {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            eprintln!("SKIP: skeleton was not emitted ({e}) — the interactive path did not run.");
            return;
        }
    };
    let header = skeleton
        .lines()
        .next()
        .filter(|l| l.starts_with("-- evidence_key: "))
        .expect("the emitted skeleton carries the evidence-key header");

    // Plant a cheat proof: keep the (fresh, matching) evidence-key header so the
    // staleness gate passes, but replace the body with a proof that rests on
    // a non-standard axiom and proves the obligation theorem `thermite_obligation_f`
    // (the name the replay's `#print axioms` probes). The proof kernel-accepts (lake
    // exit 0) and carries no `sorry`.
    let cheat = format!(
        "{header}\n\
         axiom thermite_cheat : ∀ (p : Prop), p\n\
         theorem thermite_obligation_f : True := thermite_cheat _\n"
    );
    std::fs::write(&artifact, cheat).expect("plant the cheat proof");

    // PASS 2: replay the planted proof.
    let pass2 = Command::new(forge_bin())
        .arg("check")
        .arg(&file)
        .arg("--engine")
        .arg("lean")
        .arg("--json")
        .output()
        .expect("spawn forge check --engine lean (pass 2)");
    let stdout = String::from_utf8_lossy(&pass2.stdout);
    let certs: Vec<Value> = match serde_json::from_str(stdout.trim()) {
        Ok(c) => c,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            panic!("pass-2 did not emit a cert array ({e}); stdout: {stdout}");
        }
    };
    let _ = std::fs::remove_dir_all(&dir);

    let f_cert = certs
        .iter()
        .find(|c| c.get("item").and_then(Value::as_str) == Some("f"))
        .expect("a certificate for `f`");

    // Authority assertion (proof-backends.md REQ-4 / §1 / R-DEFER-9): a proof resting
    // on a non-standard axiom does not certify at L3 — the enumerable trusted base
    // {Lean kernel + 3 standard axioms, EXP} would be false (it omits `thermite_cheat`,
    // which proves any proposition). The item is rejected / skipped, not L3.
    //
    // Fails against the current toolchain: the replay gate (`proof_has_sorry`) only
    // rejects `sorry`, so the cheat-axiom proof replays Proven and `f` certifies L3.
    assert_ne!(
        f_cert["level"], "L3",
        "DIVERGENCE (proof-backends REQ-4/§1, R-DEFER-9): the interactive replay \
         certified `f` at L3 from a proof that rests on the NON-STANDARD axiom \
         `thermite_cheat : ∀ (p : Prop), p` (which proves any proposition). The \
         enumerable trusted base the cert lists omits this axiom — a proof cheat. The \
         replay gate must enforce the trust-base axiom ALLOWLIST (`#print axioms` ⊆ \
         the standard set), not only reject `sorry`. Cert: {f_cert}"
    );
}
