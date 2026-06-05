//! `forge/src/manifest.rs` — the certificate schema (`thermite-design.md` §5.1,
//! Appendix A). The `Certificate` is the deliverable's trust statement (§6): a
//! STABLE, versioned data contract that `forge check` emits. This module owns the
//! schema and its `serde_json` (de)serialization; it performs NO I/O and runs NO
//! verification — `check.rs` (`.design/forge/check.md`) produces the values.
//!
//! Governing design: `.design/forge/certificate-manifest.md`.
//!
//! The schema is fixed NOW at its full Appendix A shape; the PRODUCERS arrive
//! over several issues (the "two-speed schema"). #5 fills `item`, `level`,
//! `effects`, `slag`, and `obligations` with real derived values; the
//! `contract_quality.*` battery fields are FORWARD-DECLARED (honest #5 values,
//! NOT asserted against the golden cert, made live by #6/#12/#13) and
//! `suggested_move` is a reserved `None`. `solver_time_ms` is present but
//! non-deterministic and excluded from the cert-oracle comparison.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (stable schema, Appendix A) | SHIPPED | `struct Certificate { item, level, solver_time_ms, contract_quality, effects, slag, obligations, suggested_move }` mirrors Appendix A field order; consumed by `check::check_file` in `check.rs`. |
//! | REQ-2 (fields #5 produces now) | SHIPPED | `Certificate::new` sets `item`/`level`/`effects`/`slag`/`obligations` from real pipeline data; `effects_of` maps `EffectRow` to the `["pure"]` row; called by `check::assemble_certificate`. |
//! | REQ-3 (forward-declared fields) | SHIPPED | `ContractQuality::forward_declared` returns honest non-asserted #5 values (`tautology=false`, `vacuous_precondition=false`, `mutants_killed="0/0"`, `survivor=None`); `oracle_subset` excludes them. |
//! | REQ-4 (`suggested_move` reserved) | SHIPPED | `Certificate::new` sets `suggested_move: None`; `SuggestedMove` is the reserved (currently un-constructed in production) slot type, serialized as `null`/omitted. |
//! | REQ-5 (per-obligation results) | SHIPPED | `struct ObligationResult { name, status, location, diagnostic }` + `enum ObligationStatus`; the `obligations` field; consumed by `check::assemble_certificate` + `cli::render_human`. |
//! | REQ-6 (`solver_time_ms` excluded) | SHIPPED | `solver_time_ms: u64` present (Appendix A); `Certificate::oracle_subset` omits it (and `contract_quality`), and `cli::render_human` labels it non-deterministic. |
//! | REQ-7 (serde_json serialization) | SHIPPED | `#[derive(Serialize, Deserialize)]`; `Level` serializes to `"L0".."L3"`; `cli::run_check` serializes via `serde_json::to_string_pretty`; deterministic field order from struct declaration order. |

use serde::{Deserialize, Serialize};
use thermite_syntax::{Effect, EffectRow};

/// The assurance level (`thermite-design.md` §6). Serializes to the string form
/// `"L0".."L3"` to match the golden cert's `"level": "L3"` (REQ-1, REQ-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Level {
    /// L0 — unverified / `#[slag]` escape hatch (§6, §8).
    L0,
    /// L1 — executable runtime check compiled in (§6).
    L1,
    /// L2 — bounded model check (Kani; issue #9) (§6).
    L2,
    /// L3 — SMT proof: the contract holds for all inputs (§6).
    L3,
}

/// The status of a single proof obligation (REQ-5). v0.1 records discharged or
/// failed; the failure carries a source-located diagnostic (the §5.1
/// "counterexamples, not adjectives" payload), never a bare boolean.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationStatus {
    /// The obligation was discharged by the solver.
    Discharged,
    /// The obligation failed; see the `diagnostic` and `location` on the result.
    Failed,
}

/// One per-obligation verification result (REQ-5, `.design/forge/check.md`
/// REQ-4). For a clean proof, `check.rs` records the verified item(s) as
/// `Discharged`; for a failure it records the failed obligation with verus's
/// `error: <clause>` description and its `--> file:line:col` source span — the
/// §5.1 "counterexample, not adjective".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationResult {
    /// The obligation identity (the verus function name for a discharged item,
    /// or the failed-clause description for a failure).
    pub name: String,
    /// Discharged or failed.
    pub status: ObligationStatus,
    /// `file:line:col` source span of the obligation, when verus reports one.
    /// `None` for a summary-only discharged result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// The concrete failure diagnostic from verus's stderr (`error: <clause>`),
    /// present only on a failure. Never a bare "verification failed".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

