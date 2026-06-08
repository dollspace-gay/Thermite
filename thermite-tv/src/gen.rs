//! The OFF-CORPUS generator of well-typed SpecTherm contract clauses
//! (`.design/verified/contract-tv.md` REQ-3; epic crosslink #139 / blocker
//! #142). This is **the corpus-bound escape** — the thesis payoff of contract-TV.
//!
//! ## Why a generator (the corpus bound it un-bounds)
//!
//! Of the five existing fidelity layers, only ONE actually catches a wrong
//! lowering of a contract's MEANING — golden files. And golden files are
//! per-corpus: they certify the lowering of the EXACT programs under
//! `tests/golden/lower/`. A lowering bug that only manifests on a clause shape
//! NOT in the corpus is invisible. TV over an UNBOUNDED, deterministically
//! generated clause space removes that bound: [`generate_clauses`] emits a
//! diverse stream of well-typed contract-position [`Expr`]s, each lowered to its
//! production predicate (`thermite_lower::lower_contract_expr`) AND encoded to the
//! independent reference (`crate::ref_encode::ref_contract_pred`), and the
//! per-clause Z3 equivalence obligation (`crate::obligation::equivalence_obligation`)
//! is discharged on each. The faithful lowerer makes ALL verify; ANY counterexample
//! is a REAL off-corpus infidelity finding (`thermite-design.md` §1 — trust a
//! skeptical third party can audit, here over an unbounded clause space).
//!
//! ## The fixed typed vocabulary (so generated clauses lower + frame uniformly)
//!
//! A generated clause is a `bool`-valued predicate over a FIXED typed environment
//! — the vocabulary below — so a SINGLE obligation frame (the forge
//! off-corpus run's [`crate::obligation::ObligationFrame`]) frames EVERY generated
//! clause without per-clause type inference. The world:
//!
//! - `Seq<u32>` slice values `xs`, `ys` (seq-bound — their `@`-view is the
//!   identity);
//! - `Seq<u8>` byte-view value `s` (the #127 byte-view receiver);
//! - `int` index/scalar values `n`, `m`, `k`;
//! - the bounded-int `result: u64` (nat-coerced when compared to a `nat`-valued
//!   spec-fn call) and `old_acc: u64`;
//! - the `nat`-returning spec fn `spec_sum(Seq<u32>) -> nat`;
//! - the 8 frozen combinators, each generated with the CORRECT argument KINDS per
//!   `thermite_spec::lookup(name).arg_kinds` (`Slice`/`Index`/`Pred`/`Value`), so a
//!   generated `forall_below(xs, n, |x| …)` has an `int` index in the `Index` slot
//!   — never a slice (the #145 arg-kind discipline).
//!
//! This vocabulary is the contract between the generator (here, in the independent
//! crate) and the forge off-corpus run's frame — it is documented as the binding
//! shape both sides agree on, exactly as `thermite-design.md` §4.2 freezes the
//! sublanguage.
//!
//! ## Determinism (R-CODE-5)
//!
//! Generation is a pure function of `(seed, n)` — a self-contained SplitMix64
//! PRNG ([`Rng`]), NO `rand` crate, NO wall-clock, NO global state. The same
//! `(seed, n)` ALWAYS yields the same `Vec<Expr>` (asserted in `tests`). This is
//! the seeded-reproducibility contract (AC-7).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-3 (off-corpus generator) | SHIPPED | `pub fn generate_clauses` here — a DETERMINISTIC (SplitMix64-seeded, no `rand`/clock, R-CODE-5) generator of well-typed `bool`-valued contract-position `Expr`s over the frozen sublanguage (comparisons over all `BinOp`s, logical connectives incl. nesting, the 8 combinators with the correct arg KINDS per `thermite_spec::lookup(_).arg_kinds`, `spec_sum` calls, `result`/`old(acc)`, byte-view method calls, casts). Non-test consumer: the forge off-corpus run `forge::contract_tv::run_generated` (lowers each via `thermite_lower::lower_contract_expr` → TV-checks via `equivalence_obligation`). Pure generation in the INDEPENDENT crate — no `thermite-lower` dep (AC-6 intact). Coverage + reproducibility asserted in `tests` + `forge/tests/contract_tv_conformance.rs` (AC-7). |

