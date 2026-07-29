//! `forge/src/bitvector.rs` — the QF_BV lowering for `@bv`-tagged clauses
//! (`.design/stage3-bv-reconstruction.md` REQ-2 / AC-2 / AC-3, stage-3 increment).
//!
//! A `@bvN`-tagged clause is interpreted over fixed-width wraparound (machine)
//! semantics: every variable is an `N`-bit bit-vector and every operator is its
//! `2`'s-complement / unsigned machine counterpart, so addition, multiplication
//! and the bitwise/shift operators all overflow as the hardware does. The
//! clause is decided by Verus's `by(bit_vector)` mode — which is, mechanically, a
//! QF_BV solver query (Z3's bit-blaster). Following the stage-1 [`crate::engine::
//! NlsatEngine`] precedent (which reaches Z3's nlsat tactic *directly* for QF_NRA
//! rather than through a Verus VC round-trip), this engine renders the clause to an
//! SMT-LIB2 `QF_BV` query and runs Z3 directly: the same decision procedure
//! `by(bit_vector)` invokes, reached as its own [`crate::engine::EngineName::
//! BitVector`] route so a mixed-mechanism function (the RFC's `mix64` shape — two
//! `@bv64` clauses beside one unbounded clause) attributes each clause to the engine
//! that grounds it.
//!
//! ## The three deliverables of REQ-2
//!
//! - **The lowering.** [`render_bv_prop`] / [`render_bv_term`] translate the
//!   arithmetic / comparison / bitwise / boolean clause fragment into QF_BV at the
//!   tag's width. A clause valid for *every* `N`-bit assignment (the negation is
//!   `unsat`) is [`BvOutcome::Proved`]; a satisfiable negation yields a
//!   [`BvOutcome::Counterexample`] carrying the witnessing **bit pattern** per
//!   variable (REQ-2 / AC-3 — "bit-level `Counterexample` with the bit pattern
//!   attached").
//! - **The dedicated 64-bit multiplier budget profile.** A 64-bit bit-vector
//!   *multiplication between two non-constant terms* is the known QF_BV cost cliff
//!   (full 64×64 bit-blasting). [`BvBudgetProfile::for_query`] routes such a query
//!   through a bounded `rlimit`/timeout profile so an over-budget
//!   multiplier query is reported as [`BvOutcome::Timeout`] under the named profile —
//!   **never** a silent `unknown` and never a silent downgrade (REQ-2 / AC-3).
//! - **The verdict plumbing.** The engine implements the four-slot [`crate::engine::
//!   Engine`] interface; the rich [`BvOutcome`] (which the three-arm `Verdict` cannot
//!   represent — a budget `Timeout` is distinct from an undecided `Unknown`) is the
//!   route's entry point, mapped down for the generic trait caller.
//!
//! A `@bv` clause certifies at the caged rung [`crate::manifest::Level::L4`]: it is
//! decidable QF_BV with complete bit-pattern countermodels — the L4 refutation quality
//! (RFC-1 §2/§4), never degraded. Rung and trust base are orthogonal: the rung records
//! refutation quality, while the trust base (the QF_BV decision procedure, SOLVER) is
//! recorded separately in the attribution. Kernel-grounding the bit-vector discharge
//! (proof reconstruction) is REQ-7/REQ-8, which shrinks that trust base at the same
//! rung; this module is the lowering only.

use std::collections::BTreeMap;

use thermite_syntax::{BinOp, BvWidth, Expr, UnaryOp};

/// The outcome of a `@bv` clause discharge over fixed-width QF_BV semantics
/// (`.design/stage3-bv-reconstruction.md` REQ-2). Richer than the three-arm
/// [`crate::engine::Verdict`]: a budget [`BvOutcome::Timeout`] (the 64-bit multiplier
/// cost cliff) is held distinct from an [`BvOutcome::Unknown`] skip (Z3 absent
/// / the clause outside the renderable fragment), so the route never launders the
/// multiplier cliff into a silent `unknown` (AC-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BvOutcome {
    /// `unsat` over QF_BV: the clause holds for every `N`-bit assignment satisfying
    /// the precondition → the clause is machine-valid at width `N` → certify at the
    /// caged rung (L4): decidable, complete bit-pattern countermodels; the SOLVER trust
    /// base (Z3 QF_BV) is recorded separately in the attribution.
    Proved,
    /// `sat` over QF_BV: a concrete `N`-bit assignment falsifies the clause — a
    /// bit-level countermodel. Carries the witnessing bit pattern per variable
    /// (the AC-3 "bit pattern in the certificate").
    Counterexample {
        /// The falsifying bit pattern, one entry per declared variable.
        bits: Vec<BvBitPattern>,
    },
    /// The query exhausted its dedicated budget (the 64-bit multiplier cost cliff,
    /// AC-3). Carries the named budget profile and the Z3 detail; reported as
    /// `Timeout`, never `unknown` and never a silent downgrade.
    Timeout {
        /// The budget profile that bounded the query (e.g. `bv64-multiplier`).
        profile: String,
        /// The Z3 rlimit/timeout detail (the captured signal head).
        detail: String,
    },
    /// A skip: Z3 is absent, the clause is outside the renderable QF_BV
    /// fragment, or the query did not render. It is never a false verdict or the
    /// image of a budget exhaustion (that is [`BvOutcome::Timeout`]).
    Unknown(String),
}

/// One variable's falsifying bit pattern in a QF_BV countermodel
/// (`.design/stage3-bv-reconstruction.md` REQ-2 / AC-3). Carries the width, the
/// numeric value, and the rendered binary + hex bit strings so the certificate shows
/// the actual bits the hardware would hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BvBitPattern {
    /// The variable name.
    pub var: String,
    /// The bit width (`N` of the `@bvN` tag).
    pub width: u32,
    /// The numeric value of the bit pattern (`value < 2^width`).
    pub value: u128,
    /// The binary rendering, zero-padded to `width` bits (e.g. `0b0000…0011`).
    pub bits: String,
}

impl BvBitPattern {
    /// Render the bit pattern as a single human line (the certificate diagnostic
    /// fragment): `a = 0b…0011 (0x…3, bv64)`. Deterministic (R-CODE-5).
    #[must_use]
    pub fn render(&self) -> String {
        let hex_width = self.width.div_ceil(4) as usize;
        format!(
            "{} = {} (0x{:0width$x}, bv{})",
            self.var,
            self.bits,
            self.value,
            self.width,
            width = hex_width
        )
    }
}

/// A QF_BV budget profile (`.design/stage3-bv-reconstruction.md` REQ-2 — the
/// dedicated 64-bit multiplier profile). The `rlimit` bounds Z3's bit-blasting work
/// and `timeout_secs` is the wall-clock cap; an over-budget query returns `unknown`
/// from Z3 with a budget set, which the engine reports as [`BvOutcome::Timeout`]
/// (never a bare `unknown`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BvBudgetProfile {
    /// The profile name (recorded in the `Timeout` verdict).
    pub name: String,
    /// The Z3 `rlimit` (resource budget). `0` = unbounded (the default profile).
    pub rlimit: u64,
    /// The wall-clock timeout in seconds (the `-T:` flag).
    pub timeout_secs: u64,
}

impl BvBudgetProfile {
    /// The default profile: generous, for the cheap fragment (bitwise / shift / add /
    /// non-64-bit multiply). A scalar QF_BV query bit-blasts cheaply, so the default
    /// is unbounded `rlimit` with the same 10s wall cap the nlsat route uses.
    #[must_use]
    pub fn default_profile() -> Self {
        BvBudgetProfile {
            name: "bv-default".to_string(),
            rlimit: 0,
            timeout_secs: 10,
        }
    }

