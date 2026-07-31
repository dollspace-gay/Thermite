//! `forge/src/closure.rs` — the §9 end-to-end vs to-the-boundary classification
//! (issue #17, `.design/forge/e2e-vs-boundary.md`; `thermite-design.md` §9).
//!
//! `thermite-design.md` §9 promises the manifest distinguishes "verified to the
//! boundary" from "verified, period". This module computes, for each `fn` in a
//! parsed file, its transitive intra-file call closure and classifies the
//! function's assurance scope:
//!
//! - End-to-end ("verified, period"): nothing in the closure is a
//!   `#[boundary]` (foreign, unproven body, #16) or `#[slag]` (trusted-by-fiat
//!   body, #6) fn. Every link is a proved / `spec fn` / combinator body, so the
//!   whole-program guarantee rests only on the toolchain.
//! - To-the-boundary ("verified to the boundary"): the closure transitively
//!   reaches a `#[boundary]` or `#[slag]` fn. The fn's own contract is verified,
//!   but the end-to-end guarantee crosses a foreign/unproven body at the crossing
//!   (`goal.md` R-DEFER-9: mark a guarantee that depends on an unproven
//!   foreign body; do not claim end-to-end when a boundary is reached).
//!
//! The analysis is pure and structural: it owns no prover invocation and changes
//! no verdict. It layers the §9 composition rule on top of the per-fn verdicts
//! `check::check_file` already produced; the result is recorded as the additive
//! `Certificate::assurance_scope` field ([`crate::manifest::AssuranceScope`]).
//!
//! ## What is pure, what is a crossing (`.design/forge/e2e-vs-boundary.md`)
//!
//! - Nodes are the file's `Item::Fn` and `Item::SpecFn`.
//! - Edges come from walking each `fn` body's `Expr::Call` / `Expr::MethodCall`
//!   (and every nested expression / statement) and resolving the callee by name.
//! - A callee resolving to an in-file `Item::SpecFn` is pure (a `spec fn` is total
//!   / terminating / body-Thermite-verified, §4.2; never a crossing, even when
//!   self-recursive like `spec_sum`).
//! - A callee resolving to a registry combinator (`forall_in`, `sorted`, … — the
//!   `thermite_spec` set, §4.2) is pure (a frozen-trigger proved library).
//! - A callee resolving to an in-file `Item::Fn` that is `#[boundary]` / `#[slag]`
//!   is a crossing; an in-file pure `Item::Fn` inherits its closure.
//! - A callee resolving to nothing in-file and not a combinator (a cross-file
//!   callee) is pure and ignored, not a crossing (OQ-1: cross-file resolution is a
//!   documented v0.1 limitation).
//!
//! The walk is cycle-safe (a visited set keyed by fn name; recursion does not
//! loop) and bounded (each node touched once), so it is O(nodes + edges) and
//! deterministic (R-CODE-5: a pure function of the parsed `Program`, no wall-clock
//! / unordered iteration in the verdict; the `via` crossing is the first reached
//! in source order, and the result is keyed in a `BTreeMap`).
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=forge-closure-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-CLOSURE-CALLGRAPH | shipped | `forge/src/closure.rs` | Cycle-safe transitive call closure |  |
//! | REQ-FORGE-CLOSURE-CERT-FIELD | shipped | `forge/src/closure.rs` | Per-function assurance scope certificate field |  |
//! | REQ-FORGE-CLOSURE-DETERMINISM | shipped | `forge/src/closure.rs` | Deterministic assurance closure results |  |
//! | REQ-FORGE-CLOSURE-PROJECT-CLAIM | shipped | `forge/src/closure.rs` | Closure feeds project-level assurance scope |  |
//! | REQ-FORGE-CLOSURE-SCOPE-ORTHOGONAL | shipped | `forge/src/closure.rs` | Assurance scope is orthogonal to level |  |
//! | REQ-FORGE-CLOSURE-SCOPE-RULE | shipped | `forge/src/closure.rs` | End-to-end versus to-boundary rule |  |
//! <!-- /generated:reqs -->
//!
//! ## #52 reuse note (`.design/lower/boundary-composition.md`)
//!
//! The private `CallGraph::from_program` + the cycle-safe DFS are also
//! consumed by the §9 boundary-composition weaving (#52): `pub fn
//! reachable_in_file_fns` reuses the same walker (a new `CallGraph::reachable_fns`
//! DFS sibling of `reach_crossing`) to return every in-file `Item::Fn` a caller
//! transitively references, which `check::item_subprogram` weaves into the
//! caller's §5.3 sub-program (regular fns with their body, boundary/slag fns
//! as `#[verifier::external_body]` signatures). No walker is duplicated.
//!
//! ## Cluster C10 — ergonomics ripple (`.design/basis/11-ergonomics.md`, #112)
//!
//! <!-- generated:reqs view=forge-closure-ergonomics-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-CLOSURE-MATCH-GUARD | shipped | `forge/src/closure.rs` | Closure walks match-arm guards |  |
//! <!-- /generated:reqs -->

