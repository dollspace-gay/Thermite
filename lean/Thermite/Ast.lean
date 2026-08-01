/-
  Thermite/Ast.lean — the contract-sublanguage AST as a Lean `inductive`, for the
  comparison + logical fragment (increment (a), #170) extended with the arithmetic
  operators (increment (b), #176), the casts (increment (c), #177), and the
  spec-context rewrites (increment (f), #178: slice→`@`/subrange, indexing, and the
  method→`spec_*` byte-view dispatch, the #127 class); epic #169.

  Governing design: `.design/verified/thermite-semantics.md` REQ-1/REQ-6 (the
  `S_C` denotation domain; the Lean module layout) + AC-1 (S is stated over the
  exact frozen subset the encoders admit) + Architecture §"S_C" (the binop map +
  the `cast → nat/int` coercion rule, "the #122 class is a property of the
  production string ... the (T1) obligation is precisely 'does the production
  string parse to an AST whose denotation matches'").

  This mirrors the relevant `thermite-syntax/src/ast.rs` `Expr` / `BinOp` /
  `Type` / `PrimType` variants for this fragment:
    - integer literals          (`Expr::IntLit { value, .. }`)
    - bool literals             (`Expr::BoolLit(b)`)
    - variables / `result` / `old(x)`  (`Expr::Path` + the `old(_)` call form —
      all denote as free names of type `Int`, per `S_C`'s "literals/refs denote
      themselves" rule; the obligation binds each as a distinct param)
    - comparison binops         (`BinOp::{Eq,Ne,Lt,Le,Gt,Ge}`)
    - logical binops + negation (`BinOp::{And,Or}`, `UnaryOp::Not`)
    - arithmetic binops (#176)   (`BinOp::{Add,Sub,Mul,Div,Rem,Shl,Shr,BitAnd,
                                  BitOr,BitXor}`: integer/`int` arithmetic over
                                  the values; no wraparound, overflow is an
                                  exec-side obligation, increment #171, not here)
    - casts (#177)               (`Expr::Cast { expr, ty }` to
                                  `u64`/`u32`/`usize`/`nat`/`int`, value coercions)

  Partiality (#176; normative home `thermite2-semantics.md` §4.1): `Div`/`Rem`/`Shl`/`Shr`
  are partial in the source. A zero divisor / a zero shift is rejected as a precondition
  (an L0 obligation discharged outside the contract clause, `ast.rs` `BinOp::Rem`
  "partial: requires a nonzero divisor"). The denotation models them with Lean's total
  `Int` operations under the divisor-≠0 convention; because `denote` and `refDenote` use
  the same total operation, T1 holds regardless of the guard (the guard is the source-side
  precondition, not part of the binop's meaning when the precondition holds;
  Euclidean-consistent, see `Denote.lean` and `thermite2-semantics.md` §4.1). The cast
  value coercions (#177) follow the same shape — value-preserving in spec position, the
  no-overflow frame a source obligation (`thermite2-semantics.md` §4.2).

  The spec-context rewrites (#178). In contract position a slice/`String` param does
  not denote a scalar; it denotes a sequence, and the encoder rewrites the use sites:
    - a slice param `xs`            → `xs@` (the `Seq` view; the identity on the value)
    - `xs[i]`                       → `xs@[i]` (the i-th element)
    - `&xs[..i]` / `&xs[a..b]`      → `xs@.subrange(0, i as int)` / `xs@.subrange(a, b)`
    - a `String` receiver `s.byte_at(i)` → `s.spec_byte_at(i)` (the byte-view dispatch)
    - `s.len()`                     → `s.spec_len()`             (the byte-view dispatch)
  The theorem (`ref_sound`): these rewrites preserve meaning (`@`/subrange/`spec_*` is
  the identity-on-meaning coercion from the exec slice/`String` to its spec sequence).
  The #127 negative lemma shows a wrong dispatch (wrong index / wrong receiver-method)
  fails soundness. To model this `Ast.lean` gains:
    - `SeqVar`             — a free sequence name (a `&[u32]` slice / a `String`'s bytes)
    - `Expr.idx`           — `xs[i]` over a sequence var at an integer index
    - `RangeArg` + `Expr.subrange` — the `&xs[..i]`/`&xs[a..b]`/`&xs[a..]` range borrow
    - `Expr.seqLen`        — `xs.len()` / `s.len()` (the sequence length, → `spec_len`)
    - `Expr.byteAt`        — `s.byte_at(i)` (the i-th byte, → `spec_byte_at`)
  These are integer-valued (an element / a byte / a length) except `subrange`, which is
  sequence-valued and feeds another `idx`/`seqLen`/`byteAt` (so the prefix's meaning is
  observed through a later element/length read, the way a contract clause uses it).

  The 6 bounded-quantifier combinators (#179, increment 1d-i). In contract position a
  combinator call `Call(C, args)` denotes its frozen `verus_l3` quantifier form
  (`thermite-spec/src/combinators.rs`), with each argument threaded per its registry
  arg-kind (`CombinatorSig.arg_kinds`/`ArgKind`):
    - `forall_in(s, p)`    = `∀ i, 0 ≤ i < s.len() → p(s[i])`
    - `exists_in(s, p)`    = `∃ i, 0 ≤ i < s.len() ∧ p(s[i])`
    - `sorted(s)`          = `∀ i j, 0 ≤ i ≤ j < s.len() → s[i] ≤ s[j]`
    - `forall_below(s,n,p)`= `∀ i, 0 ≤ i < n ∧ i < s.len() → p(s[i])`
    - `forall_from(s,n,p)` = `∀ i, n ≤ i < s.len() → p(s[i])`
    - `disjoint(a, b)`     = `∀ i j, (0 ≤ i < a.len() ∧ 0 ≤ j < b.len()) → a[i] ≠ b[j]`
  To embed these `Ast.lean` gains:
    - `CombName`           — the 6 frozen names (the closed combinator set, sans the 2
      recursive combinators `count_where`/`permutation_of`, which are #182 / 1d-ii and
      absent here — no embed-then-`sorry`).
    - `Pred`               — a flat predicate closure `|x| <body>` (`ArgKind::Pred`): a
      bound element-var name + a body `Expr` over the comparison/logical/arithmetic
      fragment (§4.2 "no anonymous nested quantifiers", so the body reuses the existing
      `Expr`). `p(s[i])` denotes as the body with the bound var ↦ the i-th element.
    - `Expr.comb`          — a combinator call carrying: the name `c`; a primary slice
      `seq` (`ArgKind::Slice` → the `@`-view, #178/1f); an optional second slice `seq2`
      (only `disjoint`); an optional scalar index `idx` (only `forall_below`/`forall_from`,
      `ArgKind::Index`, a scalar `int`, not a slice `@`-view: the #145 bug class); and
      an optional predicate `pred` (`ArgKind::Pred`). Each combinator populates exactly
      the fields its `arg_kinds` declares (the others `none`).

  The match-in-ens / `is` payload-in-contract forms (#180, increment 1g, the C7
  payload-in-contract class, `.design/basis/09-option-result.md`). In contract position a
  built-in `Option`/`Result` value (a param / `result`) may be projected by a spec-`match`
  or tested by `is`:
    - `match e { Some(v) => P(v), None => Q }`  (the Option form), and
      `match e { Ok(v) => P(v), Err(err) => Q(err) }`  (the Result form): the arm selected
      by `e`'s variant, with the payload bound in the arm predicate. Mirrors `ast.rs`
      `Expr::Match`/`MatchArm`/`Pattern` + `ref_encode.rs`'s `encode_match`/`encode_pattern`
      (the #150 work): the scrutinee + each arm body encode by the same recursion, the
      pattern is `encode_pattern`'s built-in `Some(x)`/`None`/`Ok(x)`/`Err(e)` form.
    - `e is Some` / `e is None` / `e is Ok` / `e is Err`: the variant discriminant test
      (`ast.rs` `Expr::Is`; `ref_encode.rs`'s `Expr::Is` arm `({s} is {variant})`). `Prop`-
      sorted, true iff `e`'s value is that variant.
  To embed these `Ast.lean` gains:
    - `OptResVar`            — a free `Option`/`Result`-valued name (the scrutinee of a
      `match`/`is`; a param or `result`). Its value (`none`/`some v`/`ok v`/`err e`) lives in
      the env (`Denote.lean` `Env.optres`); the payload `v`/`e` is an `Int` (the C7 corpus
      payloads are integer-valued: `i`/`v`/`err`; faithful to the corpus, not general ADTs).
    - `Variant`              — the 4 built-in variants `Some`/`None`/`Ok`/`Err`
      (`encode_pattern`'s `is_builtin_variant` set; a user enum variant is absent here, since
      `encode_pattern` `Err`s on it, so it is out of `S_C` and not embedded).
    - `MatchArm`             — a variant pattern (the matched `Variant` + an optional payload
      binder name, `Pattern::Enum`'s `fields[0]` binding / `Pattern::Binding`) + an arm-body
      `Expr` (the flat arm predicate). The C7 `match` arms are flat predicates over the bound
      payload (§4.2 cage), so the body reuses the existing `Expr`. (Guards / wildcard arms are
      out of the C7 fragment; the corpus matches are exhaustive 2-arm `Some/None`/`Ok/Err`.)
    - `Expr.match_`          — a contract-position `match scrut arms`: the scrutinee `Expr`
      (an `optResVar`) + the arms. `Prop`-sorted (a `match`-in-`ens` is a predicate; each arm
      body is a flat `bool`).
    - `Expr.is_`             — a variant test `scrut is variant`: the scrutinee `Expr` + the
      tested `Variant`. `Prop`-sorted.

  General user ADTs are not covered (scoped to Option/Result): `encode_pattern`
  itself only admits the built-in `Some/None/Ok/Err` (its `is_builtin_variant` gate `Err`s on a
  user variant, lacking the enum-qualification map), so user ADTs are out of what the encoder
  produces. They are not in `S_C` for this increment and are not embedded (no
  embed-then-`sorry`). A general-ADT match (with the variant→discriminant + payload model) is a
  future increment if/when the encoder admits user variants.

  Named spec-fn calls (#181, increment 1e, the well-founded recursive fragment). A user
  `spec fn foo(p0, p1, …) -> R { <body> } dec <measure>` (`thermite-syntax::ast::SpecFnItem`:
  `params`/`ret`/`dec`/`body`) referenced in a contract is an `Expr::Call` whose callee path is
  not a frozen combinator and not `old` (`ref_encode.rs::encode_call` case (3)). The encoder emits
  `name(<encoded args>)`; it does not inline the body. The body is lowered once as its own Verus
  `spec fn` (the registry entry), and the call site is a call to that fn. To embed this `Ast.lean`
  gains:
    - `Expr.specCall (name : String) (args : List Expr)` — the call form, mirroring
      `Expr::Call { callee := Path [name], args }` for a non-combinator/non-`old` callee
      (`encode_call`'s case (3)). The args are `Expr`s of the same fragment (re-encoded by
      `encode_call_arg`). Mutual with `Expr` (the args are a `List Expr`). Integer- or
      boolean/`Prop`-sorted per the spec fn's return type (the corpus spec fns return `nat`/`int`
      or `bool`; the denotation reads the integer or boolean meaning of the resolved body).
    - `SpecFn` (a `structure`) — a spec fn's signature/body: `params : List String` (the param
      names, bound to the denoted args), `body : Expr` (the body over the same fragment, may contain
      further `specCall`s — recursion). Mirrors `SpecFnItem.params`/`body`. (The `dec` measure
      lives in the termination argument of the denotation, not in this datum; see `Denote.lean`'s
      fuel-indexed well-founded denotation: the spec fn is total/terminating by §4.2's mandatory
      `dec`, modelled as "the denotation holds for all fuel"; the encoder + source share the same
      fuel, so T1 is fuel-uniform, not a fuel-cap vacuity dodge.)
    - `Registry` — `String → Option SpecFn`, the spec-fn registry shared between `denote` and
      `refDenote` (the body's meaning is the same; it is lowered once + sound by the fragment, and the
      registry is the external ground truth, like the combinator registry `lookup`). Carried in the
      `Env` (`Denote.lean` `Env.specs`).

  The 2 recursive / aggregate combinators (#182, increment 1d-ii, the last contract brick,
  completing the closed 8-combinator set 8/8). `count_where` (a recursive `nat` count) and
  `permutation_of` (multiset equality) are embedded (`CombName.countWhere`/`permutationOf`,
  reusing the `Expr.comb` constructor). `count_where` is a value-combinator: it threads
  `intVal`/`refIntVal` (read at its integer count), faithful to the recursive `verus_l3`
  (`Denote.lean` `countWhereVal`, structural recursion on the source `List`, core Lean, no Mathlib,
  no fuel: the list shrinks by `List.tail`/`drop_first`). `permutation_of` threads `denote`/
  `refDenote` (a `Prop`), modelled via the count-characterization `∀ x, a.count x = b.count x`
  (core `List.count`, not Mathlib's `Multiset`; this is multiset equality, and is what makes the
  multiset-vs-set teeth `[1,1,2]`/`[1,2,2]` bite). Core Lean sufficed, no Mathlib wall.

  Deferred, not embedded here (no `sorry`-behind-a-variant; embedding-then-`sorry`
  is forbidden). The remaining sub-increment:
    - general user-ADT match/is (beyond the built-in Option/Result), see above.
  It is a real future inductive case, listed (not stubbed) so the deferral is documented.
-/

namespace Thermite

/-- The comparison operators of the frozen contract sublanguage — mirrors the
    `BinOp::{Eq,Ne,Lt,Le,Gt,Ge}` arms of `thermite-syntax/src/ast.rs`. These relate
    two integer operands and denote a `Prop`. The `==`-vs-`<=` faithfulness is the
    distinction between `CmpOp.Eq` and `CmpOp.Le`. -/
inductive CmpOp where
  | eq
  | ne
  | lt
  | le
  | gt
  | ge
  deriving DecidableEq, Repr

/-- The binary logical connectives — mirrors `BinOp::{And,Or}`. -/
inductive LogOp where
  | and
  | or
  deriving DecidableEq, Repr

/-- The arithmetic binary operators of the frozen contract sublanguage (#176) —
    mirrors the `BinOp::{Add,Sub,Mul,Div,Rem,Shl,Shr,BitAnd,BitOr,BitXor}` arms of
    `thermite-syntax/src/ast.rs`. In contract position these are integer (`int`)
    arithmetic over the operand values, with no wraparound (the unbounded-`int` spec
    domain; overflow is an exec obligation, not modelled here). `Div`/`Rem`/`Shl`/
    `Shr` are partial (divisor/shift ≠ 0 precondition); see `Denote.lean` for the
    total-operation-under-the-convention modelling. These take two `Int` operands
    and produce an `Int` (unlike `CmpOp`, which produces a `Prop`). -/
inductive ArithOp where
  | add
  | sub
  | mul
  | div
  | rem
  | shl
  | shr
  | bitAnd
  | bitOr
  | bitXor
  deriving DecidableEq, Repr

/-- The 6 bounded-quantifier combinator names (#179) — the frozen `verus_l3` forms of
    `thermite-spec/src/combinators.rs` whose denotation is a bounded `∀`/`∃` over a
    slice. The 2 recursive / aggregate combinators (`count_where`/`permutation_of`) are
    #182 (1d-ii) and absent here: their `verus_l3` is a well-founded recursion /
    a multiset equality, not a bounded quantifier, so embedding them here would force a
    `sorry` (forbidden). This enum is the closed bounded-quantifier subset. -/
inductive CombName where
  | forallIn      -- `forall_in(s, p)`    = `∀ i, 0 ≤ i < s.len() → p(s[i])`
  | existsIn      -- `exists_in(s, p)`    = `∃ i, 0 ≤ i < s.len() ∧ p(s[i])`
  | sorted        -- `sorted(s)`          = `∀ i j, 0 ≤ i ≤ j < s.len() → s[i] ≤ s[j]`
  | forallBelow   -- `forall_below(s,n,p)`= `∀ i, 0 ≤ i < n ∧ i < s.len() → p(s[i])`
  | forallFrom    -- `forall_from(s,n,p)` = `∀ i, n ≤ i < s.len() → p(s[i])`
  | disjoint      -- `disjoint(a, b)`     = `∀ i j, (0≤i<a.len() ∧ 0≤j<b.len()) → a[i]≠b[j]`
  -- The 2 recursive / aggregate combinators (#182, increment 1d-ii, the last contract brick,
  -- completing the closed 8-combinator set). Mirrors the frozen `verus_l3` of
  -- `thermite-spec/src/combinators.rs`:
  | countWhere    -- `count_where(s, p)` : a value (`nat`/`Int`), the recursive count of the elements
                  --   of `s` satisfying `p`. `verus_l3` (recursive, `decreases s.len()`):
                  --     `if s.len()==0 { 0 } else { (if p(s[0]) {1} else {0}) + count_where(s.drop_first(), p) }`
                  --   `arg_kinds = [Slice, Pred]`, `result = Usize` (`ResultKind::Usize`). Threads the
                  --   `intVal`/`refIntVal` side (a value-combinator), unlike the 6 bounded combinators
                  --   (which are `Prop` via `denote`/`refDenote`).
  | permutationOf -- `permutation_of(a, b)` : a `Prop` (`bool`), `a` is a permutation of `b`.
                  --   `verus_l3`: `a.to_multiset() == b.to_multiset()` (multiset equality, not set).
                  --   Modelled without Mathlib's `Multiset` via the count-characterization
                  --   `∀ x, a.count x = b.count x` (core `List.count`), which is multiset
                  --   equality. `arg_kinds = [Slice, Slice]`, `result = Bool`. Threads `denote`/
                  --   `refDenote` (a `Prop`-combinator like `disjoint`, two slice args, no predicate).
  deriving DecidableEq, Repr

/-- The cast targets of the frozen contract sublanguage (#177) — mirrors the
    cast-admitting `Type`/`PrimType` arms `ref_encode.rs::cast_target` accepts:
    the bounded prims `u64`/`u32`/`usize` (`PrimType::{U64,U32,Usize}`) and the
    spec arithmetic ladder `nat`/`int` (`Type::Named "nat"`/`"int"`). A cast to
    `bool` or any other type is `Unsupported` in the encoder, so it is out of `S_C`
    and absent here. -/
inductive CastTy where
  | u8
  | u16
  | u64
  | u32
  | usize
  | nat
  | int
  deriving DecidableEq, Repr

/-- The 4 built-in `Option`/`Result` variants of the C7 payload-in-contract fragment
    (#180) — mirrors `ref_encode.rs::is_builtin_variant`'s set `{"Some","None","Ok","Err"}`
    (the only variants `encode_pattern` admits unqualified; a user variant is a
    `RefEncodeError`, so out of `S_C` and absent here). These are both the `match`-arm
    pattern heads (`Some(v)`/`None`/`Ok(v)`/`Err(e)`) and the `is`-test discriminants
    (`e is Some`/`e is None`/`e is Ok`/`e is Err`). -/
inductive Variant where
  | some_   -- `Some(v)` / `is Some`
  | none_   -- `None`     / `is None`
  | ok      -- `Ok(v)`    / `is Ok`
  | err     -- `Err(e)`   / `is Err`
  deriving DecidableEq, Repr

/- The contract-sublanguage expression. Four syntactic sorts are distinguished by
   the inductive shape: `intLit`/`var`/`arith`/`cast`/`idx`/`seqLen`/`byteAt` build
   integer terms (operands of a comparison / an element / a byte / a length);
   `cmp`/`logic`/`neg`/`boolLit` build boolean/`Prop` terms; `seqVar`/`subrange`
   build sequence terms (the slice→`@`-view + the `subrange` borrow, observed through
   a later `idx`/`seqLen`/`byteAt`). A `var` carries a `String` name (a param,
   `result`, or the `old(x)` pre-state binding, all free integer names, per `S_C`);
   a `seqVar`/`strVar` carries a free sequence name (#178).

   `RangeArg` (the slice-borrow range) is mutual inductive with `Expr` because its
   bounds are integer-valued `Expr`s (cast `as int` by the encoder). -/
mutual
/-- The contract-sublanguage expression (the integer/bool/sequence terms). -/
inductive Expr where
  /-- An integer literal `IntLit { value }`. Lean models the value as `Int`
      (the spec numeric domain `S_C` denotes into is unbounded `int`). -/
  | intLit (value : Int)
  /-- A boolean literal `BoolLit(b)`. -/
  | boolLit (value : Bool)
  /-- A free integer variable: a param, `result`, or `old(x)` (all bound as distinct
      obligation params of type `Int`). Mirrors `Expr::Path([name])` and the
      `old(_)` form, which `ref_encode.rs` likewise treats as free names. -/
  | var (name : String)
  /-- A free boolean variable (#253, increment (iv), the §4.1.2 `bindBool` spine
      prerequisite): a bool-sorted free name (a bool param / a bool `result`) read via
      `Env.bools`. The exec-body bridge needs the contract `ens` to mention a bool
      `result`, but the existing AST has no bool-sorted name node (only `boolLit`, the
      literal). `boolVar x` is bool-sorted: it `denote`s via `env.bools x` (the `boolLit`
      arm's shape), but has no integer meaning. In `intVal` position it falls to the
      bool-sorted catch-all `0` (a `boolVar` in Int position is a sort error the
      exporter's EXP discipline never emits). It carries no subterms and is fuel-free
      (it never reaches a `specCall`), so its blast radius across the mutual denotation
      family is mechanical: one trivial arm each. Mirrors a Verus bool path read. -/
  | boolVar (name : String)
  /-- A comparison `a <op> b` over two integer subterms (`Expr::Binary` with a
      comparison `BinOp`). -/
  | cmp (op : CmpOp) (lhs rhs : Expr)
  /-- A logical connective `a <op> b` over two boolean subterms (`Expr::Binary`
      with `BinOp::{And,Or}`). -/
  | logic (op : LogOp) (lhs rhs : Expr)
  /-- Logical negation `!a` (`Expr::Unary { op := UnaryOp::Not, .. }`). -/
  | neg (e : Expr)
  /-- An arithmetic binary `a <op> b` over two integer subterms (#176; `Expr::Binary`
      with an arithmetic `BinOp`). Builds an integer term (a comparison operand). -/
  | arith (op : ArithOp) (lhs rhs : Expr)
  /-- A cast `inner as ty` (#177; `Expr::Cast { expr := inner, ty }`). Builds an
      integer term (the coerced value). The parenthesization the encoder applies to
      `inner` (the #122/#146 discipline) is modelled in `RefEncode.lean`: because
      the cast wraps a whole subexpression as its operand, dropping the paren would
      re-parse a compound `inner` and change the denotation (the negative lemma). -/
  | cast (inner : Expr) (ty : CastTy)
  /-- A free sequence variable: a `&[u32]` slice param (#178). In contract position
      the encoder rewrites it to its `@`-view (`xs` → `xs@`), which is the identity on
      the value (the same sequence of elements). Mirrors `encode_slice_arg`'s
      `Expr::Path` arm. Sequence-sorted, observed only through `idx`/`subrange`/
      `seqLen` (a bare sequence is not a `Prop`). -/
  | seqVar (name : String)
  /-- A free `String` variable whose bytes are the sequence (#178; the #127 byte-view
      class). The encoder dispatches its `.len()`/`.byte_at(i)` to the wrapper spec
      fns `spec_len`/`spec_byte_at` (`encode_string_byteview`); the bytes it denotes
      over are the same sequence. Sequence-sorted (a `List` of byte values, modelled
      as `Int`). -/
  | strVar (name : String)
  /-- `xs[i]` — a single-element index (#178; `Expr::Index { index := Single(i) }`).
      The encoder rewrites to `xs@[i as int]` (`encode_index`'s `Single` arm over the
      receiver's `@`-view); the meaning is the i-th element. Integer-sorted. `base` is
      a sequence term (a `seqVar` or a `subrange`), `idx` an integer term. -/
  | idx (base : Expr) (index : Expr)
  /-- `&xs[..i]` / `&xs[a..b]` / `&xs[a..]` — a slice-range borrow (#178;
      `Expr::Ref` of an `Expr::Index` of a range, routed through `encode_ref`→
      `encode_index`). The encoder rewrites to `xs@.subrange(lo, hi)`; the meaning is
      the corresponding contiguous sub-sequence. Sequence-sorted (`base` a sequence
      term, the range an integer-bounded `RangeArg`). -/
  | subrange (base : Expr) (range : RangeArg)
  /-- `xs.len()` / `s.len()` — the sequence length (#178; `Expr::MethodCall` `len`).
      For a slice it rewrites to `xs@.len()` (`encode_method_call`'s `len` arm); for a
      `String` to `s.spec_len()` (`encode_string_byteview`'s `len` arm). The meaning is
      the length of the sequence. Integer-sorted. -/
  | seqLen (base : Expr)
  /-- `s.byte_at(i)` — the i-th byte of a `String`'s byte sequence (#178; the #127
      byte-view dispatch; `Expr::MethodCall` `byte_at`). The encoder rewrites to
      `s.spec_byte_at(i)` (`encode_string_byteview`'s `byte_at` arm). The meaning is
      the i-th byte. Integer-sorted. `base` is a `String`-sequence term, `index` an
      integer term. The #127 class lives here: a wrong index / a wrong receiver-method
      is a different meaning (the negative lemma). -/
  | byteAt (base : Expr) (index : Expr)
  /-- A bounded-quantifier combinator call (#179) — `Call(C, args)` for `C` in the 6
      frozen bounded combinators. Denotes its frozen `verus_l3` quantifier form
      (`combinators.rs`) with each argument threaded per its registry arg-kind. The
      fields carry the per-kind args; each combinator populates exactly the fields its
      `CombinatorSig.arg_kinds` declares (`none` otherwise):
        - `seq`  : the primary slice (`ArgKind::Slice` → the `@`-view; sequence-sorted,
                   a `seqVar`/`strVar`/`subrange`).
        - `seq2` : the second slice (only `disjoint`'s `b`; `ArgKind::Slice`).
        - `idx`  : the scalar index bound (only `forall_below`/`forall_from`'s `n`;
                   `ArgKind::Index` → a scalar `int`, not a slice `@`-view; the #145 bug
                   class lives in this arg-kind's dispatch).
        - `pred` : the predicate closure (`ArgKind::Pred`; absent for `sorted`/`disjoint`).
      Boolean/`Prop`-sorted (a combinator result is `bool`), except `countWhere` (#182), which is
      value-sorted (`ResultKind::Usize`): it threads `intVal`/`refIntVal` (read at its integer count),
      not `denote`/`refDenote`. The two #182 recursive/aggregate combinators populate the fields as:
        - `countWhere`    : `seq` = `s`, `pred` = `some p`     (`arg_kinds = [Slice, Pred]`).
        - `permutationOf` : `seq` = `a`, `seq2` = `some b`     (`arg_kinds = [Slice, Slice]`; like
          `disjoint`, two slices, no predicate). `Prop`-sorted (multiset equality). -/
  | comb (c : CombName) (seq : Expr) (seq2 : Option Expr) (idx : Option Expr)
         (pred : Option Pred)
  /-- A free `Option`/`Result`-valued variable (#180): the scrutinee of a contract-position
      `match`/`is` (a param or `result`). Its value, `none`/`some v`/`ok v`/`err e` (the
      payload an `Int`, the C7 corpus shape), lives in the env (`Denote.lean` `Env.optres`).
      Mirrors the `Expr::Path` scrutinee of an `Expr::Match`/`Expr::Is` whose type is the
      built-in `Option`/`Result`. Option/result-sorted (observed only through `match_`/`is_`). -/
  | optResVar (name : String)
  /-- A contract-position `match scrut { <arms> }` (#180; `Expr::Match`) — the C7
      payload-in-contract projection `ens match result { Some(v) => P(v), None => Q }` (and the
      `Ok`/`Err` Result form). The arm selected by the scrutinee's variant denotes its body with
      the payload bound (the binder ↦ the variant's payload value). Boolean/`Prop`-sorted (each
      arm body is a flat `bool`). Mirrors `encode_match`: the scrutinee + arm bodies via the same
      recursion, the patterns via `encode_pattern`. Mutual with `MatchArm`. -/
  | match_ (scrut : Expr) (arms : List MatchArm)
  /-- A variant discriminant test `scrut is variant` (#180; `Expr::Is`) — `result is Some` /
      `is None` / `is Ok` / `is Err`. Boolean/`Prop`-sorted, true iff the scrutinee's value is
      that variant. Mirrors `ref_encode.rs`'s `Expr::Is` arm `({s} is {variant})`. -/
  | is_ (scrut : Expr) (variant : Variant)
  /-- A named spec-fn call `name(args)` (#181; `Expr::Call` for a non-combinator/non-`old`
      callee — `ref_encode.rs::encode_call`'s case (3), which emits `name(<encoded args>)`,
      not inlining the body). The `name` resolves in the shared `Registry` (carried in the
      `Env`) to a `SpecFn` whose `body` (over the same fragment, may recurse via further
      `specCall`s) is denoted with the params bound to the denoted args. Integer- or
      boolean/`Prop`-sorted per the spec fn's return type. Mutual with `Expr` (the `args` are a
      `List Expr` of the same fragment). The well-founded denotation (the `dec` measure ⟹
      termination ⟹ the fixpoint) is the fuel-indexed `Denote.lean`/`RefEncode.lean` recursion,
      proved sound for all fuel (the source + encoder share the fuel, not a fuel-cap dodge). -/
  | specCall (name : String) (args : List Expr)
  /-- A flat predicate closure `|x| <body>` (#179; `ArgKind::Pred`; `Expr::Closure` with
      one param over the frozen `Seq<u32>` element type `u32`). `bound` is the element
      var name the closure binds (`encode_pred_arg`'s `params[0]`); `body` is the
      predicate over that element, reusing the comparison/logical/arithmetic `Expr`
      fragment (§4.2 "no anonymous nested quantifiers"; the body is flat, so the
      structural recursion terminates). `p(s[i])` denotes as `body` with `bound ↦` the
      i-th element. Mutual with `Expr` (the body is an `Expr`). -/
inductive Pred where
  | mk (bound : String) (body : Expr)
  /-- A contract-position `match` arm (#180) — mirrors `thermite-syntax::ast::MatchArm` +
      `Pattern` restricted to the C7 built-in payload patterns `encode_pattern` admits:
      the matched `Variant` (`Some`/`None`/`Ok`/`Err`, the pattern head) + an optional payload
      binder name (the `Pattern::Enum`'s single field binding `Some(v)`/`Ok(v)`/`Err(e)`; `None`
      binds nothing) + the arm-body `Expr` (the flat arm predicate over the bound payload). A
      guard / wildcard / nested pattern is out of the C7 fragment (the corpus matches are
      exhaustive 2-arm `Some/None`/`Ok/Err`), so it is not modelled. Mutual with `Expr` (the
      body is an `Expr`). -/
inductive MatchArm where
  | mk (variant : Variant) (binder : Option String) (body : Expr)
  /-- A range argument of a spec-context slice borrow (#178) — mirrors the
      `thermite-syntax::ast::IndexArg` arms `encode_index`/`encode_ref` accept:
      `RangeTo(i)` (`&xs[..i]`), `Range(a, b)` (`&xs[a..b]`), `RangeFrom(a)`
      (`&xs[a..]`). A `Single(i)` is not here: a single-index borrow is the element
      form `Expr.idx` (`encode_index`'s `IndexArg::Single` arm), not a subrange. Each
      bound is an integer-valued `Expr` (cast `as int` by the encoder,
      `encode_index_value`). -/
inductive RangeArg where
  /-- `..i` (`&xs[..i]`) → `xs@.subrange(0, i as int)` (`encode_index`'s `RangeTo`). -/
  | rangeTo (hi : Expr)
  /-- `a..b` (`&xs[a..b]`) → `xs@.subrange(a, b)` (`encode_index`'s `Range`). -/
  | range (lo hi : Expr)
  /-- `a..` (`&xs[a..]`) → `xs@.subrange(a, xs@.len())` (`encode_index`'s `RangeFrom`). -/
  | rangeFrom (lo : Expr)
end

deriving instance Repr for Expr
deriving instance Repr for Pred
deriving instance Repr for RangeArg
deriving instance Repr for MatchArm

/-- A named spec fn (#181) — its signature/body as the registry stores it. Mirrors
    `thermite-syntax::ast::SpecFnItem`'s `params`/`body` (the `name` is the registry key; the
    `ret` type only fixes whether the call is read at the integer or boolean meaning; the `dec`
    measure lives in the denotation's termination argument, not here):
      - `params` — the param names, in order, bound to the denoted call args (`SpecFnItem.params`,
        their `Param.name`s).
      - `body`   — the body `Expr` over the same contract fragment (`SpecFnItem.body`); may contain
        further `specCall`s (recursion). The body is lowered once as its own Verus `spec fn`
        (`ref_encode.rs` does not inline; `encode_call`'s case (3) emits a call), so the body's
        soundness is the existing fragment applied to `body`, and the call-site soundness is the
        generic "args sound (IH) + same registry resolves the name". -/
structure SpecFn where
  params : List String
  body : Expr

/-- The spec-fn registry (#181): the name→`SpecFn` map, shared between `denote` and `refDenote`
    (the body's meaning is the same; it is lowered once and sound by the fragment, and the registry is
    the external ground truth, like the frozen combinator registry `thermite_spec::lookup`).
    Carried in the `Env` (`Denote.lean` `Env.specs`), so both denotations resolve a `specCall`
    against the same registry, the load-bearing fact for the call-site soundness. A name
    absent from the registry denotes a canonical default (never observed by the soundness theorem,
    which only evaluates a `specCall` whose name the registry resolves). -/
abbrev Registry := String → Option SpecFn

end Thermite
