//! The independent operational-semantics reference state-denotation for the frozen
//! straight-line exec-statement subset (`.design/verified/exec-stmt-tv.md` REQ-2;
//! epic crosslink #158, blocker #159; `thermite-design.md` §4.1/§6).
//!
//! [`body_ref_state`] maps a straight-line [`Block`] (the frozen 2.2.1 subset:
//! `let`/mutable-`let`/assignment/`if`-as-statement/sequencing/tail/tail-`return`,
//! no loops) to a Verus exec expression string giving the body's final state as a
//! closed-form function of the initial state (the fn params). It is the state
//! analogue of step 2.1's [`crate::exec_encode::exec_ref_value`] (which gives a
//! single value): where 2.1 checks the per-RHS expression value, 2.2.1 adds the
//! orthogonal axis — the state sequencing and mutation-order faithfulness on top of
//! the per-RHS value faithfulness.
//!
//! ## The state-transformer semantics
//!
//! The program state is the environment of in-scope bindings (name -> its current
//! closed-form value expression in the inputs). Big-step evaluation threads an
//! initial environment (the params, each bound to itself) through the statement
//! sequence to a final environment; the body's value is the tail expression
//! evaluated in that final environment. Concretely:
//!
//! - `let [mut] n = <rhs>` binds `n` to the rhs substituted under the current env
//!   (each in-env var replaced by its current value expr). `{ let a = x + 1; let b
//!   = a * 2; b }` -> `a |-> (x + 1)`, then `b |-> ((x + 1) * 2)`, tail `b` ->
//!   `((x + 1) * 2)`.
//! - `n = <rhs>` (assignment / mutation) rebinds the in-scope cell `n` to the rhs
//!   substituted under the current env, order-sensitive: `s = s + 1; s = s * 2`
//!   threads `s |-> x` -> `s |-> (x + 1)` -> `s |-> ((x + 1) * 2)`, but the reorder
//!   `s = s * 2; s = s + 1` threads to `((x * 2) + 1)`, a different closed form
//!   (the state-sequencing check in `exec-stmt-tv.md` AC-3).
//! - `if c { .. } else { .. }` as the body tail composes the two branch
//!   state-transformers into a Verus `if`-expression over the (substituted)
//!   condition — `if c { <then-tail> } else { <else-tail> }` (`exec-stmt-tv.md`
//!   AC-4).
//! - the body's final value is the tail expr (or a tail `return <e>`) evaluated in
//!   the final env. A multi-cell final state is a tuple `(<cell0>, <cell1>, ...)`:
//!   the tail `(a, b)` projects the final `a`/`b` cells (the design's
//!   least-confident #1, grounded by B4).
//!
//! The substitution + threading + branch-composition + tuple projection are the
//! only new logic; every RHS / condition / branch-tail value is encoded by reusing
//! [`crate::exec_encode::exec_ref_value`] on the env-substituted [`Expr`] (the
//! per-RHS bounded-value reference is already independent — it carries the #122
//! inner-paren / #146 cast-`<` / bounded-overflow disciplines). So a value-infidel
//! RHS (the #122/#146/wrong-op class) is also caught by the same body obligation
//! (`exec-stmt-tv.md` AC-5).
//!
//! ## The independence boundary (REQ-2 constraint, R-CHAR-3 / AC-6)
//!
//! This module must not call any `thermite_lower::lower::*` symbol; `thermite-tv`
//! does not depend on `thermite-lower` (`Cargo.toml`; the dep graph makes reuse a
//! compile error). The reference state-denotation is authored from the frozen-subset
//! big-step imperative semantics (`exec-stmt-tv.md` REQ-1/REQ-2), not from
//! `lower_block_inner`/`lower_stmt`. Agreement of production's `lower_exec_body` with
//! this reference is N-version differential evidence, not proof.
//!
//! ## Honest boundary (out of the frozen 2.2.1 subset -> an `Err`, never silent-wrong)
//!
//! A construct outside the straight-line subset is an
//! [`crate::exec_encode::RefEncodeError::Unsupported`] (R-CODE-2 / R-APG-1 — never a
//! panic, never a silent wrong denotation): a `Stmt::Loop`/`Break`/`Continue` (step
//! 2.2.2, kernel-gated), a mid-body early `return` nested in an `if` branch (the
//! multi-exit CPS form, out of v1), a `match`-as-statement, aggregate mutation
//! other than an exact indexed write to a declared native fixed array (`Vec::push`,
//! field mutation, and projection mutation need richer theories), and a re-shadow
//! `let x = ..; let x = ..` in the same block (the flat name->value env can't
//! represent it). A silent wrong denotation would compare a wrong reference.
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=thermite-tv-exec-stmt-body-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-TV-BODY-STATE-DENOTATION | shipped | `thermite-tv/src/exec_stmt_encode.rs` | Body-TV operational state denotation |  |
//! | REQ-TV-BODY-STMT-SUBSET | shipped | `thermite-tv/src/exec_stmt_encode.rs` | Body-TV straight-line statement subset |  |
//! <!-- /generated:reqs -->
//!
//! ## Loop extension — step 2.2.2-i (`.design/verified/loop-tv.md`; epic #169, blocker #163)
//!
//! [`loop_ref_obligations`] extends the straight-line state-transformer to a v1
//! frozen-subset `while` loop (`loop-tv.md` REQ-1/REQ-2): a single `while <cond>`
//! with declared `inv`+`dec`, a straight-line scalar or finite-record body, the loop
//! last before the tail. It produces the while-rule pieces plus an exact full-result
//! obligation emitters in [`crate::obligation`] turn into Z3-checkable Verus units:
//! entry (`inv` holds on the pre-loop entry state), preservation (one iteration of the
//! body carries `inv ∧ cond` to `inv`, reusing the shipped [`body_ref_state`] step),
//! and exit (the after-loop state is `inv ∧ ¬cond`-constrained). The after-loop state
//! threads as the design's opaque-but-invariant-constrained fresh cells: a loop cannot
//! produce a closed form (it is a fixpoint), so the post-loop cells are havocked +
//! re-constrained to `inv ∧ ¬cond` (the analogue of how Verus itself models a
//! loop's after-state). Every out-of-v1 loop (`loop`-kind, `break`/`continue`, a
//! mid-body `return`, a nested loop, non-finite/aliased state, a trivially-weak `inv`) is an
//! [`RefEncodeError::Unsupported`] (R-HONEST-3 — Skipped, never silently
//! Faithful).
//!
//! <!-- generated:reqs view=thermite-tv-exec-stmt-loop-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-TV-LOOP-REFERENCE-PIECES | shipped | `thermite-tv/src/exec_stmt_encode.rs` | Loop-TV reference obligation pieces |  |
//! | REQ-TV-LOOP-STMT-SUBSET | shipped | `thermite-tv/src/exec_stmt_encode.rs` | Loop-TV v1 loop subset recognizer |  |
//! <!-- /generated:reqs -->

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use thermite_syntax::ast::{
    BinOp, Block, Clause, Expr, IndexArg, LoopKind, LoopNode, MatchArm, Pattern, Stmt, Type,
};

use crate::exec_encode::{exec_ref_value, ExecRefCtx, RefEncodeError};

/// One direct field observed by named-record lifecycle TV. Array fields compare
/// their complete finite views; every other admitted finite field uses value
/// equality in the final-state obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFieldFrame {
    pub name: String,
    pub array_view: bool,
    pub ty: Option<Type>,
}

impl RecordFieldFrame {
    pub fn new(name: impl Into<String>, array_view: bool) -> Self {
        Self {
            name: name.into(),
            array_view,
            ty: None,
        }
    }

    /// Construct a field frame with its exact independently parsed source type.
    /// Nested lifecycle reconstruction requires this; the historical untyped
    /// constructor remains available for direct-field-only test frames.
    pub fn typed(name: impl Into<String>, ty: Type) -> Self {
        Self {
            name: name.into(),
            array_view: matches!(ty, Type::Array { .. }),
            ty: Some(ty),
        }
    }
}

/// One exclusive named-record parameter and its complete ordered direct-field
/// frame. The field inventory comes from the independently parsed declaration,
/// not from production lowering text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutableRecordFrame {
    pub name: String,
    /// Exact nominal source type when the frame comes from a parsed function
    /// signature. Historical hand-built tests may leave it absent; mutable-call
    /// composition requires it on both formal and actual roots.
    pub type_name: Option<String>,
    pub fields: Vec<RecordFieldFrame>,
}

/// One immutably borrowed finite named-record parameter and its complete ordered
/// direct-field frame.  Keeping this distinct from [`MutableRecordFrame`] makes
/// the alias rule explicit: a shared root may be observed by any number of shared
/// formals, but it may not overlap any exclusive actual in the same call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedRecordFrame {
    pub name: String,
    pub type_name: String,
    pub fields: Vec<RecordFieldFrame>,
}

/// One exclusively borrowed slice or fixed-array root with its exact parsed
/// pointee type.  Call-effect composition compares this type structurally and
/// threads the complete finite sequence from `old(root)@` to `final(root)@`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutableIndexedFrame {
    pub name: String,
    pub pointee: Type,
}

/// One immutably borrowed slice or fixed-array root with its exact parsed
/// pointee type. Shared formals snapshot the caller's current complete sequence;
/// they never contribute a copy-back state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedIndexedFrame {
    pub name: String,
    pub pointee: Type,
}

/// One independently parsed in-language callee whose body may transform one or
/// more exclusive finite-record, slice, or fixed-array parameters. The
/// reference side interprets this source body directly; production continues to
/// call the ordinary generated function, so changing either column is
/// observable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutableCallEffectFrame {
    pub name: String,
    /// All formal parameter names in signature order. Borrowed formals are
    /// identified by `mutable_records`, `mutable_indexed`, `shared_records`, or
    /// `shared_indexed`; the rest are value inputs.
    pub params: Vec<String>,
    pub mutable_records: Vec<MutableRecordFrame>,
    /// Exact exclusive slice/fixed-array formals whose complete sequence state
    /// is interpreted and copied back with the record post-state.
    pub mutable_indexed: Vec<MutableIndexedFrame>,
    /// Exact finite-record shared-reference formals.  These are snapshotted from
    /// the caller's current lifecycle state and remain read-only while the callee
    /// source body is interpreted.
    pub shared_records: Vec<SharedRecordFrame>,
    /// Exact shared slice/fixed-array formals snapshotted from the current
    /// caller state. Shared/shared aliases are admitted; overlap with an
    /// exclusive actual in the same call is rejected.
    pub shared_indexed: Vec<SharedIndexedFrame>,
    pub body: Block,
}

/// One finite named-record type available to owned-local body state
/// reconstruction. The ordered field inventory is independently derived from
/// the parsed declaration and is never inferred from production lowering text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedRecordFrame {
    pub type_name: String,
    pub fields: Vec<RecordFieldFrame>,
}

impl NamedRecordFrame {
    pub fn new(type_name: impl Into<String>, fields: Vec<RecordFieldFrame>) -> Self {
        Self {
            type_name: type_name.into(),
            fields,
        }
    }
}

/// Exact independently parsed payload shape for one user-enum variant. This is
/// reference-denotation metadata, not a production-lowering artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnumVariantShapeFrame {
    Unit,
    Tuple(Vec<Type>),
    Struct(Vec<RecordFieldFrame>),
}

/// One user-enum variant and its nominal owner/payload inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariantFrame {
    pub enum_name: String,
    pub variant_name: String,
    pub shape: EnumVariantShapeFrame,
}

impl EnumVariantFrame {
    pub fn new(
        enum_name: impl Into<String>,
        variant_name: impl Into<String>,
        shape: EnumVariantShapeFrame,
    ) -> Self {
        Self {
            enum_name: enum_name.into(),
            variant_name: variant_name.into(),
            shape,
        }
    }
}

impl MutableRecordFrame {
    pub fn new(name: impl Into<String>, fields: Vec<RecordFieldFrame>) -> Self {
        Self {
            name: name.into(),
            type_name: None,
            fields,
        }
    }

    pub fn typed(
        name: impl Into<String>,
        type_name: impl Into<String>,
        fields: Vec<RecordFieldFrame>,
    ) -> Self {
        Self {
            name: name.into(),
            type_name: Some(type_name.into()),
            fields,
        }
    }
}

impl SharedRecordFrame {
    pub fn typed(
        name: impl Into<String>,
        type_name: impl Into<String>,
        fields: Vec<RecordFieldFrame>,
    ) -> Self {
        Self {
            name: name.into(),
            type_name: type_name.into(),
            fields,
        }
    }
}

impl MutableIndexedFrame {
    pub fn new(name: impl Into<String>, pointee: Type) -> Self {
        Self {
            name: name.into(),
            pointee,
        }
    }
}

impl SharedIndexedFrame {
    pub fn new(name: impl Into<String>, pointee: Type) -> Self {
        Self {
            name: name.into(),
            pointee,
        }
    }
}

impl MutableCallEffectFrame {
    pub fn new(
        name: impl Into<String>,
        params: Vec<String>,
        mutable_records: Vec<MutableRecordFrame>,
        body: Block,
    ) -> Self {
        Self {
            name: name.into(),
            params,
            mutable_records,
            mutable_indexed: Vec::new(),
            shared_records: Vec::new(),
            shared_indexed: Vec::new(),
            body,
        }
    }

    /// Add exact shared finite-record formals to a mutable call-effect frame.
    pub fn with_shared_records(mut self, records: Vec<SharedRecordFrame>) -> Self {
        self.shared_records = records;
        self
    }

    /// Add exact exclusive slice/fixed-array formals to this call-effect frame.
    pub fn with_mutable_indexed(mut self, indexed: Vec<MutableIndexedFrame>) -> Self {
        self.mutable_indexed = indexed;
        self
    }

    /// Add exact shared slice/fixed-array formals to this call-effect frame.
    pub fn with_shared_indexed(mut self, indexed: Vec<SharedIndexedFrame>) -> Self {
        self.shared_indexed = indexed;
        self
    }
}

/// The body-reference-encoding context (REQ-2). Carries the slice-bound names (so a
/// slice index in an RHS / tail encodes to the spec-view element value `xs[i as
/// int]`, mirroring the obligation's `xs: &[u32]` binding) and the native
/// fixed-array bindings whose reads and indexed writes use finite views — the same
/// information [`ExecRefCtx`] carries for the per-expr encoder, reused here for
/// the per-RHS value encoding. It carries no `nat`-coerce set (the exec state is
/// bounded-typed, never `nat`-coerced — the same as step 2.1).
///
/// This is the body dual of [`ExecRefCtx`]: where `ExecRefCtx` frames a single exec
/// expression, `BodyRefCtx` frames a whole straight-line body. The state-threading
/// environment is internal to [`body_ref_state`] (it is the closed-form-in-the-
/// inputs map, not an external knob); the ctx carries the slice and fixed-array
/// frames plus the result representation.
#[derive(Debug, Clone, Default)]
pub struct BodyRefCtx {
    /// Names bound as a slice (`&[T]`) param in the obligation — an `Index` over
    /// such a name in any RHS / condition / tail encodes to the spec-view element
    /// value `xs[i as int]` (delegated to [`exec_ref_value`] via the [`ExecRefCtx`]
    /// this ctx builds). Empty for the scalar-only B1-B4 bodies.
    slice_bound: BTreeSet<String>,
    /// Slice or fixed-array parameters held through an exclusive borrow. Their
    /// indexed writes are modeled as exact finite-sequence updates from
    /// `old(param)@` to `final(param)@`.
    mutable_indexed_bound: BTreeSet<String>,
    fixed_array_bound: BTreeSet<String>,
    fixed_array_fields: BTreeSet<String>,
    result_is_fixed_array: bool,
    result_is_unit: bool,
    mutable_records: Vec<MutableRecordFrame>,
    mutable_indexed: Vec<MutableIndexedFrame>,
    shared_records: Vec<SharedRecordFrame>,
    shared_indexed: Vec<SharedIndexedFrame>,
    /// Exact source bodies and record/indexed formal frames for reachable
    /// in-language mutable-reference callees. Boundary declarations never enter
    /// this map.
    mutable_call_effects: BTreeMap<String, MutableCallEffectFrame>,
    named_records: BTreeMap<String, NamedRecordFrame>,
    constructor_records: BTreeMap<String, NamedRecordFrame>,
    enum_variants: BTreeMap<String, EnumVariantFrame>,
    result_record: Option<NamedRecordFrame>,
}

