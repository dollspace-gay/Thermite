//! `forge/src/obligation.rs` — the backend-NEUTRAL verification obligation
//! artifact (`.design/verified/proof-backends.md` REQ-1/REQ-1.2; increment (i),
//! blocker #204).
//!
//! Today the obligation content the pipeline discharges exists only transiently,
//! materialized as Verus TEXT (`thermite-tv/src/obligation.rs`'s
//! `equivalence_obligation` family). This module REIFIES that same content as a
//! prover-NEUTRAL value (see the "honest scope" note below on serialization): an
//! [`Obligation`] carries the item, its
//! obligation [`ObligationClass`], the [`ObligationRole`], the AST slice it is
//! stated over (a `thermite-syntax` node, NOT a Verus string), and a prover-neutral
//! [`ObligationEnv`] (the generalization of `thermite-tv`'s `ObligationFrame` —
//! AST nodes + Thermite types + coercion flags, NOT rendered Verus text). An
//! engine (`engine.rs`) RENDERS it into its own input language (Verus text for the
//! Verus engine; Lean source for the future Lean engine — increment (ii)).
//!
//! The load-bearing inversion (§1): the obligation STOPS being Verus-shaped. The
//! Verus rendering (`obligation.rs`'s `param_list`/`spec_defs`/`as nat` rewrite)
//! becomes the Verus engine's `render`, not the artifact itself.
//!
//! **A note on "serializable" (REQ-1, honest scope).** The load-bearing property
//! increment (i) delivers is prover-NEUTRALITY: the artifact carries `thermite-
//! syntax` AST nodes + Thermite `Type`s + coercion flags, NOT any prover's rendered
//! text. The `thermite-syntax` AST does NOT derive `serde` in production (serde is a
//! dev-dependency there — `thermite-syntax/Cargo.toml`), and adding it is outside
//! the #204 manifest, so the artifact is a prover-neutral in-memory VALUE
//! (`Clone + PartialEq + Eq`), not a wire-serialized one. The Verus engine consumes
//! the `&Obligation` directly and keys its evidence on the LOWERED source (the
//! SHIPPED `cache::cache_key` substrate — §2(d)); wire serialization of the artifact
//! is increment (ii) work, when the Lean exporter serializes a Lean theorem (a
//! string), not the raw AST. The env's prover-neutral SCALAR content is what the
//! engine renders; that content is fully inspectable + comparable here.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (the backend-neutral Obligation artifact) | SHIPPED | `pub struct Obligation { item, class, role, ast_slice, env }` + `pub enum ObligationClass`/`ObligationRole` + `pub struct ObligationEnv`/`ObligationParam` here, prover-neutral (the `ast_slice` is an `AstSlice` of `thermite-syntax` nodes; the env carries Thermite `Type`s + coercion flags, NO Verus strings). Non-test consumer: `engine::VerusEngine::discharge`/`evidence_key` (`engine.rs`) consume an `&Obligation`, and `check::obligation_for_item` mints one per checked item on the live L3 path. |
//! | REQ-1.2 (the REGISTRY-TERMINATION class + the full-expression-position closure) | SHIPPED (class assignment + corrected closure) | `ObligationClass::RegistryTermination` is minted for an item whose called-spec-fn set (the corrected `req ∪ ens ∪ body ∪ dec(item)` seed, closure-step over each reached spec-fn's `body ∪ dec` — `check::reachable_spec_fn_names_full`) is non-empty; the Lean-path well-foundedness DISCHARGE is increment (ii), NOT-STARTED. The corrected closure walks the dec measures (was body-only). |
//!
//! The auxiliary OVERFLOW / TERMINATION classes and the multi-class minting of one
//! item's full obligation SET are part of the per-class rendering increment (ii)
//! still grows; increment (i) reifies the class enum (AC-1) + mints the CONTRACT +
//! REGISTRY-TERMINATION classes the §0 pipeline observably discharges today, which
//! is what the Verus engine's `discharge` keys on.

use thermite_syntax::{Block, Expr, FnItem, SpecFnItem, Type};

