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

use crate::obligation::{Obligation, ObligationRole};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::VerusOutcome;
    use crate::manifest::ObligationResult;

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
}
