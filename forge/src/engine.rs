//! `forge/src/engine.rs` — the backend-neutral proof-engine interface and the Verus
//! engine refactored behind it (`.design/verified/proof-backends.md` REQ-2/REQ-3/
//! REQ-3.1/REQ-8; increment (i), blocker #204).
//!
//! Today one engine is welded into `check::check_file_with_options` (the implicit
//! `run_verus` + `classify_verus_outcome` path). This module introduces the
//! [`Engine`] trait — the four slots REQ-2 names (fragment, discharge, trust
//! profile, evidence) — and refactors the Verus discharge byte-identically behind
//! it, with the one named exception of the REQ-3.1 fast-unknown remap.
//!
//! The REQ-3.1 fast-unknown seam is the single behavioral delta. The shipped
//! `classify_verus_outcome` absorbs the SMT incompleteness-`unknown` into its
//! `Counterexample` bucket (an `unknown` returned fast, no `--profile` report → the
//! failure path). A byte-identical Verus engine would map that `Counterexample` to
//! [`Verdict::Refuted`] → `ladder_action_l3` hard fail, contradicting REQ-3 ("an
//! SMT `unknown` is `Unknown`, never `Refuted`; refutation requires a witnessing
//! input"). So [`VerusEngine::verdict_of`] splits the `Counterexample` by
//! [`counterexample_is_incompleteness_unknown`], the narrow SMT-`unknown`
//! signature: a span-less failure carrying the SMT-`unknown` signal (no parsed
//! `--> file:line:col` span and no frontend `error[E…]` type error) maps to
//! [`Verdict::Unknown`] (degrade, matching §6's degrade-on-incompleteness intent); a
//! witnessed countermodel (a parsed span) and a frontend rejection (a type error
//! `error[E…]`) both stay [`Verdict::Refuted`] (hard fail, never degrades).
//!
//! The narrow signature preserves cert-oracle byte-identity. The remap is the sole
//! exception to increment (i)'s byte-identical claim, and it is inert on the
//! conformance corpus. The corpus contains `Counterexample`-bucket failures, notably
//! the provenance `careless_query` IFC path, which verus rejects with a span-less
//! type error `error[E0308]` the corpus pins at L0. A coarse "no parsed span →
//! Unknown" rule would degrade that E0308 to L2 (and crash on the ADT L2 lowering),
//! perturbing the oracle. The narrow signature keeps E0308 (and every witnessed
//! countermodel) at `Refuted` → L0, so it fires only on a genuine SMT-`unknown`, a
//! case the corpus does not contain, leaving every `conformance/*.cert.json`
//! byte-identical (REQ-3.1's "the remap only changes behavior on inputs the corpus
//! does not contain").
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=forge-engine-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-ENGINE-ATTRIBUTION | shipped | `forge/src/engine.rs` | Certificate engine attribution |  |
//! | REQ-FORGE-ENGINE-DISAGREEMENT | shipped | `forge/src/engine.rs` | Engine disagreement soundness alarm |  |
//! | REQ-FORGE-ENGINE-DISCHARGE-DISCIPLINE | shipped | `forge/src/engine.rs` | Engine verdict ladder discipline |  |
//! | REQ-FORGE-ENGINE-FAST-UNKNOWN | shipped | `forge/src/engine.rs` | Verus fast-unknown remap |  |
//! | REQ-FORGE-ENGINE-INTERACTIVE-REPLAY | shipped | `forge/src/engine.rs` | Interactive Lean proof replay |  |
//! | REQ-FORGE-ENGINE-INTERFACE | shipped | `forge/src/engine.rs` | Backend-neutral proof engine interface |  |
//! | REQ-FORGE-ENGINE-LEAN-MUTATION | shipped | `forge/src/engine.rs` | Lean engine mutation accounting |  |
//! | REQ-FORGE-ENGINE-ORDERING | shipped | `forge/src/engine.rs` | Proof engine ordering hook |  |
//! <!-- /generated:reqs -->

use crate::covenant::CovenantRecord;
use crate::lean_export::{export_item, find_item, ExportRefusal, ExportedObligation};
use crate::obligation::{Obligation, ObligationRole};
use std::collections::BTreeMap;
use std::path::PathBuf;
use thermite_syntax::{Item, Program};

/// The named engine (`.design/verified/proof-backends.md` REQ-2 `name`). The
/// evidence cache key carries this discriminator (§2(d)) so a Verus proof and a
/// Lean proof of the same item never collide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineName {
    /// The Verus/Z3 push-button engine (increment (i)).
    Verus,
    /// The Lean auto tactic-battery engine (increments (ii)/(iii), SHIPPED —
    /// #240/#247): the name [`LeanEngine::new`] constructs; reaches production via
    /// `check::check_file_with_engine` (`--engine lean|auto`).
    LeanAuto,
    /// The Lean interactive engine name. The SHIPPED interactive replay (REQ-7,
    /// `replay_interactive`) runs on the `LeanAuto`-named engine instance and
    /// attributes through `trust_profile_interactive`; this variant stays
    /// forward-declared in the cache-key discriminator (§2(d)) for a dedicated
    /// interactive-named instance.
    #[allow(
        dead_code,
        reason = "proof-backends §2(d): forward-declared cache-key discriminant; the \
                  shipped interactive replay (#247) attributes via trust_profile_interactive \
                  on the LeanAuto-named instance and never constructs this name"
    )]
    LeanInteractive,
    /// The nlsat real-relaxation engine (`.design/stage1-forge-tier.md` REQ-8 /
    /// Q-NLSAT, increment 2f): a direct Z3 `nlsat`-tactic (QF_NRA) query over the
    /// relax fragment. Today Z3 is reached only THROUGH Verus (a VC-gen solver call);
    /// this is the first real-arithmetic Z3 query as its own engine. A `Proven`
    /// nlsat discharge certifies at the kernel-grounded [`crate::manifest::Level::L4`]
    /// — its trust profile is `solver(nlsat) + spine-lemma(kernel)` (the real→integer
    /// soundness bridge is the kernel-checked `r_relax_sound`, `lean/Thermite/Relax.lean`).
    Nlsat,
}

impl EngineName {
    /// The stable tag for the evidence cache-key discriminator (§2(d)) and
    /// diagnostics (deterministic, R-CODE-5).
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            EngineName::Verus => "verus",
            EngineName::LeanAuto => "lean-auto",
            EngineName::LeanInteractive => "lean-interactive",
            EngineName::Nlsat => "nlsat",
        }
    }
}

/// The reason an engine returned [`Verdict::Unknown`] (`.design/verified/
/// proof-backends.md` REQ-2(b)/REQ-3). An `Unknown` means this engine could not
/// decide; it is not a failure verdict and it degrades per the ladder (§6). The two
/// Verus-engine reasons mirror the shipped `VerusOutcome`'s non-`Proved` arms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// The solver exhausted its SMT resource budget (rlimit) — the shipped
    /// `VerusOutcome::Timeout`. Carries the human detail (the `--profile`-derived
    /// summary the cert's `solver_profile` already records).
    VerusTimeout(String),
    /// The SMT incompleteness-`unknown` edge (the REQ-3.1 remap): a `Counterexample`
    /// outcome carrying no witnessing input (no parsed `--> span`). Semantically a
    /// timeout-class incompleteness event (the solver could not decide), so it
    /// degrades, never hard-fails. Carries the raw stderr head for diagnosis.
    IncompleteUnknown(String),
}

/// A witnessed countermodel (`.design/verified/proof-backends.md`
/// REQ-2(b)/REQ-3): a `Refuted` verdict's deliverable (§5.1). Carries the
/// per-obligation failure results the shipped `parse_stderr_failures` produced (each
/// with its `--> file:line:col` span, the witnessing input). A `Refuted` requires at
/// least one witnessing input; a witness-less failure is a [`Verdict::Unknown`],
/// never this (REQ-3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counterexample {
    /// The per-obligation failure results (the §5.1 witnesses).
    pub obligations: Vec<crate::manifest::ObligationResult>,
}

/// The replayable, cacheable evidence an engine attaches to a [`Verdict::Proven`]
/// (`.design/verified/proof-backends.md` REQ-2(d)). For the Verus engine the
/// evidence is the count of discharged obligations plus the engine's cache key, the
/// content-addressed proof-cache entry the shipped `cache::store`/`load` serves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    /// The number of obligations verus discharged (the `verified` count).
    pub verified: u64,
    /// The content-addressed evidence key (the shipped `cache_key` generalized
    /// with the engine discriminator, §2(d)).
    pub key: CacheKey,
}

/// The verdict an engine returns for a discharge (`.design/verified/
/// proof-backends.md` REQ-2(b)). The strict mapping discipline (REQ-3): a
/// tactic/solver failure without a witnessing input is [`Verdict::Unknown`], not
/// [`Verdict::Refuted`]; refutation requires a genuine countermodel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The engine proved the obligation (for all inputs at a sound-for-all-inputs
    /// engine) → certify at the engine's level (L3 for Verus); attach evidence.
    Proven(Evidence),
    /// The engine disproved the obligation with a witnessed countermodel → hard
    /// fail, never degrades (REQ-3 anti-cheat).
    Refuted(Counterexample),
    /// The engine could not decide (a timeout, a tactic-battery exhaustion, or an
    /// SMT incompleteness-`unknown`) → degrade per the ladder (REQ-3). Not a
    /// failure verdict.
    Unknown(Reason),
}

/// The content-addressed evidence key (`.design/verified/proof-backends.md`
/// REQ-2(d) / §2(d)): the shipped `cache::cache_key` generalized with the engine
/// discriminator so a Verus proof and a future Lean proof of the same item never
/// collide. Increment (i) composes the shipped five-input verus key (lowered
/// source + seed + verus version + thermite version + `CHECK_SCHEMA_VERSION`) with
/// the engine tag; the Lean analogs (toolchain rev + targeted-spine hash) are
/// increment (ii) (the field is the seam the future Lean engine widens).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey {
    /// The engine discriminator (§2(d): the engine name is the new discriminator).
    pub engine: EngineName,
    /// The shipped content-addressed key (`cache::cache_key`'s sha256 hex). The
    /// per-engine version axes are folded in by the engine (the Verus engine uses
    /// the verus-version slot; the Lean engine widens it in increment (ii)).
    pub content_address: String,
}

/// The construct/class fragment an engine can attempt (`.design/verified/
/// proof-backends.md` REQ-2(a)). For Verus this is the whole frozen subset
/// reachable via the lowering (`thermite_lower::lower` + `run_verus`), including
/// the [`crate::obligation::ObligationClass::RegistryTermination`] class (its
/// dec-check is the common discharge path, REQ-1.2(a)). The predicate is on the
/// obligation class; a future engine narrows it (the Lean-auto engine admits only
/// the specCall-free QF fragment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    /// `true` iff this engine admits every obligation class (the Verus whole-subset
    /// case). A narrower engine (Lean-auto) sets `false` and narrows `admits`.
    pub admits_all_classes: bool,
}

impl Fragment {
    /// Does this fragment admit the given obligation? (`.design/verified/
    /// proof-backends.md` REQ-2(a), which obligation classes this engine can
    /// attempt.) The whole-subset Verus fragment admits everything; the predicate
    /// is the seam a narrower future engine keys on.
    #[must_use]
    pub fn admits(&self, _o: &Obligation) -> bool {
        self.admits_all_classes
    }
}

/// The named trust base an engine adds when it says `Proven` (`.design/verified/
/// proof-backends.md` REQ-2(c)). An enumerated set of named items so an auditor
/// sees L3-via-Lean enumerates a smaller base than L3-via-Verus (the §1 enumerable
/// trusted base). For Verus: {Z3, Verus VC-gen} + the TV/lowering theorem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustProfile {
    /// The enumerated named trust items (e.g. `["Z3", "Verus VC-gen", "TV/lowering
    /// theorem (lowering_faithful)"]`).
    pub items: Vec<String>,
}

/// A proof engine behind the backend-neutral interface (`.design/verified/
/// proof-backends.md` REQ-2). The four slots: fragment (what it can attempt),
/// discharge (the verdict, under REQ-3's mapping discipline), trust profile (the
/// named base added on `Proven`), evidence (the replayable cache key). Increment
/// (i) ships the Verus instance ([`VerusEngine`]); the Lean engine is increment
/// (ii).
pub trait Engine {
    /// The engine's name (the evidence-key discriminator and diagnostics).
    fn name(&self) -> EngineName;

    /// (a) Fragment: which obligation classes / constructs this engine attempts.
    fn fragment(&self) -> Fragment;

    /// (b) Discharge: the verdict for an obligation, under REQ-3's discipline (a
    /// solver/tactic failure without a witnessing input is `Unknown`, never
    /// `Refuted`).
    ///
    /// `covenant` is the item's covenant record (REQ-4, `.design/stage1-forge-tier.md`):
    /// a NON-OPTIONAL parameter so the proof-search path cannot be entered without one
    /// (covenant-before-burn is a type-level seam, not a runtime convention). Today every
    /// program carries [`CovenantRecord::none`] (the `witness` surface syntax is REQ-3,
    /// not yet present), so the record is inert here; the covenant LOGIC (inhabit/falsify,
    /// the before-burn enforcement, the `CovenantRefuted` hard fail) is increment 2b,
    /// which fills it in at this seam without re-touching any call site.
    fn discharge(&self, o: &Obligation, covenant: &CovenantRecord) -> Verdict;

    /// (c) Trust profile: the named base added when this engine says `Proven`.
    fn trust_profile(&self) -> TrustProfile;

    /// (d) Evidence: the replayable, cacheable key (generalizes
    /// `cache::cache_key` with the engine discriminator, §2(d)).
    fn evidence_key(&self, o: &Obligation) -> CacheKey;
}

/// The Verus/Z3 engine, refactored byte-identically behind [`Engine`] except the
/// one named REQ-3.1 fast-unknown remap (`.design/verified/proof-backends.md`
/// REQ-2, AC-2). It does not spawn verus itself; `check.rs` owns the
/// `run_verus`/cache machinery (the §0.1 vacuity/mutation/strengthen meta-queries
/// stay direct verus calls outside this engine, the v1 boundary). This engine's job
/// is the verdict mapping and the four-slot profile: `check.rs` runs the shipped
/// `run_verus`, hands the engine the resulting `VerusOutcome`, and the engine maps
/// it to a [`Verdict`] (with the REQ-3.1 split). The engine carries the verdict
/// policy; `check.rs` carries the I/O. This keeps the refactor byte-identical (the
/// same `run_verus` bytes, the same cache) while routing the verdict through the
/// trait.
#[derive(Debug, Clone, Copy, Default)]
pub struct VerusEngine;