use std::collections::{BTreeMap, BTreeSet};

use thermite_syntax::{Block, Expr, IndexArg, Item, Program, Stmt};

use crate::manifest::AssuranceScope;

/// One node's classification input: whether it is itself a crossing
/// (`#[boundary]` / `#[slag]`) and the in-file callees its body reaches (the
/// out-edges), in source order. A `spec fn` has `is_crossing = false` and is
/// never a crossing (§4.2); a boundary fn has `body: None`, so no out-edges.
struct Node {
    /// `true` iff this fn is itself a crossing: a `#[boundary]` (foreign body) or
    /// `#[slag]` (fiat-trusted body) fn. The `via` for any fn reaching it is this
    /// node's name.
    is_crossing: bool,
    /// `true` iff this node is an `Item::Fn` (a regular or `#[boundary]`/`#[slag]`
    /// fn), `false` for an `Item::SpecFn`. Lets [`CallGraph::reachable_fns`]
    /// return only `fn` dependencies (#52 weaving); a `spec fn` is woven
    /// separately by `check::item_subprogram`'s `spec_items` set.
    is_fn: bool,
    /// The in-file callee names this node's body calls directly, in source order
    /// (deterministic). Resolved in [`CallGraph::from_program`]; an unresolved /
    /// `spec fn` / combinator callee that is not an in-file `Item::Fn` node is
    /// dropped here (it can never reach a crossing, REQ-1 pure).
    callees: Vec<String>,
}

/// The intra-file call graph the classification walks (REQ-1). Keyed by fn name
/// in a `BTreeMap` for deterministic iteration. Only in-file `Item::Fn` /
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
        // callees). A call resolving to a name not here is cross-file / a
        // combinator / unknown → pure, dropped (OQ-1, REQ-1).
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
                    // A `spec fn` is pure (§4.2): never a crossing. Its callees
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
                // Forge-tier item (stage1-forge-tier.md REQ-3): no v1 call-graph
                // consumer yet (increments 2b-3); not a callable node — insert
                // nothing, mirroring the inert ADT-decl arm.
                Item::Forge(_) => {}
            }
        }
        CallGraph { nodes }
    }

    /// Every in-file fn name (`Item::Fn`, not `Item::SpecFn`) transitively
    /// referenced from `start`'s body, excluding `start` itself: a cycle-safe,
    /// bounded DFS over the same out-edges [`reach_crossing`] walks (#52
    /// composition weaving). Used by `check::item_subprogram` to weave a caller's
    /// regular-fn dependencies (body) and boundary/slag dependencies
    /// (external_body signature) into its §5.3 sub-program so `lower`/`verus`
    /// resolve every referenced callee.
    ///
    /// A `spec fn` is excluded here: spec fns are woven separately by
    /// `item_subprogram` (the existing `spec_items` set), so emitting them again
    /// would duplicate. The walk follows edges through every node (so a fn reached
    /// only via a spec-fn or boundary-fn intermediary is still found), but only
    /// `Item::Fn` names are returned. Returns a `BTreeSet` (sorted, stable,
    /// deterministic, R-CODE-5).
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

    /// The first crossing (`#[boundary]`/`#[slag]` fn) reachable from `start` in a
    /// cycle-safe, source-order DFS (REQ-1/REQ-2/REQ-6). Returns the crossing's
    /// name (`via`) or `None` when the whole transitive closure is pure.
    ///
    /// `start` itself counts: a `#[boundary]`/`#[slag]` fn is its own crossing
    /// (`via` is itself). The `visited` set is keyed by name so a self- or
    /// mutually-recursive node is entered once: the walk terminates and
    /// is bounded (each node touched once). Deterministic: callees are visited in
    /// source order, so the first reached crossing is stable.
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
                // An unresolved callee is not a crossing (`collect_in_file_calls`
                // only emits in-file names).
                continue;
            };
            if node.is_crossing {
                return Some(name);
            }
            // Push callees in reverse source order so they pop in source order
            // (the deterministic first-reached `via`, REQ-6).
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
/// REQ-2/REQ-6). Returns a `BTreeMap<fn name, AssuranceScope>` (sorted, stable,
/// deterministic): `AssuranceScope::EndToEnd` when the fn's transitive intra-file
/// call closure reaches no `#[boundary]`/`#[slag]` fn; `AssuranceScope::ToBoundary
/// { via }` recording the first reached crossing otherwise.
///
/// The classification is syntactic and orthogonal to the verification level
/// (REQ-5): it reads only the call graph, never a cert. A `#[boundary]`/`#[slag]`
/// fn is trivially `ToBoundary` (it is the crossing, `via` itself). It is a pure
/// function of the parsed `Program` (R-CODE-5).
///
/// Consumed by `check::check_file_with_options`, which attaches each fn's scope to
/// its certificate (`Certificate::assurance_scope`) alongside the achieved level.
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
/// references (excluding `start` itself), for the §9 boundary-composition
/// weaving (`.design/lower/boundary-composition.md` REQ-2, crosslink #52).
///
/// `check::item_subprogram` consumes this to build a caller `f`'s isolated §5.3
/// sub-program: every regular reachable fn is woven with its body (proved),
/// and every `#[boundary]`/`#[slag]` reachable fn is woven as a
/// `#[verifier::external_body]` signature (`thermite_lower::lower`), so `verus`
/// resolves the foreign callee and `f` proves through its contract (was an
/// undefined-callee L0). A `spec fn` is excluded here: it is woven separately by
/// `item_subprogram`'s existing `spec_items` set (no duplication); the transitive
/// walk still traverses through spec-fn / boundary-fn intermediaries so a fn
/// reached only via one is still found.
///
/// Reuses the private `CallGraph::from_program` + a cycle-safe, bounded,
/// source-order DFS (`reachable_fns`), the same walker `classify` uses (the #17
/// reachability seam), never a duplicate. Returns a `BTreeSet` (sorted, stable,
/// deterministic, R-CODE-5: a pure function of the parsed `Program`).
pub fn reachable_in_file_fns(program: &Program, start: &str) -> BTreeSet<String> {
    CallGraph::from_program(program).reachable_fns(start)
}

