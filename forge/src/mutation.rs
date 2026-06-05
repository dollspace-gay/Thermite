//! `forge/src/mutation.rs` — §7 step 4 of the vacuity battery: mutation scoring
//! (`thermite-design.md` §7 line 224, "operator flips, off-by-ones, early
//! returns, branch swaps — fixed deterministic mutator set"). Given a `fn` whose
//! REAL body already verifies L3, this module generates a FROZEN, DETERMINISTIC
//! set of mutants of that body (the contract UNTOUCHED), re-lowers + re-verifies
//! each against the same contract through the existing verus driver + proof
//! cache, and scores the **kill ratio** (`killed / scored`). A mutant verus
//! REJECTS is **killed** (the contract caught the wrong body — good); a mutant
//! verus PROVES is a **survivor** (the contract cannot tell the mutant from the
//! real body — too weak). A configurable floor (default 60%, §7) gates
//! certification: below the floor the item does NOT certify and the surviving
//! mutants are the precise strengthening prompt.
//!
//! Governing design: `.design/forge/mutation-scoring.md`.
//!
//! ## Polarity (the value-add)
//!
//! A mutant is a DELIBERATELY-WRONG body. If verus still PROVES it against the
//! contract, the contract is satisfied by both the right body and the wrong one
//! — it under-specifies. So `Proved` = SURVIVED = a hole in the contract; a
//! verus FAILURE = KILLED = the contract did its job (REQ-4). This is the same
//! polarity inversion #13's harnesses use, applied to the body.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (frozen deterministic mutator set) | SHIPPED | `pub fn generate` walks a `FnItem.body` in source order and applies the FIXED families: operator flips (`flip_binop`), off-by-ones (`IntLit n`→`n+1`/`n-1`, skip `n-1` at 0), early returns (`return <type-zero>` via `zero_value_for` at body head), branch swaps (negate `if`/`Expr::If` cond, else swap arms). Consumer: `check::mutation_score` in `check.rs`. |
//! | REQ-2 (deterministic order + seed + cap) | SHIPPED | a pre-order walk in a fixed family order, capped by `pub const MUTANT_CAP`; selection is the first `MUTANT_CAP` mutants in enumeration order. The seam takes the pinned `check::DEFAULT_SOLVER_SEED` (recorded in the run; the enumeration is seed-stable). Consumer: `check::mutation_score`. |
//! | REQ-3 (re-lower + re-verify vs same contract) | SHIPPED | `pub fn generate` clones the original `FnItem` and mutates only `body`; `check::mutation_score` weaves each mutant via `item_subprogram` + `thermite_lower::lower` and runs the existing `run_verus`. The `req`/`ens`/`inv`/`dec` are the original's, unchanged. |
//! | REQ-4 (KILLED vs SURVIVED) | SHIPPED | `pub fn classify_mutant` maps a `MutantOutcome`: `Proved` → SURVIVED, `Killed` (counterexample / timeout) → KILLED; a lowering failure is DROPPED (not scored). Consumer: `check::mutation_score`. |
//! | REQ-5 (kill ratio + floor gate, default 60%) | SHIPPED | `pub struct MutationScore` carries `killed`/`scored`/`survivor`; `pub fn MutationScore::kill_ratio` + `pub const MUTATION_FLOOR: f64 = 0.60`; `pub fn MutationScore::meets_floor`. The `cli` `--mutation-floor <FLOAT>` lever threads a non-default floor. Consumer: `check::mutation_score` + the floor gate in `check::check_file_with_options`. |
//! | REQ-6 (graduate `mutants_killed`/`survivor`) | SHIPPED | `MutationScore::mutants_killed_string` builds the `"K/N"` form; `check` sets it via `Certificate::with_mutation_score` / `Certificate::rejected_weak_contract`. |
//! | REQ-7 (gate AFTER L3, reuse proof cache) | SHIPPED | `check::mutation_score` runs only on a `VerusOutcome::Proved` real body and content-addresses each mutant via `cache::cache_key`/`load`/`store`. |
//! | REQ-8 (deterministic kill ratio) | SHIPPED | `generate` is a pure function of the AST + the frozen table; each mutant verdict is the deterministic verus run the L3 path + cache rely on, so `mutants_killed` is deterministic (asserted by the same-input-twice conformance double-run). `mutants_killed`/`survivor` stay oracle-EXCLUDED (OQ-1). |

use thermite_syntax::{BinOp, Block, Expr, FnItem, PrimType, Stmt, Type};

