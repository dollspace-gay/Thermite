//! `forge/src/engine.rs` — the backend-NEUTRAL proof-Engine interface + the Verus
//! engine refactored behind it (`.design/verified/proof-backends.md` REQ-2/REQ-3/
//! REQ-3.1/REQ-8; increment (i), blocker #204).
//!
//! Today exactly one engine is welded into `check::check_file_with_options` (the
//! implicit `run_verus` + `classify_verus_outcome` path). This module introduces
//! the [`Engine`] trait — the four slots REQ-2 names (FRAGMENT / DISCHARGE / TRUST
//! PROFILE / EVIDENCE) — and refactors the Verus discharge BYTE-IDENTICALLY behind
//! it, with the ONE named exception of the REQ-3.1 fast-unknown remap.
//!
//! **The REQ-3.1 fast-unknown seam (the single behavioral delta).** The SHIPPED
//! `classify_verus_outcome` absorbs the SMT incompleteness-`unknown` into its
//! `Counterexample` bucket (an `unknown` returned FAST, no `--profile` report → the
//! failure path). A naive byte-identical Verus engine would map that
//! `Counterexample` to [`Verdict::Refuted`] → `ladder_action_l3` HARD FAIL — which
//! CONTRADICTS REQ-3 ("an SMT `unknown` is `Unknown`, never `Refuted`; refutation
//! requires a witnessing input"). So [`VerusEngine::verdict_of`] SPLITS the
//! `Counterexample` by [`counterexample_is_incompleteness_unknown`], the NARROW
//! SMT-`unknown` signature: ONLY a span-less failure carrying the SMT-`unknown`
//! signal (no parsed `--> file:line:col` span AND no frontend `error[E…]` type
//! error) → [`Verdict::Unknown`] (DEGRADE, matching §6's degrade-on-incompleteness
//! intent); a WITNESSED countermodel (a parsed span) AND a FRONTEND rejection (a
//! type error `error[E…]`) both stay [`Verdict::Refuted`] (HARD FAIL, never
//! degrades).
//!
//! **Why the narrow signature (cert-oracle byte-identity).** The remap is the SOLE
//! exception to increment (i)'s byte-identical claim, and it MUST be inert on the
//! conformance corpus. The corpus DOES contain `Counterexample`-bucket failures —
//! notably the provenance `careless_query` IFC path, which verus rejects with a
//! span-less type error `error[E0308]` the corpus pins at L0. A coarse "no parsed
//! span → Unknown" rule would WRONGLY degrade that E0308 to L2 (and crash on the
//! ADT L2 lowering), perturbing the oracle. The narrow signature keeps E0308 (and
//! every witnessed countermodel) at `Refuted` → L0, so it fires ONLY on a genuine
//! SMT-`unknown` — a case the corpus does not contain — leaving every
//! `conformance/*.cert.json` byte-identical (REQ-3.1's "the remap only changes
//! behavior on inputs the corpus doesn't contain").
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-2 (the Engine interface — 4 slots) | SHIPPED (Verus instance) | `pub trait Engine { name, fragment, discharge, trust_profile, evidence_key }` + `pub enum Verdict`/`Evidence`/`Counterexample`/`Reason` + `pub struct TrustProfile`/`Fragment`/`CacheKey`; the `pub struct VerusEngine` fills all four slots from SHIPPED code (FRAGMENT = the frozen lowering subset; DISCHARGE = `classify_verus_outcome`'s three-way map lifted to `Verdict` with the REQ-3.1 remap; TRUST PROFILE = {Z3, Verus VC-gen} + the TV/lowering theorem; EVIDENCE = the content-addressed `cache::cache_key` generalized with an engine discriminator). Non-test consumer: `check::ladder_for_timeout` routes the per-item L3 certification discharge through `VerusEngine`. The Lean engine (`LeanAuto`/`LeanInteractive`) is increment (ii), NOT-STARTED. |
//! | REQ-3 (discharge discipline — Unknown degrades, Refuted hard-fails) | SHIPPED | `pub fn verdict_ladder_action` maps a `Verdict` to the SHIPPED `degrade::L3Verdict` for a `role = Certification` obligation: `Proven` → the proved L3 cert (CertifyL3); `Unknown` → `Timeout`-shaped degrade trigger (`run_ladder` → L2/L1); `Refuted` → `Counterexample` (HARD FAIL, never degrades). Consumer: `check::ladder_for_timeout`. Generalized off `degrade::ladder_action_l3`. |
//! | REQ-3.1 (the fast-unknown remap) | SHIPPED | `VerusEngine::verdict_of` splits `VerusOutcome::Counterexample` by `counterexample_is_incompleteness_unknown` (the NARROW SMT-`unknown` signature): ONLY a span-less failure carrying the SMT-`unknown` signal (no frontend `error[E` / no parsed `--> span`) → `Unknown(Reason::IncompleteUnknown)` (degrade); a witnessed countermodel AND a frontend type error (E0308) stay `Refuted` (hard-fail). The remap is INERT on the corpus (witnessed + E0308 cases stay hard-fail). Tested: a synthetic SMT-`unknown` degrades, a witnessed countermodel + an E0308 type error both hard-fail (`engine.rs` tests + `forge/tests/engine_interface.rs`). |
//! | REQ-8 (engine ordering hook) | SHIPPED (Verus rung) | `pub fn default_engines` returns the ordered engine list (Verus first); increment (i) wires the hook with the single Verus rung, increment (ii) adds the Lean rungs. Consumer: `check::ladder_for_timeout` reads the first engine (Verus). |
//! | REQ-4 (certificate attribution — per-obligation {engine, trust_profile}) | SHIPPED (increment (iii), #247) | `pub struct EngineAttribution { engine, trust_profile }` (the `{engine, trust_profile}` pair) is built by `pub fn attribution_for`; `manifest::Certificate::engine_attribution` is the ADDITIVE serde field (`#[serde(default, skip_serializing_if = "Option::is_none")]`), populated by `Certificate::with_engine_attribution` ONLY when a NON-default engine (Lean) discharges (the default Verus path leaves it `None` so corpus certs are byte-identical — the `serde(default)` keeps the goldens green). Honest-min aggregation UNCHANGED (`AssuranceManifest::aggregate`). Oracle-EXCLUDED (OQ-2 decided diagnostic-only for byte-identity). Consumer: `cli::run_check` (the `--engine lean` path attaches it). |
//! | REQ-5 (engine disagreement = soundness alarm) | SHIPPED (increment (iii), #247) | `pub fn check_disagreement` is the multi-engine dispatch guard: on the SAME obligation, one engine `Proven` + another `Refuted` (a WITNESSED countermodel) returns `Err(Disagreement { proven_engine, refuted_engine, item, counterexample })` — a structured HARD halt naming both engines + the obligation, NEVER resolved by preference. `Proven ⊕ Unknown` is benign (`Ok`). Surfaced as `ForgeError::SoundnessAlarm` (`cli.rs`). Tested: `StubProven ⊕ StubRefuted` fires; `Proven ⊕ Unknown` does not (`engine.rs` tests + `forge/tests/engine_attribution.rs`). |
//! | REQ-7 (interactive proofs — skeleton emit + replay with staleness gate + sorry detection) | SHIPPED (increment (iii), #247; the #252 helper-surface elimination) | `pub fn interactive_proof_path` is the deterministic `<file>.lean-proofs/<item>.lean` artifact path; `pub fn replay_interactive` REPLAYS a PRESENT proof (`lake env lean`) with the obligation-hash staleness gate (the emitted `-- evidence_key: <hex>` header must match the current `evidence_key`; a mismatch → `Unknown("stale proof — re-derive")`), EMITS the skeleton (the exporter's tier-(c) source + the evidence-key header) when ABSENT, and DETECTS `sorry` explicitly (`proof_has_sorry` greps the source AND `#print axioms` for `sorryAx`/`sorry`) → `Unknown` (NEVER `Proven`); a kernel-accepted sorry-free replay → `Proven` with the interactive trust profile (`trust_profile_interactive`). **#252 ARCHITECTURAL FIX (ending the command-injection whack-a-mole — 5 bypass generations #248..#252):** `reconstruct_replay` no longer splices any author HELPERS section — the reconstructed file is EXACTLY the canonical generator preamble + `R_item` + the canonical `theorem thermite_obligation_<item> : <statement> := <author PROOF TERM>` + the anchored `#print axioms`. The ONLY author-controlled text is the PROOF TERM (after the obligation theorem's first `:=`); author content OUTSIDE it is DROPPED (it has nowhere to live, so it can never share the obligation's elaboration scope — the indented-`notation` #252 poison and the column-0 #251 poison both vanish). Auxiliary lemmas inline as `have`/`let`/`suffices` inside the proof term (no expressivity loss for a single-obligation proof). The `disallowed_helper_command`/`author_helpers` allowlist (a blocklist on a Turing-complete elaborator — UNSOUNDABLE) is DELETED. BELT (`proof_term_command_token`): the extracted proof term is REJECTED (→ Unknown) if it carries a top-level command keyword (`notation`/`macro`/`macro_rules`/`syntax`/`set_option`/`attribute`/`instance`/`open`/`export`/`import`/`namespace`/`initialize`/`#…`) in ANY position (exact-token, whitespace-independent — catches an `… in`-style command form). The #250 duplicate-declaration check + the #249/#250 axiom anchor + `statements_match` + the type-check STAY. Tested: skeleton emitted; a clean inline-have proof replays Proven; a stale hash → Unknown; a sorry-bearing inline proof → Unknown (`interactive_inline_have_clean_proven_sorry_unknown`); the helper section is dropped + the belt rejects an `open … in` term (`reconstruct_drops_author_helper_section`); the belt scan (`proof_term_command_token_scans_position_independently`) (`forge/tests/lean_engine.rs` + `engine.rs` tests). |
//! | REQ-9 (engine-generic mutation battery — the Lean path) | SHIPPED (increment (iii), #247) | `pub fn lean_mutant_outcome` classifies a Lean-engine mutant discharge under the engine-generic kill semantics (`Refuted ∪ Unknown-after-attempt` = killed; a mutant OUTSIDE the Lean fragment = `UntestedAgainstLean`, never counted killed); `pub struct LeanMutationTally` accumulates `killed / attempted-minus-equivalent` with the untested-against-lean count reported, NEVER inflating the ratio. Consumer: `check::lean_mutation_score` (the `--engine lean` mutation path). The Verus-path battery (`check::mutation_score`) is UNTOUCHED. |

use crate::lean_export::{export_item, find_item, ExportRefusal, ExportedObligation};
use crate::obligation::{Obligation, ObligationRole};
use std::path::PathBuf;
use thermite_syntax::Program;

/// The named engine (`.design/verified/proof-backends.md` REQ-2 `name`). Verus is
/// the only instance increment (i) ships; `LeanAuto`/`LeanInteractive` are named
/// for increment (ii) so the EVIDENCE cache key already carries the discriminator
/// (so a Verus proof and a future Lean proof of the same item never collide, §2(d)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineName {
    /// The Verus/Z3 push-button engine (increment (i)).
    Verus,
    /// The Lean auto tactic-battery engine (increment (ii), NOT-STARTED). Named in
    /// the EVIDENCE-key discriminator from day one (§2(d)) so a future Lean proof
    /// and the Verus proof of the same lowered source never collide; constructed
    /// when increment (ii) lands the Lean engine.
    #[allow(
        dead_code,
        reason = "proof-backends §2(d): the Lean engine names are forward-declared in the \
                  cache-key discriminator; constructed by increment (ii) (NOT-STARTED, #204 chain)"
    )]
    LeanAuto,
    /// The Lean interactive engine (increment (ii)/(iii), NOT-STARTED). Forward-
    /// declared with [`EngineName::LeanAuto`] (same rationale).
    #[allow(
        dead_code,
        reason = "proof-backends §2(d): the Lean engine names are forward-declared in the \
                  cache-key discriminator; constructed by increment (ii)/(iii) (NOT-STARTED)"
    )]
    LeanInteractive,
}

impl EngineName {
    /// The stable tag for the EVIDENCE cache-key discriminator (§2(d)) and
    /// diagnostics (deterministic — R-CODE-5).
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            EngineName::Verus => "verus",
            EngineName::LeanAuto => "lean-auto",
            EngineName::LeanInteractive => "lean-interactive",
        }
    }
}

/// The reason an engine returned [`Verdict::Unknown`] (`.design/verified/
/// proof-backends.md` REQ-2(b)/REQ-3). An `Unknown` is "this engine could not
/// decide", NOT a failure verdict — it DEGRADES per the ladder (§6). The two
/// Verus-engine reasons mirror the SHIPPED `VerusOutcome`'s non-`Proved` arms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// The solver exhausted its SMT resource budget (rlimit) — the SHIPPED
    /// `VerusOutcome::Timeout`. Carries the human detail (the `--profile`-derived
    /// summary the cert's `solver_profile` already records).
    VerusTimeout(String),
    /// The SMT incompleteness-`unknown` edge (the REQ-3.1 remap): a `Counterexample`
    /// outcome carrying NO witnessing input (no parsed `--> span`). Semantically a
    /// timeout-class INCOMPLETENESS event (the solver could not decide), so it
    /// DEGRADES, never hard-fails. Carries the raw stderr head for diagnosis.
    IncompleteUnknown(String),
}

/// A genuine WITNESSED countermodel (`.design/verified/proof-backends.md`
/// REQ-2(b)/REQ-3): a `Refuted` verdict's deliverable (§5.1 "counterexamples, not
/// adjectives"). Carries the per-obligation failure results the SHIPPED
/// `parse_stderr_failures` produced (each with its `--> file:line:col` span — the
/// witnessing input). A `Refuted` requires AT LEAST ONE witnessing input; a
/// witness-less failure is an [`Verdict::Unknown`], never this (REQ-3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counterexample {
    /// The per-obligation failure results (the §5.1 witnesses).
    pub obligations: Vec<crate::manifest::ObligationResult>,
}

/// The replayable, cacheable evidence an engine attaches to a [`Verdict::Proven`]
/// (`.design/verified/proof-backends.md` REQ-2(d)). For the Verus engine the
/// evidence is the count of discharged obligations + the engine's cache key — the
/// content-addressed proof-cache entry the SHIPPED `cache::store`/`load` serves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    /// The number of obligations verus discharged (the `verified` count).
    pub verified: u64,
    /// The content-addressed evidence key (the SHIPPED `cache_key` generalized
    /// with the engine discriminator — §2(d)).
    pub key: CacheKey,
}

/// The verdict an engine returns for a discharge (`.design/verified/
/// proof-backends.md` REQ-2(b)). The strict mapping discipline (REQ-3): a
/// tactic/solver FAILURE WITHOUT a witnessing input is [`Verdict::Unknown`], NEVER
/// [`Verdict::Refuted`] — refutation requires a genuine countermodel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The engine PROVED the obligation (for all inputs at a sound-for-all-inputs
    /// engine) → certify at the engine's level (L3 for Verus); attach evidence.
    Proven(Evidence),
    /// The engine DISPROVED the obligation with a genuine WITNESSED countermodel
    /// → HARD FAIL, NEVER degrades (REQ-3 anti-cheat).
    Refuted(Counterexample),
    /// The engine could NOT decide (a timeout, a tactic-battery exhaustion, or an
    /// SMT incompleteness-`unknown`) → DEGRADE per the ladder (REQ-3). NOT a
    /// failure verdict.
    Unknown(Reason),
}

/// The content-addressed EVIDENCE key (`.design/verified/proof-backends.md`
/// REQ-2(d) / §2(d)): the SHIPPED `cache::cache_key` generalized with the ENGINE
/// discriminator so a Verus proof and a future Lean proof of the same item never
/// collide. Increment (i) composes the SHIPPED five-input verus key (lowered
/// source + seed + verus version + thermite version + `CHECK_SCHEMA_VERSION`) with
/// the engine tag; the Lean analogs (toolchain rev + targeted-spine hash) are
/// increment (ii) (the field is the seam — the future Lean engine widens it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey {
    /// The engine discriminator (§2(d): "the ENGINE name is the new discriminator").
    pub engine: EngineName,
    /// The SHIPPED content-addressed key (`cache::cache_key`'s sha256 hex) — the
    /// per-engine version axes are folded in by the engine (the Verus engine uses
    /// the verus-version slot; the Lean engine widens it in increment (ii)).
    pub content_address: String,
}

/// The construct/class FRAGMENT an engine can ATTEMPT (`.design/verified/
/// proof-backends.md` REQ-2(a)). For Verus this is the WHOLE frozen subset
/// reachable via the lowering (`thermite_lower::lower` + `run_verus`), including
/// the [`crate::obligation::ObligationClass::RegistryTermination`] class (its
/// dec-check is the common discharge path, REQ-1.2(a)). The predicate is on the
/// obligation class; a future engine narrows it (the Lean-auto engine admits only
/// the specCall-free QF fragment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    /// `true` iff this engine ADMITS every obligation class (the Verus whole-subset
    /// case). A narrower engine (Lean-auto) sets `false` + narrows `admits`.
    pub admits_all_classes: bool,
}

impl Fragment {
    /// Does this fragment ADMIT the given obligation? (`.design/verified/
    /// proof-backends.md` REQ-2(a) — "which obligation classes this engine can
    /// ATTEMPT".) The whole-subset Verus fragment admits everything; the predicate
    /// is the seam a narrower future engine keys on.
    #[must_use]
    pub fn admits(&self, _o: &Obligation) -> bool {
        self.admits_all_classes
    }
}

/// The named trust base an engine ADDS when it says `Proven` (`.design/verified/
/// proof-backends.md` REQ-2(c)). An ENUMERATED set of named items so an auditor
/// sees L3-via-Lean enumerates a smaller base than L3-via-Verus (the §1 "enumerable
/// trusted base"). For Verus: {Z3, Verus VC-gen} + the TV/lowering theorem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustProfile {
    /// The enumerated named trust items (e.g. `["Z3", "Verus VC-gen", "TV/lowering
    /// theorem (lowering_faithful)"]`).
    pub items: Vec<String>,
}

/// A proof ENGINE behind the backend-neutral interface (`.design/verified/
/// proof-backends.md` REQ-2). The four slots: FRAGMENT (what it can ATTEMPT),
/// DISCHARGE (the verdict, under REQ-3's mapping discipline), TRUST PROFILE (the
/// named base added on `Proven`), EVIDENCE (the replayable cache key). Increment
/// (i) ships the Verus instance ([`VerusEngine`]); the Lean engine is increment
/// (ii).
pub trait Engine {
    /// The engine's name (the EVIDENCE-key discriminator + diagnostics).
    fn name(&self) -> EngineName;

    /// (a) FRAGMENT — which obligation classes / constructs this engine ATTEMPTS.
    fn fragment(&self) -> Fragment;

    /// (b) DISCHARGE — the verdict for an obligation, under REQ-3's discipline (a
    /// solver/tactic failure WITHOUT a witnessing input is `Unknown`, never
    /// `Refuted`).
    fn discharge(&self, o: &Obligation) -> Verdict;

    /// (c) TRUST PROFILE — the named base ADDED when this engine says `Proven`.
    fn trust_profile(&self) -> TrustProfile;

    /// (d) EVIDENCE — the replayable, cacheable key (generalizes
    /// `cache::cache_key` with the engine discriminator, §2(d)).
    fn evidence_key(&self, o: &Obligation) -> CacheKey;
}

/// The Verus/Z3 engine, refactored byte-identically behind [`Engine`] EXCEPT the
/// one named REQ-3.1 fast-unknown remap (`.design/verified/proof-backends.md`
/// REQ-2, AC-2). It does NOT spawn verus itself — `check.rs` owns the
/// `run_verus`/cache machinery (the §0.1 vacuity/mutation/strengthen meta-queries
/// stay direct verus calls OUTSIDE this engine, the deliberate v1 boundary). This
/// engine's job is the VERDICT MAPPING + the four-slot profile: `check.rs` runs the
/// SHIPPED `run_verus`, hands the engine the resulting `VerusOutcome`, and the
/// engine maps it to a [`Verdict`] (with the REQ-3.1 split). So the engine carries
/// the verdict policy; `check.rs` carries the I/O. This keeps the refactor
/// byte-identical (the same `run_verus` bytes, the same cache) while routing the
/// VERDICT through the trait.
#[derive(Debug, Clone, Copy, Default)]
pub struct VerusEngine;

impl VerusEngine {
    /// Map a SHIPPED [`crate::check::VerusOutcome`] to a backend-neutral [`Verdict`]
    /// (REQ-2(b) / REQ-3.1). This is the LOAD-BEARING verdict policy: the SHIPPED
    /// three-way `classify_verus_outcome` map lifted to `Verdict`, WITH the one
    /// named REQ-3.1 remap on the `Counterexample` arm.
    ///
    /// - `Proved` → [`Verdict::Proven`] (with the discharged-count evidence + key).
    /// - `Timeout` → [`Verdict::Unknown`] (`VerusTimeout`) → DEGRADE (unchanged).
    /// - `Counterexample` SPLIT by [`counterexample_is_incompleteness_unknown`]:
    ///   - the genuine SMT-`unknown` signature (span-less, no frontend `error[E…]`,
    ///     an explicit `unknown` signal) → [`Verdict::Unknown`] (`IncompleteUnknown`)
    ///     → DEGRADE (the REQ-3.1 delta: today this HARD-FAILS; behind the interface
    ///     it degrades, matching §6's degrade-on-incompleteness intent);
    ///   - EVERYTHING ELSE (a WITNESSED countermodel with a parsed `--> span`, OR a
    ///     FRONTEND type error `error[E…]` like the provenance E0308) → [`Verdict::
    ///     Refuted`] (HARD FAIL — BYTE-IDENTICAL to today: a real bug / rejection
    ///     never degrades).
    #[must_use]
    pub fn verdict_of(&self, outcome: &crate::check::VerusOutcome, key: CacheKey) -> Verdict {
        use crate::check::VerusOutcome;
        match outcome {
            VerusOutcome::Proved { verified } => Verdict::Proven(Evidence {
                verified: *verified,
                key,
            }),
            VerusOutcome::Timeout { detail, .. } => {
                Verdict::Unknown(Reason::VerusTimeout(detail.clone()))
            }
            VerusOutcome::Counterexample { obligations } => {
                if counterexample_is_incompleteness_unknown(obligations) {
                    // The REQ-3.1 remap: ONLY the genuine SMT-incompleteness
                    // `unknown` edge (a witness-LESS failure carrying the explicit
                    // SMT-`unknown` signature, NO frontend/type error) degrades.
                    // Refutation requires a witnessing input; an incompleteness
                    // `unknown` is "the solver could not decide", not a disproof.
                    let detail = obligations
                        .first()
                        .and_then(|o| o.diagnostic.clone())
                        .unwrap_or_else(|| {
                            "verus returned an SMT-incompleteness `unknown`".to_string()
                        });
                    Verdict::Unknown(Reason::IncompleteUnknown(detail))
                } else {
                    // EVERYTHING ELSE stays `Refuted` → HARD FAIL — BYTE-IDENTICAL
                    // to the SHIPPED pipeline (`Counterexample → ladder_action_l3
                    // HardFail`). This covers a genuine `postcondition not satisfied`
                    // countermodel AND a frontend rejection (a TYPE error `error[E…]`,
                    // e.g. the IFC un-typeable `careless_query` E0308 the provenance
                    // corpus pins at L0). The remap is INERT on the corpus: only the
                    // narrow genuine-`unknown` signature (which the corpus does not
                    // contain) is rerouted, so every `conformance/*.cert.json` is
                    // unperturbed (the increment (i) cert-oracle AC).
                    Verdict::Refuted(Counterexample {
                        obligations: obligations.clone(),
                    })
                }
            }
        }
    }
}

