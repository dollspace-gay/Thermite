//! `forge/src/closure.rs` — the §9 end-to-end vs to-the-boundary classification
//! (issue #17, `.design/forge/e2e-vs-boundary.md`; `thermite-design.md` §9).
//!
//! `thermite-design.md` §9 promises the manifest distinguishes "verified to the
//! boundary" from "verified, period". This module computes, for each `fn` in a
//! parsed file, its **transitive intra-file call closure** and classifies the
//! function's *assurance scope*:
//!
//! - **END-TO-END** ("verified, period") — nothing in the closure is a
//!   `#[boundary]` (foreign, unproven body, #16) or `#[slag]` (trusted-by-fiat
//!   body, #6) fn. Every link is a proved / `spec fn` / combinator body, so the
//!   whole-program guarantee rests only on the toolchain.
//! - **TO-THE-BOUNDARY** ("verified to the boundary") — the closure transitively
//!   reaches a `#[boundary]` or `#[slag]` fn. The fn's own contract is verified,
//!   but the end-to-end guarantee crosses a foreign/unproven body at the crossing
//!   (`goal.md` R-DEFER-9: HONESTLY mark a guarantee that depends on an unproven
//!   foreign body — never claim end-to-end when a boundary is reached).
//!
//! The analysis is PURE and structural: it owns NO prover invocation and changes
//! NO verdict. It LAYERS the §9 composition rule on top of the per-fn verdicts
//! `check::check_file` already produced — the result is recorded as the additive
//! `Certificate::assurance_scope` field ([`crate::manifest::AssuranceScope`]).
//!
//! ## What is PURE, what is a CROSSING (`.design/forge/e2e-vs-boundary.md`)
//!
//! - Nodes are the file's `Item::Fn` and `Item::SpecFn`.
//! - Edges come from walking each `fn` body's `Expr::Call` / `Expr::MethodCall`
//!   (and every nested expression / statement) and resolving the callee by NAME.
//! - A callee resolving to an in-file `Item::SpecFn` is PURE (a `spec fn` is total
//!   / terminating / body-Thermite-verified, §4.2 — never a crossing, even when
//!   self-recursive like `spec_sum`).
//! - A callee resolving to a registry combinator (`forall_in`, `sorted`, … — the
//!   `thermite_spec` set, §4.2) is PURE (a frozen-trigger proved library).
//! - A callee resolving to an in-file `Item::Fn` that is `#[boundary]` / `#[slag]`
//!   is a CROSSING; an in-file pure `Item::Fn` inherits ITS closure.
//! - A callee resolving to nothing in-file and not a combinator (a cross-file
//!   callee) is PURE/IGNORED, NOT a crossing (OQ-1: cross-file resolution is a
//!   documented v0.1 limitation).
//!
//! The walk is cycle-safe (a visited set keyed by fn name; recursion does not
//! loop) and bounded (each node touched once), so it is O(nodes + edges) and
//! DETERMINISTIC (R-CODE-5: a pure function of the parsed `Program`, no wall-clock
//! / unordered iteration in the verdict — the `via` crossing is the first reached
//! in source order, and the result is keyed in a `BTreeMap`).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (transitive call closure, cycle-safe + bounded) | SHIPPED | `pub fn classify` builds `CallGraph::from_program` (walks `Expr::Call`/`MethodCall` via `collect_calls`/`walk_expr`/`walk_block`, resolving in-file callees by name; a `spec fn` / combinator / unresolved callee is PURE), then `reach_crossing` does a cycle-safe DFS (a `visited` `BTreeSet` keyed by name; a self/mutual-recursive node is touched once). Consumer: `check::check_file_with_options` (attaches each cert's `assurance_scope`). |
//! | REQ-2 (END-TO-END vs TO-THE-BOUNDARY rule) | SHIPPED | `classify` maps each `Item::Fn` to `AssuranceScope::EndToEnd` iff `reach_crossing` finds NO `#[boundary]`/`#[slag]` fn in its closure, else `AssuranceScope::ToBoundary { via }` recording the first reached crossing. A `#[boundary]`/`#[slag]` fn is itself the crossing (`via` is itself). |
//! | REQ-3 (per-fn additive cert field) | SHIPPED | `AssuranceScope` (in `manifest.rs`) is the additive `Certificate::assurance_scope` field; this module produces the value, `check.rs` attaches it. |
//! | REQ-4 (project-level claim) | SHIPPED | `degrade.rs`'s `AssuranceManifest::aggregate` computes `ProjectScope` from the per-fn scopes (END-TO-END iff every fn is). |
//! | REQ-5 (scope ⊥ level) | SHIPPED | `classify` reads only the call graph (`#[boundary]`/`#[slag]` syntactic flags) — never a cert `Level`; `check.rs` attaches the scope ALONGSIDE the achieved level, so an L3 fn whose closure crosses a boundary is `ToBoundary` at `Level::L3`. |
//! | REQ-6 (determinism) | SHIPPED | `classify` returns a `BTreeMap<String, AssuranceScope>` (sorted, stable); the `via` crossing is the FIRST reached in source-order DFS (`reach_crossing` iterates callees in source order, visited-guarded). A pure function of the `Program` — no wall-clock / unordered map in the verdict (R-CODE-5). |
//!
//! ## #52 reuse note (`.design/lower/boundary-composition.md`)
//!
//! The private `CallGraph::from_program` + the cycle-safe DFS are now ALSO
//! consumed by the §9 boundary-composition weaving (#52): `pub fn
//! reachable_in_file_fns` reuses the SAME walker (a new `CallGraph::reachable_fns`
//! DFS sibling of `reach_crossing`) to return every in-file `Item::Fn` a caller
//! transitively references, which `check::item_subprogram` weaves into the
//! caller's §5.3 sub-program (regular fns with their real body, boundary/slag fns
//! as `#[verifier::external_body]` signatures). No walker is duplicated.
//!
//! ## Cluster C10 — ergonomics ripple (`.design/basis/11-ergonomics.md`, #112)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-3 (MatchArm.guard ripple) | SHIPPED | `walk_expr`'s `Expr::Match` arm walks `arm.guard` (a guard may CALL a fn — its callee crosses into the call graph / closure surface). `Pattern::Or` needs no closure arm (the call-graph walk is over expressions). Consumer: `classify` / `reachable_in_file_fns`. |

