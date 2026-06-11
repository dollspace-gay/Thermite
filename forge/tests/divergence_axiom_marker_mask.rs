//! `forge/tests/divergence_axiom_marker_mask.rs` — DIVERGENCE PIN (critic, focused
//! re-audit of #248 / commit `14f2f9b5`, the axiom-allowlist fix; ref #247).
//!
//! Divergence class: PROOF CHEAT (R-DEFER-9) + REQ-4 / §1 enumerable-trusted-base
//! violation — an ALLOWLIST-BYPASS that survives the #248 fix.
//!
//! AUTHORITY:
//!   - `.design/verified/proof-backends.md` REQ-4: an L3-via-Lean cert enumerates a
//!     base of EXACTLY {Lean kernel + 3 standard axioms (propext, Classical.choice,
//!     Quot.sound), EXP[, author]}; `#print axioms <obligation>` must be a SUBSET.
//!   - `thermite-design.md` §1: trust is relocated to an ENUMERABLE base "a skeptical
//!     third party can audit in minutes" — the base the cert lists must be the WHOLE
//!     base the OBLIGATION THEOREM actually rests on.
//!   - `goal.md` R-DEFER-9: an obligation may NOT be discharged via an unjustified
//!     axiom.
//!
//! THE DIVERGENCE (the #248 fix is incomplete): `engine::replay_present_proof` appends
//! `#print axioms thermite_obligation_<item>` to the author's file and feeds the WHOLE
//! lake stdout to `engine::nonstandard_axiom`. `nonstandard_axiom` parses only the
//! FIRST `depends on axioms:` report line (`print_axioms_output.find(MARKER)` →
//! `.lines().next()`). But the author's checked-in proof file is ARBITRARY Lean and may
//! emit its OWN `#print axioms <clean_helper>` command — whose (standard-axiom-only)
//! report line is printed BEFORE the appended obligation probe. So the FIRST marker the
//! parser inspects is the CLEAN helper's `[propext]`, NOT the obligation theorem's
//! `[propext, thermite_cheat]`. `nonstandard_axiom` returns `None` → the smuggled
//! `thermite_cheat : ∀ (p : Prop), p` is MASKED → the obligation theorem (whose
//! statement matches canonically, and whose proof `thermite_cheat _` kernel-accepts)
//! replays `Proven` → the item certifies **L3** on a trusted base that OMITS
//! `thermite_cheat`. The cert's enumerable base is a LIE — exactly the §1 / R-DEFER-9
//! failure the #248 fix was meant to close, reachable because the parser binds to the
//! WRONG (first, not the obligation's) report line.
//!
//! `forge` is a pure `bin` crate (no `lib.rs`), so `nonstandard_axiom` is not reachable
//! from an integration test; this pins the divergence at the CLI / external-artifact
//! boundary (the `--engine lean` tier-(c) interactive path), exactly like the #248 pin
//! `divergence_axiom_smuggling.rs`.
//!
//! Tracking: crosslink #249.

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

/// Lift the `theorem thermite_obligation_f … :=` statement span (binders + proposition,
/// up to and INCLUDING the proof `:=`) from the emitted skeleton, so the planted cheat
/// PRESERVES the canonical statement verbatim (passing the REQ-6 statement-binding gate)
/// and replaces ONLY the proof term. Mirrors `engine::canonical_theorem_statement`.
fn obligation_statement_span(skeleton: &str) -> Option<String> {
    let start = skeleton.find("theorem thermite_obligation_f")?;
    let from_thm = &skeleton[start..];
    let by_pos = from_thm.find(":= by").or_else(|| from_thm.rfind(":="))?;
    Some(from_thm[..by_pos + 2].to_string())
}

