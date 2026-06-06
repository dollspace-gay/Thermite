//! The SpecTherm validator — the boundary API that walks a parsed
//! `thermite-syntax` program's contract positions and enforces §4.2's "locked
//! cage": a contract may use ONLY registered combinators (right name + arity +
//! arg-kinds), declared `spec fn` calls, and the built-in operators / literals /
//! paths the grammar already sanctions — nothing else.
//!
//! Governing design: `.design/spec/spectherm-combinators.md` (REQ-3/4/5).
//! Verified against the oracle at `tests/golden/combinators/` (accept.json /
//! reject.json), R-CHAR-3.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-3 (validator accept rule) | SHIPPED | `pub fn validate` collects `spec fn` names then walks `Contract.req`/`ens`, `LoopNode.invs`/`dec`, `SpecFnItem.body`; accepts registered combinators (via `combinators::lookup`), declared spec-fn calls, and grammar built-ins. Every `accept.json` case validates clean (`tests/combinators_conformance.rs`). |
//! | REQ-4 (reject cases, structured `SpecError`) | SHIPPED | `enum SpecError` with `UnknownCombinator`/`WrongArity`/`WrongArgKind`/`ForbiddenCall`/`ExpressionTooDeep`; `validate` returns `Result<(), Vec<SpecError>>`, never panics. Every `reject.json` case yields the expected cause. |
//! | REQ-5 (bounded recursion — no overflow) | SHIPPED | a single `MAX_RECURSION_DEPTH` guard wraps EVERY recursive descent (`walk_expr`, closure bodies, match arms, index args, if/block tails) via `descend`; deep input yields `ExpressionTooDeep`, never an overflow (`validate_never_panics`). |
//! | REQ-6 (flat-closure-fragment rule — no anonymous nested quantifiers) | SHIPPED | `check_arg_kind`'s `Pred` arm sets `Validator::in_combinator_closure` for the whole closure-body descent (kept set through all nested sub-expressions/closures); while set, `walk_call` rejects any callee resolving via `combinators::lookup` with `SpecError::NestedCombinator`, while a declared `spec fn` call stays accepted. Consumer: `validate` → `walk_clause`/`walk_block` reach `walk_call`. Verification: `reject.json` `nested_combinator_in_closure` → `NestedCombinator`; `accept.json` `named_spec_fn_in_closure` → `Ok`; the flat corpus closures stay `Ok` (`tests/combinators_conformance.rs`). |
//!
//! ## Basis Stage 1a — the ADT validator GATE (`.design/basis/01-adts.md`)
//!
//! Stage 1a parses the ADT surface but does NOT yet validate it (REQ-5
//! exhaustiveness + REQ-6 well-formedness land in Stage 1b). The validator is
//! the honest 1a gate: it SCREAMS, not silently mis-processes.
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | 1a GATE (`SpecError::UnsupportedAdt`) | SHIPPED | `SpecError::UnsupportedAdt { construct, span }`; returned from `run`'s `Item::Struct`/`Item::Enum` arms and `walk_expr_inner`'s `Expr::StructLit`/`Expr::Is`/`Expr::Deref` arms. Consumer: `validate` (the boundary API `forge check` calls before lowering) — every ADT program PARSES then dies at this gate with a structured "ADTs not yet supported in the validator" error (the handled-or-loud scream). Stage 1b REPLACES these arms with real variant-set collection + exhaustiveness + well-formedness. |

use std::collections::HashSet;
use std::fmt;

use thermite_syntax::{Block, Clause, Expr, IndexArg, Item, MatchArm, Program, Span, Stmt};

use crate::combinators::{self, ArgKind, CombinatorSig};

/// The maximum recursive-descent nesting depth the validator will follow before
/// returning an `ExpressionTooDeep` diagnostic. A fixed constant for determinism
/// (R-CODE-5), mirroring `thermite-syntax`'s parser `MAX_RECURSION_DEPTH`.
///
/// This single bound guards EVERY recursive descent in the walk — nested
/// combinator/spec-fn arguments, `Binary`/`Index`/`Cast`/`Ref`/`Field`
/// operands, closure bodies, `Match` scrutinee + arm bodies, `If` branches, and
/// block statements/tails — so a pathological deeply-nested contract surfaces a
/// structured error rather than overflowing the native stack and aborting the
/// process (REQ-5; the #29/#31/#32 expr-only-guard lesson: do not leave any
/// recursive path unbounded).
const MAX_RECURSION_DEPTH: usize = 64;