/// A complete, fail-closed dependency closure for an L3 artifact. Unlike the
/// legacy assurance classifier above, this surface never treats an unresolved
/// call as pure: unknown and indirect calls are errors before code emission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedClosure {
    pub roots: Vec<String>,
    pub functions: BTreeSet<String>,
    pub spec_functions: BTreeSet<String>,
    pub edges: BTreeSet<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifiedClosureError {
    UnknownExport { name: String },
    ExportNotFunction { name: String },
    UnresolvedCall { path: Vec<String>, callee: String },
    IndirectCall { path: Vec<String> },
}

impl std::fmt::Display for VerifiedClosureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifiedClosureError::UnknownExport { name } => {
                write!(f, "unknown verified-build export `{name}`")
            }
            VerifiedClosureError::ExportNotFunction { name } => {
                write!(
                    f,
                    "verified-build export `{name}` is not an executable `fn`"
                )
            }
            VerifiedClosureError::UnresolvedCall { path, callee } => write!(
                f,
                "incomplete verified-build closure: {} calls unresolved `{callee}`",
                path.join(" -> ")
            ),
            VerifiedClosureError::IndirectCall { path } => write!(
                f,
                "incomplete verified-build closure: {} contains an unsupported indirect call",
                path.join(" -> ")
            ),
        }
    }
}