impl ObligationResult {
    /// A discharged obligation (a verified verus function), summary-only.
    pub fn discharged(name: impl Into<String>) -> Self {
        ObligationResult {
            name: name.into(),
            status: ObligationStatus::Discharged,
            location: None,
            diagnostic: None,
        }
    }

    /// A failed obligation carrying its source location + diagnostic witness
    /// (§5.1 "counterexamples, not adjectives").
    pub fn failed(
        name: impl Into<String>,
        location: Option<String>,
        diagnostic: Option<String>,
    ) -> Self {
        ObligationResult {
            name: name.into(),
            status: ObligationStatus::Failed,
            location,
            diagnostic,
        }
    }
}

/// The contract-quality block (`thermite-design.md` §7, Appendix A) — REQ-3.
/// FORWARD-DECLARED in #5: the vacuity battery (`tautology`/
/// `vacuous_precondition`, #6/#13) and the mutation scorer
/// (`mutants_killed`/`survivor`, #12) are not yet built, so these carry honest
/// non-asserted values and are EXCLUDED from the cert-oracle comparison
/// (`Certificate::oracle_subset`). The schema reserves the slot; the value is
/// filled by its producer, never fabricated here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractQuality {
    /// Is the contract a tautology? (issue #6/#13) — `false` placeholder in #5.
    pub tautology: bool,
    /// Is the precondition vacuous? (issue #6/#13) — `false` placeholder in #5.
    pub vacuous_precondition: bool,
    /// Mutation kill ratio `"killed/total"` (issue #12) — `"0/0"` (unscored) in
    /// #5; typed `String` to match the Appendix A `"17/18"` shape (OQ-1).
    pub mutants_killed: String,
    /// The surviving-mutant description (issue #12) — `None` (unscored) in #5.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub survivor: Option<String>,
}

impl ContractQuality {
    /// The honest #5 value: the battery has not run, so nothing is asserted. NOT
    /// a fabricated pass — `mutants_killed` is the unscored `"0/0"`, not the
    /// golden `"17/18"` (REQ-3; `conformance/README.md` forward-declaration).
    pub fn forward_declared() -> Self {
        ContractQuality {
            tautology: false,
            vacuous_precondition: false,
            mutants_killed: "0/0".to_string(),
            survivor: None,
        }
    }
}

/// A reserved `suggested_move` heuristic hint (`thermite-design.md` §5.1) —
/// REQ-4. The slot exists so populating it later (missing-invariant patterns,
/// overflow-guard templates, trigger hints) is not a breaking schema change. In
/// #5 the `Certificate`'s `suggested_move` is always `None` (a reserved honest
/// absence: not a placeholder string and not an unimplemented stub).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestedMove {
    /// A short kind tag for the heuristic (e.g. `"missing-invariant"`).
    pub kind: String,
    /// The suggested edit text.
    pub detail: String,
}

/// The certificate `forge check` emits for one item (`thermite-design.md` §5.1,
/// Appendix A). Field declaration order is the deterministic serialization order
/// (REQ-7) and mirrors Appendix A: `item`, `level`, `solver_time_ms`,
/// `contract_quality`, `effects`, `slag`; the #5 additive schema surface
/// (`obligations` — REQ-5; `suggested_move` — REQ-4) follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Certificate {
    /// The checked item's name.
    pub item: String,
    /// The assurance level (REQ-2: L3 iff verus reports 0 errors).
    pub level: Level,
    /// Wall-clock solver time in ms — NON-DETERMINISTIC, excluded from the
    /// oracle comparison (REQ-6; `conformance/README.md`). `#[serde(default)]`
    /// so the golden deterministic-subset cert (which OMITS this non-det field)
    /// still deserializes into a full `Certificate` (certificate-manifest.md
    /// AC-2 — the schema is a faithful superset of the golden subset).
    #[serde(default)]
    pub solver_time_ms: u64,
    /// The contract-quality battery block — FORWARD-DECLARED in #5 (REQ-3).
    pub contract_quality: ContractQuality,
    /// The item's effect row (REQ-2: `["pure"]` for the corpus).
    pub effects: Vec<String>,
    /// Whether the item is `#[slag]` — always `false` in #5 (slag is #6/§8).
    pub slag: bool,
    /// Per-obligation results parsed from verus (REQ-5; #5 additive field).
    /// `#[serde(default)]` so a golden cert that does not enumerate the
    /// per-obligation array (the golden asserts only the item-level summary,
    /// certificate-manifest.md OQ-2) deserializes into a `Certificate`.
    #[serde(default)]
    pub obligations: Vec<ObligationResult>,
    /// Reserved heuristic-hint slot — `None` in #5 (REQ-4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_move: Option<SuggestedMove>,
}