    /// The dedicated 64-bit multiplier profile (REQ-2 / AC-3): a 64×64 bit-blast is
    /// the QF_BV cost cliff, so a 64-bit multiplication between two non-constant
    /// terms is bounded by a tight `rlimit` and a short wall cap. An over-budget
    /// multiplier query then returns `unknown` (rlimit/timeout) and is reported as
    /// [`BvOutcome::Timeout`] under this profile — the loud, non-silent failure REQ-2
    /// requires.
    #[must_use]
    pub fn multiplier64_profile() -> Self {
        BvBudgetProfile {
            // A bounded resource budget. 64×64 multiplication
            // bit-blasts into a quadratic adder network whose validity queries
            // routinely exceed a modest rlimit; the bound makes the cliff a
            // deterministic `Timeout` rather than an unbounded hang.
            name: "bv64-multiplier".to_string(),
            rlimit: 2_000_000,
            timeout_secs: 5,
        }
    }

    /// Select the budget profile for a clause query (`.design/stage3-bv-reconstruction.md`
    /// REQ-2). The dedicated 64-bit multiplier profile applies iff the tag width is 64
    /// and the clause contains a multiplication whose two operands are both
    /// non-constant (a 64-bit multiplier, the cost cliff); every other query
    /// — including a multiply by a literal, which bit-blasts as a cheap shift/add —
    /// takes the default profile.
    #[must_use]
    pub fn for_query(width: BvWidth, exprs: &[&Expr]) -> Self {
        if width == BvWidth::W64 && exprs.iter().any(|e| contains_variable_multiply(e)) {
            Self::multiplier64_profile()
        } else {
            Self::default_profile()
        }
    }
}

/// Does `e` contain a multiplication where both operands are non-constant
/// (`.design/stage3-bv-reconstruction.md` REQ-2 — the 64-bit multiplier cost-cliff
/// detector)? A `var * var` (or `var * (expr-with-a-var)`) is the expensive 64×64
/// bit-blast; a `var * 8` is a cheap shift/add and does not trip the dedicated
/// profile.
#[must_use]
pub fn contains_variable_multiply(e: &Expr) -> bool {
    match e {
        Expr::Binary {
            op: BinOp::Mul,
            lhs,
            rhs,
        } if !is_constant(lhs) && !is_constant(rhs) => true,
        Expr::Binary { lhs, rhs, .. } => {
            contains_variable_multiply(lhs) || contains_variable_multiply(rhs)
        }
        Expr::Unary { expr, .. } => contains_variable_multiply(expr),
        _ => false,
    }
}