/// The backend-neutral obligation class (`.design/verified/proof-backends.md`
/// REQ-1 / AC-1). The variant set is the UNION of the `thermite-tv/src/obligation.rs`
/// emitters (CONTRACT / EXEC / BODY / LOOP-{entry,preservation,exit}), the §6/§7
/// in-item auxiliaries Verus discharges (OVERFLOW / TERMINATION), and REQ-1.2's
/// [`ObligationClass::RegistryTermination`]. The three §0.1 meta/battery query
/// classes (vacuity / equivalence / strengthen) are deliberately NOT here — they
/// stay direct verus invocations OUTSIDE the Engine interface in v1 (the role
/// discriminator carries that future seam; see [`ObligationRole`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// AC-1 requires the class enum's variants to be the FULL union of the
// `obligation.rs` emitters + the §6/§7 in-item auxiliaries + REGISTRY-TERMINATION.
// Increment (i) mints CONTRACT + REGISTRY-TERMINATION on the live path; the EXEC /
// BODY / LOOP-* / OVERFLOW / TERMINATION variants are forward-declared per AC-1 and
// minted as the per-class rendering grows (increment (ii)+). Each carries a stable
// `tag()`. The forward-declared variants are not yet CONSTRUCTED in production.
#[allow(
    dead_code,
    reason = "proof-backends AC-1: the obligation-class enum is the FULL union of \
              classes by design; EXEC/BODY/LOOP-*/OVERFLOW/TERMINATION are forward-\
              declared and minted as the per-class rendering grows (increment (ii)+)"
)]
pub enum ObligationClass {
    /// A contract clause `∀ inputs, ⟦req⟧ → ⟦ens[result := body]⟧` — the
    /// `equivalence_obligation` content (the canonical `result == spec_sum(xs)`
    /// shape). This is the class the Verus engine's per-item L3 discharge proves.
    Contract,
    /// An EXEC value obligation (`exec_equivalence_obligation`): the production
    /// exec expr's bounded value equals the reference exec value.
    Exec,
    /// A straight-line BODY state obligation (`body_equivalence_obligation`).
    Body,
    /// A loop ENTRY obligation (`loop_entry_obligation`): `inv` holds on entry.
    LoopEntry,
    /// A loop PRESERVATION obligation (`loop_preservation_obligation`): one
    /// iteration preserves `inv` and steps the state faithfully.
    LoopPreservation,
    /// A loop EXIT obligation (`loop_exit_obligation`): the claimed after-loop
    /// characterization follows from `inv ∧ ¬cond`.
    LoopExit,
    /// An OVERFLOW / bounds obligation (the bounded `S_E`: `execDenote = none`
    /// exactly at overflow) Verus discharges inside an item.
    Overflow,
    /// A TERMINATION obligation: the item's own `dec` measure is a valid
    /// well-founded descent (the source `dec` → the well-founded fixpoint).
    Termination,
    /// The REGISTRY-TERMINATION class (REQ-1.2, the #215 fix): for an item with a
    /// non-empty called-spec-fn set, EVERY reached spec-fn carries a per-spec-fn
    /// obligation that its `dec` measure is a VALID well-founded descent. The
    /// parser guarantees dec PRESENCE; this class is dec VALIDITY, and it is NEVER
    /// assumed. Discharged today by Verus's own dec-check when Verus proves the
    /// item (REQ-1.2(a), the common path); the Lean well-foundedness DISCHARGE is
    /// increment (ii).
    RegistryTermination,
}

impl ObligationClass {
    /// A stable lower-kebab tag for serialization / cache-key domain separation /
    /// diagnostics (deterministic — R-CODE-5).
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            ObligationClass::Contract => "contract",
            ObligationClass::Exec => "exec",
            ObligationClass::Body => "body",
            ObligationClass::LoopEntry => "loop-entry",
            ObligationClass::LoopPreservation => "loop-preservation",
            ObligationClass::LoopExit => "loop-exit",
            ObligationClass::Overflow => "overflow",
            ObligationClass::Termination => "termination",
            ObligationClass::RegistryTermination => "registry-termination",
        }
    }
}

/// The polarity / intent discriminator (`.design/verified/proof-backends.md`
/// REQ-1). Increment (i) mints ONLY [`ObligationRole::Certification`] obligations:
/// the §0.1 meta/battery queries (vacuity / equivalence / strengthen) are NOT
/// reified as `Obligation`s in v1 (they keep their own direct verus calls, OQ-5).
/// The role field IS the seam those inverted/advisory roles will key on, so the
/// REQ-3 discharge discipline (Unknown→degrade, Refuted→hard-fail) can be scoped
/// to `Certification` from day one without dragging the OUT-list in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObligationRole {
    /// An item-correctness CERTIFICATION obligation — REQ-3's discharge discipline
    /// applies (a `Proven` certifies, an `Unknown` degrades, a `Refuted` hard-fails).
    Certification,
}

