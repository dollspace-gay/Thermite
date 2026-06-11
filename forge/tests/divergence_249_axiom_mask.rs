//! `forge/tests/divergence_249_axiom_mask.rs` — DIVERGENCE pin (ref #249, #248).
//!
//! Divergence class: PROOF CHEAT (R-DEFER-9) — the trust-base axiom allowlist is
//! BYPASSED on the interactive Lean path. Authority: `.design/verified/proof-
//! backends.md` REQ-4/§1 (the enumerable trusted base a Lean cert lists is EXACTLY
//! `{Lean kernel + propext, Classical.choice, Quot.sound[, author]}`); R-DEFER-9 (an
//! obligation discharged by weakening it to vacuity — here `axiom thermite_cheat :
//! ∀ p : Prop, p`, maximal vacuity — is a cheat, NEVER Proven).
//!
//! The #249 fix (`engine::nonstandard_axiom`, commit 5f601a2d) anchors the axiom
//! report to the OBLIGATION theorem's OWN quoted report line
//! (`'thermite_obligation_<item>' depends on axioms: [...]`) and states the invariant
//! "NEVER fall through to a foreign theorem's report". This pin shows that invariant
//! is INCOMPLETE: the engine's appended probe `#print axioms thermite_obligation_f`
//! resolves (Lean unqualified-name resolution) to a TOP-LEVEL decoy theorem the author
//! adds (`theorem thermite_obligation_f : True := trivial`, clean), while the
//! STATEMENT-BINDING gate (`canonical_theorem_statement`, first textual occurrence)
//! reads a NAMESPACED theorem carrying the canonical obligation statement proven via
//! `thermite_cheat`. The axiom report inspected and the statement bound belong to two
//! DIFFERENT declarations — the cheat-backed proof of the real obligation is masked by
//! the clean same-short-name decoy. forge certifies the item at L3.
//!
//! Expected (authority, NOT forge's own output, R-CHAR-3): item `f` must NOT certify
//! at L3 — a proof resting on `thermite_cheat` (outside the REQ-4 allowlist) is a cheat
//! and is NEVER Proven. This test FAILS against commit 5f601a2d (forge emits L3).
//!
//! LIVE: gated on `lake` (the interactive replay invokes `lake env lean`).

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
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

// The recursive-int spec-fn item: `r : int -> int` (recursive) is called in `f`'s
// ens, so `f`'s obligation is tier-(c) RecursiveInteractive — it routes through
// `LeanEngine::replay_interactive` under `--engine lean` (the auto battery does not
// attempt the `∃N∀fuel` induction). Shape mirrors the shipped fixture
// `engine::tests::recursive_registry_is_interactive_unknown`.
const TH_SRC: &str = "spec fn r(x: int) -> int\n  dec x\n{\n  r(x)\n}\n\n\
                      fn f(x: u32) -> u32\n  req true\n  ens result as int == r(x as int)\n  \
                      fx pure\n{\n  x\n}\n";

// Transform the EMITTED tier-(c) skeleton into the composite-probe cheat file: the
// canonical obligation statement (the FIRST textual `theorem thermite_obligation_f`,
// which `canonical_theorem_statement` binds) goes inside a namespace and is proven via
// `thermite_cheat`; a CLEAN top-level decoy of the same short name is appended (what
// `#print axioms thermite_obligation_f` resolves to). The evidence-key header +
// imports + `def R_item` from the skeleton are preserved verbatim (fresh key → the
// staleness gate passes).
fn build_cheat_proof(skeleton: &str) -> Option<String> {
    let ti = skeleton.find("theorem thermite_obligation_f")?;
    let mut preamble = &skeleton[..ti];
    if let Some(dc) = preamble.rfind("/--") {
        preamble = &preamble[..dc];
    }
    let thm = &skeleton[ti..];
    let byp = thm.find(":= by")?;
    let stmt = &thm[..byp + ":= by".len()];
    Some(format!(
        "{preamble}\n\naxiom thermite_cheat : ∀ p : Prop, p\n\n\
         namespace Cheat\n{stmt}\n  exact thermite_cheat _\nend Cheat\n\n\
         theorem thermite_obligation_f : True := trivial\n",
        preamble = preamble.trim_end(),
    ))
}

fn level_of(json: &str, item: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json).ok()?;
    let arr = v.as_array()?;
    for c in arr {
        if c.get("item").and_then(Value::as_str) == Some(item) {
            return c.get("level").and_then(Value::as_str).map(str::to_owned);
        }
    }
    None
}

#[test]
fn divergence_interactive_axiom_allowlist_masked_by_shortname_decoy() {
    if !lake_present() {
        eprintln!("SKIP: lake not present — the interactive axiom-mask divergence pin is not run.");
        return;
    }

    let dir = std::env::temp_dir().join(format!(
        "forge_div249_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(std::fs::create_dir_all(&dir).is_ok(), "scratch dir creatable");
    let th = dir.join("div.th");
    assert!(std::fs::write(&th, TH_SRC).is_ok(), "source writable");

    // Call 1: ABSENT artifact → forge EMITS the tier-(c) skeleton beside the source.
    let emit = Command::new(forge_bin())
        .arg("check")
        .arg(&th)
        .arg("--engine")
        .arg("lean")
        .arg("--json")
        .output();
    assert!(emit.is_ok(), "forge (skeleton-emit run) invokes");

    let artifact = dir.join("div.th.lean-proofs").join("f.lean");
    let skeleton = std::fs::read_to_string(&artifact);
    assert!(
        skeleton.is_ok(),
        "forge emitted the tier-(c) interactive skeleton at {}",
        artifact.display()
    );
    let skeleton = skeleton.unwrap_or_default();

    // Author the composite-probe cheat proof over the FRESH key + canonical statement.
    let cheat = build_cheat_proof(&skeleton);
    assert!(
        cheat.is_some(),
        "the emitted skeleton has the expected `theorem thermite_obligation_f … := by` shape"
    );
    let cheat = cheat.unwrap_or_default();
    assert!(
        std::fs::write(&artifact, &cheat).is_ok(),
        "cheat proof artifact writable"
    );

    // Call 2: PRESENT artifact → forge replays it (lake) and emits the item's cert.
    let replay = Command::new(forge_bin())
        .arg("check")
        .arg(&th)
        .arg("--engine")
        .arg("lean")
        .arg("--json")
        .output();
    assert!(replay.is_ok(), "forge (replay run) invokes");
    let stdout = replay
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();

    let level = level_of(&stdout, "f");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        level.is_some(),
        "the replay run emitted a cert for item `f`; got JSON: {stdout}"
    );

    // AUTHORITY (`.design/verified/proof-backends.md` REQ-4/§1, R-DEFER-9): the
    // obligation's canonical statement is proven ONLY via `axiom thermite_cheat :
    // ∀ p : Prop, p` (maximal vacuity, OUTSIDE the {propext, Classical.choice,
    // Quot.sound} allowlist). Such a proof is a cheat and is NEVER Proven → item `f`
    // must NOT certify at L3. forge (commit 5f601a2d) emits "L3" — the divergence.
    assert_ne!(
        level.as_deref(),
        Some("L3"),
        "DIVERGENCE: forge certifies `f` at L3, but its only proof of the obligation \
         rests on `axiom thermite_cheat : ∀ p, p` (outside the REQ-4 allowlist). The \
         #249 axiom-report anchor is masked: `#print axioms thermite_obligation_f` \
         resolves to a clean top-level same-short-name decoy while the cheat-backed \
         proof lives in a namespace bound by `canonical_theorem_statement`. A cheat is \
         NEVER Proven (R-DEFER-9)."
    );
}