impl BodyRefCtx {
    /// A context in which the named free vars are bound as slice (`&[T]`) params.
    pub fn with_slice_bound<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        BodyRefCtx {
            slice_bound: names.into_iter().map(Into::into).collect(),
            mutable_indexed_bound: BTreeSet::new(),
            fixed_array_bound: BTreeSet::new(),
            fixed_array_fields: BTreeSet::new(),
            result_is_fixed_array: false,
            result_is_unit: false,
            mutable_records: Vec::new(),
            mutable_indexed: Vec::new(),
            shared_records: Vec::new(),
            shared_indexed: Vec::new(),
            mutable_call_effects: BTreeMap::new(),
            named_records: BTreeMap::new(),
            constructor_records: BTreeMap::new(),
            enum_variants: BTreeMap::new(),
            result_record: None,
        }
    }

    /// Add the exclusive slice/fixed-array borrows whose indexed writes are part
    /// of the observable body state.
    pub fn with_mutable_indexed_bound<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.mutable_indexed_bound = names.into_iter().map(Into::into).collect();
        self
    }

    /// Add native fixed-array inputs to the body frame.
    pub fn with_fixed_array_bound<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.fixed_array_bound = names.into_iter().map(Into::into).collect();
        self
    }

    /// Add exact direct `root.field` paths whose parsed field type is a native
    /// fixed array.
    pub fn with_fixed_array_fields<I, S>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.fixed_array_fields = paths.into_iter().map(Into::into).collect();
        self
    }

    /// Record that the body's result is a native fixed array and must be
    /// compared extensionally through its finite sequence view.
    pub fn with_fixed_array_result(mut self, is_array: bool) -> Self {
        self.result_is_fixed_array = is_array;
        self
    }

    /// Record whether a tail-less body has the explicit unit result type.
    pub fn with_unit_result(mut self, is_unit: bool) -> Self {
        self.result_is_unit = is_unit;
        self
    }

    /// Add complete direct-field frames for exclusive named-record borrows.
    pub fn with_mutable_records(mut self, records: Vec<MutableRecordFrame>) -> Self {
        self.mutable_records = records;
        self
    }

    /// Add exact exclusive slice/fixed-array root metadata. The historical name
    /// set remains the observation switch; these frames additionally make call
    /// argument types independently comparable.
    pub fn with_mutable_indexed(mut self, indexed: Vec<MutableIndexedFrame>) -> Self {
        self.mutable_indexed = indexed;
        self
    }

    /// Add complete direct-field frames for immutably borrowed named records.
    pub fn with_shared_records(mut self, records: Vec<SharedRecordFrame>) -> Self {
        self.shared_records = records;
        self
    }

    /// Add exact shared slice/fixed-array root metadata. These roots are
    /// immutable snapshots but may be supplied to a mutable callee as shared
    /// actuals when they do not overlap an exclusive actual.
    pub fn with_shared_indexed(mut self, indexed: Vec<SharedIndexedFrame>) -> Self {
        self.shared_indexed = indexed;
        self
    }

    /// Add reachable in-language mutable-reference callees. Forge supplies these
    /// from the already validated unique function namespace of the reachable
    /// source closure.
    pub fn with_mutable_call_effects(mut self, effects: Vec<MutableCallEffectFrame>) -> Self {
        self.mutable_call_effects = effects
            .into_iter()
            .map(|effect| (effect.name.clone(), effect))
            .collect();
        self
    }

    /// Add the exact finite named-record declarations usable by typed owned
    /// locals in this body.
    pub fn with_named_records(mut self, records: Vec<NamedRecordFrame>) -> Self {
        self.named_records = records
            .into_iter()
            .map(|record| (record.type_name.clone(), record))
            .collect();
        self
    }

    /// Add every parsed record declaration used to recover constructor-field
    /// types. This is deliberately distinct from `named_records`, whose smaller
    /// structural set controls admission of owned record mutation.
    pub fn with_constructor_records(mut self, records: Vec<NamedRecordFrame>) -> Self {
        self.constructor_records = records
            .into_iter()
            .map(|record| (record.type_name.clone(), record))
            .collect();
        self
    }

    /// Add the independently parsed user-enum variant ownership map used to
    /// qualify unqualified patterns in body-reference `match` expressions.
    pub fn with_enum_variants<I>(mut self, variants: I) -> Self
    where
        I: IntoIterator<Item = EnumVariantFrame>,
    {
        self.enum_variants = variants
            .into_iter()
            .map(|variant| (variant.variant_name.clone(), variant))
            .collect();
        self
    }

    /// Remove enum constructor spellings shadowed by value bindings in the
    /// current function frame.  Thermite resolves a bare path such as `Idle`
    /// to a parameter/local before considering an unqualified enum variant, so
    /// the independent reference must preserve that lexical distinction too.
    pub fn with_bound_value_names<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for name in names {
            self.enum_variants.remove(name.as_ref());
        }
        self
    }

    /// Record the exact field frame of a named-record result. Result equality is
    /// then emitted per field, using complete sequence views for native arrays.
    pub fn with_result_record(mut self, record: Option<NamedRecordFrame>) -> Self {
        self.result_record = record;
        self
    }

    /// Build the [`ExecRefCtx`] the per-RHS value encoder uses (the slice-bound set
    /// passes straight through — every RHS / tail value is a step-2.1 exec value).
    fn exec_ref_ctx(&self) -> ExecRefCtx {
        ExecRefCtx::with_slice_bound(self.slice_bound.iter().cloned())
            .with_fixed_array_bound(self.fixed_array_bound.iter().cloned())
            .with_fixed_array_fields(self.fixed_array_fields.iter().cloned())
    }

    fn is_mutable_indexed_bound(&self, name: &str) -> bool {
        self.mutable_indexed_bound.contains(name)
    }

    fn named_record(&self, type_name: &str) -> Option<&NamedRecordFrame> {
        self.named_records.get(type_name)
    }

    fn constructor_record(&self, type_name: &str) -> Option<&NamedRecordFrame> {
        self.constructor_records.get(type_name)
    }

    fn enum_variant(&self, path: &[String]) -> Option<&EnumVariantFrame> {
        match path {
            [variant] => self.enum_variants.get(variant),
            [enumeration, variant] => self
                .enum_variants
                .get(variant)
                .filter(|frame| frame.enum_name == *enumeration),
            _ => None,
        }
    }

    fn without_enum_variants(&self, names: &BTreeSet<String>) -> Self {
        let mut scoped = self.clone();
        for name in names {
            scoped.enum_variants.remove(name);
        }
        scoped
    }

    fn mutable_record(&self, name: &str) -> Option<&MutableRecordFrame> {
        self.mutable_records
            .iter()
            .find(|record| record.name == name)
    }

    fn shared_record(&self, name: &str) -> Option<&SharedRecordFrame> {
        self.shared_records
            .iter()
            .find(|record| record.name == name)
    }

    fn mutable_indexed(&self, name: &str) -> Option<&MutableIndexedFrame> {
        self.mutable_indexed
            .iter()
            .find(|indexed| indexed.name == name)
    }

    fn shared_indexed(&self, name: &str) -> Option<&SharedIndexedFrame> {
        self.shared_indexed
            .iter()
            .find(|indexed| indexed.name == name)
    }

    fn mutable_call_effect(&self, name: &str) -> Option<&MutableCallEffectFrame> {
        self.mutable_call_effects.get(name)
    }

    fn qualify_pattern_path(&self, path: &[String]) -> String {
        if let [variant] = path {
            if let Some(frame) = self.enum_variants.get(variant) {
                return format!("{}::{variant}", frame.enum_name);
            }
        }
        path.join("::")
    }
}

/// The big-step state environment: each in-scope binding name -> its current
/// closed-form value [`Expr`] (a function of the initial inputs). A `let`/assignment
/// rebinds a name to its RHS substituted under this env; the tail is evaluated under
/// the final env. Keeping the value as an [`Expr`] (not a string) lets every value
/// be encoded by reusing [`exec_ref_value`] on the substituted [`Expr`] — the
/// independence boundary (the per-RHS bounded-value reference is unchanged), so the
/// only new logic is the substitution + threading.
#[derive(Clone, Default)]
struct Env {
    values: BTreeMap<String, Expr>,
    fixed_arrays: BTreeSet<String>,
    named_records: BTreeMap<String, String>,
}

impl Env {
    fn new() -> Self {
        Self::default()
    }

    fn contains_key(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }

    fn get(&self, name: &str) -> Option<&Expr> {
        self.values.get(name)
    }

    fn insert(&mut self, name: String, value: Expr) {
        self.values.insert(name, value);
    }

    fn keys(&self) -> impl Iterator<Item = &String> {
        self.values.keys()
    }

    fn mark_fixed_array(&mut self, name: String) {
        self.fixed_arrays.insert(name);
    }

    fn is_fixed_array(&self, name: &str) -> bool {
        self.fixed_arrays.contains(name)
    }

    fn mark_named_record(&mut self, name: String, type_name: String) {
        self.named_records.insert(name, type_name);
    }

    fn named_record_type(&self, name: &str) -> Option<&str> {
        self.named_records.get(name).map(String::as_str)
    }