impl ObligationRole {
    /// A stable tag for serialization / cache-key domain separation.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            ObligationRole::Certification => "certification",
        }
    }
}

/// The parsed AST node(s) the obligation is stated over (`.design/verified/
/// proof-backends.md` §1 `ast_slice: ExprOrBlock`). The SAME `&Expr` / `&Block`
/// the `thermite-tv` obligation functions consume — kept as owned clones so the
/// `Obligation` is a self-contained, serializable artifact (it outlives the
/// borrow of the parsed `Program`). An engine renders THIS into its language; the
/// artifact never carries pre-rendered prover text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AstSlice {
    /// A single expression node (a contract clause, an exec expr, a loop measure).
    /// Forward-declared for the EXEC / LOOP-measure classes (increment (ii)+); the
    /// CONTRACT obligation increment (i) mints uses the `Block` body slice.
    #[allow(
        dead_code,
        reason = "proof-backends §1: the single-Expr slice serves the EXEC/loop-measure \
                  classes the per-class rendering grows in increment (ii)+; increment (i) \
                  mints Block body slices"
    )]
    Expr(Box<Expr>),
    /// A block node (a straight-line body, a loop body).
    Block(Box<Block>),
}

/// One free-var binding in the obligation env, at its THERMITE type — the
/// prover-neutral generalization of `thermite-tv`'s `ParamDecl` (which carries a
/// VERUS `type_str`). The engine renders the `Type` into ITS spelling (`u64` /
/// `Seq<u32>` for Verus; the Lean sort for Lean), so the artifact stays neutral
/// (§1 — "free vars at their THERMITE types, not Verus strings; the engine renders
/// them").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationParam {
    /// The free-var name as it appears in the AST slice.
    pub name: String,
    /// The Thermite type (NOT a Verus string) — the engine renders it.
    pub ty: Type,
}

/// The typing / env context — the prover-neutral generalization of
/// `thermite-tv`'s `ObligationFrame` (`.design/verified/proof-backends.md` §1).
/// Carries the PRE-rendering content (AST nodes + Thermite types + coercion flags)
/// so an engine renders it into its language. NO Verus strings live here (the
/// `spec_defs` are spec-fn NAMES resolved against the shared frozen registry, not
/// verbatim Verus `verus_l3` text; the coercion flags are named param sets the
/// engine maps to its own view — Verus's `@`-view / `as nat`, Lean's `Seq`/`toNat`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObligationEnv {
    /// The free vars at their Thermite types (the clause params + `result` +
    /// `old(_)` values), in signature order.
    pub params: Vec<ObligationParam>,
    /// The enclosing precondition as an AST node (NOT rendered text), if any.
    pub req: Option<Box<Expr>>,
    /// The in-scope spec-fn definition NAMES the obligation depends on, resolved
    /// against the shared frozen registry. Per REQ-1.2 / §4 EXP each of these
    /// names is in the FULL-expression-position called-spec-fn closure
    /// (`req ∪ ens ∪ body ∪ dec`, transitively, closure-step over `body ∪ dec`),
    /// and each carries a `RegistryTermination` obligation. The engine resolves a
    /// name to its real-bodied definition (the Verus engine emits its `verus_l3`
    /// def; the Lean engine populates `R_item` with the encoded body).
    pub spec_defs: Vec<String>,
    /// Coercion-frame: params bound directly as a `Seq<_>` view (the engine renders
    /// the `@`-view / `Seq` view from this flag).
    pub seq_params: Vec<String>,
    /// Coercion-frame: params coerced `as nat` / `toNat`.
    pub nat_coerce_params: Vec<String>,
    /// Coercion-frame: `String` params.
    pub string_params: Vec<String>,
    /// Coercion-frame: `Map` params.
    pub map_params: Vec<String>,
}