/// Compute the executable and spec dependency closure rooted at `exports`.
/// Every free call is classified as an in-file node, a frozen language builtin,
/// or a hard error. The returned edge set and node sets are sorted and therefore
/// canonicalizable without depending on hash-map or traversal order.
pub fn verified_closure(
    program: &Program,
    exports: &[String],
) -> Result<VerifiedClosure, VerifiedClosureError> {
    let mut item_kinds: BTreeMap<String, bool> = BTreeMap::new();
    let mut variants = BTreeSet::new();
    for item in &program.items {
        match item {
            Item::Fn(f) => {
                item_kinds.insert(f.name.clone(), true);
            }
            Item::SpecFn(s) => {
                item_kinds.insert(s.name.clone(), false);
            }
            Item::Enum(e) => {
                for variant in &e.variants {
                    variants.insert(variant.name.clone());
                }
            }
            Item::Struct(_) | Item::Forge(_) => {}
        }
    }

    let mut roots = exports.to_vec();
    roots.sort();
    roots.dedup();
    for root in &roots {
        match item_kinds.get(root) {
            None => return Err(VerifiedClosureError::UnknownExport { name: root.clone() }),
            Some(false) => {
                return Err(VerifiedClosureError::ExportNotFunction { name: root.clone() })
            }
            Some(true) => {}
        }
    }

    let mut functions = BTreeSet::new();
    let mut spec_functions = BTreeSet::new();
    let mut edges = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut stack: Vec<(String, Vec<String>)> = roots
        .iter()
        .rev()
        .map(|root| (root.clone(), vec![root.clone()]))
        .collect();

    while let Some((name, path)) = stack.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        match item_kinds.get(&name) {
            Some(true) => {
                functions.insert(name.clone());
            }
            Some(false) => {
                spec_functions.insert(name.clone());
            }
            None => continue,
        }

        let Some(item) = program.items.iter().find(|item| item.name() == name) else {
            continue;
        };
        let calls = collect_verified_calls(item);
        for call in calls.into_iter().rev() {
            match call {
                VerifiedCall::Named(callee) if item_kinds.contains_key(&callee) => {
                    edges.insert((name.clone(), callee.clone()));
                    let mut callee_path = path.clone();
                    callee_path.push(callee.clone());
                    stack.push((callee, callee_path));
                }
                VerifiedCall::Named(callee) if is_verified_builtin(&callee, &variants) => {}
                VerifiedCall::Named(callee) => {
                    return Err(VerifiedClosureError::UnresolvedCall { path, callee })
                }
                VerifiedCall::Qualified(root) if is_verified_qualified_root(&root, program) => {}
                VerifiedCall::Qualified(callee) => {
                    return Err(VerifiedClosureError::UnresolvedCall { path, callee })
                }
                VerifiedCall::Indirect => return Err(VerifiedClosureError::IndirectCall { path }),
            }
        }
    }

    Ok(VerifiedClosure {
        roots,
        functions,
        spec_functions,
        edges,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum VerifiedCall {
    Named(String),
    Qualified(String),
    Indirect,
}

fn is_verified_builtin(name: &str, variants: &BTreeSet<String>) -> bool {
    thermite_spec::lookup(name).is_some()
        || thermite_spec::schemes::lookup(name).is_some()
        || variants.contains(name)
        || matches!(
            name,
            "Some"
                | "None"
                | "Ok"
                | "Err"
                | "parse_u64"
                | "bytes_eq"
                | "all_digits"
                | "is_digit"
                | "parse_be"
                | "pow10"
                | "occurs_at"
                | "contains_sub"
                | "count_sep"
                | "sep_free"
                | "is_space"
        )
}

fn is_verified_qualified_root(root: &str, program: &Program) -> bool {
    matches!(root, "u32" | "u64" | "usize" | "bool" | "Option" | "Result")
        || program.items.iter().any(|item| match item {
            Item::Struct(s) => s.name == root,
            Item::Enum(e) => e.name == root,
            _ => false,
        })
}

fn collect_verified_calls(item: &Item) -> Vec<VerifiedCall> {
    let mut calls = Vec::new();
    match item {
        Item::Fn(f) => {
            collect_verified_expr_calls(&f.contract.req.expr, &mut calls);
            for ens in &f.contract.ens {
                collect_verified_expr_calls(&ens.expr, &mut calls);
            }
            if let Some(dec) = &f.dec {
                collect_verified_expr_calls(&dec.expr, &mut calls);
            }
            if let Some(body) = &f.body {
                collect_verified_block_calls(body, &mut calls);
            }
        }
        Item::SpecFn(s) => {
            collect_verified_expr_calls(&s.dec.expr, &mut calls);
            collect_verified_block_calls(&s.body, &mut calls);
        }
        Item::Struct(s) => {
            if let Some(inv) = &s.inv {
                collect_verified_expr_calls(&inv.expr, &mut calls);
            }
        }
        Item::Enum(_) | Item::Forge(_) => {}
    }
    calls
}

fn collect_verified_block_calls(block: &Block, calls: &mut Vec<VerifiedCall>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { init, .. } => collect_verified_expr_calls(init, calls),
            Stmt::Assign { target, value } => {
                collect_verified_expr_calls(target, calls);
                collect_verified_expr_calls(value, calls);
            }
            Stmt::Return(value) => {
                if let Some(value) = value {
                    collect_verified_expr_calls(value, calls);
                }
            }
            Stmt::If { cond, then, else_ } => {
                collect_verified_expr_calls(cond, calls);
                collect_verified_block_calls(then, calls);
                if let Some(else_) = else_ {
                    collect_verified_block_calls(else_, calls);
                }
            }
            Stmt::Loop(node) => {
                for inv in &node.invs {
                    collect_verified_expr_calls(&inv.expr, calls);
                }
                collect_verified_expr_calls(&node.dec.expr, calls);
                if let thermite_syntax::LoopKind::While(cond) = &node.kind {
                    collect_verified_expr_calls(cond, calls);
                }
                collect_verified_block_calls(&node.body, calls);
            }
            Stmt::Expr(expr) => collect_verified_expr_calls(expr, calls),
            Stmt::Break | Stmt::Continue => {}
        }
    }
    if let Some(tail) = &block.tail {
        collect_verified_expr_calls(tail, calls);
    }
}

