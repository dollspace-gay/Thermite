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
//! | REQ-1 (frozen deterministic mutator set) | SHIPPED | `pub fn generate` walks a `FnItem.body` in source order and applies the FIXED families: operator flips (`flip_binop`), off-by-ones (`IntLit n`→`n+1`/`n-1`, skip `n-1` at 0), early returns (`return <value>` via `early_return_value` at body head — scalar zero via `zero_value_for`, OR the empty-slice literal `&[]`/`&mut []` for a reference-to-slice return so EVERY real body is scored, #48), branch swaps (negate `if`/`Expr::If` cond, else swap arms). Consumer: `check::mutation_score` in `check.rs`. |
//! | REQ-2 (deterministic order + seed + cap) | SHIPPED | a pre-order walk in a fixed family order, capped by `pub const MUTANT_CAP`; selection is the first `MUTANT_CAP` mutants in enumeration order. The seam takes the pinned `check::DEFAULT_SOLVER_SEED` (recorded in the run; the enumeration is seed-stable). Consumer: `check::mutation_score`. |
//! | REQ-3 (re-lower + re-verify vs same contract) | SHIPPED | `pub fn generate` clones the original `FnItem` and mutates only `body`; `check::mutation_score` weaves each mutant via `item_subprogram` + `thermite_lower::lower` and runs the existing `run_verus`. The `req`/`ens`/`inv`/`dec` are the original's, unchanged. |
//! | REQ-4 (KILLED vs SURVIVED) | SHIPPED | `pub fn classify_mutant` maps a `MutantOutcome`: `Proved` → SURVIVED, `Killed` (counterexample / timeout) → KILLED; a lowering failure is DROPPED (not scored). Consumer: `check::mutation_score`. |
//! | REQ-5 (kill ratio + floor gate, default 60%) | SHIPPED | `pub struct MutationScore` carries `killed`/`scored`/`survivor`; `pub fn MutationScore::kill_ratio` + `pub const MUTATION_FLOOR: f64 = 0.60`; `pub fn MutationScore::meets_floor`. A `0/0` score (no scoreable mutant) is BELOW the floor (`kill_ratio == 0.0`), not a vacuous pass — a contract that cannot be mutation-validated is gated `WeakContract` (#48, anti-Goodhart). The `cli` `--mutation-floor <FLOAT>` lever threads a non-default floor. Consumer: `check::mutation_score` + the floor gate in `check::check_file_with_options`. VERUS-ANCHORED (epic #60, REQ-11 / `.design/verified/self-verification.md` Target E): the f64 `meets_floor(0.60)` is anchored to the proved INTEGER cross-multiply `thermite_verified::meets_floor_60` (the #48 `scored == 0 ⟹ !pass` is verus-proved integer-only) by the in-module `tests::verus_anchor` f64↔integer grid (`0..=20 × 0..=20`, Option B); the grid AGREES on every cell (OQ-E: 0 divergences — the cross-multiply is the exact rational test, the f64 boundary is conformance-tested not masked). |
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
    /// (`scored == 0` — every mutant failed to lower, or the body had no mutation
    /// site AND no early-return mutant could be synthesized), the ratio is `0.0`:
    /// a contract that CANNOT be mutation-validated has NOT met the §7 bar, so the
    /// floor is NOT met (#48). This is the 0/0 backstop — a `0/0` score is treated
    /// as below-floor (gated `WeakContract`), never a silent vacuous `1.0` pass
    /// that lets an under-constraining contract certify L3 UNSCORED (§7 step 4 /
    /// `goal.md` R-DEFER-9 anti-Goodhart). With the widened early-return mutant
    /// (`early_return_value` synthesizes one for ref/slice returns too), a 0/0
    /// score is unreachable for a real `fn` body; the backstop is the floor-of-
    /// last-resort for any genuinely un-synthesizable return type.
    pub fn kill_ratio(&self) -> f64 {
        if self.scored == 0 {
            0.0
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

    // A boundary fn (`.design/boundary/ffi-boundary.md` REQ-2) has `body: None` —
    // its body is FOREIGN, so there is nothing to mutate (mutation scores a
    // KNOWN-GOOD Thermite body, §7's premise). It never reaches here in
    // production (`check.rs` routes a boundary fn to L1 before any L3 proof +
    // mutation stage), but handle `None` as an empty mutant set rather than panic
    // (R-CODE-2). The `real_body` below is the in-language body the families walk.
    let Some(real_body) = &f.body else {
        return mutants;
    };

    // Family 1: early return at body head. EVERY real `fn` body gets this mutant
    // (the §7 discriminator / value-add mutant) so the floor is never silently
    // skipped via a 0/0 score (#48). Listed first so the cap never crowds it out.
    // The returned value is the return type's canonical zero (`zero_value_for`)
    // OR, for a reference/slice return that has no scalar zero, a synthesized
    // valid early return — an empty subslice borrowing a matching slice param
    // (`&p[..0]`, valid lifetime) or the empty-slice literal `&[]` (OQ-3 widened).
    if let Some((value, desc)) = early_return_value(f) {
        let mut body = real_body.clone();
        body.stmts.insert(0, Stmt::Return(Some(value)));
        mutants.push(mutant_with_body(
            f,
            body,
            format!("insert early `return {desc}` at body head"),
        ));
    }

    // Families 2-4: walk the body collecting per-site mutated bodies. Each entry
    // is a (mutated Block, description); we rebuild the `FnItem` around it.
    let mut sink = MutantSink::new();
    sink.walk_block(real_body);
    for (body, desc) in sink.into_mutants(real_body) {
        mutants.push(mutant_with_body(f, body, desc));
    }

    mutants.truncate(MUTANT_CAP);
    mutants
}

/// Build a [`Mutant`] from the original `f` and a mutated `body` (REQ-1/REQ-3):
/// the contract and signature are cloned verbatim; only `body` changes.
fn mutant_with_body(f: &FnItem, body: Block, desc: String) -> Mutant {
    let mut item = f.clone();
    // A mutant is always a bodied in-language fn (its source `f` proved L3, so it
    // had a real body); the field is `Option<Block>` since #16, so wrap in `Some`.
    item.body = Some(body);
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
        // #92 integer operators: sound, value-distinguishable op-swaps so EVERY
        // new operator yields a kill-able mutant (the §7 battery exercises the new
        // ops, not a neutral no-op). `%`↔`/` (a remainder vs a quotient differ
        // wherever the divisor doesn't divide evenly), `<<`↔`>>` (shift direction),
        // `&`↔`|` and `^`↔`&` (distinct bit results) — each flips to an operator
        // of the SAME arity/operand types so the mutant always type-checks.
        BinOp::Rem => BinOp::Div,
        BinOp::Shl => BinOp::Shr,
        BinOp::Shr => BinOp::Shl,
        BinOp::BitAnd => BinOp::BitOr,
        BinOp::BitOr => BinOp::BitAnd,
        BinOp::BitXor => BinOp::BitAnd,
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
        BinOp::Rem => "%",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::BitXor => "^",
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

/// The early-return mutant's `(value, description)` for `f`'s return type (REQ-1,
/// OQ-3 widened by #48). EVERY real `fn` body must get an early-return mutant so
/// the §7 floor is never silently skipped via a 0/0 score:
///
/// - a scalar return uses its canonical zero (`zero_value_for`): `0` for an
///   integer prim, `false` for `bool`, `None` for an `Option`;
/// - a reference-to-slice return (`&[T]` / `&mut [T]`) has no scalar zero, so it
///   synthesizes the empty-slice literal early return `&[]` (`&mut []`). An empty
///   slice is the canonical "trivial" slice (it borrows nothing, so its lifetime
///   is always valid and it LOWERS to exec code Verus accepts — `RangeTo`
///   subslices like `&xs[..0]` are NOT supported in Verus exec position, so the
///   empty literal is the right synthesis). A weak `ens` that does not pin the
///   result (`ens result.len() <= N`) PROVES `&[]` → the mutant SURVIVES → the
///   floor gates the weak contract; a strong `ens result == xs` REJECTS `&[]`
///   (unless `xs` is empty) → the mutant is KILLED → no over-gating (#48).
/// - a bounded-`Vec` return (`Vec<T>`, `.design/basis/04-collections.md` REQ-5)
///   has no scalar zero either, so it synthesizes the EMPTY-Vec construction
///   `TVec<Suffix> { data: Vec::new() }` — the exact `thermite_lower`
///   wrapper-newtype literal a `Vec<T>` lowers to (`tvec_name` in `lower.rs`),
///   constructed empty. This MIRRORS the #48 slice precedent (`&[]` for `&[T]`)
///   for the `Vec`-return class: an empty `Vec` is the canonical "trivial" Vec
///   (`len() == 0`, always `well_formed`), so EVERY `Vec`-returning body is
///   scored rather than escaping via a 0/0 gate (#74). A strong `ens
///   result.len() == v.len() + 1` REJECTS the empty Vec (`0 != v.len()+1`) → the
///   mutant is KILLED → a genuinely-proved `push_one` SCORES the floor and
///   certifies L3 (it does NOT bypass the gate). A weak `ens result.len() <= N`
///   PROVES the empty Vec → the mutant SURVIVES → the floor still gates the weak
///   `Vec` contract (the synthesis ENABLES scoring; it does not auto-pass).
/// - a bounded-`String` return (`Type::String`, `.design/basis/07-strings.md`
///   REQ-4) has no scalar zero either, so it synthesizes the EMPTY-`TString`
///   construction `TString { data: Vec::new() }` (`empty_string_value`) — the
///   exact `thermite_lower` wrapper-newtype literal a `String` lowers to
///   (`Type::String => "TString"` in `lower.rs`), constructed empty. This MIRRORS
///   the #74 `Vec` precedent for the `String`-return class (#80): an empty
///   `TString` is the canonical "trivial" String (`len() == 0`, always
///   `well_formed`), so EVERY `String`-returning body is scored rather than
///   escaping via a 0/0 gate. A strong `ens result.len() == a.len() + b.len()`
///   (the corpus `join`) REJECTS the empty String (`0 != a.len()+b.len()` for
///   non-empty inputs) so the mutant is KILLED — a genuinely-proved `concat`
///   SCORES the floor and certifies L3 (it does NOT bypass the gate). A weak `ens
///   result.len() <= N` PROVES the empty String so the mutant SURVIVES — the floor
///   STILL gates the weak `String` contract.
///
/// `None` is returned only for a genuinely un-synthesizable return type (`Unit`, a
/// non-slice ref, a non-`Option` generic, a `Vec` of a non-Copy-primitive element
/// that the wrapper does not support) — see the 0/0 backstop in `kill_ratio`.
fn early_return_value(f: &FnItem) -> Option<(Expr, String)> {
    if let Some(zero) = zero_value_for(&f.ret) {
        return Some((zero, zero_desc(&f.ret).to_string()));
    }
    // A reference-to-slice return: the empty-slice literal `&[]` / `&mut []`.
    if let Type::Ref { mutable, inner } = &f.ret {
        if matches!(inner.as_ref(), Type::Slice(_)) {
            let empty = Expr::Ref {
                mutable: *mutable,
                expr: Box::new(empty_slice_literal()),
            };
            let amp = if *mutable { "&mut " } else { "&" };
            return Some((empty, format!("{amp}[]")));
        }
    }
    // A bounded-`Vec` return: the empty-Vec wrapper literal `TVec<Suffix> { data:
    // Vec::new() }` (#74, mirroring the #48 `&[]` slice precedent).
    if let Type::Vec(elem) = &f.ret {
        if let Some((value, desc)) = empty_vec_value(elem) {
            return Some((value, desc));
        }
    }
    // A bounded-`String` return: the empty-`TString` wrapper literal
    // `TString { data: Vec::new() }` (#80, mirroring the #74 empty-`Vec` arm
    // EXACTLY for the `Type::String` class). A `String` has no scalar zero, so
    // without this arm a `String`-returning body whose surface body has no
    // binop/off-by-one/branch site (`{ a.concat(b) }`) yields ZERO mutants → a
    // `0/0` score → the #48 anti-Goodhart backstop spuriously gates a
    // genuinely-L3-proved fn to `WeakContract`/L0. An empty `TString` is the
    // canonical "trivial" String (`len() == 0`, always `well_formed`), the exact
    // `thermite_lower::lower` wrapper-newtype literal a `String` lowers to
    // (`TString { data }` over `vstd::vec::Vec<u8>` — the single nullary `TString`
    // wrapper, no per-element suffix unlike `Vec`). A strong `ens result.len() ==
    // a.len() + b.len()` REJECTS the empty String (`0 != a.len()+b.len()` for
    // non-empty inputs) → the mutant is KILLED → `join` SCORES the floor and
    // certifies L3 (the synthesis ENABLES scoring; it does NOT bypass the gate). A
    // WEAK `ens result.len() <= N` PROVES the empty String → the mutant SURVIVES →
    // the floor STILL gates the weak String contract.
    if let Type::String = &f.ret {
        return Some(empty_string_value());
    }
    None
}

/// The empty-`String` early-return value: the wrapper-newtype struct literal
/// `TString { data: Vec::new() }` (#80). The wrapper NAME mirrors
/// `thermite_lower::lower`'s `Type::String => "TString"` — a Thermite `String`
/// lowers to the single `TString` newtype over `vstd::vec::Vec<u8>` (a nullary
/// node, fixed `u8` element — unlike `Vec<T>`'s per-element `TVec<Suffix>`, there
/// is exactly one `TString`). An empty `vstd::vec::Vec::new()` has `len() == 0`,
/// so the constructed wrapper is `well_formed` and lowers to exec code Verus
/// accepts — the same shape `empty_vec_value` synthesizes for the `Vec`-return
/// class (#74). The `data` field is the verified vstd `Vec::new()`.
fn empty_string_value() -> (Expr, String) {
    let empty = Expr::StructLit {
        path: vec!["TString".to_string()],
        // The verified `vstd::vec::Vec::new()` (an empty byte backing run).
        fields: vec![(
            "data".to_string(),
            Expr::Call {
                callee: Box::new(Expr::Path(vec!["Vec".to_string(), "new".to_string()])),
                args: Vec::new(),
            },
        )],
    };
    (empty, "TString { data: Vec::new() }".to_string())
}

/// The empty-`Vec` early-return value for a `Vec<elem>` return: the wrapper-newtype
/// struct literal `TVec<Suffix> { data: Vec::new() }` (#74). The wrapper NAME
/// mirrors `thermite_lower::lower`'s `tvec_name` — a `Vec<u64>` lowers to the
/// `TVecU64` newtype over `vstd::vec::Vec<u64>`, so the early-return mutant must
/// construct THAT newtype empty (an empty `vstd::vec::Vec` has `len() == 0`, so the
/// constructed wrapper is `well_formed` and lowers to exec code Verus accepts). The
/// `data` field is the verified vstd `Vec::new()`. Returns `None` for a `Vec`
/// element type the wrapper does not materialize (a non-Copy-primitive element —
/// `lower.rs::tvec_name` itself rejects these via `LowerError::Unsupported`), so
/// the mutant is simply not synthesized (dropped from the denominator, OQ-5), never
/// an over-gate.
fn empty_vec_value(elem: &Type) -> Option<(Expr, String)> {
    let suffix = match elem {
        Type::Prim(PrimType::U32) => "U32",
        Type::Prim(PrimType::U64) => "U64",
        Type::Prim(PrimType::Usize) => "Usize",
        Type::Prim(PrimType::Bool) => "Bool",
        _ => return None,
    };
    let wrapper = format!("TVec{suffix}");
    let empty = Expr::StructLit {
        path: vec![wrapper.clone()],
        // The verified `vstd::vec::Vec::new()` (an empty backing run).
        fields: vec![(
            "data".to_string(),
            Expr::Call {
                callee: Box::new(Expr::Path(vec!["Vec".to_string(), "new".to_string()])),
                args: Vec::new(),
            },
        )],
    };
    Some((empty, format!("{wrapper} {{ data: Vec::new() }}")))
}

/// The empty-slice literal expression `[]`. The AST has no dedicated array-literal
/// node; the lowerer emits a `Path`'s sole segment verbatim, so a single-segment
/// `Path(["[]"])` lowers to the exec literal `[]` (then wrapped in `Expr::Ref` for
/// `&[]`). Verus accepts `&[]` in exec position (a zero-length borrowed slice).
fn empty_slice_literal() -> Expr {
    Expr::Path(vec!["[]".to_string()])
}

/// The canonical zero VALUE of a SCALAR return type for the early-return mutant
/// (REQ-1, OQ-3): `0` for an integer prim, `false` for `bool`, `None` for an
/// `Option`. A type with no scalar zero (`Unit`, a `Ref`, a bare `Slice`, a
/// non-`Option` generic) yields `None` here; reference-to-slice returns are
/// handled by `early_return_value`. Returning a value of the function's return
/// type keeps the mutant well-typed so it LOWERS (only the contract should reject
/// it, not the type checker).
fn zero_value_for(ret: &Type) -> Option<Expr> {
    match ret {
        Type::Prim(PrimType::U32 | PrimType::U64 | PrimType::Usize) => Some(Expr::IntLit {
            value: 0,
            raw: "0".to_string(),
        }),
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
            // Mutation reasons over the numeric `value` only (#37); the verbatim
            // `raw` is irrelevant to off-by-one — a mutated literal is rebuilt
            // with `raw = value.to_string()` (a plain decimal) in `apply_expr`.
            Expr::IntLit { value: n, .. } => {
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
            // Basis Stage 1a (`.design/basis/01-adts.md`): the ADT expressions
            // define no NEW mutation site themselves (no off-by-one literal,
            // binop, or branch), but the honest scan descends into their
            // sub-expressions so a mutable site nested inside is still found.
            // Dead-in-1a (the ADT program dies at the validator before
            // mutation, which runs only after a successful L3 proof).
            Expr::StructLit { fields, .. } => {
                for (_, value) in fields {
                    self.scan_expr(value, ctr);
                }
            }
            Expr::Is { scrutinee, .. } => self.scan_expr(scrutinee, ctr),
            Expr::Deref(inner) => self.scan_expr(inner, ctr),
            // The prefix `!` (#92): it defines no NEW mutation site of its own (no
            // off-by-one, binop, or branch), but the honest scan descends into the
            // operand so a mutable site nested under `!` is still found.
            Expr::Unary { expr, .. } => self.scan_expr(expr, ctr),
            // A string literal (`.design/basis/07-strings.md` REQ-1) is a LEAF and
            // is NOT an off-by-one target (it is text, not a numeric literal) — it
            // defines no NEW mutation site and has no sub-expression to descend
            // into, so it joins the no-op `BoolLit`/`Path` arm.
            Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
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
            Expr::IntLit { value: n, raw } => {
                let site = self.ctr.intlit;
                self.ctr.intlit += 1;
                if site == self.action.site {
                    if let MutKind::OffByOne(v) = &self.action.kind {
                        // A mutated literal sets `raw = value.to_string()` — a
                        // plain decimal (no `_`); #37 keeps the value semantics.
                        return Expr::IntLit {
                            value: *v,
                            raw: v.to_string(),
                        };
                    }
                }
                Expr::IntLit {
                    value: *n,
                    raw: raw.clone(),
                }
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
            // Basis Stage 1a (`.design/basis/01-adts.md`): the mutation
            // rewriter rebuilds the ADT node FAITHFULLY, recursing into its
            // sub-expressions so a mutation site nested inside is applied. This
            // is the honest neutral value (an identity-preserving rebuild, not a
            // panic). Dead-in-1a (mutation runs only post-L3-proof; an ADT
            // program never reaches it — it dies at the validator).
            Expr::StructLit { path, fields } => Expr::StructLit {
                path: path.clone(),
                fields: fields
                    .iter()
                    .map(|(name, value)| (name.clone(), self.apply_expr(value)))
                    .collect(),
            },
            Expr::Is { scrutinee, variant } => Expr::Is {
                scrutinee: Box::new(self.apply_expr(scrutinee)),
                variant: variant.clone(),
            },
            Expr::Deref(inner) => Expr::Deref(Box::new(self.apply_expr(inner))),
            // The prefix `!` (#92): rebuild faithfully, recursing the operand so a
            // mutation site nested under `!` is applied (identity-preserving for the
            // node itself).
            Expr::Unary { op, expr } => Expr::Unary {
                op: *op,
                expr: Box::new(self.apply_expr(expr)),
            },
            Expr::BoolLit(b) => Expr::BoolLit(*b),
            Expr::Path(p) => Expr::Path(p.clone()),
            // A string literal (`.design/basis/07-strings.md` REQ-1) is a LEAF with
            // no mutation site (text, not an off-by-one target) — the rewriter
            // rebuilds it by IDENTITY, exactly as for `BoolLit`/`Path`.
            Expr::StrLit(s) => Expr::StrLit(s.clone()),
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

    // REQ-5 / #48 backstop: a 0/0 score (no scoreable mutant) does NOT meet the
    // floor — a contract that cannot be mutation-validated has not met the §7 bar
    // (anti-Goodhart, goal.md R-DEFER-9), so it is gated, NOT a silent vacuous
    // pass. Expected value traces to §7 step 4 (the floor catches an
    // under-constraining contract), not to forge's output (R-CHAR-3).
    #[test]
    fn empty_score_is_below_floor() {
        let score = MutationScore {
            killed: 0,
            scored: 0,
            survivor: None,
        };
        assert_eq!(score.kill_ratio(), 0.0);
        assert!(!score.meets_floor(MUTATION_FLOOR));
    }

    // #48: a reference-to-slice return synthesizes an early-return mutant (the
    // empty-slice literal `&[]`) so EVERY real `fn` body is scored — the 0/0 escape
    // is unreachable. The weak `pick` fixture from the divergence issue:
    // `fn pick(xs: &[u32]) -> &[u32] ... { xs }`. Expected mutant traces to REQ-1's
    // widened early-return family (R-CHAR-3), not to the generator's output.
    #[test]
    fn slice_return_synthesizes_early_return_mutant() {
        let f = parse_fn(
            "fn pick(xs: &[u32]) -> &[u32] req xs.len() <= 10 ens result.len() <= 10 fx pure { xs }",
        );
        let mutants = generate(&f, 0);
        assert!(
            mutants
                .iter()
                .any(|m| m.desc == "insert early `return &[]` at body head"),
            "a `&[u32]` return uses the empty-slice literal for the early-return \
             mutant: {:?}",
            mutants.iter().map(|m| &m.desc).collect::<Vec<_>>()
        );
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

    // =======================================================================
    // REQ-11 (Target E) — the Verus-anchor for the mutation FLOOR gate (#48 anti-
    // Goodhart, `.design/verified/self-verification.md` REQ-11 / AC-11c, mechanism
    // (c)).
    //
    // PLACEMENT DEVIATION (Option B, orchestrator-authorized): the design doc names
    // a `mutation::verus_anchor` block (forge is binary-only). Nested in the
    // existing `tests` module so the anti-pattern gate's `#[cfg(test)]` exemption
    // covers it. `thermite-verified` is a forge DEV-dependency.
    //
    // AC-11c — the f64↔INTEGER grid: over `killed ∈ 0..=20`, `scored ∈ 0..=20`,
    // assert the PRODUCTION f64 `MutationScore { killed, scored, survivor: None }
    // .meets_floor(0.60)` equals the VERUS-PROVED integer
    // `thermite_verified::meets_floor_60(killed, scored)` for every grid point.
    // Expected = the proved integer spec (R-CHAR-3, never forge's own f64 output).
    // The verus proof is over the INTEGER property `scored == 0 ⟹ !pass` + the
    // cross-multiply; the f64↔integer agreement is THIS test's job (OQ-E).
    //
    // OQ-E (the f64 boundary subtlety): f64 `0.60` is NOT exactly 3/5, so a ratio
    // EXACTLY on the boundary (e.g. 12/20 == 0.60) could in principle diverge by a
    // rounding ULP between the f64 `>=` and the integer cross-multiply. The grid is
    // RUN here (not assumed); if ANY cell diverges it is reported, NOT masked
    // (R-DEFER-9). The empirical expectation (from the cross-multiply being the
    // exact rational test) is 0 divergences on 0..=20.
    // =======================================================================
    mod verus_anchor {
        use super::*;
        use thermite_verified::meets_floor_60;

        /// AC-11c — the f64↔integer grid over `0..=20 × 0..=20` at the default 0.60
        /// floor: the PRODUCTION f64 `meets_floor(0.60)` agrees with the VERUS-
        /// PROVED integer `meets_floor_60` at EVERY grid point. In particular the
        /// `(0, 0)` point reads `false` on BOTH sides (the #48 anti-Goodhart gate —
        /// a `0/0` score never passes). Any divergence is asserted-OUT (and would be
        /// reported honestly, OQ-E), never silently masked.
        #[test]
        fn meets_floor_f64_matches_proved_integer_spec_over_grid() {
            let mut checked = 0usize;
            let mut divergences: Vec<(usize, usize, bool, bool)> = Vec::new();
            for killed in 0usize..=20 {
                for scored in 0usize..=20 {
                    let score = MutationScore {
                        killed,
                        scored,
                        survivor: None,
                    };
                    // R-CHAR-3: the EXPECTED verdict is the verus-proved INTEGER spec.
                    let expected = meets_floor_60(killed, scored);
                    let produced = score.meets_floor(MUTATION_FLOOR);
                    if produced != expected {
                        divergences.push((killed, scored, produced, expected));
                    }
                    checked += 1;
                }
            }
            // OQ-E: report ANY divergence explicitly (do not delete the cell).
            assert!(
                divergences.is_empty(),
                "f64↔integer floor-gate divergences (killed, scored, f64_pass, \
                 integer_pass) — OQ-E boundary divergence, report honestly: {divergences:?}"
            );
            assert_eq!(checked, 21 * 21, "the full 0..=20 × 0..=20 grid enumerated");
        }

        /// AC-11b/d (the #48 property made OBSERVABLE on BOTH representations): a
        /// `0/0` score (no scoreable mutant) reads `false` on the production f64 gate
        /// AND on the verus-proved integer spec — the anti-Goodhart gate holds in the
        /// production impl regardless of the f64 representation (the load-bearing #48
        /// invariant). Expected = the proved `scored == 0 ⟹ !pass` (R-CHAR-3).
        #[test]
        fn zero_scored_never_passes_on_both_representations() {
            let empty = MutationScore {
                killed: 0,
                scored: 0,
                survivor: None,
            };
            assert!(
                !empty.meets_floor(MUTATION_FLOOR),
                "#48: a 0/0 score must NOT pass the production f64 floor"
            );
            assert!(
                !meets_floor_60(0, 0),
                "#48: a 0/0 score must NOT pass the proved integer spec"
            );
        }
    }
}
