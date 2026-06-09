/-
  Thermite/Denote.lean — the SOURCE denotation `⟦·⟧_{S_C}` for the comparison +
  logical contract fragment (increment (a), #170) EXTENDED with the ARITHMETIC
  operators (#176), the CASTS (#177), the SPEC-CONTEXT REWRITES (#178 — slice→
  `@`/subrange, indexing, the `String` byte-view length/byte-at), and the 6
  BOUNDED-QUANTIFIER COMBINATORS (#179 / 1d-i — `forall_in`/`exists_in`/`sorted`/
  `forall_below`/`forall_from`/`disjoint`, each denoting its FROZEN `verus_l3`
  quantifier form from `thermite-spec/src/combinators.rs`, with the predicate closure
  `p(s[i])` applied at the i-th element via `Env.bindInt`), and the MATCH-IN-ENS / `is`
  PAYLOAD-IN-CONTRACT forms (#180 / 1g — `match scrut { Some(v) => P(v), None => Q }` /
  `Ok`/`Err`, and `scrut is Variant`: the arm SELECTED by the scrutinee's `OptResVal` variant
  denotes its body with the payload BOUND via `Env.bindInt`; the `is`-test reads the variant).

  THE OPTION/RESULT ENVIRONMENT (#180). A `match`/`is` scrutinee (a param / `result` of a
  built-in `Option`/`Result` type) denotes an `OptResVal` (`none`/`some v`/`ok v`/`err e`, the
  payload an `Int` — the C7 corpus shape). The `Env` gains an `optres` map. The SOURCE meaning:
  `match scrut arms` = the body of the arm whose pattern variant matches `scrut`'s variant,
  denoted with the payload bound (`denoteArms`); `scrut is V` = `scrut`'s variant is `V`.

  THE SEQUENCE ENVIRONMENT (#178). A slice param `xs: &[u32]` and a `String`'s bytes
  denote a SEQUENCE (`List Int`), not a scalar. The `Env` is therefore extended to map
  free SEQUENCE names to `List Int` alongside the integer names. The SOURCE meaning
  (BEFORE the rewrite): `xs[i]` = the i-th element; `&xs[..i]` = the prefix
  `subrange(0,i)`; `s.byte_at(i)` = the i-th byte; `s.len()` = the length.

  THE SEQUENCE-ACCESS PARTIALITY CONVENTION (#178). An index `xs[i]` carries a source
  obligation `0 ≤ i < |xs|` (Verus rejects an out-of-bounds spec index — an L0/source
  precondition, exactly like the div-by-zero of #176). We model the access with the
  TOTAL `List.getD … 0` (out-of-range → `0`). This is sound for `S_C` for the SAME
  reason as the arithmetic partial point: the in-range obligation is a SOURCE-side
  precondition, and — load-bearing for T1 — BOTH `denote`/`intVal` (source) and
  `RefEncode.refDenote`/`refIntVal` (encoder) route the access through the SAME total
  `seqIdx` over the SAME `List` (the `@`-view is the identity on the value), so the
  soundness equation holds regardless of the out-of-range value. The same holds for
  `subrange` (`List.take`/`List.drop`, total) and `seqLen` (`List.length`, total).

  Governing design: `.design/verified/thermite-semantics.md` Architecture §"S_C —
  the spec/contract sublanguage", REQ-1. This is `S_C` RESTRICTED to the fragment
  `Ast.lean` embeds: comparisons/logical denote the corresponding math relation
  (the STANDARD meaning), `Eq → =`, `Le → ≤`, `And → ∧`, `Not → ¬`; ARITHMETIC ops
  denote `int` arithmetic over the operand VALUES (`Add → +`, `Sub → -`, …; the
  doc's `⟦Binary(Add,a,b)⟧ = ⟦a⟧ + ⟦b⟧` rule, "arithmetic in subterms ... at the
  spec-context numeric domain"); CASTS denote the value coercion (the doc's
  `cast → nat/int` rule: "an integer cast denotes the value as an unbounded
  `nat`/`int` (no wrap — spec arithmetic is mathematical)").

  THE PARTIALITY CONVENTION (#176). `Div`/`Rem`/`Shl`/`Shr` are PARTIAL in the
  source (`ast.rs`: `BinOp::Rem` "requires a nonzero divisor"; the zero divisor /
  zero shift is rejected as a SOURCE PRECONDITION / L0 obligation, discharged
  OUTSIDE the clause). We model them with Lean's TOTAL `Int` operations
  (`Int./` is Euclidean-ish / T-division; `Int.%` its companion). This is sound
  for `S_C` because: (i) the divisor-≠0 precondition is a SOURCE-SIDE obligation,
  not part of the binop's contract MEANING; (ii) — load-bearing for T1 — `denote`
  and `refDenote` route the op through the SAME shared `arithDenote` function, so
  whatever total convention is chosen, BOTH sides agree (the soundness theorem is
  about the encoder's operator MAP being faithful, not about the partial-point
  value). The convention is stated here and held consistent across both denotations.

  THE CAST/`nat` CONVENTION (#177). `as nat` is the value injected into the
  naturals: a non-negative `int` maps to itself, a negative `int` maps to `0`
  (Lean `Int.toNat` clamps; in `S_C` an `as nat` always carries a `≥ 0` source
  frame, so the clamp point is never the intended value — the same shape as the
  zero-divisor convention). `as int`/`as u64`/`as u32`/`as usize` are
  value-preserving on the spec `int` domain (the bounded cast carries its
  no-overflow frame as a source obligation, exactly like div-by-zero), so at the
  CONTRACT level they denote the value unchanged. Again the convention is SHARED
  by `denote` and `refDenote` (`castDenote`), so T1 is insensitive to the clamp.

  THE NAMED SPEC-FN CALLS (#181, increment 1e — the WELL-FOUNDED RECURSIVE fragment). A
  `specCall name args` (`ast.rs` `Expr::Call` for a non-combinator/non-`old` callee — the
  `ref_encode.rs::encode_call` case (3)) denotes by RESOLVING the name in the SHARED `Registry`
  (`Env.specs`) to a `SpecFn { params, body }`, BINDING the params to the denoted args, and denoting
  the BODY. The body is an `Expr` of the SAME fragment and MAY contain further `specCall`s
  (recursion). §4.2 mandates a `dec` termination measure on every spec fn ⟹ the recursion terminates
  ⟹ the denotation is a well-founded fixpoint (the design `⟦Call(f,args)⟧ = ⟦body_of(f)⟧[params ↦
  ⟦args⟧]`, "well-defined because §4.2 mandates a `dec` measure").

  THE WELL-FOUNDED DENOTATION (the genuine difficulty). Rather than re-derive each registry's `dec`
  measure inside Lean, the denotation is FUEL-INDEXED: `denote (fuel : Nat) …`. A `specCall` consumes
  one unit of fuel (`fuel+1 → fuel` for the body); a structural subterm keeps the SAME fuel (it is
  smaller by `sizeOf`). The recursion is then well-founded on the lexicographic `(fuel, sizeOf e)`
  (Lean's `termination_by`/`decreasing_by`, CORE — no Mathlib). THIS IS NOT A FUEL-CAP VACUITY
  DODGE: (i) the soundness theorem `ref_sound` is proved for ALL `fuel` (∀-quantified); (ii) the
  SOURCE (`denote`) and the ENCODER (`refDenote`) use the SAME `fuel` and the SAME `Registry`, so at
  EVERY fuel — including the fuel-`0` bottom, where both bottom out to the IDENTICAL shared default
  `True`/`0` — the two sides AGREE (the call-site soundness is exactly "args agree by the IH + the
  SAME registry resolves the SAME body, denoted at the SAME fuel"). The `dec`-bounded source spec fn
  terminates, so for a real call there is always a fuel at which the fixpoint is reached; T1 holds
  uniformly across fuel, which is STRONGER than holding at one fuel.

  An `Env` maps each free name (a param / `result` / `old(x)`) to an `Int` — exactly
  the per-clause obligation binding `ref_encode.rs` describes — and carries the SHARED spec-fn
  `Registry` (#181). The non-spec-fn fragment is a TOTAL structural recursion (a clause is a pure
  predicate; no state, no loops); the spec-fn calls add the fuel-indexed well-founded layer.
-/
import Thermite.Ast

namespace Thermite

/-- An `Option`/`Result` VALUE (#180, the C7 payload-in-contract fragment): the value an
    `optResVar` scrutinee denotes. `none`/`some v`/`ok v`/`err e` — the payload `v`/`e` an
    `Int` (the C7 corpus payloads are integer-valued; faithful to the corpus, NOT general
    ADTs). The `match`-arm selection reads the variant; the `is`-test reads the variant; the
    payload is bound into the arm's predicate via `Env.bindInt`. -/
inductive OptResVal where
  | none_
  | some_ (v : Int)
  | ok    (v : Int)
  | err   (e : Int)
  deriving DecidableEq, Repr

/-- The denotation environment: a valuation of free names. Integer names (params,
    `result`, `old(x)`) map to `Int`; SEQUENCE names (a `&[u32]` slice / a `String`'s
    bytes — #178) map to `List Int`; OPTION/RESULT names (a `match`/`is` scrutinee — #180)
    map to an `OptResVal`. `S_C`'s `Env` extended to the sequence + option/result domains.
    The maps are independent (a name is bound at exactly one sort by the obligation's
    parameter binding). -/
structure Env where
  /-- The integer-valued free names (params / `result` / `old(x)`). -/
  ints : String → Int
  /-- The sequence-valued free names (slice params, `String` byte sequences). The
      `@`-view is the IDENTITY on this value — a slice and its `@`-view are the same
      `List Int` (#178). -/
  seqs : String → List Int
  /-- The `Option`/`Result`-valued free names (a `match`/`is` scrutinee — #180). -/
  optres : String → OptResVal
  /-- The SHARED spec-fn `Registry` (#181): the name→`SpecFn` map a `specCall` resolves
      against. SHARED between `denote` and `refDenote` (the body is lowered ONCE + sound by the
      fragment; the registry is the external ground truth), which is what makes the call-site
      soundness "the SAME registry resolves the SAME body". -/
  specs : Registry

/-- Bind an integer name to a value in an environment (#179): used to interpret a
    predicate closure `|x| <body>` at a concrete element — the bound var `x` is set to
    the i-th element while the sequence names are unchanged. The SHARED env-update both
    `denote` (source) and `RefEncode.refDenote` (encoder) route a combinator predicate
    through, so the predicate's denotation is identical on both sides modulo the body's
    own integer/sequence subterms (settled by `refVal_eq`). -/
def Env.bindInt (env : Env) (name : String) (v : Int) : Env :=
  { env with ints := fun s => if s = name then v else env.ints s }

/-- Bind a spec fn's PARAMS to its (already-denoted) ARG VALUES (#181): the call
    `foo(args)` binds each `params[k]` to `vals[k]` as an integer name, then denotes the
    body in that env (`⟦body_of(f)⟧[params ↦ ⟦args⟧]`). SHARED by `denote`/`refDenote` (the
    param binding is the call's calling convention — identical on both sides; the only call-site
    content is whether the args agree, settled by the soundness IH). A length mismatch (a
    mis-arity call — rejected by the validator upstream) simply binds the common prefix; a
    well-formed call has `|params| = |args|`. The spec-fn body sees ONLY its params (a spec fn is
    closed over its params — §4.2), so the rest of the env is irrelevant to the body, but is carried
    so the SHARED `specs` registry stays in scope for nested `specCall`s. -/
def Env.bindParams (env : Env) (params : List String) (vals : List Int) : Env :=
  match params, vals with
  | [], _ => env
  | _, [] => env
  | p :: ps, v :: vs => (env.bindInt p v).bindParams ps vs

/-- The VARIANT an `OptResVal` is (#180): the discriminant a `match` arm selects on / an
    `is`-test reads. Shared by `denote`/`refDenote` (the Verus `match`/`is` discriminant is the
    SAME meaning on both sides — the encoder reuses Verus's variant semantics verbatim). -/
def OptResVal.variant : OptResVal → Variant
  | OptResVal.none_  => Variant.none_
  | OptResVal.some_ _ => Variant.some_
  | OptResVal.ok _    => Variant.ok
  | OptResVal.err _   => Variant.err

/-- The PAYLOAD an `OptResVal` carries (#180): the `Int` bound into a `Some(v)`/`Ok(v)`/`Err(e)`
    arm's predicate. `None` carries no payload — modelled as `0` (a `None` arm has no binder, so
    the payload is never observed; the `0` keeps the read total). Shared by `denote`/`refDenote`. -/
def OptResVal.payload : OptResVal → Int
  | OptResVal.none_   => 0
  | OptResVal.some_ v => v
  | OptResVal.ok v    => v
  | OptResVal.err e   => e

/-- The `is`-test result (#180): does an `OptResVal` have the tested `Variant`? Shared by
    `denote`/`refDenote` (the Verus `(e is V)` discriminant test is the SAME meaning on both
    sides — `ref_encode.rs`'s `Expr::Is` arm reuses Verus's `is`). -/
def OptResVal.isVariant (v : OptResVal) (target : Variant) : Bool :=
  decide (v.variant = target)

/-- The shared TOTAL sequence-index access (#178): the i-th element, or `0` when the
    index is out of range (negative or ≥ length). The in-range obligation `0 ≤ i < |s|`
    is a SOURCE precondition (L0); the total `0`-default is the analogue of the
    div-by-zero convention, held CONSISTENT between `denote` and `refDenote` (both route
    through `seqIdx`), so T1 is insensitive to the out-of-range point. -/
def seqIdx (s : List Int) (i : Int) : Int :=
  s.getD i.toNat 0

/-- The shared TOTAL `subrange(lo, hi)` (#178): the contiguous sub-sequence of indices
    `[lo, hi)`. Modelled as `drop lo` then `take (hi - lo)` over `List`, total under the
    `0 ≤ lo ≤ hi ≤ |s|` source frame (out-of-range clamps via `List.take`/`List.drop`,
    held consistent across both denotations). `&xs[..i]` = `subrange 0 i` = `take i`. -/
def seqSub (s : List Int) (lo hi : Int) : List Int :=
  (s.drop lo.toNat).take (hi.toNat - lo.toNat)

/-- The SHARED recursive COUNT of a `count_where` combinator (#182), defined FAITHFULLY to the
    frozen `verus_l3` (`thermite-spec/src/combinators.rs`):

      `spec fn count_where(s, p) -> nat decreases s.len() {`
      `  if s.len() == 0 { 0 } else { (if p(s[0]) {1} else {0}) + count_where(s.drop_first(), p) } }`

    This is STRUCTURAL recursion over the source `List Int` (core Lean — NO Mathlib, NO fuel: the
    list shrinks by `List.tail`, mirroring Verus's `s.drop_first()` / `decreases s.len()`). The
    per-element predicate is a `p : Int → Prop` (the closure body's denotation at the element); the
    `if p(s[0])` test uses `Classical` decidability (the soundness axiom set already admits
    `Classical`), so the `verus_l3` `(if p(s[0]) {1nat} else {0nat})` is modelled EXACTLY. SHARED by
    `intVal` (source) and `RefEncode.refIntVal` (encoder): BOTH pass the SAME structural-count
    function the SAME slice + predicate, so the soundness content is purely whether the slice + the
    per-element predicate agree (the `refSeqVal_eq_seqVal` + the recursive `ref_sound` IH on the
    flat closure body) — exactly mirroring how the bounded combinators reuse the frozen body.
    `noncomputable` because the per-element test uses `Classical.propDecidable` (the soundness axiom
    set already admits `Classical`); the artifact is a PROOF object, not run. -/
noncomputable def countWhereVal (p : Int → Prop) : List Int → Int
  | []      => 0
  | x :: xs => (@ite _ (p x) (Classical.propDecidable _) (1 : Int) 0) + countWhereVal p xs

/-- `count_where`'s `verus_l3` head-then-tail step, made explicit for the soundness proof: the count
    of `x :: xs` is `(if p x then 1 else 0) + countWhereVal p xs`. Holds by `rfl` (the defining
    equation). Mirrors the `verus_l3` `else` branch `(if p(s[0]) {1} else {0}) + count_where(s.drop_first(), p)`. -/
theorem countWhereVal_cons (p : Int → Prop) (x : Int) (xs : List Int) :
    countWhereVal p (x :: xs)
      = (@ite _ (p x) (Classical.propDecidable _) (1 : Int) 0) + countWhereVal p xs := rfl

/-- The COUNT-CHARACTERIZATION of MULTISET equality (#182), modelling `permutation_of`'s frozen
    `verus_l3` `a.to_multiset() == b.to_multiset()` WITHOUT Mathlib's `Multiset`: two sequences are
    permutations iff EVERY value occurs the SAME number of times in both (core `List.count`). This IS
    multiset equality (two multisets are equal iff their per-element multiplicities coincide) — and
    is precisely what distinguishes it from SET equality (membership): `[1,1,2]` and `[1,2,2]` have
    the SAME set `{1,2}` but DIFFERENT multisets (`count 1` is `2` vs `1`), so `permEq` is FALSE on
    them while a set-based model would wrongly say TRUE (the multiset-vs-set teeth). SHARED by
    `denote`/`refDenote` (both compute the SAME `permEq` over the two `@`-viewed slices; the
    soundness content is purely that the two slices agree). -/
def permEq (a b : List Int) : Prop :=
  ∀ x : Int, a.count x = b.count x

/-- The SHARED integer meaning of an arithmetic operator over two `Int` operand
    VALUES (#176). Defined ONCE here so BOTH `denote` (source) and
    `RefEncode.refDenote` (encoder) route through it — the soundness content for
    arithmetic is the encoder's OPERATOR-MAP faithfulness (`arith → "+"` etc.),
    mirroring `tokRel (encOp op)` for comparisons, NOT a re-derivation of each
    op's integer value.

    - `add`/`sub`/`mul`/`div`/`rem` use core `Int` arithmetic. `div`/`rem` are the
      TOTAL `Int./`/`Int.%` under the divisor-≠0 convention (the partial point is a
      source precondition; see the module header).
    - `shl`/`shr`/`bitAnd`/`bitOr`/`bitXor` are defined over the `Nat` value of each
      operand (`Int.toNat`) using core `Nat` bit ops, re-injected as `Int`. The
      frozen `S_C` bitwise/shift operands are bounded UNSIGNED values (`u64`/`u32`/
      `usize`), so `Int.toNat` is value-preserving on them (the `≥ 0` frame, the
      same source obligation as the cast `nat` clamp). Core-only — no Mathlib. -/
def arithDenote : ArithOp → Int → Int → Int
  | ArithOp.add,    x, y => x + y
  | ArithOp.sub,    x, y => x - y
  | ArithOp.mul,    x, y => x * y
  | ArithOp.div,    x, y => x / y
  | ArithOp.rem,    x, y => x % y
  | ArithOp.shl,    x, y => (Nat.shiftLeft x.toNat y.toNat : Int)
  | ArithOp.shr,    x, y => (Nat.shiftRight x.toNat y.toNat : Int)
  | ArithOp.bitAnd, x, y => (Nat.land x.toNat y.toNat : Int)
  | ArithOp.bitOr,  x, y => (Nat.lor x.toNat y.toNat : Int)
  | ArithOp.bitXor, x, y => (Nat.xor x.toNat y.toNat : Int)

/-- The SHARED value coercion of a cast target over an `Int` operand value (#177).
    Defined ONCE so BOTH `denote` and `RefEncode.refDenote` route through it — the
    soundness content for casts is the encoder's CAST-TARGET map faithfulness +
    the PARENTHESIZATION of the inner (the #122/#146 class, modelled in
    `RefEncode.lean`), NOT a re-derivation of the coercion.

    - `nat`: inject into the naturals (`Int.toNat`, clamping a negative to `0`; in
      `S_C` an `as nat` carries a `≥ 0` source frame — see the module header).
    - `int`/`u64`/`u32`/`usize`: value-preserving on the spec `int` domain (the
      bounded no-overflow frame is a source obligation, carried like div-by-zero). -/
def castDenote : CastTy → Int → Int
  | CastTy.nat,   v => (v.toNat : Int)
  | CastTy.int,   v => v
  | CastTy.u64,   v => v
  | CastTy.u32,   v => v
  | CastTy.usize, v => v

/- `⟦·⟧_{S_C}` on the SEQUENCE-valued terms (#178): a `seqVar`/`strVar` denotes its
   environment sequence (the `@`-view is the identity on this value); a `subrange`
   denotes the contiguous sub-sequence `seqSub` of its base over the range bounds
   (`&xs[..i]` = `seqSub 0 i`, `&xs[a..b]` = `seqSub a b`, `&xs[a..]` = `seqSub a |xs|`).
   These are the SOURCE meanings (before the rewrite); the encoder's `@`/`subrange`
   rewrite preserves them. A non-sequence node denotes the empty sequence (never
   observed by the soundness theorem, which only evaluates `seqVal` on sequence-sorted
   bases — keeps it TOTAL without a `sorry`). Mutual with `intVal` (`subrange`'s bounds
   are integer terms). -/
/-- The `OptResVal` a `match`/`is` SCRUTINEE denotes (#180): an `optResVar` reads its env
    `optres` value; any other node is not an `Option`/`Result` scrutinee in a well-formed C7
    clause, so it denotes the canonical `none` (never observed — the soundness theorem only
    evaluates this on an `optResVar` scrutinee; keeps it total without a `sorry`). Shared by
    `denote`/`refDenote` (the scrutinee value is a free name, the SAME on both sides). This is
    fuel-free: a scrutinee is a free name, never a `specCall` (the C7 corpus scrutinees are
    params/`result`), so it reads the env directly. -/
def scrutVal : Expr → Env → OptResVal
  | Expr.optResVar x, env => env.optres x
  | _, _ => OptResVal.none_

/- THE FUEL-INDEXED DENOTATION (#178/#179/#180 fragment EXTENDED with #181 spec-fn calls).
   The non-spec-fn fragment is a structural recursion (fuel threaded UNCHANGED to subterms — a
   subterm is smaller by `sizeOf`); a `specCall` CONSUMES one unit of fuel (`fuel+1 → fuel` for the
   resolved body), so the whole recursion is well-founded on the lexicographic `(fuel, sizeOf e)`
   (`termination_by`/`decreasing_by`, CORE Lean — no Mathlib). The fuel is SHARED with the encoder
   (`RefEncode.lean`'s identically-fuelled `refDenote`), so the call-site soundness is fuel-uniform
   (see the module header — NOT a fuel-cap vacuity dodge: T1 holds for ALL fuel, both sides agree at
   every fuel including the fuel-`0` shared bottom). The block is `seqVal`/`intVal`/`intValArgs`/
   `denote`/`denoteArms`, all fuel-indexed and mutually recursive (the #181 `specCall` is BOTH an
   integer term — `intVal` — and a predicate — `denote` — depending on the spec fn's return sort,
   so both route to the body denotation at the consumed fuel). -/
mutual
/-- `⟦·⟧_{S_C}` on the SEQUENCE-valued terms (#178), fuel-indexed (#181). See the block comment.
    Fuel is threaded UNCHANGED to the base/bound subterms (a subrange base/bound is smaller). -/
noncomputable def seqVal : Nat → Expr → Env → List Int
  | _,    Expr.seqVar x, env => env.seqs x
  | _,    Expr.strVar x, env => env.seqs x
  | fuel, Expr.subrange base r, env =>
      let s := seqVal fuel base env
      match r with
      | RangeArg.rangeTo hi    => seqSub s 0 (intVal fuel hi env)
      | RangeArg.range lo hi   => seqSub s (intVal fuel lo env) (intVal fuel hi env)
      | RangeArg.rangeFrom lo  => seqSub s (intVal fuel lo env) (s.length : Int)
  | _, _, _ => []
  termination_by fuel e _ => (fuel, sizeOf e)

/-- `⟦·⟧_{S_C}` on the INTEGER-valued terms (the operands of a comparison), fuel-indexed (#181): a
    literal denotes itself, a variable denotes its environment value, an arithmetic term denotes the
    shared `arithDenote` of its operands, a cast the shared `castDenote` of its inner; `idx`/`seqLen`
    /`byteAt` read the sequence (#178). THE #181 `specCall` arm: an integer-returning spec fn call
    `name(args)` resolves `name` in the SHARED `Env.specs` registry to a `SpecFn { params, body }`,
    binds the params to the denoted args (`intValArgs` — at the SAME fuel, the args are smaller), and
    denotes the BODY at the CONSUMED fuel (`⟦body⟧[params ↦ ⟦args⟧]`). At fuel `0` (or an unresolved
    name) it bottoms to the shared default `0` (NOT a dodge — `refIntVal` bottoms IDENTICALLY, so T1
    holds at fuel `0`; the soundness is fuel-uniform). Mutual with `seqVal`/`intValArgs`. -/
noncomputable def intVal : Nat → Expr → Env → Int
  | fuel, Expr.arith op a b,  env => arithDenote op (intVal fuel a env) (intVal fuel b env)
  | fuel, Expr.cast inner ty, env => castDenote ty (intVal fuel inner env)
  | fuel, Expr.idx base i,    env => seqIdx (seqVal fuel base env) (intVal fuel i env)
  | fuel, Expr.seqLen base,   env => (seqVal fuel base env).length
  | fuel, Expr.byteAt base i, env => seqIdx (seqVal fuel base env) (intVal fuel i env)
  | fuel+1, Expr.specCall name args, env =>
      match env.specs name with
      | some fn => intVal fuel fn.body (env.bindParams fn.params (intValArgs (fuel+1) args env))
      | none    => 0
  -- THE #182 `count_where` VALUE-combinator: a recursive `nat` COUNT (`ResultKind::Usize`). Read on
  -- the INTEGER side (`intVal`), faithful to the frozen `verus_l3` recursive count. The per-element
  -- predicate is the closure body denoted at the element (`denote fuel body (env.bindInt bound ·)`,
  -- the SAME closure-at-element shape the bounded combinators use via `Env.bindInt`); a missing
  -- predicate (never well-formed — `count_where` declares a `Pred` slot) counts `True` everywhere.
  -- The count itself is the SHARED structural `countWhereVal` over the slice's `@`-view (`seqVal`).
  | fuel, Expr.comb CombName.countWhere seq _ _ pred, env =>
      let s := seqVal fuel seq env
      let p : Int → Prop := fun x =>
        match pred with
        | some (Pred.mk bound body) => denote fuel body (env.bindInt bound x)
        | none => True
      countWhereVal p s
  | _,    Expr.intLit n,      _   => n
  | _,    Expr.var x,         env => env.ints x
  -- A boolean-sorted node never appears as a comparison operand in a well-formed
  -- clause; it has no integer meaning, so it denotes the canonical `0` (a fuel-`0`
  -- `specCall` likewise bottoms here — IDENTICAL on the encoder side). The soundness
  -- theorem only evaluates `intVal` on integer-sorted subterms, so this default is
  -- never observed there; it keeps `intVal` TOTAL without a `sorry`.
  | _, _, _ => 0
  termination_by fuel e _ => (fuel, sizeOf e)

/-- The denoted ARG VALUES of a `specCall` (#181): each arg's `intVal` at the SAME fuel (an arg is a
    structural subterm of the call — smaller by `sizeOf`). SHARED structure with the encoder's
    `refIntValArgs` (the per-arg encoding is `encode_call_arg`; the soundness content is that the
    args agree, settled by the recursive `ref_sound`/`refVal_eq` IH on each arg). Mutual with
    `intVal`. -/
noncomputable def intValArgs : Nat → List Expr → Env → List Int
  | _,    [],        _   => []
  | fuel, a :: rest, env => intVal fuel a env :: intValArgs fuel rest env
  termination_by fuel args _ => (fuel, sizeOf args)

/-- `⟦·⟧_{S_C}` — the SOURCE meaning of a contract predicate as a Lean `Prop`, fuel-indexed (#181).
    Each comparison/logical/negation denotes the STANDARD mathematical relation (the `S_C` inference
    rules), defined HERE following the SOURCE meaning — to be proved equal to `RefEncode.refDenote`
    (which follows the ENCODER's structure), so the soundness theorem has content. THE #181
    `specCall` arm: a boolean-returning spec fn call resolves `name` in the SHARED `Env.specs`,
    binds params to the denoted args, and denotes the BODY at the CONSUMED fuel (the body's
    soundness is the EXISTING fragment applied to it). Mutual with `seqVal`/`intVal`/`intValArgs`/
    `denoteArms`. -/
noncomputable def denote : Nat → Expr → Env → Prop
  | _,    Expr.boolLit b, _   => (b = true)
  | fuel, Expr.cmp op a b, env =>
      let x := intVal fuel a env
      let y := intVal fuel b env
      match op with
      | CmpOp.eq => x = y
      | CmpOp.ne => x ≠ y
      | CmpOp.lt => x < y
      | CmpOp.le => x ≤ y
      | CmpOp.gt => x > y
      | CmpOp.ge => x ≥ y
  | fuel, Expr.logic op a b, env =>
      match op with
      | LogOp.and => denote fuel a env ∧ denote fuel b env
      | LogOp.or  => denote fuel a env ∨ denote fuel b env
  | fuel, Expr.neg e, env => ¬ denote fuel e env
  -- The MATCH-IN-ENS form (#180): the arm SELECTED by the scrutinee's variant denotes its body
  -- with the payload BOUND (the C7 `match result { Some(v) => P(v), None => Q }`).
  | fuel, Expr.match_ scrut arms, env =>
      denoteArms fuel (scrutVal scrut env) arms env
  -- The `is`-test (#180): the variant DISCRIMINANT test `scrut is variant`.
  | _,    Expr.is_ scrut variant, env =>
      ((scrutVal scrut env).isVariant variant = true)
  -- The 6 BOUNDED-QUANTIFIER combinators (#179): each denotes its FROZEN `verus_l3` quantifier
  -- form, fuel threaded unchanged to the predicate body (the body is a structural subterm).
  | fuel, Expr.comb c seq seq2 idx pred, env =>
      let s := seqVal fuel seq env
      let s2 := match seq2 with | some e => seqVal fuel e env | none => []
      let n := match idx with | some e => intVal fuel e env | none => 0
      let p : Int → Prop := fun i =>
        match pred with
        | some (Pred.mk bound body) => denote fuel body (env.bindInt bound (seqIdx s i))
        | none => True
      match c with
      | CombName.forallIn =>
          ∀ i : Int, (0 ≤ i ∧ i < (s.length : Int)) → p i
      | CombName.existsIn =>
          ∃ i : Int, (0 ≤ i ∧ i < (s.length : Int)) ∧ p i
      | CombName.sorted =>
          ∀ i j : Int, (0 ≤ i ∧ i ≤ j ∧ j < (s.length : Int)) → seqIdx s i ≤ seqIdx s j
      | CombName.forallBelow =>
          ∀ i : Int, (0 ≤ i ∧ i < n ∧ i < (s.length : Int)) → p i
      | CombName.forallFrom =>
          ∀ i : Int, (n ≤ i ∧ i < (s.length : Int)) → p i
      | CombName.disjoint =>
          ∀ i j : Int,
            ((0 ≤ i ∧ i < (s.length : Int)) ∧ (0 ≤ j ∧ j < (s2.length : Int))) →
              seqIdx s i ≠ seqIdx s2 j
      -- THE #182 `permutation_of(a, b)` Prop-combinator: MULTISET equality `a.to_multiset() ==
      -- b.to_multiset()`, modelled via the count-characterization `permEq` over the two `@`-viewed
      -- slices (`s` = `a`, `s2` = `b`). Like `disjoint`, it threads two slices, no predicate.
      | CombName.permutationOf => permEq s s2
      -- `count_where` is a VALUE-combinator (`ResultKind::Usize`) — it is READ on the `intVal` side
      -- (above), never as a top-level predicate; here (its `denote` arm, unreachable in a well-formed
      -- clause where `count_where(..)` appears as a `nat` operand) it denotes the canonical `True`.
      | CombName.countWhere => True
  -- THE #181 SPEC-FN CALL as a top-level PREDICATE (a boolean-returning spec fn): resolve `name`,
  -- bind params to the denoted args, denote the body at the CONSUMED fuel. At fuel `0` / an
  -- unresolved name it bottoms to `True` (IDENTICAL to `refDenote`'s bottom — T1 holds at fuel `0`).
  | fuel+1, Expr.specCall name args, env =>
      match env.specs name with
      | some fn => denote fuel fn.body (env.bindParams fn.params (intValArgs (fuel+1) args env))
      | none    => True
  -- An integer-sorted leaf/term as a top-level predicate denotes `True` vacuously (never reached
  -- by the soundness theorem, whose top-level `Expr`s are predicates). Keeps `denote` TOTAL.
  | _, _, _ => True
  termination_by fuel e _ => (fuel, sizeOf e)

/-- The `match`-arm SELECTION + payload BINDING (#180), fuel-indexed (#181), SHARED structure for
    `denote` and `RefEncode.refDenote` (the encoder reuses the Verus `match` semantics verbatim).
    Walks the arms in source order; for the FIRST arm whose `Variant` matches, denotes that arm's
    body with the payload bound (fuel threaded unchanged — a body is a structural subterm). Mutual
    with `denote`. -/
noncomputable def denoteArms : Nat → OptResVal → List MatchArm → Env → Prop
  | _,    _,     [], _ => True
  | fuel, scrut, MatchArm.mk variant binder body :: rest, env =>
      if scrut.variant = variant then
        match binder with
        | some x => denote fuel body (env.bindInt x scrut.payload)
        | none   => denote fuel body env
      else
        denoteArms fuel scrut rest env
  termination_by fuel _ arms _ => (fuel, sizeOf arms)
end

end Thermite
