//! Divergence pin (crosslink #192, ref #166 #189), now satisfied by the root-cause
//! fix: the rlimit/timeout signal discriminator was duplicated across the three forge
//! translation-validation phases (`contract_tv`, `body_tv`, `exec_tv`) and the copies
//! drifted. `contract_tv::is_rlimit_signal` dropped the `"resource limit exceeded"`
//! phrase, so the bundled z3's own resourceout literal `max. resource limit exceeded`
//! on an `errors >= 1` run was fabricated into `ClauseVerdict::Divergent`, and
//! `exec_tv::discharge` had no rlimit gate at all (every `errors >= 1` run → Divergent
//! unconditionally, the same #189 class).
//!
//! The fix (the oracle these tests pin): one shared discriminator
//! `forge/src/tv_signal.rs::is_rlimit_signal` covering the full phrase set
//! (`rlimit exceeded`, `rlimit) exceeded`, `resource limit exceeded`, the last of
//! which also catches z3's `max. resource limit exceeded`), consumed by all three
//! phases (the per-phase copies deleted). With one shared discriminator the phrase set
//! cannot drift again, and the z3-phrased resourceout is caught in every phase.
//!
//! The z3 phrase is load-bearing rather than defensive padding: the distributed z3
//! binary (verus 0.2026.05.24.ecee80a toolchain, `~/.local/share/verus/verus-x86-linux/z3`)
//! contains the literal diagnostic `max. resource limit exceeded`, Z3's own
//! resourceout message (its `:reason-unknown` text on an rcounts exhaustion). That
//! string contains `resource limit exceeded` but neither `rlimit exceeded` nor
//! `rlimit) exceeded` (verus's own phrasing, `Resource limit (rlimit) exceeded`, is a
//! separate `air` literal). An `errors >= 1` run whose combined output surfaces only
//! the Z3-phrased resourceout (e.g. a raw `:reason-unknown` passthrough) must therefore
//! degrade to Unverifiable, never a fabricated Divergent (R-HONEST-3: a non-infidelity
//! outcome is never a false Divergent; R-CODE-4: a timeout degrades, never a false
//! result).
//!
//! Authority (the expected values come from these, not from the toolchain's own
//! output — R-CHAR-3):
//! - `forge/src/body_tv.rs`'s former `is_rlimit_signal` (the #189 precedent): the
//!   three-phrase set the shared discriminator must carry.
//! - The z3 binary literal `max. resource limit exceeded` (observed via `strings` on
//!   the distributed toolchain), independent of any Thermite source.
//! - `goal.md` R-HONEST-3 / R-CODE-4: a timeout is never a fabricated Divergent.
//!
//! Mechanics: `is_rlimit_signal` is `pub(crate)` to the `forge` binary crate,
//! unreachable from an integration test, and a real Z3 rlimit exhaustion is not
//! deterministically forcible. So this pin works at the source seam (the same
//! technique the original #192 pin used, re-aimed at the new shared helper per the
//! #192 dispatch's authorization): it extracts the `contains("…")` phrase literals
//! from `tv_signal.rs::is_rlimit_signal` and (a) asserts the shared discriminator's
//! phrase set covers body_tv's #189 three-phrase authority (phrase parity: the
//! property is that no phrase drifted away), (b) evaluates the shared phrase set with
//! the discriminator's semantics, `lowercased.contains(phrase)`, against z3's
//! resourceout literal and asserts detection. It also asserts the root-cause property
//! that the per-phase copies are gone (each consumer routes to the shared helper), so
//! drift cannot recur.

/// Extract the lowercase `contains("…")` phrase literals from the
/// `fn is_rlimit_signal` block of a source file. Panics if the fn or its
/// phrases cannot be found (a refactor moved the seam, so the pin must be re-aimed
/// rather than silently passing).
fn rlimit_phrases(src: &str, which: &str) -> Vec<String> {
    let start = src
        .find("fn is_rlimit_signal(")
        .unwrap_or_else(|| panic!("{which}: `fn is_rlimit_signal(` not found — re-aim this pin"));
    let block_end = src[start..]
        .find("\n}")
        .map(|i| start + i)
        .unwrap_or_else(|| panic!("{which}: unterminated is_rlimit_signal block"));
    let block = &src[start..block_end];
    let mut phrases = Vec::new();
    let mut rest = block;
    while let Some(i) = rest.find("contains(\"") {
        let after = &rest[i + "contains(\"".len()..];
        let end = after
            .find("\")")
            .unwrap_or_else(|| panic!("{which}: unterminated contains literal"));
        phrases.push(after[..end].to_ascii_lowercase());
        rest = &after[end..];
    }
    assert!(
        !phrases.is_empty(),
        "{which}: no contains(\"…\") phrases extracted — re-aim this pin"
    );
    phrases
}

/// The shared discriminator's source — the sole `is_rlimit_signal` after the #192 fix.
fn tv_signal_src() -> &'static str {
    include_str!("../src/tv_signal.rs")
}

fn contract_tv_src() -> &'static str {
    include_str!("../src/contract_tv.rs")
}