use std::collections::{BTreeMap, BTreeSet};

use thermite_syntax::{Block, Expr, IndexArg, Item, Program, Stmt};

use crate::manifest::AssuranceScope;

/// One node's classification input: whether it is itself a crossing
/// (`#[boundary]` / `#[slag]`) and the in-file callees its body reaches (the
/// out-edges), in source order. A `spec fn` has `is_crossing = false` and is
/// never a crossing (§4.2); a boundary fn has `body: None`, so NO out-edges.
struct Node {
    /// `true` iff this fn is itself a CROSSING: a `#[boundary]` (foreign body) or
    /// `#[slag]` (fiat-trusted body) fn. The `via` for any fn reaching it is this
    /// node's name.
    is_crossing: bool,
    /// `true` iff this node is an `Item::Fn` (a regular OR `#[boundary]`/`#[slag]`
    /// fn), `false` for an `Item::SpecFn`. Lets [`CallGraph::reachable_fns`]
    /// return only `fn` dependencies (#52 weaving) — a `spec fn` is woven
    /// separately by `check::item_subprogram`'s `spec_items` set.
    is_fn: bool,
    /// The in-file callee names this node's body calls directly, in source order
    /// (deterministic). Resolved in [`CallGraph::from_program`]; an unresolved /
    /// `spec fn` / combinator callee that is NOT an in-file `Item::Fn` node is
    /// dropped here (it can never reach a crossing — REQ-1 PURE).
    callees: Vec<String>,
}

/// The intra-file call graph the classification walks (REQ-1). Keyed by fn name
/// in a `BTreeMap` for deterministic iteration. Only IN-FILE `Item::Fn` /
/// `Item::SpecFn` are nodes; the edges are resolved-by-name calls.
struct CallGraph {
    nodes: BTreeMap<String, Node>,
}

