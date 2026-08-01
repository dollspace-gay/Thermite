//! `forge/src/mutation.rs` — §7 step 4 of the vacuity battery: mutation scoring
//! (`thermite-design.md` §7 line 224, "operator flips, off-by-ones, early
//! returns, branch swaps — fixed deterministic mutator set"). Given a `fn` whose
//! body already verifies L3, this module generates a frozen, deterministic
//! set of mutants of that body (the contract untouched), re-lowers + re-verifies
//! each against the same contract through the existing verus driver + proof
//! cache, and scores the kill ratio (`killed / scored`). A mutant verus
//! rejects is killed (the contract caught the wrong body — good); a mutant
//! verus proves is a survivor (the contract cannot tell the mutant from the
//! body — too weak). A configurable floor (default 60%, §7) gates
//! certification: below the floor the item does not certify and the surviving
//! mutants are the precise strengthening prompt.
//!
//! Governing design: `.design/forge/mutation-scoring.md`.
//!
//! ## Polarity
//!
//! A mutant is a wrong body. If verus still proves it against the
//! contract, the contract is satisfied by both the right body and the wrong one:
//! it under-specifies. So `Proved` = survived = a hole in the contract; a
//! verus failure = killed = the contract did its job (REQ-4). This is the same
//! polarity inversion #13's harnesses use, applied to the body.
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=forge-mutation-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-MUTATION-AFTER-L3-CACHE | shipped | `forge/src/mutation.rs` | Mutation gate runs after L3 and reuses proof cache |  |
//! | REQ-FORGE-MUTATION-CERT-FIELDS | shipped | `forge/src/mutation.rs` | Mutation result certificate fields |  |
//! | REQ-FORGE-MUTATION-DETERMINISM | shipped | `forge/src/mutation.rs` | Deterministic mutation kill ratio |  |
//! | REQ-FORGE-MUTATION-FLOOR-GATE | shipped | `forge/src/mutation.rs` | Mutation kill-ratio floor gate |  |
//! | REQ-FORGE-MUTATION-FROZEN-SET | shipped | `forge/src/mutation.rs` | Frozen deterministic mutator set |  |
//! | REQ-FORGE-MUTATION-ORDER-CAP | shipped | `forge/src/mutation.rs` | Deterministic mutant order, seed, and cap |  |
//! | REQ-FORGE-MUTATION-POLARITY | shipped | `forge/src/mutation.rs` | Killed versus survived mutant polarity |  |
//! | REQ-FORGE-MUTATION-SAME-CONTRACT | shipped | `forge/src/mutation.rs` | Mutants re-verify against the original contract |  |
//! <!-- /generated:reqs -->
//!
//! ## Cluster C10 — ergonomics ripple (`.design/basis/11-ergonomics.md`, #112)
//!
//! <!-- generated:reqs view=forge-mutation-ergonomics-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-MUTATION-MATCH-GUARD | shipped | `forge/src/mutation.rs` | Mutation scans match-arm guards |  |
//! <!-- /generated:reqs -->

use thermite_syntax::{BinOp, Block, Expr, FnItem, Item, PrimType, Stmt, StructItem, Type};

/// The fixed budget on the number of mutants scored per `fn` (REQ-2; OQ-2). §7
/// says "budgeted" without a number; this is a documented `const` (R-CODE-5 —
/// the budget is a fixed input, not wall-clock). Each mutant is a full verus run
/// (cheap on a cache hit, #8), so the cap bounds the gate's cost. The corpus
/// `fn`s produce on the order of tens of mutants; `64` covers them while bounding
/// a pathologically large body. Selection when the candidate count exceeds the
/// cap is the first `MUTANT_CAP` mutants in the deterministic enumeration order
/// (REQ-2).
pub const MUTANT_CAP: usize = 64;

/// The default mutation kill-ratio floor (`thermite-design.md` §7 "a
/// configurable floor (default 60%)"). `kill_ratio >= MUTATION_FLOOR` certifies;
/// below it the item does not certify (verdict-in-cert reject). The `cli`
/// `--mutation-floor <FLOAT>` lever overrides it; a non-default floor is an
/// explicit, documented choice (mirroring the existing `--rlimit` lever).
pub const MUTATION_FLOOR: f64 = 0.60;

/// One generated mutant: a `FnItem` with the same contract as the original and a
/// mutated body, plus a human description naming the change (REQ-1). The
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
/// inverted from the L3 proof: a verus success on a wrong body means the
/// contract did not catch the change → survived; a verus failure means the
/// contract caught it → killed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutantOutcome {
    /// verus proved the mutant — the contract holds for the wrong body too, so it
    /// cannot distinguish the mutant: a survivor (the contract is too weak here).
    Survived,
    /// verus did not prove the mutant (a counterexample or a timeout — OQ-4): the
    /// contract caught the change. Killed.
    Killed,
}

/// Map a verus verdict polarity to a [`MutantOutcome`] (REQ-4). `proved == true`
/// (verus succeeded on the wrong body) is a survivor; `proved == false` (a
/// counterexample, or a timeout counted killed per OQ-4) is killed. This is the
/// single classification seam `check::mutation_score` calls; a mutant that fails
/// to lower is dropped before this (not scored, OQ-5).
pub fn classify_mutant(proved: bool) -> MutantOutcome {
    if proved {
        MutantOutcome::Survived
    } else {
        MutantOutcome::Killed
    }
}

/// The result of scoring a `fn`'s frozen mutant set (REQ-5). `killed`/`scored`
/// are over the mutants that lowered + ran (un-lowerable mutants are dropped from
/// the denominator, OQ-5). `survivor` is a representative surviving mutant's
/// description (the first survivor in deterministic enumeration order, REQ-2), or
/// `None` when every scored mutant was killed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationScore {
    /// Mutants verus failed to prove (the contract caught them).
    pub killed: usize,
    /// Mutants that lowered + ran and were not proved equivalent (the kill-ratio
    /// denominator). Excludes un-lowerable mutants (OQ-5) and mutants Verus proved
    /// observably equivalent to the body under the precondition
    /// (`.design/forge/equivalent-mutants.md` REQ-2/REQ-4, #101 — a true
    /// equivalent mutant is not contract weakness, so it drops from the
    /// denominator rather than depressing the ratio).
    pub scored: usize,
    /// The count of survivors Verus proved observably equivalent to the body
    /// under `req` (`.design/forge/equivalent-mutants.md` REQ-2/REQ-4, #101). A
    /// proved-equivalent mutant is excluded from both the survivor set and
    /// `scored`; this field records how many were so excluded (a transparency
    /// datum, not a denominator input — `scored` is already net of them). `0`
    /// when no survivor was proved equivalent (the entire pre-#101 corpus).
    pub equivalent: usize,
    /// A representative surviving mutant's description (the §7 strengthening
    /// prompt), or `None` if every scored mutant was killed or proved equivalent.
    /// A proved-equivalent mutant is not recorded here (it is not a survivor —
    /// only a distinguishing survivor remains, REQ-3).
    pub survivor: Option<String>,
}