impl VerusEngine {
    /// Map a shipped [`crate::check::VerusOutcome`] to a backend-neutral [`Verdict`]
    /// (REQ-2(b) / REQ-3.1). The verdict policy: the shipped three-way
    /// `classify_verus_outcome` map lifted to `Verdict`, with the one named REQ-3.1
    /// remap on the `Counterexample` arm.
    ///
    /// - `Proved` → [`Verdict::Proven`] (with the discharged-count evidence + key).
    /// - `Timeout` → [`Verdict::Unknown`] (`VerusTimeout`) → degrade (unchanged).
    /// - `Counterexample` split by [`counterexample_is_incompleteness_unknown`]:
    ///   - the genuine SMT-`unknown` signature (span-less, no frontend `error[E…]`,
    ///     an explicit `unknown` signal) → [`Verdict::Unknown`] (`IncompleteUnknown`)
    ///     → degrade (the REQ-3.1 delta: today this hard-fails; behind the interface
    ///     it degrades, matching §6's degrade-on-incompleteness intent);
    ///   - everything else (a witnessed countermodel with a parsed `--> span`, or a
    ///     frontend type error `error[E…]` like the provenance E0308) → [`Verdict::
    ///     Refuted`] (hard fail, byte-identical to today: a real bug or rejection
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
                    // The REQ-3.1 remap: only the genuine SMT-incompleteness
                    // `unknown` edge (a witness-less failure carrying the explicit
                    // SMT-`unknown` signature, no frontend/type error) degrades.
                    // Refutation requires a witnessing input; an incompleteness
                    // `unknown` means the solver could not decide, not a disproof.
                    let detail = obligations
                        .first()
                        .and_then(|o| o.diagnostic.clone())
                        .unwrap_or_else(|| {
                            "verus returned an SMT-incompleteness `unknown`".to_string()
                        });
                    Verdict::Unknown(Reason::IncompleteUnknown(detail))
                } else {
                    // Everything else stays `Refuted` → hard fail, byte-identical
                    // to the shipped pipeline (`Counterexample → ladder_action_l3
                    // HardFail`). This covers a `postcondition not satisfied`
                    // countermodel and a frontend rejection (a type error `error[E…]`,
                    // e.g. the IFC un-typeable `careless_query` E0308 the provenance
                    // corpus pins at L0). The remap is inert on the corpus: only the
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
        // The Verus engine admits the whole frozen subset reachable via the
        // lowering: every obligation class, including RegistryTermination (its
        // dec-check is the common discharge path, REQ-1.2(a)).
        Fragment {
            admits_all_classes: true,
        }
    }

    fn discharge(&self, o: &Obligation, covenant: &CovenantRecord) -> Verdict {
        // The trait `discharge` is the obligation-level entry. `check.rs` owns the
        // `run_verus` I/O for the per-item L3 path (it already has the lowered
        // source + cache wired), so the live discharge goes through
        // `VerusEngine::verdict_of` from there. Here, an obligation the Verus
        // fragment does not admit is an `Unknown` (it could not be attempted;
        // REQ-3: never a `Refuted` without a witness, never a false `Proven`).
        // Because `fragment().admits_all_classes` is `true` this never fires for
        // Verus today; it is the REQ-3-compliant default for a future narrowed
        // fragment (no `Proven`, no `Refuted`).
        let _ = o;
        // REQ-4 seam: the covenant record is threaded but inert in the foundation
        // (covenant-before-burn enforcement + the falsify producer are 2b).
        let _ = covenant;
        Verdict::Unknown(Reason::IncompleteUnknown(
            "the Verus engine discharges per-item obligations through \
             check::ladder_for_timeout (the run_verus path); a bare trait discharge \
             with no run is undecided (REQ-3: never a witness-less Refuted)"
                .to_string(),
        ))
    }

    fn trust_profile(&self) -> TrustProfile {
        // REQ-2(c) trust profile: {Z3, Verus VC-gen} + the TV/lowering theorem
        // (`lowering_faithful`, relative to {Z3 soundness, S = intended meaning,
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
        // REQ-2(d): the engine-discriminated key. The content side is derived from
        // the obligation's item + class + role tags (a stable, prover-neutral
        // address); the live per-item path supplies the richer lowered-source key
        // (the shipped `cache::cache_key`) via `engine_cache_key`, so a hit is a
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

/// The default engine ordering (`.design/verified/proof-backends.md` REQ-8): Verus
/// first (fast, push-button). Increment (i) wires the ordering hook with the single
/// Verus rung; increment (ii) adds the Lean-auto / Lean-interactive rungs after
/// Verus, then the existing L2/L1 degrade. Returns the ordered engine names so the
/// caller (`check::ladder_for_timeout`) reads the first (Verus) rung.
#[must_use]
pub fn default_engines() -> Vec<EngineName> {
    // Increment (i): Verus only. The Lean rungs (REQ-8 "Lean-auto second,
    // Lean-interactive on demand") are increment (ii), appended here when the Lean
    // engine lands, before the existing L2/L1 degrade.
    vec![EngineName::Verus]
}

/// Build the engine-discriminated evidence key for the live per-item L3 path
/// (`.design/verified/proof-backends.md` REQ-2(d) / §2(d)). Composes the shipped
/// content-addressed `cache::cache_key` hex (over lowered source, seed, verus
/// version, thermite version, and `CHECK_SCHEMA_VERSION`) with the engine
/// discriminator, so a Verus proof and a future Lean proof of the same lowered
/// source never collide.
/// This is the key the engine attaches to its `Proven` evidence; the shipped
/// `cache::load`/`store` still serve/persist the cert under the shipped hex key
/// (the engine tag is the additive §2(d) discriminator the future Lean engine
/// keys on; it does not change the shipped verus cache address, so the corpus
/// cache hits are unperturbed).
#[must_use]
pub fn engine_cache_key(engine: EngineName, content_address: String) -> CacheKey {
    CacheKey {
        engine,
        content_address,
    }
}

/// The Lean schema version (`.design/verified/proof-backends.md` REQ-8 / §2(d)).
/// Bumped when the exporter's emitted-source shape or the obligation→Lean encoding
/// changes (the analogue of `cache::CHECK_SCHEMA_VERSION` for the Lean engine), so a
/// cached Lean `Proven` is invalidated when the exporter logic changes; a hit is a
/// fresh verify against the current exporter + spine.
pub const LEAN_SCHEMA_VERSION: u32 = 2;

/// A process-local monotonic nonce for unique replay scratch-file names
/// (`.design/verified/proof-backends.md` REQ-7(ii); the interactive replay writes a
/// `#print axioms` probe to a temp file). Keyed alongside the pid + item name so
/// concurrent replays in the same process never collide on the scratch path (the
/// collision that flaked `interactive_filled_valid_proof_replays_proven` under
/// parallel test runs). Deterministic per call (R-CODE-5).
static NEXT_REPLAY_NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The Thermite→Lean obligation engine (`.design/verified/proof-backends.md` REQ-6/
/// REQ-7/REQ-8, engine #2; increment (ii-b), the #240 chain). Implements the
/// [`Engine`] trait: `fragment()` = pure-contract items whose constructs are
/// spine-exportable; `discharge()` = export → write to a scratch dir → `lake env
/// lean <file>` (cwd `lean/`) → kernel accept = [`Verdict::Proven`], tactic
/// failure / timeout / lake absent / tier-(c) interactive = [`Verdict::Unknown`]
/// (never [`Verdict::Refuted`]: a Lean tactic failure is not a witnessed
/// countermodel, REQ-3 anti-cheat); `trust_profile()` = {Lean kernel + 3 standard
/// axioms, EXP}; `evidence_key()` = obligation content + lean-toolchain content +
/// lake-manifest revs + a `lean/Thermite/` spine content hash + `LEAN_SCHEMA_VERSION`
/// (REQ-8 / §2(d)).
///
/// The engine carries the parsed [`Program`] (the exporter needs the spec-fn
/// definitions + the source item) and the path to the `lean/` package root (where
/// `lake env lean` runs). It is not wired into the default `check` path; Verus stays
/// the sole default engine (byte-identical), and this engine is constructed directly
/// by tests and (increment (iii)) the `--engine lean` surface.
#[derive(Debug, Clone)]
// proof-backends REQ-6/REQ-7/REQ-8 (the #240 chain): the Lean engine #2. The
// increment-(iii) production surface (#247) is live: `check::check_file_with_engine`
// (the `--engine lean|auto` dispatch) and `check::lean_mutation_score` construct it;
// the four-slot `Engine` impl is verified live in `forge/tests/lean_engine.rs`.
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
    /// root (`.design/verified/proof-backends.md` REQ-6). Production consumers:
    /// `check::check_file_with_engine` (`--engine lean|auto`, #247) and
    /// `check::lean_mutation_score` (the REQ-9 Lean mutation path).
    #[must_use]
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
    /// source + tier; an [`ExportRefusal`] means the fragment does not admit this
    /// obligation (a skip; the engine maps it to `Unknown`, never `Refuted`).
    fn export(&self, o: &Obligation) -> Result<ExportedObligation, ExportRefusal> {
        let item = find_item(&self.program, &o.item).ok_or_else(|| {
            ExportRefusal::OutOfFragment(format!("item `{}` not found in the program", o.item))
        })?;
        export_item(o, &self.program, item)
    }

    /// Locate the `lake` binary (`.design/verified/proof-backends.md` REQ-6, "locate
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
    /// → [`Verdict::Unknown`] (never `Refuted`: a Lean tactic failure is not a
    /// witnessed countermodel, REQ-3); lake absent (`ENOENT`) → `Unknown`. The
    /// `key` is the engine's evidence key (attached to a `Proven`).
    fn run_lake(
        &self,
        file: &std::path::Path,
        item: &str,
        source: &str,
        verified: u64,
        key: CacheKey,
    ) -> Verdict {
        use std::process::Command;
        let lake = Self::lake_binary();
        let output = Command::new(&lake)
            .arg("env")
            .arg("lean")
            .arg(file)
            .current_dir(&self.lean_root)
            .output();
        match output {
            Ok(out) if out.status.success() => {
                // REQ-2 / AC-5 certify-time axiom gate, HOISTED onto the auto path: a
                // clean lake exit is necessary but NOT sufficient for `Proven`. The
                // emitted source carries a `#print axioms <obligation theorem>` probe
                // (appended in `discharge`); the SAME gate the interactive replay runs
                // ([`certify_lean_axioms`]) checks the obligation theorem is sorry-free
                // and rests only on the allowlisted axioms. A surviving `sorry` or a
                // smuggled axiom downgrades to `Unknown` (an honest skip — the cert's
                // enumerable trusted base cannot be vouched for), NEVER `Proven`.
                let probe_out = String::from_utf8_lossy(&out.stdout);
                match certify_lean_axioms(source, &probe_out, item) {
                    Ok(()) => Verdict::Proven(Evidence { verified, key }),
                    Err(reason) => Verdict::Unknown(Reason::IncompleteUnknown(format!(
                        "lake kernel-accepted the obligation but the certify-time axiom \
                         gate REFUSED it (REQ-2/AC-5): {reason}"
                    ))),
                }
            }
            Ok(out) => {
                // A non-zero exit is a tactic/elaboration failure: the engine could
                // not kernel-check the theorem. This is `Unknown` (degrade), not
                // `Refuted`, because there is no witnessing input (REQ-3 anti-cheat: a
                // Lean tactic failure is not a countermodel). R-6(a): `lean` writes its
                // diagnostics (`unsolved goals`, elaboration errors) to stdout, not
                // stderr, so the Unknown reason carries both streams or it loses the
                // diagnostic that explains why the obligation did not discharge.
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let detail = if stderr.trim().is_empty() {
                    stdout.into_owned()
                } else if stdout.trim().is_empty() {
                    stderr.into_owned()
                } else {
                    format!("{stdout}\n{stderr}")
                };
                let head: String = detail.chars().take(400).collect();
                Verdict::Unknown(Reason::IncompleteUnknown(format!(
                    "lake/lean did not kernel-accept the exported obligation (tactic \
                     failure / elaboration error — NOT a countermodel, REQ-3): {head}"
                )))
            }
            Err(e) => {
                // lake absent / spawn failure: `Unknown` (the engine could not run),
                // never `Refuted`. R-CODE-4: the subprocess failure is surfaced
                // structured, not swallowed as success.
                Verdict::Unknown(Reason::IncompleteUnknown(format!(
                    "could not invoke `lake env lean` (lake absent or un-spawnable): {e}"
                )))
            }
        }
    }

    /// A content hash of the `lean/Thermite/` spine the exported theorem
    /// instantiates (`.design/verified/proof-backends.md` §2(d), the targeted-spine
    /// content hash). Walks `lean/Thermite/**` recursively (the #246 widening: the
    /// non-recursive walk left `lean/Thermite/Exec/**` unhashed, so an Exec-subtree
    /// edit kept the same key; increment (iv) targets Exec, so the spine hash must
    /// cover the whole subtree). Files are content-addressed by their path relative to
    /// the spine root (so a moved/renamed file changes the key) and sorted for
    /// determinism (R-CODE-5). On a read error the digest degrades to a marker (never
    /// a panic, R-CODE-2).
    fn spine_content_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let spine_dir = self.lean_root.join("Thermite");
        let mut entries: Vec<PathBuf> = Vec::new();
        if !Self::collect_lean_files(&spine_dir, &mut entries) {
            return "spine-unreadable".to_string();
        }
        // Sort by the path relative to the spine root (stable across cwd; covers the
        // whole recursive subtree).
        entries.sort_by(|a, b| {
            let ra = a.strip_prefix(&spine_dir).unwrap_or(a);
            let rb = b.strip_prefix(&spine_dir).unwrap_or(b);
            ra.cmp(rb)
        });
        let mut hasher = Sha256::new();
        // The marker is bumped to v2 with the recursive widening, so a prior cached
        // key (non-recursive v1) universally misses.
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
    /// Returns `false` if the root directory is unreadable (the degrade-to-marker
    /// signal); a subdirectory read error is skipped (best-effort, never a panic,
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

    /// The obligation-content hash (`.design/verified/proof-backends.md` §2(d) /
    /// REQ-7(ii), the #246 fix): the canonical emitted Lean terms for `req`/`ens`/
    /// `body`/`dec` plus the registry bodies — i.e. the exporter's rendered source,
    /// which contains those terms. Hashing the rendered source means editing
    /// `ens result >= a` to `ens result >= b` (or editing a reached spec-fn's body)
    /// changes the key (the staleness REQ-7(ii) demands; a cached/replayed `Proven`
    /// never silently survives a contract change). On an export refusal (the item
    /// is not exportable) the content degrades to a structured refusal marker (still a
    /// stable, content-distinguishing string; a refused item never reaches a cached
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
    /// proof-backends.md` §2(d), the engine-toolchain version is the
    /// `lean-toolchain` rev + the `lake-manifest` revs). Reads the two files;
    /// missing files degrade to a marker (never a panic, R-CODE-2).
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
        // The Lean engine admits only the pure-contract class whose constructs the
        // exporter can emit (the `admits_all_classes = false` seam REQ-2(a) named for
        // a narrowed engine). The per-obligation admission is decided by `export`
        // succeeding (in `discharge`); the fragment flag marks it as a narrowed
        // engine so the ladder hook knows to gate on `admits` (which runs the full
        // export attempt).
        Fragment {
            admits_all_classes: false,
        }
    }

    fn discharge(&self, o: &Obligation, covenant: &CovenantRecord) -> Verdict {
        // REQ-4 seam: the covenant record is threaded but inert in the foundation
        // (covenant-before-burn enforcement + the falsify producer are 2b).
        let _ = covenant;

        // 1. Export. A refusal (out-of-fragment / not-pure-contract / incomplete
        //    registry / open hole) = the fragment does not admit this obligation →
        //    `Unknown` (a skip), never `Refuted`/`Proven` (REQ-3 anti-cheat: a skip
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

        // 2. Tier-(c) is interactive-only: the engine does not invoke lake (the
        //    `∃N∀fuel` form needs an authored induction). Return `Unknown` without
        //    running lake (the file may still be emitted for increment-(iii) use).
        if !exported.tier.is_auto() {
            return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                "tier ({}) recursive-registry obligation is INTERACTIVE-only \
                 (the auto battery does not attempt it; REQ-7(ii)) — Lean-auto SKIPs",
                exported.tier.tag()
            )));
        }

        // 3. Append the obligation theorem's `#print axioms` probe so the auto path runs
        //    the SAME certify-time axiom gate the interactive replay runs (REQ-2 / AC-5:
        //    the gate is hoisted onto EVERY Lean discharge path, not just interactive
        //    replay). Without this, a clean lake exit was certified `Proven` with NO
        //    axiom check on the auto tiers — a smuggled axiom / surviving `sorry` would
        //    not be caught. The probe anchors on the obligation theorem's exact name.
        let thm_name = format!("thermite_obligation_{}", proof_thm_sanitize(&o.item));
        let probed_source = format!("{}\n\n#print axioms {thm_name}\n", exported.source);

        // 4. Write the (probed) source to a scratch file and invoke lake, gating the
        //    `Proven` on the axiom report.
        let scratch = match self.write_scratch_source(o, &probed_source) {
            Ok(p) => p,
            Err(e) => {
                return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                    "could not write the exported Lean obligation to a scratch file: {e}"
                )));
            }
        };
        let key = self.evidence_key(o);
        let verdict = self.run_lake(&scratch, &o.item, &probed_source, 1, key);
        // Best-effort scratch cleanup (R: clean up scratch). A cleanup failure does
        // not change the verdict.
        let _ = std::fs::remove_file(&scratch);
        verdict
    }

    fn trust_profile(&self) -> TrustProfile {
        // REQ-2(c) trust profile: {Lean kernel + 3 standard axioms} + EXP (the
        // exporter correspondence). An auditor sees this enumerates a smaller base
        // than Verus's {Z3, Verus VC-gen, lowering theorem} along the named axes
        // (§1 / REQ-4, "smaller along the named axes", OQ-3).
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
        // content hash, and the LEAN_SCHEMA_VERSION, so a toolchain or spine bump
        // forces a universal miss (a hit is a fresh verify against the current
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
        // The obligation content (#246 / REQ-7(ii)): the canonical emitted Lean terms
        // for req/ens/body/dec + the registry bodies (the rendered exporter source).
        // Two same-named items with different ens (or a reached spec-fn body edit) →
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
    /// Discharge a ready-made exported Lean SOURCE that was NOT minted from an
    /// [`Obligation`] — the forge-tier `lemma` path (`.design/stage1-forge-tier.md`
    /// REQ-7, increment 2e). The lemma exporter ([`crate::lean_export::export_lemma`])
    /// produces a self-contained file (preamble + `R_item` + the
    /// `thermite_obligation_<lemma>` theorem proved by the author's frozen-battery
    /// tactics); this appends the SAME certify-time axiom probe (`#print axioms
    /// thermite_obligation_<lemma>`) the auto [`Engine::discharge`] path appends, writes
    /// a scratch file, runs lake, and gates `Proven` on the axiom report via the SAME
    /// [`certify_lean_axioms`] gate (a surviving `sorry` / smuggled axiom / lake failure
    /// is an honest `Unknown`, never `Proven`). Lean-absent → `Unknown` (a skip). The
    /// evidence key binds the source + toolchain + spine content so a bump forces a
    /// miss.
    pub(crate) fn discharge_source(&self, source: &str, item: &str) -> Verdict {
        use sha2::{Digest, Sha256};
        let thm_name = format!("thermite_obligation_{}", proof_thm_sanitize(item));
        let probed_source = format!("{source}\n\n#print axioms {thm_name}\n");

        let mut h = Sha256::new();
        h.update(b"thermite-lean-lemma-evidence-v1");
        h.update(item.as_bytes());
        h.update(probed_source.as_bytes());
        h.update(self.toolchain_rev().as_bytes());
        h.update(self.spine_content_hash().as_bytes());
        let content_address: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
        let key = CacheKey {
            engine: self.name,
            content_address,
        };

        let pid = std::process::id();
        let nonce = NEXT_REPLAY_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let safe = proof_thm_sanitize(item);
        let scratch =
            std::env::temp_dir().join(format!("forge_lean_lemma_{pid}_{safe}_{nonce}.lean"));
        if let Err(e) = std::fs::write(&scratch, &probed_source) {
            return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                "could not write the exported forge-tier lemma to a scratch file: {e}"
            )));
        }
        let verdict = self.run_lake(&scratch, item, &probed_source, 1, key);
        let _ = std::fs::remove_file(&scratch);
        verdict
    }

    /// Write the exported Lean source to a deterministic scratch file in the system
    /// temp dir (`.design/verified/proof-backends.md` REQ-7, "export → write to a
    /// scratch dir"). The file name is keyed on the item + the process id so
    /// concurrent runs do not collide. Returns the path; the caller invokes lake on
    /// it and removes it after.
    fn write_scratch_source(&self, o: &Obligation, source: &str) -> std::io::Result<PathBuf> {
        let safe: String = o
            .item
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("forge_lean_export_{safe}_{pid}.lean"));
        std::fs::write(&path, source)?;
        Ok(path)
    }

    /// Does the Lean fragment admit this obligation for an auto discharge?
    /// (`.design/verified/proof-backends.md` REQ-9, the "untested against lean"
    /// boundary.) The Lean fragment's admission is not the static `admits_all_classes`
    /// flag (it is `false`); it is whether the obligation exports and lands in an auto
    /// tier (a)/(b). A refusal (out-of-spine / not-pure-contract / incomplete registry
    /// / non-int result) or a tier-(c) interactive obligation is not admitted by the
    /// auto path — "untested against lean" (REQ-9), never a kill. This runs the export
    /// (the same one `discharge` runs), so it is the genuine per-mutant admission gate.
    #[must_use]
    pub fn admits_auto(&self, o: &Obligation) -> bool {
        matches!(self.export(o), Ok(e) if e.tier.is_auto())
    }

    /// Run the REQ-6a arbitrary-result re-elaboration tautology check on an obligation
    /// (`.design/stage1-forge-tier.md` REQ-6 / AC-10, increment 2d — anti-Goodhart
    /// defense (a)). The L3 counterpart of `vacuity_solver.rs::build_tautology_harness`:
    /// it exports the SAME obligation with `result` bound to a fresh universally-
    /// quantified `(r : Int)` instead of the body denotation
    /// ([`crate::lean_export::export_arbitrary_result_harness`]), then DRIVES THE
    /// EXISTING discharge path — the same `#print axioms` probe + [`Self::run_lake`] a
    /// normal obligation uses (no new elaborator, per the substrate note).
    ///
    /// The polarity mirrors the Verus harness exactly:
    /// - lake kernel-accepts the harness ([`Verdict::Proven`]) → the `ens` holds for an
    ///   ARBITRARY result, so the contract says nothing about what the body computes: a
    ///   body-ignoring **tautology** → [`ArbitraryResultOutcome::Tautology`] (reject).
    /// - lake fails to elaborate (an elaboration/tactic failure, not a countermodel) →
    ///   the `ens` genuinely constrains the result → [`ArbitraryResultOutcome::Clean`].
    ///   A true tautology the auto battery cannot close is a missed detection (the SAFE
    ///   completeness gap — never an unsound false reject), exactly as the Verus harness.
    /// - the harness is not exportable on the auto path (a refusal / a tier-(c)
    ///   recursive obligation / lake absent / the axiom gate) → [`ArbitraryResultOutcome::
    ///   Skipped`] — the check could not run, so it NEVER rejects (the item keeps its
    ///   real proof; the tautology gate is an additional layer, not a replacement).
    #[must_use]
    pub fn arbitrary_result_reelaboration(&self, o: &Obligation) -> ArbitraryResultOutcome {
        let item = match find_item(&self.program, &o.item) {
            Some(i) => i,
            None => {
                return ArbitraryResultOutcome::Skipped(format!(
                    "item `{}` not found in the program",
                    o.item
                ));
            }
        };
        // Export the arbitrary-result harness (same registry / req / ens as the real
        // obligation; only `result` is the fresh `(r : Int)` binder). A refusal is an
        // honest skip (not exportable on the auto path), never a reject.
        let harness =
            match crate::lean_export::export_arbitrary_result_harness(o, &self.program, item) {
                Ok(e) => e,
                Err(refusal) => {
                    return ArbitraryResultOutcome::Skipped(format!(
                        "the arbitrary-result harness is not exportable on the auto path \
                     (an honest skip, not a verdict): {refusal}"
                    ));
                }
            };
        // Tier-(c) recursive obligations are interactive-only — the auto battery does
        // not attempt them, so the tautology check is "untested" (Skipped), never a
        // reject (mirrors `discharge`'s tier-(c) skip).
        if !harness.tier.is_auto() {
            return ArbitraryResultOutcome::Skipped(format!(
                "tier ({}) recursive obligation is interactive-only; the arbitrary-result \
                 re-elaboration is the auto path only",
                harness.tier.tag()
            ));
        }
        // Append the obligation theorem's `#print axioms` probe (the same gate the real
        // discharge runs) and drive lake exactly as a normal obligation.
        let thm_name = format!("thermite_obligation_{}", proof_thm_sanitize(&o.item));
        let probed = format!("{}\n\n#print axioms {thm_name}\n", harness.source);
        let pid = std::process::id();
        let nonce = NEXT_REPLAY_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let safe = proof_thm_sanitize(&o.item);
        let scratch =
            std::env::temp_dir().join(format!("forge_lean_taut_{safe}_{pid}_{nonce}.lean"));
        if let Err(e) = std::fs::write(&scratch, &probed) {
            return ArbitraryResultOutcome::Skipped(format!(
                "could not write the arbitrary-result harness scratch file: {e}"
            ));
        }
        let key = self.evidence_key(o);
        let verdict = self.run_lake(&scratch, &o.item, &probed, 1, key);
        let _ = std::fs::remove_file(&scratch);
        match verdict {
            // The harness kernel-accepted for an ARBITRARY result → body-ignoring ens.
            Verdict::Proven(_) => ArbitraryResultOutcome::Tautology,
            // A genuine elaboration/tactic failure (the auto battery could not close the
            // arbitrary-result goal) → the ens constrains the result → clean. An env /
            // spawn / axiom-gate condition is a skip (the check could not run), never a
            // claim of clean (R-CODE-4: an undetermined run is not read as a verdict).
            Verdict::Unknown(Reason::IncompleteUnknown(detail))
                if detail.contains("did not kernel-accept") =>
            {
                ArbitraryResultOutcome::Clean
            }
            Verdict::Unknown(reason) => ArbitraryResultOutcome::Skipped(format!(
                "the arbitrary-result harness run was inconclusive (env/spawn/axiom-gate, \
                 not an elaboration failure): {}",
                match reason {
                    Reason::VerusTimeout(d) | Reason::IncompleteUnknown(d) => d,
                }
            )),
            // The Lean engine never produces a witnessed `Refuted` (a tactic failure is
            // Unknown, not a countermodel, REQ-3) — defensively a skip, never a reject.
            Verdict::Refuted(_) => ArbitraryResultOutcome::Skipped(
                "the arbitrary-result harness returned a (spurious) Refuted — treated as \
                 inconclusive (the Lean engine maps tactic failures to Unknown)"
                    .to_string(),
            ),
        }
    }

    /// Replay (or emit) a tier-(c) item's interactive proof artifact
    /// (`.design/verified/proof-backends.md` REQ-7(ii) / §6 tier (c)). The artifact
    /// lives at [`interactive_proof_path`] (`<source_file>.lean-proofs/<item>.lean`):
    ///
    /// - Absent: emit the skeleton (the exporter's tier-(c) source + the
    ///   evidence-key header) and return `Unknown` ("skeleton emitted — an agent
    ///   authors the induction"). A skeleton is never `Proven` (it carries `sorry`).
    /// - Present: the staleness gate: the emitted `-- evidence_key: <hex>` header
    ///   must match the current [`evidence_key`](Engine::evidence_key). A mismatch =
    ///   stale → `Unknown("stale proof — re-derive")` (never silently reused). A match
    ///   → replay via `lake env lean`; then detect `sorry` explicitly ([`proof_has_sorry`]
    ///   over the source and `#print axioms`); a `sorry` → `Unknown` (never `Proven`,
    ///   even though lake exits 0 on a `sorry`); a kernel-accepted, sorry-free replay →
    ///   `Proven` with the interactive trust profile (the `verified` count is 1, the
    ///   one item obligation).
    ///
    /// `source_file` is the `.th` source the artifact is checked in beside; `o` is the
    /// tier-(c) obligation. Never a panic (R-CODE-2); subprocess failures surfaced
    /// (R-CODE-4); deterministic given the filesystem + toolchain (R-CODE-5).
    pub fn replay_interactive(&self, source_file: &std::path::Path, o: &Obligation) -> Verdict {
        // The exporter's tier-(c) source (the skeleton body). A refusal means the item
        // is not exportable → a skip (Unknown), never a verdict.
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

        // The canonical exporter source, regenerated from the current obligation (the
        // generator-controlled preamble/imports + `R_item` + the canonical
        // `theorem thermite_obligation_<item> : <statement> := …` line). The replay
        // reconstructs a fresh replay file from this canonical source, splicing in only
        // the author's extracted proof term (proof-backends REQ-6 / R-DEFER-9, the #252
        // helper-surface elimination: the author file content outside the proof term is
        // dropped). The statement, name, and `#print axioms` target are then the same
        // generator-emitted declaration by construction, so a same-short-name decoy is
        // structurally impossible.

        // Present → the staleness gate + reconstruct-and-splice replay; absent → emit
        // the skeleton.
        match std::fs::read_to_string(&path) {
            Ok(existing) => {
                self.replay_present_proof(&path, &existing, &key, &o.item, &exported.source)
            }
            Err(_) => {
                // Absent: emit the skeleton (header + the tier-(c) exported source).
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

    /// The present-proof arm of [`replay_interactive`]: the staleness gate →
    /// reconstruct-and-splice → replay → sorry detection. Split out so the read/write
    /// I/O stays in the caller.
    ///
    /// The replay does not validate the author's file by pattern-matching (the #250
    /// decoy game: a same-short-name decoy theorem that the appended `#print axioms`
    /// probe resolves to while the statement-binding gate reads a different, namespaced
    /// declaration). It reconstructs a fresh, fully generator-controlled replay
    /// file from `canonical_source` (the exporter's preamble/imports + `R_item` + the
    /// canonical `theorem thermite_obligation_<item> : <statement> := …` line),
    /// splicing in only the author's extracted proof term (the #252 helper-surface
    /// elimination: the author file content outside the proof term is dropped, never
    /// spliced; auxiliary lemmas inline as `have`/`let`/`suffices`). The statement, the
    /// theorem name, and the `#print axioms` probe target are then the same
    /// generator-emitted declaration by construction, so a decoy is structurally
    /// impossible. A smuggled axiom used by the proof appears in the anchored dependency
    /// report (the allowlist catches it); an unused decoy axiom is inert.
    fn replay_present_proof(
        &self,
        path: &std::path::Path,
        existing: &str,
        key: &CacheKey,
        item: &str,
        canonical_source: &str,
    ) -> Verdict {
        // The staleness gate (REQ-7(ii)): the header's evidence key must match the
        // current key. A mismatch = the obligation / toolchain / spine changed → the
        // proof is stale and must be re-derived, never silently reused. This stays on
        // the author's file (the header the author kept from the emitted skeleton).
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

        // Reconstruct (proof-backends REQ-6 / R-DEFER-9, the #250 + #252 fixes): split the
        // canonical exporter source into (preamble, canonical theorem statement); extract
        // from the author's file only the unique `thermite_obligation_<item>` declaration's
        // proof term (the #252 helper-surface elimination: no author helpers are spliced);
        // emit a fresh replay file. A duplicate obligation declaration (the #250 decoy) →
        // reject; a missing canonical statement (malformed exporter output) → never trusted
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

        // The proof is fresh: replay the reconstructed file via lake and capture the
        // anchored `#print axioms <thm>` (already appended by `reconstruct_replay`) for
        // the explicit sorry check + the trust-base axiom allowlist (lake exits 0 on a
        // `sorry`, so the source/axioms scan is what distinguishes a genuine proof,
        // REQ-7(ii)). The probe target is the canonical declaration by construction.
        let probe = reconstructed;
        let pid = std::process::id();
        // A per-call nonce + the item name keeps the scratch path unique across
        // concurrent replays in the same process (the same pid); a shared
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
                // Explicit sorry detection (REQ-7(ii)): lake exits 0 on a `sorry`, so
                // a clean exit is not sufficient. Scan the source token and the
                // `#print axioms` output (which prints `sorryAx` for a surviving
                // `sorry`). A `sorry` is never `Proven`.
                let axioms = String::from_utf8_lossy(&out.stdout);
                if proof_has_sorry(&probe, &axioms) {
                    return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                        "the interactive proof `{}` carries an OPEN `sorry` (detected in the \
                         source and/or `#print axioms` — `sorryAx`); a `sorry` is NEVER Proven \
                         (REQ-7(ii)), even though lake exits 0",
                        path.display()
                    )));
                }
                // Trust-base axiom allowlist (proof-backends REQ-4 / REQ-7(ii) / §1 /
                // R-DEFER-9): the enumerable trusted base a Lean cert lists is
                // {Lean kernel + the 3 standard axioms, EXP[, author]}. `#print axioms`
                // reports the whole axiom set the kernel-accepted theorem rests on; any
                // axiom outside `{propext, Classical.choice, Quot.sound}` (a smuggled
                // `axiom thermite_cheat : ∀ p, p`, an oracle, …) means the cert's base
                // misstates what the proof rests on. Such a proof is never Proven (a
                // proof cheat), even though it kernel-accepts.
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
                        // No `#print axioms` report for the obligation theorem in the output:
                        // an author's own earlier `#print axioms <helper>` is never read in
                        // its place (the #249 marker-mask). Without the obligation's own axiom
                        // list the enumerable base cannot be vouched for; never Proven.
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
                // A kernel-accepted, sorry-free, allowlist-clean, statement-bound replay
                // → Proven with the interactive trust profile (the author is a reviewed
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

    /// Reconstruct a fresh, entirely generator-controlled replay file from the canonical
    /// exporter source + only the author's extracted proof term (`.design/verified/
    /// proof-backends.md` REQ-6 / R-DEFER-9, the #252 architectural fix that eliminates
    /// the author helper surface). The emitted file is, in order:
    ///
    /// 1. the canonical preamble (the exporter's header + `import` + `R_item` + the
    ///    resolution lemmas + the obligation theorem's doc comment), everything before the
    ///    canonical `theorem thermite_obligation_<item>` line, regenerated from the current
    ///    obligation (never trusted from the author);
    /// 2. the canonical theorem line `theorem thermite_obligation_<item> : <statement> :=`
    ///    with the author's extracted proof term (everything after the author declaration's
    ///    first `:=`) spliced after `:=`;
    /// 3. the anchored `#print axioms thermite_obligation_<item>` probe.
    ///
    /// The #252 architectural decision (after 5 bypass generations #248..#252). The replay
    /// file carries no author-controlled text other than the proof term. The earlier design
    /// spliced an author helpers section into the obligation's elaboration scope and tried
    /// to sanitize it with a command blocklist (`disallowed_helper_command`). A blocklist on
    /// a Turing-complete elaborator cannot be made sound: #251 closed column-0 commands;
    /// #252 escaped via indentation (Lean is whitespace-insensitive at the top level, so an
    /// indented `notation:max "Thermite.stabilizesProp" => (fun _ _ => True)` re-elaborates
    /// the byte-identical canonical statement to `True`); unicode-whitespace /
    /// comment-nesting / `open … in` variants would follow. Eliminating the helper surface
    /// is the sound fix: the author supplies only the proof term, which the
    /// kernel type-checks against the fixed, generator-emitted, already-elaborated goal type
    /// (the statement is left of `:=` and is generator-controlled). A proof term cannot
    /// vacate that goal; `sorry`/`admit` → `sorryAx` and `native_decide` → `ofReduceBool`
    /// are caught by the axiom allowlist. Auxiliary lemmas inline as `have`/`let`/`suffices`
    /// inside the proof term (Lean supports this in tactic + term mode; no expressivity loss
    /// for a single-obligation proof).
    ///
    /// Rejects (returns `Err(detail)`) when: the short name `thermite_obligation_<item>`
    /// occurs as a declaration more than once anywhere in the author's file (the #250
    /// decoy, "duplicate obligation declaration"); the canonical source has no extractable
    /// statement; the author's file has no obligation declaration to splice a proof from;
    /// the author's declared statement does not match the canonical one (modulo whitespace);
    /// or (the #252 belt) the extracted proof term carries a top-level command keyword in
    /// any position (an `… in`-style command form smuggled into the term). Deterministic
    /// (R-CODE-5); never a panic (R-CODE-2).
    fn reconstruct_replay(
        &self,
        canonical_source: &str,
        author_file: &str,
        item: &str,
    ) -> Result<String, String> {
        let thm_name = format!("thermite_obligation_{}", proof_thm_sanitize(item));

        // (1) The canonical preamble + the canonical statement, regenerated from the
        // exporter source (generator-controlled).
        let canonical_statement =
            canonical_theorem_statement(canonical_source, item).ok_or_else(|| {
                "the canonical exporter source has no extractable obligation theorem statement \
                 (a malformed exporter output — never trusted as a binding)"
                    .to_string()
            })?;
        // The preamble ends at the exact-name obligation theorem header (#268: an
        // exact-name anchor, never a prefix; a suffixed sibling `_entry`/`_converges` on
        // a multi-theorem while-shaped file must not latch the split).
        let preamble_end = theorem_anchor_pos(canonical_source, &thm_name).ok_or_else(|| {
            "the canonical exporter source declares no obligation theorem to anchor the \
             reconstruction on"
                .to_string()
        })?;
        let preamble = canonical_source[..preamble_end].trim_end();

        // (2) Uniqueness + the author's proof term. Count the obligation theorem's
        // short-name declaration sites in the author's file: more than one anywhere (any
        // namespace) → the #250 decoy → reject. Exactly one → splice its proof term.
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
        // The author's proof term: everything after the declaration's first `:=` up to
        // the declaration's end (the next top-level command/declaration, or EOF).
        let decl_end = decl_block_end(author_file, decl_start);
        let decl_text = &author_file[decl_start..decl_end];
        let assign = proof_assign_pos(decl_text).ok_or_else(|| {
            format!("the author's `{thm_name}` declaration has no `:=` proof term to splice")
        })?;
        let proof_term = decl_text[assign + ":=".len()..].trim_end();

        // Defense layer (proof-backends REQ-6, retained): the reconstruction forces the
        // canonical statement by construction, and we also cross-check that the author's
        // own declaration statement matches the canonical one (modulo whitespace). A
        // mismatch is a stale/wrong-statement author file; reject with a precise
        // diagnostic rather than silently overwriting their (different) statement.
        let author_statement = &decl_text[..assign + ":=".len()];
        if !statements_match(author_statement, &canonical_statement) {
            return Err(format!(
                "statement mismatch: the author's `{thm_name}` declaration proves \
                 `{author_statement}` but the current obligation's canonical statement is \
                 `{canonical_statement}` (the author fills ONLY the proof term after `:=`)"
            ));
        }

        // The #252 belt (proof-backends REQ-6 / §1 / R-DEFER-9): the proof term is the only
        // author-controlled text, and it is type-checked against the fixed generator-emitted
        // goal, so a proof term cannot vacate that goal. As a defense layer against an
        // `… in`-style top-level command form smuggled into the term, reject if the proof
        // term carries a top-level command keyword in any position (exact-token,
        // whitespace-independent). The author content outside the proof term is dropped
        // (never spliced); there is no helper surface to sanitize.
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
        // anchored probe. No author helper section exists; the only author-controlled text
        // is the proof term (the #252 elimination). Any author file content outside the
        // proof term (an indented `notation`, a file-level helper, …) is dropped; it has
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

/// The REQ-3 discharge discipline, generalized off the shipped ladder
/// (`.design/verified/proof-backends.md` REQ-3): map an engine's [`Verdict`] for a
/// `role = Certification` obligation to the shipped [`crate::degrade::L3Verdict`]
/// the ladder consumes. `Proven` → certify L3 (the carried cert); `Unknown` →
/// the `Timeout`-shaped degrade trigger (`run_ladder` → L2/L1); `Refuted` → the
/// `Counterexample` hard fail (never degrades). The §0.1 meta/battery queries
/// (`role` ≠ `Certification`) are outside this discipline; they are not minted as
/// `Obligation`s in v1, so this function is total over the `Certification` role.
///
/// `proved_cert` / `cx_cert` are the assembled certs the caller built from the
/// same outcome (the L3 cert on `Proven`, the counterexample cert on `Refuted`);
/// the `Unknown` arm carries the degrade `reason` onto the lower rung (REQ-4),
/// matching the shipped `VerusTimeout` reason shape.
#[must_use]
pub fn verdict_ladder_action(
    verdict: &Verdict,
    role: ObligationRole,
    proved_cert: crate::manifest::Certificate,
    cx_cert: crate::manifest::Certificate,
) -> crate::degrade::L3Verdict {
    // REQ-3 applies to certification obligations only. The §0.1 meta queries are
    // not minted as Obligations in v1, so `role` is always `Certification` here;
    // the match makes the scoping explicit and total (no `_` swallow).
    match role {
        ObligationRole::Certification => match verdict {
            Verdict::Proven(_) => crate::degrade::L3Verdict::Proved(proved_cert),
            // Unknown degrades (REQ-3): the shipped ladder's `Timeout` trigger runs
            // L2/L1. A fast-`unknown` (REQ-3.1) lands here, matching §6's
            // degrade-on-incompleteness intent; not a hard fail.
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
            // Refuted hard-fails (REQ-3 anti-cheat): a witnessed countermodel never
            // degrades. This generalizes `ladder_action_l3`'s
            // `Counterexample → HardFail`.
            Verdict::Refuted(_) => crate::degrade::L3Verdict::Counterexample(cx_cert),
        },
    }
}

/// Does a `Counterexample` outcome carry the genuine SMT-incompleteness `unknown`
/// signature? (`.design/verified/proof-backends.md` REQ-3.1.) This is the narrow
/// remap predicate: the REQ-3.1 fast-`unknown` is the case where the SMT solver
/// returned `unknown` (the solver could not decide, an incompleteness event
/// semantically like a timeout), as opposed to (a) a witnessed countermodel
/// (`postcondition not satisfied` with a parsed `--> span`), or (b) a frontend
/// rejection (a type error `error[E…]`, e.g. the IFC un-typeable `careless_query`
/// E0308 the provenance corpus pins at L0).
///
/// The shipped `classify_verus_outcome` lumps all three span-less failures into the
/// `Counterexample` bucket. To keep the cert oracle byte-identical (the increment
/// (i) AC), the remap fires only on the genuine incompleteness signature and
/// defaults to `Refuted` (the shipped `Counterexample → HardFail`) for everything
/// else. The signature: no obligation carries a witnessing `--> span` location
/// (a real countermodel is witnessed and stays `Refuted`), and no diagnostic
/// carries a frontend error marker (`error[E`: a Rust/VIR type error is a genuine
/// rejection, not an SMT `unknown`, and stays `Refuted` → L0), and a diagnostic
/// explicitly names the SMT `unknown` incompleteness verdict. This makes the remap
/// inert on the corpus (which contains witnessed failures + E0308 type errors, not
/// genuine SMT `unknown`s), so every `conformance/*.cert.json` is unperturbed.
/// Determinism: a pure function of the parsed obligations (R-CODE-5).
#[must_use]
pub fn counterexample_is_incompleteness_unknown(
    obligations: &[crate::manifest::ObligationResult],
) -> bool {
    // A witnessed countermodel (any parsed `--> span`) is a genuine disproof → not
    // remapped (stays `Refuted`).
    if obligations.iter().any(|o| o.location.is_some()) {
        return false;
    }
    // A frontend error (`error[E…]`, a type/VIR rejection like the IFC E0308) is a
    // genuine rejection, not an SMT `unknown` → not remapped (stays `Refuted` → L0,
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
    // a model). Only this narrow case degrades (REQ-3.1). A bare/empty diagnostic
    // is not remapped; without a positive `unknown` signal we keep the shipped
    // hard-fail (conservative: the corpus stays byte-identical).
    obligations.iter().any(|o| {
        o.diagnostic
            .as_deref()
            .is_some_and(|d| d.to_ascii_lowercase().contains("unknown"))
    })
}

// ============================================================================
// REQ-4 — certificate attribution (`.design/verified/proof-backends.md` REQ-4 / §5,
// increment (iii), #247): the per-obligation `{engine, trust_profile}` pair. Additive
// (the cert field is `Option`, populated only when a non-default engine discharges),
// so the default Verus path leaves it `None` and the corpus certs stay byte-identical.
// Honest-min project aggregation is unchanged; this is per-obligation metadata
// orthogonal to `Level` (§5 "project aggregation stays honest-min").
// ============================================================================

/// The per-obligation engine attribution (`.design/verified/proof-backends.md`
/// REQ-4): the engine that proved an obligation + that engine's trust profile, so an
/// auditor reading an L3 cert sees whether L3-via-Lean enumerates a smaller base
/// ({Lean kernel + 3 axioms, EXP}) than L3-via-Verus ({Z3, Verus VC-gen, lowering
/// theorem}). A serde value (the `Certificate` field is `Option<EngineAttribution>`,
/// additive). Determinism: a pure function of the engine identity (R-CODE-5).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EngineAttribution {
    /// The engine that discharged the obligation (its stable tag).
    pub engine: String,
    /// The enumerated named trust items that engine adds on a `Proven` (the §1
    /// enumerable trusted base, the auditor-visible base).
    pub trust_profile: Vec<String>,
}

/// Build the [`EngineAttribution`] for an engine (`.design/verified/proof-backends.md`
/// REQ-4): the `{engine tag, trust profile items}` pair. Called on the discharge path
/// whenever a non-default engine (Lean) proves an obligation, so the cert records the
/// smaller trust base; the default Verus path does not attach it (the cert stays
/// byte-identical; the `serde(default)` keeps the goldens green).
#[must_use]
pub fn attribution_for(engine: &dyn Engine) -> EngineAttribution {
    EngineAttribution {
        engine: engine.name().tag().to_string(),
        trust_profile: engine.trust_profile().items,
    }
}

// ============================================================================
// REQ-5 — the disagreement halt (`.design/verified/proof-backends.md` REQ-5 / §5 /
// AC-5, increment (iii), #247): one engine `Proven` + another `Refuted` (a witnessed
// countermodel) on the same certification obligation = a soundness alarm. The
// toolchain halts with a structured hard error naming both engines + the obligation;
// it never silently picks the favorable `Proven`. `Proven ⊕ Unknown` is benign (the
// Unknown engine could not decide, and per REQ-3.1 a witness-less Verus failure is
// `Unknown`, so it cannot spuriously fire this alarm against a Lean kernel `Proven`).
// ============================================================================

/// A soundness alarm (`.design/verified/proof-backends.md` REQ-5): one engine
/// `Proven` and another `Refuted` (a witnessed countermodel) on the same obligation.
/// A genuine countermodel from one engine contradicting a proof from another means
/// one engine (or the exporter/lowering, or `S` itself) is unsound; proceeding would
/// launder unsoundness into a certificate, the failure §1's enumerable-base promise
/// forbids. Carries both engine names + the obligation + the refuting counterexample
/// (the deliverable, §5.1).
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

/// Check the verdicts of two engines on the same obligation for the disagreement
/// alarm (`.design/verified/proof-backends.md` REQ-5 / AC-5). This is the
/// multi-engine dispatch guard: if one verdict is `Proven` and the other is `Refuted`
/// (a witnessed countermodel), halt with a structured [`Disagreement`]. Every other
/// pairing is benign (`Ok`): `Proven ⊕ Unknown`, `Proven ⊕ Proven`, `Unknown ⊕
/// anything`, `Refuted ⊕ Refuted` (both witnessed a failure: agreement, not a
/// soundness contradiction; the hard fail stands), etc. Per REQ-3.1 a Verus
/// witness-less fast-`unknown` is `Unknown`, so it cannot fire this alarm against a
/// Lean `Proven`; only a witnessed countermodel can, which is the real unsoundness
/// case. Determinism: a pure function of the two verdicts (R-CODE-5).
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
        // Every other pairing is benign, including Proven ⊕ Unknown (the Unknown
        // engine could not decide), Refuted ⊕ Refuted (both witnessed a failure,
        // agreement on the bug), and any Unknown pairing.
        _ => Ok(()),
    }
}