impl CallGraph {
    /// Build the call graph from a parsed `Program` (REQ-1). Every `Item::Fn` and
    /// `Item::SpecFn` becomes a node; a `fn`'s out-edges are the in-file callees
    /// its body reaches. A node `is_crossing` iff it is a `#[boundary]` or
    /// `#[slag]` `Item::Fn` (a `spec fn` is never a crossing, §4.2).
    fn from_program(program: &Program) -> Self {
        // First pass: the set of in-file fn/spec-fn names (the resolvable
        // callees). A call resolving to a name NOT here is cross-file / a
        // combinator / unknown → PURE, dropped (OQ-1, REQ-1).
        let in_file: BTreeSet<&str> = program.items.iter().map(|i| i.name()).collect();

        let mut nodes = BTreeMap::new();
        for item in &program.items {
            match item {
                Item::Fn(f) => {
                    let is_crossing = f.boundary.is_some() || f.slag.is_some();
                    // A boundary fn has `body: None` (a leaf crossing, no
                    // out-edges); an in-language fn walks its body for calls.
                    let callees = match &f.body {
                        Some(body) => collect_in_file_calls(body, &in_file),
                        None => Vec::new(),
                    };
                    nodes.insert(
                        f.name.clone(),
                        Node {
                            is_crossing,
                            is_fn: true,
                            callees,
                        },
                    );
                }
                Item::SpecFn(s) => {
                    // A `spec fn` is PURE (§4.2): never a crossing. Its callees
                    // are recorded so the closure walk terminates on them, but a
                    // spec fn can only reach other spec fns / combinators (the
                    // SpecTherm cage), none of which is a crossing.
                    let callees = collect_in_file_calls(&s.body, &in_file);
                    nodes.insert(
                        s.name.clone(),
                        Node {
                            is_crossing: false,
                            is_fn: false,
                            callees,
                        },
                    );
                }
                // Basis Stage 1a (`.design/basis/01-adts.md`): a `struct`/`enum`
                // item is not a callable call-graph node — it is neither a
                // crossing nor a fn with out-edges. The neutral value is to
                // insert no node. Dead-in-1a (gated at the validator).
                Item::Struct(_) | Item::Enum(_) => {}
            }
        }
        CallGraph { nodes }
    }

    /// Every IN-FILE fn name (`Item::Fn`, NOT `Item::SpecFn`) transitively
    /// referenced from `start`'s body, EXCLUDING `start` itself — a cycle-safe,
    /// bounded DFS over the same out-edges [`reach_crossing`] walks (#52
    /// composition weaving). Used by `check::item_subprogram` to weave a caller's
    /// regular-fn dependencies (real body) and boundary/slag dependencies
    /// (external_body signature) into its §5.3 sub-program so `lower`/`verus`
    /// resolve every referenced callee.
    ///
    /// A `spec fn` is EXCLUDED here: spec fns are woven separately by
    /// `item_subprogram` (the existing `spec_items` set), so emitting them again
    /// would duplicate. The walk follows edges THROUGH every node (so a fn reached
    /// only via a spec-fn or boundary-fn intermediary is still found), but only
    /// `Item::Fn` names are RETURNED. Returns a `BTreeSet` (sorted, stable —
    /// DETERMINISTIC, R-CODE-5).
    fn reachable_fns(&self, start: &str) -> BTreeSet<String> {
        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut result: BTreeSet<String> = BTreeSet::new();
        let mut stack: Vec<String> = vec![start.to_string()];
        while let Some(name) = stack.pop() {
            if !visited.insert(name.clone()) {
                continue;
            }
            let Some(node) = self.nodes.get(&name) else {
                continue;
            };
            // Record every reached `fn` node except `start` itself. A `spec fn`
            // node is `is_fn == false` and is left for `item_subprogram`'s
            // `spec_items` weaving (no duplication).
            if name != start && node.is_fn {
                result.insert(name.clone());
            }
            for callee in node.callees.iter().rev() {
                if !visited.contains(callee) {
                    stack.push(callee.clone());
                }
            }
        }
        result
    }