fn collect_verified_expr_calls(expr: &Expr, calls: &mut Vec<VerifiedCall>) {
    match expr {
        Expr::Call { callee, args } => {
            match callee.as_ref() {
                Expr::Path(parts) if parts.len() == 1 => {
                    if let Some(name) = parts.first() {
                        calls.push(VerifiedCall::Named(name.clone()));
                    }
                }
                Expr::Path(parts) => {
                    if let Some(root) = parts.first() {
                        calls.push(VerifiedCall::Qualified(root.clone()));
                    }
                }
                other => {
                    calls.push(VerifiedCall::Indirect);
                    collect_verified_expr_calls(other, calls);
                }
            }
            for arg in args {
                collect_verified_expr_calls(arg, calls);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_verified_expr_calls(receiver, calls);
            for arg in args {
                collect_verified_expr_calls(arg, calls);
            }
        }
        Expr::Field { receiver, .. } | Expr::TupleProj { receiver, .. } => {
            collect_verified_expr_calls(receiver, calls)
        }
        Expr::Closure { body, .. } => collect_verified_expr_calls(body, calls),
        Expr::Match { scrutinee, arms } => {
            collect_verified_expr_calls(scrutinee, calls);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_verified_expr_calls(guard, calls);
                }
                collect_verified_expr_calls(&arm.body, calls);
            }
        }
        Expr::If { cond, then, else_ } => {
            collect_verified_expr_calls(cond, calls);
            collect_verified_block_calls(then, calls);
            collect_verified_block_calls(else_, calls);
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_verified_expr_calls(lhs, calls);
            collect_verified_expr_calls(rhs, calls);
        }
        Expr::Index { base, index } => {
            collect_verified_expr_calls(base, calls);
            match index {
                IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
                    collect_verified_expr_calls(e, calls)
                }
                IndexArg::Range(a, b) => {
                    collect_verified_expr_calls(a, calls);
                    collect_verified_expr_calls(b, calls);
                }
            }
        }
        Expr::Cast { expr, .. }
        | Expr::Ref { expr, .. }
        | Expr::Deref(expr)
        | Expr::Unary { expr, .. }
        | Expr::Is {
            scrutinee: expr, ..
        } => collect_verified_expr_calls(expr, calls),
        Expr::StructLit { fields, .. } => {
            for (_, value) in fields {
                collect_verified_expr_calls(value, calls);
            }
        }
        Expr::Tuple(values) => {
            for value in values {
                collect_verified_expr_calls(value, calls);
            }
        }
        Expr::Quantifier { domain, body, .. } => {
            collect_verified_expr_calls(domain, calls);
            collect_verified_expr_calls(body, calls);
        }
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
    }
}