/// The FIXED budget on the number of mutants scored per `fn` (REQ-2; OQ-2). §7
/// says "budgeted" without a number; this is a documented `const` (R-CODE-5 —
/// the budget is a fixed input, not wall-clock). Each mutant is a full verus run
/// (cheap on a cache hit, #8), so the cap bounds the gate's cost. The corpus
/// `fn`s naturally produce on the order of tens of mutants; `64` comfortably
/// covers them while bounding a pathologically large body. Selection when the
/// candidate count exceeds the cap is the FIRST `MUTANT_CAP` mutants in the
/// deterministic enumeration order (REQ-2).
pub const MUTANT_CAP: usize = 64;

/// The default mutation kill-ratio floor (`thermite-design.md` §7 "a
/// configurable floor (default 60%)"). `kill_ratio >= MUTATION_FLOOR` certifies;
/// below it the item does NOT certify (verdict-in-cert reject). The `cli`
/// `--mutation-floor <FLOAT>` lever overrides it; a non-default floor is a
/// deliberate, documented choice (mirroring the existing `--rlimit` lever).
pub const MUTATION_FLOOR: f64 = 0.60;

/// One generated mutant: a `FnItem` with the SAME contract as the original and a
/// MUTATED body, plus a human description naming the change (REQ-1). The
/// description is the §7 "precise strengthening prompt" payload surfaced as a
/// cert's `survivor` when this mutant survives.
#[derive(Debug, Clone)]
pub struct Mutant {
    /// The mutated `fn` — contract untouched, only `body` differs from the
    /// original (REQ-1/REQ-3).
    pub item: FnItem,
    /// A human description of the single change this mutant applies (REQ-1), e.g.
    /// `"flip binary operator Add->Sub"` or `"insert early `return 0` at body
    /// head"`. Carried into the cert's `survivor` on a survival.
    pub desc: String,
}

/// The classification of one mutant's verus run (REQ-4). The polarity is
/// INVERTED from the L3 proof: a verus SUCCESS on a deliberately-wrong body is
/// the BAD news (the contract did not catch the change → SURVIVED); a verus
/// FAILURE is the GOOD news (the contract caught it → KILLED).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutantOutcome {
    /// verus PROVED the mutant — the contract holds for the wrong body too, so it
    /// cannot distinguish the mutant: a SURVIVOR (the contract is too weak here).
    Survived,
    /// verus did NOT prove the mutant (a counterexample OR a timeout — OQ-4): the
    /// contract caught the change. KILLED (good).
    Killed,
}

/// Map a verus verdict polarity to a [`MutantOutcome`] (REQ-4). `proved == true`
/// (verus succeeded on the wrong body) is a SURVIVOR; `proved == false` (a
/// counterexample, or a timeout counted KILLED per OQ-4) is KILLED. This is the
/// single classification seam `check::mutation_score` calls; a mutant that fails
/// to LOWER is dropped BEFORE this (not scored, OQ-5), never passed here.
pub fn classify_mutant(proved: bool) -> MutantOutcome {
    if proved {
        MutantOutcome::Survived
    } else {
        MutantOutcome::Killed
    }
}

/// The result of scoring a `fn`'s frozen mutant set (REQ-5). `killed`/`scored`
/// are over the mutants that LOWERED + ran (un-lowerable mutants are dropped from
/// the denominator, OQ-5). `survivor` is a representative surviving mutant's
/// description (the first survivor in deterministic enumeration order, REQ-2), or
/// `None` when every scored mutant was killed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationScore {
    /// Mutants verus FAILED to prove (the contract caught them) — good.
    pub killed: usize,
    /// Mutants that lowered + ran (the denominator). Excludes un-lowerable
    /// mutants (OQ-5).
    pub scored: usize,
    /// A representative surviving mutant's description (the §7 strengthening
    /// prompt), or `None` if every scored mutant was killed.
    pub survivor: Option<String>,
}

impl MutationScore {
    /// The kill ratio `killed / scored` (REQ-5). When NO mutant was scored
    /// (`scored == 0` — e.g. every mutant failed to lower, or the body has no
    /// mutation site), the ratio is `1.0`: there is no surviving counterexample
    /// to the contract's strength, so the floor is vacuously met (the gate does
    /// not reject a body it could not mutate). Documented: a `0/0` score is a
    /// pass, not a reject.
    pub fn kill_ratio(&self) -> f64 {
        if self.scored == 0 {
            1.0
        } else {
            self.killed as f64 / self.scored as f64
        }
    }

    /// `true` iff the kill ratio meets `floor` (REQ-5). The certification gate:
    /// `>= floor` certifies, `< floor` is a `WeakContract` reject.
    pub fn meets_floor(&self, floor: f64) -> bool {
        self.kill_ratio() >= floor
    }

    /// The Appendix A `"killed/scored"` string form for
    /// `contract_quality.mutants_killed` (REQ-6; the `"17/18"` shape).
    pub fn mutants_killed_string(&self) -> String {
        format!("{}/{}", self.killed, self.scored)
    }
}