impl Certificate {
    /// Assemble a #5 certificate from the real pipeline data (REQ-2). `check.rs`
    /// derives `level`/`obligations` from verus and `effects` from the item's
    /// `fx` row; the forward-declared and reserved fields take their honest #5
    /// values here.
    pub fn new(
        item: impl Into<String>,
        level: Level,
        effects: Vec<String>,
        solver_time_ms: u64,
        obligations: Vec<ObligationResult>,
    ) -> Self {
        Certificate {
            item: item.into(),
            level,
            solver_time_ms,
            contract_quality: ContractQuality::forward_declared(),
            effects,
            slag: false,
            obligations,
            suggested_move: None,
        }
    }

    /// The DETERMINISTIC, currently-producible oracle subset (REQ-3/REQ-6,
    /// `.design/forge/check.md` AC-1): `(item, level, effects, slag)`. The
    /// forward-declared `contract_quality.*` and the non-deterministic
    /// `solver_time_ms` are STRUCTURALLY excluded by being absent from this
    /// tuple. The cert-oracle (`tests/check_conformance.rs`) and the human
    /// renderer (`cli::render_human`, which prints exactly this subset plus the
    /// excluded `solver_time_ms` labelled as such) treat these four as the
    /// oracle-stable fields in #5.
    pub fn oracle_subset(&self) -> (&str, Level, &[String], bool) {
        (&self.item, self.level, &self.effects, self.slag)
    }
}

/// Map a parsed `EffectRow` to the certificate's `effects` string vector
/// (REQ-2). `Pure` → `["pure"]`; a non-pure row maps each `Effect` to its
/// canonical lowercase token in declaration order (deterministic, R-CODE-5).
/// Covers EVERY `Effect` variant (the whole closed enum), not just the corpus's
/// `pure`.
pub fn effects_of(fx: &EffectRow) -> Vec<String> {
    match fx {
        EffectRow::Pure => vec!["pure".to_string()],
        EffectRow::Set(effects) => effects.iter().map(effect_token).collect(),
    }
}