// ============================================================================
// REQ-7 — interactive proofs (`.design/verified/proof-backends.md` REQ-7(ii) / §4
// "INTERACTIVE" / §6 tier (c), increment (iii), #247): for a tier-(c) item the engine
// emits the skeleton to `<file>.lean-proofs/<item>.lean` when absent; when present the
// file is replayed (lake) with the obligation-hash staleness gate (the emitted header
// carries the evidence_key; a mismatch = stale → Unknown("stale proof — re-derive"),
// never silently reused). A `sorry` is detected explicitly (lake exits 0 on a `sorry`,
// so check and handle) and is never `Proven`. A kernel-accepted, sorry-free replay =
// `Proven` with the interactive trust profile.
// ============================================================================

/// The evidence-key header line a skeleton / interactive proof carries
/// (`.design/verified/proof-backends.md` REQ-7(ii)). The emitted header pins the
/// obligation's evidence key so a replay can detect staleness (a changed obligation /
/// toolchain / spine bumps the key → the header no longer matches → the proof is
/// stale and must be re-derived, never silently reused).
pub const INTERACTIVE_EVIDENCE_KEY_MARKER: &str = "-- evidence_key: ";

/// The deterministic path of a tier-(c) item's interactive proof artifact
/// (`.design/verified/proof-backends.md` REQ-7(ii), "a proof file checked in next to
/// the source"): `<file>.lean-proofs/<item>.lean`. The artifact lives beside the
/// source so it is reviewed + version-controlled with it (OQ-4). Deterministic
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