fn body_tv_src() -> &'static str {
    include_str!("../src/body_tv.rs")
}

fn exec_tv_src() -> &'static str {
    include_str!("../src/exec_tv.rs")
}

/// The #189 authority phrase set, hand-derived from the body_tv precedent the #166/#189
/// fixes named (not read from any current toolchain fn — R-CHAR-3): the three phrases
/// the discriminator must carry so a Verus/Z3 timeout stays out of the Divergent class.
fn body_tv_189_authority_phrases() -> [&'static str; 3] {
    [
        "rlimit exceeded",
        "rlimit) exceeded",
        "resource limit exceeded",
    ]
}

/// Phrase parity (the #192 oracle, no longer drifted): the shared discriminator's
/// phrase set must cover body_tv's #189 three-phrase authority, otherwise a Verus
/// output could classify Timeout/Unverifiable in one TV phase but a fabricated
/// Divergent in another. Against the drifted pre-#192 code this failed (contract_tv
/// dropped `"resource limit exceeded"`); it now passes because there is one shared
/// discriminator carrying the full set.
#[test]
fn divergence_contract_tv_rlimit_phrases_drifted_from_body_tv() {
    let shared = rlimit_phrases(tv_signal_src(), "tv_signal");
    for phrase in body_tv_189_authority_phrases() {
        assert!(
            shared.contains(&phrase.to_string()),
            "the SHARED tv_signal::is_rlimit_signal does not carry {phrase:?}, which \
             body_tv's #189 authority requires — a drifted/incomplete discriminator: the \
             same verus output would classify Timeout/Unverifiable in one TV phase but a \
             fabricated Divergent in another (R-HONEST-3). shared phrases: {shared:?}"
        );
    }

    // The root-cause property (#192): the per-phase copies are gone. Each TV phase
    // routes to the shared discriminator, so the phrase set cannot drift again. A
    // resurrected private copy (a `fn is_rlimit_signal(` in any consumer) re-opens the
    // drift and fails this pin.
    for (src, which) in [
        (contract_tv_src(), "contract_tv"),
        (body_tv_src(), "body_tv"),
        (exec_tv_src(), "exec_tv"),
    ] {
        assert!(
            !src.contains("fn is_rlimit_signal("),
            "{which} carries its OWN `fn is_rlimit_signal(` — the #192 root cause (copy \
             drift). It must consume the shared `crate::tv_signal::is_rlimit_signal` so the \
             phrase set cannot drift across the three TV phases."
        );
        assert!(
            src.contains("tv_signal::is_rlimit_signal"),
            "{which} does not consume the shared `crate::tv_signal::is_rlimit_signal` — the \
             #192 fix requires all three TV phases to route to the ONE discriminator."
        );
    }
}

/// The behavioral consequence: Z3's own resourceout diagnostic (`max. resource limit
/// exceeded`, the literal present in the distributed z3 binary, independent of any
/// Thermite source) on an `errors >= 1` run must be detected as a timeout signal so the
/// discharge paths route it to Unverifiable, never the Divergent arm. Evaluates the
/// shared discriminator's extracted phrase set with the discriminator's semantics
/// (`output.to_ascii_lowercase().contains(phrase)`). Hand-derived (R-CHAR-3):
/// `"max. resource limit exceeded"` lowercased contains `"resource limit exceeded"` (the
/// shared third clause → detected) but neither `"rlimit exceeded"` nor `"rlimit) exceeded"`
/// (no `rlimit` token in the Z3 phrasing). Against the drifted pre-#192 contract_tv copy
/// (two clauses) this failed; it now passes because the shared discriminator carries the
/// third clause for every phase.
#[test]
fn divergence_z3_resourceout_phrase_escapes_contract_tv_rlimit_gate() {
    // Z3's resourceout message as it would surface in a combined verus output that
    // still carries an errors-counting results line (the #189 shape).
    let z3_resourceout_run =
        "unknown: max. resource limit exceeded\nverification results:: 0 verified, 1 errors";
    let lowered = z3_resourceout_run.to_ascii_lowercase();

    // The reference: body_tv's #189 phrase set detects it (via the third clause).
    assert!(
        body_tv_189_authority_phrases()
            .iter()
            .any(|p| lowered.contains(p)),
        "authority self-check: body_tv's #189 phrase set must detect Z3's resourceout \
         literal (it does, via 'resource limit exceeded'); if this fires, the authority \
         moved — re-aim the pin"
    );

    // The fix: the shared discriminator's phrase set detects it too (the same property,
    // now for all three phases via the one helper).
    let shared = rlimit_phrases(tv_signal_src(), "tv_signal");
    assert!(
        shared.iter().any(|p| lowered.contains(p)),
        "the SHARED tv_signal::is_rlimit_signal does not detect Z3's own resourceout \
         diagnostic ('max. resource limit exceeded', a literal in the distributed z3 \
         binary): an errors >= 1 run carrying only the Z3-phrased resourceout would fall \
         past the rlimit arm into the Divergent arm — a solver-budget exhaustion fabricated \
         into an infidelity, the exact #189-class false finding (#166/#192). shared \
         phrases: {shared:?}"
    );
}