    /// The first CROSSING (`#[boundary]`/`#[slag]` fn) reachable from `start` in a
    /// cycle-safe, source-order DFS (REQ-1/REQ-2/REQ-6). Returns the crossing's
    /// name (`via`) or `None` when the whole transitive closure is pure.
    ///
    /// `start` itself counts: a `#[boundary]`/`#[slag]` fn is its own crossing
    /// (`via` is itself). The `visited` set is keyed by name so a self- or
    /// mutually-recursive node is entered exactly once — the walk terminates and
    /// is bounded (each node touched once). DETERMINISTIC: callees are visited in
    /// source order, so the FIRST reached crossing is stable.
    fn reach_crossing(&self, start: &str) -> Option<String> {
        let mut visited: BTreeSet<String> = BTreeSet::new();
        // Explicit work stack (source-order DFS) so a deep / recursive program
        // never overflows the call stack (bounded — REQ-1).
        let mut stack: Vec<String> = vec![start.to_string()];
        while let Some(name) = stack.pop() {
            if !visited.insert(name.clone()) {
                // Already visited — cycle-safe (recursion does not loop).
                continue;
            }
            let Some(node) = self.nodes.get(&name) else {
                // An unresolved callee (defensive — `collect_in_file_calls` only
                // emits in-file names): not a crossing.
                continue;
            };
            if node.is_crossing {
                return Some(name);
            }
            // Push callees in REVERSE source order so they pop in source order
            // (the deterministic FIRST-reached `via`, REQ-6).
            for callee in node.callees.iter().rev() {
                if !visited.contains(callee) {
                    stack.push(callee.clone());
                }
            }
        }
        None
    }
}

/// Classify every `fn` / `spec fn` in `program` by its assurance scope (REQ-1/
/// REQ-2/REQ-6). Returns a `BTreeMap<fn name, AssuranceScope>` (sorted, stable —
/// deterministic): `AssuranceScope::EndToEnd` when the fn's transitive intra-file
/// call closure reaches NO `#[boundary]`/`#[slag]` fn; `AssuranceScope::ToBoundary
/// { via }` recording the first reached crossing otherwise.
///
/// The classification is SYNTACTIC and orthogonal to the verification level
/// (REQ-5): it reads only the call graph, never a cert. A `#[boundary]`/`#[slag]`
/// fn is trivially `ToBoundary` (it IS the crossing, `via` itself). It is a pure
/// function of the parsed `Program` (R-CODE-5).
///
/// Consumed by `check::check_file_with_options`, which attaches each fn's scope to
/// its certificate (`Certificate::assurance_scope`) ALONGSIDE the achieved level.
pub fn classify(program: &Program) -> BTreeMap<String, AssuranceScope> {
    let graph = CallGraph::from_program(program);
    let mut scopes = BTreeMap::new();
    for name in graph.nodes.keys() {
        let scope = match graph.reach_crossing(name) {
            Some(via) => AssuranceScope::ToBoundary { via },
            None => AssuranceScope::EndToEnd,
        };
        scopes.insert(name.clone(), scope);
    }
    scopes
}

/// The set of in-file `Item::Fn` names that `start`'s body transitively
/// references (EXCLUDING `start` itself), for the §9 boundary-composition
/// weaving (`.design/lower/boundary-composition.md` REQ-2, crosslink #52).
///
/// `check::item_subprogram` consumes this to build a caller `f`'s isolated §5.3
/// sub-program: every regular reachable fn is woven with its REAL body (proved),
/// and every `#[boundary]`/`#[slag]` reachable fn is woven as a
/// `#[verifier::external_body]` signature (`thermite_lower::lower`), so `verus`
/// resolves the foreign callee and `f` proves THROUGH its contract (was an
/// undefined-callee L0). A `spec fn` is EXCLUDED here — it is woven separately by
/// `item_subprogram`'s existing `spec_items` set (no duplication); the transitive
/// walk still traverses THROUGH spec-fn / boundary-fn intermediaries so a fn
/// reached only via one is still found.
///
/// Reuses the private `CallGraph::from_program` + a cycle-safe, bounded,
/// source-order DFS (`reachable_fns`) — the SAME walker `classify` uses (the #17
/// reachability seam), never a duplicate. Returns a `BTreeSet` (sorted, stable —
/// DETERMINISTIC, R-CODE-5: a pure function of the parsed `Program`).
pub fn reachable_in_file_fns(program: &Program, start: &str) -> BTreeSet<String> {
    CallGraph::from_program(program).reachable_fns(start)
}

