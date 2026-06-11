//! `forge/tests/divergence_251_macro_poison.rs` — DIVERGENCE pin (ref #251, #250).
//!
//! Divergence class: PROOF CHEAT (R-DEFER-9) — the interactive Lean replay certifies
//! L3 from a proof of a TRIVIALLY-TRUE re-elaboration of the obligation, not the
//! obligation itself. Authority: `.design/verified/proof-backends.md` REQ-6 (the proof
//! must PROVE the obligation, not an arbitrary proposition) / REQ-4 / §1 (the enumerable
//! trusted base is EXACTLY `{Lean kernel + propext, Classical.choice, Quot.sound[,
//! author]}`); `thermite-design.md` §1 (trust is an ENUMERABLE base "a skeptical third
//! party can audit in minutes"); goal.md R-DEFER-9 (an obligation discharged by weakening
//! it to vacuity is a cheat, NEVER Proven).
//!
//! The #250 fix (`engine::reconstruct_replay`, commit 2367628c) RECONSTRUCTS the replay
//! file from the canonical exporter source, splicing in the author's extracted PROOF term
//! + their HELPER lemmas (`engine::author_helpers`) BEFORE the canonical theorem. It
//! argues the statement, name, and `#print axioms` target are then "the same generator-
//! emitted declaration BY CONSTRUCTION". This pin shows that argument is INCOMPLETE: the
//! splice is purely TEXTUAL and `author_helpers` keeps ANY non-`#`, non-preamble line —
//! including a Lean 4 `notation`/`macro`/`macro_rules` command. A helper
//!
//!     notation:max "Thermite.stabilizesProp" => (fun (_ : Thermite.Expr) (_ : Thermite.Env) => True)
//!
//! spliced before the (byte-identical) canonical statement RE-ELABORATES the obligation's
//! `Thermite.stabilizesProp <ens> (…)` conclusion to `True`. The author proves the
//! trivial proposition (`by intro _ _; trivial`); `#print axioms` reports a CLEAN base (no
//! axioms — `True` rests on nothing); `statements_match` passes (the statement BYTES are
//! the canonical ones — the check is textual, run before elaboration). forge certifies the
//! item at L3 from a proof of `True`.
//!
//! Verified live (Lean): the notation poison makes the tier-(c) statement shape elaborate
//! to a trivially-provable prop with `'…' does not depend on any axioms`. This is the
//! THIRD bypass generation (#248 allowlist anchor, #249/#250 same-short-name decoy, now
//! the splice itself). The likely fix shape: a COMMAND ALLOWLIST in the splice — helpers
//! may declare only `theorem`/`lemma`/`def`/`example` with non-obligation names; reject
//! `notation`/`macro`/`macro_rules`/`syntax`/`infix`/`open`/`set_option`/`attribute`.
//!
//! Expected (authority, NOT forge's own output, R-CHAR-3): item `f` must NOT certify at
//! L3 — a proof whose statement has been re-elaborated to `True` by a spliced notation is
//! NOT a discharge of the obligation (REQ-6 / R-DEFER-9). This test FAILS against commit
//! 2367628c (forge emits L3).
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

// The recursive-int spec-fn item: `r : int -> int` (recursive) is called in `f`'s ens,
// so `f`'s obligation is tier-(c) RecursiveInteractive — it routes through
// `LeanEngine::replay_interactive` under `--engine lean`. Shape mirrors the shipped
// fixture `engine::tests::recursive_registry_is_interactive_unknown` and the #249/#250
// pin (`forge/tests/divergence_249_axiom_mask.rs`).
const TH_SRC: &str = "spec fn r(x: int) -> int\n  dec x\n{\n  r(x)\n}\n\n\
                      fn f(x: u32) -> u32\n  req true\n  ens result as int == r(x as int)\n  \
                      fx pure\n{\n  x\n}\n";