/// Walk a `Block` collecting the names of every in-file callee its expressions
/// reach (the out-edges for one node, in source order). A name resolving to an
/// in-file node (`in_file`) is kept; any other callee (a combinator, a cross-file
/// fn, an unknown, REQ-1 pure) is dropped. Deterministic: a depth-first walk in
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
            // / combinators only, all pure, but walk them anyway for
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
/// Every other expression is walked for nested calls (an `if`-condition call, a
/// call argument, a closure body, e.g. the closure body of `forall_in(xs, |x|
/// helper(x))`). Only a name in `in_file` is emitted (REQ-1 pure for the rest).
fn walk_expr(expr: &Expr, in_file: &BTreeSet<&str>, out: &mut Vec<String>) {
    match expr {
        Expr::Call { callee, args } => {
            // Resolve the callee name: the leading `Path` segment of `f(args)`
            // (the free-fn form). A non-`Path` callee (an indirect call) resolves
            // to no in-file node, so it is pure and ignored (OQ-1). Walk the callee
            // too so a nested call inside it is not missed.
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
        // expressions, but the call-graph walk descends into their
        // sub-expressions. An in-file call could sit in a struct-literal field
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
        // A raw quantified formula (`.design/stage2-stratified-cage.md` REQ-0): a
        // call out-edge can appear in the domain or the body.
        Expr::Quantifier { domain, body, .. } => {
            walk_expr(domain, in_file, out);
            walk_expr(body, in_file, out);
        }
        // Leaves: no nested call to find. A string literal
        // (`.design/basis/07-strings.md` REQ-1) is a leaf with no sub-expression and
        // no callee, so it contributes no out-edge.
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
    }
}

/// Emit `name` as an out-edge iff it resolves to an in-file node (REQ-1). A name
/// not in `in_file` (a combinator, a cross-file callee, an unknown) is pure and
/// dropped: it can never reach a crossing (OQ-1).
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

    // REQ-2 / AC-1: a pure-Thermite fn calling only a spec fn is end-to-end.
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

    // REQ-2 / AC-2: a direct boundary caller is to-the-boundary via the boundary;
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

    // REQ-2 / AC-4: a slag fn in the closure is to-the-boundary (slag == boundary
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

    // REQ-1 / AC-6: a self-recursive pure fn does not loop and is end-to-end
    // (recursion is not a crossing).
    #[test]
    fn self_recursive_pure_fn_is_end_to_end_and_terminates() {
        let src = "\
spec fn spec_sum(xs: &[u32]) -> u64 dec xs.len() { match xs { [] => 0, [head, ..t] => head as u64 + spec_sum(t), } }";
        let scopes = classify(&parse(src));
        assert_eq!(scopes.get("spec_sum"), Some(&AssuranceScope::EndToEnd));
    }

    // REQ-1 / AC-6: mutual recursion a -> b -> a terminates and (both pure) is
    // end-to-end.
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

    // OQ-1: an unresolved (cross-file) callee is pure and ignored, not a crossing.
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

    // #52 REQ-2 (transitive): h's sub-program weaves both g (body) and
    // ext_id (external_body); both are reachable `fn`s.
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

    // #52 / AC-4: a pure caller of only a `spec fn` has no `fn` deps, so
    // nothing is woven and no external_body is emitted (the corpus `sum`
    // shape). A `spec fn` is excluded from the returned set (woven separately).
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

    // #52 isolation (§5.3): a sibling `fn` not referenced is not woven; the
    // reachability is the transitive closure, never all-in-file fns.
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

    #[test]
    fn verified_closure_is_exact_sorted_and_omits_unreachable_siblings() {
        let program = parse(
            "fn helper(x: u64) -> u64 req true ens result == x fx pure { x } \
             fn root(x: u64) -> u64 req true ens result == x fx pure { helper(x) } \
             fn unrelated(x: u64) -> u64 req true ens result == x fx pure { x }",
        );
        let closure = verified_closure(&program, &["root".to_string()]).unwrap();
        assert_eq!(closure.roots, vec!["root"]);
        assert_eq!(
            closure.functions,
            BTreeSet::from(["helper".to_string(), "root".to_string()])
        );
        assert_eq!(
            closure.edges,
            BTreeSet::from([("root".to_string(), "helper".to_string())])
        );
        assert!(!closure.functions.contains("unrelated"));
    }

    #[test]
    fn verified_closure_fails_closed_on_unknown_and_indirect_calls() {
        let unresolved =
            parse("fn root(x: u64) -> u64 req true ens result == x fx pure { missing(x) }");
        assert!(matches!(
            verified_closure(&unresolved, &["root".to_string()]),
            Err(VerifiedClosureError::UnresolvedCall { callee, .. }) if callee == "missing"
        ));

        let indirect =
            parse("fn root(x: u64) -> u64 req true ens result == x fx pure { (|y| y)(x) }");
        assert!(matches!(
            verified_closure(&indirect, &["root".to_string()]),
            Err(VerifiedClosureError::IndirectCall { .. })
        ));
    }
}