/// Is `e` a constant (a literal, or an arithmetic combination only of literals)
/// over the renderable fragment? Used by the cost-cliff detector to tell a cheap
/// `var * 8` (shift/add) from an expensive `var * var` (full multiplier).
fn is_constant(e: &Expr) -> bool {
    match e {
        Expr::IntLit { .. } | Expr::BoolLit(_) => true,
        Expr::Path(_) => false,
        Expr::Binary { lhs, rhs, .. } => is_constant(lhs) && is_constant(rhs),
        Expr::Unary { expr, .. } => is_constant(expr),
        _ => false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// The QF_BV lowering (REQ-2 — render the clause fragment at a fixed width).
// ─────────────────────────────────────────────────────────────────────────────

/// Render an arithmetic / bitwise TERM to an SMT-LIB2 `(_ BitVec width)` expression
/// at a fixed `width` (`.design/stage3-bv-reconstruction.md` REQ-2). Every operator is
/// its fixed-width machine counterpart, so the encoding is faithful to wraparound
/// semantics: `+`→`bvadd`, `*`→`bvmul`, `-`→`bvsub`, `/`→`bvudiv`, `%`→`bvurem`,
/// `<<`→`bvshl`, `>>`→`bvlshr` (unsigned — the scalar types are all unsigned), the
/// bitwise ops to `bvand`/`bvor`/`bvxor`, and prefix `!` (on a term) to `bvnot`. An
/// integer literal renders as the width-`N` bit-vector constant. `Err` names the
/// out-of-fragment construct (a skip reason, never a silent mis-render).
pub fn render_bv_term(e: &Expr, width: u32) -> Result<String, String> {
    match e {
        Expr::IntLit { value, .. } => Ok(format!("(_ bv{} {width})", value % (modulus(width)))),
        Expr::Path(segs) if segs.len() == 1 => Ok(segs[0].clone()),
        Expr::Binary { op, lhs, rhs } => {
            let sym = match op {
                BinOp::Add => "bvadd",
                BinOp::Sub => "bvsub",
                BinOp::Mul => "bvmul",
                BinOp::Div => "bvudiv",
                BinOp::Rem => "bvurem",
                BinOp::Shl => "bvshl",
                BinOp::Shr => "bvlshr",
                BinOp::BitAnd => "bvand",
                BinOp::BitOr => "bvor",
                BinOp::BitXor => "bvxor",
                other => {
                    return Err(format!(
                        "`{other:?}` is not a QF_BV term operator (it is a comparison/connective)"
                    ))
                }
            };
            let l = render_bv_term(lhs, width)?;
            let r = render_bv_term(rhs, width)?;
            Ok(format!("({sym} {l} {r})"))
        }
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => Ok(format!("(bvnot {})", render_bv_term(expr, width)?)),
        Expr::Cast { expr, .. } => render_bv_term(expr, width),
        other => Err(format!(
            "`{other:?}` is outside the QF_BV term fragment (only integer literals, \
             single-segment variables, the arithmetic/bitwise/shift operators, and `!` \
             lower to bit-vectors)"
        )),
    }
}

/// Render a proposition to an SMT-LIB2 `Bool` over fixed-width bit-vectors
/// (`.design/stage3-bv-reconstruction.md` REQ-2). Comparisons over the unsigned
/// scalar types map to the unsigned bit-vector relations (`<`→`bvult`, `<=`→`bvule`,
/// `>`→`bvugt`, `>=`→`bvuge`); `==`→`=`, `!=`→`(not (= …))`; the connectives to
/// `and`/`or`/`not`. `Err` names the out-of-fragment construct (a skip).
pub fn render_bv_prop(e: &Expr, width: u32) -> Result<String, String> {
    match e {
        Expr::BoolLit(b) => Ok(if *b { "true" } else { "false" }.to_string()),
        Expr::Binary { op, lhs, rhs } => match op {
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let l = render_bv_term(lhs, width)?;
                let r = render_bv_term(rhs, width)?;
                let rel = match op {
                    BinOp::Eq | BinOp::Ne => "=",
                    BinOp::Lt => "bvult",
                    BinOp::Le => "bvule",
                    BinOp::Gt => "bvugt",
                    BinOp::Ge => "bvuge",
                    _ => unreachable!("the outer match fixed the comparison set"),
                };
                let cmp = format!("({rel} {l} {r})");
                Ok(if matches!(op, BinOp::Ne) {
                    format!("(not {cmp})")
                } else {
                    cmp
                })
            }
            BinOp::And => Ok(format!(
                "(and {} {})",
                render_bv_prop(lhs, width)?,
                render_bv_prop(rhs, width)?
            )),
            BinOp::Or => Ok(format!(
                "(or {} {})",
                render_bv_prop(lhs, width)?,
                render_bv_prop(rhs, width)?
            )),
            other => Err(format!(
                "`{other:?}` is an arithmetic/bitwise operator, not a proposition — a \
                 `@bv` clause must be a comparison or a boolean connective at its root"
            )),
        },
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => Ok(format!("(not {})", render_bv_prop(expr, width)?)),
        other => Err(format!(
            "`{other:?}` is outside the QF_BV proposition fragment (a `@bv` clause is a \
             comparison / boolean connective over the bit-vector term fragment)"
        )),
    }
}

/// `2^width` as a `u128` (`width ≤ 64`, so this never overflows). The literal-reduction
/// modulus for [`render_bv_term`].
fn modulus(width: u32) -> u128 {
    1u128 << width
}

// ─────────────────────────────────────────────────────────────────────────────
// The engine (REQ-2 — the BitVector route's discharge entry point).
// ─────────────────────────────────────────────────────────────────────────────

/// The QF_BV bit-vector engine (`.design/stage3-bv-reconstruction.md` REQ-2): the
/// [`crate::engine::EngineName::BitVector`] route. Stateless beyond Z3 discovery (the
/// route passes the rendered clause pieces), mirroring the stateless shape the
/// stage-1 [`crate::engine::NlsatEngine`] exposes through [`BitVectorEngine::
/// discharge_bv`].
#[derive(Debug, Clone, Default)]
pub struct BitVectorEngine;

impl BitVectorEngine {
    /// Construct a BitVector engine. Production consumer: the `--engine bv` per-clause
    /// route (`check::bv_check`).
    #[must_use]
    pub fn new() -> Self {
        BitVectorEngine
    }

    /// Is `z3` invocable? The skip-guard for the live BitVector tests — CI shards
    /// without z3 SKIP rather than fail (the sibling `NlsatEngine::z3_present`
    /// precedent). No production caller — [`BitVectorEngine::run_z3`] handles
    /// z3-absence inline (a graceful [`BvOutcome::Unknown`] skip).
    #[allow(
        dead_code,
        reason = "REQ-2 live-test skip-guard: the in-crate live BitVector tests call it \
                  (CI shards without z3 SKIP); discharge_bv handles z3-absence inline in \
                  production"
    )]
    #[must_use]
    pub fn z3_present() -> bool {
        std::process::Command::new("z3")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Discharge one `@bvN` clause over fixed-width QF_BV semantics
    /// (`.design/stage3-bv-reconstruction.md` REQ-2 / AC-2 / AC-3). The query asserts
    /// the precondition (if any) and the negation of the clause over width-`N`
    /// bit-vectors, then asks Z3:
    ///
    /// - `unsat` → [`BvOutcome::Proved`] (machine-valid at width `N`);
    /// - `sat` → [`BvOutcome::Counterexample`] carrying the witnessing bit pattern;
    /// - `unknown` with a budget set → [`BvOutcome::Timeout`] under the named profile
    ///   (the 64-bit multiplier cliff — never a bare `unknown`);
    /// - Z3 absent / unrenderable clause → [`BvOutcome::Unknown`] (a skip).
    ///
    /// `vars` are the variables in scope (parameters, plus `result` for a function
    /// whose clause has already had `result` grounded by the body); `req` is the
    /// optional precondition.
    #[must_use]
    pub fn discharge_bv(
        &self,
        vars: &[String],
        req: Option<&Expr>,
        clause: &Expr,
        width: BvWidth,
    ) -> BvOutcome {
        let n = width.bits();
        // Render the clause first — an unrenderable clause is a skip, never a
        // false verdict (and never a spurious counterexample from a dropped guard).
        let clause_smt = match render_bv_prop(clause, n) {
            Ok(s) => s,
            Err(reason) => {
                return BvOutcome::Unknown(format!(
                    "the `@bv{n}` clause did not render to QF_BV: {reason}"
                ))
            }
        };
        // The precondition becomes a hypothesis. If it is present but unrenderable we
        // SKIP (Unknown) rather than drop it — dropping a guard could mint a spurious
        // counterexample at an assignment the precondition rules out (R-CODE-4).
        let req_smt = match req {
            Some(r) => match render_bv_prop(r, n) {
                Ok(s) => Some(s),
                Err(reason) => {
                    return BvOutcome::Unknown(format!(
                        "the `@bv{n}` clause's precondition did not render to QF_BV (skipping \
                         rather than dropping the guard): {reason}"
                    ))
                }
            },
            None => None,
        };

        let profile = BvBudgetProfile::for_query(
            width,
            &req.into_iter()
                .chain(std::iter::once(clause))
                .collect::<Vec<_>>(),
        );
        let query = build_bv_query(vars, req_smt.as_deref(), &clause_smt, n, &profile);

        match Self::run_z3(&query, profile.timeout_secs) {
            Ok((result, model)) => match result.as_str() {
                "unsat" => BvOutcome::Proved,
                "sat" => BvOutcome::Counterexample {
                    bits: parse_bv_model(&model, vars, n),
                },
                // `unknown` under a bounded profile is the budget cliff (AC-3): a
                // resource-limited multiplier query. Reported as `Timeout`, never a
                // bare `unknown`. The default profile is unbounded, so an `unknown`
                // there is a (rare for decidable QF_BV) solver event we still
                // surface as a wall-timeout rather than a false verdict.
                "unknown" => BvOutcome::Timeout {
                    profile: profile.name.clone(),
                    detail: format!(
                        "Z3 returned `unknown` under the `{}` budget profile (rlimit {}, \
                         {}s) — the QF_BV query exhausted its dedicated budget (the 64-bit \
                         multiplier cost cliff); reported as Timeout, never a silent unknown",
                        profile.name, profile.rlimit, profile.timeout_secs
                    ),
                },
                other => BvOutcome::Unknown(format!(
                    "Z3 returned an unexpected result `{other}` on the `@bv{n}` query"
                )),
            },
            Err(reason) => BvOutcome::Unknown(reason),
        }
    }

    /// Is the precondition `req` satisfiable at width `N`? Anti-Goodhart vacuity check
    /// (RFC-1 §10): a `@bv` clause is discharged as `req ⇒ clause`, so an unsatisfiable
    /// `req` proves *every* clause vacuously — the gaming vector the v1 cage rejects as
    /// `VacuousPrecondition`. The bv route's mutation gate only catches this for
    /// result-referencing clauses (every mutant survives); a param-only clause or a
    /// `@bv` lemma (no body → no mutation) would otherwise certify L4 — and, post-REQ-8,
    /// carry a kernel-checked trust label — on a vacuous proof. This asks Z3 the same
    /// rendering the discharge uses (`req` at width `N`), so the verdict is consistent
    /// with the discharge and never a width artifact.
    ///
    /// `Some(true)` = satisfiable (non-vacuous); `Some(false)` = unsatisfiable (vacuous — reject);
    /// `None` = cannot decide (`req` absent/unrenderable, Z3 absent, or `unknown`), so the
    /// caller does not flag vacuity and falls through to the normal discharge —
    /// conservative, never a false vacuity rejection.
    #[must_use]
    pub fn req_satisfiable(&self, vars: &[String], req: &Expr, width: BvWidth) -> Option<bool> {
        let n = width.bits();
        let req_smt = render_bv_prop(req, n).ok()?;
        let mut s = String::from("(set-logic QF_BV)\n");
        for v in vars {
            s.push_str(&format!("(declare-const {v} (_ BitVec {n}))\n"));
        }
        s.push_str(&format!("(assert {req_smt})\n(check-sat)\n"));
        match Self::run_z3(&s, BvBudgetProfile::default_profile().timeout_secs) {
            Ok((result, _)) => match result.as_str() {
                "sat" => Some(true),
                "unsat" => Some(false),
                _ => None,
            },
            Err(_) => None,
        }
    }

    /// Discharge the `@bvN(nowrap)` no-overflow side obligation over fixed-width QF_BV
    /// semantics (`.design/stage3-bv-reconstruction.md` REQ-5 / AC-6 — lock 3). A
    /// `nowrap` tag declares that, although the clause is interpreted at machine width,
    /// wrap is not the author's intent: every wrap-prone arithmetic operation in the
    /// clause body (`+`, `-`, `*`) must stay within `N` bits for every input satisfying
    /// the precondition. The obligation is itself a QF_BV query — "does some operation
    /// overflow at width `N`?" — asked the OPPOSITE way round to the main clause: it
    /// asserts the precondition and the disjunction of the per-operation overflow
    /// conditions (not negated, since we are hunting for an overflowing assignment).
    ///
    /// - `unsat` → no input overflows → [`BvOutcome::Proved`] (the obligation holds);
    /// - `sat` → a concrete overflowing input → [`BvOutcome::Counterexample`] carrying
    ///   the witnessing bit pattern (AC-6 — "fails with a concrete overflowing input");
    /// - `unknown` under a bounded profile → [`BvOutcome::Timeout`] (the multiplier
    ///   cliff — the overflow query zero-extends a 64-bit multiply to 128 bits, so it
    ///   inherits the same dedicated budget; never a bare `unknown`);
    /// - Z3 absent / an out-of-fragment operand → [`BvOutcome::Unknown`] (a skip).
    ///
    /// A clause whose body carries no wrap-prone operation (a pure comparison such as
    /// `result == a`) has nothing that could overflow, so the obligation holds vacuously
    /// ([`BvOutcome::Proved`]) without a solver round-trip. `vars` / `req` mirror
    /// [`BitVectorEngine::discharge_bv`] (the `result`-grounded clause closed over the
    /// parameters).
    #[must_use]
    pub fn discharge_nowrap(
        &self,
        vars: &[String],
        req: Option<&Expr>,
        clause: &Expr,
        width: BvWidth,
    ) -> BvOutcome {
        let n = width.bits();
        // Collect the per-operation overflow conditions. An out-of-fragment operand is an
        // skip (never a silent pass) — the obligation is not quietly dropped.
        let mut conds = Vec::new();
        if let Err(reason) = collect_overflow_conditions(clause, n, &mut conds) {
            return BvOutcome::Unknown(format!(
                "the `@bv{n}(nowrap)` side obligation did not render to QF_BV: {reason}"
            ));
        }
        // No wrap-prone arithmetic → nothing can overflow → the obligation holds
        // vacuously, with no solver query (and no spurious `unknown` when Z3 is absent).
        if conds.is_empty() {
            return BvOutcome::Proved;
        }
        // The precondition is a hypothesis on the overflowing assignment. As in
        // `discharge_bv`, an unrenderable `req` is a SKIP, never a dropped guard — dropping
        // it could mint an overflow witness at an assignment the precondition rules out.
        let req_smt = match req {
            Some(r) => match render_bv_prop(r, n) {
                Ok(s) => Some(s),
                Err(reason) => {
                    return BvOutcome::Unknown(format!(
                        "the `@bv{n}(nowrap)` obligation's precondition did not render to QF_BV \
                         (skipping rather than dropping the guard): {reason}"
                    ))
                }
            },
            None => None,
        };

        let overflow_smt = if conds.len() == 1 {
            conds.remove(0)
        } else {
            format!("(or {})", conds.join(" "))
        };
        // The overflow query reuses the clause's budget profile: a 64-bit variable
        // multiply zero-extends to a 128-bit `bvmul` here, an even costlier bit-blast, so
        // it deserves the dedicated multiplier budget as the main query does.
        let profile = BvBudgetProfile::for_query(
            width,
            &req.into_iter()
                .chain(std::iter::once(clause))
                .collect::<Vec<_>>(),
        );
        let query = build_nowrap_query(vars, req_smt.as_deref(), &overflow_smt, n, &profile);

        match Self::run_z3(&query, profile.timeout_secs) {
            Ok((result, model)) => match result.as_str() {
                // No assignment overflows → the no-overflow obligation holds.
                "unsat" => BvOutcome::Proved,
                // A concrete overflowing input → the obligation fails, witnessed by the
                // bit pattern (AC-6).
                "sat" => BvOutcome::Counterexample {
                    bits: parse_bv_model(&model, vars, n),
                },
                // The 128-bit multiplier bit-blast cliff: reported as Timeout under the
                // named profile, never a bare `unknown` (the `discharge_bv` precedent).
                "unknown" => BvOutcome::Timeout {
                    profile: profile.name.clone(),
                    detail: format!(
                        "Z3 returned `unknown` under the `{}` budget profile (rlimit {}, {}s) \
                         on the `@bv{n}(nowrap)` overflow query — reported as Timeout, never a \
                         silent unknown",
                        profile.name, profile.rlimit, profile.timeout_secs
                    ),
                },
                other => BvOutcome::Unknown(format!(
                    "Z3 returned an unexpected result `{other}` on the `@bv{n}(nowrap)` query"
                )),
            },
            Err(reason) => BvOutcome::Unknown(reason),
        }
    }

    /// Run Z3 over an SMT-LIB2 `query` (fed on stdin), returning `(result, model)`.
    /// `Err` on Z3 absent / spawn failure / no result token (a skip reason,
    /// never a silent success — R-CODE-4). Mirrors [`crate::engine::NlsatEngine`]'s
    /// `run_z3`, parameterized by the budget profile's wall timeout.
    fn run_z3(query: &str, timeout_secs: u64) -> Result<(String, String), String> {
        use std::io::Write as _;
        use std::process::{Command, Stdio};
        let mut child = Command::new("z3")
            .arg("-smt2")
            .arg("-in")
            .arg(format!("-T:{timeout_secs}"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    "z3 is not on PATH (the BitVector QF_BV route needs the bundled z3) — \
                     skipping"
                        .to_string()
                } else {
                    format!("could not spawn z3: {e}")
                }
            })?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(query.as_bytes())
                .map_err(|e| format!("could not write the QF_BV query to z3: {e}"))?;
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
        Ok((result, stdout.into_owned()))
    }
}