impl Engine for VerusEngine {
    fn name(&self) -> EngineName {
        EngineName::Verus
    }

    fn fragment(&self) -> Fragment {
        // The Verus engine ADMITS the whole frozen subset reachable via the
        // lowering — every obligation class, INCLUDING RegistryTermination (its
        // dec-check is the common discharge path, REQ-1.2(a)).
        Fragment {
            admits_all_classes: true,
        }
    }

    fn discharge(&self, o: &Obligation) -> Verdict {
        // The trait `discharge` is the obligation-level entry. `check.rs` owns the
        // real `run_verus` I/O for the per-item L3 path (it already has the lowered
        // source + cache wired), so the LIVE discharge goes through
        // `VerusEngine::verdict_of` from there. Here, an obligation the Verus
        // fragment does NOT admit is an honest `Unknown` (it could not be attempted
        // — REQ-3: never a `Refuted` without a witness, never a false `Proven`).
        // Because `fragment().admits_all_classes` is `true` this never fires for
        // Verus today; it is the REQ-3-compliant default for a future narrowed
        // fragment, and it is NEVER a proof cheat (no `Proven`, no `Refuted`).
        let _ = o;
        Verdict::Unknown(Reason::IncompleteUnknown(
            "the Verus engine discharges per-item obligations through \
             check::ladder_for_timeout (the run_verus path); a bare trait discharge \
             with no run is undecided (REQ-3: never a witness-less Refuted)"
                .to_string(),
        ))
    }

    fn trust_profile(&self) -> TrustProfile {
        // REQ-2(c) / TRUST PROFILE: {Z3, Verus VC-gen} + the TV/lowering theorem
        // (`lowering_faithful`, RELATIVE to {Z3 soundness, S = intended meaning,
        // Lean kernel} per `Faithfulness.lean`). The enumerable trusted base (§1).
        TrustProfile {
            items: vec![
                "Z3".to_string(),
                "Verus VC-gen".to_string(),
                "TV/lowering theorem (lowering_faithful)".to_string(),
            ],
        }
    }

    fn evidence_key(&self, o: &Obligation) -> CacheKey {
        // REQ-2(d): the engine-discriminated key. The CONTENT side is derived from
        // the obligation's item + class + role tags (a stable, prover-neutral
        // address); the LIVE per-item path supplies the richer lowered-source key
        // (the SHIPPED `cache::cache_key`) via `engine_cache_key`, so a HIT is a
        // fresh verify (§2(d)). Here we give the obligation-level identity key.
        CacheKey {
            engine: EngineName::Verus,
            content_address: format!(
                "{item}::{class}::{role}",
                item = o.item,
                class = o.class.tag(),
                role = o.role.tag(),
            ),
        }
    }
}

/// The DEFAULT engine ordering (`.design/verified/proof-backends.md` REQ-8): Verus
/// first (fast, push-button). Increment (i) wires the ordering hook with the single
/// Verus rung; increment (ii) adds the Lean-auto / Lean-interactive rungs AFTER
/// Verus, THEN the existing L2/L1 degrade. Returns the ordered engine names so the
/// caller (`check::ladder_for_timeout`) reads the first (Verus) rung.
#[must_use]
pub fn default_engines() -> Vec<EngineName> {
    // Increment (i): Verus only. The Lean rungs (REQ-8 "Lean-auto second,
    // Lean-interactive on demand") are increment (ii) — appended here when the Lean
    // engine lands, BEFORE the existing L2/L1 degrade.
    vec![EngineName::Verus]
}

/// Build the engine-discriminated EVIDENCE key for the LIVE per-item L3 path
/// (`.design/verified/proof-backends.md` REQ-2(d) / §2(d)). Composes the SHIPPED
/// content-addressed `cache::cache_key` hex (over lowered source, seed, verus
/// version, thermite version, and `CHECK_SCHEMA_VERSION`) with the engine
/// discriminator, so a Verus proof and a future Lean proof of the same lowered
/// source never collide.
/// This is the key the engine attaches to its `Proven` evidence; the SHIPPED
/// `cache::load`/`store` still serve/persist the cert under the SHIPPED hex key
/// (the engine tag is the additive §2(d) discriminator the future Lean engine
/// keys on — it does not change the SHIPPED verus cache address, so the corpus
/// cache hits are unperturbed).
#[must_use]
pub fn engine_cache_key(engine: EngineName, content_address: String) -> CacheKey {
    CacheKey {
        engine,
        content_address,
    }
}

/// The Lean SCHEMA version (`.design/verified/proof-backends.md` REQ-8 / §2(d)).
/// Bumped when the exporter's emitted-source SHAPE or the obligation→Lean encoding
/// changes (the analogue of `cache::CHECK_SCHEMA_VERSION` for the Lean engine), so a
/// cached Lean `Proven` is invalidated when the exporter logic changes — a HIT is a
/// fresh verify against the CURRENT exporter + spine.
pub const LEAN_SCHEMA_VERSION: u32 = 2;

/// A process-local monotonic nonce for unique replay scratch-file names
/// (`.design/verified/proof-backends.md` REQ-7(ii) — the interactive replay writes a
/// `#print axioms` probe to a temp file). Keyed alongside the pid + item name so
/// concurrent replays in the SAME process never collide on the scratch path (the
/// collision that flaked `interactive_filled_valid_proof_replays_proven` under
/// parallel test runs). Deterministic per call (R-CODE-5).
static NEXT_REPLAY_NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The Thermite→Lean obligation ENGINE (`.design/verified/proof-backends.md` REQ-6/
/// REQ-7/REQ-8 — engine #2; increment (ii-b), the #240 chain). Implements the
/// [`Engine`] trait: `fragment()` = pure-contract items whose constructs are
/// spine-exportable; `discharge()` = export → write to a scratch dir → `lake env
/// lean <file>` (cwd `lean/`) → kernel accept = [`Verdict::Proven`], tactic
/// failure / timeout / lake absent / tier-(c) interactive = [`Verdict::Unknown`]
/// (NEVER [`Verdict::Refuted`] — a Lean tactic failure is NOT a witnessed
/// countermodel, REQ-3 anti-cheat); `trust_profile()` = {Lean kernel + 3 standard
/// axioms, EXP}; `evidence_key()` = obligation content + lean-toolchain content +
/// lake-manifest revs + a `lean/Thermite/` spine content hash + `LEAN_SCHEMA_VERSION`
/// (REQ-8 / §2(d)).
///
/// The engine carries the parsed [`Program`] (the exporter needs the spec-fn
/// definitions + the source item) and the path to the `lean/` package root (where
/// `lake env lean` runs). NOT wired into the default `check` path — Verus stays the
/// sole default engine (byte-identical); this engine is constructed directly by
/// tests + (increment (iii)) the `--engine lean` surface.
#[derive(Debug, Clone)]
// proof-backends REQ-6/REQ-7/REQ-8 (the #240 chain): the Lean engine #2 is the
// engine-#2 public API, constructed DIRECTLY BY TESTS in this increment (the
// `--engine lean` / `#[engine(lean)]` production surface is increment (iii), OQ-1).
// It is forward-declared here exactly like the shipped `EngineName::LeanAuto`
// variant (which carries the SAME per-item allow + rationale), so the four-slot
// `Engine` impl is verified live (`forge/tests/lean_engine.rs`) before the CLI
// dispatcher wires it (R-DEFER-1's non-test consumer arrives with increment (iii)).
#[allow(
    dead_code,
    reason = "proof-backends REQ-6/7/8 (#240): the Lean engine #2 is forward-declared \
              + test-constructed in increment (ii-b); the `--engine lean` production \
              dispatcher is increment (iii) (OQ-1), mirroring the shipped \
              `EngineName::LeanAuto` forward-declaration"
)]
pub struct LeanEngine {
    /// The parsed source program (the exporter resolves the source item + the
    /// spec-fn definitions for `R_item`).
    program: Program,
    /// The `lean/` package root (the cwd for `lake env lean`). The spine modules
    /// (`Thermite.Stabilize` etc.) resolve against this package.
    lean_root: PathBuf,
    /// Whether `LeanAuto` (the auto tactic battery, tiers (a)/(b)) or
    /// `LeanInteractive` (tier (c), no auto). The default is `LeanAuto`.
    name: EngineName,
}

impl LeanEngine {
    /// Construct a `LeanAuto` engine over a parsed program + the `lean/` package
    /// root (`.design/verified/proof-backends.md` REQ-6).
    #[must_use]
    #[allow(
        dead_code,
        reason = "proof-backends REQ-6 (#240): the engine-#2 constructor, test-constructed \
                  in increment (ii-b); the `--engine lean` production dispatcher is \
                  increment (iii) (OQ-1) — see the LeanEngine struct rationale"
    )]
    pub fn new(program: Program, lean_root: PathBuf) -> Self {
        LeanEngine {
            program,
            lean_root,
            name: EngineName::LeanAuto,
        }
    }

    /// The parsed program this engine carries (`.design/verified/proof-backends.md`
    /// REQ-9 — the per-mutant obligation minting on the Lean mutation path needs the
    /// program's spec-fn defs). The read accessor `check::lean_mutation_score` uses.
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// Export the obligation's source item to a self-contained Lean file
    /// (`.design/verified/proof-backends.md` REQ-6). `Ok` carries the exported
    /// source + tier; an [`ExportRefusal`] is "the fragment does not admit this
    /// obligation" (a skip — the engine maps it to `Unknown`, never `Refuted`).
    fn export(&self, o: &Obligation) -> Result<ExportedObligation, ExportRefusal> {
        let item = find_item(&self.program, &o.item).ok_or_else(|| {
            ExportRefusal::OutOfFragment(format!("item `{}` not found in the program", o.item))
        })?;
        export_item(o, &self.program, item)
    }

    /// Locate the `lake` binary (`.design/verified/proof-backends.md` REQ-6 — "locate
    /// lake via PATH / ~/.elan/bin"). Returns the binary name `lake` (resolved on
    /// PATH) or the `~/.elan/bin/lake` absolute fallback if PATH lookup is unlikely.
    /// Deterministic given the environment (R-CODE-5).
    fn lake_binary() -> PathBuf {
        // Prefer the elan-managed lake if present (the live test environment), else
        // the bare `lake` on PATH. We probe the elan path explicitly so a
        // non-login shell (which may not have ~/.elan/bin on PATH) still finds it.
        if let Some(home) = std::env::var_os("HOME") {
            let elan = PathBuf::from(home).join(".elan/bin/lake");
            if elan.exists() {
                return elan;
            }
        }
        PathBuf::from("lake")
    }

    /// Run `lake env lean <file>` in the `lean/` package root and return the kernel
    /// verdict (`.design/verified/proof-backends.md` REQ-7). A clean exit (status 0)
    /// = the kernel accepted the theorem (the auto battery discharged it) →
    /// [`Verdict::Proven`]; a non-zero exit (a tactic failure, an elaboration error)
    /// → [`Verdict::Unknown`] (NEVER `Refuted` — a Lean tactic failure is not a
    /// witnessed countermodel, REQ-3); lake absent (`ENOENT`) → `Unknown`. The
    /// `key` is the engine's evidence key (attached to a `Proven`).
    fn run_lake(&self, file: &std::path::Path, verified: u64, key: CacheKey) -> Verdict {
        use std::process::Command;
        let lake = Self::lake_binary();
        let output = Command::new(&lake)
            .arg("env")
            .arg("lean")
            .arg(file)
            .current_dir(&self.lean_root)
            .output();
        match output {
            Ok(out) if out.status.success() => Verdict::Proven(Evidence { verified, key }),
            Ok(out) => {
                // A non-zero exit is a tactic/elaboration FAILURE — the engine could
                // not kernel-check the theorem. This is `Unknown` (DEGRADE), NEVER
                // `Refuted`: there is no witnessing input (REQ-3 anti-cheat — a Lean
                // tactic failure is not a countermodel).
                let detail = String::from_utf8_lossy(&out.stderr);
                let head: String = detail.chars().take(400).collect();
                Verdict::Unknown(Reason::IncompleteUnknown(format!(
                    "lake/lean did not kernel-accept the exported obligation (tactic \
                     failure / elaboration error — NOT a countermodel, REQ-3): {head}"
                )))
            }
            Err(e) => {
                // lake absent / spawn failure: `Unknown` (the engine could not run),
                // never `Refuted`. R-CODE-4: the subprocess failure is surfaced
                // structured, never swallowed as success.
                Verdict::Unknown(Reason::IncompleteUnknown(format!(
                    "could not invoke `lake env lean` (lake absent or un-spawnable): {e}"
                )))
            }
        }
    }

    /// A content hash of the `lean/Thermite/` spine the exported theorem
    /// INSTANTIATES (`.design/verified/proof-backends.md` §2(d) — "the TARGETED-SPINE
    /// content hash"). Walks `lean/Thermite/**` RECURSIVELY (the #246 widening — the
    /// non-recursive walk left `lean/Thermite/Exec/**` unhashed, so an Exec-subtree
    /// edit kept the SAME key; increment (iv) targets Exec, so the spine hash must
    /// cover the WHOLE subtree). Files are content-addressed by their path RELATIVE to
    /// the spine root (so a moved/renamed file changes the key) and sorted for
    /// determinism (R-CODE-5). On a read error the digest degrades to a marker (never
    /// a panic — R-CODE-2).
    fn spine_content_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let spine_dir = self.lean_root.join("Thermite");
        let mut entries: Vec<PathBuf> = Vec::new();
        if !Self::collect_lean_files(&spine_dir, &mut entries) {
            return "spine-unreadable".to_string();
        }
        // Sort by the path RELATIVE to the spine root (stable across cwd; covers the
        // whole recursive subtree).
        entries.sort_by(|a, b| {
            let ra = a.strip_prefix(&spine_dir).unwrap_or(a);
            let rb = b.strip_prefix(&spine_dir).unwrap_or(b);
            ra.cmp(rb)
        });
        let mut hasher = Sha256::new();
        // The marker is BUMPED to v2 with the recursive widening, so a prior cached
        // key (non-recursive v1) universally MISSES.
        hasher.update(b"thermite-lean-spine-v2-recursive");
        for path in entries {
            if let Ok(bytes) = std::fs::read(&path) {
                let rel = path.strip_prefix(&spine_dir).unwrap_or(&path);
                let rel_str = rel.to_string_lossy();
                hasher.update((rel_str.len() as u64).to_le_bytes());
                hasher.update(rel_str.as_bytes());
                hasher.update((bytes.len() as u64).to_le_bytes());
                hasher.update(&bytes);
            }
        }
        let digest = hasher.finalize();
        digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
    }

    /// Recursively collect every `.lean` file under `dir` into `out`
    /// (`.design/verified/proof-backends.md` §2(d), the #246 recursive widening).
    /// Returns `false` if the ROOT directory is unreadable (the degrade-to-marker
    /// signal); a subdirectory read error is skipped (best-effort, never a panic —
    /// R-CODE-2). Deterministic given the filesystem.
    fn collect_lean_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) -> bool {
        let rd = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => return false,
        };
        for entry in rd.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                // A subdirectory read failure is skipped (best-effort), not fatal.
                let _ = Self::collect_lean_files(&path, out);
            } else if path.extension().is_some_and(|x| x == "lean") {
                out.push(path);
            }
        }
        true
    }

    /// The OBLIGATION-CONTENT hash (`.design/verified/proof-backends.md` §2(d) /
    /// REQ-7(ii), the #246 fix): the canonical emitted Lean terms for `req`/`ens`/
    /// `body`/`dec` PLUS the registry bodies — i.e. the EXPORTER'S RENDERED SOURCE,
    /// which contains exactly those terms. Hashing the rendered source means editing
    /// `ens result >= a` to `ens result >= b` (or editing a reached spec-fn's body)
    /// CHANGES the key (the staleness REQ-7(ii) demands — a cached/replayed `Proven`
    /// can NEVER silently survive a contract change). On an export refusal (the item
    /// is not exportable) the content degrades to a STRUCTURED refusal marker (still a
    /// stable, content-distinguishing string — a refused item never reaches a cached
    /// `Proven` anyway). Deterministic (R-CODE-5); never a panic (R-CODE-2).
    fn obligation_content_hash(&self, o: &Obligation) -> String {
        let content = match find_item(&self.program, &o.item) {
            Some(item) => match export_item(o, &self.program, item) {
                Ok(exported) => exported.source,
                Err(refusal) => format!("export-refused::{refusal}"),
            },
            None => format!("item-absent::{}", o.item),
        };
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(b"thermite-lean-obligation-content-v1");
        h.update(content.as_bytes());
        let digest = h.finalize();
        digest.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The lean-toolchain + lake-manifest revision string (`.design/verified/
    /// proof-backends.md` §2(d) — "the ENGINE-TOOLCHAIN version is the
    /// `lean-toolchain` rev + the `lake-manifest` revs"). Reads the two files;
    /// missing files degrade to a marker (never a panic — R-CODE-2).
    fn toolchain_rev(&self) -> String {
        let toolchain = std::fs::read_to_string(self.lean_root.join("lean-toolchain"))
            .unwrap_or_else(|_| "no-toolchain".to_string());
        let manifest = std::fs::read_to_string(self.lean_root.join("lake-manifest.json"))
            .map(|s| {
                use sha2::{Digest, Sha256};
                let mut h = Sha256::new();
                h.update(s.as_bytes());
                h.finalize()
                    .iter()
                    .take(6)
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            })
            .unwrap_or_else(|_| "no-manifest".to_string());
        format!("{}+manifest:{manifest}", toolchain.trim())
    }
}

impl Engine for LeanEngine {
    fn name(&self) -> EngineName {
        self.name
    }

    fn fragment(&self) -> Fragment {
        // The Lean engine does NOT admit the whole subset — only the pure-contract
        // class whose constructs the exporter can emit (the `admits_all_classes =
        // false` seam REQ-2(a) named for a narrowed engine). The per-obligation
        // admission is decided by `export` succeeding (in `discharge`); the fragment
        // flag marks it as a NARROWED engine so the ladder hook knows to gate on
        // `admits` (which runs the full export attempt).
        Fragment {
            admits_all_classes: false,
        }
    }

    fn discharge(&self, o: &Obligation) -> Verdict {
        // 1. EXPORT. A refusal (out-of-fragment / not-pure-contract / incomplete
        //    registry / open hole) = the fragment does not admit this obligation →
        //    `Unknown` (a skip), NEVER `Refuted`/`Proven` (REQ-3 anti-cheat — a skip
        //    is not a disproof and not a proof).
        let exported = match self.export(o) {
            Ok(e) => e,
            Err(refusal) => {
                return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                    "the Lean engine's fragment does not admit this obligation \
                     (an honest skip, not a verdict): {refusal}"
                )));
            }
        };

        // 2. TIER-(c) is INTERACTIVE-only: the engine does NOT invoke lake (the
        //    `∃N∀fuel` form needs an authored induction). Return `Unknown` WITHOUT
        //    running lake (the file may still be emitted for increment-(iii) use).
        if !exported.tier.is_auto() {
            return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                "tier ({}) recursive-registry obligation is INTERACTIVE-only \
                 (the auto battery does not attempt it; REQ-7(ii)) — Lean-auto SKIPs",
                exported.tier.tag()
            )));
        }

        // 3. WRITE the exported source to a scratch file + INVOKE lake.
        let scratch = match self.write_scratch(o, &exported) {
            Ok(p) => p,
            Err(e) => {
                return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                    "could not write the exported Lean obligation to a scratch file: {e}"
                )));
            }
        };
        let key = self.evidence_key(o);
        let verdict = self.run_lake(&scratch, 1, key);
        // Best-effort scratch cleanup (R: clean up scratch). A cleanup failure does
        // not change the verdict.
        let _ = std::fs::remove_file(&scratch);
        verdict
    }

    fn trust_profile(&self) -> TrustProfile {
        // REQ-2(c) / TRUST PROFILE: {Lean kernel + 3 standard axioms} + EXP (the
        // exporter correspondence). An auditor sees this enumerates a SMALLER base
        // than Verus's {Z3, Verus VC-gen, lowering theorem} along the named axes
        // (§1 / REQ-4 — "smaller along the named axes", OQ-3).
        TrustProfile {
            items: vec![
                "Lean kernel".to_string(),
                "propext".to_string(),
                "Classical.choice".to_string(),
                "Quot.sound".to_string(),
                "EXP (the exporter correspondence — arm-by-arm + the drift tripwire)".to_string(),
            ],
        }
    }

    fn evidence_key(&self, o: &Obligation) -> CacheKey {
        // REQ-2(d) / §2(d): the engine-discriminated key composing the obligation
        // content, the lean-toolchain + lake-manifest revs, the targeted-spine
        // content hash, and the LEAN_SCHEMA_VERSION — so a toolchain OR spine bump
        // forces a universal MISS (a HIT is a fresh verify against the CURRENT
        // semantics + toolchain).
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(b"thermite-lean-evidence-v2");
        h.update(o.item.as_bytes());
        h.update(o.class.tag().as_bytes());
        h.update(o.role.tag().as_bytes());
        for name in &o.env.spec_defs {
            h.update(name.as_bytes());
        }
        // The OBLIGATION CONTENT (#246 / REQ-7(ii)): the canonical emitted Lean terms
        // for req/ens/body/dec + the registry bodies (the rendered exporter source).
        // Two same-named items with DIFFERENT ens (or a reached spec-fn body edit) →
        // different content hash → different key (no silent stale-Proven reuse).
        h.update(self.obligation_content_hash(o).as_bytes());
        h.update(self.toolchain_rev().as_bytes());
        h.update(self.spine_content_hash().as_bytes());
        h.update(LEAN_SCHEMA_VERSION.to_le_bytes());
        let digest = h.finalize();
        let content_address: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        CacheKey {
            engine: self.name,
            content_address,
        }
    }
}