/// The backend-NEUTRAL verification obligation (`.design/verified/proof-backends.md`
/// REQ-1). A serializable artifact stated against the mechanized semantics `S`,
/// independent of any prover's input language. An [`crate::engine::Engine`]
/// renders + discharges it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obligation {
    /// The fn / spec-fn the obligation belongs to (§5.3 per-item isolation).
    pub item: String,
    /// The obligation class (AC-1).
    pub class: ObligationClass,
    /// The polarity / intent discriminator (REQ-3 keys its discipline on
    /// `Certification`).
    pub role: ObligationRole,
    /// The parsed AST node(s) the obligation is stated over (the `source: &Expr` /
    /// `body: &Block` the `thermite-tv` obligation functions consume).
    pub ast_slice: AstSlice,
    /// The prover-neutral typing / env context.
    pub env: ObligationEnv,
}

impl Obligation {
    /// Mint the per-item CONTRACT certification obligation for a checked exec
    /// `fn` (`.design/verified/proof-backends.md` §1 — "the (T1)-style equality the
    /// spine already proves the reference encoder satisfies, lifted to the per-item
    /// obligation"). The `ast_slice` is the fn's body (the production side the
    /// `ens[result := body]` shape characterizes); `env` carries the params at
    /// their Thermite types, the `req` clause expr, and the in-scope spec-fn names
    /// (`called_spec_fns`, the full-expression-position closure, REQ-1.2/#226). A
    /// boundary fn (`body == None`) has no in-language body obligation — the env's
    /// `req` and params still characterize the contract, and the body slice falls
    /// back to an empty block (the engine renders the boundary signature, never an
    /// in-language body).
    #[must_use]
    pub fn contract_for_fn(f: &FnItem, called_spec_fns: Vec<String>) -> Obligation {
        let params = f
            .params
            .iter()
            .map(|p| ObligationParam {
                name: p.name.clone(),
                ty: p.ty.clone(),
            })
            .collect();
        let ast_slice = match &f.body {
            Some(b) => AstSlice::Block(Box::new(b.clone())),
            None => AstSlice::Block(Box::new(Block {
                stmts: Vec::new(),
                tail: None,
            })),
        };
        Obligation {
            item: f.name.clone(),
            class: ObligationClass::Contract,
            role: ObligationRole::Certification,
            ast_slice,
            env: ObligationEnv {
                params,
                req: Some(Box::new(f.contract.req.expr.clone())),
                spec_defs: called_spec_fns,
                ..ObligationEnv::default()
            },
        }
    }

    /// Mint the per-item CONTRACT certification obligation for a checked `spec fn`
    /// (`.design/verified/proof-backends.md` §1). A spec fn is a pure
    /// contract-free definition; its certification obligation is stated over its
    /// `body` with its params in env. The `called_spec_fns` closure (REQ-1.2/#226,
    /// seeded by the spec fn's own `body ∪ dec`) names every reached spec-fn.
    #[must_use]
    pub fn contract_for_spec_fn(s: &SpecFnItem, called_spec_fns: Vec<String>) -> Obligation {
        let params = s
            .params
            .iter()
            .map(|p| ObligationParam {
                name: p.name.clone(),
                ty: p.ty.clone(),
            })
            .collect();
        Obligation {
            item: s.name.clone(),
            class: ObligationClass::Contract,
            role: ObligationRole::Certification,
            ast_slice: AstSlice::Block(Box::new(s.body.clone())),
            env: ObligationEnv {
                params,
                req: None,
                spec_defs: called_spec_fns,
                ..ObligationEnv::default()
            },
        }
    }

