//! Compile-time effect-row subsumption (`fx`) — the static half of Thermite's
//! effect system (`thermite-design.md` §4.1: "Effect rows compose: a caller's
//! row must subsume every callee's row, checked at compile time").
//!
//! A caller's `fx` row MUST subsume every callee's row. `fx pure` permits
//! nothing, so a `pure` function that calls an effectful one is a compile-time
//! rejection. This component is the **compile-time check ONLY**; the runtime
//! syscall sandbox (§4.1 "killed at the syscall boundary") is DEFERRED to issue
//! #21 (`goal.md` EXCLUDED-from-kernel). R-SPEC-5: the v0.1 form is implemented
//! FULLY; the deferred form (sandbox) is not built — `effects.rs` has no codegen
//! path, only a checking path (REQ-6 / AC-6).
//!
//! Governing design: `.design/lower/effect-subsumption.md` (REQ-1..REQ-6).
//!
//! ## The lattice (REQ-1)
//!
//! The effects form a lattice over the powerset of the nine atoms in
//! `thermite_syntax::ast::Effect` (`Read`/`Write`/`Net`/`Alloc`/`Time`/`Rand`/
//! `Panic`/`Diverge`/`Term`), ordered by subset inclusion. `EffectRow::Pure` ≡ the empty
//! set `{}` is the bottom — it permits nothing and is subsumed by everything. The
//! join of two rows is set union. v0.1 subsumption is **atom-KIND level**
//! (path-insensitive): a `Write(_)` caller subsumes any `Write(_)` callee (OQ-1
//! — path-granular subsumption is a deferred refinement that needs a path lattice
//! the v0.1 kernel does not build).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (effect lattice) | SHIPPED | `enum EffectKind` (the 9 atoms, incl. the #106 `Term`) + `effects` (the powerset projection of an `EffectRow`); consumer `subsumes`/`missing_atoms`; asserted by `tests/effects.rs::lattice_law` (AC-1). |
//! | REQ-2 (subsumption accept relation) | SHIPPED | `pub fn subsumes` (`effects(callee) ⊆ effects(caller)`, `Pure` subsumes only `Pure`); consumer `check_effects`; asserted by `lattice_law` + `crafted_accepts` (AC-1/AC-3). Epic #60: the bit-level subset test is DELEGATED to the Verus-verified `thermite_verified::subsumes_masks` (mechanism (c), the 9-atom `u16` bitset widened for #106); `effects::subsumes` is anchored to the `verus` proof by the exhaustive 262144-pair (512×512) equivalence test `tests/effects_verified.rs`. |
//! | REQ-3 (check entry point + call graph) | SHIPPED | `pub fn check_effects` builds a name→`fx` map over `FnItem`s (`SpecFnItem`/combinators noted pure) and walks each body's `Expr` tree per `Call`/`MethodCall`; consumer `tests/effects.rs` + the #4 lowering pipeline surface; asserted by `corpus_accepts` (AC-2) + `crafted_rejects` (AC-4). |
//! | REQ-4 (structured rejection, `LowerError`) | SHIPPED | `LowerError::EffectNotSubsumed { caller, callee, missing, span }` in `lower.rs`; produced by `check_effects`; `missing` = `effects(callee) \ effects(caller)`; asserted by `crafted_rejects` (AC-4). |
//! | REQ-5 (maximal-row / slag boundary) | SHIPPED | boundary recorded — `check_effects` enforces subsumption only; maximal-row triage is forge's vacuity stage (#6), not here. No maximal-row judgement in this file. |
//! | REQ-6 (runtime sandbox deferred to #21) | SHIPPED | boundary recorded — this file emits NO syscall sandbox / NO runtime scaffolding; it returns `Result<(), Vec<LowerError>>` and has no codegen path (AC-6). |

use std::collections::BTreeMap;

use thermite_syntax::ast::{
    Block, Effect, EffectRow, Expr, IndexArg, Item, LoopKind, Program, Stmt,
};
use thermite_syntax::lexer::Span;

use crate::lower::LowerError;

/// The maximum recursive-descent depth before the body walk returns
/// `LowerError::TooDeep`. Mirrors `lower.rs`'s `MAX_EMIT_DEPTH` discipline (and
/// `thermite-syntax`'s parser guard): a single shared counter bounds the
/// `Expr`-tree walk so a pathological (or adversarial, post-recovery) AST returns
/// a structured error rather than overflowing the native stack (REQ-3 / AC-5).
/// Fixed constant (determinism, `goal.md` R-CODE-5).
const MAX_WALK_DEPTH: usize = 256;