/// Build the SMT-LIB2 `QF_BV` query whose satisfiability decides a `@bvN` clause
/// (`.design/stage3-bv-reconstruction.md` REQ-2). Declares each variable as an
/// `N`-bit bit-vector, sets the budget profile's `rlimit` (when bounded — the 64-bit
/// multiplier cliff), asserts the precondition and the negation of the clause, and
/// asks for a model (the bit-pattern witness on `sat`).
#[must_use]
pub fn build_bv_query(
    vars: &[String],
    req_smt: Option<&str>,
    clause_smt: &str,
    width: u32,
    profile: &BvBudgetProfile,
) -> String {
    let mut s = String::new();
    s.push_str("(set-logic QF_BV)\n");
    // A bounded profile's rlimit is the 64-bit multiplier budget (AC-3): an
    // over-budget query returns `unknown`, reported as `Timeout`.
    if profile.rlimit > 0 {
        s.push_str(&format!("(set-option :rlimit {})\n", profile.rlimit));
    }
    for v in vars {
        s.push_str(&format!("(declare-const {v} (_ BitVec {width}))\n"));
    }
    if let Some(req) = req_smt {
        s.push_str(&format!("(assert {req})\n"));
    }
    // The clause is valid at width N iff `req ∧ ¬clause` is unsatisfiable.
    s.push_str(&format!("(assert (not {clause_smt}))\n"));
    s.push_str("(check-sat)\n");
    s.push_str("(get-model)\n");
    s
}

// ─────────────────────────────────────────────────────────────────────────────
// The `nowrap` no-overflow side obligation (REQ-5 — lock 3).
// ─────────────────────────────────────────────────────────────────────────────