impl MutationScore {
    /// The kill ratio `killed / scored` (REQ-5). When no mutant was scored
    /// (`scored == 0` — every mutant failed to lower, or the body had no mutation
    /// site and no early-return mutant could be synthesized), the ratio is `0.0`:
    /// a contract that cannot be mutation-validated has not met the §7 bar, so the
    /// floor is not met (#48). This is the 0/0 backstop — a `0/0` score is treated
    /// as below-floor (gated `WeakContract`) rather than a vacuous `1.0` pass
    /// that would let an under-constraining contract certify L3 unscored (§7 step 4 /
    /// `goal.md` R-DEFER-9 anti-Goodhart). With the widened early-return mutant
    /// (`early_return_value` synthesizes one for ref/slice returns too), a 0/0
    /// score is unreachable for a real `fn` body; the backstop is the floor-of-
    /// last-resort for any un-synthesizable return type.
    ///
    /// The §7 equivalent-mutant exclusion (`.design/forge/equivalent-mutants.md`
    /// REQ-4, #101) preserves this backstop: `scored` is already net of
    /// proved-equivalent mutants, so a fn all of whose mutants are killed or
    /// proved-equivalent — and none killed — reduces to `0/0`, which this
    /// backstop still gates `WeakContract` (the degenerate `refuse(x) req x == 0
    /// ens result == 0 { x }`, AC-5). Exclusion narrows the denominator without
    /// opening a vacuous `1.0` pass for a fn the battery could not exercise.
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

/// Generate the frozen, deterministic mutant set of `f`'s body (REQ-1/REQ-2).
///
/// The walk is pre-order over the body in source order; at each site the fixed
/// mutator families are applied in this fixed family order:
///   1. **early return** — one mutant inserting `return <zero-of-ret-type>` at
///      the front of the body block (skipped when the return type has no
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
/// (REQ-2). Each mutant is the original `f` with only `body` changed — the
/// contract (`req`/`ens`/`fx`, loop `inv`/`dec`) is untouched (REQ-1/REQ-3). A
/// pure function of `f` + the frozen table ⇒ the same ordered list every run
/// (REQ-8); `_seed` is taken for the documented determinism seam (the
/// enumeration is seed-stable; selection is order-prefix, not random).
///
/// `adt_deps` carries the program's ADT items (the same `&[Item]` every
/// production caller already threads into `check::item_subprogram`) so the
/// F-STRUCT-zero family (REQ-10/REQ-11) can resolve a `Type::Named` struct
/// return's field list — the early-return zero ladder needs the `StructItem`
/// definitions. A def-free `fn` (no struct return) passes `&[]` and the family
/// is inert. The Lean-path caller threads its full program's items here too.
pub fn generate(f: &FnItem, _seed: u64, adt_deps: &[Item]) -> Vec<Mutant> {
    let mut mutants = Vec::new();

    // A boundary fn (`.design/boundary/ffi-boundary.md` REQ-2) has `body: None` —
    // its body is foreign, so there is nothing to mutate (mutation scores a
    // known-good Thermite body, §7's premise). It does not reach here in
    // production (`check.rs` routes a boundary fn to L1 before any L3 proof +
    // mutation stage); handle `None` as an empty mutant set rather than panic
    // (R-CODE-2). The `real_body` below is the in-language body the families walk.
    let Some(real_body) = &f.body else {
        return mutants;
    };

    // Family 1: early return at body head. Every real `fn` body gets this mutant
    // (the §7 discriminator mutant) so the floor is not skipped via a 0/0 score
    // (#48). Listed first so the cap does not crowd it out.
    // The returned value is the return type's canonical zero (`zero_value_for`)
    // or, for a reference/slice return that has no scalar zero, a synthesized
    // valid early return — the empty-slice literal `&[]`, the empty `Vec`/`String`
    // wrapper, or (REQ-10) the named-struct field-zero literal resolved against
    // `adt_deps`.
    if let Some((value, desc)) = early_return_value(f, adt_deps) {
        let mut body = real_body.clone();
        body.stmts.insert(0, Stmt::Return(Some(value)));
        mutants.push(mutant_with_body(
            f,
            body,
            format!("insert early `return {desc}` at body head"),
        ));
    }

    // Family 1 (cont.) — F-IDENT identity returns (REQ-9): for each parameter
    // whose type exactly equals the return type (the AST `Type`'s derived
    // structural `PartialEq`, no ref-stripping — OQ-7), synthesize one mutant
    // inserting `return <param>` at the body head, one per matching param in
    // declaration order, each labeled with the param name so multi-param matches
    // stay distinguishable (OQ-8). Emitted after the zero-value early
    // return and before families 2-4, so the `MUTANT_CAP` order-prefix does not
    // crowd out the discriminator mutants. A strong contract refutes the identity
    // (`to_1based`'s `ens result == x + 1` rejects `return x`); a weak contract
    // proves it (the survivor the §7 floor names — the `move_up` `return b` hole).
    for p in &f.params {
        if p.ty == f.ret {
            let mut body = real_body.clone();
            body.stmts
                .insert(0, Stmt::Return(Some(Expr::Path(vec![p.name.clone()]))));
            mutants.push(mutant_with_body(
                f,
                body,
                format!(
                    "insert early `return {0}` at body head (identity of param `{0}`)",
                    p.name
                ),
            ));
        }
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
    // had a body); the field is `Option<Block>` since #16, so wrap in `Some`.
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
        // #92 integer operators: sound, value-distinguishable op-swaps so every
        // new operator yields a kill-able mutant (the §7 battery exercises the new
        // ops). `%`↔`/` (a remainder vs a quotient differ
        // wherever the divisor doesn't divide evenly), `<<`↔`>>` (shift direction),
        // `&`↔`|` and `^`↔`&` (distinct bit results) — each flips to an operator
        // of the same arity/operand types so the mutant always type-checks.
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
/// OQ-3 widened by #48). Every real `fn` body gets an early-return mutant so
/// the §7 floor is not skipped via a 0/0 score:
///
/// - a scalar return uses its canonical zero (`zero_value_for`): `0` for an
///   integer prim, `false` for `bool`, `None` for an `Option`;
/// - a reference-to-slice return (`&[T]` / `&mut [T]`) has no scalar zero, so it
///   synthesizes the empty-slice literal early return `&[]` (`&mut []`). An empty
///   slice is the canonical "trivial" slice (it borrows nothing, so its lifetime
///   is always valid and it lowers to exec code Verus accepts — `RangeTo`
///   subslices like `&xs[..0]` are not supported in Verus exec position, so the
///   empty literal is the right synthesis). A weak `ens` that does not pin the
///   result (`ens result.len() <= N`) proves `&[]` → the mutant survives → the
///   floor gates the weak contract; a strong `ens result == xs` rejects `&[]`
///   (unless `xs` is empty) → the mutant is killed → no over-gating (#48).
/// - a bounded-`Vec` return (`Vec<T>`, `.design/basis/04-collections.md` REQ-5)
///   has no scalar zero either, so it synthesizes the empty-Vec construction
///   `TVec<Suffix> { data: Vec::new() }` — the `thermite_lower`
///   wrapper-newtype literal a `Vec<T>` lowers to (`tvec_name` in `lower.rs`),
///   constructed empty. This mirrors the #48 slice precedent (`&[]` for `&[T]`)
///   for the `Vec`-return class: an empty `Vec` is the canonical "trivial" Vec
///   (`len() == 0`, always `well_formed`), so every `Vec`-returning body is
///   scored rather than escaping via a 0/0 gate (#74). A strong `ens
///   result.len() == v.len() + 1` rejects the empty Vec (`0 != v.len()+1`) → the
///   mutant is killed → a proved `push_one` scores the floor and
///   certifies L3 (it does not bypass the gate). A weak `ens result.len() <= N`
///   proves the empty Vec → the mutant survives → the floor still gates the weak
///   `Vec` contract (the synthesis enables scoring; it does not auto-pass).
/// - a bounded-`String` return (`Type::String`, `.design/basis/07-strings.md`
///   REQ-4) has no scalar zero either, so it synthesizes the empty-`TString`
///   construction `TString { data: Vec::new() }` (`empty_string_value`) — the
///   `thermite_lower` wrapper-newtype literal a `String` lowers to
///   (`Type::String => "TString"` in `lower.rs`), constructed empty. This mirrors
///   the #74 `Vec` precedent for the `String`-return class (#80): an empty
///   `TString` is the canonical "trivial" String (`len() == 0`, always
///   `well_formed`), so every `String`-returning body is scored rather than
///   escaping via a 0/0 gate. A strong `ens result.len() == a.len() + b.len()`
///   (the corpus `join`) rejects the empty String (`0 != a.len()+b.len()` for
///   non-empty inputs) so the mutant is killed — a proved `concat`
///   scores the floor and certifies L3 (it does not bypass the gate). A weak `ens
///   result.len() <= N` proves the empty String so the mutant survives — the floor
///   still gates the weak `String` contract.
///
/// `None` is returned only for an un-synthesizable return type (`Unit`, a
/// non-slice ref, a non-`Option` generic, a `Vec` of a non-Copy-primitive element
/// that the wrapper does not support) — see the 0/0 backstop in `kill_ratio`.
fn early_return_value(f: &FnItem, adt_deps: &[Item]) -> Option<(Expr, String)> {
    if let Some(zero) = zero_value_for(&f.ret) {
        return Some((zero, zero_desc(&f.ret).to_string()));
    }
    // A named-struct return (`Type::Named`): synthesize the field-zero struct
    // literal (REQ-10) resolved against `adt_deps`. Returns `None` (no mutant) if
    // the name is an enum, an unknown type, or any field lacks a synthesizable
    // zero (the OQ-5 drop, mirroring the `Type::Tuple` rule).
    if let Type::Named(name) = &f.ret {
        if let Some((value, desc)) = struct_zero_value(name, adt_deps) {
            return Some((value, desc));
        }
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
    // for the `Type::String` class). A `String` has no scalar zero, so
    // without this arm a `String`-returning body whose surface body has no
    // binop/off-by-one/branch site (`{ a.concat(b) }`) yields zero mutants → a
    // `0/0` score → the #48 anti-Goodhart backstop gates an
    // L3-proved fn to `WeakContract`/L0. An empty `TString` is the
    // canonical "trivial" String (`len() == 0`, always `well_formed`), the
    // `thermite_lower::lower` wrapper-newtype literal a `String` lowers to
    // (`TString { data }` over `vstd::vec::Vec<u8>` — the single nullary `TString`
    // wrapper, no per-element suffix unlike `Vec`). A strong `ens result.len() ==
    // a.len() + b.len()` rejects the empty String (`0 != a.len()+b.len()` for
    // non-empty inputs) → the mutant is killed → `join` scores the floor and
    // certifies L3 (the synthesis enables scoring; it does not bypass the gate). A
    // weak `ens result.len() <= N` proves the empty String → the mutant survives →
    // the floor still gates the weak String contract.
    if let Type::String = &f.ret {
        return Some(empty_string_value());
    }
    None
}

/// The empty-`String` early-return value: the wrapper-newtype struct literal
/// `TString { data: Vec::new() }` (#80). The wrapper name mirrors
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
/// struct literal `TVec<Suffix> { data: Vec::new() }` (#74). The wrapper name
/// mirrors `thermite_lower::lower`'s `tvec_name` — a `Vec<u64>` lowers to the
/// `TVecU64` newtype over `vstd::vec::Vec<u64>`, so the early-return mutant
/// constructs that newtype empty (an empty `vstd::vec::Vec` has `len() == 0`, so the
/// constructed wrapper is `well_formed` and lowers to exec code Verus accepts). The
/// `data` field is the verified vstd `Vec::new()`. Returns `None` for a `Vec`
/// element type the wrapper does not materialize (a non-Copy-primitive element —
/// `lower.rs::tvec_name` itself rejects these via `LowerError::Unsupported`), so
/// the mutant is not synthesized (dropped from the denominator, OQ-5), not
/// an over-gate.
fn empty_vec_value(elem: &Type) -> Option<(Expr, String)> {
    let suffix = match elem {
        Type::Prim(PrimType::U8) => "U8",
        Type::Prim(PrimType::U16) => "U16",
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

/// The canonical zero value of a scalar return type for the early-return mutant
/// (REQ-1, OQ-3): `0` for an integer prim, `false` for `bool`, `None` for an
/// `Option`. A type with no scalar zero (`Unit`, a `Ref`, a bare `Slice`, a
/// non-`Option` generic) yields `None` here; reference-to-slice returns are
/// handled by `early_return_value`. Returning a value of the function's return
/// type keeps the mutant well-typed so it lowers (the contract should reject
/// it, not the type checker).
fn zero_value_for(ret: &Type) -> Option<Expr> {
    match ret {
        Type::Prim(
            PrimType::U8 | PrimType::U16 | PrimType::U32 | PrimType::U64 | PrimType::Usize,
        ) => Some(Expr::IntLit {
            value: 0,
            raw: "0".to_string(),
        }),
        Type::Prim(PrimType::Bool) => Some(Expr::BoolLit(false)),
        // Cluster C7 (`.design/basis/09-option-result.md` REQ-1): `Option<T>` is now
        // the dedicated `Type::Option` node (not a string-named `Generic`), so the
        // early-return zero value of an `Option`-returning fn is `None` keyed on the
        // node kind — the OQ-1 ripple at this `Generic { name: "Option" }` reader.
        // (A `Result`-returning fn has no canonical scalar zero — its `Err(e)` needs
        // a typed reason — so it falls through to `None` here, like a bare `Slice`.)
        Type::Option(_) => Some(Expr::Path(vec!["None".to_string()])),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): the
        // early-return zero value of a tuple-returning fn is the tuple of its
        // elements' zero values (`(u64, u64)` → `(0, 0)`) — the #48/#74/#80
        // early-return-synthesis pattern extended to the tuple-return class. Without
        // this, a tuple-returning fn whose body has no binop/off-by-one/branch site
        // (the grounded `swap` body `(b, a)`) yields zero mutants → a `0/0` score →
        // the anti-Goodhart backstop gates an L3-proved fn to
        // `WeakContract`/L0 (AC-4 requires swap → L3). A strong `ens result.0 == b
        // && result.1 == a` rejects `(0, 0)` (for nonzero b/a) → the mutant is
        // killed → swap scores the floor and certifies L3 (the synthesis enables
        // scoring; it does not bypass the gate — a weak tuple `ens` proves `(0, 0)`
        // → the mutant survives → the floor still gates it). Returns `None` if any
        // element lacks a scalar zero (a `Ref`/`Result` element), so the mutant is
        // not synthesized (dropped from the denominator, OQ-5), not an
        // over-gate. The element zeros recurse, so a nested tuple composes.
        Type::Tuple(tys) => {
            let mut elems = Vec::with_capacity(tys.len());
            for t in tys {
                elems.push(zero_value_for(t)?);
            }
            Some(Expr::Tuple(elems))
        }
        _ => None,
    }
}

/// The F-STRUCT-zero early-return value for a `Type::Named(name)` struct return
/// (REQ-10/REQ-11): the field-zero struct literal `name { field: <zero>, … }`,
/// resolved against the threaded `adt_deps`. Each field's zero comes from the
/// same synthesis ladder the early-return family owns (`zero_value_with_defs`:
/// the scalar `zero_value_for` arms, the #74/#80 empty `Vec`/`String` wrappers,
/// the C9-B tuple recursion, and — recursively — a nested named struct's own
/// field zeros).
///
/// Returns `None` (no mutant — the OQ-5 drop, mirroring the `Type::Tuple` rule)
/// when:
///   - `name` resolves to no struct in `adt_deps` (an enum-named return: no
///     canonical variant to choose; an unknown name), or
///   - any field lacks a synthesizable zero (a `Box`/`Ref`/`Result`/enum-typed
///     field — the recursion terminates because a struct can only reference
///     another struct through `Box`, which has no zero, so a self-referential
///     struct field drops here and the recursion does not cycle, REQ-11).
///
/// Type-invariant interaction (REQ-10): a struct `inv` is contract — if the
/// field-zero literal violates it, Verus fails the construction obligation and
/// the mutant is killed (the polarity). For the corpus structs the zeros
/// satisfy the `inv` (`Account { balance: 0 }`: `0 <= 1_000_000`;
/// `Buffer { text: <empty>, cursor: 0 }`: `0 <= 0 && 0 <= 1_000_000`), so the
/// mutant is scored against the `ens`.
fn struct_zero_value(name: &str, adt_deps: &[Item]) -> Option<(Expr, String)> {
    let def = find_struct(name, adt_deps)?;
    let mut fields = Vec::with_capacity(def.fields.len());
    for field in &def.fields {
        // Any field without a synthesizable zero ⇒ no mutant for this struct
        // (the OQ-5 drop — not an over-gate).
        let zero = zero_value_with_defs(&field.ty, adt_deps)?;
        fields.push((field.name.clone(), zero));
    }
    let lit = Expr::StructLit {
        path: vec![name.to_string()],
        fields,
    };
    Some((lit, format!("{name} {{ <field zeros> }}")))
}

/// Resolve a `struct name` definition among the threaded ADT items (REQ-11). An
/// `Item::Enum` of the same name resolves to `None` here (F-STRUCT-zero is a
/// struct-only family — an enum has no canonical variant, the OQ-5 drop).
fn find_struct<'a>(name: &str, adt_deps: &'a [Item]) -> Option<&'a StructItem> {
    adt_deps.iter().find_map(|i| match i {
        Item::Struct(s) if s.name == name => Some(s),
        _ => None,
    })
}

/// The zero value of a field/return type with access to the program's struct
/// defs (REQ-11): the defs-threaded sibling of [`zero_value_for`]. It defers to
/// the def-free `zero_value_for` for every shipped arm (scalars, `Option`,
/// tuples — no behavior change), adds the #74/#80 empty `Vec`/`String` wrappers
/// (a struct field can be a `Vec`/`String`, e.g. `Buffer.text`), and — for a
/// `Type::Named` field — recurses through `struct_zero_value` so a nested struct
/// composes. A `Type::Tuple` recurses element-wise through this defs-threaded
/// form so a tuple of structs composes too. Returns `None` (the OQ-5 drop) for
/// any un-synthesizable field type (`Box`/`Ref`/`Result`/`Map`/enum).
fn zero_value_with_defs(ty: &Type, adt_deps: &[Item]) -> Option<Expr> {
    if let Some(zero) = zero_value_for(ty) {
        return Some(zero);
    }
    match ty {
        // The #74/#80 empty-wrapper literals — a struct field can own a bounded
        // `Vec`/`String` (e.g. the corpus `Buffer.text: String`).
        Type::Vec(elem) => empty_vec_value(elem).map(|(v, _)| v),
        Type::String => Some(empty_string_value().0),
        // A nested named-struct field recurses (REQ-11). The recursion terminates
        // (a struct references another struct only through `Box`, which has no
        // zero) — no explicit cycle check needed.
        Type::Named(name) => struct_zero_value(name, adt_deps).map(|(v, _)| v),
        // A tuple field recurses element-wise through the defs-threaded form so a
        // tuple containing a struct / Vec / String field composes (the def-free
        // `zero_value_for`'s `Type::Tuple` arm cannot reach struct fields).
        Type::Tuple(tys) => {
            let mut elems = Vec::with_capacity(tys.len());
            for t in tys {
                elems.push(zero_value_with_defs(t, adt_deps)?);
            }
            Some(Expr::Tuple(elems))
        }
        _ => None,
    }
}

/// The human description of the early-return zero value (matches `zero_value_for`).
fn zero_desc(ret: &Type) -> &'static str {
    match ret {
        Type::Prim(
            PrimType::U8 | PrimType::U16 | PrimType::U32 | PrimType::U64 | PrimType::Usize,
        ) => "0",
        Type::Prim(PrimType::Bool) => "false",
        // Cluster C7 (`.design/basis/09-option-result.md` REQ-1): the description of
        // the `Option`-returning early-return zero, keyed on `Type::Option` (the
        // OQ-1 ripple — `Option` is no longer a string-named `Generic`).
        Type::Option(_) => "None",
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): a
        // static label for the synthesized zero-tuple early-return mutant (the
        // per-element zeros are in `zero_value_for`; this is only the human desc).
        Type::Tuple(_) => "(0, ..)",
        _ => "<none>",
    }
}

/// Negate an `if` condition for a branch-swap mutant (REQ-1). When the condition
/// is a flippable comparison (`<`↔`>=`, `<=`↔`>`, `==`↔`!=`), negation is the
/// complementary flip (`!(a < b)` ≡ `a >= b`), encoded in the operator set so the
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
/// records the action needed to rebuild a mutated copy of the whole body with
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
                // The loop's `inv`/`dec` are contract (the mutator never touches
                // them — out of scope in the design); only the loop body is mutated.
                self.scan_block(&l.body, ctr);
            }
            Stmt::Expr(e) => self.scan_expr(e, ctr),
            // break/continue carry no sub-expression and are not a mutation
            // target in v0.1 (#93, verus-lowering.md OQ-4): the scan produces no
            // mutant for them (a leaf, like `Return(None)`).
            Stmt::Break | Stmt::Continue => {}
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
                    // A C10 match guard is a mutable sub-expression too
                    // (`.design/basis/11-ergonomics.md` REQ-3).
                    if let Some(guard) = &arm.guard {
                        self.scan_expr(guard, ctr);
                    }
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
            // define no new mutation site themselves (no off-by-one literal,
            // binop, or branch), but the scan descends into their
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
            // The prefix `!` (#92): it defines no new mutation site of its own (no
            // off-by-one, binop, or branch), but the scan descends into the
            // operand so a mutable site nested under `!` is still found.
            Expr::Unary { expr, .. } => self.scan_expr(expr, ctr),
            // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): a
            // tuple construction / projection defines no new mutation site of its
            // own (the projection index is not a v1 mutant — REQ-8 leaf walk), but
            // a mutable site (a binop, an off-by-one literal) can sit in a tuple
            // element / under a projection's receiver, so the scan descends.
            Expr::Tuple(elems) => {
                for e in elems {
                    self.scan_expr(e, ctr);
                }
            }
            Expr::TupleProj { receiver, .. } => self.scan_expr(receiver, ctr),
            // A raw quantified formula (`.design/stage2-stratified-cage.md` REQ-0): a
            // mutation site can live in the domain or the body — descend into both.
            Expr::Quantifier { domain, body, .. } => {
                self.scan_expr(domain, ctr);
                self.scan_expr(body, ctr);
            }
            // A string literal (`.design/basis/07-strings.md` REQ-1) is a leaf and
            // is not an off-by-one target (it is text, not a numeric literal) — it
            // defines no new mutation site and has no sub-expression to descend
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
            // break/continue have no sub-expression to rewrite (#93): copied
            // verbatim (not a mutation target — OQ-4).
            Stmt::Break => Stmt::Break,
            Stmt::Continue => Stmt::Continue,
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
                        // A C10 match guard is a mutable sub-expression
                        // (`.design/basis/11-ergonomics.md` REQ-3) — apply the
                        // mutation through it so a guard mutant is scoreable.
                        guard: arm.guard.as_ref().map(|g| self.apply_expr(g)),
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
            // rewriter rebuilds the ADT node faithfully, recursing into its
            // sub-expressions so a mutation site nested inside is applied. This
            // is an identity-preserving rebuild rather than a
            // panic. Dead-in-1a (mutation runs only post-L3-proof; an ADT
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
            // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109):
            // rebuild the tuple / projection faithfully, recursing so a mutation
            // site nested in an element / under the receiver is applied; the
            // projection index is identity-preserved (not a v1 mutant).
            Expr::Tuple(elems) => Expr::Tuple(elems.iter().map(|e| self.apply_expr(e)).collect()),
            Expr::TupleProj { receiver, index } => Expr::TupleProj {
                receiver: Box::new(self.apply_expr(receiver)),
                index: *index,
            },
            // A raw quantified formula (`.design/stage2-stratified-cage.md` REQ-0):
            // rebuild the binder faithfully, recursing into the domain and body so a
            // mutation site in either is applied; the binder head (`quant`/`var`/
            // `sort`) is identity-preserved.
            Expr::Quantifier {
                quant,
                var,
                sort,
                domain,
                body,
            } => Expr::Quantifier {
                quant: *quant,
                var: var.clone(),
                sort: sort.clone(),
                domain: Box::new(self.apply_expr(domain)),
                body: Box::new(self.apply_expr(body)),
            },
            Expr::BoolLit(b) => Expr::BoolLit(*b),
            Expr::Path(p) => Expr::Path(p.clone()),
            // A string literal (`.design/basis/07-strings.md` REQ-1) is a leaf with
            // no mutation site (text, not an off-by-one target) — the rewriter
            // rebuilds it by identity, as for `BoolLit`/`Path`.
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

    /// Parse a program and return all its items so the F-STRUCT-zero family
    /// (REQ-10/REQ-11) can be exercised with the struct defs threaded as
    /// `adt_deps` (the same items a production caller weaves). Pair with `parse_fn`
    /// (or a name filter) to pull the fn under test.
    fn parse_items(src: &str) -> Vec<Item> {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        parsed.program.items
    }

    /// The fn named `fn_name` among `items` (clones it). Asserts presence.
    fn fn_named(items: &[Item], fn_name: &str) -> FnItem {
        let found = items.iter().find_map(|i| match i {
            Item::Fn(f) if f.name == fn_name => Some(f.clone()),
            _ => None,
        });
        assert!(found.is_some(), "fixture has the named fn `{fn_name}`");
        found.unwrap_or_else(unreachable_fn)
    }

    /// A never-reached fallback (the `assert!` above fires first) — keeps
    /// `fn_named` gate-clean (no `.unwrap()`/`.expect()`/`panic!` on the added
    /// patch lines). Builds a trivially-default `FnItem` via re-parse of a stub.
    fn unreachable_fn() -> FnItem {
        parse_fn("fn _u() -> u32 req true ens true fx pure { 0 }")
    }

    // AC-5 (re-derived for #269, REQ-12): the frozen set + deterministic order for
    // a small fn. Expected mutants trace to REQ-1/REQ-9's table (R-CHAR-3), not to
    // the generator's own output. The fn
    // `fn f(x: u32) -> u32 req x < 10 ens result == x fx pure { x + 1 }` has, in
    // family order:
    //   - family 1a: one zero early return (ret u32 -> `return 0`),
    //   - family 1b (F-IDENT, REQ-9): `x: u32` matches the `u32` return ->
    //     `return x` (identity of param `x`) — emitted after the zero return,
    //     before families 2-4,
    //   - family 2: one Binary `+` (Add->Sub flip),
    //   - family 3: one IntLit `1` (1->2, 1->0).
    #[test]
    fn frozen_set_and_order_for_small_fn() {
        let f = parse_fn("fn f(x: u32) -> u32 req x < 10 ens result == x fx pure { x + 1 }");
        let mutants = generate(&f, 0, &[]);
        let descs: Vec<&str> = mutants.iter().map(|m| m.desc.as_str()).collect();
        assert_eq!(
            descs,
            vec![
                "insert early `return 0` at body head",
                "insert early `return x` at body head (identity of param `x`)",
                "flip binary operator +->-",
                "off-by-one literal 1->2",
                "off-by-one literal 1->0",
            ],
            "frozen mutator set in the documented family order (zero-return, then \
             F-IDENT identity-returns in param order, then families 2-4)"
        );
    }

    // REQ-9 (F-IDENT): a parameter whose type exactly equals the return type yields
    // one identity-return mutant, labeled with the param name. A by-value struct
    // return matches a by-value struct param (the `move_up`-class `b: Buffer ->
    // Buffer`). Expected trace: REQ-9's table (R-CHAR-3).
    #[test]
    fn ident_return_for_exact_type_match() {
        let items = parse_items(
            "struct S { a: u64 } \
             fn id(s: S) -> S req true ens result.a <= 1_000_000 fx pure { s }",
        );
        let f = fn_named(&items, "id");
        let descs: Vec<String> = generate(&f, 0, &items)
            .into_iter()
            .map(|m| m.desc)
            .collect();
        assert!(
            descs
                .iter()
                .any(|d| d == "insert early `return s` at body head (identity of param `s`)"),
            "an exact-type-match param yields an identity-return mutant: {descs:?}"
        );
    }

    // REQ-9 (OQ-8): two params of the same matching type yield two identity mutants
    // in declaration order (no dedup) — `min2`'s `return a` and `return b`.
    #[test]
    fn ident_return_one_per_matching_param_in_order() {
        let f = parse_fn(
            "fn min2(a: u64, b: u64) -> u64 req true ens result <= a ens result <= b fx pure \
             { if a <= b { a } else { b } }",
        );
        let idents: Vec<String> = generate(&f, 0, &[])
            .into_iter()
            .map(|m| m.desc)
            .filter(|d| d.contains("identity of param"))
            .collect();
        assert_eq!(
            idents,
            vec![
                "insert early `return a` at body head (identity of param `a`)".to_string(),
                "insert early `return b` at body head (identity of param `b`)".to_string(),
            ],
            "two matching params -> two identity mutants in declaration order (OQ-8)"
        );
    }

    // REQ-9 (OQ-7): no ref-stripping — a `b: &Buf` param with a `Buf` return gets
    // no identity mutant (exact `Type` equality only).
    #[test]
    fn ident_return_no_ref_stripping() {
        let items = parse_items(
            "struct Buf { n: u64 } \
             fn deref_id(b: &Buf) -> Buf req true ens result.n <= 1_000_000 fx pure { *b }",
        );
        let f = fn_named(&items, "deref_id");
        let descs: Vec<String> = generate(&f, 0, &items)
            .into_iter()
            .map(|m| m.desc)
            .collect();
        assert!(
            !descs.iter().any(|d| d.contains("identity of param")),
            "a `&Buf` param against a `Buf` return is NOT an exact match -> no \
             identity mutant in v1 (OQ-7): {descs:?}"
        );
    }

    // REQ-9: an exact ref-type match does yield an identity mutant (the divergence
    // fixture `pick(xs: &[u32]) -> &[u32]` — `return xs` borrows nothing new).
    #[test]
    fn ident_return_exact_ref_match() {
        let f = parse_fn(
            "fn pick(xs: &[u32]) -> &[u32] req xs.len() <= 10 ens result.len() <= 10 fx pure { xs }",
        );
        let descs: Vec<String> = generate(&f, 0, &[]).into_iter().map(|m| m.desc).collect();
        assert!(
            descs
                .iter()
                .any(|d| d == "insert early `return xs` at body head (identity of param `xs`)"),
            "an exact `&[u32]` ref match yields an identity mutant: {descs:?}"
        );
    }

    // REQ-10 (F-STRUCT-zero): a named-struct return synthesizes the field-zero
    // struct literal early-return mutant, resolved against the threaded defs. The
    // corpus `Account { balance: u64 }` -> `Account { <field zeros> }`. AC-8 also
    // checks the F-IDENT `return a`. Expected trace: REQ-10's table (R-CHAR-3).
    #[test]
    fn struct_zero_return_for_named_struct() {
        let items = parse_items(
            "struct Account { balance: u64 } \
             fn deposit(a: Account, amount: u64) -> Account \
               req a.balance + amount <= 1_000_000 \
               ens result.balance == a.balance + amount fx pure \
             { Account { balance: a.balance + amount } }",
        );
        let f = fn_named(&items, "deposit");
        let descs: Vec<String> = generate(&f, 0, &items)
            .into_iter()
            .map(|m| m.desc)
            .collect();
        assert!(
            descs
                .iter()
                .any(|d| d == "insert early `return Account { <field zeros> }` at body head"),
            "a named-struct return synthesizes the field-zero struct literal: {descs:?}"
        );
        assert!(
            descs
                .iter()
                .any(|d| d == "insert early `return a` at body head (identity of param `a`)"),
            "deposit's Account param yields the identity mutant too: {descs:?}"
        );
    }

    // REQ-10 / AC-10 (the OQ-5 drop): a struct return with a zero-less field (a
    // `Box`-typed field) generates no F-STRUCT-zero mutant — never an error.
    #[test]
    fn struct_zero_drops_when_a_field_has_no_zero() {
        let items = parse_items(
            "struct Node { next: Box<Node> } \
             fn wrap(n: Box<Node>) -> Node req true ens true fx alloc { Node { next: n } }",
        );
        let f = fn_named(&items, "wrap");
        let descs: Vec<String> = generate(&f, 0, &items)
            .into_iter()
            .map(|m| m.desc)
            .collect();
        assert!(
            !descs.iter().any(|d| d.contains("<field zeros>")),
            "a struct with a Box-typed (zero-less) field drops the F-STRUCT-ZERO \
             mutant (OQ-5), never an error: {descs:?}"
        );
    }

    // REQ-10: an enum-named return gets no F-STRUCT-zero mutant (no canonical
    // variant — the OQ-5 drop), but the F-IDENT identity is still generated.
    #[test]
    fn struct_zero_drops_for_enum_named_return() {
        let items = parse_items(
            "enum Color { Red, Green } \
             fn pick_color(c: Color) -> Color req true ens true fx pure { c }",
        );
        let f = fn_named(&items, "pick_color");
        let descs: Vec<String> = generate(&f, 0, &items)
            .into_iter()
            .map(|m| m.desc)
            .collect();
        assert!(
            !descs.iter().any(|d| d.contains("<field zeros>")),
            "an enum-named return gets no struct-zero mutant (OQ-5): {descs:?}"
        );
        assert!(
            descs
                .iter()
                .any(|d| d == "insert early `return c` at body head (identity of param `c`)"),
            "the enum param's identity mutant is still generated: {descs:?}"
        );
    }

    // REQ-11: a struct field that is itself a String/Vec zeros via the #74/#80
    // empty-wrapper ladder (the corpus `Buffer { text: String, cursor: u64 }`).
    #[test]
    fn struct_zero_composes_string_and_scalar_fields() {
        let items = parse_items(
            "struct Buffer { text: String, cursor: u64 } \
             fn mk(t: String) -> Buffer req t.len() <= 1_000_000 \
               ens result.cursor <= result.text.len() fx alloc \
             { Buffer { text: t, cursor: 0 } }",
        );
        let f = fn_named(&items, "mk");
        let descs: Vec<String> = generate(&f, 0, &items)
            .into_iter()
            .map(|m| m.desc)
            .collect();
        assert!(
            descs
                .iter()
                .any(|d| d == "insert early `return Buffer { <field zeros> }` at body head"),
            "a Buffer struct (String + u64 fields) synthesizes its field-zero \
             literal via the #74/#80 ladder: {descs:?}"
        );
    }

    // REQ-1 (OQ-3): an `Option` return type's early-return mutant is `return None`.
    #[test]
    fn option_return_early_return_is_none() {
        let f = parse_fn("fn g(x: u32) -> Option<usize> req x < 10 ens true fx pure { Some(0) }");
        let mutants = generate(&f, 0, &[]);
        assert!(
            mutants
                .iter()
                .any(|m| m.desc == "insert early `return None` at body head"),
            "Option return -> early `return None`: {:?}",
            mutants.iter().map(|m| &m.desc).collect::<Vec<_>>()
        );
    }

    // REQ-1: the off-by-one `n-1` mutant is skipped at n == 0 (u128 cannot
    // represent -1) — documented, not a silent wrap.
    #[test]
    fn off_by_one_skips_minus_one_at_zero() {
        let f = parse_fn("fn h(x: u32) -> u32 req x < 10 ens result >= 0 fx pure { 0 }");
        let mutants = generate(&f, 0, &[]);
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
        let a: Vec<String> = generate(&f, 0, &[]).into_iter().map(|m| m.desc).collect();
        let b: Vec<String> = generate(&f, 0, &[]).into_iter().map(|m| m.desc).collect();
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
        assert!(generate(&f, 0, &[]).len() <= MUTANT_CAP);
    }

    // REQ-1/REQ-3: a mutant's contract is byte-identical to the original; only the
    // body differs.
    #[test]
    fn mutant_keeps_contract_changes_only_body() {
        let f = parse_fn("fn f(x: u32) -> u32 req x < 10 ens result == x fx pure { x + 1 }");
        let mutants = generate(&f, 0, &[]);
        for m in &mutants {
            assert_eq!(m.item.contract, f.contract, "contract untouched");
            assert_eq!(m.item.name, f.name);
            assert_eq!(m.item.params, f.params);
            assert_eq!(m.item.ret, f.ret);
            assert_ne!(m.item.body, f.body, "body mutated");
        }
    }

    // REQ-4: the classification polarity — a verus success (proved the wrong body) is
    // a survivor; a verus failure is killed. Traces to the design's §7 polarity
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
            equivalent: 0,
            survivor: None,
        };
        assert_eq!(score.kill_ratio(), 1.0);
        assert!(score.meets_floor(MUTATION_FLOOR));
        assert_eq!(score.mutants_killed_string(), "3/3");

        let weak = MutationScore {
            killed: 1,
            scored: 3,
            equivalent: 0,
            survivor: Some("insert early `return 0` at body head".to_string()),
        };
        assert!((weak.kill_ratio() - 0.3333).abs() < 0.01);
        assert!(!weak.meets_floor(MUTATION_FLOOR), "1/3 is below 0.60");
        assert!(weak.meets_floor(0.2), "1/3 is above the lowered 0.2 floor");
        assert_eq!(weak.mutants_killed_string(), "1/3");
    }

    // REQ-5 / #48 backstop: a 0/0 score (no scoreable mutant) does not meet the
    // floor — a contract that cannot be mutation-validated has not met the §7 bar
    // (anti-Goodhart, goal.md R-DEFER-9), so it is gated, not a vacuous
    // pass. Expected value traces to §7 step 4 (the floor catches an
    // under-constraining contract), not to forge's output (R-CHAR-3).
    #[test]
    fn empty_score_is_below_floor() {
        let score = MutationScore {
            killed: 0,
            scored: 0,
            equivalent: 0,
            survivor: None,
        };
        assert_eq!(score.kill_ratio(), 0.0);
        assert!(!score.meets_floor(MUTATION_FLOOR));
    }

    // #48: a reference-to-slice return synthesizes an early-return mutant (the
    // empty-slice literal `&[]`) so every real `fn` body is scored — the 0/0 escape
    // is unreachable. The weak `pick` fixture from the divergence issue:
    // `fn pick(xs: &[u32]) -> &[u32] ... { xs }`. Expected mutant traces to REQ-1's
    // widened early-return family (R-CHAR-3), not to the generator's output.
    #[test]
    fn slice_return_synthesizes_early_return_mutant() {
        let f = parse_fn(
            "fn pick(xs: &[u32]) -> &[u32] req xs.len() <= 10 ens result.len() <= 10 fx pure { xs }",
        );
        let mutants = generate(&f, 0, &[]);
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
        let mutants = generate(&f, 0, &[]);
        assert!(
            mutants
                .iter()
                .any(|m| m.desc.contains("negate `if` condition")),
            "a comparison `if` records a negate-condition mutant: {:?}",
            mutants.iter().map(|m| &m.desc).collect::<Vec<_>>()
        );
    }

    // =======================================================================
    // REQ-11 (Target E) — the Verus-anchor for the mutation floor gate (#48 anti-
    // Goodhart, `.design/verified/self-verification.md` REQ-11 / AC-11c, mechanism
    // (c)).
    //
    // Placement deviation (Option B, orchestrator-authorized): the design doc names
    // a `mutation::verus_anchor` block (forge is binary-only). Nested in the
    // existing `tests` module so the anti-pattern gate's `#[cfg(test)]` exemption
    // covers it. `thermite-verified` is a forge dev-dependency.
    //
    // AC-11c — the f64↔integer grid: over `killed ∈ 0..=20`, `scored ∈ 0..=20`,
    // assert the production f64 `MutationScore { killed, scored, survivor: None }
    // .meets_floor(0.60)` equals the verus-proved integer
    // `thermite_verified::meets_floor_60(killed, scored)` for every grid point.
    // Expected = the proved integer spec (R-CHAR-3, never forge's own f64 output).
    // The verus proof is over the integer property `scored == 0 ⟹ !pass` + the
    // cross-multiply; the f64↔integer agreement is this test's job (OQ-E).
    //
    // OQ-E (the f64 boundary subtlety): f64 `0.60` is not exactly 3/5, so a ratio
    // on the boundary (e.g. 12/20 == 0.60) could in principle diverge by a
    // rounding ULP between the f64 `>=` and the integer cross-multiply. The grid is
    // run here (not assumed); if any cell diverges it is reported, not masked
    // (R-DEFER-9). The empirical expectation (from the cross-multiply being the
    // exact rational test) is 0 divergences on 0..=20.
    // =======================================================================
    mod verus_anchor {
        use super::*;
        use thermite_verified::meets_floor_60;

        /// AC-11c — the f64↔integer grid over `0..=20 × 0..=20` at the default 0.60
        /// floor: the production f64 `meets_floor(0.60)` agrees with the
        /// verus-proved integer `meets_floor_60` at every grid point. In particular the
        /// `(0, 0)` point reads `false` on both sides (the #48 anti-Goodhart gate —
        /// a `0/0` score never passes). Any divergence is asserted-out (and would be
        /// reported, OQ-E), not masked.
        #[test]
        fn meets_floor_f64_matches_proved_integer_spec_over_grid() {
            let mut checked = 0usize;
            let mut divergences: Vec<(usize, usize, bool, bool)> = Vec::new();
            for killed in 0usize..=20 {
                for scored in 0usize..=20 {
                    let score = MutationScore {
                        killed,
                        scored,
                        equivalent: 0,
                        survivor: None,
                    };
                    // R-CHAR-3: the expected verdict is the verus-proved integer spec.
                    let expected = meets_floor_60(killed, scored);
                    let produced = score.meets_floor(MUTATION_FLOOR);
                    if produced != expected {
                        divergences.push((killed, scored, produced, expected));
                    }
                    checked += 1;
                }
            }
            // OQ-E: report any divergence explicitly (do not delete the cell).
            assert!(
                divergences.is_empty(),
                "f64↔integer floor-gate divergences (killed, scored, f64_pass, \
                 integer_pass) — OQ-E boundary divergence, report honestly: {divergences:?}"
            );
            assert_eq!(checked, 21 * 21, "the full 0..=20 × 0..=20 grid enumerated");
        }

        /// AC-11b/d (the #48 property made observable on both representations): a
        /// `0/0` score (no scoreable mutant) reads `false` on the production f64 gate
        /// and on the verus-proved integer spec — the anti-Goodhart gate holds in the
        /// production impl regardless of the f64 representation (the #48
        /// invariant). Expected = the proved `scored == 0 ⟹ !pass` (R-CHAR-3).
        #[test]
        fn zero_scored_never_passes_on_both_representations() {
            let empty = MutationScore {
                killed: 0,
                scored: 0,
                equivalent: 0,
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