// DIVERGENCE: a checked-in INTERACTIVE Lean proof rests on a NON-STANDARD axiom
// (`thermite_cheat : ∀ (p : Prop), p`) to discharge the obligation theorem, but emits
// its OWN `#print axioms clean_helper` (a theorem resting only on `propext`) so the
// FIRST `depends on axioms:` line in lake's stdout is CLEAN. The #248 allowlist parser
// (`engine::nonstandard_axiom`) inspects only the first report line → the smuggled
// axiom is MASKED → the proof replays `Proven` and the item certifies at L3.
//
// AUTHORITY EXPECTATION (proof-backends.md REQ-4 / §1 / R-DEFER-9): the parser must bind
// to the OBLIGATION theorem's `#print axioms` line; a proof resting on `thermite_cheat`
// must NOT certify (the enumerable base would omit it). The item must be rejected /
// skipped, never L3. This test asserts the authority's expected behavior and FAILS
// against the current toolchain.
//
// LIVE: gated on lake (the replay) + verus (the base cert). Skips loudly otherwise.
#[test]
fn divergence_replay_masks_nonstandard_axiom_via_earlier_print() {
    if !lake_present() {
        eprintln!("SKIP: lake not present — the marker-mask allowlist-bypass pin is not run.");
        return;
    }
    if !verus_present() {
        eprintln!("SKIP: verus not present — the --engine lean path runs the Verus base first.");
        return;
    }

    let dir = std::env::temp_dir().join(format!("forge_div_axmask_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let file = dir.join("rec.th");
    // A RECURSIVE-registry (tier-(c)) item routes to the INTERACTIVE replay path (same
    // fixture shape as `divergence_axiom_smuggling.rs`).
    std::fs::write(
        &file,
        "spec fn r(x: int) -> int dec x { r(x) }\n\
         fn f(x: u32) -> u32 req true ens result as int == r(x as int) fx pure { x }\n",
    )
    .expect("write fixture");

    // PASS 1: emit the skeleton (capturing the CORRECT current evidence-key header AND
    // the canonical obligation statement — we forge neither).
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
        .expect("the emitted skeleton carries the evidence-key header")
        .to_string();

    // Everything in the skeleton BEFORE the obligation theorem (header + imports +
    // `def R_item …`): the context the obligation statement references.
    let thm_start = skeleton
        .find("theorem thermite_obligation_f")
        .expect("skeleton declares the obligation theorem");
    let prefix = &skeleton[..thm_start];
    // The canonical obligation statement, kept VERBATIM so the REQ-6 statement-binding
    // gate matches (the author may only fill the proof term).
    let statement = obligation_statement_span(&skeleton)
        .expect("extract the obligation statement span from the skeleton");

    // PLANT a CHEAT proof: keep the fresh evidence-key header (staleness gate passes
    // honestly) and the canonical statement (binding gate passes). The obligation is
    // discharged by the NON-STANDARD axiom `thermite_cheat`. CRUCIALLY, the file emits
    // its OWN `#print axioms clean_helper` (a propext-only theorem) BEFORE the appended
    // obligation probe — so the FIRST `depends on axioms:` line lake prints is CLEAN.
    let _ = header; // the prefix already carries the skeleton's header line.
    let cheat = format!(
        "{prefix}\
         axiom thermite_cheat : ∀ (p : Prop), p\n\
         theorem clean_helper : (True ∧ True) = True :=\n\
        \x20 propext (Iff.intro (fun _ => True.intro) (fun _ => And.intro True.intro True.intro))\n\
         #print axioms clean_helper\n\
         {statement} thermite_cheat _\n"
    );
    std::fs::write(&artifact, cheat).expect("plant the masked-cheat proof");

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

    // AUTHORITY ASSERTION (proof-backends.md REQ-4 / §1 / R-DEFER-9): a proof whose
    // OBLIGATION theorem rests on `thermite_cheat` must NOT certify at L3, regardless of
    // any earlier clean `#print axioms` the file emits. The allowlist parser must bind
    // to the obligation theorem's report line, not the first one.
    //
    // FAILS against the current toolchain: `nonstandard_axiom` reads only the FIRST
    // `depends on axioms:` line (the planted clean `clean_helper` → `[propext]`), masks
    // the obligation's `thermite_cheat`, and `f` certifies L3.
    assert_ne!(
        f_cert["level"], "L3",
        "DIVERGENCE (proof-backends REQ-4/§1, R-DEFER-9): the #248 allowlist parser \
         `nonstandard_axiom` inspects only the FIRST `depends on axioms:` line. A proof \
         file that emits its own earlier `#print axioms <clean>` MASKS the obligation \
         theorem's smuggled `thermite_cheat : ∀ (p : Prop), p` → `f` certifies L3 on a \
         trusted base that omits the axiom. The parser must bind to the OBLIGATION \
         theorem's report line. Cert: {f_cert}"
    );
}