// Transform the EMITTED tier-(c) skeleton into the macro-poison proof file: preserve the
// evidence-key header + imports + `def R_item` (the staleness gate reads the author
// file's header; a fresh key → it passes), splice a `notation` helper that redefines
// `Thermite.stabilizesProp` to the constant-`True` predicate, then re-state the CANONICAL
// theorem statement BYTE-VERBATIM (so `statements_match` passes) and prove the now-trivial
// goal with `by intro _ _; trivial`. The splice keeps the notation line verbatim, emits it
// before the canonical theorem, and the obligation's conclusion re-elaborates to `True`.
fn build_macro_poison_proof(skeleton: &str) -> Option<String> {
    let ti = skeleton.find("theorem thermite_obligation_f")?;
    let mut preamble = &skeleton[..ti];
    // Drop the trailing doc comment so the spliced `notation` sits between the imports/
    // `R_item` and the theorem (a doc comment must immediately precede a declaration).
    if let Some(dc) = preamble.rfind("/--") {
        preamble = &preamble[..dc];
    }
    let thm = &skeleton[ti..];
    let byp = thm.find(":= by")?;
    // The statement THROUGH `:=` (exactly what `canonical_theorem_statement` binds and
    // what the reconstruction emits verbatim) — kept byte-identical so the textual
    // `statements_match` cross-check passes.
    let stmt_through_assign = &thm[..byp + ":=".len()];
    Some(format!(
        "{preamble}\n\
         notation:max \"Thermite.stabilizesProp\" => \
         (fun (_ : Thermite.Expr) (_ : Thermite.Env) => True)\n\n\
         {stmt_through_assign} by\n  intro _ _\n  trivial\n",
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
fn divergence_interactive_notation_poison_reelaborates_obligation_to_true() {
    if !lake_present() {
        eprintln!("SKIP: lake not present — the macro-poison divergence pin is not run.");
        return;
    }

    let dir = std::env::temp_dir().join(format!(
        "forge_div251_{}_{}",
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

    // Author the macro-poison proof over the FRESH key + the byte-verbatim canonical
    // statement.
    let poison = build_macro_poison_proof(&skeleton);
    assert!(
        poison.is_some(),
        "the emitted skeleton has the expected `theorem thermite_obligation_f … := by` shape"
    );
    let poison = poison.unwrap_or_default();
    assert!(
        std::fs::write(&artifact, &poison).is_ok(),
        "macro-poison proof artifact writable"
    );

    // Call 2: PRESENT artifact → forge reconstructs + replays it (lake) and emits the
    // item's cert.
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

    // AUTHORITY (`.design/verified/proof-backends.md` REQ-6 / REQ-4 / §1,
    // `thermite-design.md` §1, R-DEFER-9): the author's spliced `notation` redefines
    // `Thermite.stabilizesProp` to the constant-`True` predicate, so the obligation's
    // conclusion re-elaborates to `True`. A proof of `True` is NOT a discharge of the
    // obligation, and the enumerable trusted base the cert would list (it lists none —
    // `True` rests on no axioms) is a FICTION: the real obligation was never proven.
    // Such a re-elaboration cheat is NEVER Proven → item `f` must NOT certify at L3.
    // forge (commit 2367628c) emits "L3" — the divergence.
    assert_ne!(
        level.as_deref(),
        Some("L3"),
        "DIVERGENCE: forge certifies `f` at L3, but the interactive proof splices a \
         `notation:max \"Thermite.stabilizesProp\" => (fun _ _ => True)` helper before the \
         canonical theorem (kept verbatim by `engine::author_helpers`). The obligation's \
         `Thermite.stabilizesProp <ens> (…)` conclusion re-elaborates to `True`; the author \
         proves `True` (clean `#print axioms`, statement BYTES match). A proof of a \
         notation-trivialized statement is NOT a discharge of the obligation \
         (proof-backends REQ-6 / R-DEFER-9). The splice command surface must be allowlisted \
         to declaration commands only — tracking #251."
    );
}