/// Walk a `Block` collecting the names of every in-file callee its expressions
/// reach (the out-edges for one node, in source order). A name resolving to an
/// in-file node (`in_file`) is kept; any other callee (a combinator, a cross-file
/// fn, an unknown — REQ-1 PURE) is dropped. Deterministic: a depth-first walk in
/// source order with no deduplication needed (the closure walk dedups via
/// `visited`).
fn collect_in_file_calls(block: &Block, in_file: &BTreeSet<&str>) -> Vec<String> {
    let mut out = Vec::new();
    walk_block(block, in_file, &mut out);
    out
}

/// Recursively walk a block's statements + tail, collecting in-file callee names.
fn walk_block(block: &Block, in_file: &BTreeSet<&str>, out: &mut Vec<String>) {
    for stmt in &block.stmts {
        walk_stmt(stmt, in_file, out);
    }
    if let Some(tail) = &block.tail {
        walk_expr(tail, in_file, out);
    }
}

/// Recursively walk one statement, collecting in-file callee names.
fn walk_stmt(stmt: &Stmt, in_file: &BTreeSet<&str>, out: &mut Vec<String>) {
    match stmt {
        Stmt::Let { init, .. } => walk_expr(init, in_file, out),
        Stmt::Assign { target, value } => {
            walk_expr(target, in_file, out);
            walk_expr(value, in_file, out);
        }
        Stmt::Return(opt) => {
            if let Some(e) = opt {
                walk_expr(e, in_file, out);
            }
        }
        Stmt::If { cond, then, else_ } => {
            walk_expr(cond, in_file, out);
            walk_block(then, in_file, out);
            if let Some(b) = else_ {
                walk_block(b, in_file, out);
            }
        }
        Stmt::Loop(node) => {
            // Invariants / decreases are spec expressions (they may call spec fns
            // / combinators only — all PURE — but walk them anyway for
            // completeness; an in-file pure fn referenced there is a real edge).
            for inv in &node.invs {
                walk_expr(&inv.expr, in_file, out);
            }
            walk_expr(&node.dec.expr, in_file, out);
            if let thermite_syntax::LoopKind::While(cond) = &node.kind {
                walk_expr(cond, in_file, out);
            }
            walk_block(&node.body, in_file, out);
        }
        Stmt::Expr(e) => walk_expr(e, in_file, out),
        // break/continue carry no sub-expression and no callee (#93): no
        // call-graph edge (the layer-neutral leaf value).
        Stmt::Break | Stmt::Continue => {}
    }
}