/// The nine atomic effect KINDS (REQ-1), the carriers of subsumption. This is
/// the path-insensitive projection of `thermite_syntax::ast::Effect`: `Read(p)`,
/// `Write(p)`, `Net(d)` collapse to `Read`/`Write`/`Net` regardless of the path/
/// domain argument (OQ-1 — v0.1 subsumption is atom-kind level). The remaining
/// six atoms (`Alloc`/`Time`/`Rand`/`Panic`/`Diverge`/`Term`) are argument-free.
/// `Term` is the #106 terminal-control atom (`fx term` → the `ioctl` seccomp
/// grant, runtime-sandbox.md REQ-7) — the 9th atom that widened the proved bitset
/// from `u8` to `u16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EffectKind {
    Read,
    Write,
    Net,
    Alloc,
    Time,
    Rand,
    Panic,
    Diverge,
    Term,
}

impl EffectKind {
    /// The bit position of this atom kind in the 9-atom `u16` bitset shared with
    /// the Verus-verified core (`thermite_verified`, epic #60 / #106): Read=0,
    /// Write=1, Net=2, Alloc=3, Time=4, Rand=5, Panic=6, Diverge=7, Term=8. This
    /// is the representation port of `.design/verified/self-verification.md`
    /// (REQ-5) — `subsumes` projects each `EffectRow` to its mask via this bit and
    /// delegates the subset test to `thermite_verified::subsumes_masks`, the
    /// plain-Rust mirror of the `verus`-proved exec body. WIDENED `u8`→`u16` for
    /// the 9th atom `Term` (#106) — the proved bitset is now `u16`.
    fn bit(self) -> u16 {
        let index: u16 = match self {
            EffectKind::Read => 0,
            EffectKind::Write => 1,
            EffectKind::Net => 2,
            EffectKind::Alloc => 3,
            EffectKind::Time => 4,
            EffectKind::Rand => 5,
            EffectKind::Panic => 6,
            EffectKind::Diverge => 7,
            EffectKind::Term => 8,
        };
        1u16 << index
    }

    /// The atom-kind of a concrete `Effect`, dropping the path/domain argument
    /// (REQ-2, OQ-1 — path-insensitive in v0.1).
    fn of(effect: &Effect) -> EffectKind {
        match effect {
            Effect::Read(_) => EffectKind::Read,
            Effect::Write(_) => EffectKind::Write,
            Effect::Net(_) => EffectKind::Net,
            Effect::Alloc => EffectKind::Alloc,
            Effect::Time => EffectKind::Time,
            Effect::Rand => EffectKind::Rand,
            Effect::Panic => EffectKind::Panic,
            Effect::Diverge => EffectKind::Diverge,
            Effect::Term => EffectKind::Term,
        }
    }

    /// A representative `Effect` for this kind, used to populate the `missing`
    /// set of an `EffectNotSubsumed` error (REQ-4). The path/domain carriers
    /// (`Read`/`Write`/`Net`) are reported with an empty path, since v0.1
    /// subsumption is path-insensitive (OQ-1) and the agent's fix is to add the
    /// effect KIND to the caller's row.
    fn to_effect(self) -> Effect {
        match self {
            EffectKind::Read => Effect::Read(String::new()),
            EffectKind::Write => Effect::Write(String::new()),
            EffectKind::Net => Effect::Net(String::new()),
            EffectKind::Alloc => Effect::Alloc,
            EffectKind::Time => Effect::Time,
            EffectKind::Rand => Effect::Rand,
            EffectKind::Panic => Effect::Panic,
            EffectKind::Diverge => Effect::Diverge,
            EffectKind::Term => Effect::Term,
        }
    }
}

/// The atom-kind set of an effect row (REQ-1/REQ-2): `effects(Pure) = {}`,
/// `effects(Set(v)) = { kind(e) | e ∈ v }`. A `BTreeSet`-like deduped, ordered
/// `Vec` (ordering is by `EffectKind`'s derived `Ord` — deterministic, R-CODE-5).
fn effects(row: &EffectRow) -> Vec<EffectKind> {
    let mut kinds: Vec<EffectKind> = match row {
        EffectRow::Pure => Vec::new(),
        EffectRow::Set(v) => v.iter().map(EffectKind::of).collect(),
    };
    kinds.sort_unstable();
    kinds.dedup();
    kinds
}