/// Does an interactive proof source carry an open `sorry` (`.design/verified/
/// proof-backends.md` REQ-7(ii): lake warns but exits 0 on sorry, so sorry detection
/// is explicit and sorry is never Proven)? The check is two-fold: (1) a textual
/// `sorry` token in the source (the skeleton's placeholder an agent must fill), and
/// (2) a `sorryAx` / `sorry` in the `#print axioms` output (a `sorry` that survived
/// elaboration, the authoritative kernel signal). Either is an open hole → the proof
/// is not a genuine kernel proof and is never `Proven`. Determinism: a pure function
/// of the inspected strings (R-CODE-5).
#[must_use]
pub fn proof_has_sorry(source: &str, print_axioms_output: &str) -> bool {
    source_contains_sorry_token(source) || axioms_contain_sorry(print_axioms_output)
}

/// A textual `sorry` token in the proof source (a whole-word match so a substring
/// like `sorryless` does not false-positive). The skeleton emits `  sorry  --
/// INTERACTIVE …`, so an unfilled skeleton trips this.
fn source_contains_sorry_token(source: &str) -> bool {
    source
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|tok| tok == "sorry")
}

/// A `sorryAx` / `sorry` axiom in a `#print axioms` output (the authoritative kernel
/// signal that a `sorry` survived elaboration: lake exits 0 on a `sorry`, so the
/// axioms output is what distinguishes a genuine kernel proof from a `sorry`-carrying
/// one).
fn axioms_contain_sorry(print_axioms_output: &str) -> bool {
    let lower = print_axioms_output.to_ascii_lowercase();
    lower.contains("sorryax") || lower.contains("sorry")
}

/// The trust-base axiom allowlist: the standard Lean axiom set the kernel-proven spine
/// rests on (`{propext, Classical.choice, Quot.sound}`, `.design/verified/
/// thermite-semantics.md`, the (T1)/(T2) axiom enumeration). A Lean cert's enumerable
/// trusted base is this set + EXP[, author]; an axiom outside it is a smuggled
/// dependency the cert would not enumerate.
const STANDARD_AXIOM_ALLOWLIST: [&str; 3] = ["propext", "Classical.choice", "Quot.sound"];

/// The outcome of inspecting a `#print axioms` output for the obligation theorem's own
/// report line (`.design/verified/proof-backends.md` REQ-4/§1, R-DEFER-9).
#[derive(Debug, PartialEq, Eq)]
enum AxiomReport {
    /// The obligation theorem's report line lists only allowlisted axioms (or it does
    /// not depend on any axioms): the enumerable base holds.
    Clean,
    /// The obligation theorem's report line lists a non-standard axiom (a smuggled
    /// dependency the cert would not enumerate): the named axiom is outside the allowlist.
    Nonstandard(String),
    /// No report line for the obligation theorem was found in the output. The parser
    /// never falls through to a foreign theorem's report (an author's own `#print axioms`
    /// emitted earlier); a missing anchor is a hard reject, never `Clean`.
    Missing,
}

/// Strictly parse a `#print axioms` output anchored on the obligation theorem's own report
/// line and classify it (`.design/verified/proof-backends.md` REQ-4/§1, R-DEFER-9). Lake
/// prints the inspected theorem's quoted name verbatim: `'thermite_obligation_<item>'
/// depends on axioms: [a, b, c]` or `'thermite_obligation_<item>' does not depend on any
/// axioms`. The author's checked-in proof file is arbitrary Lean and may emit its own
/// `#print axioms <clean_helper>` before the appended obligation probe, so we bind to the
/// obligation theorem's report, not the first `depends on axioms:` line, or a clean
/// helper's report masks the obligation's smuggled axiom (the #249 divergence). We scan
/// all lines for the anchor `'<thm>' …`; if multiple match we inspect every one (the first
/// non-standard axiom across them wins); if none match → [`AxiomReport::Missing`] (never
/// fall through to a foreign line). The bracket list is parsed strictly: split on `,`,
/// trim, reject any name not in the allowlist. `sorryAx` is out of the allowlist too, so
/// this also catches a surviving `sorry`; [`proof_has_sorry`] runs first for the
/// dedicated `sorry` message. Deterministic, a pure function of its inputs (R-CODE-5).
#[must_use]
fn nonstandard_axiom(print_axioms_output: &str, item: &str) -> AxiomReport {
    // The anchor is the obligation theorem's quoted name as lake prints it: `'<thm>'`.
    let thm_name = format!("thermite_obligation_{}", proof_thm_sanitize(item));
    let anchor = format!("'{thm_name}'");
    const MARKER: &str = "depends on axioms:";

    // Inspect every line that names the obligation theorem (defense in depth: if the
    // output carries more than one report for it, all are checked). A line of the form
    // `'<thm>' depends on axioms: [a, b, c]` carries the bracket list; the other form,
    // `'<thm>' does not depend on any axioms`, has no bracket after the marker → clean.
    let mut saw_anchor = false;
    for line in print_axioms_output.lines() {
        if !line.contains(&anchor) {
            continue;
        }
        saw_anchor = true;
        // Anchor on `depends on axioms:` so lake's warning/linter text (which can itself
        // carry `[…]` lists, a `simp only [Thermite.Env.bindInt, …]` unused-arg hint) is
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
        // theorem's report; a missing anchor is a hard reject (the obligation's real
        // axiom list is unknown, so the enumerable base cannot be vouched for).
        AxiomReport::Missing
    }
}

/// The shared certify-time axiom gate (REQ-2 / AC-5, `.design/stage1-forge-tier.md`): run
/// on EVERY Lean discharge path — the auto tiers (a)/(b) (via [`LeanEngine::run_lake`]) AND
/// the interactive replay (via `replay_interactive`). Given the emitted Lean `source` and
/// the lake/lean output (which must contain the obligation theorem's `#print axioms`
/// report), returns `Ok(())` iff the obligation is sorry-free AND its axioms ⊆ the
/// allowlist; otherwise `Err(reason)` naming the surviving `sorry`, the smuggled axiom, or
/// the missing report. Hoisting this onto the auto path closes the AC-5 hole: a clean lake
/// exit was previously certified `Proven` with no axiom check on the auto tiers. The two
/// callers share THIS function so the gate's behavior cannot drift between paths.
fn certify_lean_axioms(source: &str, lake_output: &str, item: &str) -> Result<(), String> {
    // (1) `sorry` first (the dedicated message): a `sorry` survives a clean lake exit
    // (lake exits 0 on a `sorry`), so the source/`sorryAx`-axioms scan is what
    // distinguishes a genuine kernel proof. NEVER `Proven` (REQ-7(ii)).
    if proof_has_sorry(source, lake_output) {
        return Err(format!(
            "the exported obligation `{item}` carries a `sorry` (detected in the source \
             and/or `#print axioms` — `sorryAx`); a `sorry` is NEVER certified"
        ));
    }
    // (2) The trust-base axiom allowlist anchored on the obligation theorem's own
    // `#print axioms` report (proof-backends REQ-4 / §1): {propext, Classical.choice,
    // Quot.sound}. An axiom outside it is a smuggled dependency the cert would not
    // enumerate; a missing report means the enumerable base cannot be vouched for.
    match nonstandard_axiom(lake_output, item) {
        AxiomReport::Clean => Ok(()),
        AxiomReport::Nonstandard(extra) => Err(format!(
            "non-standard axiom: {extra}: the obligation theorem for `{item}` \
             kernel-accepts but its `#print axioms` rests on an axiom outside \
             {{propext, Classical.choice, Quot.sound}} (a smuggled dependency)"
        )),
        AxiomReport::Missing => Err(format!(
            "axiom report missing: lake emitted no `#print axioms` report for the \
             obligation theorem of `{item}`, so its enumerable trusted base cannot be \
             vouched for (REQ-2/AC-5: never certify an un-anchored axiom set)"
        )),
    }
}

/// The byte offset of the `theorem <thm_name>` header that anchors the obligation
/// theorem, matching the exact theorem name and never a prefix (`.design/verified/
/// proof-backends.md` REQ-7: the replay reconstruction anchors the obligation theorem;
/// the #268 anchor-class fix). A raw `find("theorem thermite_obligation_<item>")` is a
/// prefix match: on a multi-theorem while-shaped file (`thermite_obligation_<item>_entry`
/// at the top, the bare contract `thermite_obligation_<item>` lower down,
/// `thermite_obligation_<item>_converges` after) the first match latches `_entry` and the
/// reconstruction binds the wrong theorem, dropping the 5+2 conjunction (the same
/// anchor-resolution-binds-the-wrong-declaration class as #249/#250). The anchor here
/// requires the char immediately after `<thm_name>` to be a non-identifier (not
/// alphanumeric/`_`/`.`), so `_entry`/`_converges` (and any other suffixed sibling) never
/// false-match the bare name, and the keyword itself to start a token (mirrors
/// [`declaration_sites`]' boundary logic). Returns the offset of the `theorem` keyword, or
/// `None` when no exact-name header is present. Deterministic (R-CODE-5); never a panic
/// (R-CODE-2).
#[must_use]
fn theorem_anchor_pos(source: &str, thm_name: &str) -> Option<usize> {
    let prefix = format!("theorem {thm_name}");
    let mut from = 0usize;
    while let Some(rel) = source[from..].find(&prefix) {
        let kw_start = from + rel;
        let name_end = kw_start + prefix.len();
        // The char after `<thm_name>` must be a non-identifier char (whitespace, `:`,
        // `(`, EOF, …), so a suffixed sibling (`_entry`, `_converges`) is not a match.
        let boundary_ok = source[name_end..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '.'));
        // The `theorem` keyword must itself start a token (`mytheorem` must not match).
        let kw_token_ok = source[..kw_start]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        if boundary_ok && kw_token_ok {
            return Some(kw_start);
        }
        from = name_end;
    }
    None
}

/// Extract the canonical theorem statement of `thermite_obligation_<item>` from a Lean
/// source (`.design/verified/proof-backends.md` REQ-6, the statement-binding surface):
/// the text from the `theorem thermite_obligation_<item>` keyword through (and
/// including) the `:=` that begins the proof term — i.e. the binders + the proposition
/// the author may not change (they fill only the proof after `:=`/`by`). The proof
/// delimiter is anchored on the first `:= by` / bare `:=` after the theorem header that
/// is not a record-update `:=` (the spine's `{ v with specs := R_item }` uses `:=`
/// inside the proposition); we anchor on `:= by`, falling back to a `:=` not preceded by
/// ` with ` …. The emitted forms close with `:= by`, so `:= by` is the reliable anchor.
/// Returns `None` when no such theorem/`:= by` is found. Deterministic (R-CODE-5).
#[must_use]
fn canonical_theorem_statement(source: &str, item: &str) -> Option<String> {
    let thm_name = format!("thermite_obligation_{}", proof_thm_sanitize(item));
    let start = theorem_anchor_pos(source, &thm_name)?;
    let from_thm = &source[start..];
    // The proof term starts at `:= by` (both the auto and interactive emitted forms
    // close the conclusion with `… := by`). A record-update `specs := R_item` never has
    // ` by` after `:=`, so `:= by` is unambiguous. Include up to and including `:=`.
    let by_pos = from_thm.find(":= by").or_else(|| {
        // Defensive: a hand-authored proof might use `:= <term>` (no `by`). Anchor on
        // the last `:=` whose left context is not a record-update ` with … specs`.
        from_thm.rfind(":=")
    })?;
    Some(from_thm[..by_pos + 2].to_string())
}

/// Whitespace-insensitive equality of two theorem statements (`.design/verified/
/// proof-backends.md` REQ-6, "modulo whitespace; be strict"). Collapses every run of
/// ASCII/Unicode whitespace to a single space and trims, so the author's reformatting
/// of the emitted skeleton's statement (line wrapping) does not spuriously mismatch, but
/// a different statement (a different proposition / binders) does. Deterministic
/// (R-CODE-5).
#[must_use]
fn statements_match(a: &str, b: &str) -> bool {
    fn norm(s: &str) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }
    norm(a) == norm(b)
}

/// The Lean declaration keywords a top-level declaration can begin with (the
/// reconstruct-and-splice boundary detection, `.design/verified/proof-backends.md` REQ-6 /
/// the #250 fix). A line whose first whitespace-trimmed token is one of these begins a new
/// top-level declaration; `#`-prefixed lines (`#print`/`#check`/`#eval`) and the section
/// commands (`namespace`/`section`/`end`/`open`/`variable`) likewise end the preceding
/// declaration's text. Used to bound a declaration's body and to detect declaration sites.
const DECL_KEYWORDS: [&str; 8] = [
    "theorem", "lemma", "def", "abbrev", "example", "instance", "axiom", "opaque",
];

/// Does a source line begin a top-level Lean declaration or command (the boundary that
/// ends the preceding declaration's text)? A `#`-command (`#print`/`#check`) or a section
/// command also ends it. The line must be a top-level line (column 0; a declaration's
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

/// The byte offset of the end of the top-level declaration that starts at `start`
/// (`.design/verified/proof-backends.md` REQ-6, the #250 fix): the start of the next
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

/// The byte offset of a declaration's proof-delimiter `:=` (`.design/verified/
/// proof-backends.md` REQ-6, the #250 fix), distinguished from a record-update `:=`
/// inside the proposition (the spine's `{ v with specs := R_item }`). The emitted /
/// authored forms close the conclusion with `… := by` (auto + interactive induction), so
/// `:= by` is the primary, unambiguous anchor. A term-mode hand proof (`… := <term>`, no
/// `by`) falls back to the first `:=` whose immediately-preceding non-space token is not
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