/// Collect the per-operation no-overflow side conditions of a `@bvN(nowrap)` clause
/// body (`.design/stage3-bv-reconstruction.md` REQ-5 / AC-6). For every wrap-prone
/// arithmetic operation in `e` — addition, subtraction, multiplication — push the
/// SMT-LIB2 `Bool` that is true exactly when that operation overflows `width` bits, with
/// its operands rendered as the actual width-`N` machine terms fed into it. The walk
/// recurses through the whole expression (propositions, connectives, comparisons, and
/// the operand sub-terms), so a nested `(a + b) * c` emits a condition for both the
/// inner add and the outer multiply. The disjunction of the collected conditions is
/// satisfiable iff some input overflows. `Err` names an out-of-fragment operand — an
/// skip, so the obligation is never silently dropped. Pure (R-CODE-5).
fn collect_overflow_conditions(e: &Expr, width: u32, out: &mut Vec<String>) -> Result<(), String> {
    match e {
        Expr::Binary { op, lhs, rhs } => {
            if let Some(cond) = overflow_condition(*op, lhs, rhs, width)? {
                out.push(cond);
            }
            collect_overflow_conditions(lhs, width, out)?;
            collect_overflow_conditions(rhs, width, out)
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => {
            collect_overflow_conditions(expr, width, out)
        }
        _ => Ok(()),
    }
}

/// The SMT-LIB2 `Bool` that holds exactly when the `op` of `lhs op rhs` overflows
/// `width` bits (`.design/stage3-bv-reconstruction.md` REQ-5 / AC-6), or `Ok(None)` when
/// `op` is not a wrap-prone arithmetic operator (comparisons, connectives, bitwise/shift
/// and division never overflow the unsigned width — a shift/bitwise result is defined
/// at width, and unsigned `udiv`/`urem` only shrink). The three overflow tests are the
/// textbook unsigned ones:
///
/// - `+` overflows iff the sum carries out — equivalently the width-`N` result is LESS
///   than an operand: `(bvult (bvadd l r) l)`;
/// - `-` underflows iff it would go below zero — i.e. `l < r`: `(bvult l r)`;
/// - `*` overflows iff the true `2N`-bit product has any high-half bit set: zero-extend
///   both operands to `2N`, multiply, and check the top `N` bits are non-zero.
///
/// `Err` propagates an out-of-fragment operand from [`render_bv_term`].
fn overflow_condition(
    op: BinOp,
    lhs: &Expr,
    rhs: &Expr,
    width: u32,
) -> Result<Option<String>, String> {
    if !matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul) {
        return Ok(None);
    }
    let l = render_bv_term(lhs, width)?;
    let r = render_bv_term(rhs, width)?;
    let cond = match op {
        BinOp::Add => format!("(bvult (bvadd {l} {r}) {l})"),
        BinOp::Sub => format!("(bvult {l} {r})"),
        BinOp::Mul => {
            let hi = 2 * width - 1;
            format!(
                "(not (= ((_ extract {hi} {width}) \
                 (bvmul ((_ zero_extend {width}) {l}) ((_ zero_extend {width}) {r}))) \
                 (_ bv0 {width})))"
            )
        }
        _ => unreachable!("the guard fixed the wrap-prone operator set"),
    };
    Ok(Some(cond))
}

/// Build the SMT-LIB2 `QF_BV` query whose satisfiability decides a `@bvN(nowrap)`
/// no-overflow side obligation (`.design/stage3-bv-reconstruction.md` REQ-5 / AC-6).
/// Unlike [`build_bv_query`] (which asserts the NEGATED clause to seek a falsifying
/// model), this asserts the precondition and the overflow disjunction directly — a `sat`
/// model is a concrete OVERFLOWING input (the obligation fails), `unsat` means no input
/// overflows (the obligation holds). Sets the budget profile's `rlimit` when bounded
/// (the 64-bit multiplier cliff, now widened to a 128-bit `bvmul`).
#[must_use]
pub fn build_nowrap_query(
    vars: &[String],
    req_smt: Option<&str>,
    overflow_smt: &str,
    width: u32,
    profile: &BvBudgetProfile,
) -> String {
    let mut s = String::new();
    s.push_str("(set-logic QF_BV)\n");
    if profile.rlimit > 0 {
        s.push_str(&format!("(set-option :rlimit {})\n", profile.rlimit));
    }
    for v in vars {
        s.push_str(&format!("(declare-const {v} (_ BitVec {width}))\n"));
    }
    if let Some(req) = req_smt {
        s.push_str(&format!("(assert {req})\n"));
    }
    // The obligation fails iff `req ∧ (some operation overflows)` is satisfiable — so the
    // overflow disjunction is asserted directly (not negated, unlike the main clause).
    s.push_str(&format!("(assert {overflow_smt})\n"));
    s.push_str("(check-sat)\n");
    s.push_str("(get-model)\n");
    s
}

/// Parse Z3's `(get-model)` output into the falsifying bit pattern per variable
/// (`.design/stage3-bv-reconstruction.md` REQ-2 / AC-3). Extracts each `(define-fun
/// NAME () (_ BitVec W) value)` and decodes value — a hex `#x…`, a binary `#b…`, or a
/// `(_ bvK W)` literal — into a [`BvBitPattern`]. A variable Z3 omits (unconstrained)
/// is recorded as all-zeros (a concrete representative of the free choice).
#[must_use]
pub fn parse_bv_model(model: &str, vars: &[String], width: u32) -> Vec<BvBitPattern> {
    let parsed = collect_define_funs(model);
    vars.iter()
        .map(|v| {
            let value = parsed.get(v).copied().unwrap_or(0) % modulus(width);
            BvBitPattern {
                var: v.clone(),
                width,
                value,
                bits: render_bits(value, width),
            }
        })
        .collect()
}

/// Collect the `(define-fun NAME () (_ BitVec W) value)` bindings of a Z3 model into a
/// `name → value` map. The value decoder handles the three Z3 bit-vector renderings
/// (`#x…` hex, `#b…` binary, `(_ bvK W)` literal). A binding whose value does not
/// decode is skipped (the variable then reads as all-zeros — a conservative concrete
/// witness, never a panic, R-CODE-2).
fn collect_define_funs(model: &str) -> BTreeMap<String, u128> {
    let mut out = BTreeMap::new();
    let needle = "(define-fun ";
    let mut search = 0;
    while let Some(rel) = model[search..].find(needle) {
        let open = search + rel;
        let after = open + needle.len();
        let rest = &model[after..];
        let name_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let name = rest[..name_end].trim().to_string();
        match matching_paren(model, open) {
            Some(close) => {
                let inner = &model[after..close];
                if let Some(value) = decode_bv_value(inner) {
                    if !name.is_empty() {
                        out.insert(name, value);
                    }
                }
                search = close + 1;
            }
            None => search = after,
        }
    }
    out
}

/// Decode the value of a `(define-fun NAME () (_ BitVec W) value)` body into a
/// numeric bit pattern. The body is `NAME () (_ BitVec W) value`; the value is the
/// trailing token(s) after the `(_ BitVec W)` sort. Handles `#x…`, `#b…`, and the
/// `(_ bvK W)` literal form.
fn decode_bv_value(inner: &str) -> Option<u128> {
    // Anchor past the declared `(_ BitVec W)` sort; the remainder is the value.
    let value_text = match inner.find("BitVec") {
        Some(pos) => {
            let after_sort = &inner[pos + "BitVec".len()..];
            // skip the width and the closing `)` of the sort
            match after_sort.find(')') {
                Some(rp) => after_sort[rp + 1..].trim(),
                None => after_sort.trim(),
            }
        }
        None => inner.trim(),
    };
    parse_bv_literal(value_text)
}