impl LeanEngine {
    /// Write the exported Lean source to a deterministic scratch file in the system
    /// temp dir (`.design/verified/proof-backends.md` REQ-7 — "export → write to a
    /// scratch dir"). The file name is keyed on the item + the process id so
    /// concurrent runs do not collide. Returns the path; the caller invokes lake on
    /// it and removes it after.
    fn write_scratch(
        &self,
        o: &Obligation,
        exported: &ExportedObligation,
    ) -> std::io::Result<PathBuf> {
        let safe: String = o
            .item
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("forge_lean_export_{safe}_{pid}.lean"));
        std::fs::write(&path, &exported.source)?;
        Ok(path)
    }

    /// Does the Lean fragment ADMIT this obligation for an AUTO discharge?
    /// (`.design/verified/proof-backends.md` REQ-9 — the "untested against lean"
    /// boundary.) The Lean fragment's admission is NOT the static `admits_all_classes`
    /// flag (it is `false`); it is whether the obligation EXPORTS and lands in an AUTO
    /// tier (a)/(b). A refusal (out-of-spine / not-pure-contract / incomplete registry
    /// / non-int result) OR a tier-(c) interactive obligation is NOT admitted by the
    /// AUTO path — "untested against lean" (REQ-9), never a kill. This runs the export
    /// (the same one `discharge` runs), so it is the genuine per-mutant admission gate.
    #[must_use]
    pub fn admits_auto(&self, o: &Obligation) -> bool {
        matches!(self.export(o), Ok(e) if e.tier.is_auto())
    }

    /// REPLAY (or EMIT) a tier-(c) item's INTERACTIVE proof artifact
    /// (`.design/verified/proof-backends.md` REQ-7(ii) / §6 tier (c)). The artifact
    /// lives at [`interactive_proof_path`] (`<source_file>.lean-proofs/<item>.lean`):
    ///
    /// - **ABSENT** → EMIT the skeleton (the exporter's tier-(c) source + the
    ///   evidence-key header) and return `Unknown` ("skeleton emitted — an agent
    ///   authors the induction"). A skeleton is NEVER `Proven` (it carries `sorry`).
    /// - **PRESENT** → the STALENESS gate: the emitted `-- evidence_key: <hex>` header
    ///   must match the CURRENT [`evidence_key`](Engine::evidence_key). A MISMATCH =
    ///   STALE → `Unknown("stale proof — re-derive")` (NEVER silently reused). A match
    ///   → REPLAY via `lake env lean`; then DETECT `sorry` explicitly ([`proof_has_sorry`]
    ///   over the source AND `#print axioms`) — a `sorry` → `Unknown` (NEVER `Proven`,
    ///   even though lake exits 0 on a `sorry`); a kernel-accepted, sorry-FREE replay →
    ///   `Proven` with the INTERACTIVE trust profile (the `verified` count is 1 — the
    ///   one item obligation).
    ///
    /// `source_file` is the `.th` source the artifact is checked in beside; `o` is the
    /// tier-(c) obligation. NEVER a panic (R-CODE-2); subprocess failures surfaced
    /// (R-CODE-4); deterministic given the filesystem + toolchain (R-CODE-5).
    pub fn replay_interactive(&self, source_file: &std::path::Path, o: &Obligation) -> Verdict {
        // The exporter's tier-(c) source (the skeleton body). A refusal means the item
        // is not even exportable → an honest skip (Unknown), never a verdict.
        let exported = match self.export(o) {
            Ok(e) => e,
            Err(refusal) => {
                return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                    "the Lean engine cannot export this obligation for an interactive proof \
                     (an honest skip): {refusal}"
                )));
            }
        };
        let key = self.evidence_key(o);
        let header = format!("{INTERACTIVE_EVIDENCE_KEY_MARKER}{}\n", key.content_address);
        let path = interactive_proof_path(source_file, &o.item);

        // The CANONICAL exporter SOURCE — regenerated from the CURRENT obligation (the
        // generator-controlled preamble/imports + `R_item` + the canonical
        // `theorem thermite_obligation_<item> : <statement> := …` line). The replay does
        // NOT validate the author's file by pattern-matching; it RECONSTRUCTS a fresh
        // replay file from THIS canonical source, splicing in ONLY the author's extracted
        // PROOF TERM (proof-backends REQ-6 / R-DEFER-9, the #252 helper-surface elimination
        // — the author file content OUTSIDE the proof term is DROPPED). The statement, name,
        // and `#print axioms` target are then the SAME generator-emitted declaration BY
        // CONSTRUCTION — a same-short-name decoy is structurally impossible.

        // PRESENT → the staleness gate + reconstruct-and-splice replay; ABSENT → emit
        // the skeleton.
        match std::fs::read_to_string(&path) {
            Ok(existing) => {
                self.replay_present_proof(&path, &existing, &key, &o.item, &exported.source)
            }
            Err(_) => {
                // ABSENT: emit the skeleton (header + the tier-(c) exported source).
                if let Some(parent) = path.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                            "could not create the interactive proof directory `{}`: {e}",
                            parent.display()
                        )));
                    }
                }
                let skeleton = format!("{header}{}", exported.source);
                if let Err(e) = std::fs::write(&path, skeleton) {
                    return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                        "could not write the interactive proof skeleton `{}`: {e}",
                        path.display()
                    )));
                }
                Verdict::Unknown(Reason::IncompleteUnknown(format!(
                    "interactive proof skeleton EMITTED to `{}` (carries a `sorry` — an \
                     agent/human authors the induction, REQ-7(ii)); NOT proven",
                    path.display()
                )))
            }
        }
    }

    /// The PRESENT-proof arm of [`replay_interactive`]: the staleness gate →
    /// RECONSTRUCT-AND-SPLICE → replay → sorry detection. Split out so the read/write
    /// I/O stays in the caller.
    ///
    /// The replay does NOT validate the AUTHOR'S file by pattern-matching (the #250
    /// decoy game — a same-short-name decoy theorem that the appended `#print axioms`
    /// probe resolves to while the statement-binding gate reads a DIFFERENT, namespaced
    /// declaration). Instead it RECONSTRUCTS a fresh, fully generator-controlled replay
    /// file from `canonical_source` (the exporter's preamble/imports + `R_item` + the
    /// canonical `theorem thermite_obligation_<item> : <statement> := …` line),
    /// SPLICING in ONLY the author's extracted PROOF TERM (the #252 helper-surface
    /// elimination — the author file content OUTSIDE the proof term is DROPPED, never
    /// spliced; auxiliary lemmas inline as `have`/`let`/`suffices`). The statement, the
    /// theorem name, and the `#print axioms` probe target are then the SAME
    /// generator-emitted declaration BY CONSTRUCTION — a decoy is structurally impossible.
    /// A smuggled axiom USED by the proof appears in the anchored dependency report (the
    /// allowlist catches it); an unused decoy axiom is inert.
    fn replay_present_proof(
        &self,
        path: &std::path::Path,
        existing: &str,
        key: &CacheKey,
        item: &str,
        canonical_source: &str,
    ) -> Verdict {
        // The STALENESS gate (REQ-7(ii)): the header's evidence key must match the
        // CURRENT key. A mismatch = the obligation / toolchain / spine changed → the
        // proof is STALE and must be re-derived, NEVER silently reused. This stays on
        // the AUTHOR'S file (the header the author kept from the emitted skeleton).
        let recorded_key = existing
            .lines()
            .find_map(|l| l.strip_prefix(INTERACTIVE_EVIDENCE_KEY_MARKER))
            .map(str::trim);
        if recorded_key != Some(key.content_address.as_str()) {
            return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                "stale proof — re-derive: the interactive proof `{}` carries evidence key \
                 {recorded:?} but the current obligation's key is `{current}` (the obligation, \
                 Lean toolchain, or targeted spine changed — REQ-7(ii); a stale proof is NEVER \
                 silently reused)",
                path.display(),
                recorded = recorded_key,
                current = key.content_address,
            )));
        }

        // RECONSTRUCT (proof-backends REQ-6 / R-DEFER-9, the #250 + #252 fixes): split the
        // CANONICAL exporter source into (preamble, canonical theorem statement); extract
        // from the AUTHOR'S file ONLY the UNIQUE `thermite_obligation_<item>` declaration's
        // PROOF TERM (the #252 helper-surface elimination — no author HELPERS are spliced);
        // emit a fresh replay file. A DUPLICATE obligation declaration (the #250 decoy) →
        // REJECT; a missing canonical statement (malformed exporter output) → never trusted
        // as a binding.
        let reconstructed = match self.reconstruct_replay(canonical_source, existing, item) {
            Ok(r) => r,
            Err(detail) => {
                return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                    "{detail} (interactive proof `{}`; proof-backends REQ-6 / R-DEFER-9 — the \
                     replay file is RECONSTRUCTED from the canonical exporter source with the \
                     author's proof spliced in; the obligation statement, name, and `#print \
                     axioms` probe target are the same generator-emitted declaration by \
                     construction)",
                    path.display()
                )));
            }
        };

        // The proof is FRESH: REPLAY the RECONSTRUCTED file via lake + capture the
        // ANCHORED `#print axioms <thm>` (already appended by `reconstruct_replay`) for
        // the explicit sorry check + the trust-base axiom ALLOWLIST (lake exits 0 on a
        // `sorry`, so the source/axioms scan is what distinguishes a genuine proof —
        // REQ-7(ii)). The probe target is the canonical declaration by construction.
        let probe = reconstructed;
        let pid = std::process::id();
        // A per-call nonce + the item name keeps the scratch path UNIQUE across
        // concurrent replays in the SAME process (the same pid) — a shared
        // `forge_lean_replay_{pid}.lean` collided under parallel test runs (R-CODE-5:
        // deterministic given the call, no cross-call interference).
        let nonce = NEXT_REPLAY_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let safe_item = proof_thm_sanitize(item);
        let scratch =
            std::env::temp_dir().join(format!("forge_lean_replay_{pid}_{safe_item}_{nonce}.lean"));
        if let Err(e) = std::fs::write(&scratch, &probe) {
            return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                "could not write the interactive replay scratch file: {e}"
            )));
        }
        use std::process::Command;
        let lake = Self::lake_binary();
        let output = Command::new(&lake)
            .arg("env")
            .arg("lean")
            .arg(&scratch)
            .current_dir(&self.lean_root)
            .output();
        let _ = std::fs::remove_file(&scratch);
        match output {
            Ok(out) if out.status.success() => {
                // EXPLICIT sorry detection (REQ-7(ii)): lake exits 0 on a `sorry`, so
                // a clean exit is NOT sufficient. Scan the SOURCE token AND the
                // `#print axioms` output (which prints `sorryAx` for a surviving
                // `sorry`). A `sorry` is NEVER `Proven`.
                let axioms = String::from_utf8_lossy(&out.stdout);
                if proof_has_sorry(&probe, &axioms) {
                    return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                        "the interactive proof `{}` carries an OPEN `sorry` (detected in the \
                         source and/or `#print axioms` — `sorryAx`); a `sorry` is NEVER Proven \
                         (REQ-7(ii)), even though lake exits 0",
                        path.display()
                    )));
                }
                // TRUST-BASE AXIOM ALLOWLIST (proof-backends REQ-4 / REQ-7(ii) / §1 /
                // R-DEFER-9): the enumerable trusted base a Lean cert lists is EXACTLY
                // {Lean kernel + the 3 standard axioms, EXP[, author]}. `#print axioms`
                // reports the WHOLE axiom set the kernel-accepted theorem rests on; any
                // axiom OUTSIDE `{propext, Classical.choice, Quot.sound}` (a smuggled
                // `axiom thermite_cheat : ∀ p, p`, an oracle, …) means the cert's base
                // would be a LIE — the proof rests on more than it enumerates. Such a
                // proof is NEVER Proven (a proof cheat), even though it kernel-accepts.
                match nonstandard_axiom(&axioms, item) {
                    AxiomReport::Nonstandard(extra) => {
                        return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                            "non-standard axiom: {extra}: the interactive proof `{}` \
                             kernel-accepts but the obligation theorem's `#print axioms` rests on \
                             `{extra}`, OUTSIDE the trust-base allowlist {{propext, \
                             Classical.choice, Quot.sound}} (proof-backends REQ-4/§1, R-DEFER-9); \
                             the enumerable trusted base would be a LIE — NEVER Proven",
                            path.display()
                        )));
                    }
                    AxiomReport::Missing => {
                        // No `#print axioms` report for the OBLIGATION theorem in the output:
                        // an author's own earlier `#print axioms <helper>` must NEVER be read in
                        // its place (the #249 marker-mask). Without the obligation's own axiom
                        // list the enumerable base cannot be vouched for — NEVER Proven.
                        return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                            "axiom report missing: the interactive proof `{}` kernel-accepts but \
                             lake emitted no `#print axioms` report for the obligation theorem \
                             `thermite_obligation_{}` (proof-backends REQ-4/§1, R-DEFER-9); its \
                             trusted base cannot be enumerated — NEVER Proven",
                            path.display(),
                            proof_thm_sanitize(item)
                        )));
                    }
                    AxiomReport::Clean => {}
                }
                // A kernel-accepted, sorry-FREE, allowlist-clean, statement-bound replay
                // → Proven with the INTERACTIVE trust profile (the author is a reviewed
                // step, OQ-4).
                Verdict::Proven(Evidence {
                    verified: 1,
                    key: key.clone(),
                })
            }
            Ok(out) => {
                let detail = String::from_utf8_lossy(&out.stderr);
                let head: String = detail.chars().take(400).collect();
                Verdict::Unknown(Reason::IncompleteUnknown(format!(
                    "the interactive proof `{}` did NOT kernel-accept on replay (an \
                     elaboration/tactic error — NOT a countermodel, REQ-3): {head}",
                    path.display()
                )))
            }
            Err(e) => Verdict::Unknown(Reason::IncompleteUnknown(format!(
                "could not invoke `lake env lean` for the interactive replay: {e}"
            ))),
        }
    }

    /// RECONSTRUCT a fresh, ENTIRELY generator-controlled replay file from the CANONICAL
    /// exporter source + ONLY the author's extracted PROOF TERM (`.design/verified/
    /// proof-backends.md` REQ-6 / R-DEFER-9, the #252 ARCHITECTURAL fix — the elimination
    /// of the author HELPER surface). The emitted file is EXACTLY, IN ORDER:
    ///
    /// 1. the canonical PREAMBLE (the exporter's header + `import` + `R_item` + the
    ///    resolution lemmas + the obligation theorem's doc comment) — everything BEFORE the
    ///    canonical `theorem thermite_obligation_<item>` line, regenerated from the CURRENT
    ///    obligation (never trusted from the author);
    /// 2. the CANONICAL theorem line `theorem thermite_obligation_<item> : <statement> :=`
    ///    with the author's EXTRACTED PROOF TERM (everything after the author declaration's
    ///    first `:=`) spliced after `:=`;
    /// 3. the ANCHORED `#print axioms thermite_obligation_<item>` probe.
    ///
    /// **The #252 architectural decision (ending the command-injection whack-a-mole — 5
    /// bypass generations #248..#252).** The replay file carries NO author-controlled text
    /// other than the PROOF TERM. The earlier design spliced an author HELPERS section into
    /// the obligation's elaboration scope and tried to SANITIZE it with a command blocklist
    /// (`disallowed_helper_command`). A blocklist on a Turing-complete elaborator cannot be
    /// made sound: #251 closed column-0 commands; #252 escaped via INDENTATION (Lean is
    /// whitespace-insensitive at the top level, so an indented `notation:max
    /// "Thermite.stabilizesProp" => (fun _ _ => True)` re-elaborates the byte-identical
    /// canonical statement to `True`); unicode-whitespace / comment-nesting / `open … in`
    /// variants would follow. ELIMINATING the helper surface ENTIRELY is the sound fix: the
    /// author can only supply the PROOF TERM, which the kernel type-checks against the
    /// FIXED, generator-emitted, already-elaborated goal type (the statement is left of
    /// `:=` and is generator-controlled). A proof term cannot vacate that goal; `sorry`/
    /// `admit` → `sorryAx` and `native_decide` → `ofReduceBool` are caught by the axiom
    /// allowlist. Legit auxiliary lemmas inline as `have`/`let`/`suffices` INSIDE the proof
    /// term (Lean supports this fully in tactic + term mode — no expressivity loss for a
    /// single-obligation proof).
    ///
    /// REJECTS (returns `Err(detail)`) when: the short name `thermite_obligation_<item>`
    /// occurs as a declaration MORE than once anywhere in the author's file (the #250
    /// decoy — "duplicate obligation declaration"); the canonical source has no extractable
    /// statement; the author's file has no obligation declaration to splice a proof from;
    /// the author's declared statement does not match the canonical one (modulo whitespace);
    /// or (the #252 BELT) the extracted proof term carries a top-level COMMAND keyword in
    /// any position (an `… in`-style command form smuggled into the term). Deterministic
    /// (R-CODE-5); never a panic (R-CODE-2).
    fn reconstruct_replay(
        &self,
        canonical_source: &str,
        author_file: &str,
        item: &str,
    ) -> Result<String, String> {
        let thm_name = format!("thermite_obligation_{}", proof_thm_sanitize(item));
        let needle = format!("theorem {thm_name}");

        // (1) The canonical PREAMBLE + the canonical STATEMENT, regenerated from the
        // exporter source (generator-controlled).
        let canonical_statement =
            canonical_theorem_statement(canonical_source, item).ok_or_else(|| {
                "the canonical exporter source has no extractable obligation theorem statement \
                 (a malformed exporter output — never trusted as a binding)"
                    .to_string()
            })?;
        let preamble_end = canonical_source.find(&needle).ok_or_else(|| {
            "the canonical exporter source declares no obligation theorem to anchor the \
             reconstruction on"
                .to_string()
        })?;
        let preamble = canonical_source[..preamble_end].trim_end();

        // (2) UNIQUENESS + the author's PROOF TERM. Count the obligation theorem's
        // SHORT-NAME declaration sites in the author's file: MORE than one anywhere (any
        // namespace) → the #250 decoy → REJECT. EXACTLY one → splice its proof term.
        let decl_sites = declaration_sites(author_file, &thm_name);
        if decl_sites.len() > 1 {
            return Err(format!(
                "duplicate obligation declaration: the short name `{thm_name}` is declared \
                 {} times in the author's proof file (a same-short-name decoy is structurally \
                 a cheat — the #250 mask); REJECTED",
                decl_sites.len()
            ));
        }
        let decl_start = match decl_sites.first() {
            Some(&s) => s,
            None => {
                return Err(format!(
                    "the author's proof file declares no `{thm_name}` to splice a proof from"
                ));
            }
        };
        // The author's PROOF TERM: everything after the declaration's FIRST `:=` up to
        // the declaration's END (the next top-level command/declaration, or EOF).
        let decl_end = decl_block_end(author_file, decl_start);
        let decl_text = &author_file[decl_start..decl_end];
        let assign = proof_assign_pos(decl_text).ok_or_else(|| {
            format!("the author's `{thm_name}` declaration has no `:=` proof term to splice")
        })?;
        let proof_term = decl_text[assign + ":=".len()..].trim_end();

        // DEFENSE LAYER (proof-backends REQ-6, retained): the reconstruction FORCES the
        // canonical statement by construction, but we ALSO cross-check that the author's
        // OWN declaration statement matches the canonical one (modulo whitespace). A
        // mismatch is a stale/wrong-statement author file — REJECT with a precise
        // diagnostic rather than silently overwriting their (different) statement.
        let author_statement = &decl_text[..assign + ":=".len()];
        if !statements_match(author_statement, &canonical_statement) {
            return Err(format!(
                "statement mismatch: the author's `{thm_name}` declaration proves \
                 `{author_statement}` but the current obligation's canonical statement is \
                 `{canonical_statement}` (the author fills ONLY the proof term after `:=`)"
            ));
        }

        // THE #252 BELT (proof-backends REQ-6 / §1 / R-DEFER-9): the proof term is the ONLY
        // author-controlled text, and it is type-checked against the FIXED generator-emitted
        // goal — a proof term cannot vacate that goal. As a cheap DEFENSE LAYER against an
        // `… in`-style top-level command form smuggled into the term, REJECT if the proof
        // term carries a top-level command keyword in ANY position (exact-token,
        // whitespace-independent). The author content OUTSIDE the proof term is DROPPED
        // (never spliced) — there is no helper surface to sanitize.
        if let Some(kw) = proof_term_command_token(proof_term) {
            return Err(format!(
                "disallowed proof-term command: {kw}: the author's proof term carries a \
                 top-level command keyword `{kw}` (a `notation`/`macro`/`macro_rules`/`syntax`/ \
                 `set_option`/`attribute`/`instance`/`open`/`import`/`#…`-style command form, \
                 e.g. smuggled via `… in`) — a command can ALTER the elaboration of the \
                 obligation theorem and forge an L3 cert from a proof of `True` (proof-backends \
                 REQ-6/§1, R-DEFER-9); the proof term may contain ONLY term/tactic syntax \
                 (auxiliary lemmas inline as `have`/`let`/`suffices`); REJECTED"
            ));
        }

        // (3)+(4) Emit: canonical preamble + canonical theorem (spliced proof term) + the
        // ANCHORED probe. NO author HELPER section exists — the ONLY author-controlled text
        // is the proof term (the #252 elimination). Any author file content outside the
        // proof term (an indented `notation`, a file-level helper, …) is DROPPED — it has
        // nowhere to live, so it can never share the obligation's elaboration scope.
        Ok(format!(
            "{preamble}\n\n{canonical_statement} {proof_term}\n\
             #print axioms {thm_name}\n"
        ))
    }
}

/// A Lean-identifier-safe form of an item name for the `#print axioms` probe theorem
/// name (mirrors `lean_export::sanitize`; deterministic, R-CODE-5).
fn proof_thm_sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The REQ-3 discharge discipline, generalized off the SHIPPED ladder
/// (`.design/verified/proof-backends.md` REQ-3): map an engine's [`Verdict`] for a
/// `role = Certification` obligation to the SHIPPED [`crate::degrade::L3Verdict`]
/// the ladder consumes. `Proven` → certify L3 (the carried cert); `Unknown` →
/// the `Timeout`-shaped degrade trigger (`run_ladder` → L2/L1); `Refuted` → the
/// `Counterexample` HARD FAIL (NEVER degrades). The §0.1 meta/battery queries
/// (`role` ≠ `Certification`) are OUTSIDE this discipline — they are not minted as
/// `Obligation`s in v1, so this function is total over the `Certification` role.
///
/// `proved_cert` / `cx_cert` are the assembled certs the caller built from the
/// same outcome (the L3 cert on `Proven`, the counterexample cert on `Refuted`);
/// the `Unknown` arm carries the degrade `reason` onto the lower rung (REQ-4),
/// matching the SHIPPED `VerusTimeout` reason shape.
#[must_use]
pub fn verdict_ladder_action(
    verdict: &Verdict,
    role: ObligationRole,
    proved_cert: crate::manifest::Certificate,
    cx_cert: crate::manifest::Certificate,
) -> crate::degrade::L3Verdict {
    // REQ-3 applies to CERTIFICATION obligations only. The §0.1 meta queries are
    // not minted as Obligations in v1, so `role` is always `Certification` here;
    // the match makes the scoping explicit + total (no `_` swallow).
    match role {
        ObligationRole::Certification => match verdict {
            Verdict::Proven(_) => crate::degrade::L3Verdict::Proved(proved_cert),
            // Unknown DEGRADES (REQ-3): the SHIPPED ladder's `Timeout` trigger runs
            // L2/L1. A fast-`unknown` (REQ-3.1) lands here, matching §6's
            // degrade-on-incompleteness intent — NOT a hard fail.
            Verdict::Unknown(reason) => crate::degrade::L3Verdict::Timeout {
                reason: crate::manifest::RejectReason {
                    cause: "VerusTimeout".to_string(),
                    detail: match reason {
                        Reason::VerusTimeout(d) => d.clone(),
                        Reason::IncompleteUnknown(d) => format!(
                            "verus returned an incompleteness-`unknown` (no witnessing \
                             input); degrading per the ladder (REQ-3.1): {d}"
                        ),
                    },
                },
            },
            // Refuted HARD-FAILS (REQ-3 anti-cheat): a WITNESSED countermodel NEVER
            // degrades. This generalizes `ladder_action_l3`'s
            // `Counterexample → HardFail`.
            Verdict::Refuted(_) => crate::degrade::L3Verdict::Counterexample(cx_cert),
        },
    }
}