/// The bounded set of built-in `MethodCall` names a CAGED position admits
/// (REQ-3(c): "the bounded built-in `MethodCall`s the grammar admits (e.g.
/// `xs.len()`)"). Any method name outside this set in a contract position is a
/// `ForbiddenCall` (REQ-4 (iv)) — the §4.2 cage is closed.
///
/// v0.1 set = `len` only: it is the single method the conformance corpus uses
/// in any contract position (`haystack.len()` in `binary_search.th`; `xs.len()`
/// in `sum.th`'s `req`/`inv`/`dec`). No other built-in method is added — per
/// REQ-1's frozen-set discipline and anti-goal §11, the set grows only by
/// design amendment from a corpus need, never speculatively.
const BUILTIN_METHODS: &[&str] = &["len"];

/// `thermite-spec`'s own error enum (workspace.md REQ-3), born with this first
/// fallible function. Span-bearing (reusing `thermite_syntax::Span`) so
/// diagnostics are crisp (pillar 4); `Display`-able. The validator NEVER panics
/// (R-CODE-2 / R-APG-1) — every rejection is a variant here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecError {
    /// A call in a contract position whose callee is neither a registered
    /// combinator nor a declared `spec fn` — an arbitrary free-function call,
    /// forbidden by the §4.2 cage (REQ-4 (i)). `name` is the unresolved callee.
    UnknownCombinator { name: String, span: Span },
    /// A registered combinator called with the wrong number of arguments
    /// (REQ-4 (ii)).
    WrongArity {
        name: String,
        expected: usize,
        found: usize,
        span: Span,
    },
    /// A registered combinator whose positional argument has the wrong kind —
    /// e.g. a non-closure where a `Pred` is required (REQ-4 (iii)). `position`
    /// is 0-based.
    WrongArgKind {
        name: String,
        position: usize,
        expected: ArgKind,
        span: Span,
    },
    /// A construct the contract sublanguage forbids that nonetheless parsed —
    /// e.g. a `MethodCall` whose callee is not a grammar built-in, or a non-call
    /// callee shape (REQ-4 (iv)). Distinct from `UnknownCombinator` (a free
    /// `Expr::Call`) so the diagnostic names the construct precisely.
    ForbiddenCall { detail: String, span: Span },
    /// A registered combinator call appearing INSIDE another combinator's
    /// predicate-closure body — an anonymous nested quantifier (REQ-6). The
    /// flat-closure-fragment rule forbids it: a combinator's `Pred`-slot closure
    /// body is a FLAT predicate (comparisons, arithmetic, boolean/logical ops,
    /// field/index, casts/refs, literals/paths, bounded built-in method calls,
    /// `Match`/`If`, and NAMED `spec fn` calls) and MAY NOT compose another
    /// bounded quantifier. The sanctioned alternative is extracting a NAMED
    /// `spec fn` (each `dec`-measured and auditable). `name` is the nested
    /// combinator. Distinct from `UnknownCombinator` (a free call resolving to
    /// nothing) and `ForbiddenCall` (a generic forbidden construct) so the
    /// diagnostic can say "extract a named `spec fn`" (§4.2; issue #40).
    NestedCombinator { name: String, span: Span },
    /// A contract expression nested past `MAX_RECURSION_DEPTH` — surfaced as a
    /// structured diagnostic so external input can never overflow the stack
    /// (REQ-5).
    ExpressionTooDeep { limit: usize, span: Span },
    /// An ADT surface construct (`struct`/`enum` item, struct-literal
    /// construction, `is` discrimination, or a `Box` deref) reached the
    /// validator before the validator knows how to check it
    /// (`.design/basis/01-adts.md`). Basis Stage 1a parses the ADT surface but
    /// does NOT yet validate it: this is the honest "handled-or-loud" SCREAM
    /// (the unifying principle of `01-adts.md`) — an ADT program PARSES, then
    /// gets a clean, structured refusal at validation rather than being silently
    /// mis-processed downstream. Stage 1b REPLACES this gate with real
    /// exhaustiveness (REQ-5) + well-formedness (REQ-6) checking. `construct`
    /// names the unsupported surface form for a crisp diagnostic (§2.4).
    UnsupportedAdt { construct: &'static str, span: Span },
}