use thermite_syntax::ast::{BinOp, Expr, PrimType, Type, UnaryOp};

/// A self-contained SplitMix64 PRNG (R-CODE-5: deterministic, seeded, no `rand`
/// crate, no wall-clock). SplitMix64 is a tiny, well-distributed integer
/// generator — exactly enough to drive the structural choices below
/// reproducibly. The same `seed` ALWAYS produces the same stream.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        // Offset the seed so `seed == 0` is not a degenerate all-zero stream.
        Rng {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        }
    }

    /// The next 64-bit value (SplitMix64). Pure state advance — deterministic.
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniform value in `0..bound` (`bound >= 1`). For `bound == 0` returns 0
    /// (never a panic — R-CODE-2; the generator never passes 0).
    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        (self.next_u64() % bound as u64) as usize
    }

    /// Pick one element of a non-empty slice of `Copy` values (deterministic).
    fn pick<T: Copy>(&mut self, choices: &[T]) -> T {
        choices[self.below(choices.len())]
    }
}

/// The fixed `Seq<u32>` slice value names (seq-bound).
const SEQ_NAMES: &[&str] = &["xs", "ys"];
/// The fixed `int` scalar/index value names.
const INT_NAMES: &[&str] = &["n", "m", "k"];
/// The fixed `u64` bounded-int names (nat-coerced against a `nat` term).
const NAT_COERCE_NAMES: &[&str] = &["result", "old_acc"];

/// Generate `n` well-typed, `bool`-valued contract-position [`Expr`]s over the
/// frozen SpecTherm sublanguage, deterministically from `seed` (REQ-3). Each is a
/// valid contract clause the production lowerer (`thermite_lower::lower_contract_expr`)
/// and the independent reference encoder (`crate::ref_encode::ref_contract_pred`)
/// both encode, so the per-clause TV obligation runs on each (the off-corpus
/// fidelity check — AC-7).
///
/// The stream is DIVERSE (not `n` copies of `true`): each clause is built by a
/// bounded recursive descent that, at the root, picks among a comparison (over any
/// `BinOp`), a logical connective (`&&`/`||`/`!`, with nesting), a combinator call
/// (all 8, with the correct arg kinds), a byte-view comparison, or a `spec_sum`
/// comparison. The construct mix is asserted in `tests`.
pub fn generate_clauses(seed: u64, n: usize) -> Vec<Expr> {
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        // ~1 in 6 clauses is a STANDALONE byte-view comparison (a frozen contract
        // construct, F3); the rest are arbitrary nested predicates over the
        // comparison/connective/combinator/nat space. Keeping byte-view a top-level
        // STANDALONE form (not mixed into the recursive connective descent) means a
        // non-byte-view clause is FULLY checkable off-corpus — the forge run skips a
        // byte-view clause HONESTLY (String/body-TV scope) without that skip
        // contaminating the connective/combinator clauses (which DO verify).
        if rng.below(6) == 0 {
            out.push(gen_byteview_cmp(&mut rng, MAX_DEPTH));
        } else {
            out.push(gen_bool(&mut rng, 0));
        }
    }
    out
}

/// The recursion-depth cap (a generated clause never nests past this — keeps the
/// emitted predicate small + the obligation fast, and bounds the recursion so the
/// generator always terminates, R-CODE-2).
const MAX_DEPTH: usize = 3;