/// Does a `Counterexample` outcome carry the genuine SMT-INCOMPLETENESS `unknown`
/// signature? (`.design/verified/proof-backends.md` REQ-3.1.) This is the NARROW
/// remap predicate: the REQ-3.1 fast-`unknown` is specifically the case where the
/// SMT solver returned `unknown` (the solver could not DECIDE — an incompleteness
/// event semantically like a timeout), as opposed to (a) a genuine WITNESSED
/// countermodel (`postcondition not satisfied` with a parsed `--> span`), or (b) a
/// FRONTEND rejection (a TYPE error `error[E…]` — e.g. the IFC un-typeable
/// `careless_query` E0308 the provenance corpus pins at L0).
///
/// The SHIPPED `classify_verus_outcome` lumps all three span-less failures into the
/// `Counterexample` bucket. To keep the cert oracle BYTE-IDENTICAL (the increment
/// (i) AC), the remap fires ONLY on the genuine incompleteness signature and
/// DEFAULTS to `Refuted` (the SHIPPED `Counterexample → HardFail`) for everything
/// else. The signature: NO obligation carries a witnessing `--> span` location
/// (a real countermodel is witnessed and stays `Refuted`), AND no diagnostic
/// carries a FRONTEND error marker (`error[E` — a Rust/VIR type error is a genuine
/// rejection, not an SMT `unknown`, and stays `Refuted` → L0), AND a diagnostic
/// explicitly names the SMT `unknown` incompleteness verdict. This makes the remap
/// INERT on the corpus (which contains witnessed failures + E0308 type errors, NOT
/// genuine SMT `unknown`s) — every `conformance/*.cert.json` is unperturbed.
/// Determinism: a pure function of the parsed obligations (R-CODE-5).
#[must_use]
pub fn counterexample_is_incompleteness_unknown(
    obligations: &[crate::manifest::ObligationResult],
) -> bool {
    // A WITNESSED countermodel (any parsed `--> span`) is a genuine disproof → NOT
    // remapped (stays `Refuted`).
    if obligations.iter().any(|o| o.location.is_some()) {
        return false;
    }
    // A FRONTEND error (`error[E…]` — a type/VIR rejection like the IFC E0308) is a
    // genuine rejection, NOT an SMT `unknown` → NOT remapped (stays `Refuted` → L0,
    // preserving the provenance corpus oracle).
    let has_frontend_error = obligations.iter().any(|o| {
        o.diagnostic
            .as_deref()
            .is_some_and(|d| d.contains("error[E"))
    });
    if has_frontend_error {
        return false;
    }
    // The genuine incompleteness signature: a diagnostic explicitly naming the SMT
    // `unknown` verdict (verus surfaces "unknown" when Z3 returns `unknown` without
    // a model). ONLY this narrow case degrades (REQ-3.1). A bare/empty diagnostic
    // is NOT remapped — without a positive `unknown` signal we keep the SHIPPED
    // hard-fail (conservative: the corpus stays byte-identical).
    obligations.iter().any(|o| {
        o.diagnostic
            .as_deref()
            .is_some_and(|d| d.to_ascii_lowercase().contains("unknown"))
    })
}

// ============================================================================
// REQ-4 — CERTIFICATE ATTRIBUTION (`.design/verified/proof-backends.md` REQ-4 / §5,
// increment (iii), #247): the per-obligation `{engine, trust_profile}` pair. ADDITIVE
// (the cert field is `Option`, populated ONLY when a NON-DEFAULT engine discharges),
// so the default Verus path leaves it `None` and the corpus certs stay byte-identical.
// Honest-min project aggregation is UNCHANGED — this is per-obligation metadata
// ORTHOGONAL to `Level` (§5 "project aggregation stays honest-min").
// ============================================================================

/// The per-obligation engine attribution (`.design/verified/proof-backends.md`
/// REQ-4): the ENGINE that proved an obligation + that engine's TRUST PROFILE, so an
/// auditor reading an L3 cert SEES whether L3-via-Lean enumerates a SMALLER base
/// ({Lean kernel + 3 axioms, EXP}) than L3-via-Verus ({Z3, Verus VC-gen, lowering
/// theorem}). A serde VALUE (the `Certificate` field is `Option<EngineAttribution>`,
/// additive). Determinism: a pure function of the engine identity (R-CODE-5).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EngineAttribution {
    /// The engine that discharged the obligation (its stable tag).
    pub engine: String,
    /// The enumerated named trust items that engine ADDS on a `Proven` (the §1
    /// enumerable trusted base — the auditor-visible base).
    pub trust_profile: Vec<String>,
}

/// Build the [`EngineAttribution`] for an engine (`.design/verified/proof-backends.md`
/// REQ-4): the `{engine tag, trust profile items}` pair. Called on the discharge path
/// whenever a NON-DEFAULT engine (Lean) proves an obligation, so the cert records the
/// smaller trust base; the default Verus path does NOT attach it (the cert stays
/// byte-identical — the `serde(default)` keeps the goldens green).
#[must_use]
pub fn attribution_for(engine: &dyn Engine) -> EngineAttribution {
    EngineAttribution {
        engine: engine.name().tag().to_string(),
        trust_profile: engine.trust_profile().items,
    }
}

// ============================================================================
// REQ-5 — THE DISAGREEMENT HALT (`.design/verified/proof-backends.md` REQ-5 / §5 /
// AC-5, increment (iii), #247): one engine `Proven` + another `Refuted` (a WITNESSED
// countermodel) on the SAME certification obligation = a SOUNDNESS ALARM. The
// toolchain HALTS with a structured hard error naming BOTH engines + the obligation;
// it NEVER silently picks the favorable `Proven`. `Proven ⊕ Unknown` is BENIGN (the
// Unknown engine simply could not decide — and per REQ-3.1 a witness-less Verus
// failure is `Unknown`, so it cannot spuriously fire this alarm against a Lean kernel
// `Proven`).
// ============================================================================

/// A SOUNDNESS ALARM (`.design/verified/proof-backends.md` REQ-5): one engine
/// `Proven` and another `Refuted` (a WITNESSED countermodel) on the SAME obligation.
/// A genuine countermodel from one engine contradicting a "proof" from another means
/// one engine (or the exporter/lowering, or `S` itself) is unsound; proceeding would
/// launder unsoundness into a certificate — the exact failure §1's enumerable-base
/// promise forbids. Carries both engine names + the obligation + the refuting
/// counterexample (the deliverable, §5.1 "counterexamples, not adjectives").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disagreement {
    /// The engine that returned `Proven`.
    pub proven_engine: String,
    /// The engine that returned `Refuted` (the witnessed countermodel).
    pub refuted_engine: String,
    /// The obligation's item (the §5.3 per-item identity).
    pub item: String,
    /// The refuting counterexample (the witnessing input — the deliverable).
    pub counterexample: Counterexample,
}

impl std::fmt::Display for Disagreement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ENGINE DISAGREEMENT on `{item}`: engine `{proven}` returned Proven while \
             engine `{refuted}` returned a WITNESSED counterexample. This is a SOUNDNESS \
             ALARM (proof-backends REQ-5) — one engine (or the exporter/lowering, or S \
             itself) is unsound. The toolchain HALTS; it NEVER picks the favorable verdict. \
             Counterexample obligations: {cx:?}",
            item = self.item,
            proven = self.proven_engine,
            refuted = self.refuted_engine,
            cx = self.counterexample.obligations,
        )
    }
}

/// Check the verdicts of TWO engines on the SAME obligation for the disagreement
/// alarm (`.design/verified/proof-backends.md` REQ-5 / AC-5). This is the
/// multi-engine dispatch guard: if one verdict is `Proven` and the other is `Refuted`
/// (a WITNESSED countermodel), HALT with a structured [`Disagreement`]. EVERY other
/// pairing is benign (`Ok`): `Proven ⊕ Unknown`, `Proven ⊕ Proven`, `Unknown ⊕
/// anything`, `Refuted ⊕ Refuted` (both witnessed a failure — agreement, not a
/// soundness contradiction; the hard fail stands), etc. Per REQ-3.1 a Verus
/// witness-less fast-`unknown` is `Unknown`, so it can NEVER fire this alarm against a
/// Lean `Proven` — only a WITNESSED countermodel can, which is exactly the real
/// unsoundness case. Determinism: a pure function of the two verdicts (R-CODE-5).
pub fn check_disagreement(
    item: &str,
    engine_a: EngineName,
    verdict_a: &Verdict,
    engine_b: EngineName,
    verdict_b: &Verdict,
) -> Result<(), Disagreement> {
    match (verdict_a, verdict_b) {
        (Verdict::Proven(_), Verdict::Refuted(cx)) => Err(Disagreement {
            proven_engine: engine_a.tag().to_string(),
            refuted_engine: engine_b.tag().to_string(),
            item: item.to_string(),
            counterexample: cx.clone(),
        }),
        (Verdict::Refuted(cx), Verdict::Proven(_)) => Err(Disagreement {
            proven_engine: engine_b.tag().to_string(),
            refuted_engine: engine_a.tag().to_string(),
            item: item.to_string(),
            counterexample: cx.clone(),
        }),
        // Every other pairing is BENIGN — including Proven ⊕ Unknown (the Unknown
        // engine simply could not decide), Refuted ⊕ Refuted (both witnessed a
        // failure — agreement on the bug), and any Unknown pairing.
        _ => Ok(()),
    }
}

// ============================================================================
// REQ-7 — INTERACTIVE PROOFS (`.design/verified/proof-backends.md` REQ-7(ii) / §4
// "INTERACTIVE" / §6 tier (c), increment (iii), #247): for a tier-(c) item the engine
// EMITS the skeleton to `<file>.lean-proofs/<item>.lean` when ABSENT; when PRESENT the
// file is REPLAYED (lake) with the obligation-hash STALENESS gate (the emitted header
// carries the evidence_key; a mismatch = stale → Unknown("stale proof — re-derive"),
// NEVER silently reused). A `sorry` is detected explicitly (lake exits 0 on a `sorry`
// — CHECK and handle) and is NEVER `Proven`. A kernel-accepted, sorry-FREE replay =
// `Proven` with the INTERACTIVE trust profile.
// ============================================================================

/// The evidence-key header line a skeleton / interactive proof carries
/// (`.design/verified/proof-backends.md` REQ-7(ii)). The emitted header pins the
/// obligation's evidence key so a REPLAY can detect STALENESS (a changed obligation /
/// toolchain / spine bumps the key → the header no longer matches → the proof is
/// stale and must be re-derived, NEVER silently reused).
pub const INTERACTIVE_EVIDENCE_KEY_MARKER: &str = "-- evidence_key: ";

/// The deterministic path of a tier-(c) item's INTERACTIVE proof artifact
/// (`.design/verified/proof-backends.md` REQ-7(ii) — "a proof file checked in next to
/// the source"): `<file>.lean-proofs/<item>.lean`. The artifact lives beside the
/// SOURCE so it is reviewed + version-controlled with it (OQ-4). Deterministic
/// (R-CODE-5).
#[must_use]
pub fn interactive_proof_path(source_file: &std::path::Path, item: &str) -> PathBuf {
    let dir = {
        let mut d = source_file.as_os_str().to_os_string();
        d.push(".lean-proofs");
        PathBuf::from(d)
    };
    let safe: String = item
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    dir.join(format!("{safe}.lean"))
}

/// Does an interactive proof source carry an OPEN `sorry` (`.design/verified/
/// proof-backends.md` REQ-7(ii) — "lake warns but exits 0 on sorry — CHECK and handle:
/// sorry detection must be explicit; sorry NEVER Proven")? The check is TWO-fold: (1)
/// a textual `sorry` token in the SOURCE (the skeleton's placeholder an agent must
/// fill), and (2) a `sorryAx` / `sorry` in the `#print axioms` OUTPUT (a `sorry` that
/// survived elaboration — the authoritative kernel signal). Either is an open hole →
/// the proof is NOT a genuine kernel proof and is NEVER `Proven`. Determinism: a pure
/// function of the inspected strings (R-CODE-5).
#[must_use]
pub fn proof_has_sorry(source: &str, print_axioms_output: &str) -> bool {
    source_contains_sorry_token(source) || axioms_contain_sorry(print_axioms_output)
}

/// A textual `sorry` token in the proof source (a whole-word match so a substring
/// like `sorryless` does not false-positive). The skeleton emits exactly `  sorry  --
/// INTERACTIVE …`, so an UNFILLED skeleton trips this.
fn source_contains_sorry_token(source: &str) -> bool {
    source
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|tok| tok == "sorry")
}

/// A `sorryAx` / `sorry` axiom in a `#print axioms` output (the authoritative kernel
/// signal that a `sorry` survived elaboration — lake exits 0 on a `sorry`, so the
/// axioms output is what distinguishes a genuine kernel proof from a `sorry`-carrying
/// one).
fn axioms_contain_sorry(print_axioms_output: &str) -> bool {
    let lower = print_axioms_output.to_ascii_lowercase();
    lower.contains("sorryax") || lower.contains("sorry")
}

/// The trust-base axiom ALLOWLIST: the standard Lean axiom set the kernel-proven spine
/// itself rests on (`{propext, Classical.choice, Quot.sound}` — `.design/verified/
/// thermite-semantics.md`, the (T1)/(T2) axiom enumeration). A Lean cert's enumerable
/// trusted base is EXACTLY this set + EXP[, author]; an axiom outside it is a smuggled
/// dependency the cert would NOT enumerate.
const STANDARD_AXIOM_ALLOWLIST: [&str; 3] = ["propext", "Classical.choice", "Quot.sound"];

/// The outcome of inspecting a `#print axioms` output for the obligation theorem's own
/// report line (`.design/verified/proof-backends.md` REQ-4/§1, R-DEFER-9).
#[derive(Debug, PartialEq, Eq)]
enum AxiomReport {
    /// The obligation theorem's report line lists only allowlisted axioms (or it does
    /// NOT depend on any axioms): the enumerable base is honest.
    Clean,
    /// The obligation theorem's report line lists a NON-standard axiom (a smuggled
    /// dependency the cert would not enumerate): the named axiom is outside the allowlist.
    Nonstandard(String),
    /// NO report line for the obligation theorem was found in the output. The parser must
    /// NEVER fall through to a foreign theorem's report (an author's own `#print axioms`
    /// emitted earlier) — a missing anchor is a hard reject, never `Clean`.
    Missing,
}

/// Strictly parse a `#print axioms` output ANCHORED on the OBLIGATION theorem's OWN report
/// line and classify it (`.design/verified/proof-backends.md` REQ-4/§1, R-DEFER-9). Lake
/// prints the inspected theorem's quoted name verbatim: `'thermite_obligation_<item>'
/// depends on axioms: [a, b, c]` or `'thermite_obligation_<item>' does not depend on any
/// axioms`. The author's checked-in proof file is ARBITRARY Lean and may emit its OWN
/// `#print axioms <clean_helper>` BEFORE the appended obligation probe — so we must bind to
/// the OBLIGATION theorem's report, NOT the first `depends on axioms:` line, or a clean
/// helper's report masks the obligation's smuggled axiom (the #249 divergence). We scan
/// ALL lines for the anchor `'<thm>' …`; if MULTIPLE match we inspect every one (the first
/// non-standard axiom across them wins); if NONE match → [`AxiomReport::Missing`] (never
/// fall through to a foreign line). The bracket list is parsed STRICTLY: split on `,`,
/// trim, reject any name not in the allowlist. `sorryAx` is OUT of the allowlist too, so
/// this ALSO catches a surviving `sorry` — but [`proof_has_sorry`] runs first for the
/// dedicated `sorry` message. Deterministic, a pure function of its inputs (R-CODE-5).
#[must_use]
fn nonstandard_axiom(print_axioms_output: &str, item: &str) -> AxiomReport {
    // The anchor is the OBLIGATION theorem's quoted name as lake prints it: `'<thm>'`.
    let thm_name = format!("thermite_obligation_{}", proof_thm_sanitize(item));
    let anchor = format!("'{thm_name}'");
    const MARKER: &str = "depends on axioms:";

    // Inspect EVERY line that names the obligation theorem (defense in depth: if the
    // output carries more than one report for it, all are checked). A line of the form
    // `'<thm>' depends on axioms: [a, b, c]` carries the bracket list; the other form,
    // `'<thm>' does not depend on any axioms`, has no bracket after the marker → clean.
    let mut saw_anchor = false;
    for line in print_axioms_output.lines() {
        if !line.contains(&anchor) {
            continue;
        }
        saw_anchor = true;
        // Anchor on `depends on axioms:` so lake's WARNING/linter text (which can itself
        // carry `[…]` lists — a `simp only [Thermite.Env.bindInt, …]` unused-arg hint) is
        // never mistaken for the axiom list. No marker on this anchored line → no bracket
        // list (the `does not depend on any axioms` form) → clean for this line.
        let Some(marker_pos) = line.find(MARKER) else {
            continue;
        };
        let after_marker = &line[marker_pos + MARKER.len()..];
        let Some(open) = after_marker.find('[') else {
            continue;
        };
        let after = &after_marker[open + 1..];
        let Some(close) = after.find(']') else {
            continue;
        };
        let list = &after[..close];
        if let Some(extra) = list
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .find(|name| !STANDARD_AXIOM_ALLOWLIST.contains(name))
        {
            return AxiomReport::Nonstandard(extra.to_string());
        }
    }
    if saw_anchor {
        AxiomReport::Clean
    } else {
        // No report line named the obligation theorem. Never fall through to a foreign
        // theorem's report — a missing anchor is a hard reject (the obligation's real
        // axiom list is unknown, so the enumerable base cannot be vouched for).
        AxiomReport::Missing
    }
}

/// Extract the CANONICAL theorem STATEMENT of `thermite_obligation_<item>` from a Lean
/// source (`.design/verified/proof-backends.md` REQ-6 — the STATEMENT BINDING surface):
/// the text from the `theorem thermite_obligation_<item>` keyword through (and
/// including) the `:=` that begins the proof term — i.e. the binders + the proposition
/// the author may NOT change (they fill only the proof after `:=`/`by`). The proof
/// delimiter is anchored on the FIRST `:= by` / bare `:=` AFTER the theorem header that
/// is NOT a record-update `:=` (the spine's `{ v with specs := R_item }` uses `:=`
/// INSIDE the proposition); we anchor on `:= by`, falling back to a `:=` not preceded by
/// ` with ` … — but the emitted forms ALWAYS close with `:= by`, so `:= by` is the
/// reliable anchor. Returns `None` when no such theorem/`:= by` is found. Deterministic
/// (R-CODE-5).
#[must_use]
fn canonical_theorem_statement(source: &str, item: &str) -> Option<String> {
    let thm_name = format!("thermite_obligation_{}", proof_thm_sanitize(item));
    let needle = format!("theorem {thm_name}");
    let start = source.find(&needle)?;
    let from_thm = &source[start..];
    // The proof term starts at `:= by` (both the auto and interactive emitted forms
    // close the conclusion with `… := by`). A record-update `specs := R_item` never has
    // ` by` after `:=`, so `:= by` is unambiguous. Include up to and INCLUDING `:=`.
    let by_pos = from_thm.find(":= by").or_else(|| {
        // Defensive: a hand-authored proof might use `:= <term>` (no `by`). Anchor on
        // the LAST `:=` whose left context is not a record-update ` with … specs`.
        from_thm.rfind(":=")
    })?;
    Some(from_thm[..by_pos + 2].to_string())
}

/// Whitespace-insensitive equality of two theorem statements (`.design/verified/
/// proof-backends.md` REQ-6 — "modulo whitespace; be strict"). Collapses every run of
/// ASCII/Unicode whitespace to a single space and trims, so the author's reformatting
/// of the EMITTED skeleton's statement (line wrapping) does not spuriously mismatch, but
/// a DIFFERENT statement (a different proposition / binders) does. Deterministic
/// (R-CODE-5).
#[must_use]
fn statements_match(a: &str, b: &str) -> bool {
    fn norm(s: &str) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }
    norm(a) == norm(b)
}

/// The Lean DECLARATION keywords a top-level declaration can begin with (the RECONSTRUCT
/// -AND-SPLICE boundary detection, `.design/verified/proof-backends.md` REQ-6 / the #250
/// fix). A line whose first whitespace-trimmed token is one of these BEGINS a new
/// top-level declaration; `#`-prefixed lines (`#print`/`#check`/`#eval`) and the section
/// commands (`namespace`/`section`/`end`/`open`/`variable`) likewise END the preceding
/// declaration's text. Used to bound a declaration's body and to detect declaration sites.
const DECL_KEYWORDS: [&str; 8] = [
    "theorem", "lemma", "def", "abbrev", "example", "instance", "axiom", "opaque",
];

/// Does a source LINE begin a top-level Lean declaration or command (the boundary that
/// ENDS the preceding declaration's text)? A `#`-command (`#print`/`#check`) or a section
/// command also ends it. The line must be a TOP-LEVEL line (column 0 — a declaration's
/// continuation/proof lines are indented). Deterministic (R-CODE-5).
fn line_is_top_level_boundary(line: &str) -> bool {
    if line.starts_with(char::is_whitespace) {
        return false;
    }
    if line.starts_with('#') {
        return true;
    }
    let first = line
        .split(|c: char| c.is_whitespace() || c == '(' || c == ':')
        .next();
    matches!(
        first,
        Some(t) if DECL_KEYWORDS.contains(&t)
            || matches!(t, "namespace" | "section" | "end" | "open" | "variable" | "set_option")
    )
}