/// Parse a single Z3 bit-vector literal (`#x3`, `#b0011`, or `(_ bv3 64)`) to a
/// `u128`. `None` on an unrecognized form (the variable then reads as all-zeros).
fn parse_bv_literal(text: &str) -> Option<u128> {
    let text = text.trim();
    if let Some(hex) = text.strip_prefix("#x") {
        return u128::from_str_radix(hex.trim(), 16).ok();
    }
    if let Some(bin) = text.strip_prefix("#b") {
        return u128::from_str_radix(bin.trim(), 2).ok();
    }
    if let Some(rest) = text.strip_prefix("(_") {
        // `(_ bvK W)` — take the `bvK` token.
        let tok = rest.split_whitespace().next()?;
        let k = tok.strip_prefix("bv")?;
        return k.parse::<u128>().ok();
    }
    None
}

/// The index of the `)` matching the `(` at `open` in `s`, or `None` if unbalanced (a
/// defensive skip, never a panic — R-CODE-2). Mirrors the nlsat model parser.
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

/// Render a numeric value as a zero-padded `0b…` binary string of exactly `width`
/// bits (`.design/stage3-bv-reconstruction.md` REQ-2 / AC-3 — the bit pattern).
fn render_bits(value: u128, width: u32) -> String {
    let mut bits = String::with_capacity(width as usize + 2);
    bits.push_str("0b");
    for i in (0..width).rev() {
        bits.push(if (value >> i) & 1 == 1 { '1' } else { '0' });
    }
    bits
}

#[cfg(test)]
mod tests {
    use super::*;
    use thermite_syntax::{BinOp, Expr};

    fn var(name: &str) -> Expr {
        Expr::Path(vec![name.to_string()])
    }

    fn lit(value: u128) -> Expr {
        Expr::IntLit {
            value,
            raw: value.to_string(),
        }
    }

    fn bin(op: BinOp, l: Expr, r: Expr) -> Expr {
        Expr::Binary {
            op,
            lhs: Box::new(l),
            rhs: Box::new(r),
        }
    }

    #[test]
    fn term_lowers_arithmetic_and_bitwise_to_fixed_width_ops() {
        // a + b * 2  →  (bvadd a (bvmul b (_ bv2 64)))
        let e = bin(BinOp::Add, var("a"), bin(BinOp::Mul, var("b"), lit(2)));
        assert_eq!(
            render_bv_term(&e, 64).unwrap(),
            "(bvadd a (bvmul b (_ bv2 64)))"
        );
        // a ^ b & c  parses as (a ^ b) & c per precedence in the AST we are handed;
        // here we build it explicitly to pin the bitwise mnemonics.
        let e = bin(BinOp::BitXor, var("a"), var("b"));
        assert_eq!(render_bv_term(&e, 32).unwrap(), "(bvxor a b)");
        // prefix ! on a term is bitwise-not.
        let e = Expr::Unary {
            op: UnaryOp::Not,
            expr: Box::new(var("a")),
        };
        assert_eq!(render_bv_term(&e, 8).unwrap(), "(bvnot a)");
    }

    #[test]
    fn literal_reduces_modulo_width() {
        // 256 ≡ 0 (mod 2^8); 255 stays.
        assert_eq!(render_bv_term(&lit(256), 8).unwrap(), "(_ bv0 8)");
        assert_eq!(render_bv_term(&lit(255), 8).unwrap(), "(_ bv255 8)");
    }

    #[test]
    fn prop_uses_unsigned_relations_and_connectives() {
        // a <= b  →  unsigned bvule (the scalar types are all unsigned).
        let e = bin(BinOp::Le, var("a"), var("b"));
        assert_eq!(render_bv_prop(&e, 64).unwrap(), "(bvule a b)");
        // a != b  →  (not (= a b)).
        let e = bin(BinOp::Ne, var("a"), var("b"));
        assert_eq!(render_bv_prop(&e, 64).unwrap(), "(not (= a b))");
        // a == b && c < d  →  (and (= a b) (bvult c d)).
        let e = bin(
            BinOp::And,
            bin(BinOp::Eq, var("a"), var("b")),
            bin(BinOp::Lt, var("c"), var("d")),
        );
        assert_eq!(render_bv_prop(&e, 64).unwrap(), "(and (= a b) (bvult c d))");
    }

    #[test]
    fn out_of_fragment_term_and_prop_are_honest_errors() {
        // A method call is outside the term fragment.
        let mc = Expr::MethodCall {
            receiver: Box::new(var("a")),
            name: "len".to_string(),
            args: vec![],
        };
        assert!(render_bv_term(&mc, 64).is_err());
        // A bare arithmetic term is not a proposition.
        let e = bin(BinOp::Add, var("a"), var("b"));
        assert!(render_bv_prop(&e, 64).is_err());
    }

    #[test]
    fn cost_cliff_detector_distinguishes_var_mul_from_literal_mul() {
        // var * var → trips the 64-bit multiplier profile.
        assert!(contains_variable_multiply(&bin(
            BinOp::Mul,
            var("a"),
            var("b")
        )));
        // var * 8 → cheap shift/add, does not trip.
        assert!(!contains_variable_multiply(&bin(
            BinOp::Mul,
            var("a"),
            lit(8)
        )));
        // nested var*var inside an add still trips.
        assert!(contains_variable_multiply(&bin(
            BinOp::Add,
            var("c"),
            bin(BinOp::Mul, var("a"), var("b"))
        )));
    }

    #[test]
    fn budget_profile_selection_is_width64_and_variable_multiply() {
        let var_mul = bin(BinOp::Eq, bin(BinOp::Mul, var("a"), var("b")), var("c"));
        // width 64 + var*var → the dedicated multiplier profile (bounded rlimit).
        let p = BvBudgetProfile::for_query(BvWidth::W64, &[&var_mul]);
        assert_eq!(p.name, "bv64-multiplier");
        assert!(p.rlimit > 0, "the multiplier profile is rlimit-bounded");
        // width 32 + var*var → default profile (the cliff is the 64-bit case).
        let p = BvBudgetProfile::for_query(BvWidth::W32, &[&var_mul]);
        assert_eq!(p.name, "bv-default");
        // width 64 + a literal multiply → default profile (cheap shift/add).
        let lit_mul = bin(BinOp::Eq, bin(BinOp::Mul, var("a"), lit(8)), var("c"));
        let p = BvBudgetProfile::for_query(BvWidth::W64, &[&lit_mul]);
        assert_eq!(p.name, "bv-default");
    }

    #[test]
    fn query_declares_bitvecs_negates_clause_and_sets_rlimit_when_bounded() {
        let q = build_bv_query(
            &["a".to_string(), "b".to_string()],
            None,
            "(= (bvmul a b) (bvmul b a))",
            64,
            &BvBudgetProfile::multiplier64_profile(),
        );
        assert!(q.contains("(set-logic QF_BV)"));
        assert!(
            q.contains("(set-option :rlimit 2000000)"),
            "bounded profile sets rlimit"
        );
        assert!(q.contains("(declare-const a (_ BitVec 64))"));
        assert!(q.contains("(assert (not (= (bvmul a b) (bvmul b a))))"));
        assert!(q.contains("(check-sat)"));
        // The default (unbounded) profile sets no rlimit.
        let q = build_bv_query(
            &["a".to_string()],
            None,
            "(= a a)",
            32,
            &BvBudgetProfile::default_profile(),
        );
        assert!(!q.contains("rlimit"), "the default profile is unbounded");
    }