impl SpecError {
    /// The source span this diagnostic points at.
    pub fn span(&self) -> Span {
        match self {
            SpecError::UnknownCombinator { span, .. }
            | SpecError::WrongArity { span, .. }
            | SpecError::WrongArgKind { span, .. }
            | SpecError::ForbiddenCall { span, .. }
            | SpecError::NestedCombinator { span, .. }
            | SpecError::ExpressionTooDeep { span, .. }
            | SpecError::UnsupportedAdt { span, .. } => *span,
        }
    }
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecError::UnknownCombinator { name, .. } => write!(
                f,
                "`{name}` is not a registered SpecTherm combinator or a declared `spec fn`; \
                 contracts admit only the frozen combinator set (§4.2)"
            ),
            SpecError::WrongArity {
                name,
                expected,
                found,
                ..
            } => write!(
                f,
                "combinator `{name}` expects {expected} argument(s), found {found}"
            ),
            SpecError::WrongArgKind {
                name,
                position,
                expected,
                ..
            } => write!(
                f,
                "combinator `{name}` argument {position} must be of kind {expected:?}"
            ),
            SpecError::ForbiddenCall { detail, .. } => {
                write!(f, "construct not permitted in a contract: {detail}")
            }
            SpecError::NestedCombinator { name, .. } => write!(
                f,
                "combinator `{name}` may not appear inside another combinator's \
                 predicate-closure body — that body must be a FLAT predicate (REQ-6); \
                 express nested quantification through a named `spec fn` instead"
            ),
            SpecError::ExpressionTooDeep { limit, .. } => write!(
                f,
                "contract expression nested deeper than the validator limit of {limit}"
            ),
            SpecError::UnsupportedAdt { construct, .. } => write!(
                f,
                "ADTs are not yet supported in the validator: `{construct}` parses but is not \
                 validated in basis Stage 1a (`.design/basis/01-adts.md`); the exhaustiveness \
                 and well-formedness checks land in Stage 1b"
            ),
        }
    }
}

impl std::error::Error for SpecError {}

/// Validate every contract position of a parsed program against the SpecTherm
/// cage (REQ-3). Returns `Ok(())` if every contract expression is accepted, else
/// `Err` with one `SpecError` per violation (accumulated, not first-stop, for
/// crisp feedback, §2.4). NEVER panics (REQ-4/REQ-5).
///
/// This is `thermite-spec`'s boundary API: the validator is the registry's first
/// production consumer (AC-5, via `combinators::lookup`), and is the gate
/// `thermite-lower` (#4) and `forge` (#6) call before lowering / the vacuity
/// battery.
pub fn validate(program: &Program) -> Result<(), Vec<SpecError>> {
    let mut v = Validator::new(program);
    v.run(program);
    if v.errors.is_empty() {
        Ok(())
    } else {
        Err(v.errors)
    }
}

/// The walk state: the declared `spec fn` name set, the current recursion depth,
/// the accumulated diagnostics, and the "caged-flat" mode flag (REQ-6).
struct Validator {
    spec_fns: HashSet<String>,
    depth: usize,
    errors: Vec<SpecError>,
    /// REQ-6 flat-closure-fragment mode. Set ONCE on entry to a combinator's
    /// `Pred`-slot closure body and kept set for ALL nested sub-expressions and
    /// nested closures within it. While set, a call resolving to a registered
    /// combinator (`combinators::lookup(name).is_some()`) is REJECTED with
    /// `NestedCombinator` (an anonymous nested quantifier); a NAMED `spec fn`
    /// call stays accepted (named composition). In a top-level contract position
    /// (flag clear) a combinator call is accepted as before (REQ-3 (a)).
    in_combinator_closure: bool,
}

