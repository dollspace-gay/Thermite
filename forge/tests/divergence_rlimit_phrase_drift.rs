//! Divergence pin (critic re-audit of #166, commit 61f6f763): the #166 fix added
//! `contract_tv::is_rlimit_signal` claiming (its own doc comment + the commit
//! message) to "mirror `body_tv::is_rlimit_signal`" — the #189 precedent that keeps
//! a Verus/Z3 resource-limit (rlimit) exhaustion OUT of the `Divergent` class. The
//! copy is DRIFTED: body_tv's discriminator matches THREE phrases
//! (`"rlimit exceeded"`, `"rlimit) exceeded"`, `"resource limit exceeded"` —
//! case-insensitive); contract_tv's copy carries only the first TWO, dropping
//! `"resource limit exceeded"`.
//!
//! WHY THE DROPPED PHRASE IS LOAD-BEARING, not defensive padding: the distributed
//! z3 binary (verus 0.2026.05.24.ecee80a toolchain, `~/.local/share/verus/
//! verus-x86-linux/z3`) contains the literal diagnostic `max. resource limit
//! exceeded` — Z3's OWN resourceout message (its `:reason-unknown` text on an
//! rcounts exhaustion). That string contains `resource limit exceeded` but
//! NEITHER `rlimit exceeded` NOR `rlimit) exceeded` (verus's own phrasing,
//! `Resource limit (rlimit) exceeded`, is a separate literal in `air`). An
//! `errors >= 1` run whose combined output surfaces only the Z3-phrased
//! resourceout (e.g. a raw `:reason-unknown` passthrough on an unexpected-SMT-
//! output path) is therefore:
//!   - body_tv: `is_rlimit_signal` → true → `DischargeOutcome::Timeout` →
//!     Unverifiable (honest);
//!   - contract_tv: `is_rlimit_signal` → false → the `errors >= 1` arm →
//!     `ClauseVerdict::Divergent` — a solver-budget exhaustion FABRICATED into a
//!     contract infidelity: the exact #189-class false finding commit 61f6f763's
//!     own message says it closes (R-HONEST-3: a non-infidelity outcome is never a
//!     false Divergent; R-CODE-4: a timeout degrades, never a false result).
//!
//! AUTHORITY:
//! - `forge/src/body_tv.rs::is_rlimit_signal` — the #189 precedent the #166 commit
//!   message and `contract_tv::is_rlimit_signal`'s doc comment BOTH name as the
//!   pattern being mirrored (the dispatch's sibling-consistency criterion: same
//!   phrases, same case-insensitivity).
//! - `goal.md` R-HONEST-3 / R-CODE-4 (via `.design/verified/contract-tv.md`, the
//!   #166 commit's own cited design sources): a timeout is never a fabricated
//!   Divergent.
//! - The z3 binary literal `max. resource limit exceeded` (observed via `strings`
//!   on the distributed toolchain), independent of any Thermite source.
//!
//! MECHANICS: `discharge`/`is_rlimit_signal` are PRIVATE fns of the `forge` binary
//! crate, unreachable from an integration test, and a real Z3 rlimit exhaustion is
//! not deterministically forcible (the same limitation the #166 teeth acknowledge).
//! So this pin works at the SOURCE seam: it extracts the `contains("…")` phrase
//! literals from each module's `fn is_rlimit_signal` block and (a) asserts
//! contract_tv's phrase set covers body_tv's (the sibling-consistency contract),
//! (b) evaluates contract_tv's extracted phrase set — the discriminator's exact
//! semantics, `lowercased.contains(phrase)` — against Z3's resourceout literal and
//! asserts detection. The expected values come from body_tv (#189) and the z3
//! binary, NEVER from contract_tv's own output (R-CHAR-3).
//!
//! Both tests FAIL against commit 61f6f763. `#[ignore]`d (tracked, not a release
//! gate: verus's own `Resource limit (rlimit) exceeded` phrasing IS caught, so the
//! common rlimit path is honest; the gap is the Z3-phrased variant). Un-ignore
//! when the fixer restores the third clause. Tracking: crosslink #192.

/// Extract the lowercase `contains("…")` phrase literals from the
/// `fn is_rlimit_signal` block of a source file. Panics loudly if the fn or its
/// phrases cannot be found (a refactor moved the seam — the pin must be re-aimed,
/// never silently passed).
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