/// Generate the FROZEN, DETERMINISTIC mutant set of `f`'s body (REQ-1/REQ-2).
///
/// The walk is pre-order over the body in SOURCE order; at each site the fixed
/// mutator families are applied in this fixed family order:
///   1. **early return** — ONE mutant inserting `return <zero-of-ret-type>` at
///      the FRONT of the body block (skipped when the return type has no
///      canonical zero, OQ-3);
///   2. **operator flips** — for each `Expr::Binary` whose `op` has a frozen
///      flip (`flip_binop`), one mutant with the flipped operator;
///   3. **off-by-ones** — for each `Expr::IntLit(n)`, `n`→`n+1` and (when
///      `n != 0`) `n`→`n-1`;
///   4. **branch swaps** — for each `Stmt::If` / `Expr::If`, one mutant negating
///      the condition (and, when the condition is not a flippable comparison,
///      one mutant swapping the arms — see `branch_swap_mutants`).
///
/// The resulting list is bounded by [`MUTANT_CAP`]: when the candidate count
/// exceeds the cap, the first `MUTANT_CAP` mutants in this order are returned
/// (REQ-2). Each mutant is the original `f` with ONLY `body` changed — the
/// contract (`req`/`ens`/`fx`, loop `inv`/`dec`) is untouched (REQ-1/REQ-3). A
/// pure function of `f` + the frozen table ⇒ the same ordered list every run
/// (REQ-8); `_seed` is taken for the documented determinism seam (the
/// enumeration is seed-stable; selection is order-prefix, not random).
pub fn generate(f: &FnItem, _seed: u64) -> Vec<Mutant> {
    let mut mutants = Vec::new();

    // Family 1: early return at body head (one mutant, if the ret type has a
    // canonical zero). Listed first so it is never crowded out by the cap on a
    // body with many sites — it is the §7 discriminator (the value-add mutant).
    if let Some(zero) = zero_value_for(&f.ret) {
        let mut body = f.body.clone();
        body.stmts.insert(0, Stmt::Return(Some(zero)));
        mutants.push(mutant_with_body(
            f,
            body,
            format!("insert early `return {}` at body head", zero_desc(&f.ret)),
        ));
    }

    // Families 2-4: walk the body collecting per-site mutated bodies. Each entry
    // is a (mutated Block, description); we rebuild the `FnItem` around it.
    let mut sink = MutantSink::new();
    sink.walk_block(&f.body);
    for (body, desc) in sink.into_mutants(&f.body) {
        mutants.push(mutant_with_body(f, body, desc));
    }

    mutants.truncate(MUTANT_CAP);
    mutants
}

/// Build a [`Mutant`] from the original `f` and a mutated `body` (REQ-1/REQ-3):
/// the contract and signature are cloned verbatim; only `body` changes.
fn mutant_with_body(f: &FnItem, body: Block, desc: String) -> Mutant {
    let mut item = f.clone();
    item.body = body;
    Mutant { item, desc }
}

/// The frozen operator-flip table (REQ-1; `thermite-design.md` §7 line 224). A
/// closed, deterministic mapping over the §4.4 `BinOp` set: `Add`↔`Sub`,
/// `Mul`↔`Div`, `Lt`↔`Le`, `Gt`↔`Ge`, `Eq`↔`Ne`, `And`↔`Or`. Operators with no
/// listed flip (none — every variant in the frozen set is covered as a pair, and
/// the function is total over `BinOp`) return `None`.
fn flip_binop(op: BinOp) -> Option<BinOp> {
    let flipped = match op {
        BinOp::Add => BinOp::Sub,
        BinOp::Sub => BinOp::Add,
        BinOp::Mul => BinOp::Div,
        BinOp::Div => BinOp::Mul,
        BinOp::Lt => BinOp::Le,
        BinOp::Le => BinOp::Lt,
        BinOp::Gt => BinOp::Ge,
        BinOp::Ge => BinOp::Gt,
        BinOp::Eq => BinOp::Ne,
        BinOp::Ne => BinOp::Eq,
        BinOp::And => BinOp::Or,
        BinOp::Or => BinOp::And,
    };
    Some(flipped)
}

/// The surface token of a `BinOp` for a mutant description (deterministic).
fn binop_token(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
    }
}

/// The canonical zero VALUE of a return type for the early-return mutant (REQ-1,
/// OQ-3): `0` for an integer prim, `false` for `bool`, `None` for an `Option`.
/// A type with no canonical zero (`Unit`, a `Ref`, a bare `Slice`, a non-`Option`
/// generic) yields `None` — the early-return mutator is SKIPPED for that `fn`
/// (dropped from the set, not an error). Returning a value of the function's
/// return type keeps the mutant well-typed so it LOWERS (only the contract should
/// reject it, not the type checker).
fn zero_value_for(ret: &Type) -> Option<Expr> {
    match ret {
        Type::Prim(PrimType::U32 | PrimType::U64 | PrimType::Usize) => Some(Expr::IntLit(0)),
        Type::Prim(PrimType::Bool) => Some(Expr::BoolLit(false)),
        Type::Generic { name, .. } if name == "Option" => {
            Some(Expr::Path(vec!["None".to_string()]))
        }
        _ => None,
    }
}