impl Validator {
    fn new(program: &Program) -> Self {
        // Collect every declared `spec fn` name first so a forward reference in
        // a contract (`ens result == sz(xs)` before `spec fn sz` is seen) still
        // resolves (REQ-3 (b)).
        let spec_fns = program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::SpecFn(s) => Some(s.name.clone()),
                Item::Fn(_) => None,
                // A `struct`/`enum` item declares no `spec fn` name
                // (`.design/basis/01-adts.md`) — neutral value `None`. The
                // item itself is gated at `run` (the `UnsupportedAdt` scream);
                // here it simply contributes no callable spec-fn name.
                Item::Struct(_) | Item::Enum(_) => None,
            })
            .collect();
        Validator {
            spec_fns,
            depth: 0,
            errors: Vec::new(),
            in_combinator_closure: false,
        }
    }

    /// Walk every contract position of every item.
    fn run(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                Item::Fn(f) => {
                    self.walk_clause(&f.contract.req);
                    for clause in &f.contract.ens {
                        self.walk_clause(clause);
                    }
                    // REQ-3: a `fn` BODY is executable surface code, NOT a
                    // contract position. We traverse it STRUCTURALLY only — to
                    // find nested `LoopNode`s and cage each loop's `invs`/`dec`
                    // (the only contract positions inside a body). The body's
                    // other expressions (`return Some(mid)`, `haystack[mid]`,
                    // assignments, …) are surface code and are NOT cage-checked.
                    // A boundary fn (ffi-boundary.md REQ-2) has `body: None` — the
                    // body is foreign, so there are no in-language loops to scan
                    // for caged `inv`/`dec` clauses. Its `req`/`ens` (walked above)
                    // are still fully caged. An in-language fn's body is scanned
                    // structurally as before.
                    if let Some(body) = &f.body {
                        self.scan_block_for_loops(body);
                    }
                }
                Item::SpecFn(s) => {
                    // A `spec fn` body is itself a contract-position expression
                    // tree (REQ-3) — fully caged; its `dec` measure is a clause.
                    self.walk_clause(&s.dec);
                    self.walk_block(&s.body, s.span);
                }
                // Basis Stage 1a: a `struct`/`enum` item PARSES but the
                // validator does not yet check it. This is the honest
                // handled-or-loud SCREAM (`.design/basis/01-adts.md` — REQ-5/
                // REQ-6 land in 1b): the validator GATES every ADT program here
                // with a structured `UnsupportedAdt` rather than letting an
                // unvalidated ADT flow into lowering. 1b REPLACES these arms
                // with real variant-set collection + exhaustiveness +
                // well-formedness checking.
                Item::Struct(s) => {
                    self.errors.push(SpecError::UnsupportedAdt {
                        construct: "struct item",
                        span: s.span,
                    });
                }
                Item::Enum(e) => {
                    self.errors.push(SpecError::UnsupportedAdt {
                        construct: "enum item",
                        span: e.span,
                    });
                }
            }
        }
    }

    /// Run `inner` one recursion level deeper, returning `false` (and recording
    /// an `ExpressionTooDeep` at `span`) if the limit is hit. The SINGLE shared
    /// guard for every recursive descent (REQ-5). `span` is the enclosing
    /// clause/item span (the AST does not carry per-`Expr` spans).
    fn descend(&mut self, span: Span, inner: impl FnOnce(&mut Self)) {
        if self.depth >= MAX_RECURSION_DEPTH {
            self.errors.push(SpecError::ExpressionTooDeep {
                limit: MAX_RECURSION_DEPTH,
                span,
            });
            return;
        }
        self.depth += 1;
        inner(self);
        self.depth -= 1;
    }

    /// Walk a contract clause (`req`/`ens`/`inv`/`dec`): its expression must be
    /// accepted by the cage rule. The clause span anchors any diagnostic.
    fn walk_clause(&mut self, clause: &Clause) {
        let span = clause.span;
        self.walk_expr(&clause.expr, span);
    }

    /// STRUCTURAL traversal of a (non-caged) `fn` body block (REQ-3): descend
    /// through statements / nested blocks / `if` / `loop` ONLY to FIND nested
    /// `LoopNode`s and cage each loop's `invs`/`dec` (recursively, for loops
    /// nested in loops). The block's own expressions — calls like `Some(mid)`,
    /// `return None`, assignments, `haystack[mid]` — are executable surface code
    /// and are NOT cage-checked here. This is the counterpart to the caged
    /// `walk_block` (used for `spec fn` bodies and caged sub-expressions): same
    /// shape walk, but it cage-checks NOTHING except the loop contract clauses it
    /// discovers.
    fn scan_block_for_loops(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.scan_stmt_for_loops(stmt);
        }
        if let Some(tail) = &block.tail {
            self.scan_expr_for_loops(tail);
        }
    }

    /// STRUCTURAL traversal of a `fn`-body statement: cage the `invs`/`dec` of
    /// any nested loop (the only contract positions in a body) and keep
    /// descending through control flow to find deeper loops. Surface expressions
    /// are descended into ONLY to reach nested loops (e.g. a `loop` inside an
    /// `if` block), never cage-checked.
    fn scan_stmt_for_loops(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Loop(loop_node) => {
                // The loop's `invs`/`dec` ARE contract positions — cage them.
                for inv in &loop_node.invs {
                    self.walk_clause(inv);
                }
                self.walk_clause(&loop_node.dec);
                // The loop BODY is still executable surface code: scan it
                // structurally for further nested loops, do not cage it.
                self.scan_block_for_loops(&loop_node.body);
            }
            Stmt::Let { init, .. } => self.scan_expr_for_loops(init),
            Stmt::Assign { target, value } => {
                self.scan_expr_for_loops(target);
                self.scan_expr_for_loops(value);
            }
            Stmt::Return(Some(e)) | Stmt::Expr(e) => self.scan_expr_for_loops(e),
            Stmt::Return(None) => {}
            Stmt::If { cond, then, else_ } => {
                self.scan_expr_for_loops(cond);
                self.scan_block_for_loops(then);
                if let Some(else_block) = else_ {
                    self.scan_block_for_loops(else_block);
                }
            }
        }
    }

    /// STRUCTURAL traversal of a `fn`-body expression: descend ONLY into the
    /// sub-expressions/blocks that can themselves contain a nested `loop` (an
    /// `if`/`match` arm body), so a loop nested inside an expression is still
    /// found and its contract caged. The expression itself is surface code and
    /// is NOT cage-checked.
    fn scan_expr_for_loops(&mut self, expr: &Expr) {
        match expr {
            Expr::If { cond, then, else_ } => {
                self.scan_expr_for_loops(cond);
                self.scan_block_for_loops(then);
                self.scan_block_for_loops(else_);
            }
            Expr::Match { scrutinee, arms } => {
                self.scan_expr_for_loops(scrutinee);
                for MatchArm { body, .. } in arms {
                    self.scan_expr_for_loops(body);
                }
            }
            Expr::Call { args, .. } => {
                for arg in args {
                    self.scan_expr_for_loops(arg);
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                self.scan_expr_for_loops(receiver);
                for arg in args {
                    self.scan_expr_for_loops(arg);
                }
            }
            Expr::Field { receiver, .. } => self.scan_expr_for_loops(receiver),
            Expr::Binary { lhs, rhs, .. } => {
                self.scan_expr_for_loops(lhs);
                self.scan_expr_for_loops(rhs);
            }
            Expr::Index { base, index } => {
                self.scan_expr_for_loops(base);
                match index {
                    IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
                        self.scan_expr_for_loops(e)
                    }
                    IndexArg::Range(lo, hi) => {
                        self.scan_expr_for_loops(lo);
                        self.scan_expr_for_loops(hi);
                    }
                }
            }
            Expr::Cast { expr: inner, .. } | Expr::Ref { expr: inner, .. } => {
                self.scan_expr_for_loops(inner)
            }
            Expr::Closure { body, .. } => self.scan_expr_for_loops(body),
            // Basis Stage 1a ADT expressions (`.design/basis/01-adts.md`): this
            // is the STRUCTURAL fn-body walk that only descends to find nested
            // loops — it cage-checks nothing. The ADT program dies at the
            // validator's item/contract gate (`UnsupportedAdt`) regardless;
            // here we simply descend into sub-expressions that could hold a
            // nested `loop` so the structural invariant (every loop's contract
            // is reachable) is preserved.
            Expr::StructLit { fields, .. } => {
                for (_, value) in fields {
                    self.scan_expr_for_loops(value);
                }
            }
            Expr::Is { scrutinee, .. } => self.scan_expr_for_loops(scrutinee),
            Expr::Deref(inner) => self.scan_expr_for_loops(inner),
            // Leaves — no nested loop possible.
            Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) => {}
        }
    }

    /// Walk a CAGED block (a `spec fn` body, or a block nested inside a caged
    /// expression such as an `if`'s arm): every statement expression and the
    /// tail expression IS a contract-position expression and is cage-checked.
    /// Any `loop`/`while` it contains carries its own `invs`/`dec` clauses.
    fn walk_block(&mut self, block: &Block, span: Span) {
        self.descend(span, |s| {
            for stmt in &block.stmts {
                s.walk_stmt(stmt, span);
            }
            if let Some(tail) = &block.tail {
                s.walk_expr(tail, span);
            }
        });
    }

    /// Walk a statement, descending into nested loops (which carry their own
    /// `invs`/`dec` contract clauses) and the expressions they hold.
    fn walk_stmt(&mut self, stmt: &Stmt, span: Span) {
        match stmt {
            Stmt::Loop(loop_node) => {
                for inv in &loop_node.invs {
                    self.walk_clause(inv);
                }
                self.walk_clause(&loop_node.dec);
                self.walk_block(&loop_node.body, loop_node.span);
            }
            Stmt::Let { init, .. } => self.walk_expr(init, span),
            Stmt::Assign { target, value } => {
                self.walk_expr(target, span);
                self.walk_expr(value, span);
            }
            Stmt::Return(Some(e)) | Stmt::Expr(e) => self.walk_expr(e, span),
            Stmt::Return(None) => {}
            Stmt::If { cond, then, else_ } => {
                self.walk_expr(cond, span);
                self.walk_block(then, span);
                if let Some(else_block) = else_ {
                    self.walk_block(else_block, span);
                }
            }
        }
    }

    /// The accept rule (REQ-3) applied at one expression node, recursing into
    /// sub-expressions under the shared depth guard (REQ-5). `span` is the
    /// enclosing clause/item span used for any diagnostic.
    fn walk_expr(&mut self, expr: &Expr, span: Span) {
        self.descend(span, |s| s.walk_expr_inner(expr, span));
    }

    fn walk_expr_inner(&mut self, expr: &Expr, span: Span) {
        match expr {
            // (c) grammar built-ins: literals and paths are leaves.
            Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) => {}

            // (a)/(b)/(iv): a free call is a combinator, a spec-fn call, or
            // forbidden.
            Expr::Call { callee, args } => self.walk_call(callee, args, span),

            // (c) bounded built-in method calls. REQ-3(c) admits only "the
            // bounded built-in `MethodCall`s the grammar admits (e.g.
            // `xs.len()`)" — NOT an arbitrary method name. A non-allowlisted
            // method name in a caged position is forbidden (REQ-4 (iv) ->
            // `ForbiddenCall`). The allowlist is `BUILTIN_METHODS`; a permitted
            // method's receiver and args are recursed into.
            Expr::MethodCall {
                receiver,
                name,
                args,
            } => {
                if !BUILTIN_METHODS.contains(&name.as_str()) {
                    self.errors.push(SpecError::ForbiddenCall {
                        detail: format!(
                            "`.{name}()` is not a bounded built-in method permitted in a \
                             contract (only {BUILTIN_METHODS:?})"
                        ),
                        span,
                    });
                }
                // Recurse operands regardless so deep/forbidden nested content
                // still surfaces (REQ-5), even on a rejected method name.
                self.walk_expr(receiver, span);
                for arg in args {
                    self.walk_expr(arg, span);
                }
            }

            // (c) field access, binary, index, cast, ref — structural built-ins.
            Expr::Field { receiver, .. } => self.walk_expr(receiver, span),
            Expr::Binary { lhs, rhs, .. } => {
                self.walk_expr(lhs, span);
                self.walk_expr(rhs, span);
            }
            Expr::Index { base, index } => {
                self.walk_expr(base, span);
                self.walk_index(index, span);
            }
            Expr::Cast { expr: inner, .. } => self.walk_expr(inner, span),
            Expr::Ref { expr: inner, .. } => self.walk_expr(inner, span),

            // (c) match / if — built-in control forms; recurse into all sub-exprs.
            Expr::Match { scrutinee, arms } => {
                self.walk_expr(scrutinee, span);
                for MatchArm { body, .. } in arms {
                    self.walk_expr(body, span);
                }
            }
            Expr::If { cond, then, else_ } => {
                self.walk_expr(cond, span);
                self.walk_block(then, span);
                self.walk_block(else_, span);
            }

            // A bare closure outside a `Pred` argument slot has no meaning in a
            // contract position (a combinator's `Pred` arg is handled in
            // `walk_call`). We still recurse the body so a deeply-nested body is
            // bounded, but flag the misplaced closure.
            Expr::Closure { body, .. } => {
                self.errors.push(SpecError::ForbiddenCall {
                    detail: "a closure may appear only as a combinator predicate argument"
                        .to_string(),
                    span,
                });
                self.walk_expr(body, span);
            }

            // Basis Stage 1a (`.design/basis/01-adts.md`): the ADT contract
            // expressions PARSE but the validator does not yet check them — it
            // SCREAMS `UnsupportedAdt` (REQ-6's `is` well-formedness + the
            // struct-literal/deref handling land in 1b/1c). Recurse operands
            // regardless so deep/forbidden nested content still surfaces (REQ-5)
            // and the program dies loudly at validation, never silently.
            Expr::StructLit { fields, .. } => {
                self.errors.push(SpecError::UnsupportedAdt {
                    construct: "struct literal",
                    span,
                });
                for (_, value) in fields {
                    self.walk_expr(value, span);
                }
            }
            Expr::Is { scrutinee, .. } => {
                self.errors.push(SpecError::UnsupportedAdt {
                    construct: "`is` discrimination",
                    span,
                });
                self.walk_expr(scrutinee, span);
            }
            Expr::Deref(inner) => {
                self.errors.push(SpecError::UnsupportedAdt {
                    construct: "`Box` deref",
                    span,
                });
                self.walk_expr(inner, span);
            }
        }
    }

    /// Walk an index argument (`a[i]`, `a[..i]`, `a[i..]`, `a[i..j]`) — each
    /// bound is a sub-expression, guarded by the shared depth counter (REQ-5).
    fn walk_index(&mut self, index: &IndexArg, span: Span) {
        match index {
            IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
                self.walk_expr(e, span)
            }
            IndexArg::Range(lo, hi) => {
                self.walk_expr(lo, span);
                self.walk_expr(hi, span);
            }
        }
    }

    /// Resolve a free `Expr::Call` callee against the cage (REQ-3 (a)/(b),
    /// REQ-4). The callee is expected to be a single-segment `Path`.
    fn walk_call(&mut self, callee: &Expr, args: &[Expr], span: Span) {
        let name = match callee {
            Expr::Path(segments) if segments.len() == 1 => &segments[0],
            // A path with `::` segments or a non-path callee is not a combinator
            // or spec-fn call the grammar admits in a contract (REQ-4 (iv)).
            _ => {
                self.errors.push(SpecError::ForbiddenCall {
                    detail: "a contract call's callee must be a bare combinator or `spec fn` name"
                        .to_string(),
                    span,
                });
                // Still recurse args so nested forbidden/deep content surfaces.
                for arg in args {
                    self.walk_expr(arg, span);
                }
                return;
            }
        };

        if let Some(sig) = combinators::lookup(name) {
            if self.in_combinator_closure {
                // REQ-6: a combinator call inside another combinator's
                // predicate-closure body is an anonymous nested quantifier —
                // forbidden. The discriminator is EXACTLY `combinators::lookup`
                // succeeding (the same test that ACCEPTS this callee in a
                // top-level contract position); the verdict is context-dependent.
                self.errors.push(SpecError::NestedCombinator {
                    name: name.clone(),
                    span,
                });
                // Still recurse the args (staying in caged-flat mode) so deeper
                // nested combinators / forbidden / too-deep content also surfaces
                // (REQ-5), and so a doubly-nested combinator is reported too.
                for arg in args {
                    self.walk_expr(arg, span);
                }
            } else {
                self.check_combinator(sig, args, span);
            }
        } else if self.spec_fns.contains(name) {
            // (b) a declared spec-fn call: accept; its arguments are ordinary
            // contract expressions (recursed, depth-guarded).
            for arg in args {
                self.walk_expr(arg, span);
            }
        } else {
            // (i) neither a combinator nor a declared spec fn — forbidden.
            self.errors.push(SpecError::UnknownCombinator {
                name: name.clone(),
                span,
            });
            for arg in args {
                self.walk_expr(arg, span);
            }
        }
    }

    /// Check a registered combinator call: arity (REQ-4 (ii)) then each
    /// argument's kind (REQ-4 (iii)), recursing into argument sub-expressions.
    fn check_combinator(&mut self, sig: &CombinatorSig, args: &[Expr], span: Span) {
        if args.len() != sig.arity {
            self.errors.push(SpecError::WrongArity {
                name: sig.name.to_string(),
                expected: sig.arity,
                found: args.len(),
                span,
            });
            // Arity is wrong; still recurse the supplied args (depth guard,
            // nested-content surfacing) but skip per-position kind checks (the
            // positions don't line up).
            for arg in args {
                self.walk_expr(arg, span);
            }
            return;
        }

        for (position, (arg, kind)) in args.iter().zip(sig.arg_kinds.iter()).enumerate() {
            self.check_arg_kind(sig.name, position, *kind, arg, span);
        }
    }

    /// Check one positional argument against its expected `ArgKind` (REQ-4
    /// (iii)), then recurse into the argument's sub-expressions.
    ///
    /// Per OQ-3, only `Pred` is syntactically decidable (MUST be `Expr::Closure`);
    /// `Slice`/`Index`/`Value` are checked shallowly: any NON-closure expression
    /// is accepted in those positions (a closure there is the only decidable
    /// error), with full typing deferred to a later pass (not a v0.1 item).
    fn check_arg_kind(
        &mut self,
        name: &'static str,
        position: usize,
        kind: ArgKind,
        arg: &Expr,
        span: Span,
    ) {
        match kind {
            ArgKind::Pred => match arg {
                // A `Pred` slot is satisfied by a closure literal (the one
                // syntactically strict kind, OQ-3). Recurse into the closure
                // BODY — the legitimate contract sub-expression — rather than
                // the closure node (which `walk_expr` would flag as a misplaced
                // bare closure). This bounds the body's depth too (REQ-5).
                //
                // REQ-6: enter "caged-flat" mode for the body. Set ONCE here and
                // keep it set for the entire body descent (all nested
                // sub-expressions AND any nested closure), then restore so a
                // sibling top-level `Pred` slot is checked independently. Inside
                // this mode a registered-combinator call is rejected with
                // `NestedCombinator` (see `walk_call`); a named `spec fn` call
                // stays accepted (named composition is the sanctioned alternative).
                // The save/restore makes re-entry a harmless no-op (a nested
                // `Pred` body's `|y|` re-sets an already-set flag).
                Expr::Closure { body, .. } => {
                    let saved = self.in_combinator_closure;
                    self.in_combinator_closure = true;
                    self.walk_expr(body, span);
                    self.in_combinator_closure = saved;
                }
                _ => {
                    self.errors.push(SpecError::WrongArgKind {
                        name: name.to_string(),
                        position,
                        expected: ArgKind::Pred,
                        span,
                    });
                    // A non-closure in a Pred slot is still an expression we
                    // recurse for deep/forbidden nested content (REQ-5).
                    self.walk_expr(arg, span);
                }
            },
            ArgKind::Slice | ArgKind::Index | ArgKind::Value => {
                if matches!(arg, Expr::Closure { .. }) {
                    // A closure in a non-Pred slot is decidably wrong; emit the
                    // kind error (the recursion below also flags the bare
                    // closure, but the precise kind diagnostic is the primary).
                    self.errors.push(SpecError::WrongArgKind {
                        name: name.to_string(),
                        position,
                        expected: kind,
                        span,
                    });
                }
                // Recurse into the argument (a `Slice`'s index expression, a
                // `Value`'s operands, etc.) so deep/forbidden nested content is
                // bounded and surfaced (REQ-5).
                self.walk_expr(arg, span);
            }
        }
    }
}