    fn without_bindings(&self, names: &BTreeSet<String>) -> Self {
        let mut scoped = self.clone();
        for name in names {
            scoped.values.remove(name);
            scoped.fixed_arrays.remove(name);
            scoped.named_records.remove(name);
        }
        scoped
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NestedLvalueStep {
    Field(String),
    Index(Expr),
}

fn nested_lvalue_path(target: &Expr) -> Result<(String, Vec<NestedLvalueStep>), RefEncodeError> {
    fn collect(expr: &Expr, steps: &mut Vec<NestedLvalueStep>) -> Result<String, RefEncodeError> {
        match expr {
            Expr::Path(path) => match path.as_slice() {
                [root] => Ok(root.clone()),
                _ => Err(RefEncodeError::Unsupported(
                    "nested mutation root must be one local or parameter name".to_string(),
                )),
            },
            Expr::Field { receiver, name } => {
                let root = collect(receiver, steps)?;
                steps.push(NestedLvalueStep::Field(name.clone()));
                Ok(root)
            }
            Expr::Index {
                base,
                index: IndexArg::Single(index),
            } => {
                let root = collect(base, steps)?;
                steps.push(NestedLvalueStep::Index(index.as_ref().clone()));
                Ok(root)
            }
            _ => Err(RefEncodeError::Unsupported(
                "nested mutation admits exact fields and optionally one final single fixed-array index"
                    .to_string(),
            )),
        }
    }

    let mut steps = Vec::new();
    let root = collect(target, &mut steps)?;
    Ok((root, steps))
}

fn target_contains_field(target: &Expr) -> bool {
    match target {
        Expr::Field { .. } => true,
        Expr::Index { base, .. } => target_contains_field(base),
        _ => false,
    }
}

fn reference_expr_is_spec_int(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Binary {
            op: BinOp::Add
                | BinOp::Sub
                | BinOp::Mul
                | BinOp::Div
                | BinOp::Rem
                | BinOp::Shl
                | BinOp::Shr
                | BinOp::BitAnd
                | BinOp::BitOr
                | BinOp::BitXor,
            ..
        }
    )
}

fn contextualize_assignment_value(expr: Expr, target_ty: &Type, spec_int: bool) -> Expr {
    if spec_int
        && matches!(
            target_ty,
            Type::Prim(
                thermite_syntax::PrimType::U8
                    | thermite_syntax::PrimType::U16
                    | thermite_syntax::PrimType::U32
                    | thermite_syntax::PrimType::U64
                    | thermite_syntax::PrimType::Usize
            )
        )
    {
        Expr::Cast {
            expr: Box::new(expr),
            ty: target_ty.clone(),
        }
    } else {
        expr
    }
}

/// Restore the bounded type supplied by an aggregate constructor field.  A
/// reference term appears in an `ensures` expression, where an unsuffixed
/// arithmetic literal otherwise lifts the whole expression to mathematical
/// `int`; construction of a parsed `u64`/`usize` field must instead denote the
/// same bounded operation as the executable body.
fn contextualize_value_for_type(
    expr: Expr,
    target_ty: &Type,
    ctx: &BodyRefCtx,
) -> Result<Expr, RefEncodeError> {
    let expr = contextualize_constructor_value(expr, ctx)?;
    if reference_expr_is_spec_int(&expr) {
        return Ok(contextualize_assignment_value(expr, target_ty, true));
    }
    match (expr, target_ty) {
        (
            literal @ Expr::IntLit { .. },
            Type::Prim(
                thermite_syntax::PrimType::U8
                | thermite_syntax::PrimType::U16
                | thermite_syntax::PrimType::U32
                | thermite_syntax::PrimType::U64
                | thermite_syntax::PrimType::Usize,
            ),
        ) => Ok(Expr::Cast {
            expr: Box::new(literal),
            ty: target_ty.clone(),
        }),
        (Expr::If { cond, then, else_ }, _) => {
            let contextualize_block = |mut block: Block| -> Result<Block, RefEncodeError> {
                if let Some(tail) = block.tail.take() {
                    block.tail = Some(Box::new(contextualize_value_for_type(
                        *tail, target_ty, ctx,
                    )?));
                }
                Ok(block)
            };
            Ok(Expr::If {
                cond,
                then: contextualize_block(then)?,
                else_: contextualize_block(else_)?,
            })
        }
        (Expr::Match { scrutinee, arms }, _) => Ok(Expr::Match {
            scrutinee,
            arms: arms
                .into_iter()
                .map(|arm| {
                    Ok(MatchArm {
                        pattern: arm.pattern,
                        guard: arm.guard,
                        body: contextualize_value_for_type(arm.body, target_ty, ctx)?,
                    })
                })
                .collect::<Result<Vec<_>, RefEncodeError>>()?,
        }),
        (Expr::Tuple(values), Type::Tuple(types)) if values.len() == types.len() => {
            Ok(Expr::Tuple(
                values
                    .into_iter()
                    .zip(types)
                    .map(|(value, ty)| contextualize_value_for_type(value, ty, ctx))
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
        (Expr::Array(values), Type::Array { elem, .. }) => Ok(Expr::Array(
            values
                .into_iter()
                .map(|value| contextualize_value_for_type(value, elem, ctx))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        (Expr::ArrayRepeat { value, len }, Type::Array { elem, .. }) => Ok(Expr::ArrayRepeat {
            value: Box::new(contextualize_value_for_type(*value, elem, ctx)?),
            len,
        }),
        (value, _) => Ok(value),
    }
}

/// Independently qualify user-variant constructors and reapply the exact parsed
/// payload field types. This is intentionally separate from pattern lowering and
/// from the production lowerer.
fn contextualize_constructor_value(expr: Expr, ctx: &BodyRefCtx) -> Result<Expr, RefEncodeError> {
    match expr {
        Expr::Path(path) => {
            let Some(variant) = ctx.enum_variant(&path) else {
                return Ok(Expr::Path(path));
            };
            if !matches!(variant.shape, EnumVariantShapeFrame::Unit) {
                return Err(RefEncodeError::Unsupported(format!(
                    "payload-bearing variant `{}::{}` used as a unit value",
                    variant.enum_name, variant.variant_name
                )));
            }
            Ok(Expr::Path(vec![
                variant.enum_name.clone(),
                variant.variant_name.clone(),
            ]))
        }
        Expr::Call { callee, args } => {
            let Expr::Path(path) = callee.as_ref() else {
                return Ok(Expr::Call { callee, args });
            };
            let Some(variant) = ctx.enum_variant(path) else {
                return Ok(Expr::Call { callee, args });
            };
            let EnumVariantShapeFrame::Tuple(types) = &variant.shape else {
                return Err(RefEncodeError::Unsupported(format!(
                    "non-tuple variant `{}::{}` used as a call constructor",
                    variant.enum_name, variant.variant_name
                )));
            };
            if args.len() != types.len() {
                return Err(RefEncodeError::Unsupported(format!(
                    "variant `{}::{}` constructor has {} arguments but its parsed payload has {} fields",
                    variant.enum_name,
                    variant.variant_name,
                    args.len(),
                    types.len()
                )));
            }
            let args = args
                .into_iter()
                .zip(types)
                .map(|(argument, ty)| contextualize_value_for_type(argument, ty, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Expr::Call {
                callee: Box::new(Expr::Path(vec![
                    variant.enum_name.clone(),
                    variant.variant_name.clone(),
                ])),
                args,
            })
        }
        Expr::StructLit { path, fields } => {
            let (qualified_path, field_frames): (Vec<String>, Vec<RecordFieldFrame>) =
                if let Some(variant) = ctx.enum_variant(&path) {
                    let EnumVariantShapeFrame::Struct(field_frames) = &variant.shape else {
                        return Err(RefEncodeError::Unsupported(format!(
                            "non-struct variant `{}::{}` used as a brace constructor",
                            variant.enum_name, variant.variant_name
                        )));
                    };
                    (
                        vec![variant.enum_name.clone(), variant.variant_name.clone()],
                        field_frames.clone(),
                    )
                } else {
                    let record_name = path.join("::");
                    let Some(record) = ctx.constructor_record(&record_name) else {
                        return Ok(Expr::StructLit { path, fields });
                    };
                    (path, record.fields.clone())
                };
            let fields = fields
                .into_iter()
                .map(|(name, value)| {
                    let field = field_frames
                        .iter()
                        .find(|field| field.name == name)
                        .ok_or_else(|| {
                            RefEncodeError::Unsupported(format!(
                                "aggregate constructor `{}` has no parsed field `{name}`",
                                qualified_path.join("::")
                            ))
                        })?;
                    let value = match &field.ty {
                        Some(ty) => contextualize_value_for_type(value, ty, ctx)?,
                        None => contextualize_constructor_value(value, ctx)?,
                    };
                    Ok((name, value))
                })
                .collect::<Result<Vec<_>, RefEncodeError>>()?;
            Ok(Expr::StructLit {
                path: qualified_path,
                fields,
            })
        }
        other => Ok(other),
    }
}

/// Rebuild a nested finite value independently of production lowering. Every
/// enclosing record is reconstructed with all sibling fields projected from
/// the pre-write value; an optional terminal index becomes one exact array
/// update. Index-then-field aliasing remains outside this increment.
fn rebuild_nested_value(
    current: Expr,
    current_ty: &Type,
    steps: &[NestedLvalueStep],
    changed: Expr,
    changed_spec_int: bool,
    ctx: &BodyRefCtx,
) -> Result<Expr, RefEncodeError> {
    let Some((step, rest)) = steps.split_first() else {
        return Ok(changed);
    };
    match step {
        NestedLvalueStep::Field(field) => {
            let Type::Named(type_name) = current_ty else {
                return Err(RefEncodeError::Unsupported(format!(
                    "field `{field}` is projected from a non-record nested value"
                )));
            };
            let record = ctx.named_record(type_name).ok_or_else(|| {
                RefEncodeError::Unsupported(format!(
                    "nested mutation lost finite record declaration `{type_name}`"
                ))
            })?;
            let field_frame = record
                .fields
                .iter()
                .find(|candidate| candidate.name == *field)
                .ok_or_else(|| {
                    RefEncodeError::Unsupported(format!(
                        "record `{type_name}` has no exact field `{field}`"
                    ))
                })?;
            let next = if rest.is_empty() {
                match &field_frame.ty {
                    Some(field_ty) => {
                        contextualize_assignment_value(changed, field_ty, changed_spec_int)
                    }
                    None => changed,
                }
            } else {
                let field_ty = field_frame.ty.as_ref().ok_or_else(|| {
                    RefEncodeError::Unsupported(format!(
                        "nested mutation field `{type_name}.{field}` has no independent source type"
                    ))
                })?;
                rebuild_nested_value(
                    Expr::Field {
                        receiver: Box::new(current.clone()),
                        name: field.clone(),
                    },
                    field_ty,
                    rest,
                    changed,
                    changed_spec_int,
                    ctx,
                )?
            };
            Ok(Expr::StructLit {
                path: vec![type_name.clone()],
                fields: record
                    .fields
                    .iter()
                    .map(|candidate| {
                        let value = if candidate.name == *field {
                            next.clone()
                        } else {
                            Expr::Field {
                                receiver: Box::new(current.clone()),
                                name: candidate.name.clone(),
                            }
                        };
                        (candidate.name.clone(), value)
                    })
                    .collect(),
            })
        }
        NestedLvalueStep::Index(index) => {
            if !rest.is_empty() {
                return Err(RefEncodeError::Unsupported(
                    "a fixed-array index must be the final nested mutation projection".to_string(),
                ));
            }
            let Type::Array { elem, .. } = current_ty else {
                return Err(RefEncodeError::Unsupported(
                    "the final indexed nested mutation receiver is not a fixed array".to_string(),
                ));
            };
            Ok(Expr::Call {
                callee: Box::new(Expr::Path(vec![
                    "vstd".to_string(),
                    "array".to_string(),
                    "spec_array_update".to_string(),
                ])),
                args: vec![
                    current,
                    index.clone(),
                    contextualize_assignment_value(changed, elem, changed_spec_int),
                ],
            })
        }
    }
}

/// Encode a straight-line [`Block`] (the frozen 2.2.1 subset) to a Verus exec
/// expression string giving the body's final state (the tail value) as a closed-form
/// function of the inputs, independently of the production lowerer (REQ-2). The
/// initial environment is implicit (each free var = itself); each `let`/assignment
/// threads the env in order (mutation = order-sensitive substitution); an `if`-tail
/// composes the branch transformers; the tail (or tail-`return`) projects the final
/// state (a multi-cell tail tuple -> a Verus tuple).
///
/// Reuses [`exec_ref_value`] on each env-substituted RHS / condition / branch-tail:
/// the per-RHS bounded-value reference (the #122/#146/overflow disciplines) is
/// unchanged; the new logic is only the state threading. Returns
/// [`RefEncodeError::Unsupported`] (never a panic / silent wrong encoding) for a
/// construct outside the frozen straight-line subset (a loop, a mid-branch early
/// return, a `match`-stmt, a mutation outside the admitted finite aggregate closure,
/// a re-shadow).
pub fn body_ref_state(block: &Block, ctx: &BodyRefCtx) -> Result<String, RefEncodeError> {
    let mut env: Env = Env::new();
    encode_block_tail(block, &mut env, ctx)
}

/// Build the body-refinement obligation's `ensures` predicate comparing the exec fn
/// `result` (named by `result_name`) to the reference final state (REQ-3 helper for
/// [`crate::obligation::body_equivalence_obligation`]). For a single-cell body this
/// is the scalar equality `result == <body_ref_state>` (the same form step 2.1 uses,
/// where `u64 == <u64 arithmetic>` Verus-coerces fine). For a multi-cell body whose
/// tail is a tuple (`(a, b)`, B4 — the design's least-confident #1) it is the
/// per-projection conjunction `result.0 == <cell0> && result.1 == <cell1>`: Verus has
/// no `SpecEq` between a `(u64, u64)` result and a `(int, int)` tuple literal (each
/// element's bounded arithmetic elaborates to `int`), but the per-projection
/// `result.0: u64 == <u64 arithmetic>` compares element-wise at the bounded type
/// (the grounded projection equality `r.0 == b`, `ast.rs` `TupleProj`). The
/// reorder and wrong-cell tests fail on whichever projection differs (B4's `b` cell).
///
/// This is the obligation-shape concern (how `result` is compared), kept distinct
/// from [`body_ref_state`] (the state denotation itself, REQ-2). Reuses the same
/// state-threading; the only addition is the multi-cell projection split.
pub fn body_ref_state_ensures(
    block: &Block,
    result_name: &str,
    ctx: &BodyRefCtx,
) -> Result<String, RefEncodeError> {
    if !ctx.mutable_records.is_empty() || !ctx.mutable_indexed_bound.is_empty() {
        return aggregate_lifecycle_ensures(block, result_name, ctx);
    }
    let mut conjuncts = Vec::new();
    if let Some(record) = &ctx.result_record {
        let reference = body_ref_state(block, ctx)?;
        for field in &record.fields {
            if field.array_view {
                conjuncts.push(format!(
                    "{result_name}.{}@ == (({reference}).{})@",
                    field.name, field.name
                ));
            } else {
                conjuncts.push(format!(
                    "{result_name}.{} == ({reference}).{}",
                    field.name, field.name
                ));
            }
        }
    }
    // A multi-cell body is one whose tail is a tuple (the final state spans cells).
    // Each cell is encoded under the body's final env (the same threading), then
    // compared to the matching `result.<i>` projection at the bounded type.
    if ctx.result_record.is_none() {
        if let Some(tail) = &block.tail {
            if let Expr::Tuple(elems) = tail.as_ref() {
                let mut env: Env = Env::new();
                for stmt in &block.stmts {
                    thread_stmt(stmt, &mut env, ctx)?;
                }
                conjuncts.extend(
                    elems
                        .iter()
                        .enumerate()
                        .map(|(i, e)| {
                            let cell = encode_value(e, &env, ctx)?;
                            Ok(format!("{result_name}.{i} == {cell}"))
                        })
                        .collect::<Result<Vec<_>, RefEncodeError>>()?,
                );
            }
        }
    }
    if conjuncts.is_empty() {
        // The single-cell (scalar / bool / if-tail) body: the plain scalar equality.
        let reference = body_ref_state(block, ctx)?;
        if ctx.result_is_fixed_array {
            conjuncts.push(format!("{result_name}@ == ({reference})@"));
        } else {
            conjuncts.push(format!("{result_name} == {reference}"));
        }
    }
    Ok(conjuncts.join(" && "))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LifecycleCell {
    text: String,
    spec_int: bool,
}

impl LifecycleCell {
    fn bounded(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            spec_int: false,
        }
    }
}

#[derive(Clone, Default)]
struct LifecycleState {
    locals: BTreeMap<String, LifecycleCell>,
    readonly_inputs: BTreeSet<String>,
    fields: BTreeMap<String, LifecycleCell>,
    indexed: BTreeMap<String, LifecycleCell>,
}

fn aggregate_lifecycle_ensures(
    block: &Block,
    result_name: &str,
    ctx: &BodyRefCtx,
) -> Result<String, RefEncodeError> {
    let mut state = LifecycleState::default();
    for record in &ctx.mutable_records {
        for field in &record.fields {
            let key = format!("{}.{}", record.name, field.name);
            state.fields.insert(
                key,
                LifecycleCell::bounded(format!("old({}).{}", record.name, field.name)),
            );
        }
    }
    for record in &ctx.shared_records {
        for field in &record.fields {
            let key = format!("{}.{}", record.name, field.name);
            state.fields.insert(
                key,
                LifecycleCell::bounded(format!("{}.{}", record.name, field.name)),
            );
        }
    }
    for indexed in &ctx.shared_indexed {
        state.indexed.insert(
            indexed.name.clone(),
            LifecycleCell::bounded(format!("{}@", indexed.name)),
        );
    }
    for name in &ctx.mutable_indexed_bound {
        state.indexed.insert(
            name.clone(),
            LifecycleCell::bounded(format!("old({name})@")),
        );
    }
    thread_lifecycle_block(block, &mut state, ctx, false, &mut Vec::new())?;

    let result = match &block.tail {
        Some(tail) => encode_lifecycle_expr(tail, &state, ctx)?,
        None if ctx.result_is_unit => LifecycleCell::bounded("()"),
        None => {
            return Err(RefEncodeError::Unsupported(
                "tail-less aggregate lifecycle body requires an explicit unit result type"
                    .to_string(),
            ));
        }
    };
    let mut ensures = vec![if ctx.result_is_fixed_array {
        format!("{result_name}@ == ({})@", result.text)
    } else {
        format!("{result_name} == {}", result.text)
    }];
    for record in &ctx.mutable_records {
        for field in &record.fields {
            let key = format!("{}.{}", record.name, field.name);
            let value = state.fields.get(&key).ok_or_else(|| {
                RefEncodeError::Unsupported(format!(
                    "named-record lifecycle lost the modeled field `{key}`"
                ))
            })?;
            if field.array_view {
                ensures.push(format!(
                    "final({}).{}@ == ({})@",
                    record.name, field.name, value.text
                ));
            } else {
                ensures.push(format!(
                    "final({}).{} == {}",
                    record.name, field.name, value.text
                ));
            }
        }
    }
    for name in &ctx.mutable_indexed_bound {
        let value = state.indexed.get(name).ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "exclusive indexed lifecycle lost the modeled root `{name}`"
            ))
        })?;
        ensures.push(format!("final({name})@ == {}", value.text));
    }
    Ok(ensures.join(" && "))
}

fn thread_lifecycle_block(
    block: &Block,
    state: &mut LifecycleState,
    ctx: &BodyRefCtx,
    branch: bool,
    active_calls: &mut Vec<String>,
) -> Result<(), RefEncodeError> {
    for statement in &block.stmts {
        thread_lifecycle_stmt(statement, state, ctx, active_calls)?;
    }
    if branch && block.tail.is_some() {
        return Err(RefEncodeError::Unsupported(
            "an aggregate lifecycle `if` statement branch may mutate state but may not carry a discarded tail value"
                .to_string(),
        ));
    }
    Ok(())
}

fn thread_lifecycle_stmt(
    statement: &Stmt,
    state: &mut LifecycleState,
    ctx: &BodyRefCtx,
    active_calls: &mut Vec<String>,
) -> Result<(), RefEncodeError> {
    match statement {
        Stmt::Let { name, init, ty, .. } => {
            if state.locals.contains_key(name) {
                return Err(RefEncodeError::Unsupported(format!(
                    "re-shadowed lifecycle binding `{name}`"
                )));
            }
            let value = if let Some((callee, args)) = direct_mutable_call(init, ctx) {
                if ty.is_none() {
                    return Err(RefEncodeError::Unsupported(
                        "a mutable-reference call result requires one direct typed `let` initializer"
                            .to_string(),
                    ));
                }
                apply_mutable_call_effect(callee, args, state, ctx, active_calls)?
            } else {
                encode_lifecycle_expr(init, state, ctx)?
            };
            state.locals.insert(name.clone(), value);
            Ok(())
        }
        Stmt::Assign { target, value } => {
            let value = encode_lifecycle_expr(value, state, ctx)?;
            match target {
                Expr::Path(path) if path.len() == 1 && state.locals.contains_key(&path[0]) => {
                    if state.readonly_inputs.contains(&path[0]) {
                        return Err(RefEncodeError::Unsupported(format!(
                            "mutable-call callee assigns immutable value parameter `{}`",
                            path[0]
                        )));
                    }
                    state.locals.insert(path[0].clone(), value);
                    Ok(())
                }
                _ if target_contains_field(target) => {
                    let (root, steps) = nested_lvalue_path(target)?;
                    let Some((NestedLvalueStep::Field(direct_field), rest)) = steps.split_first()
                    else {
                        return Err(RefEncodeError::Unsupported(
                            "named-record lifecycle target must begin with one exact field"
                                .to_string(),
                        ));
                    };
                    let record = ctx.mutable_record(&root).ok_or_else(|| {
                        RefEncodeError::Unsupported(format!(
                            "named-record assignment root `{root}` is not an independently framed exclusive parameter"
                        ))
                    })?;
                    let field = record
                        .fields
                        .iter()
                        .find(|field| field.name == *direct_field)
                        .ok_or_else(|| {
                            RefEncodeError::Unsupported(format!(
                                "field `{root}.{direct_field}` is not in the independently declared mutable-record frame"
                            ))
                        })?;
                    let key = format!("{root}.{direct_field}");
                    if rest.is_empty() {
                        state.fields.insert(key, value);
                        return Ok(());
                    }

                    let field_ty = field.ty.as_ref().ok_or_else(|| {
                        RefEncodeError::Unsupported(format!(
                            "nested lifecycle field `{key}` has no independent source type"
                        ))
                    })?;
                    let current = state.fields.get(&key).ok_or_else(|| {
                        RefEncodeError::Unsupported(format!(
                            "named-record lifecycle lost the modeled field `{key}`"
                        ))
                    })?;
                    let current_name = "__thermite_nested_current";
                    let changed_name = "__thermite_nested_changed";
                    let updated = rebuild_nested_value(
                        Expr::Path(vec![current_name.to_string()]),
                        field_ty,
                        rest,
                        Expr::Path(vec![changed_name.to_string()]),
                        value.spec_int,
                        ctx,
                    )?;
                    let mut bindings = state
                        .locals
                        .iter()
                        .map(|(name, cell)| (name.clone(), cell.text.clone()))
                        .collect::<Vec<_>>();
                    bindings.push((current_name.to_string(), current.text.clone()));
                    bindings.push((changed_name.to_string(), value.text));
                    let exec = ctx
                        .exec_ref_ctx()
                        .with_value_bindings(bindings)
                        .with_field_bindings(
                            state
                                .fields
                                .iter()
                                .map(|(name, cell)| (name.clone(), cell.text.clone())),
                        );
                    state.fields.insert(
                        key,
                        LifecycleCell::bounded(exec_ref_value(&updated, &exec)?),
                    );
                    Ok(())
                }
                Expr::Index {
                    base,
                    index: IndexArg::Single(index),
                } => {
                    let Expr::Path(path) = base.as_ref() else {
                        return Err(RefEncodeError::Unsupported(
                            "exclusive indexed lifecycle target must have one direct root"
                                .to_string(),
                        ));
                    };
                    let [root] = path.as_slice() else {
                        return Err(RefEncodeError::Unsupported(
                            "exclusive indexed lifecycle target must have one direct root"
                                .to_string(),
                        ));
                    };
                    if !ctx.is_mutable_indexed_bound(root) {
                        return Err(RefEncodeError::Unsupported(format!(
                            "indexed assignment root `{root}` is not an independently framed exclusive slice or fixed array"
                        )));
                    }
                    let encoded_index = encode_lifecycle_expr(index, state, ctx)?;
                    let index = if matches!(index.as_ref(), Expr::IntLit { .. }) {
                        encoded_index.text
                    } else {
                        format!("({}) as int", encoded_index.text)
                    };
                    let current = state.indexed.get(root).ok_or_else(|| {
                        RefEncodeError::Unsupported(format!(
                            "exclusive indexed lifecycle lost the current sequence `{root}`"
                        ))
                    })?;
                    state.indexed.insert(
                        root.clone(),
                        LifecycleCell::bounded(format!(
                            "({}).update({index}, {})",
                            current.text, value.text
                        )),
                    );
                    Ok(())
                }
                Expr::Index { .. } => Err(RefEncodeError::Unsupported(
                    "exclusive indexed lifecycle supports one direct element index, not a range"
                        .to_string(),
                )),
                _ => Err(RefEncodeError::Unsupported(
                    "aggregate lifecycle assignment target is neither a framed direct field/indexed root nor an in-scope scalar local"
                        .to_string(),
                )),
            }
        }
        Stmt::If { cond, then, else_ } => {
            let condition = encode_lifecycle_expr(cond, state, ctx)?.text;
            let before = state.clone();
            let mut then_state = before.clone();
            thread_lifecycle_block(then, &mut then_state, ctx, true, active_calls)?;
            let mut else_state = before.clone();
            if let Some(else_) = else_ {
                thread_lifecycle_block(else_, &mut else_state, ctx, true, active_calls)?;
            }

            for (key, prior) in &before.locals {
                let left = then_state.locals.get(key).unwrap_or(prior);
                let right = else_state.locals.get(key).unwrap_or(prior);
                if left != prior || right != prior {
                    state
                        .locals
                        .insert(key.clone(), merge_lifecycle_cells(&condition, left, right));
                }
            }
            for (key, prior) in &before.fields {
                let left = then_state.fields.get(key).unwrap_or(prior);
                let right = else_state.fields.get(key).unwrap_or(prior);
                if left != prior || right != prior {
                    state
                        .fields
                        .insert(key.clone(), merge_lifecycle_cells(&condition, left, right));
                }
            }
            for (key, prior) in &before.indexed {
                let left = then_state.indexed.get(key).unwrap_or(prior);
                let right = else_state.indexed.get(key).unwrap_or(prior);
                if left != prior || right != prior {
                    state
                        .indexed
                        .insert(key.clone(), merge_lifecycle_cells(&condition, left, right));
                }
            }
            Ok(())
        }
        Stmt::Expr(expr) => {
            if let Expr::Call { callee, args } = expr {
                if let Expr::Path(path) = callee.as_ref() {
                    if let [name] = path.as_slice() {
                        if ctx.mutable_call_effect(name).is_some() {
                            let _ = apply_mutable_call_effect(
                                name,
                                args,
                                state,
                                ctx,
                                active_calls,
                            )?;
                            return Ok(());
                        }
                    }
                }
            }
            let _ = encode_lifecycle_expr(expr, state, ctx)?;
            Ok(())
        }
        Stmt::Return(_) => Err(RefEncodeError::Unsupported(
            "mid-body return is outside the single-exit aggregate lifecycle subset".to_string(),
        )),
        Stmt::Loop(_) | Stmt::Break | Stmt::Continue => Err(RefEncodeError::Unsupported(
            "loops and loop control over aggregate state require a separate invariant lifecycle model"
                .to_string(),
        )),
    }
}

/// Apply one reachable in-language mutable-reference call to the independent
/// lifecycle state. Direct actuals retain their existing implicit-reference
/// spelling (`callee(root)` where `root: &mut T`/`&T`). A projected finite-record
/// actual is an explicit source borrow (`callee(&mut outer.inner)` or
/// `callee(&outer.inner)`). Exclusive access paths are pairwise structurally
/// disjoint and may not overlap a shared actual: equal paths and every
/// ancestor/descendant pair overlap, while sibling fields do not. Shared/shared
/// aliasing is harmless and admitted. The callee source body is interpreted
/// recursively with its formal names rebound to the caller's current values;
/// production still executes the ordinary lowered call and is never replaced by
/// this model.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordCallActual {
    /// Root parameter followed by zero or more finite-record field projections.
    segments: Vec<String>,
    /// Exact nominal type at the root and after every projection. Therefore this
    /// has the same length as `segments` and its last item is the actual pointee.
    types: Vec<String>,
}

impl RecordCallActual {
    fn root(&self) -> &str {
        &self.segments[0]
    }

    fn type_name(&self) -> &str {
        self.types
            .last()
            .expect("a resolved record actual always has one type")
    }

    fn is_direct(&self) -> bool {
        self.segments.len() == 1
    }

    fn display(&self) -> String {
        self.segments.join(".")
    }
}

fn access_paths_overlap(left: &[String], right: &[String]) -> bool {
    left.iter()
        .zip(right)
        .all(|(left_segment, right_segment)| left_segment == right_segment)
}

fn record_actual_segments(
    argument: &Expr,
    mutable: bool,
    callee: &str,
    formal: &str,
) -> Result<Vec<String>, RefEncodeError> {
    if let Expr::Path(path) = argument {
        if path.len() == 1 {
            return Ok(path.clone());
        }
    }

    let Expr::Ref {
        mutable: actual_mutable,
        expr,
    } = argument
    else {
        let borrow = if mutable { "&mut" } else { "&" };
        return Err(RefEncodeError::Unsupported(format!(
            "{} record actual for `{callee}::{formal}` must be one direct borrowed root or an explicit `{borrow} root.field(.field)*` projection",
            if mutable { "exclusive" } else { "shared" },
        )));
    };
    if *actual_mutable != mutable {
        return Err(RefEncodeError::Unsupported(format!(
            "{} record formal `{callee}::{formal}` received a {} projected borrow",
            if mutable { "exclusive" } else { "shared" },
            if *actual_mutable { "mutable" } else { "shared" }
        )));
    }
    let (root, steps) = nested_lvalue_path(expr)?;
    if steps.is_empty() {
        return Err(RefEncodeError::Unsupported(format!(
            "direct record actual `{root}` for `{callee}::{formal}` is already borrowed; an explicit reference would change its type"
        )));
    }
    let mut segments = vec![root];
    for step in steps {
        match step {
            NestedLvalueStep::Field(field) => segments.push(field),
            NestedLvalueStep::Index(_) => {
                return Err(RefEncodeError::Unsupported(format!(
                    "projected record actual for `{callee}::{formal}` may contain only finite-record fields"
                )));
            }
        }
    }
    Ok(segments)
}

fn resolve_record_call_actual(
    argument: &Expr,
    mutable: bool,
    callee: &str,
    formal: &str,
    ctx: &BodyRefCtx,
) -> Result<RecordCallActual, RefEncodeError> {
    let segments = record_actual_segments(argument, mutable, callee, formal)?;
    let root = &segments[0];
    let root_type = if mutable {
        ctx.mutable_record(root)
            .and_then(|record| record.type_name.as_deref())
    } else if let Some(record) = ctx.shared_record(root) {
        Some(record.type_name.as_str())
    } else {
        ctx.mutable_record(root)
            .and_then(|record| record.type_name.as_deref())
    }
    .ok_or_else(|| {
        RefEncodeError::Unsupported(format!(
            "{} record actual root `{root}` for `{callee}::{formal}` has no exact independently framed nominal type",
            if mutable { "exclusive" } else { "shared" }
        ))
    })?;

    let mut types = vec![root_type.to_string()];
    let mut current_type = root_type;
    for field_name in &segments[1..] {
        let record = ctx.named_record(current_type).ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "projected record actual `{}` for `{callee}::{formal}` crosses non-finite record `{current_type}`",
                segments.join(".")
            ))
        })?;
        let field = record
            .fields
            .iter()
            .find(|field| field.name == *field_name)
            .ok_or_else(|| {
                RefEncodeError::Unsupported(format!(
                    "projected record actual `{}` for `{callee}::{formal}` names unknown field `{current_type}.{field_name}`",
                    segments.join(".")
                ))
            })?;
        let Some(Type::Named(next_type)) = field.ty.as_ref() else {
            return Err(RefEncodeError::Unsupported(format!(
                "projected record actual `{}` for `{callee}::{formal}` ends or crosses non-record field `{current_type}.{field_name}`",
                segments.join(".")
            )));
        };
        types.push(next_type.clone());
        current_type = next_type;
    }

    Ok(RecordCallActual { segments, types })
}