/// The human description of the early-return zero value (matches `zero_value_for`).
fn zero_desc(ret: &Type) -> &'static str {
    match ret {
        Type::Prim(PrimType::U32 | PrimType::U64 | PrimType::Usize) => "0",
        Type::Prim(PrimType::Bool) => "false",
        Type::Generic { name, .. } if name == "Option" => "None",
        _ => "<none>",
    }
}

/// Negate an `if` condition for a branch-swap mutant (REQ-1). When the condition
/// is a flippable comparison (`<`↔`>=`, `<=`↔`>`, `==`↔`!=`), negation is the
/// COMPLEMENTARY flip (`!(a < b)` ≡ `a >= b`), encoded in the operator set so the
/// mutant is a clean comparison rather than a parenthesised `!`. For any other
/// condition shape the negation falls back to swapping the arms (handled by the
/// caller), so this returns `None`.
fn negate_comparison(cond: &Expr) -> Option<(Expr, &'static str)> {
    if let Expr::Binary { op, lhs, rhs } = cond {
        let complement = match op {
            BinOp::Lt => Some(BinOp::Ge),
            BinOp::Le => Some(BinOp::Gt),
            BinOp::Gt => Some(BinOp::Le),
            BinOp::Ge => Some(BinOp::Lt),
            BinOp::Eq => Some(BinOp::Ne),
            BinOp::Ne => Some(BinOp::Eq),
            _ => None,
        };
        if let Some(new_op) = complement {
            let negated = Expr::Binary {
                op: new_op,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            };
            return Some((negated, binop_token(new_op)));
        }
    }
    None
}

/// A collector for families 2-4. It walks the body and, for each mutation site,
/// records the action needed to rebuild a mutated copy of the WHOLE body with
/// exactly that one site changed. Recording an action (rather than eagerly
/// cloning the whole body per site) keeps the walk a single pass; the mutated
/// bodies are materialised in `into_mutants` by re-walking with one site armed.
///
/// The site index is a deterministic pre-order position (REQ-2): the walk visits
/// statements in source order, descending into blocks / loops / nested ifs, and
/// expressions left-to-right, so the Nth recorded site is stable across runs.
struct MutantSink {
    actions: Vec<MutAction>,
}

/// One recorded mutation action keyed by the deterministic pre-order site index
/// of the node it applies to.
struct MutAction {
    /// The pre-order index (over the relevant node kind) this action targets.
    site: usize,
    kind: MutKind,
    desc: String,
}

/// The kind of single-site mutation an action applies (families 2-4).
enum MutKind {
    /// Replace the `BinOp` at the targeted `Expr::Binary` site with this op.
    FlipBinop(BinOp),
    /// Replace the `u128` literal at the targeted `Expr::IntLit` site with this.
    OffByOne(u128),
    /// Replace the condition at the targeted `if` site with this expression.
    NegateCond(Box<Expr>),
    /// Swap the `then`/`else` arms at the targeted `if` site (else-less ifs
    /// never record this; see `branch_swap_mutants`).
    SwapArms,
}

impl MutantSink {
    fn new() -> Self {
        MutantSink {
            actions: Vec::new(),
        }
    }

    /// Enumerate every candidate action over the body in deterministic pre-order
    /// (REQ-2). Counters are threaded so each node kind has its own stable site
    /// index, independent of the others.
    fn walk_block(&mut self, block: &Block) {
        let mut ctr = Counters::default();
        self.scan_block(block, &mut ctr);
    }

    fn scan_block(&mut self, block: &Block, ctr: &mut Counters) {
        for stmt in &block.stmts {
            self.scan_stmt(stmt, ctr);
        }
        if let Some(tail) = &block.tail {
            self.scan_expr(tail, ctr);
        }
    }

    fn scan_stmt(&mut self, stmt: &Stmt, ctr: &mut Counters) {
        match stmt {
            Stmt::Let { init, .. } => self.scan_expr(init, ctr),
            Stmt::Assign { target, value } => {
                self.scan_expr(target, ctr);
                self.scan_expr(value, ctr);
            }
            Stmt::Return(Some(e)) => self.scan_expr(e, ctr),
            Stmt::Return(None) => {}
            Stmt::If { cond, then, else_ } => {
                self.record_if(cond, else_.is_some(), ctr);
                self.scan_expr(cond, ctr);
                self.scan_block(then, ctr);
                if let Some(e) = else_ {
                    self.scan_block(e, ctr);
                }
            }
            Stmt::Loop(l) => {
                // The loop's `inv`/`dec` are CONTRACT (the mutator never touches
                // them — OUT scope in the design); only the loop body is mutated.
                self.scan_block(&l.body, ctr);
            }
            Stmt::Expr(e) => self.scan_expr(e, ctr),
        }
    }