/// The byte offset of the END of the top-level declaration that STARTS at `start`
/// (`.design/verified/proof-backends.md` REQ-6, the #250 fix): the start of the NEXT
/// top-level boundary line strictly after `start`'s own line, or the source length. So
/// the declaration's text (its statement + proof body, including indented continuation
/// lines) is `source[start..decl_block_end(source, start)]`. Deterministic (R-CODE-5).
fn decl_block_end(source: &str, start: usize) -> usize {
    let mut offset = start;
    let mut first = true;
    for line in source[start..].split_inclusive('\n') {
        if !first && line_is_top_level_boundary(line) {
            return offset;
        }
        first = false;
        offset += line.len();
    }
    source.len()
}

/// The byte offset of a declaration's PROOF-delimiter `:=` (`.design/verified/
/// proof-backends.md` REQ-6, the #250 fix) — distinguished from a record-update `:=`
/// INSIDE the proposition (the spine's `{ v with specs := R_item }`). The emitted /
/// authored forms close the conclusion with `… := by` (auto + interactive induction), so
/// `:= by` is the primary, unambiguous anchor. A term-mode hand proof (`… := <term>`, no
/// `by`) falls back to the FIRST `:=` whose immediately-preceding non-space token is NOT
/// `specs` (the only record-update key the exporter emits). Returns the offset of that
/// `:=`, or `None`. Deterministic (R-CODE-5).
fn proof_assign_pos(decl_text: &str) -> Option<usize> {
    if let Some(p) = decl_text.find(":= by") {
        return Some(p);
    }
    // Term-mode fallback: the first `:=` not immediately preceded by `specs ` (the
    // record-update key). Scan all `:=` occurrences in order.
    let mut from = 0usize;
    while let Some(rel) = decl_text[from..].find(":=") {
        let pos = from + rel;
        let lhs = decl_text[..pos].trim_end();
        if !lhs.ends_with("specs") {
            return Some(pos);
        }
        from = pos + 2;
    }
    None
}

/// Every byte offset in `source` at which the SHORT NAME `thm_name` is DECLARED — i.e.
/// appears as a standalone token immediately after a [`DECL_KEYWORDS`] keyword, in ANY
/// namespace (`.design/verified/proof-backends.md` REQ-6 / R-DEFER-9, the #250 fix). The
/// offset points at the DECLARATION keyword (so the proof extraction starts there). MORE
/// than one site is the #250 same-short-name decoy → the caller REJECTS. Deterministic
/// (R-CODE-5).
fn declaration_sites(source: &str, thm_name: &str) -> Vec<usize> {
    let mut sites = Vec::new();
    for keyword in DECL_KEYWORDS {
        // The declaration prefix `<keyword> <thm_name>` with the NAME a standalone token
        // (the char after the name is a non-identifier char — a space, `:`, `(`, `\n`, …).
        let prefix = format!("{keyword} {thm_name}");
        let mut from = 0usize;
        while let Some(rel) = source[from..].find(&prefix) {
            let kw_start = from + rel;
            let name_end = kw_start + prefix.len();
            let boundary_ok = source[name_end..]
                .chars()
                .next()
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '.'));
            // The keyword itself must start a TOKEN (preceded by start-of-input or a
            // non-identifier char), so `mytheorem` / `defx` do not false-match.
            let kw_token_ok = source[..kw_start]
                .chars()
                .next_back()
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
            if boundary_ok && kw_token_ok {
                sites.push(kw_start);
            }
            from = name_end;
        }
    }
    sites.sort_unstable();
    sites
}

/// The top-level COMMAND keywords a PROOF TERM may NEVER carry (`.design/verified/
/// proof-backends.md` REQ-6 / §1 / R-DEFER-9, the #252 BELT). After the #252 architectural
/// fix the proof term is the ONLY author-controlled text and is type-checked against the
/// FIXED generator-emitted goal, so a proof term cannot vacate that goal. This belt is a
/// cheap DEFENSE LAYER against an `… in`-style top-level command form smuggled into the
/// term (`open … in`, `set_option … in`, a `#…`-command): any of these as an exact token
/// (whitespace-independent, position-independent) → REJECT. A genuine term/tactic proof
/// never needs these — auxiliary lemmas inline as `have`/`let`/`suffices`. The `#` family
/// is handled separately (any `#`-prefixed token).
const PROOF_TERM_FORBIDDEN_COMMANDS: [&str; 16] = [
    "notation",
    "infix",
    "prefix",
    "postfix",
    "macro",
    "macro_rules",
    "syntax",
    "elab",
    "set_option",
    "attribute",
    "instance",
    "open",
    "export",
    "import",
    "namespace",
    "initialize",
];

/// THE #252 BELT (`.design/verified/proof-backends.md` REQ-6 / §1 / R-DEFER-9): scan the
/// extracted PROOF TERM and report a top-level command keyword if one appears as an exact
/// token in ANY position (whitespace-independent), or a `#…`-command token. The proof term
/// is type-checked against the fixed generator goal, so this is a defense layer — not the
/// primary soundness mechanism (that is the elimination of the helper surface, so author
/// content can no longer share the obligation's elaboration scope). It catches an `open …
/// in`-style command form a proof term might smuggle. Tokenizes on whitespace AND the Lean
/// term-separator punctuation (`(`, `)`, `;`, `,`) so `open Foo in` is caught even with no
/// surrounding spaces; an IDENTIFIER that merely CONTAINS a keyword (`openMyDef`,
/// `Nat.open`) is NOT a match (the token must equal the keyword exactly, and a `.`-qualified
/// tail is excluded). Deterministic (R-CODE-5); never a panic (R-CODE-2).
fn proof_term_command_token(proof_term: &str) -> Option<String> {
    for raw in proof_term.split(|c: char| {
        // Split on whitespace AND Lean term/tactic punctuation. `:` is included so a
        // priority-tagged command form (`notation:max`, `infix:50`, `set_option … :`) yields
        // the bare command keyword as a token; a type ascription `(x : T)` is unaffected
        // (its `have`/binder tokens are not command keywords).
        c.is_whitespace() || matches!(c, '(' | ')' | ';' | ',' | '{' | '}' | '[' | ']' | ':')
    }) {
        let tok = raw.trim();
        if tok.is_empty() {
            continue;
        }
        // A `#…`-command token (`#print`, `#check`, `#eval`, …) in the term.
        if tok.starts_with('#') {
            return Some(tok.to_string());
        }
        // An exact command keyword. A `.`-qualified token (`Nat.open`, `Foo.notation`) is a
        // member access, NOT a command — exclude it (the command keyword is never the tail
        // of a dotted projection).
        if tok.contains('.') {
            continue;
        }
        if PROOF_TERM_FORBIDDEN_COMMANDS.contains(&tok) {
            return Some(tok.to_string());
        }
    }
    None
}

/// The TRUST PROFILE of an INTERACTIVE Lean proof (`.design/verified/proof-backends.md`
/// REQ-7(ii) / OQ-4): {Lean kernel + 3 standard axioms, EXP} PLUS the human/agent
/// author as a reviewed-but-not-mechanized step (the interactive path adds the author,
/// OQ-4). Distinct from the AUTO profile so the auditor sees an interactive proof
/// carries the extra reviewed-author item.
#[must_use]
pub fn trust_profile_interactive() -> TrustProfile {
    TrustProfile {
        items: vec![
            "Lean kernel".to_string(),
            "propext".to_string(),
            "Classical.choice".to_string(),
            "Quot.sound".to_string(),
            "EXP (the exporter correspondence — arm-by-arm + the drift tripwire)".to_string(),
            "interactive proof author (reviewed, not mechanized — OQ-4)".to_string(),
        ],
    }
}

// ============================================================================
// REQ-9 — THE ENGINE-GENERIC MUTATION BATTERY, the LEAN PATH (`.design/verified/
// proof-backends.md` REQ-9 / §7, increment (iii), #247). When the discharging engine
// is Lean: mutants are attempted via the SAME engine path; KILL = `Refuted ∪
// Unknown-after-attempt`; the DENOMINATOR = attempted − proven-equivalent; a mutant
// OUTSIDE the engine's fragment = "untested against lean" — reported in the cert,
// NEVER counted killed. The Verus-path battery (`check::mutation_score`) is UNTOUCHED.
// ============================================================================

/// The outcome of attempting ONE mutant against the Lean engine (`.design/verified/
/// proof-backends.md` REQ-9). The engine-generic kill semantics: a mutant is KILLED if
/// the Lean engine `Refuted`s it OR returns `Unknown` AFTER attempting it (the mutant
/// was attempted and NOT proven — matching the shipped Verus `Counterexample ∪
/// Timeout` = killed); a mutant whose obligation the Lean fragment does NOT ADMIT is
/// `UntestedAgainstLean` (never counted killed, never a survivor); a `Proven` mutant
/// SURVIVED (the mutation did not break the contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeanMutantOutcome {
    /// The Lean engine PROVED the mutant — it SURVIVED (the contract is too weak,
    /// unless then proven equivalent to the real body — the #101 exclusion the caller
    /// applies).
    Survived,
    /// The mutant was attempted by the Lean engine and KILLED (`Refuted` — a witnessed
    /// countermodel — OR `Unknown` after an attempt). Maps onto the shipped
    /// `Counterexample ∪ Timeout` = killed.
    Killed,
    /// The mutant's obligation is OUTSIDE the Lean engine's fragment (it was NEVER
    /// attempted — e.g. a recursive-registry obligation only the tier-(c) interactive
    /// path admits, or an out-of-spine construct). "Untested against lean" — NEVER
    /// counted killed (that would inflate the ratio, §7 / R-DEFER-9) and NEVER a
    /// survivor.
    UntestedAgainstLean,
}

/// Classify a Lean-engine mutant [`Verdict`] under REQ-9's engine-generic kill
/// semantics (`.design/verified/proof-backends.md` REQ-9). `admitted` is whether the
/// Lean fragment ADMITTED the mutant's obligation (a per-mutant `fragment().admits`
/// check the caller runs BEFORE the discharge): a NON-admitted mutant is
/// `UntestedAgainstLean` REGARDLESS of the verdict (it was never genuinely attempted —
/// the Lean engine maps a refusal to `Unknown`, but that is a SKIP, not an
/// attempt-and-fail). An ADMITTED mutant maps `Proven → Survived`, `Refuted/Unknown →
/// Killed`. Determinism: a pure function of `admitted` + the verdict (R-CODE-5).
#[must_use]
pub fn lean_mutant_outcome(admitted: bool, verdict: &Verdict) -> LeanMutantOutcome {
    if !admitted {
        // The fragment did not admit the mutant — it was never attempted. "Untested
        // against lean" (REQ-9), distinct from `Unknown-after-attempt`.
        return LeanMutantOutcome::UntestedAgainstLean;
    }
    match verdict {
        Verdict::Proven(_) => LeanMutantOutcome::Survived,
        // Refuted (a witnessed countermodel) OR Unknown-after-attempt → KILLED (the
        // shipped `Counterexample ∪ Timeout` = killed, generalized).
        Verdict::Refuted(_) | Verdict::Unknown(_) => LeanMutantOutcome::Killed,
    }
}

/// The running tally of a LEAN-path mutation battery (`.design/verified/
/// proof-backends.md` REQ-9). Accumulates `killed` / `attempted` (= attempted MINUS
/// proven-equivalent, the SHIPPED `scored` denominator) / `equivalent` / `untested`
/// (the "untested against lean" count REPORTED in the cert, NEVER counted killed). The
/// kill ratio is `killed / attempted` — the untested count is OUTSIDE the denominator
/// so an untested mutant can NEVER inflate the ratio.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeanMutationTally {
    /// Mutants the Lean engine KILLED (`Refuted ∪ Unknown-after-attempt`).
    pub killed: usize,
    /// The DENOMINATOR: mutants ATTEMPTED (admitted by the fragment) MINUS the
    /// proven-equivalent (the #101 exclusion the caller applies).
    pub attempted: usize,
    /// Proven-equivalent mutants (excluded from BOTH `killed`-eligible survivors AND
    /// the `attempted` denominator — the SHIPPED #101 exclusion).
    pub equivalent: usize,
    /// "Untested against lean": mutants no Lean fragment admitted (NEVER counted
    /// killed; REPORTED so the auditor sees the coverage gap — §7 honesty).
    pub untested: usize,
}

impl LeanMutationTally {
    /// Record one classified mutant (`.design/verified/proof-backends.md` REQ-9).
    /// `proven_equivalent` is the SHIPPED #101 equivalence-probe result for a SURVIVED
    /// mutant (a §0.1 meta-query, OUTSIDE the Engine interface in v1 — a direct verus
    /// query the caller threads): an equivalent survivor is dropped from BOTH the
    /// survivor set AND the denominator (it is NOT a genuine survivor).
    pub fn record(&mut self, outcome: LeanMutantOutcome, proven_equivalent: bool) {
        match outcome {
            LeanMutantOutcome::UntestedAgainstLean => self.untested += 1,
            LeanMutantOutcome::Killed => {
                self.killed += 1;
                self.attempted += 1;
            }
            LeanMutantOutcome::Survived => {
                if proven_equivalent {
                    // The #101 exclusion: a proven-equivalent survivor is dropped from
                    // BOTH the survivor set AND the denominator (never a spurious
                    // survivor, never in the ratio).
                    self.equivalent += 1;
                } else {
                    // A genuinely-distinguishing survivor: in the denominator, NOT
                    // killed.
                    self.attempted += 1;
                }
            }
        }
    }

    /// The kill ratio `killed / attempted` (`.design/verified/proof-backends.md`
    /// REQ-9). The `untested` count is OUTSIDE the denominator, so an untested mutant
    /// can NEVER inflate the ratio. A `0` denominator (no attempted-and-non-equivalent
    /// mutant) is the SHIPPED `0/0` backstop → `0.0` (below any positive floor).
    #[must_use]
    pub fn kill_ratio(&self) -> f64 {
        if self.attempted == 0 {
            0.0
        } else {
            self.killed as f64 / self.attempted as f64
        }
    }

    /// A human qualifier line (`.design/verified/proof-backends.md` REQ-9 floor guard
    /// 1): the kill ratio WITH the untested-against-lean count beside it, so a `1/1`
    /// ratio with N untested mutants can NEVER read as a clean `1.00` without the
    /// untested count. Deterministic (R-CODE-5).
    #[must_use]
    pub fn qualifier(&self) -> String {
        format!(
            "{killed}/{attempted} killed against lean ({ratio:.2}); {untested} untested against \
             lean; {equivalent} proven-equivalent (excluded)",
            killed = self.killed,
            attempted = self.attempted,
            ratio = self.kill_ratio(),
            untested = self.untested,
            equivalent = self.equivalent,
        )
    }

    /// Does the Lean-path kill ratio MEET the mutation floor (`.design/verified/
    /// proof-backends.md` REQ-9/AC-7 — the floor GATES the Lean path, the #248 fix)?
    /// Mirrors the SHIPPED `mutation::MutationScore::meets_floor`: `kill_ratio() >=
    /// floor`. The `0/0` backstop (`kill_ratio() == 0.0`) is BELOW any positive floor,
    /// so an item that generated mutants but attempted NONE against Lean (all untested)
    /// does NOT meet the floor — never a vacuous pass (§7 / R-DEFER-9). Deterministic
    /// (R-CODE-5).
    #[must_use]
    pub fn meets_floor(&self, floor: f64) -> bool {
        self.kill_ratio() >= floor
    }

    /// The `"killed/attempted"` ratio string for the `WeakContract` reject cert's
    /// `contract_quality.mutants_killed` (the `qualifier`'s leading fraction, the
    /// Lean-path analogue of `MutationScore::mutants_killed_string`). Deterministic
    /// (R-CODE-5).
    #[must_use]
    pub fn mutants_killed_string(&self) -> String {
        format!("{}/{}", self.killed, self.attempted)
    }