/// The canonical lowercase token for one `Effect` (e.g. `read(x)`, `alloc`).
fn effect_token(effect: &Effect) -> String {
    match effect {
        Effect::Read(name) => format!("read({name})"),
        Effect::Write(name) => format!("write({name})"),
        Effect::Net(name) => format!("net({name})"),
        Effect::Alloc => "alloc".to_string(),
        Effect::Time => "time".to_string(),
        Effect::Rand => "rand".to_string(),
        Effect::Panic => "panic".to_string(),
        Effect::Diverge => "diverge".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test-local serializer (mirrors `cli::run_check`'s
    /// `serde_json::to_string_pretty`).
    fn serialize(cert: &Certificate) -> String {
        serde_json::to_string_pretty(cert).expect("serialize cert")
    }

    /// The deterministic-subset equality the cert-oracle uses, expressed via the
    /// production `oracle_subset` accessor (so the test exercises the real schema
    /// property, not a re-implementation).
    fn oracle_eq(a: &Certificate, b: &Certificate) -> bool {
        a.oracle_subset() == b.oracle_subset()
    }

    // AC-1: schema matches Appendix A — every documented key present, Level::L3
    // serializes to "L3". Expected keys/values trace to `thermite-design.md`
    // Appendix A (R-CHAR-3), not to forge's own output.
    #[test]
    fn schema_matches_appendix_a() {
        let cert = Certificate::new(
            "sum",
            Level::L3,
            vec!["pure".to_string()],
            612,
            vec![ObligationResult::discharged("sum_check::sum")],
        );
        let json = serialize(&cert);
        let value: serde_json::Value = serde_json::from_str(&json).expect("parse");
        // Appendix A keys.
        for key in [
            "item",
            "level",
            "solver_time_ms",
            "contract_quality",
            "effects",
            "slag",
        ] {
            assert!(value.get(key).is_some(), "missing Appendix A key `{key}`");
        }
        // contract_quality sub-keys (Appendix A).
        let cq = value.get("contract_quality").expect("contract_quality");
        for key in ["tautology", "vacuous_precondition", "mutants_killed"] {
            assert!(cq.get(key).is_some(), "missing contract_quality.{key}");
        }
        // Level::L3 serializes to the string "L3".
        assert_eq!(value.get("level").and_then(|v| v.as_str()), Some("L3"));
    }

    // AC-2: the golden cert's deterministic subset deserializes into a
    // Certificate and re-serializes equal on those fields. Anchors to the GOLDEN
    // `conformance/sum.cert.json` (R-CHAR-3), not forge's output.
    #[test]
    fn golden_deterministic_subset_round_trips() {
        let golden_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("conformance")
            .join("sum.cert.json");
        let golden_src = std::fs::read_to_string(&golden_path).expect("read golden cert");
        let golden: Certificate = serde_json::from_str(&golden_src).expect("deserialize golden");
        assert_eq!(golden.item, "sum");
        assert_eq!(golden.level, Level::L3);
        assert_eq!(golden.effects, vec!["pure".to_string()]);
        assert!(!golden.slag);
        // A freshly assembled #5 cert with the same deterministic fields is
        // oracle-equal to the golden, despite differing battery / time fields.
        let ours = Certificate::new(
            "sum",
            Level::L3,
            vec!["pure".to_string()],
            42,
            vec![ObligationResult::discharged("sum_check::sum")],
        );
        assert!(
            oracle_eq(&golden, &ours),
            "the golden subset must oracle-match a #5 cert"
        );
    }

    // AC-3: forward-declared fields excluded from the live oracle — two certs
    // differing ONLY in contract_quality / solver_time_ms compare equal.
    #[test]
    fn oracle_ignores_forward_declared_and_time() {
        let mut a = Certificate::new("f", Level::L3, vec!["pure".to_string()], 1, vec![]);
        let mut b = a.clone();
        b.solver_time_ms = 99_999;
        b.contract_quality.mutants_killed = "17/18".to_string();
        b.contract_quality.tautology = true;
        assert!(
            oracle_eq(&a, &b),
            "oracle must ignore time + battery fields"
        );
        // But a differing deterministic field IS caught.
        a.level = Level::L1;
        assert!(!oracle_eq(&a, &b), "oracle must catch a level mismatch");
    }

    // AC-4: suggested_move is a reserved absence — serializes as omitted (its
    // Option is None), never a placeholder.
    #[test]
    fn suggested_move_is_reserved_absence() {
        let cert = Certificate::new("f", Level::L3, vec!["pure".to_string()], 0, vec![]);
        assert!(cert.suggested_move.is_none());
        let json = serialize(&cert);
        assert!(
            !json.contains("suggested_move"),
            "None suggested_move must be omitted, not a placeholder:\n{json}"
        );
    }

    // AC-5: per-obligation list present for pass and fail; a failure carries a
    // source-located diagnostic.
    #[test]
    fn obligation_results_present() {
        let pass = ObligationResult::discharged("sum_check::sum");
        assert_eq!(pass.status, ObligationStatus::Discharged);
        let fail = ObligationResult::failed(
            "postcondition not satisfied",
            Some("broken_check.rs:5:13".to_string()),
            Some("error: postcondition not satisfied".to_string()),
        );
        assert_eq!(fail.status, ObligationStatus::Failed);
        assert!(fail.location.is_some(), "failure carries a source location");
        assert!(fail.diagnostic.is_some(), "failure carries a diagnostic");
    }

    // AC-6: determinism — serializing the same Certificate twice is
    // byte-identical (R-CODE-5).
    #[test]
    fn serialization_is_deterministic() {
        let cert = Certificate::new(
            "sum",
            Level::L3,
            vec!["pure".to_string()],
            612,
            vec![ObligationResult::discharged("sum_check::sum")],
        );
        let a = serialize(&cert);
        let b = serialize(&cert);
        assert_eq!(a, b);
    }

    // effects_of covers the whole Effect enum, not just `pure` (R-DEFER-8: fix
    // the whole class). Expected tokens are this module's documented mapping.
    #[test]
    fn effects_of_covers_every_variant() {
        assert_eq!(effects_of(&EffectRow::Pure), vec!["pure".to_string()]);
        let row = EffectRow::Set(vec![
            Effect::Read("x".to_string()),
            Effect::Write("y".to_string()),
            Effect::Net("z".to_string()),
            Effect::Alloc,
            Effect::Time,
            Effect::Rand,
            Effect::Panic,
            Effect::Diverge,
        ]);
        assert_eq!(
            effects_of(&row),
            vec!["read(x)", "write(y)", "net(z)", "alloc", "time", "rand", "panic", "diverge"]
        );
    }
}