fn projected_record_value(
    actual: &RecordCallActual,
    state: &LifecycleState,
    callee: &str,
) -> Result<LifecycleCell, RefEncodeError> {
    debug_assert!(!actual.is_direct());
    let top_key = format!("{}.{}", actual.root(), actual.segments[1]);
    let mut value = state.fields.get(&top_key).cloned().ok_or_else(|| {
        RefEncodeError::Unsupported(format!(
            "mutable-reference call `{callee}` cannot observe exact caller projection `{top_key}`"
        ))
    })?;
    for field in &actual.segments[2..] {
        value = LifecycleCell::bounded(format!("({}).{field}", value.text));
    }
    Ok(value)
}

fn snapshot_record_actual(
    formal: &MutableRecordFrame,
    actual: &RecordCallActual,
    state: &LifecycleState,
    callee: &str,
) -> Result<Vec<(String, LifecycleCell)>, RefEncodeError> {
    if actual.is_direct() {
        return formal
            .fields
            .iter()
            .map(|field| {
                let actual_key = format!("{}.{}", actual.root(), field.name);
                state
                    .fields
                    .get(&actual_key)
                    .cloned()
                    .map(|value| (field.name.clone(), value))
                    .ok_or_else(|| {
                        RefEncodeError::Unsupported(format!(
                            "mutable-reference call `{callee}` cannot observe exact caller field `{actual_key}`"
                        ))
                    })
            })
            .collect();
    }

    let record = projected_record_value(actual, state, callee)?;
    Ok(formal
        .fields
        .iter()
        .map(|field| {
            (
                field.name.clone(),
                LifecycleCell::bounded(format!("({}).{}", record.text, field.name)),
            )
        })
        .collect())
}

fn snapshot_shared_record_actual(
    formal: &SharedRecordFrame,
    actual: &RecordCallActual,
    state: &LifecycleState,
    callee: &str,
) -> Result<Vec<(String, LifecycleCell)>, RefEncodeError> {
    let mutable = MutableRecordFrame::typed(
        formal.name.clone(),
        formal.type_name.clone(),
        formal.fields.clone(),
    );
    snapshot_record_actual(&mutable, actual, state, callee)
}

fn record_post_value(
    formal: &MutableRecordFrame,
    callee_state: &LifecycleState,
    callee: &str,
) -> Result<String, RefEncodeError> {
    let type_name = formal.type_name.as_deref().ok_or_else(|| {
        RefEncodeError::Unsupported(format!(
            "mutable-reference callee `{callee}` lost the exact nominal type of `{}`",
            formal.name
        ))
    })?;
    let fields = formal
        .fields
        .iter()
        .map(|field| {
            let formal_key = format!("{}.{}", formal.name, field.name);
            callee_state
                .fields
                .get(&formal_key)
                .map(|value| format!("{}: {}", field.name, value.text))
                .ok_or_else(|| {
                    RefEncodeError::Unsupported(format!(
                        "mutable-reference callee `{callee}` lost exact post-state field `{formal_key}`"
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("{type_name} {{ {} }}", fields.join(", ")))
}

fn rebuild_projected_record_text(
    current: String,
    current_type: &str,
    fields: &[String],
    replacement: &str,
    ctx: &BodyRefCtx,
) -> Result<String, RefEncodeError> {
    let Some((changed_field, rest)) = fields.split_first() else {
        return Ok(replacement.to_string());
    };
    let record = ctx.named_record(current_type).ok_or_else(|| {
        RefEncodeError::Unsupported(format!(
            "projected call copy-back lost finite record declaration `{current_type}`"
        ))
    })?;
    let changed = record
        .fields
        .iter()
        .find(|field| field.name == *changed_field)
        .ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "projected call copy-back lost field `{current_type}.{changed_field}`"
            ))
        })?;
    let next = if rest.is_empty() {
        replacement.to_string()
    } else {
        let Some(Type::Named(next_type)) = changed.ty.as_ref() else {
            return Err(RefEncodeError::Unsupported(format!(
                "projected call copy-back crosses non-record field `{current_type}.{changed_field}`"
            )));
        };
        rebuild_projected_record_text(
            format!("({current}).{changed_field}"),
            next_type,
            rest,
            replacement,
            ctx,
        )?
    };
    Ok(format!(
        "{current_type} {{ {} }}",
        record
            .fields
            .iter()
            .map(|field| {
                if field.name == *changed_field {
                    format!("{}: {next}", field.name)
                } else {
                    format!("{}: ({current}).{}", field.name, field.name)
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn copy_back_record_actual(
    formal: &MutableRecordFrame,
    actual: &RecordCallActual,
    callee_state: &LifecycleState,
    state: &mut LifecycleState,
    callee: &str,
    ctx: &BodyRefCtx,
) -> Result<(), RefEncodeError> {
    if actual.is_direct() {
        for field in &formal.fields {
            let formal_key = format!("{}.{}", formal.name, field.name);
            let value = callee_state
                .fields
                .get(&formal_key)
                .cloned()
                .ok_or_else(|| {
                    RefEncodeError::Unsupported(format!(
                        "mutable-reference callee `{callee}` lost exact post-state field `{formal_key}`"
                    ))
                })?;
            state
                .fields
                .insert(format!("{}.{}", actual.root(), field.name), value);
        }
        return Ok(());
    }

    let replacement = record_post_value(formal, callee_state, callee)?;
    let top_key = format!("{}.{}", actual.root(), actual.segments[1]);
    let current = state.fields.get(&top_key).cloned().ok_or_else(|| {
        RefEncodeError::Unsupported(format!(
            "projected call copy-back cannot observe enclosing caller field `{top_key}`"
        ))
    })?;
    let rebuilt = if actual.segments.len() == 2 {
        replacement
    } else {
        rebuild_projected_record_text(
            current.text,
            &actual.types[1],
            &actual.segments[2..],
            &replacement,
            ctx,
        )?
    };
    state
        .fields
        .insert(top_key, LifecycleCell::bounded(format!("({rebuilt})")));
    Ok(())
}

fn apply_mutable_call_effect(
    name: &str,
    args: &[Expr],
    state: &mut LifecycleState,
    ctx: &BodyRefCtx,
    active_calls: &mut Vec<String>,
) -> Result<LifecycleCell, RefEncodeError> {
    if active_calls.iter().any(|active| active == name) {
        return Err(RefEncodeError::Unsupported(format!(
            "recursive mutable-reference effect cycle reaches `{name}`"
        )));
    }
    let effect = ctx.mutable_call_effect(name).cloned().ok_or_else(|| {
        RefEncodeError::Unsupported(format!(
            "mutable-reference call `{name}` has no exact in-language effect frame"
        ))
    })?;
    if effect.params.len() != args.len() {
        return Err(RefEncodeError::Unsupported(format!(
            "mutable-reference call `{name}` has {} actual arguments but {} exact formals",
            args.len(),
            effect.params.len()
        )));
    }
    if effect.mutable_records.is_empty() && effect.mutable_indexed.is_empty() {
        return Err(RefEncodeError::Unsupported(format!(
            "mutable-reference call `{name}` has no admitted finite-record or indexed-storage formal"
        )));
    }
    let formal_positions = effect
        .params
        .iter()
        .enumerate()
        .map(|(index, formal)| (formal.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut mutable_actual_roots = BTreeMap::<String, RecordCallActual>::new();
    let mut exclusive_paths = Vec::<Vec<String>>::new();
    for formal in &effect.mutable_records {
        let position = formal_positions.get(formal.name.as_str()).ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "mutable-reference callee `{name}` lost formal `{}`",
                formal.name
            ))
        })?;
        let actual = resolve_record_call_actual(&args[*position], true, name, &formal.name, ctx)?;
        if let Some(overlap) = exclusive_paths
            .iter()
            .find(|path| access_paths_overlap(path, &actual.segments))
        {
            return Err(RefEncodeError::Unsupported(format!(
                "mutable-reference call `{name}` aliases exclusive access paths `{}` and `{}`",
                overlap.join("."),
                actual.display()
            )));
        }
        exclusive_paths.push(actual.segments.clone());
        match &formal.type_name {
            Some(expected) if expected == actual.type_name() => {}
            Some(expected) => {
                return Err(RefEncodeError::Unsupported(format!(
                    "mutable-reference call `{name}` expects `{expected}` for `{}` but actual `{}` has `{}`",
                    formal.name,
                    actual.display(),
                    actual.type_name()
                )));
            }
            None => {
                return Err(RefEncodeError::Unsupported(format!(
                    "mutable-reference call `{name}` lacks exact nominal type metadata for formal `{}`",
                    formal.name
                )));
            }
        }
        mutable_actual_roots.insert(formal.name.clone(), actual);
    }

    let mut indexed_actual_roots = BTreeMap::<String, String>::new();
    for formal in &effect.mutable_indexed {
        let position = formal_positions.get(formal.name.as_str()).ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "mutable indexed callee `{name}` lost formal `{}`",
                formal.name
            ))
        })?;
        let root = match &args[*position] {
            Expr::Path(path) if path.len() == 1 => path[0].clone(),
            _ => {
                return Err(RefEncodeError::Unsupported(format!(
                    "mutable indexed actual for `{name}::{}` must be one direct exclusive root",
                    formal.name
                )));
            }
        };
        let path = vec![root.clone()];
        if let Some(overlap) = exclusive_paths
            .iter()
            .find(|existing| access_paths_overlap(existing, &path))
        {
            return Err(RefEncodeError::Unsupported(format!(
                "mutable-reference call `{name}` aliases exclusive access paths `{}` and `{root}` across record/indexed formals",
                overlap.join(".")
            )));
        }
        exclusive_paths.push(path);
        let actual = ctx.mutable_indexed(&root).ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "mutable indexed actual `{root}` for `{name}` is not an independently framed exclusive slice or fixed array"
            ))
        })?;
        if formal.pointee != actual.pointee {
            return Err(RefEncodeError::Unsupported(format!(
                "mutable indexed call `{name}` has an exact pointee-type mismatch for formal `{}` and actual `{root}`",
                formal.name
            )));
        }
        indexed_actual_roots.insert(formal.name.clone(), root);
    }

    let mut shared_actual_roots = BTreeMap::<String, RecordCallActual>::new();
    for formal in &effect.shared_records {
        let position = formal_positions.get(formal.name.as_str()).ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "mixed-reference callee `{name}` lost shared formal `{}`",
                formal.name
            ))
        })?;
        let actual = resolve_record_call_actual(&args[*position], false, name, &formal.name, ctx)?;
        if let Some(overlap) = exclusive_paths
            .iter()
            .find(|path| access_paths_overlap(path, &actual.segments))
        {
            return Err(RefEncodeError::Unsupported(format!(
                "mixed-reference call `{name}` aliases exclusive access path `{}` through shared actual `{}` for formal `{}`",
                overlap.join("."),
                actual.display(),
                formal.name,
            )));
        }
        if formal.type_name != actual.type_name() {
            return Err(RefEncodeError::Unsupported(format!(
                "shared-reference call `{name}` expects `{}` for `{}` but actual `{}` has `{}`",
                formal.type_name,
                formal.name,
                actual.display(),
                actual.type_name(),
            )));
        }
        shared_actual_roots.insert(formal.name.clone(), actual);
    }

    let mut shared_indexed_actual_roots = BTreeMap::<String, String>::new();
    for formal in &effect.shared_indexed {
        let position = formal_positions.get(formal.name.as_str()).ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "mixed-reference callee `{name}` lost shared indexed formal `{}`",
                formal.name
            ))
        })?;
        let root = match &args[*position] {
            Expr::Path(path) if path.len() == 1 => path[0].clone(),
            _ => {
                return Err(RefEncodeError::Unsupported(format!(
                    "shared indexed actual for `{name}::{}` must be one direct slice or fixed-array root",
                    formal.name
                )));
            }
        };
        let path = vec![root.clone()];
        if let Some(overlap) = exclusive_paths
            .iter()
            .find(|existing| access_paths_overlap(existing, &path))
        {
            return Err(RefEncodeError::Unsupported(format!(
                "mixed-reference call `{name}` aliases exclusive access path `{}` through shared indexed root `{root}` for formal `{}`",
                overlap.join("."),
                formal.name,
            )));
        }
        let found = if let Some(actual) = ctx.shared_indexed(&root) {
            &actual.pointee
        } else if let Some(actual) = ctx.mutable_indexed(&root) {
            &actual.pointee
        } else {
            return Err(RefEncodeError::Unsupported(format!(
                "shared indexed actual `{root}` for `{name}` is not an independently framed slice or fixed array"
            )));
        };
        if formal.pointee != *found {
            return Err(RefEncodeError::Unsupported(format!(
                "shared indexed call `{name}` has an exact pointee-type mismatch for formal `{}` and actual `{root}`",
                formal.name
            )));
        }
        shared_indexed_actual_roots.insert(formal.name.clone(), root);
    }

    let mut callee_state = LifecycleState::default();
    for (position, formal_name) in effect.params.iter().enumerate() {
        if effect
            .mutable_records
            .iter()
            .any(|record| record.name == *formal_name)
            || effect
                .mutable_indexed
                .iter()
                .any(|indexed| indexed.name == *formal_name)
            || effect
                .shared_records
                .iter()
                .any(|record| record.name == *formal_name)
            || effect
                .shared_indexed
                .iter()
                .any(|indexed| indexed.name == *formal_name)
        {
            continue;
        }
        let value = encode_lifecycle_expr(&args[position], state, ctx)?;
        callee_state.locals.insert(formal_name.clone(), value);
        callee_state.readonly_inputs.insert(formal_name.clone());
    }
    for formal in &effect.mutable_records {
        let actual = mutable_actual_roots.get(&formal.name).ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "mutable-reference callee `{name}` lost actual root for formal `{}`",
                formal.name
            ))
        })?;
        for (field, value) in snapshot_record_actual(formal, actual, state, name)? {
            callee_state
                .fields
                .insert(format!("{}.{}", formal.name, field), value);
        }
    }
    for formal in &effect.shared_records {
        let actual = shared_actual_roots.get(&formal.name).ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "mixed-reference callee `{name}` lost actual shared root for formal `{}`",
                formal.name
            ))
        })?;
        for (field, value) in snapshot_shared_record_actual(formal, actual, state, name)? {
            callee_state
                .fields
                .insert(format!("{}.{}", formal.name, field), value);
        }
    }
    for formal in &effect.mutable_indexed {
        let actual_root = indexed_actual_roots.get(&formal.name).ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "mutable indexed callee `{name}` lost actual root for formal `{}`",
                formal.name
            ))
        })?;
        let value = state.indexed.get(actual_root).cloned().ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "mutable indexed call `{name}` cannot observe exact caller sequence `{actual_root}`"
            ))
        })?;
        callee_state.indexed.insert(formal.name.clone(), value);
    }
    for formal in &effect.shared_indexed {
        let actual_root = shared_indexed_actual_roots
            .get(&formal.name)
            .ok_or_else(|| {
                RefEncodeError::Unsupported(format!(
                    "mixed-reference callee `{name}` lost actual shared indexed root for formal `{}`",
                    formal.name
                ))
            })?;
        let value = state.indexed.get(actual_root).cloned().ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "mixed-reference call `{name}` cannot snapshot exact caller sequence `{actual_root}`"
            ))
        })?;
        callee_state.indexed.insert(formal.name.clone(), value);
    }

    let mut callee_ctx = ctx.clone();
    callee_ctx.mutable_records = effect.mutable_records.clone();
    callee_ctx.mutable_indexed = effect.mutable_indexed.clone();
    callee_ctx.mutable_indexed_bound = effect
        .mutable_indexed
        .iter()
        .map(|indexed| indexed.name.clone())
        .collect();
    callee_ctx.shared_indexed = effect.shared_indexed.clone();
    callee_ctx.slice_bound = effect
        .mutable_indexed
        .iter()
        .map(|indexed| (&indexed.name, &indexed.pointee))
        .chain(
            effect
                .shared_indexed
                .iter()
                .map(|indexed| (&indexed.name, &indexed.pointee)),
        )
        .filter(|(_, pointee)| matches!(pointee, Type::Slice(_)))
        .map(|(name, _)| name.clone())
        .collect();
    callee_ctx.fixed_array_bound = effect
        .mutable_indexed
        .iter()
        .map(|indexed| (&indexed.name, &indexed.pointee))
        .chain(
            effect
                .shared_indexed
                .iter()
                .map(|indexed| (&indexed.name, &indexed.pointee)),
        )
        .filter(|(_, pointee)| matches!(pointee, Type::Array { .. }))
        .map(|(name, _)| name.clone())
        .collect();
    callee_ctx.shared_records = effect.shared_records.clone();
    active_calls.push(name.to_string());
    let interpreted = thread_lifecycle_block(
        &effect.body,
        &mut callee_state,
        &callee_ctx,
        false,
        active_calls,
    );
    active_calls.pop();
    interpreted?;
    // Interpret the callee's exact source result under its post-state. A
    // statement-position caller may discard this cell; a direct `let` caller
    // binds it. Either way, an unsupported or effectful tail cannot disappear.
    let result = match &effect.body.tail {
        Some(tail) => encode_lifecycle_expr(tail, &callee_state, &callee_ctx)?,
        None => LifecycleCell::bounded("()"),
    };

    for formal in &effect.mutable_records {
        let actual = mutable_actual_roots.get(&formal.name).ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "mutable-reference callee `{name}` lost actual root for formal `{}`",
                formal.name
            ))
        })?;
        copy_back_record_actual(formal, actual, &callee_state, state, name, ctx)?;
    }
    for formal in &effect.mutable_indexed {
        let actual_root = indexed_actual_roots.get(&formal.name).ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "mutable indexed callee `{name}` lost actual root for formal `{}`",
                formal.name
            ))
        })?;
        let value = callee_state
            .indexed
            .get(&formal.name)
            .cloned()
            .ok_or_else(|| {
                RefEncodeError::Unsupported(format!(
                    "mutable indexed callee `{name}` lost exact post-state sequence `{}`",
                    formal.name
                ))
            })?;
        state.indexed.insert(actual_root.clone(), value);
    }
    Ok(result)
}