/// Generate a `bool`-valued predicate at recursion `depth`. At the cap, only leaf
/// `bool` forms (a comparison / combinator / byte-view test / nat comparison) are
/// produced so the recursion always bottoms out.
///
/// NOTE on byte-view (`s.byte_at(i)`/`s.len()`): a generated byte-view clause IS
/// emitted (it is a frozen contract construct, F3 in the teeth) — but the FORGE
/// off-corpus run frames `s` as a `Seq<u8>` directly, so production's byte-view
/// rewrite (which keys on a `&String` param + the TString wrapper) does NOT apply
/// uniformly and the run reports such a clause `Skipped` (an HONEST not-checked,
/// not a false faithful — `forge::contract_tv`). Byte-view lowering FIDELITY is
/// covered by the F3 teeth + the String corpus programs; framing it off-corpus
/// needs the TString-wrapper bridge, which is String/body-TV scope (#139 step 2).
fn gen_bool(rng: &mut Rng, depth: usize) -> Expr {
    // At the depth cap, a leaf bool form only (no further nesting). Byte-view is NOT
    // in this recursive descent (it is a top-level STANDALONE form in
    // `generate_clauses`) so a nested connective/combinator clause stays fully
    // off-corpus-checkable.
    let choice = if depth >= MAX_DEPTH {
        rng.below(4)
    } else {
        rng.below(7)
    };
    match choice {
        // (0) A comparison between two int/nat terms over any comparison BinOp.
        0 => gen_comparison(rng, depth),
        // (1) A combinator call (one of the 8 frozen, correct arg kinds).
        1 => gen_combinator(rng, depth),
        // (2) A `result`/`old_acc` compared to `spec_sum(seq)` over ANY op (the
        //     `Eq` coercion shape + the NON-`Eq` bare shapes — #147 gap #2).
        2 => gen_nat_cmp(rng),
        // (3) A CAST-`<`-class comparison (`n as u32 < k`) — the #146/#148 off-corpus
        //     regression guard (#147). A leaf form, so it appears at every depth.
        3 => gen_cast_lt(rng, depth),
        // (4) A logical AND of two sub-predicates (nesting).
        4 => Expr::Binary {
            op: BinOp::And,
            lhs: Box::new(gen_bool(rng, depth + 1)),
            rhs: Box::new(gen_bool(rng, depth + 1)),
        },
        // (5) A logical OR of two sub-predicates (nesting).
        5 => Expr::Binary {
            op: BinOp::Or,
            lhs: Box::new(gen_bool(rng, depth + 1)),
            rhs: Box::new(gen_bool(rng, depth + 1)),
        },
        // (6) A logical NOT of a sub-predicate (nesting).
        _ => Expr::Unary {
            op: UnaryOp::Not,
            expr: Box::new(gen_bool(rng, depth + 1)),
        },
    }
}

/// One of the comparison operators (the `==`/`<=`-class — the F1 teeth surface).
const CMP_OPS: &[BinOp] = &[
    BinOp::Eq,
    BinOp::Ne,
    BinOp::Lt,
    BinOp::Le,
    BinOp::Gt,
    BinOp::Ge,
];

/// A comparison between two `int`-valued scalar terms over any comparison op.
fn gen_comparison(rng: &mut Rng, depth: usize) -> Expr {
    let op = rng.pick(CMP_OPS);
    Expr::Binary {
        op,
        lhs: Box::new(gen_int(rng, depth)),
        rhs: Box::new(gen_int(rng, depth)),
    }
}

/// The arithmetic operators a generated `int` subterm may combine over.
const ARITH_OPS: &[BinOp] = &[BinOp::Add, BinOp::Sub, BinOp::Mul];

/// An `int`-valued scalar term: a named int var (`n`/`m`/`k`), an int literal, a
/// `count_where(seq, pred)` result (a combinator returning a count), or — below
/// the depth cap — an arithmetic combination of two int subterms.
fn gen_int(rng: &mut Rng, depth: usize) -> Expr {
    let choice = if depth >= MAX_DEPTH {
        rng.below(2)
    } else {
        rng.below(3)
    };
    match choice {
        // A named int scalar.
        0 => path(rng.pick(INT_NAMES)),
        // A small int literal (0..16) — a bounded value so combinations stay small.
        1 => int_lit(rng.below(16) as u128),
        // An arithmetic combination (only below the cap).
        _ => Expr::Binary {
            op: rng.pick(ARITH_OPS),
            lhs: Box::new(gen_int(rng, depth + 1)),
            rhs: Box::new(gen_int(rng, depth + 1)),
        },
    }
}