    #[test]
    fn model_parser_decodes_hex_binary_and_bv_literal_to_bit_patterns() {
        let model = "(model\n  (define-fun a () (_ BitVec 64) #x0000000000000003)\n  \
                     (define-fun b () (_ BitVec 8) #b00000101)\n  \
                     (define-fun c () (_ BitVec 64) (_ bv7 64))\n)";
        let bits = parse_bv_model(
            model,
            &["a".to_string(), "b".to_string(), "c".to_string()],
            64,
        );
        assert_eq!(bits[0].var, "a");
        assert_eq!(bits[0].value, 3);
        assert!(
            bits[0].bits.ends_with("0011"),
            "a's low bits are 0011: {}",
            bits[0].bits
        );
        assert_eq!(bits[1].value, 5, "b = #b00000101 = 5");
        assert_eq!(bits[2].value, 7, "c = (_ bv7 64) = 7");
        // The render line carries the bits + hex + width.
        assert!(bits[0].render().contains("bv64"));
        assert!(bits[0].render().contains("0x"));
    }

    #[test]
    fn unconstrained_variable_reads_as_all_zeros() {
        // A variable Z3 omits from the model is a concrete all-zeros witness.
        let bits = parse_bv_model("(model)", &["z".to_string()], 16);
        assert_eq!(bits[0].value, 0);
        assert_eq!(bits[0].bits, "0b0000000000000000");
    }

    #[test]
    fn render_bits_is_zero_padded_to_width() {
        assert_eq!(render_bits(3, 8), "0b00000011");
        assert_eq!(render_bits(0, 4), "0b0000");
    }

    // ── Live QF_BV tests (REQ-2 / AC-2 / AC-3). z3 ships beside the verus
    // distribution; a CI shard without z3 skips rather than fails (the nlsat-route
    // precedent). These exercise `discharge_bv` directly against the real solver.

    fn bv_skip() -> bool {
        if BitVectorEngine::z3_present() {
            return false;
        }
        eprintln!("SKIP: z3 absent — the live QF_BV BitVector tests need the bundled z3.");
        true
    }

    /// AC-2 (engine level): a machine-valid `@bv64` clause is `Proved`. `a + b == b + a`
    /// is the wraparound-add commutativity the bit-vector route discharges with no proof
    /// block.
    #[test]
    fn live_machine_valid_clause_is_proved() {
        if bv_skip() {
            return;
        }
        let engine = BitVectorEngine::new();
        let clause = bin(
            BinOp::Eq,
            bin(BinOp::Add, var("a"), var("b")),
            bin(BinOp::Add, var("b"), var("a")),
        );
        let out = engine.discharge_bv(
            &["a".to_string(), "b".to_string()],
            None,
            &clause,
            BvWidth::W64,
        );
        assert_eq!(out, BvOutcome::Proved, "bvadd commutes at width 64");
    }

    /// AC-3 (engine level): a planted non-injective shift dies as a `Counterexample`
    /// carrying the bit pattern. `x << 1` loses the top bit, so two distinct inputs
    /// collide — z3 returns the witnessing bits.
    #[test]
    fn live_non_injective_shift_yields_counterexample_with_bits() {
        if bv_skip() {
            return;
        }
        let engine = BitVectorEngine::new();
        // req x != y ; ens@bv64 (x << 1) != (y << 1)  — false (shl-by-1 is not injective).
        let req = bin(BinOp::Ne, var("x"), var("y"));
        let clause = bin(
            BinOp::Ne,
            bin(BinOp::Shl, var("x"), lit(1)),
            bin(BinOp::Shl, var("y"), lit(1)),
        );
        let out = engine.discharge_bv(
            &["x".to_string(), "y".to_string()],
            Some(&req),
            &clause,
            BvWidth::W64,
        );
        match out {
            BvOutcome::Counterexample { bits } => {
                assert_eq!(bits.len(), 2, "a bit pattern per variable");
                assert!(bits.iter().all(|b| b.width == 64));
                // The witness falsifies: x != y but x<<1 == y<<1.
                assert_ne!(bits[0].value, bits[1].value, "x != y in the witness");
                assert_eq!(
                    (bits[0].value << 1) & u64::MAX as u128,
                    (bits[1].value << 1) & u64::MAX as u128,
                    "x<<1 == y<<1 at width 64 (the collision)"
                );
            }
            other => panic!("expected a bit-level Counterexample, got {other:?}"),
        }
    }

    /// AC-3 (engine level): an over-budget 64-bit multiplier query is reported under the
    /// dedicated budget profile and is never a silent `unknown`. A factoring-style
    /// validity (`a * b != <prime>` with non-trivial bounded factors) forces the full
    /// 64×64 bit-blast — the cost cliff — so the bounded profile reports `Timeout`. The
    /// robust invariant asserted here (across z3 versions): the outcome is never
    /// `Unknown`; when it IS a `Timeout`, it names the `bv64-multiplier` profile.
    #[test]
    fn live_multiplier_budget_is_never_silent_unknown() {
        if bv_skip() {
            return;
        }
        let engine = BitVectorEngine::new();
        // req a>1 && a<2^32 && b>1 && b<2^32 ; ens@bv64 a*b != 2^61-1 (a Mersenne prime).
        let req = bin(
            BinOp::And,
            bin(
                BinOp::And,
                bin(BinOp::Gt, var("a"), lit(1)),
                bin(BinOp::Lt, var("a"), lit(1u128 << 32)),
            ),
            bin(
                BinOp::And,
                bin(BinOp::Gt, var("b"), lit(1)),
                bin(BinOp::Lt, var("b"), lit(1u128 << 32)),
            ),
        );
        let clause = bin(
            BinOp::Ne,
            bin(BinOp::Mul, var("a"), var("b")),
            lit((1u128 << 61) - 1),
        );
        let out = engine.discharge_bv(
            &["a".to_string(), "b".to_string()],
            Some(&req),
            &clause,
            BvWidth::W64,
        );
        assert!(
            !matches!(out, BvOutcome::Unknown(_)),
            "the 64-bit multiplier cliff is NEVER a silent unknown (AC-3): {out:?}"
        );
        if let BvOutcome::Timeout { profile, .. } = &out {
            assert_eq!(
                profile, "bv64-multiplier",
                "the dedicated budget profile is named"
            );
        }
    }

    // ── The `nowrap` no-overflow side obligation (REQ-5 / AC-6, lock 3).

    fn overflow_conds(e: &Expr, width: u32) -> Vec<String> {
        let mut out = Vec::new();
        collect_overflow_conditions(e, width, &mut out).expect("renders");
        out
    }

    #[test]
    fn overflow_conditions_cover_add_sub_mul_with_the_textbook_predicates() {
        // a + b → unsigned add carry-out: result < an operand.
        assert_eq!(
            overflow_conds(&bin(BinOp::Add, var("a"), var("b")), 64),
            vec!["(bvult (bvadd a b) a)".to_string()]
        );
        // a - b → unsigned underflow: a < b.
        assert_eq!(
            overflow_conds(&bin(BinOp::Sub, var("a"), var("b")), 32),
            vec!["(bvult a b)".to_string()]
        );
        // a * b → high-half of the 2N-bit product non-zero.
        assert_eq!(
            overflow_conds(&bin(BinOp::Mul, var("a"), var("b")), 8),
            vec!["(not (= ((_ extract 15 8) (bvmul ((_ zero_extend 8) a) \
                  ((_ zero_extend 8) b))) (_ bv0 8)))"
                .to_string()]
        );
    }

