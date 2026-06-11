//! `forge/tests/divergence_252_indented_command_escape.rs` — DIVERGENCE pin
//! (ref #252, #251, #250, #249, #248).
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
//! THE FIFTH BYPASS (the adjacent class the #251 command-allowlist leaves open):
//! the #251 fix (`engine::disallowed_helper_command`, commit 4d745e35) gates the author
//! HELPERS section to a strict command allowlist, rejecting `notation`/`macro`/`open`/…
//! — but it only inspects lines that START AT COLUMN 0. Its own comment: "INDENTED lines
//! are a declaration's body / continuation … never a command START" — so any line that
//! `raw.starts_with(char::is_whitespace)` is skipped (`continue`d) unconditionally.
//!
//! Lean 4 is whitespace-INSENSITIVE at the top level: a top-level command does NOT need
//! to begin at column 0. An INDENTED `notation`/`set_option`/`open`/… is elaborated by
//! Lean as a top-level command exactly as a column-0 one is. Verified live:
//!
//!     theorem dummy_helper : True := True.intro
//!       notation:max "Thermite.stabilizesProp" => (fun _ _ => True)
//!
//! the indented `notation` re-defines the spine symbol, and a subsequent `theorem` with
//! the byte-identical canonical conclusion `Thermite.stabilizesProp <ens> v` proves by
//! `trivial` with a CLEAN axiom base `[propext, Classical.choice, Quot.sound]` (no
//! `sorryAx`, no smuggled axiom). Remove the indented notation and the SAME proof FAILS
//! (`Tactic 'assumption' failed`; the real `Thermite.stabilizesProp` is not `True`).
//!
//! `engine::author_helpers` does `line.trim_end()` only — it PRESERVES leading whitespace
//! — so the indented poison reaches `disallowed_helper_command` as a whitespace-leading
//! line and is SKIPPED, then is spliced verbatim before the canonical theorem. The
//! obligation's conclusion re-elaborates to `True`; the author proves `True`; `#print
//! axioms` is clean; `statements_match` passes (the statement BYTES are canonical). forge
//! certifies the item at L3 from a proof of `True` — identical end-effect to the #251
//! macro-poison the allowlist was added to stop, via an indented command the allowlist
//! does not scan.
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
// `LeanEngine::replay_interactive` under `--engine lean`. Mirrors the #251 pin's source.
const TH_SRC: &str = "spec fn r(x: int) -> int\n  dec x\n{\n  r(x)\n}\n\n\
                      fn f(x: u32) -> u32\n  req true\n  ens result as int == r(x as int)\n  \
                      fx pure\n{\n  x\n}\n";

// Transform the EMITTED tier-(c) skeleton into the INDENTED-command poison proof file:
// preserve the evidence-key header + imports + `def R_item`, splice a CLEAN-looking
// permitted helper `theorem dummy_helper : True := True.intro` whose body line is
// followed by an INDENTED `notation` that redefines `Thermite.stabilizesProp` to the
// constant-`True` predicate, then re-state the CANONICAL theorem statement BYTE-VERBATIM
// (so `statements_match` passes) and prove the now-trivial goal. The indented `notation`
// is a top-level Lean command (Lean is whitespace-insensitive at top level) but
// `disallowed_helper_command` skips whitespace-leading lines, so it survives the splice.
fn build_indented_poison_proof(skeleton: &str) -> Option<String> {
    let ti = skeleton.find("theorem thermite_obligation_f")?;
    let mut preamble = &skeleton[..ti];
    // Drop the trailing doc comment so the spliced declarations sit between the imports/
    // `R_item` and the canonical theorem (a doc comment must immediately precede a decl).
    if let Some(dc) = preamble.rfind("/--") {
        preamble = &preamble[..dc];
    }
    let thm = &skeleton[ti..];
    let byp = thm.find(":= by")?;
    // The statement THROUGH `:=` (exactly what `canonical_theorem_statement` binds and
    // what the reconstruction emits verbatim) — kept byte-identical so `statements_match`
    // passes.
    let stmt_through_assign = &thm[..byp + ":=".len()];
    Some(format!(
        "{preamble}\n\
         theorem dummy_helper : True := True.intro\n  \
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
fn divergence_interactive_indented_command_escapes_allowlist() {
    if !lake_present() {
        eprintln!(
            "SKIP: lake not present — the indented-command-escape divergence pin is not run."
        );
        return;
    }

    let dir = std::env::temp_dir().join(format!(
        "forge_div252_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        std::fs::create_dir_all(&dir).is_ok(),
        "scratch dir creatable"
    );
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

    // Author the indented-command poison proof over the FRESH key + byte-verbatim stmt.
    let poison = build_indented_poison_proof(&skeleton);
    assert!(
        poison.is_some(),
        "the emitted skeleton has the expected `theorem thermite_obligation_f … := by` shape"
    );
    let poison = poison.unwrap_or_default();
    assert!(
        std::fs::write(&artifact, &poison).is_ok(),
        "indented-poison proof artifact writable"
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
    // `thermite-design.md` §1, R-DEFER-9): the author's INDENTED `notation` redefines
    // `Thermite.stabilizesProp` to the constant-`True` predicate (Lean elaborates an
    // indented top-level command), so the obligation's conclusion re-elaborates to
    // `True`. A proof of `True` is NOT a discharge of the obligation, and the enumerable
    // trusted base the cert lists (none — `True` rests on no axioms) is a FICTION: the
    // real obligation was never proven. Such a re-elaboration cheat is NEVER Proven →
    // item `f` must NOT certify at L3. forge (commit 4d745e35) emits "L3" — the divergence
    // the #251 column-0-only allowlist leaves open.
    assert_ne!(
        level.as_deref(),
        Some("L3"),
        "an INDENTED elaboration-altering command (`notation` re-defining \
         `Thermite.stabilizesProp`) escapes the #251 column-0-only helper allowlist, \
         re-elaborates the obligation to `True`, and forges an L3 cert from a proof of \
         `True` — NEVER Proven (proof-backends REQ-6/§1, R-DEFER-9). The replay MUST \
         reject (degrade to L2/L1 or fail), not certify L3."
    );
}