/// The NON-`Eq` nat-comparison ops the generator now EXERCISES (#147 / regression-
/// guarding #146/#148 off-corpus). Production coerces `as nat` ONLY on `Eq`
/// (`lower_nat_equality` is `Eq`-only); a `<=`/`<`/`>`/`>=`/`!=` comparison of a
/// bounded `u64` to a `nat`-valued `spec_sum(seq)` is lowered BARE (`acc <=
/// spec_sum(xs)`), which verus accepts as a mixed `u64`/`nat` comparison. These are
/// exactly the clauses #147 gap #2 added to `ref_encode` (Eq-only coercion), so
/// generating them CONFIRMS the reference matches production's Eq-only rule
/// off-corpus (a divergence here = the reference coerced the wrong op).
const NAT_CMP_OPS: &[BinOp] = &[
    BinOp::Eq,
    BinOp::Ne,
    BinOp::Lt,
    BinOp::Le,
    BinOp::Gt,
    BinOp::Ge,
];

/// A `result`/`old_acc` (a bounded `u64`) compared to a `nat`-valued
/// `spec_sum(seq)` call over ANY comparison op (#147): the `Eq` coercion shape
/// (`result == spec_sum(xs)` → `result as nat == spec_sum(xs)`) AND — newly (#147
/// gap #2) — the NON-`Eq` BARE shapes (`acc <= spec_sum(xs)`, `i != spec_sum`),
/// which production lowers WITHOUT the `as nat` coercion (it is `Eq`-only). The
/// off-corpus run thus exercises BOTH coercion branches: a divergence on a non-`Eq`
/// clause = the reference applied the coercion on the wrong op (the #147 gap #2
/// regression guard). Verus accepts the mixed `u64`/`nat` comparison directly, so
/// every op is well-typed on BOTH encoders.
fn gen_nat_cmp(rng: &mut Rng) -> Expr {
    let op = rng.pick(NAT_CMP_OPS);
    let scalar = path(rng.pick(NAT_COERCE_NAMES));
    let call = Expr::Call {
        callee: Box::new(path("spec_sum")),
        args: vec![path(rng.pick(SEQ_NAMES))],
    };
    // Randomize which side the nat call is on (both orders are valid clauses). For a
    // non-`Eq` op the SIDE matters to production's text (`acc <= spec_sum` vs
    // `spec_sum <= acc`) but BOTH lower bare — the reference matches either way.
    if rng.below(2) == 0 {
        Expr::Binary {
            op,
            lhs: Box::new(scalar),
            rhs: Box::new(call),
        }
    } else {
        Expr::Binary {
            op,
            lhs: Box::new(call),
            rhs: Box::new(scalar),
        }
    }
}