/// Recognize the sole admitted result-consuming form: a bare call to a framed
/// in-language mutable callee used directly as a `let` initializer. Nested uses
/// remain fail-closed until expression evaluation order and borrow aliasing are
/// independently modeled.
fn direct_mutable_call<'a>(expr: &'a Expr, ctx: &BodyRefCtx) -> Option<(&'a str, &'a [Expr])> {
    let Expr::Call { callee, args } = expr else {
        return None;
    };
    let Expr::Path(path) = callee.as_ref() else {
        return None;
    };
    let [name] = path.as_slice() else {
        return None;
    };
    ctx.mutable_call_effect(name)
        .is_some()
        .then_some((name.as_str(), args.as_slice()))
}

fn merge_lifecycle_cells(
    condition: &str,
    left: &LifecycleCell,
    right: &LifecycleCell,
) -> LifecycleCell {
    let (left_text, right_text, spec_int) = match (left.spec_int, right.spec_int) {
        (true, false) => (left.text.clone(), format!("({} as int)", right.text), true),
        (false, true) => (format!("({} as int)", left.text), right.text.clone(), true),
        (left_int, _) => (left.text.clone(), right.text.clone(), left_int),
    };
    LifecycleCell {
        text: format!("if {condition} {{ {left_text} }} else {{ {right_text} }}"),
        spec_int,
    }
}

fn encode_lifecycle_expr(
    expr: &Expr,
    state: &LifecycleState,
    ctx: &BodyRefCtx,
) -> Result<LifecycleCell, RefEncodeError> {
    if expr_contains_mutable_call(expr, ctx) {
        return Err(RefEncodeError::Unsupported(
            "a mutable-reference call result may only be consumed as one direct `let` initializer; nested expression, condition, argument, assignment, and tail uses require a wider evaluation-order and alias frame"
                .to_string(),
        ));
    }
    let exec = ctx
        .exec_ref_ctx()
        .with_value_bindings(
            state
                .locals
                .iter()
                .map(|(name, cell)| (name.clone(), cell.text.clone())),
        )
        .with_field_bindings(
            state
                .fields
                .iter()
                .map(|(name, cell)| (name.clone(), cell.text.clone())),
        )
        .with_indexed_bindings(
            state
                .indexed
                .iter()
                .map(|(name, cell)| (name.clone(), cell.text.clone())),
        );
    let text = exec_ref_value(expr, &exec)?;
    let spec_int = match expr {
        Expr::Path(path) if path.len() == 1 => {
            state.locals.get(&path[0]).is_some_and(|cell| cell.spec_int)
        }
        Expr::Field { receiver, name } => {
            if let Expr::Path(path) = receiver.as_ref() {
                if let [root] = path.as_slice() {
                    state
                        .fields
                        .get(&format!("{root}.{name}"))
                        .is_some_and(|cell| cell.spec_int)
                } else {
                    false
                }
            } else {
                false
            }
        }
        Expr::Binary {
            op:
                BinOp::Add
                | BinOp::Sub
                | BinOp::Mul
                | BinOp::Div
                | BinOp::Rem
                | BinOp::Shl
                | BinOp::Shr
                | BinOp::BitAnd
                | BinOp::BitOr
                | BinOp::BitXor,
            ..
        } => true,
        _ => false,
    };
    Ok(LifecycleCell { text, spec_int })
}

fn expr_contains_mutable_call(expr: &Expr, ctx: &BodyRefCtx) -> bool {
    let any = |items: &[Expr]| {
        items
            .iter()
            .any(|item| expr_contains_mutable_call(item, ctx))
    };
    match expr {
        Expr::Path(_) | Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::StrLit(_) => false,
        Expr::Array(items) | Expr::Tuple(items) => any(items),
        Expr::ArrayRepeat { value, .. }
        | Expr::Unary { expr: value, .. }
        | Expr::Cast { expr: value, .. }
        | Expr::Ref { expr: value, .. }
        | Expr::Deref(value)
        | Expr::TupleProj {
            receiver: value, ..
        }
        | Expr::Field {
            receiver: value, ..
        }
        | Expr::Closure { body: value, .. }
        | Expr::Is {
            scrutinee: value, ..
        } => expr_contains_mutable_call(value, ctx),
        Expr::Binary { lhs, rhs, .. } => {
            expr_contains_mutable_call(lhs, ctx) || expr_contains_mutable_call(rhs, ctx)
        }
        Expr::Call { callee, args } => {
            direct_mutable_call(expr, ctx).is_some()
                || expr_contains_mutable_call(callee, ctx)
                || any(args)
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_contains_mutable_call(receiver, ctx) || any(args)
        }
        Expr::Match { scrutinee, arms } => {
            expr_contains_mutable_call(scrutinee, ctx)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|guard| expr_contains_mutable_call(guard, ctx))
                        || expr_contains_mutable_call(&arm.body, ctx)
                })
        }
        Expr::If { cond, then, else_ } => {
            expr_contains_mutable_call(cond, ctx)
                || block_contains_mutable_call(then, ctx)
                || block_contains_mutable_call(else_, ctx)
        }
        Expr::Index { base, index } => {
            expr_contains_mutable_call(base, ctx)
                || match index {
                    IndexArg::Single(index)
                    | IndexArg::RangeTo(index)
                    | IndexArg::RangeFrom(index) => expr_contains_mutable_call(index, ctx),
                    IndexArg::Range(start, end) => {
                        expr_contains_mutable_call(start, ctx)
                            || expr_contains_mutable_call(end, ctx)
                    }
                }
        }
        Expr::StructLit { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_contains_mutable_call(value, ctx)),
        Expr::Quantifier { domain, body, .. } => {
            expr_contains_mutable_call(domain, ctx) || expr_contains_mutable_call(body, ctx)
        }
    }
}

fn block_contains_mutable_call(block: &Block, ctx: &BodyRefCtx) -> bool {
    block
        .stmts
        .iter()
        .any(|statement| stmt_contains_mutable_call(statement, ctx))
        || block
            .tail
            .as_deref()
            .is_some_and(|tail| expr_contains_mutable_call(tail, ctx))
}

fn stmt_contains_mutable_call(statement: &Stmt, ctx: &BodyRefCtx) -> bool {
    match statement {
        Stmt::Let { init, .. } | Stmt::Expr(init) => expr_contains_mutable_call(init, ctx),
        Stmt::Assign { target, value } => {
            expr_contains_mutable_call(target, ctx) || expr_contains_mutable_call(value, ctx)
        }
        Stmt::Return(value) => value
            .as_ref()
            .is_some_and(|value| expr_contains_mutable_call(value, ctx)),
        Stmt::If { cond, then, else_ } => {
            expr_contains_mutable_call(cond, ctx)
                || block_contains_mutable_call(then, ctx)
                || else_
                    .as_ref()
                    .is_some_and(|block| block_contains_mutable_call(block, ctx))
        }
        Stmt::Loop(loop_node) => {
            let condition = match &loop_node.kind {
                LoopKind::While(condition) => expr_contains_mutable_call(condition, ctx),
                LoopKind::Loop => false,
            };
            condition || block_contains_mutable_call(&loop_node.body, ctx)
        }
        Stmt::Break | Stmt::Continue => false,
    }
}

// =============================================================================
// Loop extension — step 2.2.2-i (`.design/verified/loop-tv.md` REQ-1/REQ-2)
// =============================================================================

/// The loop reference pieces a frozen-subset `while` loop produces
/// (`loop-tv.md` REQ-2), consumed by the obligation emitters in [`crate::obligation`]
/// to build the three Z3-checkable Verus units (entry / preservation / exit). Each
/// field is an independent reference encoding (composing [`exec_ref_value`] on the
/// env-substituted cond / inv / cell exprs — no `thermite-lower` symbol, AC-7).
///
/// The loop is the v1 frozen subset: a single `while <cond>` with non-empty `invs` +
/// a `dec`, a straight-line scalar or finite-record body, and a trailing loop.
/// The mutated cells are the bare scalar names the body rebinds (the design's `lo`/
/// `hi`); they are bound as the loop-step parameters in the preservation obligation
/// and as the opaque-but-invariant-constrained after-loop cells in the exit
/// obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopObligations {
    /// The mutated scalar or finite-record cell names, in a stable
    /// (sorted) order — the order they are declared/projected in the obligations.
    pub cells: Vec<String>,
    /// Entry (`loop-tv.md` REQ-2.1): the conjoined `inv` with each cell substituted
    /// by its pre-loop entry value (the prefix straight-line state, encoded by the
    /// shipped [`body_ref_state`] threading). The entry obligation asserts this
    /// predicate holds (`entry-state ⟹ inv`).
    pub entry_pred: String,
    /// The loop condition `<cond>` over the free cells (cells as the loop-step
    /// params) — the preservation obligation's `requires inv && cond` guard and the
    /// negated `!(cond)` in the exit characterization.
    pub cond: String,
    /// The conjoined loop `inv` over the free cells — the preservation obligation's
    /// `requires inv` guard and (negated-cond conjunct) the exit characterization.
    pub inv: String,
    /// Preservation (`loop-tv.md` REQ-2.2): per-cell, the single-iteration stepped
    /// value — the shipped [`body_ref_state`] step of the loop body (the loop body
    /// is a straight-line `Block`). `step_cells[i]` is the closed form cell `i` holds
    /// after one iteration, as a function of the entry cells. The obligation's
    /// `ensures` is `result.i == step_cells[i]` (the body-TV reuse, AC-5) and the inv
    /// at the stepped state (the preservation conjunct, AC-2).
    pub step_cells: Vec<String>,
    /// Preservation: the conjoined `inv` with each cell substituted by its stepped
    /// value (`step_cells`) — the obligation's preservation `ensures` conjunct
    /// (`inv ∧ cond` carried to `inv` by one iteration).
    pub inv_at_step: String,
    /// Exact full-loop result characterization when the tail returns the sole
    /// loop cell.  The cell is replaced by `result`, so this predicate states
    /// the independently encoded invariant and negated condition over the
    /// generated function's actual return value. Exact collateral framing is
    /// explicit in the invariant, while preservation observes every record leaf.
    pub exit_result_pred: Option<String>,
}

/// Recognize + encode the v1 frozen-subset `while` loop in `block`, producing the
/// independent reference pieces ([`LoopObligations`]) the [`crate::obligation`]
/// emitters turn into Z3-checkable Verus units (`loop-tv.md` REQ-1/REQ-2). The loop
/// must be the last statement before the tail (the `binary_search` shape — v1's
/// after-loop continuation is in scope only there); a prefix of straight-line
/// statements establishes the pre-loop entry state.
///
/// Reuses the shipped [`body_ref_state`] threading for both the pre-loop entry state
/// and the single-iteration body step (the loop body is itself a straight-line
/// `Block`); the cond / inv value encoding reuses [`exec_ref_value`] on the
/// env-substituted [`Expr`] (independence preserved — no `thermite-lower` dep).
///
/// Returns [`RefEncodeError::Unsupported`] (never a panic / silent wrong encoding,
/// R-HONEST-3) for an out-of-v1 loop: a `loop`-kind (multi-exit), a `break`/
/// `continue` / mid-body `return` in the body (multi-exit CPS), a nested loop, a
/// non-finite/aliased state body, or a trivially-weak `inv` (`inv true` — the after-loop
/// `true ∧ ¬cond` is vacuous, cannot enter the (a) rule). Each is Skipped,
/// never silently Faithful (the 2.2.2 boundary in the certificate).
pub fn loop_ref_obligations(
    block: &Block,
    ctx: &BodyRefCtx,
) -> Result<LoopObligations, RefEncodeError> {
    let (prefix, loop_node) = recognize_v1_loop(block, ctx)?;
    let cond_expr = match &loop_node.kind {
        LoopKind::While(c) => c.as_ref(),
        LoopKind::Loop => {
            // Unreachable: recognize_v1_loop rejects the `loop`-kind. Kept exhaustive
            // (no `_`/panic, R-APG-1).
            return Err(RefEncodeError::Unsupported(
                "`loop`-kind (the infinite-loop form is a multi-exit CPS shape — OUT \
                 of the v1 single-`while` subset, Skipped honestly)"
                    .to_string(),
            ));
        }
    };

    // A trivially-weak `inv` (the conjunction is the bare `true`) is out of v1 (the
    // after-loop `true ∧ ¬cond` is vacuous), checked before encoding.
    if invariant_is_vacuous(&loop_node.invs) {
        return Err(RefEncodeError::Unsupported(
            "trivially-weak loop invariant (`inv true` — the after-loop `true ∧ ¬cond` \
             characterization is vacuous; the loop cannot enter the (a) rule, Skipped \
             honestly — bounded unrolling is the future v0.2 L2 fallback)"
                .to_string(),
        ));
    }

    // The mutated cells: bare scalar rebinds and roots of exact finite-record
    // field writes. `recognize_v1_loop` plus `thread_stmt` reject every target
    // whose type/path cannot be derived independently. Sorted for a stable
    // declaration/projection order across the three obligations.
    let mut cells: Vec<String> = collect_assigned_cells(&loop_node.body)?
        .into_iter()
        .collect();
    cells.sort();
    if cells.is_empty() {
        return Err(RefEncodeError::Unsupported(
            "v1 `while` loop with no state-cell mutation (a loop whose body mutates \
             no in-scope scalar or finite-record cell carries no per-iteration state step — OUT of the \
             v1 subset)"
                .to_string(),
        ));
    }

    // The entry state: thread the pre-loop prefix straight-line statements (the
    // `let mut lo = 0; let mut hi = haystack.len();` prefix) into the entry env: the
    // closed-form value of every prefix-introduced binding (cells and read-only
    // in-scope bindings the inv/cond reference) in the fn inputs. The entry invariant
    // substitutes the whole entry env (so a referenced `hi |-> n` is resolved, not
    // left free); the fn inputs are the only surviving free vars. Every cell must have
    // a prefix `let mut` binding (an assigned cell needs an in-scope introducer);
    // Return Err otherwise.
    let mut entry_env: Env = Env::new();
    for stmt in prefix {
        thread_stmt(stmt, &mut entry_env, ctx)?;
    }
    for cell in &cells {
        if !entry_env.contains_key(cell) {
            return Err(RefEncodeError::Unsupported(format!(
                "loop cell `{cell}` has no pre-loop `let mut` binding in the straight-\
                 line prefix (malformed v1 loop — the entry state is undefined)"
            )));
        }
    }
    let entry_subst = entry_env;

    // The stepped state: thread one iteration of the loop body (the shipped
    // straight-line state-transformer, the loop body is a straight-line Block). Each
    // cell starts free (bound to itself — the loop-step's param), so the stepped
    // value is a closed form in the entry cells. A cell the body does not rebind keeps
    // its identity binding (unchanged across the iteration).
    let mut step_env: Env = Env::new();
    for cell in &cells {
        step_env.insert(cell.clone(), Expr::Path(vec![cell.clone()]));
        if let Some(type_name) = entry_subst.named_record_type(cell) {
            step_env.mark_named_record(cell.clone(), type_name.to_string());
        }
        if entry_subst.is_fixed_array(cell) {
            step_env.mark_fixed_array(cell.clone());
        }
    }
    for stmt in &loop_node.body.stmts {
        thread_stmt(stmt, &mut step_env, ctx)?;
    }
    // The per-cell stepped closed form (in the entry cells) + the cell→stepped-form
    // substitution env — both read from `step_env`, where every `cell` is present (it
    // was seeded with its identity binding above, never removed by threading); an
    // absent key is handled as the identity, no panic.
    let mut step_subst: Env = Env::new();
    let mut step_cells: Vec<String> = Vec::with_capacity(cells.len());
    for cell in &cells {
        let stepped = match step_env.get(cell) {
            Some(expr) => expr.clone(),
            None => Expr::Path(vec![cell.clone()]),
        };
        step_cells.push(encode_value(&stepped, &Env::new(), ctx)?);
        step_subst.insert(cell.clone(), stepped);
    }

    // The condition + the invariant over the free cells (encoded as bool-valued exec
    // predicates — reuse exec_ref_value on the env-substituted cond / inv).
    let cond = encode_predicate(cond_expr, &Env::new(), ctx)?;
    let inv = encode_inv_clauses(&loop_node.invs, &Env::new(), ctx)?.join(" && ");

    // The entry-substituted invariant (cells → entry values) and the step-substituted
    // invariant (cells → stepped values) — reuse the same inv-clause encoder under the
    // substitution env.
    let entry_pred = encode_inv_clauses(&loop_node.invs, &entry_subst, ctx)?.join(" && ");
    let inv_at_step = encode_inv_clauses(&loop_node.invs, &step_subst, ctx)?.join(" && ");

    // When the source tail returns the sole loop cell, bind that opaque exit
    // cell to the actual wrapper result. This creates a full-loop post-state
    // predicate rather than ending assurance at the isolated while-rule. For a
    // finite record cell. The authored invariant is the explicit fixpoint
    // summary; the preservation obligation independently observes every record
    // leaf for one exact step.
    let exit_result_pred = if cells.len() == 1
        && matches!(block.tail.as_deref(), Some(Expr::Path(path)) if path.as_slice() == [cells[0].as_str()])
    {
        let cell = &cells[0];
        let mut result_subst = Env::new();
        result_subst.insert(cell.clone(), Expr::Path(vec!["result".to_string()]));
        let inv_at_result = encode_inv_clauses(&loop_node.invs, &result_subst, ctx)?.join(" && ");
        let cond_at_result = encode_predicate(cond_expr, &result_subst, ctx)?;
        Some(format!(
            "{inv_at_result} && {}",
            negate_condition(&cond_at_result)
        ))
    } else {
        None
    };

    Ok(LoopObligations {
        cells,
        entry_pred,
        cond,
        inv,
        step_cells,
        inv_at_step,
        exit_result_pred,
    })
}