fn body_tv_src() -> &'static str {
    include_str!("../src/body_tv.rs")
}

fn contract_tv_src() -> &'static str {
    include_str!("../src/contract_tv.rs")
}

/// SIBLING CONSISTENCY (the #166 commit's own "mirrors body_tv::is_rlimit_signal"
/// claim): every rlimit phrase body_tv's #189 discriminator detects must also be
/// detected by contract_tv's copy — otherwise the two TV layers classify the SAME
/// verus output differently (body_tv: Timeout/Unverifiable; contract_tv: a
/// fabricated Divergent). FAILS against 61f6f763: contract_tv drops
/// `"resource limit exceeded"`.
#[test]
#[ignore = "divergence: contract_tv::is_rlimit_signal drops body_tv's 'resource limit exceeded' clause; tracking #192"]
fn divergence_contract_tv_rlimit_phrases_drifted_from_body_tv() {
    let body = rlimit_phrases(body_tv_src(), "body_tv");
    let contract = rlimit_phrases(contract_tv_src(), "contract_tv");
    for phrase in &body {
        assert!(
            contract.contains(phrase),
            "contract_tv::is_rlimit_signal does not detect {phrase:?}, which \
             body_tv::is_rlimit_signal (the #189 authority the #166 commit claims to \
             mirror) does — a drifted copy: the same verus output classifies \
             Timeout/Unverifiable in body-TV but a fabricated Divergent in contract-TV \
             (R-HONEST-3). contract_tv phrases: {contract:?}"
        );
    }
}

/// THE BEHAVIORAL CONSEQUENCE: Z3's own resourceout diagnostic (`max. resource
/// limit exceeded` — the literal present in the distributed z3 binary, independent
/// of any Thermite source) on an `errors >= 1` run MUST be detected as a timeout
/// signal so `discharge` routes it to Unverifiable, never the Divergent arm.
/// Evaluates contract_tv's extracted phrase set with the discriminator's exact
/// semantics (`output.to_ascii_lowercase().contains(phrase)`). HAND-DERIVED
/// (R-CHAR-3): `"max. resource limit exceeded"` lowercased contains
/// `"resource limit exceeded"` (body_tv's third clause → detected, honest) but
/// neither `"rlimit exceeded"` nor `"rlimit) exceeded"` (no `rlimit` token in the
/// Z3 phrasing) — so contract_tv's two-clause copy returns false and the run falls
/// to the `errors >= 1` Divergent arm. FAILS against 61f6f763.
#[test]
#[ignore = "divergence: Z3-phrased resourceout ('max. resource limit exceeded') not detected by contract_tv — fabricated Divergent; tracking #192"]
fn divergence_z3_resourceout_phrase_escapes_contract_tv_rlimit_gate() {
    // Z3's resourceout message as it would surface in a combined verus output that
    // still carries an errors-counting results line (the #189-class shape).
    let z3_resourceout_run =
        "unknown: max. resource limit exceeded\nverification results:: 0 verified, 1 errors";
    let lowered = z3_resourceout_run.to_ascii_lowercase();

    // The honest reference: body_tv's #189 discriminator detects it.
    let body = rlimit_phrases(body_tv_src(), "body_tv");
    assert!(
        body.iter().any(|p| lowered.contains(p)),
        "authority self-check: body_tv's #189 phrase set must detect Z3's resourceout \
         literal (it does, via 'resource limit exceeded'); if this fires, the \
         authority moved — re-aim the pin"
    );

    // The divergence: contract_tv's drifted copy does NOT.
    let contract = rlimit_phrases(contract_tv_src(), "contract_tv");
    assert!(
        contract.iter().any(|p| lowered.contains(p)),
        "contract_tv::is_rlimit_signal does not detect Z3's own resourceout \
         diagnostic ('max. resource limit exceeded', a literal in the distributed z3 \
         binary): an errors >= 1 run carrying only the Z3-phrased resourceout falls \
         past the rlimit arm into `errors >= 1` → ClauseVerdict::Divergent — a \
         solver-budget exhaustion fabricated into a contract infidelity, the exact \
         #189-class false finding commit 61f6f763 claims to close (R-HONEST-3 / \
         R-CODE-4). contract_tv phrases: {contract:?}"
    );
}