    fn scan_expr(&mut self, expr: &Expr, ctr: &mut Counters) {
        match expr {
            Expr::IntLit(n) => {
                let site = ctr.intlit;
                ctr.intlit += 1;
                // `n`→`n+1` (always) and `n`→`n-1` (skip at 0: `IntLit` is u128,
                // it cannot represent -1; documented, not a silent wrap, REQ-1).
                self.actions.push(MutAction {
                    site,
                    kind: MutKind::OffByOne(n.wrapping_add(1)),
                    desc: format!("off-by-one literal {n}->{}", n + 1),
                });
                if *n != 0 {
                    self.actions.push(MutAction {
                        site,
                        kind: MutKind::OffByOne(n - 1),
                        desc: format!("off-by-one literal {n}->{}", n - 1),
                    });
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                let site = ctr.binary;
                ctr.binary += 1;
                if let Some(flipped) = flip_binop(*op) {
                    self.actions.push(MutAction {
                        site,
                        kind: MutKind::FlipBinop(flipped),
                        desc: format!(
                            "flip binary operator {}->{}",
                            binop_token(*op),
                            binop_token(flipped)
                        ),
                    });
                }
                self.scan_expr(lhs, ctr);
                self.scan_expr(rhs, ctr);
            }
            Expr::If { cond, then, else_ } => {
                // An `Expr::If` always has both arms (ast.rs `Expr::If.else_` is a
                // non-optional `Block`), so a swap is always recordable.
                self.record_if(cond, true, ctr);
                self.scan_expr(cond, ctr);
                self.scan_block(then, ctr);
                self.scan_block(else_, ctr);
            }
            Expr::Call { callee, args } => {
                self.scan_expr(callee, ctr);
                for a in args {
                    self.scan_expr(a, ctr);
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                self.scan_expr(receiver, ctr);
                for a in args {
                    self.scan_expr(a, ctr);
                }
            }
            Expr::Field { receiver, .. } => self.scan_expr(receiver, ctr),
            Expr::Closure { body, .. } => self.scan_expr(body, ctr),
            Expr::Match { scrutinee, arms } => {
                self.scan_expr(scrutinee, ctr);
                for arm in arms {
                    self.scan_expr(&arm.body, ctr);
                }
            }
            Expr::Index { base, index } => {
                self.scan_expr(base, ctr);
                self.scan_index(index, ctr);
            }
            Expr::Cast { expr, .. } => self.scan_expr(expr, ctr),
            Expr::Ref { expr, .. } => self.scan_expr(expr, ctr),
            Expr::BoolLit(_) | Expr::Path(_) => {}
        }
    }

    fn scan_index(&mut self, index: &thermite_syntax::IndexArg, ctr: &mut Counters) {
        use thermite_syntax::IndexArg;
        match index {
            IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
                self.scan_expr(e, ctr)
            }
            IndexArg::Range(a, b) => {
                self.scan_expr(a, ctr);
                self.scan_expr(b, ctr);
            }
        }
    }

    /// Record the branch-swap mutant(s) for an `if` site (REQ-1, family 4): a
    /// negate-condition mutant when the condition is a flippable comparison, else
    /// (when there are two arms) an arm-swap mutant. An else-less `if` whose
    /// condition is not a flippable comparison records nothing (no arms to swap,
    /// no clean negation).
    fn record_if(&mut self, cond: &Expr, has_else: bool, ctr: &mut Counters) {
        let site = ctr.iff;
        ctr.iff += 1;
        if let Some((negated, tok)) = negate_comparison(cond) {
            self.actions.push(MutAction {
                site,
                kind: MutKind::NegateCond(Box::new(negated)),
                desc: format!("negate `if` condition (comparison -> {tok})"),
            });
        } else if has_else {
            self.actions.push(MutAction {
                site,
                kind: MutKind::SwapArms,
                desc: "swap `if` then/else arms".to_string(),
            });
        }
    }

    /// Materialise one mutated body per recorded action by re-walking `body` with
    /// exactly that one action armed (REQ-1/REQ-2). The order matches the
    /// recording order, which is the deterministic pre-order family sequence.
    fn into_mutants(self, body: &Block) -> Vec<(Block, String)> {
        self.actions
            .into_iter()
            .map(|action| {
                let mut applier = Applier {
                    action: &action,
                    ctr: Counters::default(),
                };
                let mutated = applier.apply_block(body);
                (mutated, action.desc)
            })
            .collect()
    }
}

/// Per-node-kind pre-order site counters (REQ-2). Each node kind is numbered
/// independently in source order so a recorded `site` is matched to exactly the
/// same node when re-walking to apply it.
#[derive(Default)]
struct Counters {
    binary: usize,
    intlit: usize,
    iff: usize,
}

/// Re-walks a body applying exactly one armed action at its target site,
/// returning a fresh mutated body. The walk mirrors `MutantSink::scan_*` so the
/// site numbering is identical.
struct Applier<'a> {
    action: &'a MutAction,
    ctr: Counters,
}

