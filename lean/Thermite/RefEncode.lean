/-
  Thermite/RefEncode.lean — a Lean model of the reference encoder's output as a
  denotation, for the comparison + logical fragment (#170) extended with the
  arithmetic operators (#176), the casts (#177), the spec-context rewrites
  (#178 — slice→`@`/subrange, indexing, the `String` byte-view dispatch, the #127 class),
  the 6 bounded-quantifier combinators (#179), the match-in-ens / `is` payload-in-contract
  forms (#180 / 1g — `encode_match`/`encode_pattern` [the #150 work] + the `Expr::Is` arm), and the
  named spec-fn calls (#181 / 1e — `encode_call`'s case (3)).

  What the #181 spec-fn calls model (faithful to `ref_encode.rs::encode_call`'s case (3)). A
  `specCall name args` — an `Expr::Call` whose callee path is not `old` and not a frozen combinator
  (`thermite_spec::lookup(name).is_none()`) — is lowered to a Verus spec-fn call `name(<encoded
  args>)`. The encoder does not inline the body (`Ok(format!("{name}({})", encoded_args.join(", ")))`;
  the body is lowered once as its own Verus `spec fn`, the registry entry). So the encoder's
  meaning of a call is: resolve `name` in the same `Env.specs` registry the source uses, bind the
  params to the encoder-denoted args (`refIntValArgs`, the per-arg `encode_call_arg` — a slice gets
  its `@`-view, a closure its `|x: u32|` form; for the corpus the args are scalar/slice terms of the
  already-proven fragment), and denote the same body at the consumed fuel. The call-site soundness is
  therefore the generic theorem "the args agree (the `refVal_eq` IH) + the same registry resolves the
  same body, denoted at the same fuel". The fuel is shared with the source
  (`Denote.lean`), so T1 is fuel-uniform (both sides bottom out identically at fuel `0`).

  What the #180 match/`is` models (faithful to `thermite-tv/src/ref_encode.rs`). In contract
  position a built-in `Option`/`Result` value is projected by a spec-`match` or tested by `is`:
    - `match scrut { Some(v) => P(v), None => Q }` (and `Ok`/`Err`) → a Verus `match`
      expression (`encode_match`): the scrutinee + each arm body via the same `refDenote`
      recursion, each arm pattern via `encode_pattern` (the built-in `Some(x)`/`None`/`Ok(x)`/
      `Err(e)` variant + payload binder). The arm selection-by-variant is the Verus `match`
      meaning (the shared `scrutVal`/`refDenoteArms` walk); the soundness content is the
      scrutinee/body encoding + the variant/binder choice. A swapped arm body (Some/None bodies
      exchanged) is a different meaning — the negative `match_arm_swap_breaks_soundness`.
    - `scrut is Variant` → the Verus `(scrut is V)` discriminant test (`Expr::Is` arm); its
      meaning is the shared `isVariant`. The faithfulness content is the variant choice: a wrong
      variant (`is Some` emitted as `is None`) is a different meaning — the negative
      `is_wrong_variant_breaks_soundness`.

  What the #178 rewrites model (faithful to `thermite-tv/src/ref_encode.rs`). In
  contract position the encoder rewrites slice/`String` use sites; this module models
  the meaning of the rewritten Verus the encoder produces:

    - slice var `xs` → `xs@`  (`encode_slice_arg`'s `Expr::Path` arm): the `@`-view is
      the identity on the sequence value (`refSeqVal (seqVar x) = env.seqs x`).
    - `xs[i]` → `xs@[i as int]`  (`encode_index`'s `IndexArg::Single` arm over
      `encode_receiver`'s `@`-view): the i-th element — `seqIdx (view) i`.
    - `&xs[..i]` → `xs@.subrange(0, i as int)`  (`encode_ref`→`encode_index`'s `RangeTo`
      arm); `&xs[a..b]` → `.subrange(a, b)` (`Range`); `&xs[a..]` →
      `.subrange(a, xs@.len() as int)` (`RangeFrom`): the contiguous sub-sequence —
      `seqSub`.
    - `s.byte_at(i)` → `s.spec_byte_at(i)`  (`encode_string_byteview`'s `byte_at` arm —
      the #127 byte-view dispatch): the i-th byte — `seqIdx (bytes) i`.
    - `s.len()` → `s.spec_len()`  (`encode_string_byteview`'s `len` arm) / `xs@.len()`
      (`encode_method_call`'s `len` arm): the length — `(view).length`.

  The #127 dispatch is modelled as an explicit step (`encByteView` : the method name →
  the byte-view spec fn). The faithfulness decision point for #178 lives there: a
  misdispatch (`byte_at`→a wrong index, or `byte_at`→the length spec fn) is a different
  meaning — the negative lemma `byteview_misdispatch_breaks_soundness` shows it fails
  soundness at a concrete sequence env (mirroring `cast_paren_drop_breaks_soundness`).

  Governing design: `.design/verified/thermite-semantics.md` REQ-2/REQ-6 (model what
  `thermite-tv`'s `ref_contract_pred` produces) + the (T1) obligation §"The concrete
  (T1) obligation" (the content lives in the binop/coercion re-statement, not a
  vacuous `X=X`) + Architecture §"S_C" (the `cast → nat/int` rule + the #122 paren).

  What this models. `thermite-tv/src/ref_encode.rs` does not produce a `Prop`; it
  produces a Verus predicate string. Two structural facts of that encoder are
  modelled here:

    1. The operator map `binop_str` (`ref_encode.rs::binop_str`):
         comparisons  Eq→"==", Ne→"!=", Lt→"<", Le→"<=", Gt→">", Ge→">="
         logical      And→"&&", Or→"||"          (+ Not→"!", encode_unary)
         arithmetic   Add→"+", Sub→"-", Mul→"*", Div→"/", Rem→"%",
                      Shl→"<<", Shr→">>", BitAnd→"&", BitOr→"|", BitXor→"^"
       modelled as Lean operator-token data (`VerusCmpTok`/`VerusLogTok`/
       `VerusArithTok`), independent of `Denote`'s source operators.

    2. The cast + its parenthesization (`ref_encode.rs::encode_cast`/`cast_target`):
       `encode_cast` emits `({inner}) as {target}`, wrapping the inner expression
       in parens unconditionally (the #122 paren discipline: a bare path/literal is
       unaffected, a compound inner is bound correctly). `cast_target` maps the
       target type: `U32→"u32"`, `U64→"u64"`, `Usize→"usize"`, `Named "nat"→"nat"`,
       `Named "int"→"int"`. We model the cast by interpreting the whole inner term,
       then applying the target coercion. That is faithful because the
       encoder parenthesizes the inner, so the cast binds the whole subexpression,
       never re-associating across a lower-precedence inner operator. The negative
       lemma below shows a paren-dropped encoder would not be sound — the #122/#146
       teeth.

  To keep the soundness theorem non-vacuous, this module follows the encoder's
  structure (the operator/cast-target maps + the parenthesization) rather than the
  source meaning; the shared value functions `arithDenote`/`castDenote` (`Denote.lean`)
  are the meaning the emitted token/coercion is interpreted at, as
  `tokRel`/`tokConn` interpret the emitted comparison/logical tokens.
-/
import Thermite.Ast
import Thermite.Denote

namespace Thermite

/-- A Verus comparison-operator token — the thing `binop_str` emits into the
    predicate string (`"=="`, `"<="`, …), modelled as a Lean datum so the encoder's
    operator choice is an explicit, independent step. -/
inductive VerusCmpTok where
  | eqTok   -- "=="
  | neTok   -- "!="
  | ltTok   -- "<"
  | leTok   -- "<="
  | gtTok   -- ">"
  | geTok   -- ">="
  deriving DecidableEq, Repr

/-- A Verus logical-connective token (`"&&"`, `"||"`). -/
inductive VerusLogTok where
  | andTok  -- "&&"
  | orTok   -- "||"
  deriving DecidableEq, Repr

/-- A Verus arithmetic-operator token — the thing `binop_str`'s arithmetic arms emit
    (`"+"`, `"-"`, `"*"`, `"/"`, `"%"`, `"<<"`, `">>"`, `"&"`, `"|"`, `"^"`),
    modelled as a Lean datum so the encoder's arithmetic operator choice is an
    explicit, independent step (the faithfulness decision point for #176). -/
inductive VerusArithTok where
  | plusTok    -- "+"
  | minusTok   -- "-"
  | starTok    -- "*"
  | slashTok   -- "/"
  | percentTok -- "%"
  | shlTok     -- "<<"
  | shrTok     -- ">>"
  | ampTok     -- "&"
  | pipeTok    -- "|"
  | caretTok   -- "^"
  deriving DecidableEq, Repr

/-- A Verus cast-target token — the spelling `cast_target` emits (`"u64"`, `"u32"`,
    `"usize"`, `"nat"`, `"int"`), modelled as a Lean datum so the encoder's
    cast-target choice is an explicit, independent step (the faithfulness decision
    point for #177). -/
inductive VerusCastTok where
  | u64Tok
  | u32Tok
  | usizeTok
  | natTok
  | intTok
  deriving DecidableEq, Repr

/-- The encoder's comparison-operator map — mirrors `ref_encode.rs::binop_str` for
    the comparison ops. The faithfulness decision point: an infidelity (`Eq→leTok`,
    the `==`-vs-`<=` bug) would live here. -/
def encOp : CmpOp → VerusCmpTok
  | CmpOp.eq => VerusCmpTok.eqTok
  | CmpOp.ne => VerusCmpTok.neTok
  | CmpOp.lt => VerusCmpTok.ltTok
  | CmpOp.le => VerusCmpTok.leTok
  | CmpOp.gt => VerusCmpTok.gtTok
  | CmpOp.ge => VerusCmpTok.geTok

/-- The encoder's logical-connective map (`And→"&&"`, `Or→"||"`). -/
def encLog : LogOp → VerusLogTok
  | LogOp.and => VerusLogTok.andTok
  | LogOp.or  => VerusLogTok.orTok

/-- The encoder's arithmetic-operator map — mirrors `ref_encode.rs::binop_str`'s
    arithmetic arms (`Add→"+"`, `Sub→"-"`, `Mul→"*"`, `Div→"/"`, `Rem→"%"`,
    `Shl→"<<"`, `Shr→">>"`, `BitAnd→"&"`, `BitOr→"|"`, `BitXor→"^"`). The #176
    faithfulness decision point: an infidelity (e.g. `Add→minusTok`) would live here. -/
def encArith : ArithOp → VerusArithTok
  | ArithOp.add    => VerusArithTok.plusTok
  | ArithOp.sub    => VerusArithTok.minusTok
  | ArithOp.mul    => VerusArithTok.starTok
  | ArithOp.div    => VerusArithTok.slashTok
  | ArithOp.rem    => VerusArithTok.percentTok
  | ArithOp.shl    => VerusArithTok.shlTok
  | ArithOp.shr    => VerusArithTok.shrTok
  | ArithOp.bitAnd => VerusArithTok.ampTok
  | ArithOp.bitOr  => VerusArithTok.pipeTok
  | ArithOp.bitXor => VerusArithTok.caretTok

/-- The encoder's cast-target map — mirrors `ref_encode.rs::cast_target`
    (`U64→"u64"`, `U32→"u32"`, `Usize→"usize"`, `Named "nat"→"nat"`,
    `Named "int"→"int"`). The #177 faithfulness decision point: an infidelity
    (e.g. `nat→intTok`) would live here. -/
def encCast : CastTy → VerusCastTok
  | CastTy.u64   => VerusCastTok.u64Tok
  | CastTy.u32   => VerusCastTok.u32Tok
  | CastTy.usize => VerusCastTok.usizeTok
  | CastTy.nat   => VerusCastTok.natTok
  | CastTy.int   => VerusCastTok.intTok

/-- The standard-model interpretation of a Verus comparison token over two integers
    — the meaning `⟦·⟧` of the emitted operator string. -/
def tokRel : VerusCmpTok → Int → Int → Prop
  | VerusCmpTok.eqTok, x, y => x = y
  | VerusCmpTok.neTok, x, y => x ≠ y
  | VerusCmpTok.ltTok, x, y => x < y
  | VerusCmpTok.leTok, x, y => x ≤ y
  | VerusCmpTok.gtTok, x, y => x > y
  | VerusCmpTok.geTok, x, y => x ≥ y

/-- The standard-model interpretation of a Verus connective token. -/
def tokConn : VerusLogTok → Prop → Prop → Prop
  | VerusLogTok.andTok, p, q => p ∧ q
  | VerusLogTok.orTok,  p, q => p ∨ q

/-- The standard-model interpretation of a Verus arithmetic token over two integer
    operand values — the meaning `⟦·⟧` of the emitted operator string. Routes
    through the shared `arithDenote` (`Denote.lean`): the token determines which
    `ArithOp` meaning, so the encoder's faithfulness is `tokArith (encArith op) = the
    op's arithmetic`, the round-trip the soundness theorem discharges (mirroring
    `tokRel (encOp op)`). -/
def tokArith : VerusArithTok → Int → Int → Int
  | VerusArithTok.plusTok,    x, y => arithDenote ArithOp.add x y
  | VerusArithTok.minusTok,   x, y => arithDenote ArithOp.sub x y
  | VerusArithTok.starTok,    x, y => arithDenote ArithOp.mul x y
  | VerusArithTok.slashTok,   x, y => arithDenote ArithOp.div x y
  | VerusArithTok.percentTok, x, y => arithDenote ArithOp.rem x y
  | VerusArithTok.shlTok,     x, y => arithDenote ArithOp.shl x y
  | VerusArithTok.shrTok,     x, y => arithDenote ArithOp.shr x y
  | VerusArithTok.ampTok,     x, y => arithDenote ArithOp.bitAnd x y
  | VerusArithTok.pipeTok,    x, y => arithDenote ArithOp.bitOr x y
  | VerusArithTok.caretTok,   x, y => arithDenote ArithOp.bitXor x y

/-- The standard-model interpretation of a Verus cast-target token over an integer
    operand value — the meaning `⟦·⟧` of the emitted `as <target>`. Routes through
    the shared `castDenote` (`Denote.lean`). -/
def tokCast : VerusCastTok → Int → Int
  | VerusCastTok.u64Tok,   v => castDenote CastTy.u64 v
  | VerusCastTok.u32Tok,   v => castDenote CastTy.u32 v
  | VerusCastTok.usizeTok, v => castDenote CastTy.usize v
  | VerusCastTok.natTok,   v => castDenote CastTy.nat v
  | VerusCastTok.intTok,   v => castDenote CastTy.int v

/-- A Verus byte-view spec-fn token — the thing `encode_string_byteview` dispatches a
    `String`-receiver method to (`.spec_byte_at(i)` for the i-th byte, `.spec_len()` for
    the length), modelled as a Lean datum so the encoder's byte-view dispatch choice is
    an explicit, independent step. The #127 faithfulness decision point: a misdispatch
    (the name-collision bug — `byte_at` routed to the wrong spec fn / index) lives here.
    `specByteAt` reads the i-th byte; `specLen` reads the length (a `slice`/`Seq`
    receiver's `.len()`→`recv.len()` is the same length meaning). -/
inductive VerusByteView where
  | specByteAt  -- "spec_byte_at(i)" — the i-th byte (Seq index over the byte view)
  | specLen     -- "spec_len()" / ".len()" — the length
  deriving DecidableEq, Repr

/-- The encoder's byte-view dispatch for the `byte_at` method — mirrors
    `encode_string_byteview`'s `"byte_at" => …spec_byte_at(idx)` arm. The #127
    faithfulness decision point: routes `byte_at` to the i-th-byte spec fn. -/
def encByteAt : VerusByteView := VerusByteView.specByteAt

/-- The encoder's byte-view dispatch for the `len` method — mirrors
    `encode_string_byteview`'s `"len" => …spec_len()` arm (and `encode_method_call`'s
    `"len"` slice arm). Routes `len` to the length spec fn. -/
def encLen : VerusByteView := VerusByteView.specLen

/-- The standard-model interpretation of a Verus byte-view spec fn over a sequence and
    an index — the meaning `⟦·⟧` of the emitted `.spec_byte_at(i)` / `.spec_len()`.
    `specByteAt` reads the i-th byte (`seqIdx`, the shared total access); `specLen`
    reads the length (the index argument is ignored). Routes through the shared
    `seqIdx`/`List.length` (`Denote.lean`); the dispatch token determines which read,
    so the encoder's #127 faithfulness is `byteView encByteAt = the byte`/`byteView
    encLen = the length`, the property the soundness theorem discharges. -/
def byteView : VerusByteView → List Int → Int → Int
  | VerusByteView.specByteAt, s, i => seqIdx s i
  | VerusByteView.specLen,    s, _ => (s.length : Int)

/- The integer-operand meaning the encoder assigns to a term (`refIntVal`) + the
   sequence-valued meaning (`refSeqVal`), mutual (#178: `idx`/`subrange` cross sorts).
   `ref_encode.rs` emits `value.to_string()` for an `IntLit`, the joined path for a
   `var`, the parenthesized `({l} {op} {r})` for an arithmetic binary
   (`encode_binary`'s non-comparison arm), `({inner}) as {target}` for a cast
   (`encode_cast`), the indexed `recv[idx]` for `xs[i]` (`encode_index`'s `Single`
   arm), and the byte-view spec fns for the `String` method calls
   (`encode_string_byteview`). Structured as encode-then-interpret on the operator /
   cast-target / byte-view map, following the encoder:

   - `arith`: `tokArith (encArith op)` of the two re-encoded operands. The encoder
     wholly parenthesizes a binary (`encode_binary`: `format!("({l} {} {r})")`), so
     the operands are bound correctly and the operator map is the only content.
   - `cast`: `tokCast (encCast ty)` of the re-encoded whole inner. The encoder
     wraps the inner in parens (`encode_cast`: `format!("({e}) as {target}")`), so
     the cast binds the entire `refIntVal inner` (no re-association). The #122
     discipline: dropping that paren would make a compound inner re-parse and the
     cast bind only a sub-term (the negative lemma).
   - `idx`: `byteView encByteAt (refSeqVal base) (refIntVal i)` — `encode_index` emits
     `recv[idx]` over the receiver's `@`-view (the index `as int`); the meaning is the
     i-th element of the view (`refSeqVal` is the same sequence, the `@`-view being the
     identity). #178.
   - `seqLen`: `byteView encLen (refSeqVal base) 0` — the encoder dispatches
     `len`→`spec_len()` / `.len()`; the meaning is the length of the view. #178.
   - `byteAt`: `byteView encByteAt (refSeqVal base) (refIntVal i)` — the encoder
     dispatches `byte_at`→`spec_byte_at(i)`; the i-th byte. The #127 class: the
     dispatch choice is the content (the negative lemma mis-dispatches). -/
mutual
/-- The integer-operand meaning the encoder assigns to a term, fuel-indexed (#181). See the block
    comment. The #181 `specCall` arm: an integer-returning spec-fn call `name(args)` is lowered to
    the Verus call `name(<encoded args>)` (`encode_call`'s case (3), not inlined), modelled by
    resolving `name` in the same `Env.specs` registry, binding params to the encoder-denoted args
    (`refIntValArgs`), and denoting the same body at the consumed fuel. At fuel `0` / an unresolved
    name it bottoms to `0`, identical to `intVal`'s bottom, so T1 holds at fuel `0`. -/
noncomputable def refIntVal : Nat → Expr → Env → Int
  | fuel, Expr.arith op a b,  env => tokArith (encArith op) (refIntVal fuel a env) (refIntVal fuel b env)
  | fuel, Expr.cast inner ty, env => tokCast (encCast ty) (refIntVal fuel inner env)
  | fuel, Expr.idx base i,    env => byteView encByteAt (refSeqVal fuel base env) (refIntVal fuel i env)
  | fuel, Expr.seqLen base,   env => byteView encLen (refSeqVal fuel base env) 0
  | fuel, Expr.byteAt base i, env => byteView encByteAt (refSeqVal fuel base env) (refIntVal fuel i env)
  | fuel+1, Expr.specCall name args, env =>
      match env.specs name with
      | some fn => refIntVal fuel fn.body (env.bindParams fn.params (refIntValArgs (fuel+1) args env))
      | none    => 0
  -- The #182 `count_where` value-combinator (`encode_combinator_call` — emits `count_where(s@, |x:u32| body)`
  -- referencing the registry `verus_l3` recursive count, not re-implemented). Read on the integer side
  -- (`refIntVal`). The encoder reuses the shared `countWhereVal` (the frozen `verus_l3` body) over the
  -- slice `@`-view (`refSeqVal`, `encode_combinator_arg`'s `ArgKind::Slice`) and the predicate closure
  -- body re-encoded by the same `refDenote` recursion (`ArgKind::Pred` via `encode_pred_arg`), applied
  -- at the element via the shared `Env.bindInt`. Structurally identical to the source `intVal` arm; the
  -- soundness content is the slice + per-element predicate agreement (`refSeqVal_eq_seqVal` + the
  -- recursive `ref_sound` IH on the flat closure body).
  | fuel, Expr.comb CombName.countWhere seq _ _ pred, env =>
      let s := refSeqVal fuel seq env
      let p : Int → Prop := fun x =>
        match pred with
        | some (Pred.mk bound body) => refDenote fuel body (env.bindInt bound x)
        | none => True
      countWhereVal p s
  | _,    Expr.intLit n,      _   => n
  | _,    Expr.var x,         env => env.ints x
  | _, _, _ => 0
  termination_by fuel e _ => (fuel, sizeOf e)

/-- The encoder's sequence-valued meaning (#178), fuel-indexed (#181): a slice var `xs`→`xs@` is the
    identity on the sequence value (`encode_slice_arg`); a `String` byte receiver is emitted bare and
    its bytes are the same sequence; a `subrange` is the encoder's `.subrange(lo, hi)`
    (`encode_index`'s range arms), `seqSub` over the re-encoded base. Routes through the same
    `seqSub` as the source. Mutual with `refIntVal` (`subrange`'s bounds are integer terms). -/
noncomputable def refSeqVal : Nat → Expr → Env → List Int
  | _,    Expr.seqVar x, env => env.seqs x
  | _,    Expr.strVar x, env => env.seqs x
  | fuel, Expr.subrange base r, env =>
      let s := refSeqVal fuel base env
      match r with
      | RangeArg.rangeTo hi    => seqSub s 0 (refIntVal fuel hi env)
      | RangeArg.range lo hi   => seqSub s (refIntVal fuel lo env) (refIntVal fuel hi env)
      | RangeArg.rangeFrom lo  => seqSub s (refIntVal fuel lo env) (s.length : Int)
  | _, _, _ => []
  termination_by fuel e _ => (fuel, sizeOf e)

/-- The encoder-denoted arg values of a `specCall` (#181): each arg's `refIntVal` at the same fuel
    (the per-arg `encode_call_arg`). Structurally identical to the source `intValArgs`; the soundness
    content is that each arg agrees (the `refVal_eq` IH). Mutual with `refIntVal`. -/
noncomputable def refIntValArgs : Nat → List Expr → Env → List Int
  | _,    [],        _   => []
  | fuel, a :: rest, env => refIntVal fuel a env :: refIntValArgs fuel rest env
  termination_by fuel args _ => (fuel, sizeOf args)

/-- `⟦ ref_contract_pred(P) ⟧` — the meaning, under the standard model, of the Verus
    predicate string the reference encoder produces, fuel-indexed (#181). Structured as
    encode-then-interpret (`tokRel (encOp op) …`), following the encoder, so the
    equality with `denote` is a real theorem. The #181 `specCall` arm: a boolean-returning spec-fn
    call is lowered to the Verus call `name(<encoded args>)` (not inlined), modelled by resolving
    `name` in the same `Env.specs`, binding params to the encoder-denoted args, and denoting the
    same body at the consumed fuel. Mutual with `refDenoteArms` (#180) and the fuel-indexed
    `refIntVal`/`refIntValArgs` (#181). -/
noncomputable def refDenote : Nat → Expr → Env → Prop
  | _,    Expr.boolLit b, _   => (b = true)
  | fuel, Expr.cmp op a b, env =>
      tokRel (encOp op) (refIntVal fuel a env) (refIntVal fuel b env)
  | fuel, Expr.logic op a b, env =>
      tokConn (encLog op) (refDenote fuel a env) (refDenote fuel b env)
  | fuel, Expr.neg e, env => ¬ refDenote fuel e env
  -- The bool-var (#253, the §4.1.2 spine prerequisite). The encoder emits the bare Verus bool
  -- path read; its meaning is the env `bools` value — the identical arm to `denote`, so T1
  -- re-establishes by `Iff.rfl` at this node.
  | _,    Expr.boolVar x, env => (env.bools x = true)
  -- The match-in-ens form (#180; `encode_match`). The encoder emits a Verus `match` expression
  -- `match {scrut} { {pat} => {body}, … }` whose arm-selection-by-variant is the Verus `match`
  -- meaning (the shared `scrutVal`/arm-walk, not re-implemented — `encode_match` reuses Verus's
  -- match). What `encode_match` re-encodes (the faithfulness surface) is: the scrutinee (the same
  -- recursion — `scrutVal` reads the free name, shared), each arm's pattern (`encode_pattern`'s
  -- built-in `Some(x)`/`None`/`Ok(x)`/`Err(e)` — the variant + payload-binder choice), and each
  -- arm's body (the same independent `refDenote` recursion, so a swapped/corrupted arm body is
  -- caught). The pattern-bound payload var is in scope in the body as production binds it.
  | fuel, Expr.match_ scrut arms, env =>
      refDenoteArms fuel (scrutVal scrut env) arms env
  -- The `is`-test (#180; `ref_encode.rs`'s `Expr::Is` arm `({s} is {variant})`). The encoder emits
  -- the Verus `(scrut is Variant)` discriminant test; its meaning is the shared `isVariant` (the
  -- Verus `is` semantics, not re-implemented). The faithfulness content is the variant choice: a
  -- wrong variant (`is Some` emitted as `is None`) is a different meaning (the negative lemma).
  | _,    Expr.is_ scrut variant, env =>
      ((scrutVal scrut env).isVariant variant = true)
  -- The 6 bounded-quantifier combinators (#179). The encoder reuses the shared frozen
  -- `lookup(C).verus_l3` quantifier body verbatim (`encode_combinator_call` emits
  -- `name(args)` to the registry `spec fn` — the body is the shared ground truth, not
  -- re-implemented), so the quantifier form here is structurally identical to `denote`'s.
  -- What the encoder re-implements (the faithfulness surface, `encode_combinator_arg`) is
  -- the per-arg-kind threading:
  --   - `ArgKind::Slice` → `encode_slice_arg`'s `@`-view = `refSeqVal` (the identity on
  --     the sequence value).
  --   - `ArgKind::Index` → `encode_index_value`'s scalar `<n> as int` = `refIntVal` of a
  --     scalar, never the slice `@`-view. The #145 fix (`forall_below`/`forall_from`'s
  --     `n: int`; `n@` would be a Verus type error / a wrong meaning — the negative lemma).
  --   - `ArgKind::Pred` → `encode_pred_arg`'s `|x: u32| <body>`, the body re-encoded by the
  --     same independent `refDenote`/`refIntVal` recursion (so a predicate infidelity is
  --     caught), applied at the i-th element via the shared `Env.bindInt`.
  | fuel, Expr.comb c seq seq2 idx pred, env =>
      let s := refSeqVal fuel seq env
      let s2 := match seq2 with | some e => refSeqVal fuel e env | none => []
      let n := match idx with | some e => refIntVal fuel e env | none => 0
      let p : Int → Prop := fun i =>
        match pred with
        | some (Pred.mk bound body) => refDenote fuel body (env.bindInt bound (seqIdx s i))
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
      -- The #182 `permutation_of(a, b)` (`encode_combinator_call` — emits `permutation_of(a@, b@)`
      -- referencing the registry `verus_l3` `a.to_multiset() == b.to_multiset()`, not re-implemented).
      -- The encoder reuses the shared `permEq` (the count-characterization of multiset equality) over
      -- the two slice `@`-views (`refSeqVal`, `encode_combinator_arg`'s `ArgKind::Slice` for both args).
      -- Structurally identical to the source `denote` arm; the content is the two slices agreeing.
      | CombName.permutationOf => permEq s s2
      -- `count_where` is value-sorted — read on the `refIntVal` side; here vacuously `True`.
      | CombName.countWhere => True
  -- The #181 spec-fn call as a top-level predicate (`encode_call`'s case (3) — the Verus call,
  -- not inlined): resolve `name` in the same `Env.specs`, bind params to the encoder-denoted args,
  -- denote the same body at the consumed fuel. At fuel `0` / an unresolved name → `True`, identical
  -- to `denote`'s bottom (T1 holds at fuel `0`). The call-site soundness is the generic theorem:
  -- the args agree (the `refVal_eq` IH) + the same registry resolves the same body (`ref_sound` IH).
  | fuel+1, Expr.specCall name args, env =>
      match env.specs name with
      | some fn => refDenote fuel fn.body (env.bindParams fn.params (refIntValArgs (fuel+1) args env))
      | none    => True
  | _, _, _ => True
  termination_by fuel e _ => (fuel, sizeOf e)

/-- The encoder's `match`-arm selection + payload binding (#180; `encode_match`). Structurally
    identical to `denoteArms` (the arm selection by variant is the Verus `match` meaning, which
    `encode_match` reuses verbatim); the only difference is that each arm body is denoted by the
    encoder's `refDenote` (so a body infidelity is caught). The pattern's variant + payload-binder
    are `encode_pattern`'s built-in `Some(x)`/`None`/`Ok(x)`/`Err(e)` choice, modelled as the
    arm's `Variant` + binder. The `match_` case of `ref_sound` proves this equals `denoteArms`
    (by the recursive IH on each body). -/
noncomputable def refDenoteArms : Nat → OptResVal → List MatchArm → Env → Prop
  | _,    _,     [], _ => True
  | fuel, scrut, MatchArm.mk variant binder body :: rest, env =>
      if scrut.variant = variant then
        match binder with
        | some x => refDenote fuel body (env.bindInt x scrut.payload)
        | none   => refDenote fuel body env
      else
        refDenoteArms fuel scrut rest env
  termination_by fuel _ arms _ => (fuel, sizeOf arms)
end

end Thermite