/// Recursively walk one expression, collecting the names of in-file callees it
/// invokes. The call forms (`.design/forge/e2e-vs-boundary.md` "the call graph"):
///
/// - `Expr::Call { callee, .. }` — resolve the leading `Path` segment to a name.
/// - `Expr::MethodCall { name, .. }` — the method `name` is the callee name.
///
/// Every other expression is walked for NESTED calls (an `if`-condition call, a
/// call argument, a closure body — the closure body of e.g. `forall_in(xs, |x|
/// helper(x))`). Only a name in `in_file` is emitted (REQ-1 PURE for the rest).
fn walk_expr(expr: &Expr, in_file: &BTreeSet<&str>, out: &mut Vec<String>) {
    match expr {
        Expr::Call { callee, args } => {
            // Resolve the callee NAME: the leading `Path` segment of `f(args)`
            // (the free-fn form). A non-`Path` callee (an indirect call) resolves
            // to no in-file node — PURE/ignored (OQ-1). Walk the callee too so a
            // nested call inside it is not missed.
            if let Expr::Path(segments) = callee.as_ref() {
                if let Some(first) = segments.first() {
                    record(first, in_file, out);
                }
            } else {
                walk_expr(callee, in_file, out);
            }
            for arg in args {
                walk_expr(arg, in_file, out);
            }
        }
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            // The postfix `recv.m(args)` form: the method `name` is the callee.
            record(name, in_file, out);
            walk_expr(receiver, in_file, out);
            for arg in args {
                walk_expr(arg, in_file, out);
            }
        }
        Expr::Field { receiver, .. } => walk_expr(receiver, in_file, out),
        Expr::Closure { body, .. } => walk_expr(body, in_file, out),
        Expr::Match { scrutinee, arms } => {
            walk_expr(scrutinee, in_file, out);
            for arm in arms {
                // A C10 match guard is an `Expr` in the closure-spec walk too
                // (`.design/basis/11-ergonomics.md` REQ-3).
                if let Some(guard) = &arm.guard {
                    walk_expr(guard, in_file, out);
                }
                walk_expr(&arm.body, in_file, out);
            }
        }
        Expr::If { cond, then, else_ } => {
            walk_expr(cond, in_file, out);
            walk_block(then, in_file, out);
            walk_block(else_, in_file, out);
        }
        Expr::Binary { lhs, rhs, .. } => {
            walk_expr(lhs, in_file, out);
            walk_expr(rhs, in_file, out);
        }
        Expr::Index { base, index } => {
            walk_expr(base, in_file, out);
            match index {
                IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
                    walk_expr(e, in_file, out)
                }
                IndexArg::Range(a, b) => {
                    walk_expr(a, in_file, out);
                    walk_expr(b, in_file, out);
                }
            }
        }
        Expr::Cast { expr, .. } => walk_expr(expr, in_file, out),
        Expr::Ref { expr, .. } => walk_expr(expr, in_file, out),
        // Basis Stage 1a (`.design/basis/01-adts.md`): dead-in-1a ADT
        // expressions, but the honest call-graph walk descends into their
        // sub-expressions — an in-file call could sit in a struct-literal field
        // value, an `is` scrutinee, or a deref operand, so no out-edge is
        // silently dropped.
        Expr::StructLit { fields, .. } => {
            for (_, value) in fields {
                walk_expr(value, in_file, out);
            }
        }
        Expr::Is { scrutinee, .. } => walk_expr(scrutinee, in_file, out),
        Expr::Deref(inner) => walk_expr(inner, in_file, out),
        // The prefix `!` (#92): an in-file call could sit under it; descend.
        Expr::Unary { expr, .. } => walk_expr(expr, in_file, out),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): an
        // in-file call could sit in any tuple element or projection receiver, so no
        // out-edge is silently dropped — descend into both.
        Expr::Tuple(elems) => {
            for e in elems {
                walk_expr(e, in_file, out);
            }
        }
        Expr::TupleProj { receiver, .. } => walk_expr(receiver, in_file, out),
        // Leaves: no nested call to find. A string literal
        // (`.design/basis/07-strings.md` REQ-1) is a LEAF — no sub-expression, no
        // callee — so it contributes no out-edge.
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
    }
}