    /// The survivor detail for the `WeakContract` reject cert on the Lean path
    /// (`.design/verified/proof-backends.md` REQ-9/AC-7). The Lean-only tally does NOT
    /// track an individual survivor body (the #101 equivalence probe is a §0.1 verus
    /// meta-query OUTSIDE this path, so survivors are reported as a COUNT, not a named
    /// mutant), so the detail HONESTLY states the survivor/untested counts that put the
    /// item below the floor. Deterministic (R-CODE-5).
    #[must_use]
    pub fn survivor_detail(&self) -> String {
        let survivors = self.attempted.saturating_sub(self.killed);
        format!(
            "{survivors} survivor(s) over {attempted} attempted against lean; {untested} untested \
             against lean (no engine fragment admitted them — NOT counted killed); denominator = \
             attempted (the #101 equivalence exclusion is OUTSIDE the Lean-only path)",
            attempted = self.attempted,
            untested = self.untested,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::VerusOutcome;
    use crate::lean_export::ExportTier;
    use crate::manifest::ObligationResult;
    use std::process::Command;

    fn a_key() -> CacheKey {
        CacheKey {
            engine: EngineName::Verus,
            content_address: "deadbeef".to_string(),
        }
    }

    // REQ-2(b): a `Proved` outcome maps to `Proven` with the discharged count.
    // Expected from the design's DISCHARGE map (`Proved` → `Proven`), R-CHAR-3.
    #[test]
    fn proved_maps_to_proven() {
        let v = VerusEngine.verdict_of(&VerusOutcome::Proved { verified: 3 }, a_key());
        match v {
            Verdict::Proven(e) => assert_eq!(e.verified, 3),
            other => panic!("expected Proven, got {other:?}"),
        }
    }

    // REQ-2(b): a `Timeout` outcome maps to `Unknown(VerusTimeout)` (degrade).
    // Expected from the design's DISCHARGE map (`Timeout` → `Unknown`), R-CHAR-3.
    #[test]
    fn timeout_maps_to_unknown() {
        let v = VerusEngine.verdict_of(
            &VerusOutcome::Timeout {
                profile: crate::profile::SolverProfile {
                    total_instantiations: 0,
                    quantifiers: Vec::new(),
                },
                detail: "budget exhausted".to_string(),
            },
            a_key(),
        );
        assert!(
            matches!(v, Verdict::Unknown(Reason::VerusTimeout(_))),
            "a timeout is Unknown, never Refuted (REQ-3): {v:?}"
        );
    }

    // REQ-3.1 — THE FAST-UNKNOWN REMAP: a witness-LESS `Counterexample` (no parsed
    // `--> span` — the synthetic fallback) maps to `Unknown(IncompleteUnknown)`,
    // NOT `Refuted`. Expected from REQ-3.1's decision (R-CHAR-3), the SOLE
    // behavioral delta.
    #[test]
    fn witnessless_counterexample_remaps_to_unknown() {
        let witnessless = vec![ObligationResult::failed(
            "verus reported obligation failure",
            None, // NO witnessing input — the fast-`unknown` edge.
            Some("error: unknown".to_string()),
        )];
        let v = VerusEngine.verdict_of(
            &VerusOutcome::Counterexample {
                obligations: witnessless,
            },
            a_key(),
        );
        assert!(
            matches!(v, Verdict::Unknown(Reason::IncompleteUnknown(_))),
            "a witness-LESS failure is Unknown, never Refuted (REQ-3.1): {v:?}"
        );
    }

    // REQ-3.1 / REQ-3 anti-cheat: a WITNESSED `Counterexample` (≥1 parsed `-->
    // span`) STAYS `Refuted` (hard-fail, never degrades). Expected from REQ-3.1
    // ("a WITNESSED countermodel stays Refuted"), R-CHAR-3.
    #[test]
    fn witnessed_counterexample_stays_refuted() {
        let witnessed = vec![ObligationResult::failed(
            "postcondition not satisfied",
            Some("x.rs:5:13".to_string()), // a witnessing input (the span).
            Some("error: postcondition not satisfied".to_string()),
        )];
        let v = VerusEngine.verdict_of(
            &VerusOutcome::Counterexample {
                obligations: witnessed.clone(),
            },
            a_key(),
        );
        match v {
            Verdict::Refuted(cx) => assert_eq!(cx.obligations, witnessed),
            other => panic!("a witnessed countermodel must stay Refuted: {other:?}"),
        }
    }

    // REQ-3.1: the NARROW incompleteness discriminator. ONLY a span-less failure
    // carrying the SMT-`unknown` signature (no frontend error) is the fast-`unknown`
    // that degrades; a witnessed countermodel, a frontend type error, and a bare
    // failure all stay `Refuted`. This is what keeps the corpus byte-identical.
    #[test]
    fn incompleteness_discriminator_is_narrow() {
        // (1) A parsed `--> span` = a WITNESSED countermodel → NOT remapped.
        let with_loc = vec![ObligationResult::failed(
            "postcondition not satisfied",
            Some("a:1:1".to_string()),
            Some("error: postcondition not satisfied".to_string()),
        )];
        assert!(!counterexample_is_incompleteness_unknown(&with_loc));
        // (2) A FRONTEND type error (`error[E0308]`) = a genuine rejection → NOT
        // remapped (the provenance `careless_query` E0308 stays L0). Corpus-pinned.
        let e0308 = vec![ObligationResult::failed(
            "mismatched types",
            None,
            Some("error[E0308]: mismatched types".to_string()),
        )];
        assert!(
            !counterexample_is_incompleteness_unknown(&e0308),
            "an E0308 type error is a genuine rejection, NOT an SMT `unknown` (corpus L0)"
        );
        // (3) The genuine SMT-`unknown` signature → REMAPPED (degrade, REQ-3.1).
        let unknown = vec![ObligationResult::failed(
            "verus reported obligation failure",
            None,
            Some("error: Z3 returned unknown".to_string()),
        )];
        assert!(counterexample_is_incompleteness_unknown(&unknown));
        // (4) A bare span-less failure with NO `unknown` signal → NOT remapped
        // (conservative: keep the SHIPPED hard-fail).
        let bare = vec![ObligationResult::failed("e", None, Some("d".to_string()))];
        assert!(!counterexample_is_incompleteness_unknown(&bare));
        // (5) An EMPTY list → NOT remapped.
        assert!(!counterexample_is_incompleteness_unknown(&[]));
    }

    // REQ-3.1 / cert-oracle: a frontend TYPE error (`error[E0308]`, span-less)
    // stays `Refuted` → HARD FAIL — the provenance `careless_query` L0 the corpus
    // pins (R-CHAR-3, the cert-oracle-unperturbed AC).
    #[test]
    fn type_error_counterexample_stays_refuted() {
        let e0308 = vec![ObligationResult::failed(
            "mismatched types",
            None,
            Some("error[E0308]: mismatched types: expected Sql, found Tainted".to_string()),
        )];
        let v = VerusEngine.verdict_of(
            &VerusOutcome::Counterexample { obligations: e0308 },
            a_key(),
        );
        assert!(
            matches!(v, Verdict::Refuted(_)),
            "a type-error rejection stays Refuted (hard-fail → L0), NOT degraded: {v:?}"
        );
    }

    // REQ-3: the verdict→ladder map. `Proven` → certify L3; `Unknown` → degrade
    // (Timeout trigger); `Refuted` → hard-fail (Counterexample). Expected from the
    // design's REQ-3 discipline (R-CHAR-3), generalized off `ladder_action_l3`.
    #[test]
    fn verdict_ladder_action_follows_req3() {
        use crate::degrade::{ladder_action_l3, LadderAction};
        use crate::manifest::{Certificate, Level};
        let proved = Certificate::new("f", Level::L3, vec!["pure".to_string()], 0, vec![]);
        let cx = Certificate::new("f", Level::L0, vec!["pure".to_string()], 0, vec![]);

        let p = verdict_ladder_action(
            &Verdict::Proven(Evidence {
                verified: 1,
                key: a_key(),
            }),
            ObligationRole::Certification,
            proved.clone(),
            cx.clone(),
        );
        assert_eq!(ladder_action_l3(&p), LadderAction::CertifyL3);

        let u = verdict_ladder_action(
            &Verdict::Unknown(Reason::IncompleteUnknown("d".to_string())),
            ObligationRole::Certification,
            proved.clone(),
            cx.clone(),
        );
        assert_eq!(
            ladder_action_l3(&u),
            LadderAction::AttemptL2,
            "an Unknown (incl. the fast-unknown remap) DEGRADES, never hard-fails (REQ-3)"
        );

        let r = verdict_ladder_action(
            &Verdict::Refuted(Counterexample {
                obligations: vec![ObligationResult::failed(
                    "e",
                    Some("a:1:1".to_string()),
                    None,
                )],
            }),
            ObligationRole::Certification,
            proved,
            cx,
        );
        assert_eq!(
            ladder_action_l3(&r),
            LadderAction::HardFail,
            "a witnessed Refuted HARD-FAILS, never degrades (REQ-3 anti-cheat)"
        );
    }

    // REQ-2(a)/(c)/(d): the Verus engine fills all four slots non-vacuously (AC-2).
    #[test]
    fn verus_engine_fills_four_slots() {
        let e = VerusEngine;
        assert_eq!(e.name(), EngineName::Verus);
        assert!(
            e.fragment().admits_all_classes,
            "Verus admits the whole subset"
        );
        let tp = e.trust_profile();
        assert!(
            tp.items.iter().any(|i| i.contains("Z3"))
                && tp.items.iter().any(|i| i.contains("Verus VC-gen")),
            "the trust profile enumerates {{Z3, Verus VC-gen}} + the TV theorem"
        );
        assert_eq!(
            default_engines(),
            vec![EngineName::Verus],
            "REQ-8: Verus first"
        );
    }

    // ============================================================================
    // The LIVE Lean engine #2 tests (REQ-6/REQ-7; the #240 chain). These construct a
    // `LeanEngine` directly (the design's "constructed directly by tests") and invoke
    // lake LIVE (lake present at ~/.elan). A LIVE test gates on lake presence so the
    // suite is green WITHOUT lake (the verdict there is `Unknown` — never a false
    // `Proven`/`Refuted`). Every expected verdict is hand-derived (R-CHAR-3): a
    // CORRECT contract kernel-accepts (Proven), a WRONG one fails the tactic
    // (Unknown, NEVER Refuted), the omitted/divergent shapes REFUSE the export (the
    // Pin E/F Rust mirror), an out-of-fragment item is SKIPPED. The helpers use
    // `assert!`/`matches!` (NOT unwrap/expect/panic) so the anti-pattern gate is
    // clean on the Edit (R-APG-2).
    // ============================================================================

    fn lean_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("lean")
    }

    fn lake_present() -> bool {
        if let Some(home) = std::env::var_os("HOME") {
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

    fn parse_program(src: &str) -> Program {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        parsed.program
    }

    // Build a CONTRACT obligation for a named fn (asserting it exists + is a fn, no
    // unwrap/panic). A non-fn / absent item makes the `matches!` assert fail; the
    // default obligation is returned only on that already-failed path.
    fn fn_obligation(program: &Program, name: &str, called: Vec<String>) -> Obligation {
        let item = crate::lean_export::find_item(program, name);
        assert!(
            matches!(item, Some(thermite_syntax::Item::Fn(_))),
            "item `{name}` must be present and a fn, got {item:?}"
        );
        if let Some(thermite_syntax::Item::Fn(f)) = item {
            Obligation::contract_for_fn(f, called)
        } else {
            default_obligation()
        }
    }

    // A default obligation builder (reached only after the `matches!` assert above
    // has already failed); keeps `fn_obligation` total without an unwrap/panic.
    fn default_obligation() -> Obligation {
        Obligation {
            item: String::new(),
            class: crate::obligation::ObligationClass::Contract,
            role: ObligationRole::Certification,
            ast_slice: crate::obligation::AstSlice::Block(Box::new(thermite_syntax::Block {
                stmts: Vec::new(),
                tail: None,
            })),
            env: crate::obligation::ObligationEnv::default(),
        }
    }

    // (1) REQ-6/REQ-7: a hand-authored pure-contract SCALAR item kernel-accepts LIVE
    // (Proven). `add` returns `a as u64 + b as u64` and `ens result == a as u64 + b
    // as u64` — the body IS the ens RHS, so after binding `result` to the body's
    // stabilized value the goal is true; the fuel-free tier-(a) battery kernel-checks
    // it. Expected from §6.1(a) (R-CHAR-3): a CORRECT contract is Proven.
    #[test]
    fn live_scalar_correct_contract_is_proven() {
        if !lake_present() {
            eprintln!("SKIP: lake not present — live Lean Proven test not run.");
            return;
        }
        let src = "fn add(a: u32, b: u32) -> u64 req true \
                   ens result == a as u64 + b as u64 fx pure { a as u64 + b as u64 }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "add", vec![]);
        let engine = LeanEngine::new(p.clone(), lean_root());
        let v = engine.discharge(&o);
        assert!(
            matches!(v, Verdict::Proven(_)),
            "a CORRECT scalar contract must be Proven LIVE: {v:?}"
        );
    }

    // (2) REQ-7 §6.1(b): a TIER-(b) item (a non-recursive spec-fn in the ens) is
    // STATICALLY UNFOLDED to a fuel-free goal and kernel-accepts LIVE (Proven). `g`
    // returns `x + x` and `ens result as int == dbl(x as int)` where `spec fn dbl(x)
    // = x + x` — the unfolded ens is `result as int == (x as int) + (x as int)`,
    // true at `result = x + x`. Expected from §6.1(b) (R-CHAR-3).
    #[test]
    fn live_tier_b_nonrecursive_spec_fn_is_proven() {
        if !lake_present() {
            eprintln!("SKIP: lake not present — live tier-(b) Proven test not run.");
            return;
        }
        let src = "spec fn dbl(x: int) -> int dec x { x + x } \
                   fn g(x: u32) -> u32 req x < 100 ens result as int == dbl(x as int) \
                   fx pure { x + x }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "g", vec!["dbl".to_string()]);
        let engine = LeanEngine::new(p.clone(), lean_root());
        // Sanity: the exporter classifies this tier (b) (static-unfold auto).
        let item = crate::lean_export::find_item(&p, "g");
        assert!(item.is_some(), "g present");
        if let Some(item) = item {
            let exported = export_item(&o, &p, item);
            assert!(exported.is_ok(), "g must export: {exported:?}");
            if let Ok(exported) = exported {
                assert_eq!(exported.tier, ExportTier::StaticUnfoldAuto);
                assert_eq!(exported.registry_names, vec!["dbl".to_string()]);
            }
        }
        let v = engine.discharge(&o);
        assert!(
            matches!(v, Verdict::Proven(_)),
            "a tier-(b) item must be Proven LIVE via static unfold: {v:?}"
        );
    }

    // (3) REQ-7 / REQ-3 anti-cheat: a WRONG contract (`ens result == 0` for a body
    // that returns `a`) makes the auto battery FAIL → `Unknown`, NEVER `Refuted` (a
    // Lean tactic failure is not a witnessed countermodel) and NEVER `Proven`.
    // Expected from §6.1 + REQ-3 (R-CHAR-3).
    #[test]
    fn live_wrong_contract_is_unknown_never_refuted() {
        if !lake_present() {
            eprintln!("SKIP: lake not present — live wrong-contract test not run.");
            return;
        }
        let src = "fn wrong(a: u32, b: u32) -> u32 req true ens result == 0 fx pure { a }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "wrong", vec![]);
        let engine = LeanEngine::new(p.clone(), lean_root());
        let v = engine.discharge(&o);
        assert!(
            !matches!(v, Verdict::Refuted(_)),
            "a tactic FAILURE is Unknown, NEVER Refuted (REQ-3 anti-cheat): {v:?}"
        );
        assert!(
            matches!(v, Verdict::Unknown(_)),
            "a WRONG contract must be Unknown (not Proven): {v:?}"
        );
    }

    // (4) REQ-6 §4 HARD GATE — the Pin E/F Rust mirror: an OMITTED-registry obligation
    // (the ens calls `spec_sum` but the obligation's closure does NOT list it) REFUSES
    // the export → `Unknown` (a skip), NEVER a bottom-poisoned `Proven`. The Rust
    // mirror of the divergent/omitted Lean pins. Expected from §4 mechanism 1
    // (R-CHAR-3). NO lake needed.
    #[test]
    fn omitted_registry_obligation_refuses_export() {
        let src = "spec fn spec_sum(xs: &[u32]) -> u64 dec xs.len() { 0 } \
                   fn f(xs: &[u32]) -> u64 req true ens result == spec_sum(xs) fx pure { 0 }";
        let p = parse_program(src);
        // The obligation's closure OMITS `spec_sum` (the bug the gate must catch).
        let o = fn_obligation(&p, "f", vec![]);
        if let Some(item) = crate::lean_export::find_item(&p, "f") {
            let r = export_item(&o, &p, item);
            assert!(
                matches!(&r, Err(ExportRefusal::IncompleteRegistry(_))),
                "an omitted-registry obligation must REFUSE the export: {r:?}"
            );
            if let Err(ExportRefusal::IncompleteRegistry(names)) = &r {
                assert!(
                    names.contains(&"spec_sum".to_string()),
                    "the omitted spec-fn is named in the refusal: {names:?}"
                );
            }
        }
        // The engine maps the refusal to Unknown (a skip), NEVER Proven/Refuted.
        let engine = LeanEngine::new(p, lean_root());
        let v = engine.discharge(&o);
        assert!(
            matches!(v, Verdict::Unknown(_)),
            "a refused export is an Unknown skip, never a verdict: {v:?}"
        );
    }

    // ========================================================================
    // THE EXEC-BODY BRIDGE live + refusal tests (§4.1 / REQ-10, increment (iv-b),
    // blocker #253). A straight-line-body item exports the HYPOTHESIZE CONTRACT
    // theorem + the conjoined OVERFLOW theorem over `bodyDenote`/`stateOf`; a
    // bool-result item routes through `bindBool`; an always-overflow body's vacuous
    // CONTRACT is blocked by the failing OVERFLOW conjunct; while/optres refuse.
    // Verdicts hand-derived (R-CHAR-3) from §4.1.5 + PinExecOverflowVacuity.
    // ========================================================================

    // (7) REQ-10.3/10.4 — a STRAIGHT-LINE-BODY int item is Proven LIVE (incl. the
    // OVERFLOW conjunct). `id2`'s body `{ let y = x; y }` threads `y ↦ x`, tail `y`,
    // so the result `r = x` and `ens result == x` holds at `bindResult`; the body has
    // no overflow site so the OVERFLOW conjunct discharges. Both theorems kernel-accept
    // in the SAME emitted file. Expected from §4.1.5 (R-CHAR-3).
    #[test]
    fn live_straight_line_body_is_proven() {
        if !lake_present() {
            eprintln!("SKIP: lake not present — live straight-line-body test not run.");
            return;
        }
        let src = "fn id2(x: u64) -> u64 req true ens result == x fx pure { let y = x; y }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "id2", vec![]);
        // Sanity: the export emits BOTH the CONTRACT theorem and the conjoined OVERFLOW
        // theorem in one file (the §4.1.5 conjunction rule).
        if let Some(item) = crate::lean_export::find_item(&p, "id2") {
            let exported = export_item(&o, &p, item);
            assert!(
                exported.is_ok(),
                "id2 must export as a body item: {exported:?}"
            );
            if let Ok(e) = &exported {
                assert!(
                    e.source.contains("def stateOf"),
                    "emits stateOf: {}",
                    e.source
                );
                assert!(
                    e.source.contains("def body_block"),
                    "emits body_block: {}",
                    e.source
                );
                assert!(
                    e.source.contains("thermite_obligation_id2_overflow"),
                    "emits the conjoined OVERFLOW theorem: {}",
                    e.source
                );
                assert!(
                    e.source.contains("bodyConverges") && e.source.contains("bindResult"),
                    "emits the HYPOTHESIZE form via bodyConverges + bindResult"
                );
            }
        }
        let engine = LeanEngine::new(p.clone(), lean_root());
        let v = engine.discharge(&o);
        assert!(
            matches!(v, Verdict::Proven(_)),
            "a correct straight-line-body item must be Proven LIVE (incl. the OVERFLOW \
             conjunct): {v:?}"
        );
    }

    // (8) REQ-10.2 — a BOOL-RESULT straight-line item is Proven LIVE via the `bindBool`
    // bridge (the iv-a spine layer end-to-end). `t`'s body `{ true }` and `ens result
    // == true`: the result `b = true` binds via `Env.bindBool`, read as `Expr.boolVar
    // "result"`, so `ens` holds. Expected from §4.1.2 (R-CHAR-3).
    #[test]
    fn live_bool_result_body_is_proven_via_bindbool() {
        if !lake_present() {
            eprintln!("SKIP: lake not present — live bool-result test not run.");
            return;
        }
        let src = "fn t(x: u32) -> bool req true ens result == true fx pure { true }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "t", vec![]);
        if let Some(item) = crate::lean_export::find_item(&p, "t") {
            let exported = export_item(&o, &p, item);
            assert!(
                exported.is_ok(),
                "bool-result item must export: {exported:?}"
            );
            if let Ok(e) = &exported {
                assert!(
                    e.source.contains("Thermite.Expr.boolVar \"result\""),
                    "the bool result reads via boolVar: {}",
                    e.source
                );
                assert!(
                    e.source.contains("Thermite.Exec.ExecVal.bool b"),
                    "the bool antecedent binds via .bool b"
                );
            }
        }
        let engine = LeanEngine::new(p.clone(), lean_root());
        let v = engine.discharge(&o);
        assert!(
            matches!(v, Verdict::Proven(_)),
            "a correct bool-result item must be Proven LIVE via bindBool: {v:?}"
        );
    }

    // (9) REQ-10.4 / the conjunction rule — an ALWAYS-OVERFLOW body with a vacuous-looking
    // ens does NOT certify: the OVERFLOW conjunct FAILS so the LIVE verdict is NOT Proven
    // (the conjunction working end-to-end — the PinExecOverflowVacuity Rust mirror). `ovf`'s
    // body `{ let a = m + m; a }` overflows `u64` when `m` is at the rim; the CONTRACT
    // theorem may be vacuously provable but the conjoined OVERFLOW theorem `bodyDenote
    // |>.isSome` is FALSE under no precondition bounding `m` away from the rim, so the
    // single emitted file does NOT kernel-accept (the OVERFLOW theorem fails). Expected
    // from §4.1.5 + PinExecOverflowVacuity (R-CHAR-3).
    #[test]
    fn live_always_overflow_body_is_not_proven() {
        if !lake_present() {
            eprintln!("SKIP: lake not present — live overflow-vacuity test not run.");
            return;
        }
        // No req bounds `m`, so `m + m` can overflow `u64` — the OVERFLOW conjunct fails.
        let src = "fn ovf(m: u64) -> u64 req true ens result < result fx pure { let a = m + m; a }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "ovf", vec![]);
        let engine = LeanEngine::new(p.clone(), lean_root());
        let v = engine.discharge(&o);
        assert!(
            !matches!(v, Verdict::Proven(_)),
            "an always-overflow body must NOT be Proven — the OVERFLOW conjunct fails \
             (the conjunction rule, PinExecOverflowVacuity): {v:?}"
        );
        assert!(
            !matches!(v, Verdict::Refuted(_)),
            "a failed tactic is Unknown, NEVER Refuted (REQ-3 anti-cheat): {v:?}"
        );
    }

    // (10) REQ-10.6 — a WHILE-body item is REFUSED structurally (§4.1.7: S_B has no loop
    // form). The export returns `ExportRefusal::LoopBody`; the engine maps it to Unknown
    // (an honest skip), NEVER a verdict. Expected from §4.1.7 (R-CHAR-3). NO lake needed.
    #[test]
    fn while_body_item_refuses_export() {
        let src = "fn count(n: u64) -> u64 req true ens result == n fx pure \
                   { let mut i = 0; while i < n inv i <= n dec n - i { i = i + 1; } i }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "count", vec![]);
        if let Some(item) = crate::lean_export::find_item(&p, "count") {
            let r = export_item(&o, &p, item);
            assert!(
                matches!(&r, Err(ExportRefusal::LoopBody(_))),
                "a while-body item must REFUSE structurally (LoopBody): {r:?}"
            );
        }
        let engine = LeanEngine::new(p, lean_root());
        let v = engine.discharge(&o);
        assert!(
            matches!(v, Verdict::Unknown(_)),
            "a refused loop body is an Unknown skip, never a verdict: {v:?}"
        );
    }

    // (11) REQ-10 / §4.1.3 — an OPTION/RESULT-RESULT item is REFUSED (#254: `ExecVal`
    // has no optres variant). The export returns `ExportRefusal::OptResResult`; the
    // engine maps it to Unknown. Expected from §4.1.3 (R-CHAR-3). NO lake needed.
    #[test]
    fn optres_result_item_refuses_export() {
        let src = "fn maybe(x: u32) -> Option<u32> req true ens true fx pure { let y = x; y }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "maybe", vec![]);
        if let Some(item) = crate::lean_export::find_item(&p, "maybe") {
            let r = export_item(&o, &p, item);
            assert!(
                matches!(&r, Err(ExportRefusal::OptResResult(_))),
                "an Option-result item must REFUSE (OptResResult, #254): {r:?}"
            );
        }
        let engine = LeanEngine::new(p, lean_root());
        let v = engine.discharge(&o);
        assert!(
            matches!(v, Verdict::Unknown(_)),
            "a refused optres body is an Unknown skip, never a verdict: {v:?}"
        );
    }

    // (5) REQ-6 §4 SCOPE: an OUT-of-fragment item (an out-of-spine struct-field
    // access in the ens, on an int-RESULT fn so the #244 result-sort gate does NOT
    // pre-empt it) is SKIPPED (the fragment rejects it) → the export REFUSES and the
    // engine returns `Unknown`. Expected from the §4 OUT-of-spine refusal rule
    // (R-CHAR-3). NO lake.
    #[test]
    fn out_of_fragment_item_is_skipped() {
        let src = "struct P { x: u32 } \
                   fn pick(p: P) -> u32 req true \
                   ens result == p.x fx pure { 0 }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "pick", vec![]);
        if let Some(item) = crate::lean_export::find_item(&p, "pick") {
            let r = export_item(&o, &p, item);
            assert!(
                matches!(r, Err(ExportRefusal::OutOfFragment(_))),
                "a struct-field ens is out-of-fragment: {r:?}"
            );
        }
        let engine = LeanEngine::new(p, lean_root());
        let v = engine.discharge(&o);
        assert!(
            matches!(v, Verdict::Unknown(_)),
            "an out-of-fragment item is an Unknown skip: {v:?}"
        );
    }

    // REQ-7 §6.1(c): a RECURSIVE-registry item is tier (c) (interactive) — the engine
    // returns `Unknown` WITHOUT invoking lake (the `∃N∀fuel` form needs an authored
    // induction). The exported FILE is still produced (for increment-(iii)), marked
    // interactive. Expected from §6.1(c) (R-CHAR-3).
    #[test]
    fn recursive_registry_is_interactive_unknown() {
        let src = "spec fn r(x: int) -> int dec x { r(x) } \
                   fn f(x: u32) -> u32 req true ens result as int == r(x as int) fx pure { x }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "f", vec!["r".to_string()]);
        if let Some(item) = crate::lean_export::find_item(&p, "f") {
            let exported = export_item(&o, &p, item);
            assert!(
                exported.is_ok(),
                "a recursive item still EXPORTS a file (for increment-(iii)): {exported:?}"
            );
            if let Ok(exported) = exported {
                assert_eq!(exported.tier, ExportTier::RecursiveInteractive);
                assert!(
                    exported.source.contains("Thermite.stabilizes"),
                    "tier (c) emits the §4 ∃N∀fuel stabilized form"
                );
            }
        }
        let engine = LeanEngine::new(p, lean_root());
        let v = engine.discharge(&o);
        assert!(
            matches!(v, Verdict::Unknown(_)),
            "a recursive (tier-c) item is INTERACTIVE Unknown: {v:?}"
        );
    }

    // ========================================================================
    // REQ-7(ii) — THE INTERACTIVE PROOF ARTIFACT (skeleton emit + staleness gate +
    // sorry detection + replay), increment (iii), #247. The replay machinery is
    // exercised on a controlled proof file in a scratch `<file>.lean-proofs/` dir.
    // The helpers below avoid unwrap/expect/panic (the anti-pattern gate is clean on
    // an Edit, R-APG-2): IO failures surface via `assert!` on the `Result`.
    // ========================================================================

    // Create a dir (assert success, never unwrap). Returns whether it exists after.
    fn ensure_dir(p: &std::path::Path) -> bool {
        let _ = std::fs::remove_dir_all(p);
        std::fs::create_dir_all(p).is_ok()
    }

    // Write a file's bytes (assert success, never unwrap).
    fn write_file(p: &std::path::Path, content: &str) -> bool {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(p, content).is_ok()
    }

    // REQ-7(ii) — SKELETON EMITTED WHEN ABSENT: the first `replay_interactive` call on
    // an item with NO artifact EMITS the skeleton (the evidence-key header + the
    // exported source) and returns `Unknown` ("skeleton emitted"), NEVER `Proven`.
    // Expected from REQ-7(ii) (R-CHAR-3). NO lake needed.
    #[test]
    fn interactive_skeleton_emitted_when_absent() {
        let dir = std::env::temp_dir().join(format!("forge_it_emit_{}", std::process::id()));
        assert!(ensure_dir(&dir), "scratch dir creatable");
        let file = dir.join("add.th");
        let src = "fn add(a: u32, b: u32) -> u64 req true \
                   ens result == a as u64 + b as u64 fx pure { a as u64 + b as u64 }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "add", vec![]);
        let engine = LeanEngine::new(p, lean_root());

        let v = engine.replay_interactive(&file, &o);
        let artifact = interactive_proof_path(&file, "add");
        assert!(
            matches!(v, Verdict::Unknown(_)),
            "an ABSENT artifact emits the skeleton + returns Unknown (never Proven): {v:?}"
        );
        assert!(
            artifact.exists(),
            "the skeleton file is written beside the source"
        );
        let emitted = std::fs::read_to_string(&artifact).unwrap_or_default();
        assert!(
            emitted.starts_with(INTERACTIVE_EVIDENCE_KEY_MARKER),
            "the skeleton carries the evidence-key header (the staleness gate): {emitted}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // REQ-7(ii) — A STALE HASH → Unknown("stale proof — re-derive"): an artifact whose
    // header carries a DIFFERENT evidence key than the current obligation is STALE and
    // is NEVER silently reused. Expected from REQ-7(ii) (R-CHAR-3). NO lake needed (the
    // staleness gate short-circuits BEFORE the replay).
    #[test]
    fn interactive_stale_hash_is_unknown_never_reused() {
        let dir = std::env::temp_dir().join(format!("forge_it_stale_{}", std::process::id()));
        assert!(ensure_dir(&dir), "scratch dir creatable");
        let file = dir.join("add.th");
        let src = "fn add(a: u32, b: u32) -> u64 req true \
                   ens result == a as u64 + b as u64 fx pure { a as u64 + b as u64 }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "add", vec![]);
        let engine = LeanEngine::new(p, lean_root());

        // Author an artifact with a WRONG (stale) evidence-key header.
        let artifact = interactive_proof_path(&file, "add");
        assert!(
            write_file(
                &artifact,
                &format!(
                    "{INTERACTIVE_EVIDENCE_KEY_MARKER}deadbeefstalekey\n\
                     theorem t : True := by trivial\n"
                ),
            ),
            "stale artifact writable"
        );

        let v = engine.replay_interactive(&file, &o);
        let _ = std::fs::remove_dir_all(&dir);
        let stale_detail = match &v {
            Verdict::Unknown(Reason::IncompleteUnknown(d)) => Some(d.clone()),
            _ => None,
        };
        assert!(
            stale_detail
                .as_deref()
                .is_some_and(|d| d.contains("stale proof")),
            "a stale-key artifact → Unknown(\"stale proof — re-derive\"), got {v:?}"
        );
    }

    // REQ-7(ii) — A SORRY-CARRYING FILE → Unknown (NEVER Proven), even though lake
    // exits 0 on a `sorry`. The artifact has the CORRECT (fresh) key + a `sorry` body;
    // the explicit sorry detection (`proof_has_sorry`) blocks the `Proven`. Expected
    // from REQ-7(ii) (R-CHAR-3). LIVE (gated on lake).
    #[test]
    fn interactive_sorry_file_is_unknown_never_proven() {
        if !lake_present() {
            eprintln!("SKIP: lake not present — the interactive sorry-replay test is not run.");
            return;
        }
        let dir = std::env::temp_dir().join(format!("forge_it_sorry_{}", std::process::id()));
        assert!(ensure_dir(&dir), "scratch dir creatable");
        let file = dir.join("add.th");
        let src = "fn add(a: u32, b: u32) -> u64 req true \
                   ens result == a as u64 + b as u64 fx pure { a as u64 + b as u64 }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "add", vec![]);
        let engine = LeanEngine::new(p, lean_root());

        // Author an artifact with the CORRECT (fresh) key but a `sorry`-carrying proof
        // of the theorem name the replay's `#print axioms` probes.
        let key = engine.evidence_key(&o);
        let artifact = interactive_proof_path(&file, "add");
        assert!(
            write_file(
                &artifact,
                &format!(
                    "{INTERACTIVE_EVIDENCE_KEY_MARKER}{}\nimport Thermite.Stabilize\n\
                     theorem thermite_obligation_add : True := by sorry\n",
                    key.content_address
                ),
            ),
            "sorry artifact writable"
        );

        let v = engine.replay_interactive(&file, &o);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            matches!(v, Verdict::Unknown(_)),
            "a sorry-carrying proof is NEVER Proven (REQ-7(ii)), even though lake exits 0: {v:?}"
        );
    }

    // REQ-7(ii) — A FILLED, VALID, SORRY-FREE PROOF REPLAYS Proven. Driving
    // `replay_interactive` with an AUTO-tier obligation emits a COMPLETE proof (the
    // `by first | decide | omega | …` battery — NOT a `sorry`), so the SECOND call
    // (artifact PRESENT, key fresh) REPLAYS it: lake kernel-accepts the sorry-free
    // proof → `Proven`. This faithfully exercises the replay machinery on a genuine
    // kernel-accepted proof (an auto-tier body IS a complete authored proof). Expected
    // from REQ-7(ii) (R-CHAR-3). LIVE (gated on lake).
    #[test]
    fn interactive_filled_valid_proof_replays_proven() {
        if !lake_present() {
            eprintln!("SKIP: lake not present — the interactive valid-replay test is not run.");
            return;
        }
        let dir = std::env::temp_dir().join(format!("forge_it_valid_{}", std::process::id()));
        assert!(ensure_dir(&dir), "scratch dir creatable");
        let file = dir.join("add.th");
        let src = "fn add(a: u32, b: u32) -> u64 req true \
                   ens result == a as u64 + b as u64 fx pure { a as u64 + b as u64 }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "add", vec![]);
        let engine = LeanEngine::new(p, lean_root());

        // First call: ABSENT → EMIT the complete (auto-tier) proof + header → Unknown.
        let first = engine.replay_interactive(&file, &o);
        assert!(
            matches!(first, Verdict::Unknown(_)),
            "the first call emits the artifact (Unknown): {first:?}"
        );
        let artifact = interactive_proof_path(&file, "add");
        assert!(artifact.exists(), "the artifact was emitted");
        let emitted = std::fs::read_to_string(&artifact).unwrap_or_default();
        assert!(
            !proof_has_sorry(&emitted, ""),
            "an auto-tier emitted proof is a COMPLETE sorry-free proof: {emitted}"
        );

        // Second call: PRESENT + fresh key + sorry-free + lake-accepted → Proven.
        let second = engine.replay_interactive(&file, &o);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            matches!(second, Verdict::Proven(_)),
            "a PRESENT, fresh-key, sorry-free, kernel-accepted proof REPLAYS Proven \
             (REQ-7(ii)): {second:?}"
        );
    }

    // REQ-6 STATEMENT BINDING (the #248 fix): a proof file with the CORRECT (fresh)
    // evidence key but proving a DIFFERENT theorem statement (the trivial proposition,
    // not the obligation) is Unknown("statement mismatch"), NEVER Proven — the file must
    // PROVE THE OBLIGATION. The staleness gate passes (fresh key); the statement-binding
    // gate catches it BEFORE the (skipped) lake replay, so NO lake is needed. R-DEFER-9.
    #[test]
    fn interactive_statement_mismatch_is_unknown_never_proven() {
        let dir = std::env::temp_dir().join(format!("forge_it_stmtmm_{}", std::process::id()));
        assert!(ensure_dir(&dir), "scratch dir creatable");
        let file = dir.join("add.th");
        let src = "fn add(a: u32, b: u32) -> u64 req true \
                   ens result == a as u64 + b as u64 fx pure { a as u64 + b as u64 }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "add", vec![]);
        let engine = LeanEngine::new(p, lean_root());

        // Author an artifact with the CORRECT (fresh) key but the trivial proposition —
        // a proof of the WRONG statement, named like the obligation theorem.
        let key = engine.evidence_key(&o);
        let artifact = interactive_proof_path(&file, "add");
        assert!(
            write_file(
                &artifact,
                &format!(
                    "{INTERACTIVE_EVIDENCE_KEY_MARKER}{}\nimport Thermite.Stabilize\n\
                     theorem thermite_obligation_add : True := by trivial\n",
                    key.content_address
                ),
            ),
            "statement-mismatch artifact writable"
        );

        let v = engine.replay_interactive(&file, &o);
        let _ = std::fs::remove_dir_all(&dir);
        let detail = match &v {
            Verdict::Unknown(Reason::IncompleteUnknown(d)) => Some(d.clone()),
            _ => None,
        };
        assert!(
            detail
                .as_deref()
                .is_some_and(|d| d.contains("statement mismatch")),
            "a proof of a DIFFERENT statement (the obligation must be proven, not the \
             trivial proposition) → Unknown(\"statement mismatch\"), NEVER Proven (REQ-6 / \
             R-DEFER-9): got {v:?}"
        );
    }

    // REQ-6 / R-DEFER-9 (the #250 fix) — RECONSTRUCT-AND-SPLICE, pure-function layer.
    // The splice helpers are the load-bearing anti-decoy machinery: `declaration_sites`
    // counts the obligation short-name DECLARATION sites (a same-short-name decoy in ANY
    // namespace → > 1 → REJECT), `proof_assign_pos` finds the proof `:=` past the
    // record-update `specs := R_item`, and `decl_block_end` bounds the declaration. NO
    // lake needed (R-CHAR-3: the decoy game is structurally impossible by construction).
    #[test]
    fn reconstruct_splice_helpers_detect_decoy_and_splice_proof() {
        // (a) A SINGLE top-level declaration of the obligation → exactly one site.
        let single = "import Thermite.Stabilize\n\
                      theorem thermite_obligation_f : True := trivial\n";
        assert_eq!(declaration_sites(single, "thermite_obligation_f").len(), 1);

        // (b) THE #250 DECOY: a namespaced obligation declaration + a top-level
        // same-short-name decoy → TWO sites → the caller REJECTS as a duplicate.
        let decoy = "axiom thermite_cheat : ∀ p : Prop, p\n\
                     namespace Cheat\n\
                     theorem thermite_obligation_f (v : Thermite.Env) : True := thermite_cheat _\n\
                     end Cheat\n\
                     theorem thermite_obligation_f : True := trivial\n";
        assert_eq!(
            declaration_sites(decoy, "thermite_obligation_f").len(),
            2,
            "the namespaced cheat AND the top-level decoy are BOTH declaration sites — the \
             #250 mask is caught as a duplicate"
        );

        // (c) `proof_assign_pos` anchors on the PROOF `:=`, NOT the record-update `specs
        // := R_item` inside the proposition (the `:= by` form AND the term-mode form).
        let by_form = "theorem t (v : Thermite.Env) :\n  \
                       Thermite.stabilizes b { v with specs := R_item } r := by\n  exact h";
        let pos = proof_assign_pos(by_form).unwrap_or(0);
        assert!(
            by_form[pos..].starts_with(":= by"),
            "the proof anchor is the `:= by`, past the record-update `specs :=`"
        );
        let term_form = "theorem t (v : Thermite.Env) :\n  \
                         P { v with specs := R_item } := some_term _";
        let tpos = proof_assign_pos(term_form).unwrap_or(0);
        assert!(
            term_form[tpos..].starts_with(":= some_term"),
            "the term-mode anchor skips the record-update `specs :=`: {}",
            &term_form[tpos..]
        );

        // (d) END-TO-END reconstruct: a duplicate-decoy author file is REJECTED (the
        // canonical source is a well-formed single-theorem skeleton).
        let canonical = "import Thermite.Stabilize\n\n\
                         def R_item : Thermite.Registry := fun _ => none\n\n\
                         /-- doc -/\n\
                         theorem thermite_obligation_f (v : Thermite.Env) : True := by trivial\n";
        let p = parse_program("fn f(x: u32) -> u32 req true ens result == x fx pure { x }");
        let engine = LeanEngine::new(p, lean_root());
        let dup_err = match engine.reconstruct_replay(canonical, decoy, "f") {
            Err(err) => err,
            Ok(_) => String::new(),
        };
        assert!(
            dup_err.contains("duplicate obligation declaration"),
            "the #250 decoy → Err(\"duplicate obligation declaration\"), got {dup_err:?}"
        );

        // (e) A SINGLE author declaration with an INLINE-`have` proof term SPLICES: the
        // canonical statement is emitted verbatim, ONLY the proof term is spliced (the #252
        // helper-surface elimination — any file-level helper is DROPPED), imports/R_item
        // come from the canonical preamble (exactly once), and our OWN anchored probe is
        // appended. A file-level `theorem my_helper` the author leaves OUTSIDE the proof
        // term is DROPPED (it has nowhere to live).
        let author = "-- evidence_key: abc\n\
                      import Thermite.Stabilize\n\
                      def R_item : Thermite.Registry := fun _ => none\n\
                      theorem my_helper : True := True.intro\n\
                      theorem thermite_obligation_f (v : Thermite.Env) : True := by\n  \
                        have _aux : True := True.intro\n  trivial\n";
        let splice_result = engine.reconstruct_replay(canonical, author, "f");
        assert!(
            splice_result.is_ok(),
            "a single-declaration author file with an inline-have proof term must splice: \
             {splice_result:?}"
        );
        let spliced = splice_result.unwrap_or_default();
        assert_eq!(
            spliced.matches("import Thermite.Stabilize").count(),
            1,
            "the import comes from the canonical preamble exactly once: {spliced}"
        );
        assert_eq!(
            spliced.matches("def R_item").count(),
            1,
            "R_item comes from the canonical preamble exactly once: {spliced}"
        );
        assert!(
            !spliced.contains("theorem my_helper"),
            "the author's FILE-LEVEL helper is DROPPED — the helper surface is eliminated \
             (#252); only the proof term is spliced: {spliced}"
        );
        assert!(
            spliced.contains("have _aux : True := True.intro"),
            "the author's INLINE-have auxiliary (inside the proof term) is preserved: {spliced}"
        );
        assert_eq!(
            spliced.matches("theorem thermite_obligation_f").count(),
            1,
            "exactly ONE obligation theorem (the canonical one): {spliced}"
        );
        assert!(
            spliced
                .trim_end()
                .ends_with("#print axioms thermite_obligation_f"),
            "the ANCHORED probe targets the canonical declaration by construction: {spliced}"
        );
    }

    // REQ-6 / §1 / R-DEFER-9 (the #252 BELT) — the proof-term command scan. The proof term
    // is the ONLY author-controlled text and is type-checked against the FIXED generator
    // goal, so this is a defense layer against an `… in`-style command form smuggled into
    // the term. A genuine term/tactic proof (with inline `have`/`let`/`suffices`) carries
    // no command keyword; an `open … in` / `set_option … in` / `#…` form is caught
    // position-independently (exact-token). No lake needed (R-CHAR-3: a structural scan).
    #[test]
    fn proof_term_command_token_scans_position_independently() {
        // PERMITTED: a genuine tactic/term proof, INCLUDING inline `have`/`let`/`suffices`
        // auxiliaries and identifiers that merely CONTAIN a keyword (`openVal`,
        // `Nat.openInterval`) or are `.`-qualified projections.
        for ok in [
            "by\n  intro h\n  exact h",
            "by\n  have _aux : True := True.intro\n  trivial",
            "by\n  let openVal := 1\n  suffices h : True by exact h\n  trivial",
            "fun v => v.openField",
            "by exact Nat.openInterval_proof",
        ] {
            assert_eq!(
                proof_term_command_token(ok),
                None,
                "a genuine proof term (with inline have/let/suffices, keyword-containing \
                 identifiers, dotted projections) carries NO command keyword: {ok}"
            );
        }

        // REJECTED: a top-level command keyword smuggled into the term (e.g. via `… in`),
        // exact-token, in ANY position and with NO surrounding spaces (the `(open Foo in …)`
        // form). One fixture per forbidden class.
        for (term, kw) in [
            ("by open Foo in trivial", "open"),
            ("(open Thermite in trivial)", "open"),
            ("by set_option maxHeartbeats 0 in trivial", "set_option"),
            ("by\n  notation:max \"X\" => True\n  trivial", "notation"),
            ("by macro_rules | `(x) => `(True)", "macro_rules"),
            ("by macro \"x\" : term => `(True)", "macro"),
            ("by syntax \"x\" : term", "syntax"),
            ("by elab \"x\" : term => return default", "elab"),
            ("by attribute [simp] foo", "attribute"),
            ("by instance : Inhabited Nat := default", "instance"),
            ("by export Thermite in trivial", "export"),
            ("by import Thermite in trivial", "import"),
            ("by namespace Foo in trivial", "namespace"),
            ("by initialize x in trivial", "initialize"),
            ("by #check True", "#check"),
            ("by #print axioms foo", "#print"),
        ] {
            assert_eq!(
                proof_term_command_token(term).as_deref(),
                Some(kw),
                "the `{kw}` command form must be caught in the proof term: {term}"
            );
        }
    }

    // REQ-6 / §1 / R-DEFER-9 (the #252 helper-surface elimination) — author content
    // OUTSIDE the proof term is DROPPED, never spliced; the indented-command poison (the
    // #252 divergence) and the #251 macro-poison both have nowhere to live, so the
    // reconstructed file carries ONLY the canonical preamble + the proof term + the anchored
    // probe. A genuine inline-`have` proof term still splices. No lake (a structural test).
    #[test]
    fn reconstruct_drops_author_helper_section() {
        let canonical = "import Thermite.Stabilize\n\n\
                         def R_item : Thermite.Registry := fun _ => none\n\n\
                         /-- doc -/\n\
                         theorem thermite_obligation_f (v : Thermite.Env) : True := by trivial\n";
        let p = parse_program("fn f(x: u32) -> u32 req true ens result == x fx pure { x }");
        let engine = LeanEngine::new(p, lean_root());

        // The #251 macro-poison + the #252 INDENTED-command poison: both live in the author
        // file OUTSIDE the obligation declaration's proof term. The reconstruction DROPS all
        // of it (the helper surface is eliminated) — the poison never reaches the emitted
        // file, so it can never re-elaborate the obligation. The proof term (`by trivial`)
        // splices onto the canonical statement.
        for poison in [
            // column-0 notation (the #251 form)
            "-- evidence_key: abc\n\
             import Thermite.Stabilize\n\
             def R_item : Thermite.Registry := fun _ => none\n\
             notation:max \"Thermite.stabilizesProp\" => (fun _ _ => True)\n\
             theorem thermite_obligation_f (v : Thermite.Env) : True := by trivial\n",
            // INDENTED notation (the #252 form) — attached to a dummy helper's body line
            "-- evidence_key: abc\n\
             import Thermite.Stabilize\n\
             def R_item : Thermite.Registry := fun _ => none\n\
             theorem dummy_helper : True := True.intro\n  \
               notation:max \"Thermite.stabilizesProp\" => (fun _ _ => True)\n\
             theorem thermite_obligation_f (v : Thermite.Env) : True := by trivial\n",
            // a set_option / open / instance helper soup
            "-- evidence_key: abc\n\
             import Thermite.Stabilize\n\
             def R_item : Thermite.Registry := fun _ => none\n\
             set_option maxHeartbeats 0\n\
             open Thermite\n\
             instance : Inhabited Nat := ⟨0⟩\n\
             theorem thermite_obligation_f (v : Thermite.Env) : True := by trivial\n",
        ] {
            let out = engine
                .reconstruct_replay(canonical, poison, "f")
                .unwrap_or_default();
            assert!(
                !out.contains("notation")
                    && !out.contains("set_option")
                    && !out.contains("open Thermite")
                    && !out.contains("instance")
                    && !out.contains("dummy_helper"),
                "the author HELPER section (the #251/#252 poison) is DROPPED — the reconstructed \
                 file carries ONLY the canonical preamble + the proof term + the probe: {out}"
            );
            assert_eq!(
                out.matches("theorem thermite_obligation_f").count(),
                1,
                "exactly ONE obligation theorem (the canonical one): {out}"
            );
            assert!(
                out.trim_end()
                    .ends_with("#print axioms thermite_obligation_f"),
                "the anchored probe targets the canonical declaration: {out}"
            );
        }

        // A genuine INLINE-have proof term still splices (no expressivity loss — a
        // single-obligation proof inlines auxiliaries as `have`).
        let legit = "-- evidence_key: abc\n\
                     import Thermite.Stabilize\n\
                     def R_item : Thermite.Registry := fun _ => none\n\
                     theorem thermite_obligation_f (v : Thermite.Env) : True := by\n  \
                       have _aux : True := True.intro\n  trivial\n";
        let spliced = engine
            .reconstruct_replay(canonical, legit, "f")
            .unwrap_or_default();
        assert!(
            spliced.contains("have _aux : True := True.intro"),
            "a genuine inline-have proof term still splices (the #252 inline form): {spliced}"
        );

        // The #252 BELT: a proof term smuggling an `open … in` command form → REJECTED
        // (defense layer, before lake).
        let belt = "-- evidence_key: abc\n\
                    import Thermite.Stabilize\n\
                    def R_item : Thermite.Registry := fun _ => none\n\
                    theorem thermite_obligation_f (v : Thermite.Env) : True := by\n  \
                      open Thermite in trivial\n";
        let belt_err = engine
            .reconstruct_replay(canonical, belt, "f")
            .err()
            .unwrap_or_default();
        assert!(
            belt_err.contains("disallowed proof-term command: open"),
            "an `open … in` command form in the proof term → Err (the #252 belt): {belt_err:?}"
        );
    }

    // REQ-6 / REQ-7(ii) / R-DEFER-9 (the #252 inline-have migration) — LIVE auxiliary-lemma
    // replay. After the #252 helper-surface elimination, a single-obligation proof inlines
    // its auxiliaries as `have` INSIDE the proof term (no expressivity loss). A CLEAN inline
    // `have` auxiliary, genuinely used, REPLAYS Proven (clean axiom base). A proof term that
    // leans on a `sorry` (the only way an inline auxiliary can introduce a non-standard
    // axiom — file-level axioms are DROPPED) flows `sorryAx` into the obligation theorem's
    // anchored `#print axioms` → Unknown, NEVER Proven. Expected from REQ-4/§1 (R-CHAR-3).
    // LIVE (gated on lake).
    #[test]
    fn interactive_inline_have_clean_proven_sorry_unknown() {
        if !lake_present() {
            eprintln!("SKIP: lake not present — the inline-have replay test is not run.");
            return;
        }
        let dir = std::env::temp_dir().join(format!("forge_it_helper_{}", std::process::id()));
        assert!(ensure_dir(&dir), "scratch dir creatable");
        let file = dir.join("add.th");
        let src = "fn add(a: u32, b: u32) -> u64 req true \
                   ens result == a as u64 + b as u64 fx pure { a as u64 + b as u64 }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "add", vec![]);
        let engine = LeanEngine::new(p, lean_root());
        let key = engine.evidence_key(&o);
        let artifact = interactive_proof_path(&file, "add");

        // Emit the skeleton, then lift its WORKING `:= by …` proof body (the auto
        // battery) so the authored proof genuinely closes the canonical goal.
        let _ = engine.replay_interactive(&file, &o);
        let skeleton = std::fs::read_to_string(&artifact).unwrap_or_default();
        let stmt = canonical_theorem_statement(&skeleton, "add").unwrap_or_default();
        let by_pos = skeleton.find(":= by").unwrap_or(0);
        let body = skeleton[by_pos + ":= by".len()..].trim_end().to_string();

        // (1) CLEAN INLINE-have auxiliary, genuinely referenced (`have aux : True :=
        // True.intro`): Proven. The auxiliary lives INSIDE the proof term (the #252 inline
        // form), so it is preserved by the reconstruction and the anchored `#print axioms`
        // sees only the clean standard axiom base.
        let clean = format!(
            "{INTERACTIVE_EVIDENCE_KEY_MARKER}{key}\n\
             import Thermite.Stabilize\n\
             {stmt} by\n  have aux : True := True.intro\n  let _ := aux\n{body}\n",
            key = key.content_address
        );
        assert!(
            write_file(&artifact, &clean),
            "clean inline-have artifact writable"
        );
        let v_clean = engine.replay_interactive(&file, &o);
        assert!(
            matches!(v_clean, Verdict::Proven(_)),
            "a clean inline-have proof term REPLAYS Proven (the auxiliary's clean axioms are \
             transitively checked): {v_clean:?}"
        );

        // (2) SORRY in the proof term: an inline auxiliary discharged by `sorry` flows
        // `sorryAx` into the obligation theorem's anchored `#print axioms` → Unknown (NEVER
        // Proven). This is the only way an inline auxiliary can introduce a non-standard
        // axiom (file-level axioms are DROPPED, #252), so it exercises the axiom/sorry gate
        // on the surviving (proof-term-only) surface.
        let sorrytm = format!(
            "{INTERACTIVE_EVIDENCE_KEY_MARKER}{key}\n\
             import Thermite.Stabilize\n\
             {stmt} by\n  have aux : True := by sorry\n  let _ := aux\n{body}\n",
            key = key.content_address
        );
        assert!(
            write_file(&artifact, &sorrytm),
            "sorry-bearing inline artifact writable"
        );
        let v_sorry = engine.replay_interactive(&file, &o);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            matches!(v_sorry, Verdict::Unknown(_)),
            "an inline auxiliary discharged by `sorry` flows `sorryAx` into the obligation's \
             anchored `#print axioms` → Unknown, NEVER Proven (REQ-4/§1, R-DEFER-9): {v_sorry:?}"
        );
    }

    // REQ-4/§1/R-DEFER-9 (the #248 fix): the trust-base axiom ALLOWLIST parser is STRICT
    // — it anchors on the `#print axioms` REPORT line ("depends on axioms: [...]"), so a
    // lake WARNING that itself carries a `[Thermite.Env.bindInt, …]` simp-arg list does
    // NOT false-positive, and a NON-standard axiom (a smuggled cheat, the sorry axiom) IS
    // caught. Pure-function regression for `nonstandard_axiom`.
    #[test]
    fn nonstandard_axiom_parses_the_report_line_strictly() {
        // The clean standard set on the OBLIGATION theorem's anchored line → Clean.
        assert_eq!(
            nonstandard_axiom(
                "'thermite_obligation_add' depends on axioms: [propext, Classical.choice, \
                 Quot.sound]",
                "add"
            ),
            AxiomReport::Clean
        );
        // "does not depend on any axioms" (no bracket) on the anchored line → Clean.
        assert_eq!(
            nonstandard_axiom("'thermite_obligation_t' does not depend on any axioms", "t"),
            AxiomReport::Clean
        );
        // A WARNING whose simp-arg bracket list precedes the report line must NOT
        // false-positive — the parser anchors on the report marker, never the first
        // `[`. This is exactly the legit-auto-replay output shape.
        assert_eq!(
            nonstandard_axiom(
                "warning: simp only [Thermite.Env.bindInt, Thermite.intVal] at hreq\n\
                 'thermite_obligation_add' depends on axioms: [propext, Classical.choice, \
                 Quot.sound]",
                "add"
            ),
            AxiomReport::Clean,
            "a simp-arg warning bracket must NOT be mistaken for the axiom list"
        );
        // A SMUGGLED non-standard axiom IS caught (the divergence the #248 pin exhibits).
        assert_eq!(
            nonstandard_axiom(
                "'thermite_obligation_f' depends on axioms: [propext, thermite_cheat]",
                "f"
            ),
            AxiomReport::Nonstandard("thermite_cheat".to_string())
        );
        // The sorry axiom is also outside the allowlist (caught here too, belt-and-braces).
        assert_eq!(
            nonstandard_axiom("'thermite_obligation_t' depends on axioms: [sorryAx]", "t"),
            AxiomReport::Nonstandard("sorryAx".to_string())
        );
        // #249 MARKER MASK: an author's OWN earlier `#print axioms clean_helper` (clean)
        // must NOT mask the obligation theorem's smuggled axiom. The parser anchors on the
        // OBLIGATION theorem's report, so the clean helper line is ignored and the
        // obligation's `thermite_cheat` is caught.
        assert_eq!(
            nonstandard_axiom(
                "'clean_helper' depends on axioms: [propext]\n\
                 'thermite_obligation_f' depends on axioms: [propext, thermite_cheat]",
                "f"
            ),
            AxiomReport::Nonstandard("thermite_cheat".to_string()),
            "an earlier clean `#print axioms` must NOT mask the obligation theorem's axiom"
        );
        // No report line names the obligation theorem → Missing (NEVER fall through to a
        // foreign theorem's clean report).
        assert_eq!(
            nonstandard_axiom("'clean_helper' depends on axioms: [propext]", "f"),
            AxiomReport::Missing,
            "a missing obligation-theorem anchor is a hard reject, never Clean"
        );
    }

    // REQ-6 STATEMENT BINDING (the #248 fix): the canonical-statement extractor lifts
    // the `theorem thermite_obligation_<item> … :=` span (binders + proposition, up to
    // the proof term), and `statements_match` is whitespace-insensitive but
    // proposition-strict. A record-update `specs := R_item` inside the proposition does
    // NOT prematurely end the statement (the `:= by` anchor). Pure-function regression.
    #[test]
    fn canonical_statement_extraction_and_whitespace_match() {
        let canonical =
            "/- doc -/\ntheorem thermite_obligation_f (v : Thermite.Env) (r : Int) :\n  \
             Thermite.stabilizes body { v with specs := R_item } r ->\n  True := by\n  trivial";
        let opt = canonical_theorem_statement(canonical, "f");
        assert!(
            opt.is_some(),
            "a statement should extract from the canonical source"
        );
        let extracted = opt.unwrap_or_default();
        // The record-update `:=` did NOT truncate the statement (the `:= by` anchor).
        assert!(
            extracted.contains("specs := R_item") && extracted.trim_end().ends_with(":="),
            "the statement spans through the proof `:=`, past the record-update `:=`: \
             {extracted}"
        );
        // A reformatted (re-wrapped) SAME statement matches (whitespace-insensitive).
        let reformatted = "theorem thermite_obligation_f (v : Thermite.Env) (r : Int) : \
             Thermite.stabilizes body { v with specs := R_item } r -> True :=";
        assert!(statements_match(&extracted, reformatted));
        // A DIFFERENT proposition does NOT match.
        let different = "theorem thermite_obligation_f : True :=";
        assert!(!statements_match(&extracted, different));
    }

    // REQ-9/AC-7 (the #248 fix): the Lean-path tally floor gate. A `1/1` ratio MEETS the
    // default floor; a `0/0` (all-untested with mutants generated) or a below-floor
    // ratio does NOT — the SHIPPED 0/0 backstop + the WeakContract mirror.
    #[test]
    fn lean_tally_floor_gate() {
        let mut clean = LeanMutationTally::default();
        clean.record(LeanMutantOutcome::Killed, false); // 1/1
        assert!(clean.meets_floor(0.60), "1/1 meets the floor");

        let mut all_untested = LeanMutationTally::default();
        all_untested.record(LeanMutantOutcome::UntestedAgainstLean, false);
        all_untested.record(LeanMutantOutcome::UntestedAgainstLean, false);
        assert!(
            !all_untested.meets_floor(0.60),
            "0/0 (all untested, mutants generated) is BELOW the floor (the 0/0 backstop) — \
             never a vacuous L3 pass"
        );

        let mut weak = LeanMutationTally::default();
        weak.record(LeanMutantOutcome::Killed, false); // 1 killed
        weak.record(LeanMutantOutcome::Survived, false); // 1 survivor -> 1/2
        weak.record(LeanMutantOutcome::Survived, false); // -> 1/3
        assert!(
            !weak.meets_floor(0.60),
            "a below-floor ratio (1/3) does NOT certify L3-via-Lean (WeakContract mirror)"
        );
        assert_eq!(weak.mutants_killed_string(), "1/3");
    }

    // REQ-2(c)/(d): the Lean engine fills its four slots — the SMALLER trust profile
    // ({Lean kernel + 3 axioms, EXP}) and the engine-discriminated evidence key
    // (composing the toolchain rev + spine hash + LEAN_SCHEMA_VERSION). Expected from
    // REQ-2(c)/§2(d) (R-CHAR-3).
    #[test]
    fn lean_engine_fills_trust_and_evidence_slots() {
        let p = parse_program("fn id(x: u64) -> u64 req true ens result == x fx pure { x }");
        let o = fn_obligation(&p, "id", vec![]);
        let engine = LeanEngine::new(p, lean_root());
        assert_eq!(engine.name(), EngineName::LeanAuto);
        assert!(
            !engine.fragment().admits_all_classes,
            "the Lean engine is a NARROWED fragment (not the whole subset)"
        );
        let tp = engine.trust_profile();
        assert!(
            tp.items.iter().any(|i| i.contains("Lean kernel"))
                && tp.items.iter().any(|i| i.contains("EXP")),
            "the trust profile enumerates Lean kernel + EXP: {:?}",
            tp.items
        );
        let key = engine.evidence_key(&o);
        assert_eq!(key.engine, EngineName::LeanAuto);
        assert_eq!(key.content_address.len(), 64, "sha256 hex content address");
    }

    // #246 / REQ-7(ii) — STALENESS: two SAME-NAMED items with DIFFERENT `ens` must
    // produce DIFFERENT evidence keys (the obligation CONTENT is hashed, so a contract
    // edit can NEVER silently reuse a cached `Proven`). Hand-derived (R-CHAR-3): the
    // only delta is the ens RHS (`>= a` vs `>= b`); the content hash distinguishes them.
    #[test]
    fn evidence_key_differs_on_different_ens() {
        let p1 =
            parse_program("fn m(a: u32, b: u32) -> u32 req true ens result >= a fx pure { a }");
        let p2 =
            parse_program("fn m(a: u32, b: u32) -> u32 req true ens result >= b fx pure { a }");
        let o1 = fn_obligation(&p1, "m", vec![]);
        let o2 = fn_obligation(&p2, "m", vec![]);
        let e1 = LeanEngine::new(p1, lean_root());
        let e2 = LeanEngine::new(p2, lean_root());
        let k1 = e1.evidence_key(&o1);
        let k2 = e2.evidence_key(&o2);
        assert_ne!(
            k1.content_address, k2.content_address,
            "two same-named items with DIFFERENT ens must have DIFFERENT keys (#246 staleness)"
        );
    }

    // #246 — TARGETED-SPINE STALENESS: an edit ANYWHERE under `lean/Thermite/**`
    // (including a NESTED subdirectory — the recursive widening) must change the
    // evidence key. Hand-derived: a synthetic spine root with a nested `Exec/x.lean`
    // file; appending a byte to the nested file changes `spine_content_hash`, hence
    // the key. Uses a temp dir (no mutation of the real spine).
    #[test]
    fn evidence_key_differs_on_nested_spine_edit() {
        let tmp = std::env::temp_dir().join(format!("forge_spine_test_{}", std::process::id()));
        let nested = tmp.join("Thermite").join("Exec");
        assert!(
            std::fs::create_dir_all(&nested).is_ok(),
            "scratch spine dir must be creatable"
        );
        // A toolchain marker so toolchain_rev is stable across the two reads.
        let _ = std::fs::write(tmp.join("lean-toolchain"), "leanprover/lean4:test");
        let _ = std::fs::write(tmp.join("Thermite").join("Ast.lean"), "-- ast\n");
        let nested_file = nested.join("x.lean");
        let _ = std::fs::write(&nested_file, "-- exec v1\n");

        let p = parse_program("fn id(x: u64) -> u64 req true ens result == x fx pure { x }");
        let o = fn_obligation(&p, "id", vec![]);
        let e_before = LeanEngine::new(p.clone(), tmp.clone());
        let k_before = e_before.evidence_key(&o);

        // Edit the NESTED spine file (the case the non-recursive walk MISSED).
        let _ = std::fs::write(&nested_file, "-- exec v2 EDITED\n");
        let e_after = LeanEngine::new(p, tmp.clone());
        let k_after = e_after.evidence_key(&o);

        // Cleanup before the assert (so a failure still leaves a clean tree).
        let _ = std::fs::remove_dir_all(&tmp);

        assert_ne!(
            k_before.content_address, k_after.content_address,
            "a nested lean/Thermite/Exec/** edit must change the key (#246 recursive spine hash)"
        );
    }

    // ========================================================================
    // increment (iii), #247 — REQ-4/REQ-5/REQ-7/REQ-9 unit tests.
    // ========================================================================

    // A synthetic always-`Proven` verdict (the disagreement teeth, REQ-5). Built
    // directly (no engine needed for the pure `check_disagreement` guard).
    fn stub_proven() -> Verdict {
        Verdict::Proven(Evidence {
            verified: 1,
            key: a_key(),
        })
    }

    // A synthetic WITNESSED-`Refuted` verdict (the other half of the teeth, REQ-5).
    fn stub_refuted() -> Verdict {
        Verdict::Refuted(Counterexample {
            obligations: vec![ObligationResult::failed(
                "postcondition not satisfied",
                Some("f.th:3:5".to_string()),
                Some("error: postcondition not satisfied".to_string()),
            )],
        })
    }

    // A synthetic always-`Proven` ENGINE (for `attribution_for`, REQ-4). A TEST double.
    #[derive(Debug, Clone, Copy)]
    struct StubProvenEngine;
    impl Engine for StubProvenEngine {
        fn name(&self) -> EngineName {
            EngineName::LeanAuto
        }
        fn fragment(&self) -> Fragment {
            Fragment {
                admits_all_classes: true,
            }
        }
        fn discharge(&self, _o: &Obligation) -> Verdict {
            stub_proven()
        }
        fn trust_profile(&self) -> TrustProfile {
            TrustProfile {
                items: vec!["Lean kernel".to_string(), "EXP".to_string()],
            }
        }
        fn evidence_key(&self, _o: &Obligation) -> CacheKey {
            a_key()
        }
    }

    // REQ-5 — THE DISAGREEMENT HALT TEETH: Proven ⊕ witnessed-Refuted on the SAME
    // obligation FIRES the alarm, naming BOTH engines + the item. Expected from REQ-5
    // (R-CHAR-3): a Proven ⊕ witnessed-Refuted disagreement is a soundness alarm.
    #[test]
    fn proven_refuted_disagreement_halts() {
        let proven = stub_proven();
        let refuted = stub_refuted();
        let r = check_disagreement(
            "f",
            EngineName::LeanAuto,
            &proven,
            EngineName::Verus,
            &refuted,
        );
        assert!(
            r.is_err(),
            "a Proven ⊕ Refuted disagreement MUST halt (REQ-5)"
        );
        if let Err(d) = r {
            assert_eq!(d.item, "f");
            assert_eq!(d.proven_engine, EngineName::LeanAuto.tag());
            assert_eq!(d.refuted_engine, EngineName::Verus.tag());
            assert!(
                !d.counterexample.obligations.is_empty(),
                "the alarm carries the witnessing counterexample"
            );
        }
        // The order does not matter — Refuted ⊕ Proven also halts, naming the right
        // engine for each role.
        let r2 = check_disagreement(
            "f",
            EngineName::Verus,
            &refuted,
            EngineName::LeanAuto,
            &proven,
        );
        assert!(r2.is_err(), "Refuted ⊕ Proven also halts");
        if let Err(d) = r2 {
            assert_eq!(d.proven_engine, EngineName::LeanAuto.tag());
            assert_eq!(d.refuted_engine, EngineName::Verus.tag());
        }
    }

    // REQ-5 — Proven ⊕ Unknown is BENIGN (the Unknown engine simply could not decide;
    // per REQ-3.1 a witness-less Verus failure is Unknown, so it can NEVER spuriously
    // fire the alarm against a Lean Proven). Expected from REQ-5 (R-CHAR-3).
    #[test]
    fn proven_unknown_is_benign() {
        let proven = stub_proven();
        let unknown = Verdict::Unknown(Reason::IncompleteUnknown("could not decide".to_string()));
        assert!(
            check_disagreement(
                "f",
                EngineName::LeanAuto,
                &proven,
                EngineName::Verus,
                &unknown
            )
            .is_ok(),
            "Proven ⊕ Unknown is benign — NOT a soundness alarm (REQ-5)"
        );
        assert!(
            check_disagreement(
                "f",
                EngineName::Verus,
                &unknown,
                EngineName::LeanAuto,
                &proven
            )
            .is_ok(),
            "Unknown ⊕ Proven is benign too"
        );
        // Refuted ⊕ Refuted is agreement on a bug (both witnessed) — benign for the
        // alarm (the hard fail stands on its own; no soundness CONTRADICTION).
        let refuted = stub_refuted();
        assert!(
            check_disagreement(
                "f",
                EngineName::Verus,
                &refuted,
                EngineName::LeanAuto,
                &refuted
            )
            .is_ok(),
            "Refuted ⊕ Refuted is agreement, not a contradiction"
        );
    }

    // REQ-4 — ATTRIBUTION: the `{engine, trust_profile}` pair is the engine's name tag
    // + its enumerated trust items. Expected from REQ-4 (R-CHAR-3): the Lean profile is
    // SMALLER along the named axes (no Z3, no Verus VC-gen).
    #[test]
    fn attribution_records_engine_and_trust_base() {
        let lean_attr = attribution_for(&StubProvenEngine);
        assert_eq!(lean_attr.engine, EngineName::LeanAuto.tag());
        assert!(lean_attr
            .trust_profile
            .iter()
            .any(|i| i.contains("Lean kernel")));
        assert!(
            !lean_attr.trust_profile.iter().any(|i| i.contains("Z3")),
            "the Lean base does NOT enumerate Z3 (smaller along the named axes, REQ-4)"
        );
        let verus_attr = attribution_for(&VerusEngine);
        assert!(verus_attr.trust_profile.iter().any(|i| i.contains("Z3")));
    }

    // REQ-7 — SORRY DETECTION: a `sorry` token in the SOURCE OR a `sorryAx` in the
    // `#print axioms` output is detected; a clean proof with only the standard axioms
    // is NOT. Expected from REQ-7(ii) (R-CHAR-3): sorry NEVER Proven.
    #[test]
    fn sorry_detected_in_source_or_axioms() {
        // A skeleton's `sorry` token in the source.
        assert!(proof_has_sorry(
            "theorem t : True := by\n  sorry\n",
            "'t' depends on axioms: [propext]"
        ));
        // A `sorryAx` in the axioms output (a sorry that survived elaboration).
        assert!(proof_has_sorry(
            "theorem t : True := by trivial",
            "'t' depends on axioms: [sorryAx]"
        ));
        // A genuinely clean proof (no sorry token, only standard axioms) is NOT a sorry.
        assert!(
            !proof_has_sorry(
                "theorem t : True := by trivial",
                "'t' depends on axioms: [propext, Classical.choice, Quot.sound]"
            ),
            "a clean standard-axiom proof is NOT a sorry"
        );
        // A substring like `sorryless` does NOT false-positive (whole-word match).
        assert!(!proof_has_sorry("def sorryless := 1", "axioms: [propext]"));
    }

    // REQ-7 — the interactive proof artifact PATH is `<file>.lean-proofs/<item>.lean`.
    // Expected from REQ-7(ii) (R-CHAR-3).
    #[test]
    fn interactive_proof_path_is_beside_source() {
        let p = interactive_proof_path(std::path::Path::new("/x/y/prog.th"), "spec_sum");
        assert!(p.ends_with("spec_sum.lean"), "{p:?}");
        assert!(
            p.to_string_lossy().contains("prog.th.lean-proofs"),
            "the artifact lives beside the source: {p:?}"
        );
    }

    // REQ-9 — the Lean-path kill semantics: an ADMITTED mutant Lean does not prove
    // (Refuted ∪ Unknown-after-attempt) is KILLED; an admitted Proven mutant SURVIVED;
    // a NON-admitted mutant is UntestedAgainstLean (never killed). Expected from REQ-9
    // (R-CHAR-3): the engine-generic kill = the shipped Counterexample ∪ Timeout.
    #[test]
    fn lean_mutant_outcome_follows_req9() {
        let proven = stub_proven();
        let unknown = Verdict::Unknown(Reason::IncompleteUnknown("tactic failed".to_string()));
        let refuted = Verdict::Refuted(Counterexample {
            obligations: vec![],
        });
        // Admitted + Proven → Survived.
        assert_eq!(
            lean_mutant_outcome(true, &proven),
            LeanMutantOutcome::Survived
        );
        // Admitted + Unknown-after-attempt → Killed (the shipped Timeout=killed).
        assert_eq!(
            lean_mutant_outcome(true, &unknown),
            LeanMutantOutcome::Killed
        );
        // Admitted + Refuted → Killed (a witnessed countermodel).
        assert_eq!(
            lean_mutant_outcome(true, &refuted),
            LeanMutantOutcome::Killed
        );
        // NOT admitted → UntestedAgainstLean REGARDLESS of the verdict (never a kill).
        assert_eq!(
            lean_mutant_outcome(false, &unknown),
            LeanMutantOutcome::UntestedAgainstLean
        );
        assert_eq!(
            lean_mutant_outcome(false, &proven),
            LeanMutantOutcome::UntestedAgainstLean
        );
    }

    // REQ-9 — the tally: untested mutants are OUTSIDE the denominator (never inflate
    // the ratio); the #101-equivalent survivors are dropped from BOTH the survivor set
    // AND the denominator. Expected from REQ-9 + §7 (R-CHAR-3).
    #[test]
    fn lean_mutation_tally_does_not_inflate_on_untested() {
        let mut t = LeanMutationTally::default();
        t.record(LeanMutantOutcome::Killed, false); // 1 killed, +1 denom
        t.record(LeanMutantOutcome::Killed, false); // 2 killed, +1 denom
        t.record(LeanMutantOutcome::UntestedAgainstLean, false); // OUTSIDE the ratio
        t.record(LeanMutantOutcome::Survived, true); // proven-equivalent → excluded BOTH
        t.record(LeanMutantOutcome::Survived, false); // a genuine survivor → +1 denom
        assert_eq!(t.killed, 2);
        assert_eq!(
            t.attempted, 3,
            "2 killed + 1 genuine survivor; equivalent excluded"
        );
        assert_eq!(
            t.untested, 1,
            "the untested mutant is reported, not in the ratio"
        );
        assert_eq!(
            t.equivalent, 1,
            "the proven-equivalent is dropped from the denominator"
        );
        // 2/3 ≈ 0.667 — the untested mutant did NOT inflate it to 2/2 = 1.0.
        assert!(
            (t.kill_ratio() - 2.0 / 3.0).abs() < 1e-9,
            "ratio = {}",
            t.kill_ratio()
        );
        assert!(
            t.qualifier().contains("untested against lean"),
            "the qualifier names the untested count: {}",
            t.qualifier()
        );
    }
}