    #[test]
    fn overflow_conditions_recurse_into_nested_operations() {
        // (a + b) * c emits both the inner add overflow and the outer multiply overflow,
        // with the outer multiply's left operand the wrapped `(bvadd a b)` machine value.
        let e = bin(BinOp::Mul, bin(BinOp::Add, var("a"), var("b")), var("c"));
        let conds = overflow_conds(&e, 64);
        assert_eq!(conds.len(), 2, "one condition per wrap-prone op: {conds:?}");
        assert!(
            conds.iter().any(|c| c.contains("(bvult (bvadd a b) a)")),
            "the inner add overflow is collected: {conds:?}"
        );
        assert!(
            conds
                .iter()
                .any(|c| c.contains("(bvmul ((_ zero_extend 64) (bvadd a b))")),
            "the outer multiply's operand is the wrapped inner sum: {conds:?}"
        );
    }

    #[test]
    fn non_arithmetic_clause_has_no_overflow_conditions() {
        // A pure comparison / bitwise / shift body has nothing that wraps — the
        // obligation holds vacuously (no conditions to discharge).
        assert!(overflow_conds(&bin(BinOp::Eq, var("a"), var("b")), 64).is_empty());
        assert!(overflow_conds(&bin(BinOp::BitAnd, var("a"), var("b")), 64).is_empty());
        assert!(overflow_conds(&bin(BinOp::Shl, var("a"), lit(3)), 64).is_empty());
        // A comparison over arithmetic operands still surfaces the inner overflow.
        let e = bin(BinOp::Lt, bin(BinOp::Add, var("a"), var("b")), var("c"));
        assert_eq!(
            overflow_conds(&e, 64),
            vec!["(bvult (bvadd a b) a)".to_string()]
        );
    }

    #[test]
    fn out_of_fragment_operand_is_an_honest_error() {
        // A multiply whose operand is outside the term fragment (a method call) is an
        // Err — the obligation is skipped, never silently passed.
        let mc = Expr::MethodCall {
            receiver: Box::new(var("a")),
            name: "len".to_string(),
            args: vec![],
        };
        let mut out = Vec::new();
        assert!(collect_overflow_conditions(&bin(BinOp::Add, mc, var("b")), 64, &mut out).is_err());
    }

    #[test]
    fn nowrap_query_asserts_overflow_directly_not_negated() {
        let q = build_nowrap_query(
            &["a".to_string(), "b".to_string()],
            Some("(bvult a (_ bv100 64))"),
            "(bvult (bvadd a b) a)",
            64,
            &BvBudgetProfile::default_profile(),
        );
        assert!(q.contains("(set-logic QF_BV)"));
        assert!(q.contains("(declare-const a (_ BitVec 64))"));
        assert!(
            q.contains("(assert (bvult a (_ bv100 64)))"),
            "the precondition is a hypothesis"
        );
        // The overflow disjunction is asserted directly (not wrapped in `(not …)`), so a
        // model is an overflowing input.
        assert!(q.contains("(assert (bvult (bvadd a b) a))"));
        assert!(
            !q.contains("(not (bvult (bvadd a b) a))"),
            "the overflow is NOT negated"
        );
        assert!(q.contains("(check-sat)"));
    }

    /// AC-6 (engine level): a `@bv64(nowrap)` body that cannot overflow holds — `a + b`
    /// with both operands bounded below `2^32` never carries out of 64 bits.
    #[test]
    fn live_bounded_sum_passes_the_nowrap_obligation() {
        if bv_skip() {
            return;
        }
        let engine = BitVectorEngine::new();
        // req a < 2^32 && b < 2^32 ; body a + b — provably no 64-bit overflow.
        let req = bin(
            BinOp::And,
            bin(BinOp::Lt, var("a"), lit(1u128 << 32)),
            bin(BinOp::Lt, var("b"), lit(1u128 << 32)),
        );
        let body = bin(BinOp::Add, var("a"), var("b"));
        let out = engine.discharge_nowrap(
            &["a".to_string(), "b".to_string()],
            Some(&req),
            &body,
            BvWidth::W64,
        );
        assert_eq!(
            out,
            BvOutcome::Proved,
            "a bounded 64-bit sum never overflows"
        );
    }

    /// AC-6 (engine level): an unconstrained `@bv64(nowrap)` `a + b` CAN overflow — the
    /// obligation fails with a concrete overflowing bit pattern (the witness
    /// carries out of 64 bits).
    #[test]
    fn live_unbounded_sum_fails_with_a_concrete_overflowing_input() {
        if bv_skip() {
            return;
        }
        let engine = BitVectorEngine::new();
        let body = bin(BinOp::Add, var("a"), var("b"));
        let out = engine.discharge_nowrap(
            &["a".to_string(), "b".to_string()],
            None,
            &body,
            BvWidth::W64,
        );
        match out {
            BvOutcome::Counterexample { bits } => {
                assert_eq!(bits.len(), 2, "a bit pattern per variable");
                // The witness overflows: a + b wraps below an operand at width 64.
                let mask = u64::MAX as u128;
                let wrapped = (bits[0].value + bits[1].value) & mask;
                assert!(
                    wrapped < bits[0].value || wrapped < bits[1].value,
                    "the witness carries out of 64 bits: a={}, b={}",
                    bits[0].value,
                    bits[1].value
                );
            }
            other => panic!("expected an overflowing Counterexample, got {other:?}"),
        }
    }

    /// AC-6 (engine level): the multiply-overflow predicate is well-formed SMT and z3
    /// decides it both ways. A small-width `a * b` with both operands bounded below
    /// `2^(N/2)` cannot overflow `N` bits (Proved), while the same product unconstrained
    /// CAN overflow (Counterexample). This exercises the `zero_extend`/`extract` render
    /// against the real solver — a syntax slip would surface here, not just in the unit
    /// string check.
    #[test]
    fn live_multiply_overflow_predicate_is_decided_both_ways() {
        if bv_skip() {
            return;
        }
        let engine = BitVectorEngine::new();
        let body = bin(BinOp::Mul, var("a"), var("b"));
        // Bounded below 2^4 at width 8 → a*b < 2^8 always → no overflow (Proved).
        let req = bin(
            BinOp::And,
            bin(BinOp::Lt, var("a"), lit(16)),
            bin(BinOp::Lt, var("b"), lit(16)),
        );
        let bounded = engine.discharge_nowrap(
            &["a".to_string(), "b".to_string()],
            Some(&req),
            &body,
            BvWidth::W8,
        );
        assert_eq!(
            bounded,
            BvOutcome::Proved,
            "a*b with both operands < 2^4 never overflows 8 bits"
        );
        // Unconstrained → a*b can overflow 8 bits (e.g. 16*16 = 256 ≡ 0): Counterexample.
        let unbounded = engine.discharge_nowrap(
            &["a".to_string(), "b".to_string()],
            None,
            &body,
            BvWidth::W8,
        );
        match unbounded {
            BvOutcome::Counterexample { bits } => {
                let prod = bits[0].value * bits[1].value;
                assert!(
                    prod >= 256,
                    "the witness genuinely overflows 8 bits: a={}, b={}, a*b={prod}",
                    bits[0].value,
                    bits[1].value
                );
            }
            other => panic!("expected an overflowing Counterexample, got {other:?}"),
        }
    }

    /// AC-6 (engine level): a non-arithmetic `nowrap` body holds vacuously without a
    /// solver round-trip (so it passes even with z3 absent — no skip guard needed).
    #[test]
    fn nowrap_obligation_is_vacuous_for_a_non_arithmetic_body() {
        let engine = BitVectorEngine::new();
        let body = bin(BinOp::Eq, var("a"), var("b"));
        let out = engine.discharge_nowrap(
            &["a".to_string(), "b".to_string()],
            None,
            &body,
            BvWidth::W64,
        );
        assert_eq!(
            out,
            BvOutcome::Proved,
            "no wrap-prone op → vacuously no overflow"
        );
    }
}