/// The 9-atom `u16` bitset of an effect row (one bit per `EffectKind`, see
/// `EffectKind::bit`): the representation port shared with the Verus-verified
/// core (`.design/verified/self-verification.md` REQ-5). `mask(Pure) = 0`;
/// `mask(Set(v))` ORs in `EffectKind::of(e).bit()` for each `e`. Path-insensitive
/// (OQ-1), exactly the projection `effects` already performs. WIDENED `u8`→`u16`
/// for the 9th atom `Term` (#106).
fn mask(row: &EffectRow) -> u16 {
    match row {
        EffectRow::Pure => 0,
        EffectRow::Set(v) => v.iter().fold(0u16, |m, e| m | EffectKind::of(e).bit()),
    }
}

/// The subsumption relation (REQ-2): `caller` subsumes `callee` iff
/// `effects(callee) ⊆ effects(caller)`. `Pure` (the bottom, `{}`) subsumes only
/// `Pure`; a row of all atoms (the top) subsumes every row. Reflexive by
/// construction (a set is a subset of itself). This is the accept relation
/// `check_effects` asserts at every resolved call site.
///
/// SELF-VERIFICATION (epic #60, `.design/verified/self-verification.md` REQ-5):
/// the bit-level subset decision is DELEGATED to
/// `thermite_verified::subsumes_masks` — the plain-Rust mirror of the
/// `verus`-proved exec body (`(callee & !caller) == 0`). This function projects
/// each `EffectRow` to its 8-atom mask (`mask`) and hands the masks to the
/// verified core; the exhaustive 262144-pair (512×512) equivalence test
/// (`tests/effects_verified.rs`) anchors this projection to the proved subset
/// relation `thermite_verified::spec_subsumes_mask`. Behavior is IDENTICAL to the
/// former set-membership form (the masks encode the same atom-kind sets).
pub fn subsumes(caller: &EffectRow, callee: &EffectRow) -> bool {
    thermite_verified::subsumes_masks(mask(caller), mask(callee))
}

/// The atoms the callee has that the caller's row lacks (`effects(callee) \
/// effects(caller)`) — the `missing` set of an `EffectNotSubsumed` error
/// (REQ-4). Empty iff `subsumes(caller, callee)`. Returned as concrete `Effect`s
/// (path-insensitive representatives) so the diagnostic names exactly the effect
/// KINDS the agent must add to the caller's row.
fn missing_atoms(caller: &EffectRow, callee: &EffectRow) -> Vec<Effect> {
    let caller_set = effects(caller);
    effects(callee)
        .into_iter()
        .filter(|k| !caller_set.contains(k))
        .map(EffectKind::to_effect)
        .collect()
}

/// What a callee name resolves to in the program's effect call graph (REQ-3).
enum Callee<'a> {
    /// A declared `fn` with its own `fx` row.
    Fn(&'a EffectRow),
    /// A `spec fn` (no `fx`; pure by construction — §4.2 spec sublanguage is
    /// total/effect-free) or a registry combinator. Always subsumed.
    Pure,
    /// A name that is neither a declared `FnItem`, `SpecFnItem`, nor a
    /// combinator. A no-op for subsumption — the #2 validator owns unknown-name
    /// rejection (AC-5), so the effect checker does NOT panic or error here.
    Unresolved,
}