/// A CAST-`<`-class comparison (#147 — the off-corpus regression guard for the
/// #146/#148 cast-paren fix): a `Cast` LEFT operand of a `<`-LEADING comparison op
/// (`<`/`<=`), e.g. `n as u32 < k`, `n as nat <= m`. This is EXACTLY the class the
/// generator previously AVOIDED (`CAST_SAFE_CMP_OPS` / the `gen_pred_closure`
/// cast-LHS only used non-`<`-leading ops) because `x as u32 < 33` mis-parsed as a
/// generic-arg list — the bug #146/#148 FIXED in production (`lower_binary_operand`)
/// and #147 gap #2 mirrored in `ref_encode` (`encode_binary_operand`). Generating it
/// now CONFIRMS the fix holds off-corpus on BOTH encoders: a DIVERGENCE here is a
/// real off-corpus hole in the #146/#148 fix (report loudly + file a blocker).
///
/// The cast target is an integer prim (`u32`) or `nat`; the RHS is an `int` scalar
/// term (so the comparison is well-typed: `n as u32` is `u32`, `n as nat`/`int` is
/// the arithmetic ladder — both compare to a small literal / int var). The op is
/// drawn from the `<`-leading set ONLY (the class under guard).
fn gen_cast_lt(rng: &mut Rng, depth: usize) -> Expr {
    // The `<`-leading comparison ops — the exact ambiguity surface (#146/#148).
    const LT_LEADING_CMP: &[BinOp] = &[BinOp::Lt, BinOp::Le];
    let op = rng.pick(LT_LEADING_CMP);
    // Cast an `int` scalar to a `u32` (the bounded-prim cast) or `nat`/`int` (the
    // arithmetic-ladder cast) — both are the cast LEFT operand the paren guards.
    let cast_ty = match rng.below(3) {
        0 => Type::Prim(PrimType::U32),
        1 => Type::Named("nat".to_string()),
        _ => Type::Named("int".to_string()),
    };
    let lhs = Expr::Cast {
        expr: Box::new(gen_int(rng, depth + 1)),
        ty: cast_ty,
    };
    // The RHS: a small literal (a `u32`/`int`-comparable bound) — keeps the
    // comparison well-typed against any cast target.
    let rhs = int_lit(rng.below(64) as u128);
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// A byte-view comparison (the #127 surface): either `s.byte_at(i) <op> <u32>`
/// (the byte accessor, an `int` index) or `s.len() <op> n` (the length accessor).
fn gen_byteview_cmp(rng: &mut Rng, depth: usize) -> Expr {
    let op = rng.pick(CMP_OPS);
    if rng.below(2) == 0 {
        // `s.byte_at(i) <op> <u32 literal>` — the byte accessor with an int index.
        let idx = gen_int(rng, depth + 1);
        let recv = Expr::MethodCall {
            receiver: Box::new(path("s")),
            name: "byte_at".to_string(),
            args: vec![idx],
        };
        Expr::Binary {
            op,
            lhs: Box::new(recv),
            rhs: Box::new(int_lit(rng.below(256) as u128)),
        }
    } else {
        // `s.len() <op> n` — the length accessor.
        let recv = Expr::MethodCall {
            receiver: Box::new(path("s")),
            name: "len".to_string(),
            args: vec![],
        };
        Expr::Binary {
            op,
            lhs: Box::new(recv),
            rhs: Box::new(gen_int(rng, depth + 1)),
        }
    }
}

/// The 8 frozen combinator names (mirrors `thermite_spec::all()`; the arg kinds
/// are read from the registry per-call so this list never drifts from the
/// registry's frozen arities).
const COMBINATORS: &[&str] = &[
    "sorted",
    "forall_in",
    "exists_in",
    "count_where",
    "permutation_of",
    "disjoint",
    "forall_below",
    "forall_from",
];

/// Generate a frozen-combinator call with the CORRECT argument kinds per the
/// registry (REQ-3). The arg KINDS come from `thermite_spec::lookup(name).arg_kinds`
/// (`Slice`/`Index`/`Pred`/`Value`) — so a `forall_below(xs, n, |x| …)` has an
/// `int` index in its `Index` slot, never a slice (the #145 discipline the
/// reference encoder also honors). A non-`bool` combinator (`count_where` →
/// `nat`) is wrapped in a comparison so the generated clause stays `bool`-valued.
fn gen_combinator(rng: &mut Rng, depth: usize) -> Expr {
    use thermite_spec::{ArgKind, ResultKind};
    let name = rng.pick(COMBINATORS);
    // The registry is the frozen ground truth for the arg kinds + arity + result.
    let Some(sig) = thermite_spec::lookup(name) else {
        // Unreachable in practice (COMBINATORS mirrors the registry); fall back to
        // a leaf comparison rather than panic (R-CODE-2).
        return gen_comparison(rng, depth);
    };
    let args: Vec<Expr> = sig
        .arg_kinds
        .iter()
        .map(|kind| match kind {
            // A slice param — a seq value name (xs/ys).
            ArgKind::Slice => path(rng.pick(SEQ_NAMES)),
            // An int index bound — a scalar int term (NEVER a slice — #145).
            ArgKind::Index => gen_int(rng, depth + 1),
            // The predicate closure slot — `|x| <bool over x>`.
            ArgKind::Pred => gen_pred_closure(rng),
            // A plain scalar value.
            ArgKind::Value => gen_int(rng, depth + 1),
        })
        .collect();
    let call = Expr::Call {
        callee: Box::new(path(name)),
        args,
    };
    match sig.result {
        // A `bool`-valued combinator IS the predicate.
        ResultKind::Bool => call,
        // A `usize`/`nat`-valued combinator (`count_where`) → wrap in a comparison
        // so the clause stays `bool`-valued. The RHS is a small LITERAL (not a
        // named `int` var): `count_where` returns `nat`, and comparing it to a
        // possibly-NEGATIVE `int` var (`k`) is where production's `nat`-coercion
        // (`k as nat`) and the reference's name-keyed coercion DIVERGE for k<0 (a
        // generator artifact, NOT a lowering bug — #139/#142). A non-negative
        // literal is coercion-neutral on both encoders, so the `count_where` clause
        // is faithfully checked. The op is `==` (the coercion-covered shape).
        ResultKind::Usize => Expr::Binary {
            op: BinOp::Eq,
            lhs: Box::new(call),
            rhs: Box::new(int_lit(rng.below(8) as u128)),
        },
    }
}

/// A predicate closure `|x| <bool predicate over x>` for a combinator `Pred` slot.
/// The body is a comparison of the closure-bound element `x` (a `u32`) against a
/// small literal — the F2 closure-predicate surface (`x <= 10` vs `x < 10` is the
/// canonical wrong-predicate infidelity the obligation catches).
fn gen_pred_closure(rng: &mut Rng) -> Expr {
    // Occasionally cast the bound element `as u32` (exercises the #122 cast path).
    // #147: the cast-LHS body now uses ANY comparison op INCLUDING the `<`-leading
    // `<`/`<=` — the EXACT class #146/#148 fixed (production `lower_binary_operand`)
    // and #147 gap #2 mirrored (`ref_encode::encode_binary_operand`). Previously the
    // generator AVOIDED `<`-leading ops on a cast LHS (the now-removed
    // `CAST_SAFE_CMP_OPS`) because `x as u32 < 33` mis-parsed as a generic-arg list;
    // emitting it now CONFIRMS the paren fix holds off-corpus on BOTH encoders inside
    // a combinator predicate (a divergence here = a hole in the #146/#148 fix).
    let (lhs, op) = if rng.below(3) == 0 {
        (
            Expr::Cast {
                expr: Box::new(path("x")),
                ty: Type::Prim(PrimType::U32),
            },
            rng.pick(CMP_OPS),
        )
    } else {
        (path("x"), rng.pick(CMP_OPS))
    };
    Expr::Closure {
        params: vec!["x".to_string()],
        body: Box::new(Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(int_lit(rng.below(64) as u128)),
        }),
    }
}