/// Recognize the v1 frozen-subset `while` loop: `block`'s last statement must be a
/// `Stmt::Loop` with `kind: While(_)`, non-empty `invs`, a `dec`, and a straight-line
/// finite state body containing no nested loop / `break` / `continue` / mid-body `return`.
/// Returns the pre-loop prefix statements + the loop node, or an
/// [`RefEncodeError::Unsupported`] naming the out-of-v1 reason (Skipped, never
/// silently Faithful — R-HONEST-3).
fn recognize_v1_loop<'a>(
    block: &'a Block,
    ctx: &BodyRefCtx,
) -> Result<(&'a [Stmt], &'a LoopNode), RefEncodeError> {
    let Some((last, prefix)) = block.stmts.split_last() else {
        return Err(RefEncodeError::Unsupported(
            "no loop statement (the v1 loop arm requires a `while` loop as the last \
             statement before the tail)"
                .to_string(),
        ));
    };
    let Stmt::Loop(loop_node) = last else {
        return Err(RefEncodeError::Unsupported(
            "the last statement before the tail is not a loop (v1's after-loop \
             continuation is in scope ONLY when the loop is the last statement — a \
             loop followed by further straight-line mutation is a v1.1 extension)"
                .to_string(),
        ));
    };
    // The prefix must itself be straight-line (no earlier loop / break / continue /
    // mid-body return) — reuse the shipped thread_stmt rejection by threading it.
    let mut probe: Env = Env::new();
    for stmt in prefix {
        thread_stmt(stmt, &mut probe, ctx)?;
    }
    if !matches!(loop_node.kind, LoopKind::While(_)) {
        return Err(RefEncodeError::Unsupported(
            "`loop`-kind (the infinite-loop form is a multi-exit CPS shape — the corpus \
             `binary_search` uses `loop { if .. { return .. } }`; OUT of the v1 \
             single-`while` subset, Skipped honestly)"
                .to_string(),
        ));
    }
    if loop_node.invs.is_empty() {
        // Structurally LoopNode carries a non-empty invs (the parser enforces §4.1);
        // the Err keeps the rule total against a hand-built node.
        return Err(RefEncodeError::Unsupported(
            "`while` loop with no `inv` (v1's after-loop characterization needs a \
             usable invariant — Skipped honestly)"
                .to_string(),
        ));
    }
    // The body must be straight-line scalar with no nested loop-control / loop /
    // mid-body return — reject any of those before encoding.
    reject_out_of_subset_body(&loop_node.body)?;
    Ok((prefix, loop_node))
}

/// Reject an out-of-v1 loop body: a nested `Stmt::Loop`, a `break`/`continue`, or a
/// mid-body `return` (the multi-exit CPS forms) → an
/// [`RefEncodeError::Unsupported`]. Recurses into `if`-branch bodies (a `break` /
/// `return` nested in an `if` is just as out). A straight-line scalar or exact
/// finite-record body passes; per-statement type/path rejection is left to the
/// shipped [`thread_stmt`] (e.g. an aliased or non-finite assignment is
/// already an Err there).
fn reject_out_of_subset_body(body: &Block) -> Result<(), RefEncodeError> {
    for stmt in &body.stmts {
        reject_out_of_subset_stmt(stmt)?;
    }
    Ok(())
}

fn reject_out_of_subset_stmt(stmt: &Stmt) -> Result<(), RefEncodeError> {
    match stmt {
        Stmt::Loop(_) => Err(RefEncodeError::Unsupported(
            "NESTED loop in a v1 loop body (the inner loop's after-state is itself a \
             fixpoint inside the outer body-step — OUT of v1, Skipped honestly)"
                .to_string(),
        )),
        Stmt::Break => Err(RefEncodeError::Unsupported(
            "`break` in a v1 loop body (a `break` is a multi-exit form — the after-loop \
             characterization needs per-exit invariant conjuncts, a v2 extension; \
             Skipped honestly)"
                .to_string(),
        )),
        Stmt::Continue => Err(RefEncodeError::Unsupported(
            "`continue` in a v1 loop body (a back-edge is a multi-exit control form — \
             OUT of v1, Skipped honestly)"
                .to_string(),
        )),
        Stmt::Return(_) => Err(RefEncodeError::Unsupported(
            "mid-body `return` in a v1 loop body (the corpus `binary_search` uses \
             `return None`/`return Some(mid)` — a multi-exit CPS form, OUT of v1; \
             Skipped honestly)"
                .to_string(),
        )),
        Stmt::If { then, else_, .. } => {
            reject_out_of_subset_body(then)?;
            if let Some(else_block) = else_ {
                reject_out_of_subset_body(else_block)?;
            }
            Ok(())
        }
        Stmt::Let { .. } | Stmt::Assign { .. } | Stmt::Expr(_) => Ok(()),
    }
}

/// Collect the outer state-cell names a straight-line loop body mutates. A bare
/// assignment contributes its scalar/value cell; an exact field chain (with an
/// optional final fixed-array index) contributes its finite-record root. Recurses
/// into `if` branches. A branch-local `let` never contributes a cell.
fn collect_assigned_cells(body: &Block) -> Result<BTreeSet<String>, RefEncodeError> {
    let mut cells = BTreeSet::new();
    collect_assigned_cells_block(body, &mut cells)?;
    Ok(cells)
}

fn collect_assigned_cells_block(
    body: &Block,
    cells: &mut BTreeSet<String>,
) -> Result<(), RefEncodeError> {
    for stmt in &body.stmts {
        match stmt {
            Stmt::Assign { target, .. } => match target {
                Expr::Path(segments) if segments.len() == 1 => {
                    cells.insert(segments[0].clone());
                }
                _ if target_contains_field(target) => {
                    let (root, steps) = nested_lvalue_path(target)?;
                    if !matches!(steps.first(), Some(NestedLvalueStep::Field(_))) {
                        return Err(RefEncodeError::Unsupported(
                            "record-state loop assignment must begin with one exact field"
                                .to_string(),
                        ));
                    }
                    cells.insert(root);
                }
                _ => {
                    return Err(RefEncodeError::Unsupported(
                        "loop assignment target is neither a bare cell nor an exact finite-record field path"
                            .to_string(),
                    ));
                }
            },
            Stmt::If { then, else_, .. } => {
                collect_assigned_cells_block(then, cells)?;
                if let Some(else_block) = else_ {
                    collect_assigned_cells_block(else_block, cells)?;
                }
            }
            // A `let` introduces a fresh (branch-local or body-local) binding, not a
            // mutated outer cell; an `Expr`-stmt has no state effect. Neither
            // contributes a mutated loop cell.
            Stmt::Let { .. } | Stmt::Expr(_) => {}
            // The multi-exit / nested forms are already rejected by
            // reject_out_of_subset_body before this is reached; kept exhaustive.
            Stmt::Loop(_) | Stmt::Break | Stmt::Continue | Stmt::Return(_) => {}
        }
    }
    Ok(())
}

/// Encode a bool-valued predicate (a loop `cond` / `inv` clause) under `env`: the
/// cells are substituted by their env value (entry / stepped) then the predicate is
/// reused through [`exec_ref_value`] (the bounded comparison / logical reference — the
/// same independent encoder the per-RHS value uses). A predicate outside the bounded
/// exec sublanguage (a quantifier, a spec-only combinator) is an Err from
/// [`exec_ref_value`]: the v1 loop subset is scalar-comparison invariants (`lo <=
/// hi`, `i <= n`), never a `forall_*` (those are the `binary_search` v2 forms).
fn encode_predicate(expr: &Expr, env: &Env, ctx: &BodyRefCtx) -> Result<String, RefEncodeError> {
    let substituted = substitute(expr, env)?;
    exec_ref_value(&substituted, &ctx.exec_ref_ctx())
}

/// Encode each loop `inv` Clause to a bool-valued predicate string (under `env` — the
/// cell substitution for the entry / stepped invariant), via [`encode_predicate`].
fn encode_inv_clauses(
    invs: &[Clause],
    env: &Env,
    ctx: &BodyRefCtx,
) -> Result<Vec<String>, RefEncodeError> {
    invs.iter()
        .map(|clause| encode_predicate(&clause.expr, env, ctx))
        .collect()
}

/// Whether the loop's invariant conjunction is trivially weak (every clause is the
/// literal `true`) — the after-loop `true ∧ ¬cond` is vacuous, so the loop cannot
/// enter the (a) rule (`loop-tv.md` REQ-1 out — Skipped, not Faithful). A
/// single `inv true` or several all-`true` clauses are vacuous; any non-`true`
/// conjunct makes the invariant usable.
fn invariant_is_vacuous(invs: &[Clause]) -> bool {
    invs.iter()
        .all(|clause| matches!(clause.expr, Expr::BoolLit(true)))
}

/// Build the negated-condition string `(!(<cond>))` for the exit characterization
/// (`loop-tv.md` REQ-2.3: after-loop = `inv ∧ ¬cond`). Reuses the shipped
/// [`exec_ref_value`] `Not`-encoding (the bounded logical-not reference) over the
/// already-encoded condition — wrapped so the `&&` with the invariant binds the whole
/// negated predicate.
pub fn negate_condition(cond: &str) -> String {
    format!("(!({cond}))")
}

/// Thread `block`'s statements through `env` (in order), then encode its tail value
/// under the resulting env. A block with no tail (a unit-valued straight-line body)
/// is outside the v1 single-exit value subset: the body-refinement obligation
/// compares a result value, so a tail is required (an `Err` otherwise).
fn encode_block_tail(
    block: &Block,
    env: &mut Env,
    ctx: &BodyRefCtx,
) -> Result<String, RefEncodeError> {
    for stmt in &block.stmts {
        thread_stmt(stmt, env, ctx)?;
    }
    match &block.tail {
        Some(tail) => encode_value(tail, env, ctx),
        None => Err(RefEncodeError::Unsupported(
            "straight-line body with no tail value (the body-refinement obligation \
             compares a RESULT value; a unit-valued body is outside the v1 \
             single-exit value subset)"
                .to_string(),
        )),
    }
}

/// Thread one statement through `env` (REQ-2): bind/rebind a cell to its
/// env-substituted RHS. The frozen straight-line subset admits `Let`/`Assign`/
/// `Expr` here; `If`/`Return` are only admitted in tail position (handled by
/// [`encode_value`] / the tail), so an `If`/`Return` in non-tail (statement)
/// position — a mid-body branch / early return — is out of v1 (the multi-exit CPS
/// form) and an `Err`. A `Loop`/`Break`/`Continue` is step 2.2.2.
fn thread_stmt(stmt: &Stmt, env: &mut Env, ctx: &BodyRefCtx) -> Result<(), RefEncodeError> {
    match stmt {
        Stmt::Let { name, init, ty, .. } => {
            // A re-shadow `let x = ..; let x = ..` in the same block is out of v1
            // (the flat name->value env can't represent two distinct `x` cells) —
            // `Err`, never a silent wrong substitution.
            if env.contains_key(name) {
                return Err(RefEncodeError::Unsupported(format!(
                    "re-shadowed binding `{name}` in the same block (the v1 state \
                     environment is a flat name->value map; a re-shadow is OUT of the \
                     frozen subset)"
                )));
            }
            let mut substituted = substitute(init, env)?;
            if let Some(ty) = ty {
                substituted = contextualize_value_for_type(substituted, ty, ctx)?;
            }
            env.insert(name.clone(), substituted);
            if matches!(ty, Some(thermite_syntax::Type::Array { .. })) {
                env.mark_fixed_array(name.clone());
            }
            if let Some(thermite_syntax::Type::Named(type_name)) = ty {
                if ctx.named_record(type_name).is_some() {
                    env.mark_named_record(name.clone(), type_name.clone());
                }
            }
            Ok(())
        }
        Stmt::Assign { target, value } => {
            // A field chain rooted at a typed owned record local is an exact
            // recursive nominal reconstruction. A final fixed-array index is
            // modeled as one exact finite update inside that reconstruction.
            if target_contains_field(target) {
                let (local, mut steps) = nested_lvalue_path(target)?;
                let type_name = env.named_record_type(&local).ok_or_else(|| {
                    RefEncodeError::Unsupported(format!(
                        "field assignment root `{local}` is not a typed finite named-record local"
                    ))
                })?;
                for step in &mut steps {
                    if let NestedLvalueStep::Index(index) = step {
                        *index = substitute(index, env)?;
                    }
                }
                let current = env.get(&local).cloned().ok_or_else(|| {
                    RefEncodeError::Unsupported(format!(
                        "assignment to the unbound named-record local `{local}`"
                    ))
                })?;
                let changed = substitute(value, env)?;
                let changed_spec_int = reference_expr_is_spec_int(&changed);
                let updated = rebuild_nested_value(
                    current,
                    &Type::Named(type_name.to_string()),
                    &steps,
                    changed,
                    changed_spec_int,
                    ctx,
                )?;
                env.insert(local, updated);
                return Ok(());
            }

            // Scalar assignment rebinds the named cell. Fixed-array indexed
            // assignment becomes the exact vstd array-update model, whose view is
            // `old@.update(index, value)` and therefore preserves every other slot.
            if let Expr::Index {
                base,
                index: IndexArg::Single(index),
            } = target
            {
                let Expr::Path(segments) = base.as_ref() else {
                    return Err(RefEncodeError::Unsupported(
                        "fixed-array assignment target with a non-name base".to_string(),
                    ));
                };
                // Writes through an exclusive parameter borrow are observable
                // effects, not local value rebinding. Whole-body obligations route
                // these through the exact lifecycle sequence state; keep this
                // scalar-only environment unchanged for its non-effect consumers.
                if segments.len() == 1 && ctx.is_mutable_indexed_bound(&segments[0]) {
                    return Ok(());
                }
                if segments.len() != 1 || !env.is_fixed_array(&segments[0]) {
                    return Err(RefEncodeError::Unsupported(
                        "indexed assignment to a value not declared as a fixed array".to_string(),
                    ));
                }
                let name = segments[0].clone();
                let current = env.get(&name).cloned().ok_or_else(|| {
                    RefEncodeError::Unsupported(format!(
                        "assignment to the unbound fixed array `{name}`"
                    ))
                })?;
                let index = substitute(index, env)?;
                let value = substitute(value, env)?;
                env.insert(
                    name,
                    Expr::Call {
                        callee: Box::new(Expr::Path(vec![
                            "vstd".to_string(),
                            "array".to_string(),
                            "spec_array_update".to_string(),
                        ])),
                        args: vec![current, index, value],
                    },
                );
                return Ok(());
            }

            // After the exact finite-record path case above, the remaining admitted
            // mutation is a scalar/value-cell rebind: the target must be a bare
            // in-scope name.
            let name = match target {
                Expr::Path(segments) if segments.len() == 1 => segments[0].clone(),
                _ => {
                    return Err(RefEncodeError::Unsupported(
                        "assignment is neither an exact finite-record field path nor \
                         a bare scalar/value cell (computed, dereferenced, aliased, \
                         or otherwise unsupported loop target)"
                            .to_string(),
                    ));
                }
            };
            // The cell must already be in scope (a `let mut` introduced it). An
            // assignment to an unbound name is malformed input — an `Err`.
            if !env.contains_key(&name) {
                return Err(RefEncodeError::Unsupported(format!(
                    "assignment to the unbound cell `{name}` (no in-scope `let mut` \
                     introduced it — malformed straight-line body)"
                )));
            }
            // Order-sensitive: substitute under the current env (the value before
            // this assignment), then rebind. This preserves assignment order: a
            // reorder threads a different substitution chain -> a different closed
            // form (`exec-stmt-tv.md` AC-3).
            let substituted = substitute(value, env)?;
            env.insert(name, substituted);
            Ok(())
        }
        // A bare expression statement `<e>;` in the frozen scalar subset has no
        // STATE effect (a non-tail call's value is discarded; v1 scalar bodies carry
        // no side-effecting cell mutation outside an explicit assignment). It must be
        // well-formed under the env, so we encode (and discard) it to surface a
        // value-encoding error, but it does not thread the state.
        Stmt::Expr(e) => {
            let _ = substitute(e, env)?;
            Ok(())
        }
        // An `if` statement that mutates outer cells per arm (the grounded AC-4 form
        // `if x < 10 { r = r + 1; } else { r = r + 2; }` — `exec-stmt-tv.md` REQ-1
        // lists the `if`-statement as in the frozen 2.2.1 subset, AC-4 grounds it
        // `verified: 1`). It is a state-transformer: thread the then-branch into a
        // copy of the current env (-> the then-env) and the else-branch into another
        // copy (-> the else-env, an absent else == identity), then for each cell
        // either branch mutated, the post-if value becomes the Verus if-expression
        // `if <cond> { <then-cell> } else { <else-cell> }` composing the two branch
        // states (the state-transformer semantics — exec-stmt-tv.md REQ-2 / AC-4). A
        // cell mutated in neither branch is unchanged. The recursion handles a nested
        // `if`-statement in a branch; an out-of-subset branch construct (a loop, a
        // mutation outside the admitted finite closure, a mid-branch return)
        // propagates its `Err`.
        Stmt::If { cond, then, else_ } => {
            // The condition is itself an exec value — substitute it under the
            // pre-`if` env so the composed value is a closed form in the inputs.
            let cond_subst = substitute(cond, env)?;

            // Thread each branch into its own copy of the pre-`if` env. A branch-tail
            // value (a value-discarding `if c { ..; v }` statement) is out of the v1
            // mutation subset — an `Err` (the state-denotation only composes a
            // branch that mutates cells, never a discarded branch value).
            let mut then_env = env.clone();
            thread_branch(then, &mut then_env, ctx)?;
            let mut else_env = env.clone();
            if let Some(else_block) = else_ {
                thread_branch(else_block, &mut else_env, ctx)?;
            }

            // For each cell already in scope before the `if` (a branch-local `let`
            // does not leak past the branch — it lives only in the branch-env clone),
            // recompose: if either branch changed it, the post-`if` value is the
            // branch-composed Verus `if`-expression (an absent else / a non-mutating
            // branch contributes the cell's pre-`if` value, i.e. identity, which
            // the cloned env preserves). A cell mutated in neither branch keeps
            // its pre-`if` value untouched.
            let cell_names: Vec<String> = env.keys().cloned().collect();
            for name in cell_names {
                let then_val = then_env
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| Expr::Path(vec![name.clone()]));
                let else_val = else_env
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| Expr::Path(vec![name.clone()]));
                let pre_val = env.get(&name);
                if pre_val == Some(&then_val) && pre_val == Some(&else_val) {
                    // Unchanged by both branches — leave the cell as-is.
                    continue;
                }
                let composed = Expr::If {
                    cond: Box::new(cond_subst.clone()),
                    then: Block {
                        stmts: vec![],
                        tail: Some(Box::new(then_val)),
                    },
                    else_: Block {
                        stmts: vec![],
                        tail: Some(Box::new(else_val)),
                    },
                };
                env.insert(name, composed);
            }
            Ok(())
        }
        Stmt::Return(_) => Err(RefEncodeError::Unsupported(
            "early `return` in non-tail position (v1 admits `return` only in TAIL \
             position — a mid-body early return is a multi-exit CPS form, OUT of the \
             frozen subset)"
                .to_string(),
        )),
        Stmt::Loop(_) => Err(RefEncodeError::Unsupported(
            "`loop`/`while` in a straight-line body (step 2.2.2 — the after-loop \
             state needs the invariant / a fixpoint; kernel-gated, HONESTLY SKIPPED \
             in 2.2.1)"
                .to_string(),
        )),
        Stmt::Break => Err(RefEncodeError::Unsupported(
            "`break` outside a loop body (loop-control is step 2.2.2)".to_string(),
        )),
        Stmt::Continue => Err(RefEncodeError::Unsupported(
            "`continue` outside a loop body (loop-control is step 2.2.2)".to_string(),
        )),
    }
}