/// Emit `name` as an out-edge iff it resolves to an in-file node (REQ-1). A name
/// not in `in_file` (a combinator, a cross-file callee, an unknown) is PURE and
/// dropped — it can never reach a crossing (OQ-1).
fn record(name: &str, in_file: &BTreeSet<&str>, out: &mut Vec<String>) {
    if in_file.contains(name) {
        out.push(name.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a program source into a `Program`, asserting a clean parse (the
    /// fixtures here are all well-formed).
    fn parse(src: &str) -> Program {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        parsed.program
    }

    // REQ-2 / AC-1: a pure-Thermite fn calling only a spec fn is END-TO-END.
    // Anchored to the corpus `sum` shape (sum -> spec_sum, a spec fn).
    #[test]
    fn pure_caller_of_spec_fn_is_end_to_end() {
        let src = "\
spec fn spec_id(x: u32) -> u32 dec 0 { x }
fn f(x: u32) -> u32 req x < 100 ens result == x fx pure { spec_id(x) }";
        let scopes = classify(&parse(src));
        assert_eq!(scopes.get("f"), Some(&AssuranceScope::EndToEnd));
        assert_eq!(scopes.get("spec_id"), Some(&AssuranceScope::EndToEnd));
    }

    // REQ-2 / AC-2: a direct boundary caller is TO-THE-BOUNDARY via the boundary;
    // the boundary fn itself is the crossing.
    #[test]
    fn direct_boundary_caller_is_to_boundary() {
        let src = "\
#[boundary(\"ext::ext_id\")] fn ext_id(x: u32) -> u32 req x < 100 ens result == x fx pure ;
fn caller(x: u32) -> u32 req x < 100 ens result == x fx pure { ext_id(x) }";
        let scopes = classify(&parse(src));
        assert_eq!(
            scopes.get("caller"),
            Some(&AssuranceScope::ToBoundary {
                via: "ext_id".to_string()
            })
        );
        // The boundary fn is itself the crossing (via itself).
        assert_eq!(
            scopes.get("ext_id"),
            Some(&AssuranceScope::ToBoundary {
                via: "ext_id".to_string()
            })
        );
    }

    // REQ-1 / AC-3: a transitive chain h -> g -> ext_id reaches the boundary.
    #[test]
    fn transitive_boundary_chain_is_to_boundary() {
        let src = "\
#[boundary(\"ext::ext_id\")] fn ext_id(x: u32) -> u32 req x < 100 ens result == x fx pure ;
fn g(x: u32) -> u32 req x < 100 ens result == x fx pure { ext_id(x) }
fn h(x: u32) -> u32 req x < 100 ens result == x fx pure { g(x) }";
        let scopes = classify(&parse(src));
        assert_eq!(
            scopes.get("h"),
            Some(&AssuranceScope::ToBoundary {
                via: "ext_id".to_string()
            })
        );
        assert_eq!(
            scopes.get("g"),
            Some(&AssuranceScope::ToBoundary {
                via: "ext_id".to_string()
            })
        );
    }

    // REQ-2 / AC-4: a slag fn in the closure is TO-THE-BOUNDARY (slag == boundary
    // for the whole-program guarantee).
    #[test]
    fn slag_in_closure_is_to_boundary() {
        let src = "\
#[slag(reason = \"x\", owner = \"a\", review = \"required\")] fn vendored(x: u32) -> u32 req x < 100 ens result == x fx pure { x }
fn caller(x: u32) -> u32 req x < 100 ens result == x fx pure { vendored(x) }";
        let scopes = classify(&parse(src));
        assert_eq!(
            scopes.get("caller"),
            Some(&AssuranceScope::ToBoundary {
                via: "vendored".to_string()
            })
        );
    }

    // REQ-1 / AC-6: a self-recursive pure fn does not loop and is END-TO-END
    // (recursion is not a crossing).
    #[test]
    fn self_recursive_pure_fn_is_end_to_end_and_terminates() {
        let src = "\
spec fn spec_sum(xs: &[u32]) -> u64 dec xs.len() { match xs { [] => 0, [head, ..t] => head as u64 + spec_sum(t), } }";
        let scopes = classify(&parse(src));
        assert_eq!(scopes.get("spec_sum"), Some(&AssuranceScope::EndToEnd));
    }

    // REQ-1 / AC-6: mutual recursion a -> b -> a terminates and (both pure) is
    // END-TO-END.
    #[test]
    fn mutual_recursion_terminates_and_is_end_to_end() {
        let src = "\
fn a(x: u32) -> u32 req x < 100 ens result == x fx pure { b(x) }
fn b(x: u32) -> u32 req x < 100 ens result == x fx pure { a(x) }";
        let scopes = classify(&parse(src));
        assert_eq!(scopes.get("a"), Some(&AssuranceScope::EndToEnd));
        assert_eq!(scopes.get("b"), Some(&AssuranceScope::EndToEnd));
    }

    // REQ-6 / AC-7: classifying the same program twice yields identical scopes
    // and an identical `via` choice.
    #[test]
    fn classification_is_deterministic() {
        let src = "\
#[boundary(\"ext::ext_id\")] fn ext_id(x: u32) -> u32 req x < 100 ens result == x fx pure ;
fn g(x: u32) -> u32 req x < 100 ens result == x fx pure { ext_id(x) }
fn h(x: u32) -> u32 req x < 100 ens result == x fx pure { g(x) }";
        let a = classify(&parse(src));
        let b = classify(&parse(src));
        assert_eq!(a, b);
    }

    // OQ-1: an unresolved (cross-file) callee is PURE/ignored, NOT a crossing.
    #[test]
    fn unresolved_cross_file_callee_is_pure() {
        let src = "fn f(x: u32) -> u32 req x < 100 ens result == x fx pure { external_helper(x) }";
        let scopes = classify(&parse(src));
        assert_eq!(scopes.get("f"), Some(&AssuranceScope::EndToEnd));
    }

    // #52 REQ-2: `reachable_in_file_fns` returns the in-file `fn`s a caller
    // transitively references — the boundary fn `ext_id` for the direct caller.
    #[test]
    fn reachable_fns_includes_a_directly_called_boundary_fn() {
        let src = "\
#[boundary(\"ext::ext_id\")] fn ext_id(x: u32) -> u32 req x < 100 ens result == x fx pure ;
fn caller(x: u32) -> u32 req x < 100 ens result == x fx pure { ext_id(x) }";
        let deps = reachable_in_file_fns(&parse(src), "caller");
        assert!(
            deps.contains("ext_id"),
            "caller references ext_id: {deps:?}"
        );
        assert!(
            !deps.contains("caller"),
            "start itself is excluded: {deps:?}"
        );
    }

    // #52 REQ-2 (transitive): h's sub-program must weave BOTH g (real body) and
    // ext_id (external_body) — both are reachable `fn`s.
    #[test]
    fn reachable_fns_is_transitive_through_an_intermediary() {
        let src = "\
#[boundary(\"ext::ext_id\")] fn ext_id(x: u32) -> u32 req x < 100 ens result == x fx pure ;
fn g(x: u32) -> u32 req x < 100 ens result == x fx pure { ext_id(x) }
fn h(x: u32) -> u32 req x < 100 ens result == x fx pure { g(x) }";
        let deps = reachable_in_file_fns(&parse(src), "h");
        assert!(deps.contains("g"), "h transitively references g: {deps:?}");
        assert!(
            deps.contains("ext_id"),
            "h transitively reaches ext_id: {deps:?}"
        );
        assert!(!deps.contains("h"));
    }

    // #52 honesty / AC-4: a pure caller of only a `spec fn` has NO `fn` deps, so
    // nothing is woven and no external_body is ever emitted (the corpus `sum`
    // shape). A `spec fn` is EXCLUDED from the returned set (woven separately).
    #[test]
    fn reachable_fns_excludes_spec_fns_and_is_empty_for_a_pure_caller() {
        let src = "\
spec fn spec_id(x: u32) -> u32 dec 0 { x }
fn f(x: u32) -> u32 req x < 100 ens result == x fx pure { spec_id(x) }";
        let deps = reachable_in_file_fns(&parse(src), "f");
        assert!(
            deps.is_empty(),
            "a caller of only a spec fn has no in-file `fn` deps (no external_body): {deps:?}"
        );
    }

    // #52 isolation (§5.3): a sibling `fn` NOT referenced is NOT woven — the
    // reachability is exactly the transitive closure, never all-in-file fns.
    #[test]
    fn reachable_fns_omits_an_unreferenced_sibling() {
        let src = "\
fn a(x: u32) -> u32 req x < 100 ens result == x fx pure { x }
fn b(x: u32) -> u32 req x < 100 ens result == x fx pure { x }";
        let deps = reachable_in_file_fns(&parse(src), "a");
        assert!(
            !deps.contains("b"),
            "a does not reference b → b is NOT woven (§5.3 isolation): {deps:?}"
        );
    }

    // #52 determinism (R-CODE-5): the same program yields the same dep set.
    #[test]
    fn reachable_fns_is_deterministic() {
        let src = "\
#[boundary(\"ext::ext_id\")] fn ext_id(x: u32) -> u32 req x < 100 ens result == x fx pure ;
fn g(x: u32) -> u32 req x < 100 ens result == x fx pure { ext_id(x) }
fn h(x: u32) -> u32 req x < 100 ens result == x fx pure { g(x) }";
        let prog = parse(src);
        assert_eq!(
            reachable_in_file_fns(&prog, "h"),
            reachable_in_file_fns(&prog, "h")
        );
    }
}