// ---- small AST constructors -------------------------------------------------

/// A single-segment path (a var reference).
fn path(name: &str) -> Expr {
    Expr::Path(vec![name.to_string()])
}

/// An integer literal carrying the value as its own raw spelling.
fn int_lit(value: u128) -> Expr {
    Expr::IntLit {
        value,
        raw: value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// REQ-3 / AC-7: the generator is DETERMINISTIC — the same `(seed, n)` yields
    /// the identical clause stream (R-CODE-5). Two runs at the same seed are equal;
    /// a different seed diverges (so it is not constant).
    #[test]
    fn deterministic_and_seed_sensitive() {
        let a = generate_clauses(42, 50);
        let b = generate_clauses(42, 50);
        assert_eq!(a, b, "same seed must reproduce the identical stream");
        let c = generate_clauses(43, 50);
        assert_ne!(a, c, "a different seed must produce a different stream");
        assert_eq!(a.len(), 50);
    }

    /// REQ-3: the stream is DIVERSE — not `n` copies of one shape. Over a 200-clause
    /// run every top-level construct family appears (comparisons, connectives,
    /// combinators, byte-view, casts), and no single clause dominates. This is the
    /// "report the construct coverage" honesty check.
    #[test]
    fn diverse_construct_coverage() {
        let clauses = generate_clauses(7, 200);
        let cov = coverage(&clauses);
        assert!(cov.comparisons >= 5, "comparisons: {}", cov.comparisons);
        assert!(cov.connectives >= 5, "connectives: {}", cov.connectives);
        assert!(cov.combinators >= 5, "combinators: {}", cov.combinators);
        assert!(cov.byteview >= 5, "byteview: {}", cov.byteview);
        assert!(cov.casts >= 1, "casts: {}", cov.casts);
        // #147: the off-corpus run must now EXERCISE the cast-`<` class (a `Cast`
        // left operand of a `<`-leading op) AND non-`Eq` nat comparisons — the
        // #146/#148 regression-guard surface. Both must appear (else the guard is
        // vacuous).
        assert!(cov.cast_lt >= 1, "cast-`<` clauses: {}", cov.cast_lt);
        assert!(
            cov.non_eq_nat_cmp >= 1,
            "non-Eq nat comparisons: {}",
            cov.non_eq_nat_cmp
        );
        // Not all the same clause (diversity sanity).
        let first = &clauses[0];
        assert!(
            clauses.iter().any(|c| c != first),
            "all 200 clauses identical — not diverse"
        );
    }

    /// A construct-coverage tally over a clause stream (the breakdown reported in
    /// the off-corpus run). A clause contributes to MULTIPLE buckets (a combinator
    /// clause may also nest a comparison).
    #[derive(Default, Debug)]
    struct Coverage {
        comparisons: usize,
        connectives: usize,
        combinators: usize,
        byteview: usize,
        casts: usize,
        /// A `Cast` LEFT operand of a `<`-leading comparison op (`<`/`<=`) — the
        /// #146/#148 ambiguity class the generator now EXERCISES (#147).
        cast_lt: usize,
        /// A NON-`Eq` comparison whose other operand is a `nat`-valued spec-fn /
        /// combinator call (`acc <= spec_sum(xs)`, `k < count_where(..)`) — the
        /// #147 gap #2 non-Eq-coercion surface.
        non_eq_nat_cmp: usize,
    }

    /// Is `e` a call to a `nat`-returning spec fn / combinator (`spec_sum` or
    /// `count_where`) — the nat-valued term in a comparison (mirrors the off-corpus
    /// `nat_fns`)?
    fn is_nat_call(e: &Expr) -> bool {
        if let Expr::Call { callee, .. } = e {
            if let Expr::Path(segs) = callee.as_ref() {
                return matches!(segs.join("::").as_str(), "spec_sum" | "count_where");
            }
        }
        false
    }

    fn coverage(clauses: &[Expr]) -> Coverage {
        let mut c = Coverage::default();
        for cl in clauses {
            tally(cl, &mut c);
        }
        c
    }

    fn tally(e: &Expr, c: &mut Coverage) {
        match e {
            Expr::Binary { op, lhs, rhs } => {
                if matches!(op, BinOp::And | BinOp::Or) {
                    c.connectives += 1;
                } else if matches!(
                    op,
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
                ) {
                    c.comparisons += 1;
                    // A `Cast` LEFT operand of a `<`-leading op — the #146/#148
                    // ambiguity class (#147).
                    if matches!(op, BinOp::Lt | BinOp::Le)
                        && matches!(lhs.as_ref(), Expr::Cast { .. })
                    {
                        c.cast_lt += 1;
                    }
                    // A NON-`Eq` comparison against a `nat`-valued call (#147 gap #2).
                    if !matches!(op, BinOp::Eq) && (is_nat_call(lhs) || is_nat_call(rhs)) {
                        c.non_eq_nat_cmp += 1;
                    }
                }
                tally(lhs, c);
                tally(rhs, c);
            }
            Expr::Unary { expr, .. } => {
                c.connectives += 1;
                tally(expr, c);
            }
            Expr::Call { callee, args } => {
                if let Expr::Path(segs) = callee.as_ref() {
                    if thermite_spec::lookup(&segs.join("::")).is_some() {
                        c.combinators += 1;
                    }
                }
                for a in args {
                    tally(a, c);
                }
            }
            Expr::MethodCall { name, args, .. } => {
                if name == "byte_at" || name == "len" {
                    c.byteview += 1;
                }
                for a in args {
                    tally(a, c);
                }
            }
            Expr::Cast { expr, .. } => {
                c.casts += 1;
                tally(expr, c);
            }
            Expr::Closure { body, .. } => tally(body, c),
            _ => {}
        }
    }
}