/// Thread an `if`-statement branch `Block`'s statements through `env` (in order),
/// reusing the per-statement [`thread_stmt`] recursively (so a nested `if`-statement
/// in the branch is composed, and an out-of-subset branch construct — a loop, a
/// mutation outside the admitted finite closure, a mid-branch early return —
/// propagates its `Err`). A
/// branch in the v1 mutation subset is value-less (`tail: None`): it mutates outer
/// cells via `Stmt::Assign`, it does not produce a discarded value. A branch with a
/// tail value (`if c { ..; v }` as a statement) is out of the v1 mutation subset — an
/// [`RefEncodeError::Unsupported`], never a silent discard.
fn thread_branch(branch: &Block, env: &mut Env, ctx: &BodyRefCtx) -> Result<(), RefEncodeError> {
    for stmt in &branch.stmts {
        thread_stmt(stmt, env, ctx)?;
    }
    match &branch.tail {
        None => Ok(()),
        Some(_) => Err(RefEncodeError::Unsupported(
            "`if`-statement branch with a tail VALUE (a value-discarding \
             `if c { ..; v }` statement is OUT of the v1 mutation subset — a branch \
             mutates outer cells, it does not produce a discarded value)"
                .to_string(),
        )),
    }
}

/// Strip one layer of fully-enclosing parentheses from `s`, used only for
/// the `if`-condition syntax position (`if <cond> { .. }`), where the canonical
/// reference form is the bare predicate (`exec-stmt-tv.md` AC-4 `if x < 10 { .. }`).
/// `exec_ref_value` wholly parenthesizes a `Binary` (the #122 discipline), so the
/// encoded comparison condition arrives as `(x < 10)`; this removes the redundant
/// outer pair (Verus parses `if x < 10` identically). The strip is conservative: it
/// removes the pair only when the leading `(` matches the trailing `)` and that pair
/// encloses the whole string (a `(a) + (b)` is left untouched — its outer chars are
/// not a single enclosing pair). A string with no enclosing pair is returned as-is.
fn strip_one_enclosing_paren(s: &str) -> String {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'(') || bytes.last() != Some(&b')') {
        return s.to_string();
    }
    // Walk the depth; the opening `(` encloses the whole string only if depth returns
    // to 0 exactly at the final char (never reaching 0 before the end).
    let mut depth = 0i32;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 && i != bytes.len() - 1 {
                    // The leading `(` closed before the end — not a single enclosing
                    // pair (e.g. `(a) + (b)`). Leave the string untouched.
                    return s.to_string();
                }
            }
            _ => {}
        }
    }
    // The leading `(` and trailing `)` are a single enclosing pair — strip them.
    s[1..s.len() - 1].to_string()
}