/// Every byte offset in `source` at which the short name `thm_name` is declared — i.e.
/// appears as a standalone token immediately after a [`DECL_KEYWORDS`] keyword, in any
/// namespace (`.design/verified/proof-backends.md` REQ-6 / R-DEFER-9, the #250 fix). The
/// offset points at the declaration keyword (so the proof extraction starts there). More
/// than one site is the #250 same-short-name decoy → the caller rejects. Deterministic
/// (R-CODE-5).
fn declaration_sites(source: &str, thm_name: &str) -> Vec<usize> {
    let mut sites = Vec::new();
    for keyword in DECL_KEYWORDS {
        // The declaration prefix `<keyword> <thm_name>` with the name a standalone token
        // (the char after the name is a non-identifier char: a space, `:`, `(`, `\n`, …).
        let prefix = format!("{keyword} {thm_name}");
        let mut from = 0usize;
        while let Some(rel) = source[from..].find(&prefix) {
            let kw_start = from + rel;
            let name_end = kw_start + prefix.len();
            let boundary_ok = source[name_end..]
                .chars()
                .next()
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '.'));
            // The keyword itself must start a token (preceded by start-of-input or a
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

/// The top-level command keywords a proof term may not carry (`.design/verified/
/// proof-backends.md` REQ-6 / §1 / R-DEFER-9, the #252 belt). After the #252 architectural
/// fix the proof term is the only author-controlled text and is type-checked against the
/// fixed generator-emitted goal, so a proof term cannot vacate that goal. This belt is a
/// defense layer against an `… in`-style top-level command form smuggled into the
/// term (`open … in`, `set_option … in`, a `#…`-command): any of these as an exact token
/// (whitespace-independent, position-independent) → reject. A genuine term/tactic proof
/// never needs these; auxiliary lemmas inline as `have`/`let`/`suffices`. The `#` family
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

/// The #252 belt (`.design/verified/proof-backends.md` REQ-6 / §1 / R-DEFER-9): scan the
/// extracted proof term and report a top-level command keyword if one appears as an exact
/// token in any position (whitespace-independent), or a `#…`-command token. The proof term
/// is type-checked against the fixed generator goal, so this is a defense layer; the
/// primary soundness mechanism is the elimination of the helper surface, so author
/// content can no longer share the obligation's elaboration scope. It catches an `open …
/// in`-style command form a proof term might smuggle. Tokenizes on whitespace and the Lean
/// term-separator punctuation (`(`, `)`, `;`, `,`) so `open Foo in` is caught even with no
/// surrounding spaces; an identifier that merely contains a keyword (`openMyDef`,
/// `Nat.open`) is not a match (the token must equal the keyword, and a `.`-qualified
/// tail is excluded). Deterministic (R-CODE-5); never a panic (R-CODE-2).
fn proof_term_command_token(proof_term: &str) -> Option<String> {
    for raw in proof_term.split(|c: char| {
        // Split on whitespace and Lean term/tactic punctuation. `:` is included so a
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
        // member access, not a command; exclude it (the command keyword is never the tail
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

/// The trust profile of an interactive Lean proof (`.design/verified/proof-backends.md`
/// REQ-7(ii) / OQ-4): {Lean kernel + 3 standard axioms, EXP} plus the human/agent
/// author as a reviewed-but-not-mechanized step (the interactive path adds the author,
/// OQ-4). Distinct from the auto profile so the auditor sees an interactive proof
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
// REQ-8 — the nlsat real-relaxation engine (`.design/stage1-forge-tier.md` REQ-8 /
// Q-NLSAT / AC-12, increment 2f). The relax route: a relaxable polynomial contract
// (the `relax` fragment) is handed to a direct Z3 `nlsat`-tactic (QF_NRA) query — the
// FIRST real-arithmetic Z3 query as its own engine (today Z3 is reached only THROUGH
// Verus). `unsat` over ℝ ⇒ (by the kernel-checked `r_relax_sound`) valid over ℤ ⇒
// certify L4 (kernel-grounded). `sat` ⇒ the integrality check (Q8: round into the
// radius-2 ℤⁿ box) splits a genuine integer `Counterexample` from a real-only
// `RealWitness` (true over ℤ, false over ℝ) — the latter escalates UP to the forge,
// never down to a `Counterexample`.
// ============================================================================

/// The outcome of an nlsat relax discharge (`.design/stage1-forge-tier.md` REQ-8 /
/// AC-12). The richer-than-[`Verdict`] result the relax route returns: the
/// [`RealWitness`](NlsatOutcome::RealWitness) case carries a raw REAL point the 3-arm
/// engine `Verdict` cannot, so [`NlsatEngine::discharge_relax`] is the route's real
/// entry point (the `Engine::discharge` trait impl maps it down for generic callers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NlsatOutcome {
    /// `unsat` over ℝ: no real counterexample → the relaxation `∀ x : ℝ, req → ⋀ ens`
    /// holds → by `r_relax_sound` the integer clause holds → certify at L4.
    Proved,
    /// `sat` over ℝ AND an integer point in the radius-2 ℤⁿ box genuinely falsifies the
    /// integer clause → a real integer `Counterexample`. Carries the integer witness.
    Counterexample {
        /// The integer falsifying point (variable → value, textual).
        integer_point: Vec<(String, String)>,
    },
    /// `sat` over ℝ but NO integer point in the box falsifies → the clause is true over
    /// ℤ, false over ℝ → a `RealWitness` escalation (never a `Counterexample`). Carries
    /// the raw real point nlsat returned, handed to the forge as goal metadata.
    RealWitness {
        /// The raw real countermodel point (textual rationals/decimals).
        point: crate::verdict::RealPoint,
    },
    /// z3 returned `unknown`, z3 is absent, the item is not relaxable, or the query
    /// failed to render — an honest skip (never a false `Proved`, never a
    /// `Counterexample`). Carries the reason.
    Unknown(String),
}

/// The nlsat real-relaxation engine (`.design/stage1-forge-tier.md` REQ-8 / Q-NLSAT,
/// increment 2f). Carries the parsed [`Program`] (the trait `discharge` resolves the
/// obligation's `fn` by name to read its full contract — the `ens` clauses an
/// [`Obligation`] does not carry). Implements the four-slot [`Engine`] interface; the
/// relax-specific entry is [`NlsatEngine::discharge_relax`], which returns the richer
/// [`NlsatOutcome`] (the `RealWitness` the trait `Verdict` cannot represent).
#[derive(Debug, Clone)]
pub struct NlsatEngine {
    /// The parsed source program (the route resolves the `fn`'s full contract).
    program: Program,
}

impl NlsatEngine {
    /// Construct an nlsat engine over a parsed program (`.design/stage1-forge-tier.md`
    /// REQ-8). Production consumer: `check::check_file_with_engine` (`--engine nlsat`).
    #[must_use]
    pub fn new(program: Program) -> Self {
        NlsatEngine { program }
    }

    /// Locate the `z3` binary (`.design/stage1-forge-tier.md` REQ-8 / Q-NLSAT). Z3 is
    /// bundled alongside the `verus` distribution and resolved on `PATH` (the same way
    /// `check::run_verus` reaches `verus`); a bare `z3` lookup suffices in the
    /// verus-on-PATH environment. Deterministic given the environment (R-CODE-5).
    fn z3_binary() -> &'static str {
        "z3"
    }

    /// Is `z3` invocable? (`.design/stage1-forge-tier.md` REQ-8.) The skip-guard for
    /// the live nlsat tests — CI test-shards without z3 SKIP rather than fail, mirroring
    /// the sibling `lake_present()` guard on the live Lean tests. No production caller —
    /// [`NlsatEngine::run_z3`] handles z3-absence inline (a graceful `Unknown` skip).
    #[allow(
        dead_code,
        reason = "REQ-8 live-test skip-guard: the in-crate live nlsat engine tests call it \
                  (CI shards without z3 SKIP); run_z3 handles z3-absence inline in production"
    )]
    #[must_use]
    pub fn z3_present() -> bool {
        std::process::Command::new(Self::z3_binary())
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Does the nlsat fragment admit this item? (`.design/stage1-forge-tier.md` REQ-8b
    /// / AC-12.) The narrowed-fragment admission is the `relaxable` syntactic check
    /// ([`crate::relax::classify_fn`]) over the item's `fn`, not the static
    /// `admits_all_classes` flag (`false` for this engine). A non-`fn` item or a
    /// non-relaxable contract is not admitted (an honest skip, never a verdict).
    #[must_use]
    pub fn admits_relax(&self, item: &str) -> bool {
        match find_item(&self.program, item) {
            Some(Item::Fn(f)) => crate::relax::classify_fn(f).is_relaxable(),
            _ => false,
        }
    }

    /// Discharge a relaxable `fn`'s contract via the direct Z3 nlsat (QF_NRA) query
    /// (`.design/stage1-forge-tier.md` REQ-8c / AC-12 — the relax route's real entry
    /// point). Builds the negated-contract query ([`crate::relax::negated_contract_query`]),
    /// runs the nlsat tactic, and:
    ///
    /// - `unsat` → [`NlsatOutcome::Proved`] (real-valid → integer-valid by `r_relax_sound`);
    /// - `sat` → the integrality check (Q8): round the real model into the radius-2 ℤⁿ
    ///   box and test the integer clause. An integer falsifier →
    ///   [`NlsatOutcome::Counterexample`]; none → [`NlsatOutcome::RealWitness`] carrying
    ///   the raw real point;
    /// - `unknown` / z3 absent / not relaxable / render failure →
    ///   [`NlsatOutcome::Unknown`] (an honest skip, never a false verdict).
    #[must_use]
    pub fn discharge_relax(&self, f: &thermite_syntax::FnItem) -> NlsatOutcome {
        if !crate::relax::classify_fn(f).is_relaxable() {
            return NlsatOutcome::Unknown(format!(
                "`{}` is not in the relax fragment: {}",
                f.name,
                match crate::relax::classify_fn(f) {
                    crate::relax::RelaxVerdict::NotRelaxable(r) => r,
                    crate::relax::RelaxVerdict::Relaxable => String::new(),
                }
            ));
        }
        let query = match crate::relax::negated_contract_query(f) {
            Some(q) => q,
            None => {
                return NlsatOutcome::Unknown(format!(
                    "the relaxable contract of `{}` did not render to a QF_NRA query",
                    f.name
                ));
            }
        };
        // pp.decimal makes z3 print real (incl. algebraic) model values as decimals so
        // the integrality check can round them; the nlsat tactic is selected inside the
        // query (`check-sat-using qfnra-nlsat`).
        let input = format!("(set-option :pp.decimal true)\n{query}");
        let (result, model) = match Self::run_z3(&input) {
            Ok(pair) => pair,
            Err(reason) => return NlsatOutcome::Unknown(reason),
        };
        match result.as_str() {
            "unsat" => NlsatOutcome::Proved,
            "unknown" => NlsatOutcome::Unknown(format!(
                "z3 nlsat returned `unknown` on `{}`'s real relaxation (undecided)",
                f.name
            )),
            "sat" => Self::classify_sat(f, &model),
            other => NlsatOutcome::Unknown(format!(
                "z3 nlsat returned an unexpected result `{other}` on `{}`",
                f.name
            )),
        }
    }

    /// The integrality check (`.design/stage1-forge-tier.md` REQ-8c / Q8): given the
    /// `sat` real model, decide whether the counterexample is a genuine integer
    /// `Counterexample` or a real-only `RealWitness`. Rounds each variable to the
    /// nearest integer and tests the radius-2 ℤⁿ box; an integer point that falsifies
    /// the integer clause is a `Counterexample`, otherwise the raw real point is a
    /// `RealWitness`.
    fn classify_sat(f: &thermite_syntax::FnItem, model: &BTreeMap<String, String>) -> NlsatOutcome {
        let vars = crate::relax::integer_vars(f);
        // The raw real point (textual), for the RealWitness escalation. An unconstrained
        // variable z3 omits is recorded as "0" (its box center).
        let raw_point = crate::verdict::RealPoint {
            assignment: vars
                .iter()
                .map(|v| {
                    (
                        v.clone(),
                        model.get(v).cloned().unwrap_or_else(|| "0".to_string()),
                    )
                })
                .collect(),
        };
        if let Some(integer_point) = Self::integrality_box_falsifier(f, &vars, model) {
            return NlsatOutcome::Counterexample { integer_point };
        }
        NlsatOutcome::RealWitness { point: raw_point }
    }

    /// Search the radius-2 ℤⁿ box rounded from the real model for an integer point that
    /// falsifies the integer clause (`.design/stage1-forge-tier.md` REQ-8c / Q8). Each
    /// variable's box center is its rounded real value (an unconstrained / unparseable
    /// variable centers at 0); the box is the Cartesian product of `center ± {0,1,2}`.
    /// Returns the first integer falsifier (a genuine `Counterexample`), or `None` (the
    /// real countermodel is real-only → `RealWitness`). The box is small (5ⁿ over the
    /// few relax variables), well within the Q8 1s budget.
    fn integrality_box_falsifier(
        f: &thermite_syntax::FnItem,
        vars: &[String],
        model: &BTreeMap<String, String>,
    ) -> Option<Vec<(String, String)>> {
        let centers: Vec<i128> = vars
            .iter()
            .map(|v| {
                model
                    .get(v)
                    .and_then(|t| Self::real_to_f64(t))
                    .map_or(0, |x| x.round() as i128)
            })
            .collect();
        // A defensive cap: relaxable fns carry a handful of variables; 5⁶ = 15625 is the
        // ceiling we search (beyond it the box would be too large for the 1s budget — a
        // real-only escalation rather than an unbounded search).
        let n = vars.len();
        if n > 6 {
            return None;
        }
        let offsets: [i128; 5] = [-2, -1, 0, 1, 2];
        let total = 5usize.pow(n as u32);
        for idx in 0..total {
            let mut rem = idx;
            let mut assign: BTreeMap<String, i128> = BTreeMap::new();
            for (vi, v) in vars.iter().enumerate() {
                let off = offsets[rem % 5];
                rem /= 5;
                assign.insert(v.clone(), centers[vi].saturating_add(off));
            }
            if crate::relax::eval_contract_negation_over_ints(f, &assign) == Some(true) {
                return Some(
                    vars.iter()
                        .map(|v| (v.clone(), assign[v].to_string()))
                        .collect(),
                );
            }
        }
        None
    }

    /// Run z3 over the SMT-LIB2 `input` (fed on stdin), returning `(result, model)`:
    /// the first result token (`sat`/`unsat`/`unknown`) and the raw model text. `Err`
    /// on z3 absent / spawn failure / no result token (an honest skip reason, never a
    /// silent success — R-CODE-4).
    fn run_z3(input: &str) -> Result<(String, BTreeMap<String, String>), String> {
        use std::io::Write as _;
        use std::process::{Command, Stdio};
        let mut child = Command::new(Self::z3_binary())
            .arg("-smt2")
            .arg("-in")
            .arg("-T:10")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    "z3 is not on PATH (the nlsat relax route needs the bundled z3) — \
                     skipping"
                        .to_string()
                } else {
                    format!("could not spawn z3: {e}")
                }
            })?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input.as_bytes())
                .map_err(|e| format!("could not write the QF_NRA query to z3: {e}"))?;
        }
        let out = child
            .wait_with_output()
            .map_err(|e| format!("z3 did not complete: {e}"))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        let result = stdout
            .split_whitespace()
            .next()
            .map(str::to_string)
            .ok_or_else(|| {
                format!(
                    "z3 produced no result token (stderr head: {})",
                    String::from_utf8_lossy(&out.stderr)
                        .chars()
                        .take(200)
                        .collect::<String>()
                )
            })?;
        let model = Self::parse_real_model(&stdout);
        Ok((result, model))
    }

    /// Parse z3's `(get-model)` output into a `variable → raw-value-text` map
    /// (`.design/stage1-forge-tier.md` REQ-8c). Extracts each `(define-fun NAME () Real
    /// VALUE)` with balanced-paren value capture; the raw value text (a decimal, a
    /// `(- d)`, or a `(/ a b)`) is kept verbatim for the `RealWitness` point and parsed
    /// to `f64` by [`real_to_f64`](NlsatEngine::real_to_f64) for the integrality
    /// rounding.
    fn parse_real_model(model: &str) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let needle = "(define-fun ";
        let mut search = 0;
        while let Some(rel) = model[search..].find(needle) {
            let open = search + rel;
            let after = open + needle.len();
            let rest = &model[after..];
            let name_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let name = rest[..name_end].trim().to_string();
            match Self::matching_paren(model, open) {
                Some(close) => {
                    let inner = &model[after..close];
                    // The declared form is `NAME () Real VALUE`; z3 prints VALUE on the
                    // SAME or the NEXT line (multi-line models), so anchor on `) Real`
                    // and take the remainder trimmed (a decimal `1.41?`, a `(- d)`, or a
                    // `(/ a b)`).
                    if let Some(rpos) = inner.find(") Real") {
                        let value = inner[rpos + ") Real".len()..].trim().to_string();
                        if !name.is_empty() && !value.is_empty() {
                            out.insert(name, value);
                        }
                    }
                    search = close + 1;
                }
                None => {
                    search = after;
                }
            }
        }
        out
    }

    /// The index of the `)` matching the `(` at `open` in `s` (`.design/stage1-forge-
    /// tier.md` REQ-8c, the model parser). `None` if unbalanced (a defensive skip,
    /// never a panic — R-CODE-2).
    fn matching_paren(s: &str, open: usize) -> Option<usize> {
        let bytes = s.as_bytes();
        let mut depth = 0i32;
        for (i, &b) in bytes.iter().enumerate().skip(open) {
            if b == b'(' {
                depth += 1;
            } else if b == b')' {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Parse a z3 real value text to `f64` for the integrality rounding (`.design/
    /// stage1-forge-tier.md` REQ-8c). Handles a bare decimal (`1.5`, `1.4142135?` —
    /// the trailing `?` is z3's algebraic-truncation marker), a negation `(- d)`, and a
    /// rational `(/ a b)`. `None` on an unparseable form (the variable then centers at
    /// 0 in the box — a conservative real-only escalation, never a false counterexample).
    fn real_to_f64(s: &str) -> Option<f64> {
        let s = s.trim();
        if let Some(inner) = s.strip_prefix("(-").and_then(|x| x.strip_suffix(')')) {
            return Self::real_to_f64(inner.trim()).map(|v| -v);
        }
        if let Some(inner) = s.strip_prefix("(/").and_then(|x| x.strip_suffix(')')) {
            let mut parts = inner.split_whitespace();
            let a = Self::real_to_f64(parts.next()?)?;
            let b = Self::real_to_f64(parts.next()?)?;
            if b == 0.0 {
                return None;
            }
            return Some(a / b);
        }
        s.trim_end_matches('?').parse::<f64>().ok()
    }
}

impl Engine for NlsatEngine {
    fn name(&self) -> EngineName {
        EngineName::Nlsat
    }

    fn fragment(&self) -> Fragment {
        // The nlsat engine admits only the relax fragment — the `admits_all_classes =
        // false` narrowed-engine seam (REQ-2(a)). The per-item admission is the
        // `relaxable` syntactic check, run in `admits_relax` (REQ-8b); the flag marks
        // it as a narrowed engine so the route gates on `admits_relax`.
        Fragment {
            admits_all_classes: false,
        }
    }

    fn discharge(&self, o: &Obligation, covenant: &CovenantRecord) -> Verdict {
        // REQ-4 seam: the covenant record is threaded but inert on the relax route (the
        // covenant LOGIC is 2b; the relax route is a pure-real discharge).
        let _ = covenant;
        // Resolve the obligation's `fn` to read its full contract (an `Obligation` does
        // not carry the `ens` clauses the relax encoding needs).
        let Some(Item::Fn(f)) = find_item(&self.program, &o.item) else {
            return Verdict::Unknown(Reason::IncompleteUnknown(format!(
                "the nlsat engine discharges `fn` contracts; `{}` is not a relaxable fn",
                o.item
            )));
        };
        // Map the rich relax outcome down to the 3-arm engine `Verdict` (the trait's
        // total type). `RealWitness` has no engine-`Verdict` image (it carries a real
        // point) → `Unknown` here; `discharge_relax` is the route's real entry point
        // that preserves it. This keeps the trait usable for the disagreement check
        // (a `Proven` nlsat vs a `Refuted` other engine) without laundering a
        // `RealWitness` into a `Counterexample`.
        match self.discharge_relax(f) {
            NlsatOutcome::Proved => Verdict::Proven(Evidence {
                verified: 1,
                key: self.evidence_key(o),
            }),
            NlsatOutcome::Counterexample { integer_point } => Verdict::Refuted(Counterexample {
                obligations: vec![crate::manifest::ObligationResult::failed(
                    format!("{}#contract", o.item),
                    None,
                    Some(format!(
                        "nlsat found an integer counterexample: {}",
                        integer_point
                            .iter()
                            .map(|(v, x)| format!("{v} = {x}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                )],
            }),
            NlsatOutcome::RealWitness { .. } => {
                Verdict::Unknown(Reason::IncompleteUnknown(format!(
                    "`{}` is true over ℤ but false over ℝ (a RealWitness) — escalated to the \
                 forge via discharge_relax, not representable as a 3-arm Verdict",
                    o.item
                )))
            }
            NlsatOutcome::Unknown(detail) => Verdict::Unknown(Reason::IncompleteUnknown(detail)),
        }
    }

    fn trust_profile(&self) -> TrustProfile {
        // REQ-8 trust profile: solver(nlsat) + spine-lemma(kernel). The L4
        // kernel-grounded base — the Z3 nlsat real-arithmetic decision PLUS the
        // kernel-checked relax spine lemmas that bridge real-validity to
        // integer-validity (`lean/Thermite/Relax.lean`, axiom-probed ⊆ {propext,
        // Classical.choice, Quot.sound}). An auditor sees L4-via-nlsat rests on Z3's
        // nlsat soundness + the kernel lemma, distinct from L3-via-Verus's {Z3, Verus
        // VC-gen, lowering theorem}.
        TrustProfile {
            items: vec![
                "Z3 nlsat (QF_NRA real-arithmetic decision)".to_string(),
                "spine-lemma r_relax_sound (real→integer relaxation soundness, kernel-checked)"
                    .to_string(),
                "spine-lemma rencode_sound (real-encoding faithfulness, kernel-checked)"
                    .to_string(),
            ],
        }
    }

    fn evidence_key(&self, o: &Obligation) -> CacheKey {
        // REQ-2(d): the engine-discriminated key. The content side is the relaxable
        // contract's rendered QF_NRA query (so a contract edit invalidates the key),
        // falling back to the obligation identity when the fn is absent / unrenderable.
        use sha2::{Digest, Sha256};
        let content = match find_item(&self.program, &o.item) {
            Some(Item::Fn(f)) => crate::relax::negated_contract_query(f)
                .unwrap_or_else(|| format!("unrenderable::{}", o.item)),
            _ => format!("absent::{}", o.item),
        };
        let mut h = Sha256::new();
        h.update(b"thermite-nlsat-evidence-v1");
        h.update(o.item.as_bytes());
        h.update(content.as_bytes());
        let content_address: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
        CacheKey {
            engine: EngineName::Nlsat,
            content_address,
        }
    }
}

// ============================================================================
// REQ-9 — the engine-generic mutation battery, the Lean path (`.design/verified/
// proof-backends.md` REQ-9 / §7, increment (iii), #247). When the discharging engine
// is Lean: mutants are attempted via the same engine path; kill = `Refuted ∪
// Unknown-after-attempt`; the denominator = attempted − proven-equivalent; a mutant
// outside the engine's fragment = "untested against lean", reported in the cert,
// never counted killed. The Verus-path battery (`check::mutation_score`) is untouched.
// ============================================================================

/// The outcome of the REQ-6a arbitrary-result re-elaboration tautology check
/// (`.design/stage1-forge-tier.md` REQ-6 / AC-10, increment 2d — anti-Goodhart defense
/// (a)). Produced by [`LeanEngine::arbitrary_result_reelaboration`]. Only `Tautology`
/// licenses a reject; `Clean`/`Skipped` never reject (the safe completeness direction).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArbitraryResultOutcome {
    /// The harness kernel-accepted for an arbitrary `result` → the `ens` is body-ignoring
    /// (a semantic tautology over the Lean discharge domain) → the discharge gate rejects.
    Tautology,
    /// The harness failed to elaborate for an arbitrary `result` → the `ens` genuinely
    /// constrains the result → clean (the contract is not a body-ignoring tautology).
    Clean,
    /// The check could not run (export refusal / tier-(c) interactive / lake absent /
    /// axiom-gate) — an honest skip that NEVER rejects. Carries the reason.
    Skipped(String),
}

/// The outcome of attempting one mutant against the Lean engine (`.design/verified/
/// proof-backends.md` REQ-9). The engine-generic kill semantics: a mutant is killed if
/// the Lean engine `Refuted`s it or returns `Unknown` after attempting it (the mutant
/// was attempted and not proven, matching the shipped Verus `Counterexample ∪
/// Timeout` = killed); a mutant whose obligation the Lean fragment does not admit is
/// `UntestedAgainstLean` (never counted killed, never a survivor); a `Proven` mutant
/// survived (the mutation did not break the contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeanMutantOutcome {
    /// The Lean engine proved the mutant — it survived (the contract is too weak,
    /// unless then proven equivalent to the real body, the #101 exclusion the caller
    /// applies).
    Survived,
    /// The mutant was attempted by the Lean engine and killed (`Refuted`, a witnessed
    /// countermodel, or `Unknown` after an attempt). Maps onto the shipped
    /// `Counterexample ∪ Timeout` = killed.
    Killed,
    /// The mutant's obligation is outside the Lean engine's fragment (it was never
    /// attempted, e.g. a recursive-registry obligation only the tier-(c) interactive
    /// path admits, or an out-of-spine construct). "Untested against lean": never
    /// counted killed (that would inflate the ratio, §7 / R-DEFER-9) and never a
    /// survivor.
    UntestedAgainstLean,
}

/// Classify a Lean-engine mutant [`Verdict`] under REQ-9's engine-generic kill
/// semantics (`.design/verified/proof-backends.md` REQ-9). `admitted` is whether the
/// Lean fragment admitted the mutant's obligation (a per-mutant `fragment().admits`
/// check the caller runs before the discharge): a non-admitted mutant is
/// `UntestedAgainstLean` regardless of the verdict (it was never attempted;
/// the Lean engine maps a refusal to `Unknown`, but that is a skip, not an
/// attempt-and-fail). An admitted mutant maps `Proven → Survived`, `Refuted/Unknown →
/// Killed`. Determinism: a pure function of `admitted` + the verdict (R-CODE-5).
#[must_use]
pub fn lean_mutant_outcome(admitted: bool, verdict: &Verdict) -> LeanMutantOutcome {
    if !admitted {
        // The fragment did not admit the mutant; it was never attempted. "Untested
        // against lean" (REQ-9), distinct from `Unknown-after-attempt`.
        return LeanMutantOutcome::UntestedAgainstLean;
    }
    match verdict {
        Verdict::Proven(_) => LeanMutantOutcome::Survived,
        // Refuted (a witnessed countermodel) or Unknown-after-attempt → Killed (the
        // shipped `Counterexample ∪ Timeout` = killed, generalized).
        Verdict::Refuted(_) | Verdict::Unknown(_) => LeanMutantOutcome::Killed,
    }
}

/// The running tally of a Lean-path mutation battery (`.design/verified/
/// proof-backends.md` REQ-9). Accumulates `killed` / `attempted` (= attempted minus
/// proven-equivalent, the shipped `scored` denominator) / `equivalent` / `untested`
/// (the "untested against lean" count reported in the cert, never counted killed). The
/// kill ratio is `killed / attempted`; the untested count is outside the denominator
/// so an untested mutant cannot inflate the ratio.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeanMutationTally {
    /// Mutants the Lean engine killed (`Refuted ∪ Unknown-after-attempt`).
    pub killed: usize,
    /// The denominator: mutants attempted (admitted by the fragment) minus the
    /// proven-equivalent (the #101 exclusion the caller applies).
    pub attempted: usize,
    /// Proven-equivalent mutants (excluded from both `killed`-eligible survivors and
    /// the `attempted` denominator, the shipped #101 exclusion).
    pub equivalent: usize,
    /// "Untested against lean": mutants no Lean fragment admitted (never counted
    /// killed; reported so the auditor sees the coverage gap, §7 honesty).
    pub untested: usize,
}

impl LeanMutationTally {
    /// Record one classified mutant (`.design/verified/proof-backends.md` REQ-9).
    /// `proven_equivalent` is the shipped #101 equivalence-probe result for a survived
    /// mutant (a §0.1 meta-query, outside the Engine interface in v1, a direct verus
    /// query the caller threads): an equivalent survivor is dropped from both the
    /// survivor set and the denominator (it is not a genuine survivor).
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
                    // both the survivor set and the denominator (never a spurious
                    // survivor, never in the ratio).
                    self.equivalent += 1;
                } else {
                    // A distinguishing survivor: in the denominator, not
                    // killed.
                    self.attempted += 1;
                }
            }
        }
    }

    /// The kill ratio `killed / attempted` (`.design/verified/proof-backends.md`
    /// REQ-9). The `untested` count is outside the denominator, so an untested mutant
    /// cannot inflate the ratio. A `0` denominator (no attempted-and-non-equivalent
    /// mutant) is the shipped `0/0` backstop → `0.0` (below any positive floor).
    #[must_use]
    pub fn kill_ratio(&self) -> f64 {
        if self.attempted == 0 {
            0.0
        } else {
            self.killed as f64 / self.attempted as f64
        }
    }

    /// A human qualifier line (`.design/verified/proof-backends.md` REQ-9 floor guard
    /// 1): the kill ratio with the untested-against-lean count beside it, so a `1/1`
    /// ratio with N untested mutants does not read as a clean `1.00` without the
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

    /// Does the Lean-path kill ratio meet the mutation floor (`.design/verified/
    /// proof-backends.md` REQ-9/AC-7, the floor gates the Lean path, the #248 fix)?
    /// Mirrors the SHIPPED `mutation::MutationScore::meets_floor`: `kill_ratio() >=
    /// floor`. The `0/0` backstop (`kill_ratio() == 0.0`) is below any positive floor,
    /// so an item that generated mutants but attempted none against Lean (all untested)
    /// does not meet the floor (never a vacuous pass, §7 / R-DEFER-9). Deterministic
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
    /// (`.design/verified/proof-backends.md` REQ-9/AC-7). The Lean-only tally does not
    /// track an individual survivor body (the #101 equivalence probe is a §0.1 verus
    /// meta-query outside this path, so survivors are reported as a count, not a named
    /// mutant), so the detail states the survivor/untested counts that put the
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

    // REQ-2 / AC-5 (the shared certify-time axiom gate, hoisted onto EVERY Lean discharge
    // path including the auto tiers): `certify_lean_axioms` accepts a clean report, and
    // REFUSES a fourth (Classical-adjacent) axiom by name, a surviving `sorry`, and a
    // missing report — hermetically, over synthetic `#print axioms` output anchored on the
    // obligation theorem. This is the gate `run_lake` now runs on a clean lake exit, so a
    // smuggled axiom / `sorry` can no longer be certified `Proven` on the auto path.
    #[test]
    fn certify_lean_axioms_gate_accepts_clean_refuses_smuggled() {
        let item = "isqrt";
        let thm = "thermite_obligation_isqrt";
        // Clean: the obligation theorem rests only on the allowlist.
        let clean = format!("'{thm}' depends on axioms: [propext, Classical.choice, Quot.sound]");
        assert!(
            certify_lean_axioms(
                "theorem thermite_obligation_isqrt := by decide",
                &clean,
                item
            )
            .is_ok(),
            "an allowlisted axiom set must certify"
        );
        // A fourth, Classical-adjacent axiom is REFUSED by name.
        let smuggled =
            format!("'{thm}' depends on axioms: [propext, Classical.choice, Classical.em]");
        let err = certify_lean_axioms(
            "theorem thermite_obligation_isqrt := proof",
            &smuggled,
            item,
        )
        .expect_err("a fourth axiom must be refused");
        assert!(
            err.contains("Classical.em") && err.contains("non-standard axiom"),
            "the refusal names the smuggled axiom: {err}"
        );
        // A surviving `sorry` is REFUSED (lake exits 0 on a sorry; the gate catches it).
        let sorry_out = format!("'{thm}' depends on axioms: [propext, sorryAx]");
        let err = certify_lean_axioms(
            "theorem thermite_obligation_isqrt := by sorry",
            &sorry_out,
            item,
        )
        .expect_err("a sorry must be refused");
        assert!(err.contains("sorry"), "the refusal names the sorry: {err}");
        // A MISSING report (no line anchored on the obligation theorem) is REFUSED — the
        // enumerable base cannot be vouched for (never silently certified).
        let missing = "'some_other_lemma' depends on axioms: [propext]";
        let err = certify_lean_axioms(
            "theorem thermite_obligation_isqrt := by decide",
            missing,
            item,
        )
        .expect_err("a missing report must be refused");
        assert!(
            err.contains("axiom report missing"),
            "the refusal flags the missing anchor: {err}"
        );
    }

    // REQ-2 / AC-5 ("Every ExportRefusal variant has at least one test"; the refusal
    // inventory stays LOUD and COMPLETE): construct EACH of the seven structured refusal
    // variants and assert its `Display` renders the variant's class marker. The behavioral
    // construction paths are covered elsewhere (undefined-callee → IncompleteRegistry,
    // while-body → LoopBody, optres-result → OptResResult, capture-unsafe / out-of-spine →
    // OutOfFragment, open-hole → OpenHole); this pins the COMPLETE inventory so a variant
    // cannot be silently dropped (the foundation must PRESERVE every variant, REQ-2).
    #[test]
    fn export_refusal_inventory_is_complete() {
        use crate::lean_export::ExportRefusal;
        let cases = [
            (
                ExportRefusal::OutOfFragment("Expr::Tuple".to_string()),
                "out-of-fragment",
            ),
            (
                ExportRefusal::NotPureContract("fx alloc".to_string()),
                "not a pure-contract",
            ),
            (
                ExportRefusal::IncompleteRegistry(vec!["spec_helper".to_string()]),
                "incomplete registry",
            ),
            (ExportRefusal::NonIntResult("String".to_string()), "result"),
            (ExportRefusal::OpenHole("?0".to_string()), "open body hole"),
            (ExportRefusal::LoopBody("while".to_string()), "loop"),
            (ExportRefusal::OptResResult("Option".to_string()), "result"),
        ];
        assert_eq!(
            cases.len(),
            7,
            "the ExportRefusal inventory is exactly seven variants"
        );
        for (refusal, marker) in cases {
            let rendered = format!("{refusal}").to_ascii_lowercase();
            assert!(
                rendered.contains(marker),
                "ExportRefusal::{refusal:?} must render its class marker `{marker}`: {rendered}"
            );
        }
    }

    // REQ-2(b): a `Proved` outcome maps to `Proven` with the discharged count.
    // Expected from the design's discharge map (`Proved` → `Proven`), R-CHAR-3.
    #[test]
    fn proved_maps_to_proven() {
        let v = VerusEngine.verdict_of(&VerusOutcome::Proved { verified: 3 }, a_key());
        match v {
            Verdict::Proven(e) => assert_eq!(e.verified, 3),
            other => panic!("expected Proven, got {other:?}"),
        }
    }

    // REQ-2(b): a `Timeout` outcome maps to `Unknown(VerusTimeout)` (degrade).
    // Expected from the design's discharge map (`Timeout` → `Unknown`), R-CHAR-3.
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

    // REQ-3.1 — the fast-unknown remap: a witness-less `Counterexample` (no parsed
    // `--> span`, the synthetic fallback) maps to `Unknown(IncompleteUnknown)`,
    // not `Refuted`. Expected from REQ-3.1's decision (R-CHAR-3), the sole
    // behavioral delta.
    #[test]
    fn witnessless_counterexample_remaps_to_unknown() {
        let witnessless = vec![ObligationResult::failed(
            "verus reported obligation failure",
            None, // no witnessing input (the fast-`unknown` edge).
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

    // REQ-3.1 / REQ-3 anti-cheat: a witnessed `Counterexample` (≥1 parsed `-->
    // span`) stays `Refuted` (hard-fail, never degrades). Expected from REQ-3.1
    // ("a witnessed countermodel stays Refuted"), R-CHAR-3.
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

    // REQ-3.1: the narrow incompleteness discriminator. Only a span-less failure
    // carrying the SMT-`unknown` signature (no frontend error) is the fast-`unknown`
    // that degrades; a witnessed countermodel, a frontend type error, and a bare
    // failure all stay `Refuted`. This keeps the corpus byte-identical.
    #[test]
    fn incompleteness_discriminator_is_narrow() {
        // (1) A parsed `--> span` is a witnessed countermodel, not remapped.
        let with_loc = vec![ObligationResult::failed(
            "postcondition not satisfied",
            Some("a:1:1".to_string()),
            Some("error: postcondition not satisfied".to_string()),
        )];
        assert!(!counterexample_is_incompleteness_unknown(&with_loc));
        // (2) A frontend type error (`error[E0308]`) is a rejection, not
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
        // (3) The genuine SMT-`unknown` signature is remapped (degrade, REQ-3.1).
        let unknown = vec![ObligationResult::failed(
            "verus reported obligation failure",
            None,
            Some("error: Z3 returned unknown".to_string()),
        )];
        assert!(counterexample_is_incompleteness_unknown(&unknown));
        // (4) A bare span-less failure with no `unknown` signal is not remapped
        // (conservative: keep the shipped hard-fail).
        let bare = vec![ObligationResult::failed("e", None, Some("d".to_string()))];
        assert!(!counterexample_is_incompleteness_unknown(&bare));
        // (5) An empty list is not remapped.
        assert!(!counterexample_is_incompleteness_unknown(&[]));
    }

    // REQ-3.1 / cert-oracle: a frontend type error (`error[E0308]`, span-less)
    // stays `Refuted` → hard fail, the provenance `careless_query` L0 the corpus
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
    // The live Lean engine #2 tests (REQ-6/REQ-7; the #240 chain). These construct a
    // `LeanEngine` directly (the design's "constructed directly by tests") and invoke
    // lake live (lake present at ~/.elan). A live test gates on lake presence so the
    // suite is green without lake (the verdict there is `Unknown`, never a false
    // `Proven`/`Refuted`). Every expected verdict is hand-derived (R-CHAR-3): a
    // correct contract kernel-accepts (Proven), a wrong one fails the tactic
    // (Unknown, never Refuted), the omitted/divergent shapes refuse the export (the
    // Pin E/F Rust mirror), an out-of-fragment item is skipped. The helpers use
    // `assert!`/`matches!` (not unwrap/expect/panic) so the anti-pattern gate is
    // clean on the edit (R-APG-2).
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

    // Resolve a fn fixture (asserting present + a fn). Used by the live nlsat tests.
    fn fn_of<'a>(program: &'a Program, name: &str) -> &'a thermite_syntax::FnItem {
        match crate::lean_export::find_item(program, name) {
            Some(thermite_syntax::Item::Fn(f)) => f,
            other => panic!("fixture fn `{name}` must be present and a fn, got {other:?}"),
        }
    }

    // REQ-8 / AC-12 (LIVE, z3-gated): the isqrt characterization — `r*r<=n ∧
    // n<(r+1)² ∧ 1<=r → r<=n` — is a real-valid universal polynomial implication, so
    // the nlsat relax route discharges it `Proved` (unsat over ℝ → integer-valid by
    // r_relax_sound → L4). z3-absent SKIPs (CI shards have no z3), mirroring the
    // sibling lake-gated live tests.
    #[test]
    fn live_nlsat_isqrt_characterization_is_proved() {
        if !NlsatEngine::z3_present() {
            eprintln!("SKIP: z3 not present — the live nlsat relax route is not run.");
            return;
        }
        let program = parse_program(
            "fn isqrt_bound(n: u64, r: u64) -> u64\n  \
             req r * r <= n && n < (r + 1) * (r + 1) && 1 <= r\n  \
             ens r <= n\n  fx pure\n{ r }\n",
        );
        let engine = NlsatEngine::new(program.clone());
        assert!(
            engine.admits_relax("isqrt_bound"),
            "the isqrt characterization is relaxable"
        );
        assert_eq!(
            engine.discharge_relax(fn_of(&program, "isqrt_bound")),
            NlsatOutcome::Proved,
            "the real relaxation is QF_NRA-valid → L4 Proved"
        );
    }

    // REQ-8 / AC-12 (LIVE, z3-gated): `∀ n. n*n ≠ 2` is true over ℤ but false over ℝ
    // (n = √2). The relax route's integrality check finds NO integer falsifier in the
    // radius-2 box → a `RealWitness` carrying the raw real point (√2), NEVER a
    // `Counterexample`.
    #[test]
    fn live_nlsat_n_squared_ne_two_is_real_witness() {
        if !NlsatEngine::z3_present() {
            eprintln!("SKIP: z3 not present — the live nlsat relax route is not run.");
            return;
        }
        let program =
            parse_program("fn sq(n: u64) -> u64\n  req true\n  ens n * n != 2\n  fx pure\n{ n }\n");
        let engine = NlsatEngine::new(program.clone());
        match engine.discharge_relax(fn_of(&program, "sq")) {
            NlsatOutcome::RealWitness { point } => {
                let n = point
                    .assignment
                    .iter()
                    .find(|(v, _)| v == "n")
                    .map(|(_, x)| x.clone())
                    .unwrap_or_default();
                assert!(
                    n.starts_with("1.41"),
                    "the raw real point carries n ≈ √2 (got {n})"
                );
            }
            other => {
                panic!("`∀ n. n*n≠2` must yield RealWitness, never Counterexample; got {other:?}")
            }
        }
    }

    // REQ-8 (LIVE, z3-gated): a contract false over ℤ with an integer falsifier
    // (`n+1 <= n`) yields a genuine integer `Counterexample` (not a RealWitness) — the
    // integrality check finds the integer witness in the box.
    #[test]
    fn live_nlsat_integer_counterexample_is_caught() {
        if !NlsatEngine::z3_present() {
            eprintln!("SKIP: z3 not present — the live nlsat relax route is not run.");
            return;
        }
        let program = parse_program(
            "fn bad(n: u64) -> u64\n  req true\n  ens n + 1 <= n\n  fx pure\n{ n }\n",
        );
        let engine = NlsatEngine::new(program.clone());
        match engine.discharge_relax(fn_of(&program, "bad")) {
            NlsatOutcome::Counterexample { integer_point } => {
                assert!(
                    integer_point.iter().any(|(v, _)| v == "n"),
                    "the integer counterexample names `n`"
                );
            }
            other => panic!("`n+1<=n` is false over ℤ → Counterexample; got {other:?}"),
        }
    }

    // REQ-8b (no z3 needed): a div-containing contract is NOT relaxable, so the route
    // skips it (an honest `Unknown`, never a verdict) — the fragment gate.
    #[test]
    fn nlsat_div_clause_is_not_relaxable() {
        let program = parse_program(
            "fn d(n: u64) -> u64\n  req true\n  ens result == n / 2\n  fx pure\n{ n }\n",
        );
        let engine = NlsatEngine::new(program.clone());
        assert!(
            !engine.admits_relax("d"),
            "a `/` clause is out of the relax fragment"
        );
        assert!(
            matches!(
                engine.discharge_relax(fn_of(&program, "d")),
                NlsatOutcome::Unknown(_)
            ),
            "a non-relaxable contract is an honest skip"
        );
    }

    // Build a contract obligation for a named fn (asserting it exists + is a fn, no
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

    // (1) REQ-6/REQ-7: a hand-authored pure-contract scalar item kernel-accepts live
    // (Proven). `add` returns `a as u64 + b as u64` and `ens result == a as u64 + b
    // as u64`: the body is the ens RHS, so after binding `result` to the body's
    // stabilized value the goal is true; the fuel-free tier-(a) battery kernel-checks
    // it. Expected from §6.1(a) (R-CHAR-3): a correct contract is Proven.
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
        let v = engine.discharge(&o, &CovenantRecord::none());
        assert!(
            matches!(v, Verdict::Proven(_)),
            "a CORRECT scalar contract must be Proven LIVE: {v:?}"
        );
    }

    // REQ-6 / AC-10 (increment 2d, anti-Goodhart defense (a)): the arbitrary-result
    // re-elaboration REJECTS a body-ignoring `ens`. `ens x > 0` (given `req x > 0`)
    // says nothing about `result` — it holds for an ARBITRARY result, so the harness
    // (which binds `result` to a fresh `r : Int`) kernel-accepts → `Tautology`. (LIVE:
    // needs the built Lean spine; skips if lake is absent, like the sibling live tests.)
    #[test]
    fn live_arbitrary_result_rejects_body_ignoring_ens() {
        if !lake_present() {
            eprintln!("SKIP: lake not present — live arbitrary-result tautology test not run.");
            return;
        }
        let src = "fn f(x: u32) -> u32 req x > 0 ens x > 0 fx pure { x }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "f", vec![]);
        let engine = LeanEngine::new(p, lean_root());
        let outcome = engine.arbitrary_result_reelaboration(&o);
        assert_eq!(
            outcome,
            ArbitraryResultOutcome::Tautology,
            "a body-ignoring `ens x > 0` must re-elaborate for an arbitrary result \
             (tautology): {outcome:?}"
        );
    }

    // REQ-6 / AC-10 (increment 2d): the contrast — a body-CONSTRAINING `ens` is Clean.
    // `ens result == x + 1` does NOT hold for an arbitrary result (only for `r = x+1`),
    // so the arbitrary-result harness fails to elaborate → `Clean` (the gate does NOT
    // reject a genuine, body-pinning contract).
    #[test]
    fn live_arbitrary_result_clean_for_body_constraining_ens() {
        if !lake_present() {
            eprintln!("SKIP: lake not present — live arbitrary-result clean test not run.");
            return;
        }
        let src = "fn g(x: u32) -> u32 req x < 100 ens result == x + 1 fx pure { x + 1 }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "g", vec![]);
        let engine = LeanEngine::new(p, lean_root());
        let outcome = engine.arbitrary_result_reelaboration(&o);
        assert_eq!(
            outcome,
            ArbitraryResultOutcome::Clean,
            "a body-constraining `ens result == x + 1` must NOT re-elaborate for an \
             arbitrary result (clean): {outcome:?}"
        );
    }

    // (2) REQ-7 §6.1(b): a tier-(b) item (a non-recursive spec-fn in the ens) is
    // statically unfolded to a fuel-free goal and kernel-accepts live (Proven). `g`
    // returns `x + x` and `ens result as int == dbl(x as int)` where `spec fn dbl(x)
    // = x + x`: the unfolded ens is `result as int == (x as int) + (x as int)`,
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
        let v = engine.discharge(&o, &CovenantRecord::none());
        assert!(
            matches!(v, Verdict::Proven(_)),
            "a tier-(b) item must be Proven LIVE via static unfold: {v:?}"
        );
    }

    // (3) REQ-7 / REQ-3 anti-cheat: a wrong contract (`ens result == 0` for a body
    // that returns `a`) makes the auto battery fail → `Unknown`, never `Refuted` (a
    // Lean tactic failure is not a witnessed countermodel) and never `Proven`.
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
        let v = engine.discharge(&o, &CovenantRecord::none());
        assert!(
            !matches!(v, Verdict::Refuted(_)),
            "a tactic FAILURE is Unknown, NEVER Refuted (REQ-3 anti-cheat): {v:?}"
        );
        assert!(
            matches!(v, Verdict::Unknown(_)),
            "a WRONG contract must be Unknown (not Proven): {v:?}"
        );
    }

    // (4) REQ-6 §4 hard gate, the Pin E/F Rust mirror: an omitted-registry obligation
    // (the ens calls `spec_sum` but the obligation's closure does not list it) refuses
    // the export → `Unknown` (a skip), never a bottom-poisoned `Proven`. The Rust
    // mirror of the divergent/omitted Lean pins. Expected from §4 mechanism 1
    // (R-CHAR-3). No lake needed.
    #[test]
    fn omitted_registry_obligation_refuses_export() {
        let src = "spec fn spec_sum(xs: &[u32]) -> u64 dec xs.len() { 0 } \
                   fn f(xs: &[u32]) -> u64 req true ens result == spec_sum(xs) fx pure { 0 }";
        let p = parse_program(src);
        // The obligation's closure omits `spec_sum` (the bug the gate must catch).
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
        // The engine maps the refusal to Unknown (a skip), never Proven/Refuted.
        let engine = LeanEngine::new(p, lean_root());
        let v = engine.discharge(&o, &CovenantRecord::none());
        assert!(
            matches!(v, Verdict::Unknown(_)),
            "a refused export is an Unknown skip, never a verdict: {v:?}"
        );
    }

    // ========================================================================
    // The exec-body bridge live + refusal tests (§4.1 / REQ-10, increment (iv-b),
    // blocker #253). A straight-line-body item exports the hypothesize-contract
    // theorem + the conjoined overflow theorem over `bodyDenote`/`stateOf`; a
    // bool-result item routes through `bindBool`; an always-overflow body's vacuous
    // contract is blocked by the failing overflow conjunct; while/optres refuse.
    // Verdicts hand-derived (R-CHAR-3) from §4.1.5 + PinExecOverflowVacuity.
    // ========================================================================

    // (7) REQ-10.3/10.4: a straight-line-body int item is Proven live (incl. the
    // overflow conjunct). `id2`'s body `{ let y = x; y }` threads `y ↦ x`, tail `y`,
    // so the result `r = x` and `ens result == x` holds at `bindResult`; the body has
    // no overflow site so the overflow conjunct discharges. Both theorems kernel-accept
    // in the same emitted file. Expected from §4.1.5 (R-CHAR-3).
    #[test]
    fn live_straight_line_body_is_proven() {
        if !lake_present() {
            eprintln!("SKIP: lake not present — live straight-line-body test not run.");
            return;
        }
        let src = "fn id2(x: u64) -> u64 req true ens result == x fx pure { let y = x; y }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "id2", vec![]);
        // Sanity: the export emits both the contract theorem and the conjoined overflow
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
        let v = engine.discharge(&o, &CovenantRecord::none());
        assert!(
            matches!(v, Verdict::Proven(_)),
            "a correct straight-line-body item must be Proven LIVE (incl. the OVERFLOW \
             conjunct): {v:?}"
        );
    }

    // (8) REQ-10.2: a bool-result straight-line item is Proven live via the `bindBool`
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
        let v = engine.discharge(&o, &CovenantRecord::none());
        assert!(
            matches!(v, Verdict::Proven(_)),
            "a correct bool-result item must be Proven LIVE via bindBool: {v:?}"
        );
    }

    // (9) REQ-10.4 / the conjunction rule: an always-overflow body with a vacuous-looking
    // ens does not certify: the overflow conjunct fails so the live verdict is not Proven
    // (the conjunction working end-to-end, the PinExecOverflowVacuity Rust mirror). `ovf`'s
    // body `{ let a = m + m; a }` overflows `u64` when `m` is at the rim; the contract
    // theorem may be vacuously provable but the conjoined overflow theorem `bodyDenote
    // |>.isSome` is false under no precondition bounding `m` away from the rim, so the
    // single emitted file does not kernel-accept (the overflow theorem fails). Expected
    // from §4.1.5 + PinExecOverflowVacuity (R-CHAR-3).
    #[test]
    fn live_always_overflow_body_is_not_proven() {
        if !lake_present() {
            eprintln!("SKIP: lake not present — live overflow-vacuity test not run.");
            return;
        }
        // No req bounds `m`, so `m + m` can overflow `u64`; the overflow conjunct fails.
        let src = "fn ovf(m: u64) -> u64 req true ens result < result fx pure { let a = m + m; a }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "ovf", vec![]);
        let engine = LeanEngine::new(p.clone(), lean_root());
        let v = engine.discharge(&o, &CovenantRecord::none());
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

    // (10) REQ-11.4/11.5: the narrowed refusal boundary (the #264 v-b
    // narrowing of the iv-b refusal, O-5). The v1 while shape now exports (it is in the
    // §4.2.1 grammar: a single `while` with non-empty `invs` + `dec`, a straight-line
    // scalar body, the last statement before a required tail). So the old blanket
    // `while_body_item_refuses_export` no longer holds for the v1 shape; this test pins
    // the new boundary: the v1 `count` shape exports (REQ-11.4), while a non-v1 loop (a
    // `loop`-kind multi-exit, §4.2.5) still refuses `ExportRefusal::LoopBody`. The
    // narrowing is cited to §4.2.1 (R-HONEST-4).
    #[test]
    fn while_body_item_refuses_export() {
        // The v1 while shape (the §4.2.1 grammar) now exports (the (v-b) widening).
        let v1_src = "fn count(n: u64) -> u64 req true ens result == n fx pure \
                      { let mut lo = 0; while lo < n inv lo <= n dec n - lo { lo = lo + 1; } lo }";
        let p1 = parse_program(v1_src);
        let o1 = fn_obligation(&p1, "count", vec![]);
        if let Some(item) = crate::lean_export::find_item(&p1, "count") {
            let r = export_item(&o1, &p1, item);
            assert!(
                r.is_ok(),
                "the v1 WHILE shape (§4.2.1) now EXPORTS (the #264 v-b narrowing): {r:?}"
            );
        }

        // A `loop`-kind loop (the multi-exit CPS form, §4.2.5) still refuses with the
        // named structured reason (the refusal inventory is explicit, REQ-11.5).
        let loop_src = "fn lp(n: u64) -> u64 req true ens result == n fx pure \
                        { let mut lo = 0; loop inv lo <= n dec n - lo { lo = lo + 1; } lo }";
        let p2 = parse_program(loop_src);
        let o2 = fn_obligation(&p2, "lp", vec![]);
        if let Some(item) = crate::lean_export::find_item(&p2, "lp") {
            let r = export_item(&o2, &p2, item);
            assert!(
                matches!(&r, Err(ExportRefusal::LoopBody(d)) if d.contains("loop`-kind")),
                "a `loop`-kind loop must STILL refuse structurally (§4.2.5): {r:?}"
            );
        }
        let engine = LeanEngine::new(p2, lean_root());
        let v = engine.discharge(&o2, &CovenantRecord::none());
        assert!(
            matches!(v, Verdict::Unknown(_)),
            "a refused `loop`-kind body is an Unknown skip, never a verdict: {v:?}"
        );
    }

    // (11) REQ-10 / §4.1.3: an Option/Result-result item is refused (#254: `ExecVal`
    // has no optres variant). The export returns `ExportRefusal::OptResResult`; the
    // engine maps it to Unknown. Expected from §4.1.3 (R-CHAR-3). No lake needed.
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
        let v = engine.discharge(&o, &CovenantRecord::none());
        assert!(
            matches!(v, Verdict::Unknown(_)),
            "a refused optres body is an Unknown skip, never a verdict: {v:?}"
        );
    }

    // ========================================================================
    // The while-body widening (§4.2 / REQ-11, increment (v-b), blocker #264): the v1
    // while-shape exporter live oracles. A v1-shaped item exports the 5+2 obligation set
    // (recognizer mirroring `recognize_v1_loop`); under `--engine lean` the verdict is
    // `Proven` only from a kernel-accepted sorry-free discharge, else `Unknown`
    // (the §4.2.4 expected-coverage caveat), never `Refuted` without a witnessed
    // countermodel. Verdicts hand-derived (R-CHAR-3) from §4.2.3/§4.2.4.
    // ========================================================================

    // (O-1) REQ-11.4: the v1 linear-family while item exports the full 5+2 obligation
    // set, and the live verdict is never `Refuted` (no witnessed countermodel),
    // and `Proven` only from a kernel-accepted sorry-free discharge, else `Unknown` (the
    // §4.2.4 expected-coverage caveat: the auto battery degrades nonlinear/step-decode
    // residuals to a sound fail-to-certify). The export emits `Inv_item`/`mu_item`, the
    // five per-item obligations, and the two composed theorems (`while_compose` /
    // `loopDenote_exits_of_dec`). Expected from §4.2.4 (R-CHAR-3): the emission is correct;
    // the verdict is sound either way (never a false Proven, never a false Refuted).
    #[test]
    fn live_while_body_item_is_honest() {
        let src = "fn count(n: u64) -> u64 req true ens result == n fx pure \
                   { let mut lo = 0; while lo < n inv lo <= n dec n - lo { lo = lo + 1; } lo }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "count", vec![]);
        // The recognizer accepts the v1 shape and emits the 5+2 obligation set.
        if let Some(item) = crate::lean_export::find_item(&p, "count") {
            let exported = export_item(&o, &p, item);
            assert!(
                exported.is_ok(),
                "the v1 while item must EXPORT: {exported:?}"
            );
            if let Ok(e) = &exported {
                assert!(
                    e.source.contains("def Inv_item") && e.source.contains("def mu_item"),
                    "emits Inv_item + mu_item (§4.2.4): {}",
                    e.source
                );
                for suffix in [
                    "_entry",
                    "_pres",
                    "_progress",
                    "_dec",
                    "_exit",
                    "_converges",
                ] {
                    assert!(
                        e.source
                            .contains(&format!("thermite_obligation_count{suffix}")),
                        "emits the {suffix} obligation theorem: {}",
                        e.source
                    );
                }
                assert!(
                    e.source.contains("Thermite.Exec.while_compose")
                        && e.source.contains("Thermite.Exec.loopDenote_exits_of_dec"),
                    "the composed theorems apply the (v-a) spine lemmas: {}",
                    e.source
                );
                assert!(
                    e.source.contains("Thermite.Exec.whileBodyConverges"),
                    "the CONTRACT theorem binds the result THROUGH whileBodyConverges: {}",
                    e.source
                );
            }
        }
        if !lake_present() {
            eprintln!("SKIP: lake not present — live while-body verdict not run.");
            return;
        }
        let engine = LeanEngine::new(p.clone(), lean_root());
        let v = engine.discharge(&o, &CovenantRecord::none());
        // The O-1 gate (R-1): the L1 linear family certifies; both composed
        // theorems kernel-accept (sorry-free, the standard axioms), so the live verdict is
        // `Proven` via lean-auto. Never `Refuted` (a refutation needs a witnessed
        // countermodel; a tactic failure is `Unknown`, the REQ-3 anti-cheat).
        assert!(
            !matches!(v, Verdict::Refuted(_)),
            "a while-body item is NEVER Refuted without a witnessed countermodel \
             (REQ-3 anti-cheat — a tactic failure is Unknown): {v:?}"
        );
        assert!(
            matches!(v, Verdict::Proven(_)),
            "the L1 linear-family while item CERTIFIES (both composed theorems \
             kernel-accept — R-1): {v:?}"
        );
    }

    // (O-3) REQ-11.6 / §4.2.3: the termination-vacuity gate (the PinWhileVacuity Rust
    // mirror). A `while`-true-shaped body whose loop never exits must not certify L3 via a
    // vacuous contract: the conjoined `_converges` obligation (`∃ r, whileBodyConverges …`)
    // fails at the non-terminating env, so the item is Unknown/degraded, never Proven. The
    // shape `while 0 < 1 inv lo <= lo dec 0 { lo = lo; }` runs forever (cond constant true,
    // measure constant), so the `_converges`/`_dec` obligations cannot discharge. Expected
    // from §4.2.3 (R-CHAR-3): the vacuous contract discharge is unreachable as a
    // certificate (the conjunction gate); never a silent L3 on a non-terminating body.
    #[test]
    fn live_while_true_vacuity_is_not_proven() {
        // A non-exiting loop: `0 < 1` is constantly true, the measure `0` never descends.
        let src = "fn spin(lo: u64) -> u64 req true ens result == lo fx pure \
                   { let mut acc = lo; while 0 < 1 inv acc <= acc dec acc - acc { acc = acc; } acc }";
        let p = parse_program(src);
        let o = fn_obligation(&p, "spin", vec![]);
        if !lake_present() {
            // Even without lake, the export must succeed (the shape is in-grammar); the
            // gate is at discharge, not export. A `dec`/measure that never descends makes
            // the `_dec`/`_converges` obligation fail, so the item degrades.
            eprintln!("SKIP: lake not present — the while-true vacuity discharge not run.");
            return;
        }
        let engine = LeanEngine::new(p.clone(), lean_root());
        let v = engine.discharge(&o, &CovenantRecord::none());
        assert!(
            !matches!(v, Verdict::Proven(_)),
            "a non-terminating `while true`-shaped body must NOT certify L3 — the conjoined \
             `_converges` obligation fails (the §4.2.3 termination-vacuity gate, \
             PinWhileVacuity mirror): {v:?}"
        );
        assert!(
            !matches!(v, Verdict::Refuted(_)),
            "a failed termination obligation is Unknown, NEVER Refuted (REQ-3): {v:?}"
        );
    }

    // (O-2) REQ-11.5: the §4.2.5 refusal inventory (each its own structured refusal, the
    // expected class from §4.2.5, hand-derived, not from running the tool, R-CHAR-3).
    // Every out-of-v1 shape gets a named structured `ExportRefusal` (never silent, never a
    // false verdict); under `--engine lean` each is the `Unknown` skip.
    #[test]
    fn while_refusal_inventory_is_structured() {
        // Each case: (name, source, the refusal predicate). The shapes are the §4.2.5
        // enumeration; the expected refusal class is derived from the design, not the tool.
        let nested = "fn f(n: u64) -> u64 req true ens result == n fx pure \
            { let mut lo = 0; while lo < n inv lo <= n dec n - lo \
              { while lo < n inv lo <= n dec n - lo { lo = lo + 1; } } lo }";
        let brk = "fn f(n: u64) -> u64 req true ens result == n fx pure \
            { let mut lo = 0; while lo < n inv lo <= n dec n - lo { break; } lo }";
        let cont = "fn f(n: u64) -> u64 req true ens result == n fx pure \
            { let mut lo = 0; while lo < n inv lo <= n dec n - lo { continue; } lo }";
        let mid_return = "fn f(n: u64) -> u64 req true ens result == n fx pure \
            { let mut lo = 0; while lo < n inv lo <= n dec n - lo { return lo; } lo }";
        let weak_inv = "fn f(n: u64) -> u64 req true ens result == n fx pure \
            { let mut lo = 0; while lo < n inv true dec n - lo { lo = lo + 1; } lo }";

        for (name, src) in [
            ("nested-while", nested),
            ("break", brk),
            ("continue", cont),
            ("mid-return", mid_return),
            ("weak-inv-true", weak_inv),
        ] {
            let p = parse_program(src);
            let o = fn_obligation(&p, "f", vec![]);
            if let Some(item) = crate::lean_export::find_item(&p, "f") {
                let r = export_item(&o, &p, item);
                assert!(
                    matches!(&r, Err(ExportRefusal::LoopBody(_))),
                    "the `{name}` shape must REFUSE structurally (§4.2.5, LoopBody): {r:?}"
                );
                // Under `--engine lean` the refusal is the Unknown skip.
                let engine = LeanEngine::new(p.clone(), lean_root());
                let v = engine.discharge(&o, &CovenantRecord::none());
                assert!(
                    matches!(v, Verdict::Unknown(_)),
                    "the `{name}` refusal is an Unknown skip, never a verdict: {v:?}"
                );
            }
        }

        // A non-scalar assign in the loop body (`xs[i] = e`) is out of v1 (§4.2.5). The
        // recognizer rejects it before encoding.
        let non_scalar = "fn g(xs: &[u32], n: u64) -> u64 req true ens result == n fx pure \
            { let mut lo = 0; while lo < n inv lo <= n dec n - lo { xs[lo] = lo; lo = lo + 1; } lo }";
        let p = parse_program(non_scalar);
        let o = fn_obligation(&p, "g", vec![]);
        if let Some(item) = crate::lean_export::find_item(&p, "g") {
            let r = export_item(&o, &p, item);
            assert!(
                matches!(&r, Err(ExportRefusal::LoopBody(_))),
                "a non-scalar assign body must REFUSE (§4.2.5): {r:?}"
            );
        }

        // A spec-calling invariant, the (v) v1 residual (§4.2.1): out-of-shallow-fragment.
        let spec_inv = "spec fn s(xs: &[u32]) -> u64 dec xs.len() { 0 } \
            fn h(xs: &[u32], n: u64) -> u64 req true ens result == n fx pure \
            { let mut lo = 0; while lo < n inv lo <= s(xs) dec n - lo { lo = lo + 1; } lo }";
        let p = parse_program(spec_inv);
        let o = fn_obligation(&p, "h", vec!["s".to_string()]);
        if let Some(item) = crate::lean_export::find_item(&p, "h") {
            let r = export_item(&o, &p, item);
            assert!(
                matches!(&r, Err(ExportRefusal::OutOfFragment(_))),
                "a spec-calling invariant is the (v) v1 residual (§4.2.1, OutOfFragment): {r:?}"
            );
        }
    }

    // (5) REQ-6 §4 scope: an out-of-fragment item (an out-of-spine struct-field
    // access in the ens, on an int-result fn so the #244 result-sort gate does not
    // pre-empt it) is skipped (the fragment rejects it) → the export refuses and the
    // engine returns `Unknown`. Expected from the §4 out-of-spine refusal rule
    // (R-CHAR-3). No lake.
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
        let v = engine.discharge(&o, &CovenantRecord::none());
        assert!(
            matches!(v, Verdict::Unknown(_)),
            "an out-of-fragment item is an Unknown skip: {v:?}"
        );
    }

    // REQ-7 §6.1(c): a recursive-registry item is tier (c) (interactive); the engine
    // returns `Unknown` without invoking lake (the `∃N∀fuel` form needs an authored
    // induction). The exported file is still produced (for increment-(iii)), marked
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
        let v = engine.discharge(&o, &CovenantRecord::none());
        assert!(
            matches!(v, Verdict::Unknown(_)),
            "a recursive (tier-c) item is INTERACTIVE Unknown: {v:?}"
        );
    }

    // ========================================================================
    // REQ-7(ii): the interactive proof artifact (skeleton emit + staleness gate +
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

    // REQ-7(ii) — skeleton emitted when absent: the first `replay_interactive` call on
    // an item with no artifact emits the skeleton (the evidence-key header + the
    // exported source) and returns `Unknown` ("skeleton emitted"), never `Proven`.
    // Expected from REQ-7(ii) (R-CHAR-3). No lake needed.
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

    // REQ-7(ii) — a stale hash → Unknown("stale proof — re-derive"): an artifact whose
    // header carries a different evidence key than the current obligation is stale and
    // is never silently reused. Expected from REQ-7(ii) (R-CHAR-3). No lake needed (the
    // staleness gate short-circuits before the replay).
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

        // Author an artifact with a wrong (stale) evidence-key header.
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

    // REQ-7(ii) — a sorry-carrying file → Unknown (never Proven), even though lake
    // exits 0 on a `sorry`. The artifact has the correct (fresh) key + a `sorry` body;
    // the explicit sorry detection (`proof_has_sorry`) blocks the `Proven`. Expected
    // from REQ-7(ii) (R-CHAR-3). Live (gated on lake).
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

        // Author an artifact with the correct (fresh) key but a `sorry`-carrying proof
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

    // REQ-7(ii) — a filled, valid, sorry-free proof replays Proven. Driving
    // `replay_interactive` with an auto-tier obligation emits a complete proof (the
    // `by first | decide | omega | …` battery, not a `sorry`), so the second call
    // (artifact present, key fresh) replays it: lake kernel-accepts the sorry-free
    // proof → `Proven`. This exercises the replay machinery on a kernel-accepted
    // proof (an auto-tier body is a complete authored proof). Expected
    // from REQ-7(ii) (R-CHAR-3). Live (gated on lake).
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

        // First call: absent → emit the complete (auto-tier) proof + header → Unknown.
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

        // Second call: present + fresh key + sorry-free + lake-accepted → Proven.
        let second = engine.replay_interactive(&file, &o);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            matches!(second, Verdict::Proven(_)),
            "a PRESENT, fresh-key, sorry-free, kernel-accepted proof REPLAYS Proven \
             (REQ-7(ii)): {second:?}"
        );
    }

    // REQ-6 statement binding (the #248 fix): a proof file with the correct (fresh)
    // evidence key but proving a different theorem statement (the trivial proposition,
    // not the obligation) is Unknown("statement mismatch"), never Proven; the file must
    // prove the obligation. The staleness gate passes (fresh key); the statement-binding
    // gate catches it before the (skipped) lake replay, so no lake is needed. R-DEFER-9.
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

        // Author an artifact with the correct (fresh) key but the trivial proposition:
        // a proof of the wrong statement, named like the obligation theorem.
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

    // REQ-6 / R-DEFER-9 (the #250 fix): reconstruct-and-splice, pure-function layer.
    // The splice helpers are the anti-decoy machinery: `declaration_sites`
    // counts the obligation short-name declaration sites (a same-short-name decoy in any
    // namespace → > 1 → reject), `proof_assign_pos` finds the proof `:=` past the
    // record-update `specs := R_item`, and `decl_block_end` bounds the declaration. No
    // lake needed (R-CHAR-3: the decoy game is structurally impossible by construction).
    #[test]
    fn reconstruct_splice_helpers_detect_decoy_and_splice_proof() {
        // (a) A single top-level declaration of the obligation → exactly one site.
        let single = "import Thermite.Stabilize\n\
                      theorem thermite_obligation_f : True := trivial\n";
        assert_eq!(declaration_sites(single, "thermite_obligation_f").len(), 1);

        // (b) The #250 decoy: a namespaced obligation declaration + a top-level
        // same-short-name decoy → two sites → the caller rejects as a duplicate.
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

        // (c) `proof_assign_pos` anchors on the proof `:=`, not the record-update `specs
        // := R_item` inside the proposition (the `:= by` form and the term-mode form).
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

        // (d) End-to-end reconstruct: a duplicate-decoy author file is rejected (the
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

        // (e) A single author declaration with an inline-`have` proof term splices: the
        // canonical statement is emitted verbatim, only the proof term is spliced (the #252
        // helper-surface elimination: any file-level helper is dropped), imports/R_item
        // come from the canonical preamble (exactly once), and our own anchored probe is
        // appended. A file-level `theorem my_helper` the author leaves outside the proof
        // term is dropped (it has nowhere to live).
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

    // REQ-6 / §1 / R-DEFER-9 (the #252 belt): the proof-term command scan. The proof term
    // is the only author-controlled text and is type-checked against the fixed generator
    // goal, so this is a defense layer against an `… in`-style command form smuggled into
    // the term. A genuine term/tactic proof (with inline `have`/`let`/`suffices`) carries
    // no command keyword; an `open … in` / `set_option … in` / `#…` form is caught
    // position-independently (exact-token). No lake needed (R-CHAR-3: a structural scan).
    #[test]
    fn proof_term_command_token_scans_position_independently() {
        // Permitted: a genuine tactic/term proof, including inline `have`/`let`/`suffices`
        // auxiliaries and identifiers that merely contain a keyword (`openVal`,
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

        // Rejected: a top-level command keyword smuggled into the term (e.g. via `… in`),
        // exact-token, in any position and with no surrounding spaces (the `(open Foo in …)`
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

    // REQ-6 / §1 / R-DEFER-9 (the #252 helper-surface elimination): author content
    // outside the proof term is dropped, never spliced; the indented-command poison (the
    // #252 divergence) and the #251 macro-poison both have nowhere to live, so the
    // reconstructed file carries only the canonical preamble + the proof term + the anchored
    // probe. A genuine inline-`have` proof term still splices. No lake (a structural test).
    #[test]
    fn reconstruct_drops_author_helper_section() {
        let canonical = "import Thermite.Stabilize\n\n\
                         def R_item : Thermite.Registry := fun _ => none\n\n\
                         /-- doc -/\n\
                         theorem thermite_obligation_f (v : Thermite.Env) : True := by trivial\n";
        let p = parse_program("fn f(x: u32) -> u32 req true ens result == x fx pure { x }");
        let engine = LeanEngine::new(p, lean_root());

        // The #251 macro-poison + the #252 indented-command poison: both live in the author
        // file outside the obligation declaration's proof term. The reconstruction drops all
        // of it (the helper surface is eliminated), so the poison never reaches the emitted
        // file and can never re-elaborate the obligation. The proof term (`by trivial`)
        // splices onto the canonical statement.
        for poison in [
            // column-0 notation (the #251 form)
            "-- evidence_key: abc\n\
             import Thermite.Stabilize\n\
             def R_item : Thermite.Registry := fun _ => none\n\
             notation:max \"Thermite.stabilizesProp\" => (fun _ _ => True)\n\
             theorem thermite_obligation_f (v : Thermite.Env) : True := by trivial\n",
            // indented notation (the #252 form), attached to a dummy helper's body line
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

        // A genuine inline-have proof term still splices (no expressivity loss: a
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

        // The #252 belt: a proof term smuggling an `open … in` command form → rejected
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

    // REQ-6 / REQ-7(ii) / R-DEFER-9 (the #252 inline-have migration): live auxiliary-lemma
    // replay. After the #252 helper-surface elimination, a single-obligation proof inlines
    // its auxiliaries as `have` inside the proof term (no expressivity loss). A clean inline
    // `have` auxiliary, used, replays Proven (clean axiom base). A proof term that
    // leans on a `sorry` (the only way an inline auxiliary can introduce a non-standard
    // axiom, since file-level axioms are dropped) flows `sorryAx` into the obligation
    // theorem's anchored `#print axioms` → Unknown, never Proven. Expected from REQ-4/§1
    // (R-CHAR-3). Live (gated on lake).
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

        // Emit the skeleton, then lift its working `:= by …` proof body (the auto
        // battery) so the authored proof closes the canonical goal.
        let _ = engine.replay_interactive(&file, &o);
        let skeleton = std::fs::read_to_string(&artifact).unwrap_or_default();
        let stmt = canonical_theorem_statement(&skeleton, "add").unwrap_or_default();
        let by_pos = skeleton.find(":= by").unwrap_or(0);
        let body = skeleton[by_pos + ":= by".len()..].trim_end().to_string();

        // (1) Clean inline-have auxiliary, referenced (`have aux : True :=
        // True.intro`): Proven. The auxiliary lives inside the proof term (the #252 inline
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

        // (2) Sorry in the proof term: an inline auxiliary discharged by `sorry` flows
        // `sorryAx` into the obligation theorem's anchored `#print axioms` → Unknown (never
        // Proven). This is the only way an inline auxiliary can introduce a non-standard
        // axiom (file-level axioms are dropped, #252), so it exercises the axiom/sorry gate
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

    // REQ-4/§1/R-DEFER-9 (the #248 fix): the trust-base axiom allowlist parser is strict.
    // It anchors on the `#print axioms` report line ("depends on axioms: [...]"), so a
    // lake warning that itself carries a `[Thermite.Env.bindInt, …]` simp-arg list does
    // not false-positive, and a non-standard axiom (a smuggled cheat, the sorry axiom) is
    // caught. Pure-function regression for `nonstandard_axiom`.
    #[test]
    fn nonstandard_axiom_parses_the_report_line_strictly() {
        // The clean standard set on the obligation theorem's anchored line → Clean.
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
        // A warning whose simp-arg bracket list precedes the report line must not
        // false-positive; the parser anchors on the report marker, never the first
        // `[`. This is the legit-auto-replay output shape.
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
        // A smuggled non-standard axiom is caught (the divergence the #248 pin exhibits).
        assert_eq!(
            nonstandard_axiom(
                "'thermite_obligation_f' depends on axioms: [propext, thermite_cheat]",
                "f"
            ),
            AxiomReport::Nonstandard("thermite_cheat".to_string())
        );
        // The sorry axiom is also outside the allowlist (caught here too).
        assert_eq!(
            nonstandard_axiom("'thermite_obligation_t' depends on axioms: [sorryAx]", "t"),
            AxiomReport::Nonstandard("sorryAx".to_string())
        );
        // #249 marker mask: an author's own earlier `#print axioms clean_helper` (clean)
        // must not mask the obligation theorem's smuggled axiom. The parser anchors on the
        // obligation theorem's report, so the clean helper line is ignored and the
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
        // No report line names the obligation theorem → Missing (never fall through to a
        // foreign theorem's clean report).
        assert_eq!(
            nonstandard_axiom("'clean_helper' depends on axioms: [propext]", "f"),
            AxiomReport::Missing,
            "a missing obligation-theorem anchor is a hard reject, never Clean"
        );
    }

    // REQ-6 statement binding (the #248 fix): the canonical-statement extractor lifts
    // the `theorem thermite_obligation_<item> … :=` span (binders + proposition, up to
    // the proof term), and `statements_match` is whitespace-insensitive but
    // proposition-strict. A record-update `specs := R_item` inside the proposition does
    // not prematurely end the statement (the `:= by` anchor). Pure-function regression.
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
        // The record-update `:=` did not truncate the statement (the `:= by` anchor).
        assert!(
            extracted.contains("specs := R_item") && extracted.trim_end().ends_with(":="),
            "the statement spans through the proof `:=`, past the record-update `:=`: \
             {extracted}"
        );
        // A reformatted (re-wrapped) same statement matches (whitespace-insensitive).
        let reformatted = "theorem thermite_obligation_f (v : Thermite.Env) (r : Int) : \
             Thermite.stabilizes body { v with specs := R_item } r -> True :=";
        assert!(statements_match(&extracted, reformatted));
        // A different proposition does not match.
        let different = "theorem thermite_obligation_f : True :=";
        assert!(!statements_match(&extracted, different));
    }

    // REQ-7 anchor class (the #268 pin): the obligation-theorem needle is an exact-name
    // match, never a prefix. On a while-shaped multi-theorem file (`_entry` at the top,
    // the bare contract theorem lower down, `_converges` after) a raw prefix
    // `find("theorem thermite_obligation_<item>")` latches the first match (`_entry`) and
    // binds the wrong statement (dropping the 5+2 conjunction; the #249/#250
    // anchor-binds-the-wrong-declaration class). Latent today (while items are auto-tier;
    // `replay_interactive` is unreachable for them, #264) but a soundness seam the
    // moment a multi-theorem file routes to the interactive tier. This pin fails on the
    // pre-#268 prefix needle (verified by temporary revert) and passes on the exact-name
    // anchor. The fns are crate-private, so the critic could not pin this from `tests/`.
    #[test]
    fn interactive_needle_is_exact_name_never_prefix_on_while_shaped_file() {
        // A while-shaped multi-theorem source: `_entry` first (a distinct statement),
        // the bare contract theorem (the 5+2 conjunction) lower down, `_converges` after.
        let entry_stmt = "Exists.intro st (And.intro hb hi)";
        let bare_stmt = "Thermite.whileBodyConverges body { v with specs := R_item } r /\\ \
                         (Thermite.bodyDenote body (stateOf v)).isSome";
        let converges_stmt = "Exists.intro fuel hstf";
        let source = format!(
            "/- preamble -/\n\
             theorem thermite_obligation_w_entry (v : Thermite.Env) :\n  {entry_stmt} := by\n  trivial\n\n\
             theorem thermite_obligation_w (v : Thermite.Env) (r : Int) :\n  {bare_stmt} := by\n  trivial\n\n\
             theorem thermite_obligation_w_converges (v : Thermite.Env) :\n  {converges_stmt} := by\n  trivial\n"
        );

        // `canonical_theorem_statement` must bind the bare contract theorem, not `_entry`.
        let extracted = canonical_theorem_statement(&source, "w").unwrap_or_default();
        assert!(
            statements_match(
                &extracted,
                &format!(
                    "theorem thermite_obligation_w (v : Thermite.Env) (r : Int) : {bare_stmt} :="
                )
            ),
            "the needle must bind the BARE `thermite_obligation_w` (the 5+2 conjunction), \
             not the `_entry` sibling; got: {extracted}"
        );
        // It must not have latched the `_entry` sibling's (distinct) statement.
        assert!(
            !extracted.contains(entry_stmt),
            "the prefix-latch bug binds `_entry`; the exact-name anchor must not: {extracted}"
        );
        assert!(
            !extracted.contains(converges_stmt),
            "the anchor must not over-run past the bare theorem into `_converges`: {extracted}"
        );

        // The low-level anchor: the offset must point at the bare `theorem` header, past
        // the `_entry` declaration (so the reconstruction's preamble split is correct too).
        let anchor = theorem_anchor_pos(&source, "thermite_obligation_w").unwrap_or_default();
        let entry_pos = source
            .find("theorem thermite_obligation_w_entry")
            .unwrap_or_default();
        assert!(
            anchor > entry_pos,
            "the exact-name anchor ({anchor}) must skip the `_entry` header ({entry_pos})"
        );
        assert_eq!(
            source.get(anchor..anchor + "theorem thermite_obligation_w ".len()),
            Some("theorem thermite_obligation_w "),
            "the anchor lands on the bare-name header with a token boundary after the name"
        );
    }

    // REQ-9/AC-7 (the #248 fix): the Lean-path tally floor gate. A `1/1` ratio meets the
    // default floor; a `0/0` (all-untested with mutants generated) or a below-floor
    // ratio does not (the shipped 0/0 backstop + the WeakContract mirror).
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

    // REQ-2(c)/(d): the Lean engine fills its four slots: the smaller trust profile
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

    // #246 / REQ-7(ii) — staleness: two same-named items with different `ens` must
    // produce different evidence keys (the obligation content is hashed, so a contract
    // edit can never silently reuse a cached `Proven`). Hand-derived (R-CHAR-3): the
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

    // #246 — targeted-spine staleness: an edit anywhere under `lean/Thermite/**`
    // (including a nested subdirectory, the recursive widening) must change the
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

        // Edit the nested spine file (the case the non-recursive walk missed).
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

    // A synthetic always-`Proven` verdict (the disagreement guard, REQ-5). Built
    // directly (no engine needed for the pure `check_disagreement` guard).
    fn stub_proven() -> Verdict {
        Verdict::Proven(Evidence {
            verified: 1,
            key: a_key(),
        })
    }

    // A synthetic witnessed-`Refuted` verdict (the other half of the guard, REQ-5).
    fn stub_refuted() -> Verdict {
        Verdict::Refuted(Counterexample {
            obligations: vec![ObligationResult::failed(
                "postcondition not satisfied",
                Some("f.th:3:5".to_string()),
                Some("error: postcondition not satisfied".to_string()),
            )],
        })
    }

    // A synthetic always-`Proven` engine (for `attribution_for`, REQ-4). A test double.
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
        fn discharge(&self, _o: &Obligation, _covenant: &CovenantRecord) -> Verdict {
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

    // REQ-5 — the disagreement halt: Proven ⊕ witnessed-Refuted on the same
    // obligation fires the alarm, naming both engines + the item. Expected from REQ-5
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
        // The order does not matter: Refuted ⊕ Proven also halts, naming the right
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

    // REQ-5 — Proven ⊕ Unknown is benign (the Unknown engine could not decide;
    // per REQ-3.1 a witness-less Verus failure is Unknown, so it can never spuriously
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
        // Refuted ⊕ Refuted is agreement on a bug (both witnessed), benign for the
        // alarm (the hard fail stands on its own; no soundness contradiction).
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

    // REQ-4 — attribution: the `{engine, trust_profile}` pair is the engine's name tag
    // + its enumerated trust items. Expected from REQ-4 (R-CHAR-3): the Lean profile is
    // smaller along the named axes (no Z3, no Verus VC-gen).
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

    // REQ-7 — sorry detection: a `sorry` token in the source or a `sorryAx` in the
    // `#print axioms` output is detected; a clean proof with only the standard axioms
    // is not. Expected from REQ-7(ii) (R-CHAR-3): sorry never Proven.
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
        // A clean proof (no sorry token, only standard axioms) is not a sorry.
        assert!(
            !proof_has_sorry(
                "theorem t : True := by trivial",
                "'t' depends on axioms: [propext, Classical.choice, Quot.sound]"
            ),
            "a clean standard-axiom proof is NOT a sorry"
        );
        // A substring like `sorryless` does not false-positive (whole-word match).
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

    // REQ-9 — the Lean-path kill semantics: an admitted mutant Lean does not prove
    // (Refuted ∪ Unknown-after-attempt) is killed; an admitted Proven mutant survived;
    // a non-admitted mutant is UntestedAgainstLean (never killed). Expected from REQ-9
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
        // Not admitted → UntestedAgainstLean regardless of the verdict (never a kill).
        assert_eq!(
            lean_mutant_outcome(false, &unknown),
            LeanMutantOutcome::UntestedAgainstLean
        );
        assert_eq!(
            lean_mutant_outcome(false, &proven),
            LeanMutantOutcome::UntestedAgainstLean
        );
    }

    // REQ-9 — the tally: untested mutants are outside the denominator (never inflate
    // the ratio); the #101-equivalent survivors are dropped from both the survivor set
    // and the denominator. Expected from REQ-9 + §7 (R-CHAR-3).
    #[test]
    fn lean_mutation_tally_does_not_inflate_on_untested() {
        let mut t = LeanMutationTally::default();
        t.record(LeanMutantOutcome::Killed, false); // 1 killed, +1 denom
        t.record(LeanMutantOutcome::Killed, false); // 2 killed, +1 denom
        t.record(LeanMutantOutcome::UntestedAgainstLean, false); // outside the ratio
        t.record(LeanMutantOutcome::Survived, true); // proven-equivalent → excluded both
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
        // 2/3 ≈ 0.667; the untested mutant did not inflate it to 2/2 = 1.0.
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

    // REQ-7 (increment 2e): the forge-tier `lemma` discharge — `export_lemma` emits a
    // self-contained theorem (the pure `∀ params, req → ens` proposition over the
    // denotation spine) proved by the author's frozen-battery tactics; `discharge_source`
    // runs lake + the certify-time axiom gate. A clean merge-flavored arithmetic lemma
    // kernel-accepts and PROVES (needs the built Lean spine — skipped without it, like the
    // other `live_*` engine tests; the CI lean job is authoritative).
    #[test]
    fn live_forge_lemma_discharges_proven() {
        if !lake_present() {
            eprintln!("SKIP: lake not present — live forge-lemma discharge test not run.");
            return;
        }
        let src = "lemma merge_advance(i: u64, n: u64) req i < n ens i + 1 <= n \
                   proof { simp [Thermite.denote, Thermite.Env.bindInt, Thermite.intVal, \
                   Thermite.arithDenote]; omega }";
        let p = parse_program(src);
        let l = p
            .items
            .iter()
            .find_map(|i| match i {
                thermite_syntax::Item::Forge(thermite_syntax::ForgeItem::Lemma(l)) => Some(l),
                _ => None,
            })
            .expect("a lemma");
        let exported = crate::lean_export::export_lemma(l, &[], &p).expect("export the lemma");
        // The emitted theorem is the pure proposition over the spine (no body/result).
        assert!(
            exported
                .source
                .contains("theorem thermite_obligation_merge_advance (v : Thermite.Env)"),
            "the canonical theorem name anchors the axiom probe: {}",
            exported.source
        );
        assert!(
            !exported.source.contains("bindInt \"result\""),
            "a lemma has no `result` binding (unlike a fn-contract theorem): {}",
            exported.source
        );
        let engine = LeanEngine::new(p.clone(), lean_root());
        match engine.discharge_source(&exported.source, "merge_advance") {
            Verdict::Proven(_) => {}
            other => panic!("the merge lemma must discharge Proven against the spine: {other:?}"),
        }
    }
}