/// The compile-time effect-subsumption check (REQ-3). Builds a name→`Contract.fx`
/// map over the program's `FnItem`s (noting `SpecFnItem` names and registry
/// combinators as pure), then walks every `FnItem` body's `Expr` tree. For each
/// `Call`/`MethodCall` whose callee resolves to a declared `FnItem`, it asserts
/// the *caller's* `fx` subsumes the *callee's* `fx` (REQ-2), accumulating one
/// `EffectNotSubsumed` per violation rather than failing on the first (§2.4
/// actionable, crisp feedback). Calls to a `spec fn` / combinator are pure ⇒
/// always permitted; an unresolved callee is a no-op (AC-5). NEVER panics — a
/// pathological body returns `LowerError::TooDeep` (REQ-4 / AC-5).
pub fn check_effects(program: &Program) -> Result<(), Vec<LowerError>> {
    // name → declared `fx` row, over the `FnItem`s. `SpecFnItem` names are noted
    // as pure (they carry no `fx`). On a duplicate name the first declaration
    // wins (deterministic; duplicate-name rejection is the #2 validator's job).
    let mut fn_rows: BTreeMap<&str, &EffectRow> = BTreeMap::new();
    let mut spec_names: BTreeMap<&str, ()> = BTreeMap::new();
    for item in &program.items {
        match item {
            Item::Fn(f) => {
                fn_rows.entry(f.name.as_str()).or_insert(&f.contract.fx);
            }
            Item::SpecFn(s) => {
                spec_names.entry(s.name.as_str()).or_insert(());
            }
            // Basis Stage 1a (`.design/basis/01-adts.md`): a `struct`/`enum`
            // item declares no callable `fn`/`spec fn` name and carries no `fx`
            // row — the neutral value for this name-collection pass is a no-op.
            // (The item is gated at the validator before effect-check runs;
            // dead-in-1a.)
            Item::Struct(_) | Item::Enum(_) => {}
        }
    }

    let resolve = |name: &str| -> Callee<'_> {
        if let Some(row) = fn_rows.get(name) {
            Callee::Fn(row)
        } else if spec_names.contains_key(name) || thermite_spec::lookup(name).is_some() {
            Callee::Pure
        } else {
            Callee::Unresolved
        }
    };

    let mut errors: Vec<LowerError> = Vec::new();
    for item in &program.items {
        // Only `fn` items have an `fx` row and so can be a checked caller; a
        // `spec fn` is pure by construction and makes no effectful calls.
        if let Item::Fn(f) = item {
            // A boundary fn (ffi-boundary.md REQ-2) has `body: None` — its body is
            // foreign and makes no in-language calls to subsume (its own `fx` row
            // is trusted-by-fiat, OQ-4; the row is still CHECKED at the call site,
            // because the boundary fn's declared `fx` is in `fn_rows` above). Only
            // an in-language body is walked for callee subsumption.
            if let Some(body) = &f.body {
                check_block(
                    body,
                    &f.contract.fx,
                    &f.name,
                    f.span,
                    &resolve,
                    0,
                    &mut errors,
                );
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Walk a block's statements (and its tail), checking every `Call`/`MethodCall`
/// against the caller's row (REQ-3). `depth` bounds the recursion (AC-5).
#[allow(
    clippy::too_many_arguments,
    reason = "the walk threads caller row, name, span, resolver, depth, and the error sink through one recursive family; bundling them into a context struct would not reduce the surface"
)]
fn check_block<'a>(
    block: &'a Block,
    caller_fx: &EffectRow,
    caller_name: &str,
    caller_span: Span,
    resolve: &dyn Fn(&str) -> Callee<'a>,
    depth: usize,
    errors: &mut Vec<LowerError>,
) {
    if depth >= MAX_WALK_DEPTH {
        errors.push(LowerError::TooDeep {
            limit: MAX_WALK_DEPTH,
            span: caller_span,
        });
        return;
    }
    let d = depth + 1;
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { init, .. } => check_expr(
                init,
                caller_fx,
                caller_name,
                caller_span,
                resolve,
                d,
                errors,
            ),
            Stmt::Assign { target, value } => {
                check_expr(
                    target,
                    caller_fx,
                    caller_name,
                    caller_span,
                    resolve,
                    d,
                    errors,
                );
                check_expr(
                    value,
                    caller_fx,
                    caller_name,
                    caller_span,
                    resolve,
                    d,
                    errors,
                );
            }
            Stmt::Return(Some(e)) | Stmt::Expr(e) => {
                check_expr(e, caller_fx, caller_name, caller_span, resolve, d, errors)
            }
            Stmt::Return(None) => {}
            Stmt::If { cond, then, else_ } => {
                check_expr(
                    cond,
                    caller_fx,
                    caller_name,
                    caller_span,
                    resolve,
                    d,
                    errors,
                );
                check_block(
                    then,
                    caller_fx,
                    caller_name,
                    caller_span,
                    resolve,
                    d,
                    errors,
                );
                if let Some(e) = else_ {
                    check_block(e, caller_fx, caller_name, caller_span, resolve, d, errors);
                }
            }
            Stmt::Loop(l) => {
                // A `while <cond> { .. }` evaluates `<cond>` at runtime before
                // each iteration; `lower.rs` lowers it (`LoopKind::While(c)` arm),
                // so a `Call` in the condition is a reachable callee and MUST be
                // checked (§4.1: a caller subsumes EVERY callee's row). The
                // `loop` keyword has no condition. The `invs`/`dec` clauses are
                // SPEC/contract positions (§4.2 spec sublanguage is pure by
                // construction), so they are NOT walked — matching `check_expr`'s
                // documented "Loop/spec clauses are NOT walked" discipline.
                if let LoopKind::While(cond) = &l.kind {
                    check_expr(
                        cond,
                        caller_fx,
                        caller_name,
                        caller_span,
                        resolve,
                        d,
                        errors,
                    );
                }
                check_block(
                    &l.body,
                    caller_fx,
                    caller_name,
                    caller_span,
                    resolve,
                    d,
                    errors,
                );
            }
            // break/continue are loop-control statements with NO sub-expression
            // and NO callee (#93): they contribute NO effect to the row walk
            // (the layer-neutral value — verus-lowering.md REQ-12).
            Stmt::Break | Stmt::Continue => {}
        }
    }
    if let Some(tail) = &block.tail {
        check_expr(
            tail,
            caller_fx,
            caller_name,
            caller_span,
            resolve,
            d,
            errors,
        );
    }
}