    /// Mint the per-item REGISTRY-TERMINATION certification obligation (REQ-1.2):
    /// an item with a non-empty `called_spec_fns` set carries this class
    /// item-wide, conjoined with its CONTRACT class. The `ast_slice` is the item's
    /// own body (the descent measures live in `env.spec_defs`'s reached spec-fns +
    /// the item's own `dec`); the env's `spec_defs` is the full closure so the
    /// engine that discharges it (Verus's dec-check today; a Lean well-foundedness
    /// proof in increment (ii)) sees EVERY reached spec-fn. Returns `None` when the
    /// set is empty (no spec-fn dependency → no registry-termination obligation).
    #[must_use]
    pub fn registry_termination(
        item: &str,
        body: AstSlice,
        called_spec_fns: Vec<String>,
    ) -> Option<Obligation> {
        if called_spec_fns.is_empty() {
            return None;
        }
        Some(Obligation {
            item: item.to_string(),
            class: ObligationClass::RegistryTermination,
            role: ObligationRole::Certification,
            ast_slice: body,
            env: ObligationEnv {
                spec_defs: called_spec_fns,
                ..ObligationEnv::default()
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use thermite_syntax::{Item, Program};

    fn parse_one(src: &str) -> Program {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        parsed.program
    }

    fn fn_item<'a>(p: &'a Program, name: &str) -> &'a FnItem {
        p.items
            .iter()
            .find_map(|i| match i {
                Item::Fn(f) if f.name == name => Some(f),
                _ => None,
            })
            .expect("fn present")
    }

    // REQ-1: the CONTRACT obligation reifies the fn's body + params + req as
    // NEUTRAL content (no Verus strings) — the params carry their Thermite types,
    // the req is an AST node. Expected from the design's §1 artifact shape (R-CHAR-3).
    #[test]
    fn contract_obligation_is_neutral_content() {
        let p = parse_one(
            "fn add(x: u64, y: u64) -> u64 req x < 100 ens result == x + y fx pure { x + y }",
        );
        let f = fn_item(&p, "add");
        let o = Obligation::contract_for_fn(f, vec![]);
        assert_eq!(o.item, "add");
        assert_eq!(o.class, ObligationClass::Contract);
        assert_eq!(o.role, ObligationRole::Certification);
        // The params carry THERMITE types, not Verus strings.
        assert_eq!(o.env.params.len(), 2);
        assert_eq!(o.env.params[0].name, "x");
        assert_eq!(
            o.env.params[0].ty,
            Type::Prim(thermite_syntax::PrimType::U64)
        );
        // The req is an AST node (not rendered text).
        assert!(o.env.req.is_some());
        // The body slice is a Block (the production side).
        assert!(matches!(o.ast_slice, AstSlice::Block(_)));
        // No spec-fn deps for a pure-scalar item → no registry-termination class.
        assert!(o.env.spec_defs.is_empty());
    }

    // REQ-1.2: an item with a NON-empty called-spec-fn set gets a
    // REGISTRY-TERMINATION obligation; an EMPTY set yields None. Expected from the
    // design's REQ-1.2 class-assignment condition (R-CHAR-3).
    #[test]
    fn registry_termination_minted_iff_called_spec_fns_nonempty() {
        let body = AstSlice::Block(Box::new(Block {
            stmts: Vec::new(),
            tail: None,
        }));
        assert!(
            Obligation::registry_termination("f", body.clone(), vec![]).is_none(),
            "empty called-spec-fn set → NO registry-termination obligation"
        );
        let o = Obligation::registry_termination("f", body, vec!["spec_sum".to_string()])
            .expect("non-empty set → a registry-termination obligation");
        assert_eq!(o.class, ObligationClass::RegistryTermination);
        assert_eq!(o.env.spec_defs, vec!["spec_sum".to_string()]);
    }

    // REQ-1 / AC-1: the artifact is a prover-NEUTRAL VALUE — `Clone + PartialEq +
    // Eq` (the AST does not derive serde in production; see the module doc's
    // "honest scope" note). A clone is structurally equal (the comparable,
    // inspectable content the engine renders).
    #[test]
    fn obligation_is_a_comparable_neutral_value() {
        let p = parse_one("fn id(x: u64) -> u64 req true ens result == x fx pure { x }");
        let f = fn_item(&p, "id");
        let o = Obligation::contract_for_fn(f, vec![]);
        let clone = o.clone();
        assert_eq!(
            o, clone,
            "the Obligation is a comparable neutral value (REQ-1)"
        );
        // A DIFFERENT item is not equal (the value carries real content).
        let p2 = parse_one("fn other(y: u64) -> u64 req true ens result == y fx pure { y }");
        let o2 = Obligation::contract_for_fn(fn_item(&p2, "other"), vec![]);
        assert_ne!(o, o2);
    }

    // AC-1: the class tags are stable + distinct (cache-key domain separation).
    #[test]
    fn class_tags_are_distinct() {
        let all = [
            ObligationClass::Contract,
            ObligationClass::Exec,
            ObligationClass::Body,
            ObligationClass::LoopEntry,
            ObligationClass::LoopPreservation,
            ObligationClass::LoopExit,
            ObligationClass::Overflow,
            ObligationClass::Termination,
            ObligationClass::RegistryTermination,
        ];
        let mut tags: Vec<&str> = all.iter().map(|c| c.tag()).collect();
        tags.sort_unstable();
        let n = tags.len();
        tags.dedup();
        assert_eq!(tags.len(), n, "every class tag is distinct");
    }
}