/// Encode a body value position (a tail expr, a branch tail, a tail-`return`'s
/// expr) under `env` (REQ-2). An `if`-expression composes the two branch
/// state-transformers; a tuple projects the multi-cell final state; everything else
/// is an exec value -> substitute the env then reuse [`exec_ref_value`].
fn encode_value(expr: &Expr, env: &Env, ctx: &BodyRefCtx) -> Result<String, RefEncodeError> {
    // Substitute the env first so a tail / branch-tail that names a cell composed by
    // an `if`-statement (the AC-4 form: the tail `r` whose env value is the
    // branch-composed `Expr::If`) dispatches on the cell's composed value, not on the
    // bare `Path` (which `exec_ref_value` would reject as an `if expression`). For a
    // syntactic `Expr::If` / `Expr::Tuple` tail `substitute` is the identity (it does
    // not recurse into those nodes), so the B3 if-tail / B4 tuple-tail are unchanged.
    let substituted = substitute(expr, env)?;
    match &substituted {
        // The `if` state-transformer (`exec-stmt-tv.md` AC-4): compose the two branch
        // transformers into a Verus `if`-expression over the (already-substituted)
        // condition. The condition and the branch tails are encoded as exec values.
        // For a syntactic if-tail (B3) the branches are still source blocks (a fresh
        // env clone — a branch-local `let` does not leak); for a cell composed by an
        // `if`-statement the branch blocks are `{ tail: <closed-form> }` (already
        // threaded), and `encode_block_tail` re-encodes that closed form unchanged.
        Expr::If { cond, then, else_ } => {
            // The condition sits in Verus `if <cond> { .. }` syntax position, where a
            // bare predicate is the canonical form (`exec-stmt-tv.md` AC-4 reference
            // `if x < 10 { x+1 } else { x+2 }`, B3 `if c { .. }`). `exec_ref_value`
            // wholly parenthesizes a `Binary` (the #122 discipline), so strip one
            // layer of fully-enclosing parens for the condition — Verus parses
            // `if x < 10` identically to `if (x < 10)`, and this matches the pinned
            // reference form. A non-parenthesized cond (a bare path `c`) is unchanged.
            let c = strip_one_enclosing_paren(&encode_value(cond, env, ctx)?);
            let mut then_env = env.clone();
            let t = encode_block_tail(then, &mut then_env, ctx)?;
            let mut else_env = env.clone();
            let e = encode_block_tail(else_, &mut else_env, ctx)?;

            // Verus requires the two `if`/`else` arms to share a type. `exec_ref_value`
            // encodes spec arithmetic (a `Binary` over `+`/`-`/`*`/...) as `int`, but a
            // bare cell value (the identity arm of a no-else `if c { r = r + 1; } r` —
            // the else is the unchanged `r`, a `u64`) stays bounded. If the two arms
            // disagree on int-ness, coerce the bounded (non-`int`) arm with `as int` so
            // the arms unify (Verus parses `(x as int)` and the `result: u64 == <int>`
            // comparison coerces fine). When both arms are arithmetic (the grounded
            // AC-4 `(x + 1)`/`(x + 2)`, B3 `(x + 1)`/`(x - 1)`) no coercion is applied —
            // the pinned reference form is preserved.
            let t_int = branch_is_int_typed(then, env, ctx)?;
            let e_int = branch_is_int_typed(else_, env, ctx)?;
            let (t, e) = match (t_int, e_int) {
                (true, false) => (t, format!("({e} as int)")),
                (false, true) => (format!("({t} as int)"), e),
                _ => (t, e),
            };
            Ok(format!("if {c} {{ {t} }} else {{ {e} }}"))
        }
        // The multi-cell tuple projection (`exec-stmt-tv.md` REQ-2, the design's
        // least-confident #1, grounded by B4): the body's final state across cells
        // is a Verus tuple of each cell's (env-substituted) closed form.
        Expr::Tuple(elems) => {
            let parts = elems
                .iter()
                .map(|e| encode_value(e, env, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("({})", parts.join(", ")))
        }
        // Executable ADT dispatch is a body-level control-flow value, rather than
        // a leaf expression understood by `exec_ref_value`.  The source match has
        // already been recursively state-substituted above, with arm bindings
        // protected from capture, so an empty environment is the exact remaining
        // scope.  Patterns and arms are then encoded independently of production
        // lowering.
        Expr::Match { scrutinee, arms } => encode_match_value(scrutinee, arms, &Env::new(), ctx),
        // Every other value (a path, arithmetic, a cast, a call, an index, ...) is a
        // step-2.1 exec value -> reuse the independent per-RHS encoder (the
        // #122/#146/overflow disciplines unchanged). Already substituted above.
        _ => {
            let contextualized = contextualize_constructor_value(substituted, ctx)?;
            exec_ref_value(&contextualized, &ctx.exec_ref_ctx())
        }
    }
}

/// Collect the names introduced by one match pattern.  The validator normally
/// rejects duplicate bindings and mismatched or-pattern bindings first, but the
/// translation validator is fail-closed on its own: it never relies on that fact
/// when deciding which outer state bindings an arm may observe.
fn pattern_bindings(pattern: &Pattern) -> Result<BTreeSet<String>, RefEncodeError> {
    fn merge(
        target: &mut BTreeSet<String>,
        source: BTreeSet<String>,
    ) -> Result<(), RefEncodeError> {
        for name in source {
            if !target.insert(name.clone()) {
                return Err(RefEncodeError::Unsupported(format!(
                    "duplicate match-pattern binding `{name}`"
                )));
            }
        }
        Ok(())
    }

    match pattern {
        Pattern::Wildcard | Pattern::Literal(_) => Ok(BTreeSet::new()),
        Pattern::Binding(name) => Ok(BTreeSet::from([name.clone()])),
        Pattern::Enum { fields, .. } => {
            let mut bindings = BTreeSet::new();
            for field in fields {
                merge(&mut bindings, pattern_bindings(field)?)?;
            }
            Ok(bindings)
        }
        Pattern::Struct { fields, .. } => {
            let mut bindings = BTreeSet::new();
            for (_, field) in fields {
                merge(&mut bindings, pattern_bindings(field)?)?;
            }
            Ok(bindings)
        }
        Pattern::Or(alternatives) => {
            let Some(first) = alternatives.first() else {
                return Err(RefEncodeError::Unsupported(
                    "empty or-pattern in executable match".to_string(),
                ));
            };
            let expected = pattern_bindings(first)?;
            for alternative in &alternatives[1..] {
                let actual = pattern_bindings(alternative)?;
                if actual != expected {
                    return Err(RefEncodeError::Unsupported(
                        "or-pattern alternatives bind different names".to_string(),
                    ));
                }
            }
            Ok(expected)
        }
        Pattern::Slice(_) => Err(RefEncodeError::Unsupported(
            "slice pattern outside a head-fold specification function".to_string(),
        )),
    }
}

/// Encode one executable match pattern without calling the production lowerer.
/// User-enum variants are qualified from the independently supplied variant-owner
/// map; built-in and already-qualified paths remain unchanged.
fn encode_body_pattern(pattern: &Pattern, ctx: &BodyRefCtx) -> Result<String, RefEncodeError> {
    match pattern {
        Pattern::Wildcard => Ok("_".to_string()),
        Pattern::Literal(value) => exec_ref_value(value, &ctx.exec_ref_ctx()),
        Pattern::Binding(name) => Ok(name.clone()),
        Pattern::Enum { path, fields } => {
            let head = ctx.qualify_pattern_path(path);
            if fields.is_empty() {
                return Ok(head);
            }
            let fields = fields
                .iter()
                .map(|field| encode_body_pattern(field, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("{head}({})", fields.join(", ")))
        }
        Pattern::Struct { path, fields, rest } => {
            let head = ctx.qualify_pattern_path(path);
            let mut parts = Vec::with_capacity(fields.len() + usize::from(*rest));
            for (name, pattern) in fields {
                if matches!(pattern, Pattern::Binding(binding) if binding == name) {
                    parts.push(name.clone());
                } else {
                    parts.push(format!("{name}: {}", encode_body_pattern(pattern, ctx)?));
                }
            }
            if *rest {
                parts.push("..".to_string());
            }
            if parts.is_empty() {
                Ok(format!("{head} {{}}"))
            } else {
                Ok(format!("{head} {{ {} }}", parts.join(", ")))
            }
        }
        Pattern::Or(alternatives) => alternatives
            .iter()
            .map(|alternative| encode_body_pattern(alternative, ctx))
            .collect::<Result<Vec<_>, _>>()
            .map(|parts| parts.join(" | ")),
        Pattern::Slice(_) => Err(RefEncodeError::Unsupported(
            "slice pattern outside a head-fold specification function".to_string(),
        )),
    }
}

/// Encode an executable match value under the current state environment.  Arm
/// bindings shadow same-named outer locals in both guards and bodies; this is the
/// capture-sensitive part of the denotation and is intentionally independent of
/// production lowering.
fn encode_match_value(
    scrutinee: &Expr,
    arms: &[MatchArm],
    env: &Env,
    ctx: &BodyRefCtx,
) -> Result<String, RefEncodeError> {
    if arms.is_empty() {
        return Err(RefEncodeError::Unsupported(
            "executable match with no arms".to_string(),
        ));
    }
    let scrutinee = encode_value(scrutinee, env, ctx)?;
    let mut encoded_arms = Vec::with_capacity(arms.len());
    for arm in arms {
        let bindings = pattern_bindings(&arm.pattern)?;
        let arm_env = env.without_bindings(&bindings);
        let arm_ctx = ctx.without_enum_variants(&bindings);
        let pattern = encode_body_pattern(&arm.pattern, ctx)?;
        let guard = match &arm.guard {
            Some(guard) => format!(" if {}", encode_value(guard, &arm_env, &arm_ctx)?),
            None => String::new(),
        };
        let body = encode_value(&arm.body, &arm_env, &arm_ctx)?;
        encoded_arms.push(format!("{pattern}{guard} => {body}"));
    }
    Ok(format!(
        "match {scrutinee} {{ {}, }}",
        encoded_arms.join(", ")
    ))
}

/// Whether the value an `if`-expression branch `block` yields is encoded by
/// [`exec_ref_value`] as a spec `int` (vs a bounded `u64`/.../`bool`). Used to
/// unify the two arms' Verus types (`exec_ref_value` encodes a `Binary` arithmetic as
/// `int`; a bare cell value — the identity arm of a no-else `if` — stays bounded). It
/// threads the branch's own stmts into a clone of `env` then classifies the
/// (substituted) branch-tail [`Expr`]: spec arithmetic (`Binary` over `+`/`-`/`*`/
/// `/`/`%`/shift/bit-ops) is `int`; a comparison `Binary` is `bool` (not `int`);
/// everything else (a bare path cell, a literal, a cast, an index, a call) is the
/// bounded type (not `int`). A branch with no tail value would already be an `Err`
/// from [`encode_block_tail`]; here an absent tail is conservatively not-`int`.
fn branch_is_int_typed(block: &Block, env: &Env, ctx: &BodyRefCtx) -> Result<bool, RefEncodeError> {
    let mut branch_env = env.clone();
    for stmt in &block.stmts {
        thread_stmt(stmt, &mut branch_env, ctx)?;
    }
    let Some(tail) = &block.tail else {
        return Ok(false);
    };
    let value = substitute(tail, &branch_env)?;
    Ok(matches!(
        value,
        Expr::Binary {
            op: BinOp::Add
                | BinOp::Sub
                | BinOp::Mul
                | BinOp::Div
                | BinOp::Rem
                | BinOp::Shl
                | BinOp::Shr
                | BinOp::BitAnd
                | BinOp::BitOr
                | BinOp::BitXor,
            ..
        }
    ))
}

/// Substitute the env into an [`Expr`] (REQ-2): replace each free `Path` leaf that
/// names an in-env cell with that cell's current value expr, recursively. This is
/// the big-step state threading made concrete on the syntax — the result is a closed
/// form in the initial inputs (the env values are themselves already closed forms).
/// A var not in env is a free input (a param) — left verbatim. This recursion covers
/// the frozen exec-value `Expr` shapes (`exec-stmt-tv.md` REQ-1 RHS sublanguage =
/// the step-2.1 pure-exec subset); an out-of-subset value node is passed through
/// unchanged to [`exec_ref_value`], which rejects it (so the `Err` carries
/// the precise node, never a silent wrong substitution).
fn substitute(expr: &Expr, env: &Env) -> Result<Expr, RefEncodeError> {
    match expr {
        Expr::Path(segments) => {
            if segments.len() == 1 {
                if let Some(value) = env.get(&segments[0]) {
                    return Ok(value.clone());
                }
            }
            Ok(expr.clone())
        }
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::StrLit(_) => Ok(expr.clone()),
        Expr::Array(elements) => Ok(Expr::Array(
            elements
                .iter()
                .map(|element| substitute(element, env))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Expr::ArrayRepeat { value, len } => Ok(Expr::ArrayRepeat {
            value: Box::new(substitute(value, env)?),
            len: len.clone(),
        }),
        Expr::Binary { op, lhs, rhs } => Ok(Expr::Binary {
            op: *op,
            lhs: Box::new(substitute(lhs, env)?),
            rhs: Box::new(substitute(rhs, env)?),
        }),
        Expr::Unary { op, expr: inner } => Ok(Expr::Unary {
            op: *op,
            expr: Box::new(substitute(inner, env)?),
        }),
        Expr::Cast { expr: inner, ty } => Ok(Expr::Cast {
            expr: Box::new(substitute(inner, env)?),
            ty: ty.clone(),
        }),
        Expr::Call { callee, args } => Ok(Expr::Call {
            callee: Box::new(substitute(callee, env)?),
            args: args
                .iter()
                .map(|a| substitute(a, env))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => Ok(Expr::MethodCall {
            receiver: Box::new(substitute(receiver, env)?),
            name: name.clone(),
            args: args
                .iter()
                .map(|argument| substitute(argument, env))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        Expr::Index { base, index } => {
            let new_index = match index {
                IndexArg::Single(i) => IndexArg::Single(Box::new(substitute(i, env)?)),
                IndexArg::RangeTo(i) => IndexArg::RangeTo(Box::new(substitute(i, env)?)),
                IndexArg::RangeFrom(i) => IndexArg::RangeFrom(Box::new(substitute(i, env)?)),
                IndexArg::Range(a, b) => {
                    IndexArg::Range(Box::new(substitute(a, env)?), Box::new(substitute(b, env)?))
                }
            };
            Ok(Expr::Index {
                base: Box::new(substitute(base, env)?),
                index: new_index,
            })
        }
        Expr::Tuple(elems) => Ok(Expr::Tuple(
            elems
                .iter()
                .map(|e| substitute(e, env))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Expr::TupleProj { receiver, index } => Ok(Expr::TupleProj {
            receiver: Box::new(substitute(receiver, env)?),
            index: *index,
        }),
        Expr::Field { receiver, name } => {
            let receiver = substitute(receiver, env)?;
            if let Expr::StructLit { fields, .. } = &receiver {
                if let Some((_, value)) = fields.iter().find(|(field, _)| field == name) {
                    return Ok(value.clone());
                }
            }
            Ok(Expr::Field {
                receiver: Box::new(receiver),
                name: name.clone(),
            })
        }
        Expr::StructLit { path, fields } => Ok(Expr::StructLit {
            path: path.clone(),
            fields: fields
                .iter()
                .map(|(name, value)| Ok((name.clone(), substitute_constructor_field(value, env)?)))
                .collect::<Result<Vec<_>, RefEncodeError>>()?,
        }),
        Expr::Ref {
            mutable,
            expr: inner,
        } => Ok(Expr::Ref {
            mutable: *mutable,
            expr: Box::new(substitute(inner, env)?),
        }),
        Expr::Deref(inner) => Ok(Expr::Deref(Box::new(substitute(inner, env)?))),
        Expr::Match { scrutinee, arms } => Ok(Expr::Match {
            scrutinee: Box::new(substitute(scrutinee, env)?),
            arms: arms
                .iter()
                .map(|arm| {
                    let bindings = pattern_bindings(&arm.pattern)?;
                    let arm_env = env.without_bindings(&bindings);
                    Ok(MatchArm {
                        pattern: arm.pattern.clone(),
                        guard: arm
                            .guard
                            .as_ref()
                            .map(|guard| substitute(guard, &arm_env))
                            .transpose()?,
                        body: substitute(&arm.body, &arm_env)?,
                    })
                })
                .collect::<Result<Vec<_>, RefEncodeError>>()?,
        }),
        // An out-of-subset value node (a closure) is passed through unchanged so
        // [`exec_ref_value`] rejects it with the precise node tag. `Expr::If` is
        // interpreted by `encode_value`; a statement-free if nested directly in a
        // constructor field is closed by `substitute_constructor_field` above.
        other => Ok(other.clone()),
    }
}

/// Close a constructor-field value over the current body environment. A
/// statement-free `if` nested below the constructor is a pure value, but it does
/// not pass through the top-level [`encode_value`] dispatcher. Substitute its
/// condition and both value arms here so no body-local binding leaks into the
/// independently generated reference obligation. Statement-bearing arms remain
/// fail-closed until their outer-state effects can be threaded exactly.
fn substitute_constructor_field(expr: &Expr, env: &Env) -> Result<Expr, RefEncodeError> {
    let Expr::If { cond, then, else_ } = expr else {
        return substitute(expr, env);
    };
    if !then.stmts.is_empty() || !else_.stmts.is_empty() {
        return Err(RefEncodeError::Unsupported(
            "a nested if value with branch statements requires exact outer-state threading"
                .to_string(),
        ));
    }
    let then_tail = then.tail.as_deref().ok_or_else(|| {
        RefEncodeError::Unsupported("nested if then-branch has no value".to_string())
    })?;
    let else_tail = else_.tail.as_deref().ok_or_else(|| {
        RefEncodeError::Unsupported("nested if else-branch has no value".to_string())
    })?;
    Ok(Expr::If {
        cond: Box::new(substitute(cond, env)?),
        then: Block {
            stmts: Vec::new(),
            tail: Some(Box::new(substitute_constructor_field(then_tail, env)?)),
        },
        else_: Block {
            stmts: Vec::new(),
            tail: Some(Box::new(substitute_constructor_field(else_tail, env)?)),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use thermite_syntax::ast::{BinOp, Block, Expr, Stmt};

    fn path(name: &str) -> Expr {
        Expr::Path(vec![name.to_string()])
    }
    fn int(value: u128) -> Expr {
        Expr::IntLit {
            value,
            raw: value.to_string(),
        }
    }
    fn bin(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
        Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }
    fn let_(mutable: bool, name: &str, init: Expr) -> Stmt {
        Stmt::Let {
            mutable,
            name: name.to_string(),
            ty: None,
            init,
        }
    }
    fn assign(target: &str, value: Expr) -> Stmt {
        Stmt::Assign {
            target: path(target),
            value,
        }
    }
    fn index(base: &str, at: Expr) -> Expr {
        Expr::Index {
            base: Box::new(path(base)),
            index: IndexArg::Single(Box::new(at)),
        }
    }
    fn index_assign(base: &str, at: Expr, value: Expr) -> Stmt {
        Stmt::Assign {
            target: index(base, at),
            value,
        }
    }
    fn method(receiver: Expr, name: &str, args: Vec<Expr>) -> Expr {
        Expr::MethodCall {
            receiver: Box::new(receiver),
            name: name.to_string(),
            args,
        }
    }

    /// B1 reference: `{ let a = x + 1; let b = a * 2; b }` -> the threaded closed
    /// form `((x + 1) * 2)` (the let-chain substitution).
    #[test]
    fn b1_let_chain_state() {
        let block = Block {
            stmts: vec![
                let_(false, "a", bin(BinOp::Add, path("x"), int(1))),
                let_(false, "b", bin(BinOp::Mul, path("a"), int(2))),
            ],
            tail: Some(Box::new(path("b"))),
        };
        assert_eq!(
            body_ref_state(&block, &BodyRefCtx::default()).unwrap(),
            "((x + 1) * 2)"
        );
    }

    #[test]
    fn method_receivers_and_arguments_are_state_substituted() {
        let block = Block {
            stmts: vec![let_(
                false,
                "set_word",
                method(path("word"), "bit_set", vec![path("bit")]),
            )],
            tail: Some(Box::new(method(
                path("set_word"),
                "bit_test",
                vec![path("bit")],
            ))),
        };
        let encoded = body_ref_state(&block, &BodyRefCtx::default()).unwrap();
        assert!(!encoded.contains("set_word"), "{encoded}");
        assert!(encoded.contains("match (bit)"), "{encoded}");
        assert!(encoded.contains("(word) | 1u64"), "{encoded}");
    }

    /// B2 mutation-order reference: `s = s + 1; s = s * 2` threads to
    /// `((x + 1) * 2)`, and the reorder threads to a different form, so the order
    /// matters in the reference, not just in production.
    #[test]
    fn b2_mutation_order_state() {
        let ordered = Block {
            stmts: vec![
                let_(true, "s", path("x")),
                assign("s", bin(BinOp::Add, path("s"), int(1))),
                assign("s", bin(BinOp::Mul, path("s"), int(2))),
            ],
            tail: Some(Box::new(path("s"))),
        };
        assert_eq!(
            body_ref_state(&ordered, &BodyRefCtx::default()).unwrap(),
            "((x + 1) * 2)"
        );

        // The reorder is a different closed form — the state threading is real.
        let reordered = Block {
            stmts: vec![
                let_(true, "s", path("x")),
                assign("s", bin(BinOp::Mul, path("s"), int(2))),
                assign("s", bin(BinOp::Add, path("s"), int(1))),
            ],
            tail: Some(Box::new(path("s"))),
        };
        assert_eq!(
            body_ref_state(&reordered, &BodyRefCtx::default()).unwrap(),
            "((x * 2) + 1)"
        );
    }

    /// B3 reference (the `if`-branch state-transformer): the tail `if c { x + 1 }
    /// else { x - 1 }` composes the two branch tails.
    #[test]
    fn b3_if_branch_state() {
        let then = Block {
            stmts: vec![],
            tail: Some(Box::new(bin(BinOp::Add, path("x"), int(1)))),
        };
        let els = Block {
            stmts: vec![],
            tail: Some(Box::new(bin(BinOp::Sub, path("x"), int(1)))),
        };
        let block = Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::If {
                cond: Box::new(path("c")),
                then,
                else_: els,
            })),
        };
        assert_eq!(
            body_ref_state(&block, &BodyRefCtx::default()).unwrap(),
            "if c { (x + 1) } else { (x - 1) }"
        );
    }

    #[test]
    fn annotated_bounded_if_initializer_types_every_literal_arm() {
        let nested = Expr::If {
            cond: Box::new(path("d")),
            then: Block {
                stmts: vec![],
                tail: Some(Box::new(int(2))),
            },
            else_: Block {
                stmts: vec![],
                tail: Some(Box::new(int(3))),
            },
        };
        let block = Block {
            stmts: vec![Stmt::Let {
                mutable: false,
                name: "reason".to_string(),
                ty: Some(Type::Prim(thermite_syntax::PrimType::U8)),
                init: Expr::If {
                    cond: Box::new(path("c")),
                    then: Block {
                        stmts: vec![],
                        tail: Some(Box::new(int(1))),
                    },
                    else_: Block {
                        stmts: vec![],
                        tail: Some(Box::new(nested)),
                    },
                },
            }],
            tail: Some(Box::new(path("reason"))),
        };

        assert_eq!(
            body_ref_state(&block, &BodyRefCtx::default()).unwrap(),
            "if c { 1 as u8 } else { if d { 2 as u8 } else { 3 as u8 } }"
        );
    }

    #[test]
    fn nested_constructor_if_closes_over_prior_bindings() {
        let block = Block {
            stmts: vec![
                let_(false, "old_head", path("head")),
                let_(false, "successor", path("next")),
            ],
            tail: Some(Box::new(Expr::StructLit {
                path: vec!["State".to_string()],
                fields: vec![(
                    "head".to_string(),
                    Expr::If {
                        cond: Box::new(bin(BinOp::Eq, path("node"), path("old_head"))),
                        then: Block {
                            stmts: vec![],
                            tail: Some(Box::new(path("successor"))),
                        },
                        else_: Block {
                            stmts: vec![],
                            tail: Some(Box::new(path("old_head"))),
                        },
                    },
                )],
            })),
        };

        let encoded = body_ref_state(&block, &BodyRefCtx::default()).unwrap();
        assert!(!encoded.contains("old_head"), "{encoded}");
        assert!(!encoded.contains("successor"), "{encoded}");
        assert!(
            encoded.contains("head: if (node == head) { next } else { head }"),
            "{encoded}",
        );
    }

    #[test]
    fn nested_constructor_if_with_branch_statements_fails_closed() {
        let block = Block {
            stmts: vec![let_(false, "old_head", path("head"))],
            tail: Some(Box::new(Expr::StructLit {
                path: vec!["State".to_string()],
                fields: vec![(
                    "head".to_string(),
                    Expr::If {
                        cond: Box::new(path("choose")),
                        then: Block {
                            stmts: vec![let_(false, "branch", path("next"))],
                            tail: Some(Box::new(path("branch"))),
                        },
                        else_: Block {
                            stmts: vec![],
                            tail: Some(Box::new(path("old_head"))),
                        },
                    },
                )],
            })),
        };

        assert!(matches!(
            body_ref_state(&block, &BodyRefCtx::default()),
            Err(RefEncodeError::Unsupported(reason))
                if reason.contains("nested if value with branch statements")
        ));
    }

    /// B4 reference (the multi-cell tuple — the design's least-confident #1): the
    /// final state `(a, b)` projects `a |-> (x + 1)`, `b |-> (y + (x + 1))` (b uses
    /// the updated a, the order-sensitive threading).
    #[test]
    fn b4_multi_cell_tuple_state() {
        let block = Block {
            stmts: vec![
                let_(true, "a", path("x")),
                let_(true, "b", path("y")),
                assign("a", bin(BinOp::Add, path("a"), int(1))),
                assign("b", bin(BinOp::Add, path("b"), path("a"))),
            ],
            tail: Some(Box::new(Expr::Tuple(vec![path("a"), path("b")]))),
        };
        assert_eq!(
            body_ref_state(&block, &BodyRefCtx::default()).unwrap(),
            "((x + 1), (y + (x + 1)))"
        );
    }

    #[test]
    fn untouched_exclusive_storage_is_framed_as_unchanged() {
        let block = Block {
            stmts: vec![],
            tail: Some(Box::new(path("value"))),
        };
        let ctx = BodyRefCtx::default().with_mutable_indexed_bound(["data"]);
        assert_eq!(
            body_ref_state_ensures(&block, "result", &ctx).unwrap(),
            "result == value && final(data)@ == old(data)@"
        );
    }

    #[test]
    fn post_write_storage_read_observes_the_exact_sequence_state() {
        let block = Block {
            stmts: vec![index_assign("data", path("at"), path("value"))],
            tail: Some(Box::new(index("data", path("at")))),
        };
        let ctx = BodyRefCtx::with_slice_bound(["data"]).with_mutable_indexed_bound(["data"]);
        let ensures = body_ref_state_ensures(&block, "result", &ctx).unwrap();
        assert!(
            ensures.contains("(old(data)@).update((at) as int, value)"),
            "{ensures}"
        );
        assert!(ensures.contains("result =="), "{ensures}");
        assert!(ensures.contains("final(data)@ =="), "{ensures}");
    }

    /// A loop body is out of the frozen 2.2.1 subset -> an `Err`, never a
    /// silent (wrong) denotation (REQ-1 boundary).
    #[test]
    fn loop_body_is_unsupported_not_panic() {
        use thermite_syntax::ast::{Clause, LoopKind, LoopNode};
        let span = thermite_syntax::lexer::Span { start: 0, len: 0 };
        let loop_node = LoopNode {
            kind: LoopKind::While(Box::new(path("c"))),
            invs: vec![Clause {
                expr: Expr::BoolLit(true),
                text: "true".to_string(),
                span,
                bv: None,
            }],
            dec: Clause {
                expr: int(0),
                text: "0".to_string(),
                span,
                bv: None,
            },
            body: Block {
                stmts: vec![],
                tail: None,
            },
            span,
        };
        let block = Block {
            stmts: vec![Stmt::Loop(loop_node)],
            tail: Some(Box::new(path("x"))),
        };
        assert!(matches!(
            body_ref_state(&block, &BodyRefCtx::default()),
            Err(RefEncodeError::Unsupported(_))
        ));
    }

    /// A re-shadow `let x = ..; let x = ..` in the same block is out of v1 (the flat
    /// env can't represent two `x` cells) -> an `Err`.
    #[test]
    fn reshadow_is_unsupported() {
        let block = Block {
            stmts: vec![let_(false, "a", path("x")), let_(false, "a", int(1))],
            tail: Some(Box::new(path("a"))),
        };
        assert!(matches!(
            body_ref_state(&block, &BodyRefCtx::default()),
            Err(RefEncodeError::Unsupported(_))
        ));
    }
}