/// Walk an expression tree, checking every `Call`/`MethodCall` whose callee
/// resolves to a declared `FnItem` against the caller's row (REQ-2/REQ-3).
/// `depth` bounds the recursion (AC-5). A `while` LOOP CONDITION is runtime
/// code and IS walked (in the `Stmt::Loop` arm of `check_block`); the loop's
/// `inv`/`dec` SPEC clauses are NOT walked, since contract/spec positions are
/// pure by construction (§4.2).
fn check_expr<'a>(
    expr: &'a Expr,
    caller_fx: &EffectRow,
    caller_name: &str,
    caller_span: Span,
    resolve: &dyn Fn(&str) -> Callee<'a>,
    depth: usize,
    errors: &mut Vec<LowerError>,
) {
    if depth >= MAX_WALK_DEPTH {
        errors.push(LowerError::TooDeep {
            limit: MAX_WALK_DEPTH,
            span: caller_span,
        });
        return;
    }
    let d = depth + 1;
    match expr {
        Expr::Call { callee, args } => {
            // Resolve the callee by its path's last segment (the frontend is
            // registry-free; combinator/fn calls are plain `Expr::Call` with a
            // `Path` callee — `ast.rs` module doc / lower.rs precedent).
            if let Expr::Path(segs) = callee.as_ref() {
                if let Some(name) = segs.last() {
                    check_call(name, caller_fx, caller_name, caller_span, resolve, errors);
                }
            }
            check_expr(
                callee,
                caller_fx,
                caller_name,
                caller_span,
                resolve,
                d,
                errors,
            );
            for a in args {
                check_expr(a, caller_fx, caller_name, caller_span, resolve, d, errors);
            }
        }
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            // A method call `recv.m(..)` resolves by the method name `m`. Only a
            // resolved `FnItem` triggers a subsumption check; an unresolved
            // method name (intrinsics like `.len()`) is a no-op (AC-5).
            check_call(name, caller_fx, caller_name, caller_span, resolve, errors);
            check_expr(
                receiver,
                caller_fx,
                caller_name,
                caller_span,
                resolve,
                d,
                errors,
            );
            for a in args {
                check_expr(a, caller_fx, caller_name, caller_span, resolve, d, errors);
            }
        }
        Expr::Field { receiver, .. } => check_expr(
            receiver,
            caller_fx,
            caller_name,
            caller_span,
            resolve,
            d,
            errors,
        ),
        Expr::Closure { body, .. } => check_expr(
            body,
            caller_fx,
            caller_name,
            caller_span,
            resolve,
            d,
            errors,
        ),
        Expr::Match { scrutinee, arms } => {
            check_expr(
                scrutinee,
                caller_fx,
                caller_name,
                caller_span,
                resolve,
                d,
                errors,
            );
            for arm in arms {
                check_expr(
                    &arm.body,
                    caller_fx,
                    caller_name,
                    caller_span,
                    resolve,
                    d,
                    errors,
                );
            }
        }
        Expr::If { cond, then, else_ } => {
            check_expr(
                cond,
                caller_fx,
                caller_name,
                caller_span,
                resolve,
                d,
                errors,
            );
            check_block(
                then,
                caller_fx,
                caller_name,
                caller_span,
                resolve,
                d,
                errors,
            );
            check_block(
                else_,
                caller_fx,
                caller_name,
                caller_span,
                resolve,
                d,
                errors,
            );
        }
        Expr::Binary { lhs, rhs, .. } => {
            check_expr(lhs, caller_fx, caller_name, caller_span, resolve, d, errors);
            check_expr(rhs, caller_fx, caller_name, caller_span, resolve, d, errors);
        }
        Expr::Index { base, index } => {
            check_expr(
                base,
                caller_fx,
                caller_name,
                caller_span,
                resolve,
                d,
                errors,
            );
            match index {
                IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
                    check_expr(e, caller_fx, caller_name, caller_span, resolve, d, errors)
                }
                IndexArg::Range(a, b) => {
                    check_expr(a, caller_fx, caller_name, caller_span, resolve, d, errors);
                    check_expr(b, caller_fx, caller_name, caller_span, resolve, d, errors);
                }
            }
        }
        Expr::Cast { expr, .. } | Expr::Ref { expr, .. } => check_expr(
            expr,
            caller_fx,
            caller_name,
            caller_span,
            resolve,
            d,
            errors,
        ),
        // Basis Stage 1a (`.design/basis/01-adts.md`): the ADT expressions are
        // dead-in-1a (gated at the validator before effect-check), but the
        // honest walk descends into their sub-expressions — a call carrying an
        // effect could sit in a struct-literal field value, an `is` scrutinee,
        // or a deref operand, so subsumption must not silently skip it.
        Expr::StructLit { fields, .. } => {
            for (_, value) in fields {
                check_expr(
                    value,
                    caller_fx,
                    caller_name,
                    caller_span,
                    resolve,
                    d,
                    errors,
                );
            }
        }
        Expr::Is { scrutinee, .. } => check_expr(
            scrutinee,
            caller_fx,
            caller_name,
            caller_span,
            resolve,
            d,
            errors,
        ),
        Expr::Deref(inner) => check_expr(
            inner,
            caller_fx,
            caller_name,
            caller_span,
            resolve,
            d,
            errors,
        ),
        // The prefix `!` (#92): descend into the operand so an effect-bearing call
        // under a `!` (e.g. `!has_net_access()`) is still subsumption-checked.
        Expr::Unary { expr, .. } => check_expr(
            expr,
            caller_fx,
            caller_name,
            caller_span,
            resolve,
            d,
            errors,
        ),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): a
        // tuple construction's effects are the UNION of its elements' effects, so
        // every element is effect-walked; a projection's effects are exactly its
        // receiver's (the projection itself is pure). An effect-bearing call inside
        // a tuple element / under a projection is still subsumption-checked.
        Expr::Tuple(elems) => {
            for e in elems {
                check_expr(e, caller_fx, caller_name, caller_span, resolve, d, errors);
            }
        }
        Expr::TupleProj { receiver, .. } => check_expr(
            receiver,
            caller_fx,
            caller_name,
            caller_span,
            resolve,
            d,
            errors,
        ),
        // A string literal is a LEAF (`.design/basis/07-strings.md` REQ-1): no
        // sub-expressions, no calls — it contributes no effect-row obligation, so
        // it joins the no-op leaf arm alongside `IntLit`/`BoolLit`. (Materializing
        // a literal into an owned `String` carries `fx alloc`, but that is keyed at
        // the lowering/constructing site in `lower.rs`, not at this effect walk —
        // the bare literal node has no callee to subsume.)
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
    }
}

/// Resolve a single call-site callee name and, if it is a declared `FnItem`,
/// assert the caller's row subsumes it (REQ-2/REQ-3), pushing an
/// `EffectNotSubsumed` with the exact `missing` set on failure (REQ-4). A `spec
/// fn` / combinator callee is pure ⇒ always subsumed; an unresolved callee is a
/// no-op (AC-5).
fn check_call<'a>(
    name: &str,
    caller_fx: &EffectRow,
    caller_name: &str,
    caller_span: Span,
    resolve: &dyn Fn(&str) -> Callee<'a>,
    errors: &mut Vec<LowerError>,
) {
    match resolve(name) {
        Callee::Fn(callee_fx) => {
            if !subsumes(caller_fx, callee_fx) {
                errors.push(LowerError::EffectNotSubsumed {
                    caller: caller_name.to_string(),
                    callee: name.to_string(),
                    missing: missing_atoms(caller_fx, callee_fx),
                    span: caller_span,
                });
            }
        }
        Callee::Pure | Callee::Unresolved => {}
    }
}