impl Applier<'_> {
    fn apply_block(&mut self, block: &Block) -> Block {
        let stmts = block.stmts.iter().map(|s| self.apply_stmt(s)).collect();
        let tail = block.tail.as_ref().map(|t| Box::new(self.apply_expr(t)));
        Block { stmts, tail }
    }

    fn apply_stmt(&mut self, stmt: &Stmt) -> Stmt {
        match stmt {
            Stmt::Let {
                mutable,
                name,
                ty,
                init,
            } => Stmt::Let {
                mutable: *mutable,
                name: name.clone(),
                ty: ty.clone(),
                init: self.apply_expr(init),
            },
            Stmt::Assign { target, value } => Stmt::Assign {
                target: self.apply_expr(target),
                value: self.apply_expr(value),
            },
            Stmt::Return(Some(e)) => Stmt::Return(Some(self.apply_expr(e))),
            Stmt::Return(None) => Stmt::Return(None),
            Stmt::If { cond, then, else_ } => {
                let (new_cond, swap) = self.apply_if(cond, else_.is_some());
                let cond_done = self.apply_expr(&new_cond);
                let then_done = self.apply_block(then);
                let else_done = else_.as_ref().map(|e| self.apply_block(e));
                if swap {
                    if let Some(e) = else_done {
                        Stmt::If {
                            cond: cond_done,
                            then: e,
                            else_: Some(then_done),
                        }
                    } else {
                        Stmt::If {
                            cond: cond_done,
                            then: then_done,
                            else_: None,
                        }
                    }
                } else {
                    Stmt::If {
                        cond: cond_done,
                        then: then_done,
                        else_: else_done,
                    }
                }
            }
            Stmt::Loop(l) => {
                let mut l = l.clone();
                l.body = self.apply_block(&l.body);
                Stmt::Loop(l)
            }
            Stmt::Expr(e) => Stmt::Expr(self.apply_expr(e)),
        }
    }

    /// Resolve the `if`-site action: return the (possibly negated) condition and
    /// whether to swap arms. Advances the `iff` counter exactly once per `if`,
    /// matching `MutantSink::record_if`.
    fn apply_if(&mut self, cond: &Expr, _has_else: bool) -> (Expr, bool) {
        let site = self.ctr.iff;
        self.ctr.iff += 1;
        if site == self.action.site {
            match &self.action.kind {
                MutKind::NegateCond(e) => return ((**e).clone(), false),
                MutKind::SwapArms => return (cond.clone(), true),
                _ => {}
            }
        }
        (cond.clone(), false)
    }

    fn apply_expr(&mut self, expr: &Expr) -> Expr {
        match expr {
            Expr::IntLit(n) => {
                let site = self.ctr.intlit;
                self.ctr.intlit += 1;
                if site == self.action.site {
                    if let MutKind::OffByOne(v) = &self.action.kind {
                        return Expr::IntLit(*v);
                    }
                }
                Expr::IntLit(*n)
            }
            Expr::Binary { op, lhs, rhs } => {
                let site = self.ctr.binary;
                self.ctr.binary += 1;
                let new_op = if site == self.action.site {
                    if let MutKind::FlipBinop(o) = &self.action.kind {
                        *o
                    } else {
                        *op
                    }
                } else {
                    *op
                };
                Expr::Binary {
                    op: new_op,
                    lhs: Box::new(self.apply_expr(lhs)),
                    rhs: Box::new(self.apply_expr(rhs)),
                }
            }
            Expr::If { cond, then, else_ } => {
                let (new_cond, swap) = self.apply_if(cond, true);
                let cond_done = Box::new(self.apply_expr(&new_cond));
                let then_done = self.apply_block(then);
                let else_done = self.apply_block(else_);
                if swap {
                    Expr::If {
                        cond: cond_done,
                        then: else_done,
                        else_: then_done,
                    }
                } else {
                    Expr::If {
                        cond: cond_done,
                        then: then_done,
                        else_: else_done,
                    }
                }
            }
            Expr::Call { callee, args } => Expr::Call {
                callee: Box::new(self.apply_expr(callee)),
                args: args.iter().map(|a| self.apply_expr(a)).collect(),
            },
            Expr::MethodCall {
                receiver,
                name,
                args,
            } => Expr::MethodCall {
                receiver: Box::new(self.apply_expr(receiver)),
                name: name.clone(),
                args: args.iter().map(|a| self.apply_expr(a)).collect(),
            },
            Expr::Field { receiver, name } => Expr::Field {
                receiver: Box::new(self.apply_expr(receiver)),
                name: name.clone(),
            },
            Expr::Closure { params, body } => Expr::Closure {
                params: params.clone(),
                body: Box::new(self.apply_expr(body)),
            },
            Expr::Match { scrutinee, arms } => Expr::Match {
                scrutinee: Box::new(self.apply_expr(scrutinee)),
                arms: arms
                    .iter()
                    .map(|arm| thermite_syntax::MatchArm {
                        pattern: arm.pattern.clone(),
                        body: self.apply_expr(&arm.body),
                    })
                    .collect(),
            },
            Expr::Index { base, index } => Expr::Index {
                base: Box::new(self.apply_expr(base)),
                index: self.apply_index(index),
            },
            Expr::Cast { expr, ty } => Expr::Cast {
                expr: Box::new(self.apply_expr(expr)),
                ty: ty.clone(),
            },
            Expr::Ref { mutable, expr } => Expr::Ref {
                mutable: *mutable,
                expr: Box::new(self.apply_expr(expr)),
            },
            Expr::BoolLit(b) => Expr::BoolLit(*b),
            Expr::Path(p) => Expr::Path(p.clone()),
        }
    }

    fn apply_index(&mut self, index: &thermite_syntax::IndexArg) -> thermite_syntax::IndexArg {
        use thermite_syntax::IndexArg;
        match index {
            IndexArg::Single(e) => IndexArg::Single(Box::new(self.apply_expr(e))),
            IndexArg::RangeTo(e) => IndexArg::RangeTo(Box::new(self.apply_expr(e))),
            IndexArg::RangeFrom(e) => IndexArg::RangeFrom(Box::new(self.apply_expr(e))),
            IndexArg::Range(a, b) => {
                IndexArg::Range(Box::new(self.apply_expr(a)), Box::new(self.apply_expr(b)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_fn(src: &str) -> FnItem {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        parsed
            .program
            .items
            .into_iter()
            .find_map(|i| match i {
                thermite_syntax::Item::Fn(f) => Some(f),
                _ => None,
            })
            .expect("fixture has a fn")
    }

    // AC-5: the frozen set + deterministic order for a small fn. Expected mutants
    // trace to REQ-1's table (R-CHAR-3), not to the generator's own output. The
    // fn `fn f(x: u32) -> u32 req x < 10 ens result == x fx pure { x + 1 }` has:
    //   - one early return (ret u32 -> `return 0`),
    //   - one Binary `+` (Add->Sub flip),
    //   - one IntLit `1` (1->2, 1->0).
    #[test]
    fn frozen_set_and_order_for_small_fn() {
        let f = parse_fn("fn f(x: u32) -> u32 req x < 10 ens result == x fx pure { x + 1 }");
        let mutants = generate(&f, 0);
        let descs: Vec<&str> = mutants.iter().map(|m| m.desc.as_str()).collect();
        assert_eq!(
            descs,
            vec![
                "insert early `return 0` at body head",
                "flip binary operator +->-",
                "off-by-one literal 1->2",
                "off-by-one literal 1->0",
            ],
            "frozen mutator set in the documented family order"
        );
    }

    // REQ-1 (OQ-3): an `Option` return type's early-return mutant is `return None`.
    #[test]
    fn option_return_early_return_is_none() {
        let f = parse_fn("fn g(x: u32) -> Option<usize> req x < 10 ens true fx pure { Some(0) }");
        let mutants = generate(&f, 0);
        assert!(
            mutants
                .iter()
                .any(|m| m.desc == "insert early `return None` at body head"),
            "Option return -> early `return None`: {:?}",
            mutants.iter().map(|m| &m.desc).collect::<Vec<_>>()
        );
    }

    // REQ-1: the off-by-one `n-1` mutant is SKIPPED at n == 0 (u128 cannot
    // represent -1) — documented, not a silent wrap.
    #[test]
    fn off_by_one_skips_minus_one_at_zero() {
        let f = parse_fn("fn h(x: u32) -> u32 req x < 10 ens result >= 0 fx pure { 0 }");
        let mutants = generate(&f, 0);
        let obo: Vec<&str> = mutants
            .iter()
            .map(|m| m.desc.as_str())
            .filter(|d| d.starts_with("off-by-one"))
            .collect();
        // The tail literal `0` yields only `0->1` (never `0->-1`).
        assert_eq!(obo, vec!["off-by-one literal 0->1"]);
    }

    // REQ-8 / AC-4: generate is a pure function of the fn — the same fn yields the
    // byte-identical ordered mutant description list every call.
    #[test]
    fn generate_is_deterministic() {
        let f = parse_fn(
            "fn s(xs: &[u32]) -> u64 req xs.len() < 10 ens result >= 0 fx pure { \
             let mut a: u64 = 0; let mut i: usize = 0; \
             while i < xs.len() inv i <= xs.len() inv a >= 0 dec xs.len() - i \
             { a = a + xs[i] as u64; i = i + 1; } a }",
        );
        let a: Vec<String> = generate(&f, 0).into_iter().map(|m| m.desc).collect();
        let b: Vec<String> = generate(&f, 0).into_iter().map(|m| m.desc).collect();
        assert_eq!(a, b, "generate is deterministic");
        // The loop body's `+`, the off-by-ones, and the early return are all present.
        assert!(a.contains(&"insert early `return 0` at body head".to_string()));
        assert!(a.iter().any(|d| d.starts_with("flip binary operator +->-")));
        assert!(a.iter().any(|d| d.starts_with("off-by-one")));
    }

    // REQ-2: the mutant list is bounded by MUTANT_CAP.
    #[test]
    fn capped_at_mutant_cap() {
        let f = parse_fn(
            "fn s(xs: &[u32]) -> u64 req xs.len() < 10 ens result >= 0 fx pure { \
             let mut a: u64 = 0; let mut i: usize = 0; \
             while i < xs.len() inv i <= xs.len() inv a >= 0 dec xs.len() - i \
             { a = a + xs[i] as u64; i = i + 1; } a }",
        );
        assert!(generate(&f, 0).len() <= MUTANT_CAP);
    }

    // REQ-1/REQ-3: a mutant's contract is byte-identical to the original; only the
    // body differs.
    #[test]
    fn mutant_keeps_contract_changes_only_body() {
        let f = parse_fn("fn f(x: u32) -> u32 req x < 10 ens result == x fx pure { x + 1 }");
        let mutants = generate(&f, 0);
        for m in &mutants {
            assert_eq!(m.item.contract, f.contract, "contract untouched");
            assert_eq!(m.item.name, f.name);
            assert_eq!(m.item.params, f.params);
            assert_eq!(m.item.ret, f.ret);
            assert_ne!(m.item.body, f.body, "body mutated");
        }
    }

    // REQ-4: the classification polarity — verus SUCCESS (proved the wrong body) is
    // a SURVIVOR; a verus FAILURE is KILLED. Traces to the design's §7 polarity
    // table (R-CHAR-3), not forge's output.
    #[test]
    fn classify_polarity_is_inverted() {
        assert_eq!(classify_mutant(true), MutantOutcome::Survived);
        assert_eq!(classify_mutant(false), MutantOutcome::Killed);
    }

    // REQ-5/REQ-6: kill ratio + the floor + the "K/N" string.
    #[test]
    fn score_ratio_floor_and_string() {
        let score = MutationScore {
            killed: 3,
            scored: 3,
            survivor: None,
        };
        assert_eq!(score.kill_ratio(), 1.0);
        assert!(score.meets_floor(MUTATION_FLOOR));
        assert_eq!(score.mutants_killed_string(), "3/3");

        let weak = MutationScore {
            killed: 1,
            scored: 3,
            survivor: Some("insert early `return 0` at body head".to_string()),
        };
        assert!((weak.kill_ratio() - 0.3333).abs() < 0.01);
        assert!(!weak.meets_floor(MUTATION_FLOOR), "1/3 is below 0.60");
        assert!(weak.meets_floor(0.2), "1/3 is above the lowered 0.2 floor");
        assert_eq!(weak.mutants_killed_string(), "1/3");
    }

    // REQ-5: a 0/0 score (no scoreable mutant) meets the floor vacuously (not a
    // reject) — the gate does not reject a body it could not mutate.
    #[test]
    fn empty_score_meets_floor() {
        let score = MutationScore {
            killed: 0,
            scored: 0,
            survivor: None,
        };
        assert_eq!(score.kill_ratio(), 1.0);
        assert!(score.meets_floor(MUTATION_FLOOR));
    }

    // REQ-1 (family 4): a branch swap negates a comparison `if` condition.
    #[test]
    fn branch_swap_negates_comparison() {
        let f = parse_fn(
            "fn b(x: u32) -> u32 req x < 100 ens result >= 0 fx pure { \
             if x < 5 { return 1; } x }",
        );
        let mutants = generate(&f, 0);
        assert!(
            mutants
                .iter()
                .any(|m| m.desc.contains("negate `if` condition")),
            "a comparison `if` records a negate-condition mutant: {:?}",
            mutants.iter().map(|m| &m.desc).collect::<Vec<_>>()
        );
    }
}
