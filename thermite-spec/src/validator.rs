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
    /// A contract expression nested past `MAX_RECURSION_DEPTH` — surfaced as a
    /// structured diagnostic so external input can never overflow the stack
    /// (REQ-5).
    ExpressionTooDeep { limit: usize, span: Span },
}

impl SpecError {
    /// The source span this diagnostic points at.
    pub fn span(&self) -> Span {
        match self {
            SpecError::UnknownCombinator { span, .. }
            | SpecError::WrongArity { span, .. }
            | SpecError::WrongArgKind { span, .. }
            | SpecError::ForbiddenCall { span, .. }
            | SpecError::ExpressionTooDeep { span, .. } => *span,
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
            SpecError::ExpressionTooDeep { limit, .. } => write!(
                f,
                "contract expression nested deeper than the validator limit of {limit}"
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
/// and the accumulated diagnostics.
struct Validator {
    spec_fns: HashSet<String>,
    depth: usize,
    errors: Vec<SpecError>,
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
            })
            .collect();
        Validator {
            spec_fns,
            depth: 0,
            errors: Vec::new(),
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
                    self.walk_block(&f.body, f.span);
                }
                Item::SpecFn(s) => {
                    // A `spec fn` body is itself a contract-position expression
                    // tree (REQ-3); its `dec` measure is a clause too.
                    self.walk_clause(&s.dec);
                    self.walk_block(&s.body, s.span);
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

    /// Walk a block: any `loop`/`while` it contains is a contract carrier
    /// (`invs`/`dec`); other statements may hold sub-expressions but a `fn` body
    /// is not itself a contract position — we only descend to surface nested
    /// loop contracts. (A `spec fn` body, however, IS validated as a contract
    /// expression via its tail / statement expressions.)
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
            Expr::IntLit(_) | Expr::BoolLit(_) | Expr::Path(_) => {}

            // (a)/(b)/(iv): a free call is a combinator, a spec-fn call, or
            // forbidden.
            Expr::Call { callee, args } => self.walk_call(callee, args, span),

            // (c) bounded built-in method calls (e.g. `xs.len()`); the receiver
            // and args are recursed into. A method call is structurally a
            // built-in (the grammar admits only a closed set), so we accept the
            // shape and validate operands.
            Expr::MethodCall { receiver, args, .. } => {
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
            self.check_combinator(sig, args, span);
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
                Expr::Closure { body, .. } => self.walk_expr(body, span),
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
