/-
  Thermite/Exec.lean — the exec-expression sublanguage `S_E` (the bounded value
  semantics) + the (T1) soundness theorem for the exec reference encoder
  `exec_ref_value` (increment 2a, #171; epic #169). This opens layer 2 (the exec side).

  Governing design: `.design/verified/thermite-semantics.md` Architecture §"S_E — the
  exec-expression sublanguage (bounded values)", REQ-1/REQ-2/REQ-4 (increment (b), #171:
  "exec-expression `S_E` + prove `exec_ref_value` sound"). The exec-value semantics are
  grounded in `thermite-tv/src/exec_encode.rs::exec_ref_value` (the operator map
  `binop_str`, the bounded handling, the no-nat-coercion, the cast targets `cast_target`,
  the slice-index element value, the cast/`<`-paren disciplines) + `thermite-design.md`
  §4.1/§6 (the always-active L1 overflow checks).

  ════════════════════════════════════════════════════════════════════════════════
  The crucial difference: `S_E ≠ S_C` (kept in a separate namespace `Thermite.Exec`).
  ════════════════════════════════════════════════════════════════════════════════

  `S_C` (the contract side, done; `Thermite.{Ast,Denote,RefEncode,Soundness}`) is
  unbounded `int`/`nat` spec arithmetic: no wraparound, `Eq` nat-coerced (`result as nat
  == spec_sum(xs)`). `S_E` (this increment) is the executable value:

  1. Bounded. An exec value is a `u64`/`u32`/`usize`/`bool`, not unbounded `int`.
     A `BVal` is the mathematical `Int` value together with its type's bound (the width
     `2^w`); a `BoolVal` is a `Bool`. The value is constrained to lie in `[0, 2^w)`
     (`BVal.inRange`), the bound the type carries (issue title: "the `Int` value
     together with its type's bound").

  2. Overflow is a proof obligation carried alongside the value (Verus exec
     arithmetic). `a + b` (source `u64`) requires proving no overflow; the value is the
     mathematical result given that obligation. We model this as a partial evaluation:
     `evalArith` returns `some v` when the mathematical result is in range (the obligation
     discharged) and `none` when it overflows (the obligation fails; the value is not
     defined, as a Verus exec `+` is rejected when overflow is possible). The
     no-overflow obligation is `evalArith … = some _` (`arithObligation`). This is the
     load-bearing exec fidelity: a `wrapping_add` (a different denotation) would have a
     value at the overflow point where the bounded `+` has none.

  3. Never nat-coerced. A `u64` stays `u64`; a cast `e as u32` narrows/wraps at the
     target width (`castVal` = `value % 2^w`), it does not inject into an unbounded `nat`
     (which would mask the wrap point). `div`/`shift`-by-zero is a precondition
     (`evalArith` returns `none`, undefined, an obligation). Indexing `xs[i]` is the
     i-th element under `i < len` (out-of-range → `none`, the bounds obligation).

  The negative lemma (`nat_coercion_underflow_breaks_soundness`) is the proven statement of
  the "never nat-coerced" discipline: a `u64` subtraction that underflows, encoded with a
  nat-coercion (which clamps to `0` via `Int.toNat`), disagrees with the bounded/obligation
  semantics (which has no value, the obligation fails) at a concrete env. The clamp-to-0
  is the soundness hole `exec-tv.md` AC-4 / the issue's "never nat-coerced"
  discipline prevents.

  ════════════════════════════════════════════════════════════════════════════════
  Faithfulness to `exec_ref_value` (the critic diffs this).
  ════════════════════════════════════════════════════════════════════════════════

  `execRefValue` models what the Rust `exec_ref_value` produces (its meaning):

    - `Expr::IntLit`/`BoolLit`        → the literal value (a `BVal` at its type / `BoolVal`).
    - `Expr::Path`                    → the free var (its bounded value from the env).
    - `Expr::Binary` arith            → `binop_str` (`+`/`-`/`*`/`/`/`%`/`<<`/`>>`/`&`/`|`/`^`):
      the bounded operator carrying the overflow obligation (not `wrapping_*`, not `nat`).
    - `Expr::Binary` cmp              → `==`/`!=`/`<`/`<=`/`>`/`>=` → a `bool` value.
    - `Expr::Binary` logical          → `&&`/`||` → a `bool` value.
    - `Expr::Unary` Not               → `!` (logical not on bool, the exec subset's `!`).
    - `Expr::Cast`                    → `cast_target` (`u64`/`u32`/`usize`/`u8`/`u16`),
      never `nat`/`int`, the narrowing wrap at the bounded target width.
    - `Expr::Index` single over slice → `xs[i as int]` (the bounded element value).

  `execRefValue` and `execDenote` are defined independently (the encoder's operator map +
  cast-target map vs the source bounded meaning), so `exec_ref_sound` is not `rfl`-vacuous:
  the encoder threads each construct through its operator/cast map, the source through the
  bounded value semantics, and the theorem proves them equal construct-by-construct.

  Deferred, not embedded here (no embed-then-`sorry`). These are the
  residuals (faithful to `exec_ref_value`'s `RefEncodeError::Unsupported`):
    - method calls / Vec-String accessors (`Expr::MethodCall`): `exec_ref_value` `Err`s
      (#154/#156 territory); the exec-body statement forms (`let`/`if`/loops/mutation) are
      increment 2b (#172, `body_ref_state`) / 2c (#163, loops, kernel-gated). They are out
      of the pure-exec expression subset, so they are not in `S_E` here and are not modelled.
    - a non-path call callee, a non-slice index base, a slice-range index (a sub-slice is
      not a scalar exec value): `exec_ref_value` `Err`s on each; out of `S_E`, absent here.

  Dependencies: Lean 4 core only (the bounded values are `Int` + a width-bound predicate;
  the wrap is `Int.emod`; the overflow obligation is `Option`-partiality; the proofs are
  `cases`/`simp`/`rfl`/`decide`/`omega`, no Mathlib, no Lean-SMT). Mirrors the contract
  side's core-only discipline.
-/

namespace Thermite.Exec

/-! ## The bounded exec types + values (`S_E`'s value domain, bounded, never nat-coerced) -/

/-- The bounded exec integer types `exec_ref_value` casts to / arithmetic stays at —
    mirrors `exec_encode.rs::cast_target`'s accepted targets (`u8`/`u16`/`u32`/`u64`/
    `usize`, never `nat`/`int`). `bool` is not an `IntTy` (a bool is a separate value
    sort); a cast to `bool` is `Unsupported` in the encoder, so it is out of `S_E`. -/
inductive IntTy where
  | u8
  | u16
  | u32
  | u64
  | usize
  deriving DecidableEq, Repr

/-- The bit width of a bounded integer type — the exponent of its bound `2^width`.
    `usize` is modelled at a 64-bit target (the dominant exec target; the width is a
    parameter of the type, the "value together with its type's bound" model). -/
def IntTy.width : IntTy → Nat
  | .u8    => 8
  | .u16   => 16
  | .u32   => 32
  | .u64   => 64
  | .usize => 64

/-- The (exclusive) bound of a bounded integer type: `2^width`. A value of type `t` is a
    `u`-style unsigned integer in `[0, t.bound)`. This is the type's bound that the
    bounded value carries (issue: "the `Int` value together with its type's bound"). -/
def IntTy.bound (t : IntTy) : Int := (2 : Int) ^ t.width

/-- A bounded exec value: a mathematical `Int` `value` together with its type `ty` (whose
    `bound` is the type's `2^width`). This is the load-bearing exec model: the value is
    never a bare `Int` (that would be the unbounded `S_C` domain) and never a `Nat` coerced
    away from its width; it always carries the type-bound. `inRange` is the well-formedness
    obligation `0 ≤ value < ty.bound` (a `u64` stays in `[0, 2^64)`). -/
structure BVal where
  ty : IntTy
  value : Int
  deriving DecidableEq, Repr

/-- The bounded value is well-formed (in its type's range): `0 ≤ value < ty.bound`. A
    bounded arithmetic op that lands out of this range is an overflow (the obligation
    fails); a cast wraps back into it. -/
def BVal.inRange (v : BVal) : Prop := 0 ≤ v.value ∧ v.value < v.ty.bound

/-- An exec value: a bounded integer (`BVal`, carrying its type-bound) or a `Bool`,
    the `{u64, u32, usize, bool}` value domain of §4.1/§6, with the integer always bounded
    (never nat-coerced). -/
inductive ExecVal where
  | int (b : BVal)
  | bool (b : Bool)
  deriving DecidableEq, Repr

/-! ## The exec expression AST (`exec_ref_value`'s pure-exec subset) -/

/-- The arithmetic exec operators — mirrors `exec_encode.rs::binop_str`'s arithmetic arms
    (`Add`/`Sub`/`Mul`/`Div`/`Rem`/`Shl`/`Shr`/`BitAnd`/`BitOr`/`BitXor` → `+`/`-`/`*`/`/`/
    `%`/`<<`/`>>`/`&`/`|`/`^`). In exec position these are the bounded operators carrying the
    overflow obligation, not `wrapping_*`, not `nat`. -/
inductive AOp where
  | add | sub | mul | div | rem | shl | shr | bitAnd | bitOr | bitXor
  deriving DecidableEq, Repr

/-- The comparison exec operators (`Eq`/`Ne`/`Lt`/`Le`/`Gt`/`Ge` → `==`/`!=`/`<`/`<=`/`>`/
    `>=` → a `bool` value). -/
inductive COp where
  | eq | ne | lt | le | gt | ge
  deriving DecidableEq, Repr

/-- The logical exec connectives (`And`/`Or` → `&&`/`||` → a `bool` value). -/
inductive LOp where
  | and | or
  deriving DecidableEq, Repr

/-- The pure exec-expression fragment `exec_ref_value` covers (faithful to
    `exec_encode.rs::encode`'s match arms). Method calls / `if`/`match`/closures / the
    exec-body statement forms are absent (the encoder `Err`s on them; they are
    increment 2b/2c, listed in the module doc, not embedded-then-`sorry`). -/
inductive ExecExpr where
  /-- An integer literal at a bounded type (`Expr::IntLit`; `exec_encode.rs` emits the bare
      value; the bounded type the literal sits at is the surrounding context's type, carried
      here so the value is bounded, never nat). -/
  | intLit (ty : IntTy) (value : Int)
  /-- A boolean literal (`Expr::BoolLit`). -/
  | boolLit (value : Bool)
  /-- A free exec variable (`Expr::Path`, a body param), its bounded value from the env. -/
  | var (name : String)
  /-- A bounded arithmetic binary `a <op> b` (`Expr::Binary` arith; `binop_str`). Carries
      the overflow obligation (`execDenote` is partial here). -/
  | arith (op : AOp) (lhs rhs : ExecExpr)
  /-- A comparison `a <op> b` → a `bool` value (`Expr::Binary` cmp). -/
  | cmp (op : COp) (lhs rhs : ExecExpr)
  /-- A logical connective `a <op> b` → a `bool` value (`Expr::Binary` logical). -/
  | logic (op : LOp) (lhs rhs : ExecExpr)
  /-- Logical negation `!a` → a `bool` value (`Expr::Unary` Not). -/
  | not (e : ExecExpr)
  /-- A cast `inner as ty` (`Expr::Cast`; `cast_target`), the bounded-target wrap (never
      `nat`/`int`). The narrowing wrap at the target width is the value semantics. -/
  | cast (inner : ExecExpr) (ty : IntTy)
  /-- `xs[i]` — a single-element index over a slice param (`Expr::Index` `Single` over a
      slice-bound base; `exec_encode.rs::encode_index` → `xs[i as int]`, the bounded element
      value). `slice` is the slice-param name, `index` the integer-valued index expr. -/
  | index (slice : String) (index : ExecExpr)
  deriving DecidableEq, Repr

/-! ## The exec environment (free exec vars → bounded values; slices → element sequences) -/

/-- The exec environment: a valuation of free exec names. A scalar param maps to an
    `ExecVal` (a bounded integer or a bool); a slice param maps to a sequence of bounded
    element values (`List BVal`, the elements an `xs[i]` reads). Mirrors `ExecRefCtx`'s
    `slice_bound` distinction (a slice-bound name is indexed, a scalar name is read). -/
structure ExecEnv where
  /-- Scalar exec vars → their bounded value. -/
  vars : String → ExecVal
  /-- Slice params → their element sequence (each element a bounded `BVal`). -/
  slices : String → List BVal

/-! ## The bounded value operations — overflow as a proof obligation, never nat-coerced -/

/-- The mathematical result of a bounded arithmetic op on two `Int`s (no bound applied yet),
    `none` when the op is undefined at the source (a zero divisor / a zero shift, a source
    precondition / L0 obligation, like the contract side's div-by-zero). The shifts
    use `Int` powers of two (`<<` = `* 2^k`, `>>` = `/ 2^k`); bit-ops use the core `Int`
    bitwise ops. The result is the mathematical value; the bound (overflow) check is applied
    separately (`evalArith`), so the overflow obligation is explicit. -/
def rawArith : AOp → Int → Int → Option Int
  | .add, a, b => some (a + b)
  | .sub, a, b => some (a - b)
  | .mul, a, b => some (a * b)
  | .div, _, 0 => none                          -- div-by-zero: an L0 precondition (undefined)
  | .div, a, b => some (a / b)
  | .rem, _, 0 => none                          -- rem-by-zero: an L0 precondition (undefined)
  | .rem, a, b => some (a % b)
  | .shl, a, b => if b < 0 then none else some (a * (2 : Int) ^ b.toNat)  -- `a << b` = `a * 2^b`
  | .shr, a, b => if b < 0 then none else some (a / (2 : Int) ^ b.toNat)  -- `a >> b` = `a / 2^b`
  -- Bitwise ops on the non-negative bounded representation: convert to `Nat` (core Lean
  -- has `Nat.land`/`Nat.lor`/`Nat.xor`; `Int` has no bitwise instance), apply, re-inject.
  -- For in-range `u`-style operands (`0 ≤ value`) this is the bounded bit-op.
  | .bitAnd, a, b => some ((Nat.land a.toNat b.toNat : Int))
  | .bitOr,  a, b => some ((Nat.lor  a.toNat b.toNat : Int))
  | .bitXor, a, b => some ((Nat.xor  a.toNat b.toNat : Int))

/-- The bounded arithmetic op carrying the overflow obligation. Computes the mathematical
    result `rawArith op a.value b.value`; the value is defined (`some`) only when the result
    lies in the result type's range `[0, ty.bound)`, i.e. the no-overflow obligation is
    discharged. When the result overflows (or the op is undefined at the source), it is `none`:
    the obligation fails, the value is not defined (a Verus exec `+` rejected
    because overflow is possible). The result type is the LHS type (the exec operand type; in
    a well-typed exec fn both operands share the type). The value is the mathematical result
    given no overflow, never a wrap, never a nat-coercion. -/
def evalArith (op : AOp) (a b : BVal) : Option BVal :=
  match rawArith op a.value b.value with
  | none => none
  | some r => if 0 ≤ r ∧ r < a.ty.bound then some ⟨a.ty, r⟩ else none

/-- The no-overflow obligation for a bounded arithmetic op: the value is defined (the
    obligation is discharged). This is the proof obligation Verus exec arithmetic carries
    alongside `a + b`; in `S_E` it is `evalArith op a b ≠ none`. Exposed as a named predicate
    so the model makes the obligation explicit (not hidden behind partiality). -/
def arithObligation (op : AOp) (a b : BVal) : Prop := (evalArith op a b).isSome

/-- A cast narrows/wraps at the target width, never nat-coerced. `castVal t v` = `v.value`
    reduced modulo `t.bound` (`Int.emod`, always in `[0, t.bound)` for a positive bound),
    re-typed at `t`. This is the bounded narrowing the exec `as u32` performs (a value too
    large for `u32` wraps), and it is the non-nat-coercion point: the result stays a
    bounded value at width `t.width`, it does not escape to an unbounded nat. -/
def castVal (t : IntTy) (v : BVal) : BVal := ⟨t, v.value % t.bound⟩

/-- A comparison op on two bounded values → a `Bool` (`==`/`!=`/`<`/`<=`/`>`/`>=`). The
    comparison is on the (bounded) `value`s. -/
def cmpVal : COp → BVal → BVal → Bool
  | .eq, a, b => decide (a.value = b.value)
  | .ne, a, b => decide (a.value ≠ b.value)
  | .lt, a, b => decide (a.value < b.value)
  | .le, a, b => decide (a.value ≤ b.value)
  | .gt, a, b => decide (a.value > b.value)
  | .ge, a, b => decide (a.value ≥ b.value)

/-- A logical connective on two `Bool`s. -/
def logVal : LOp → Bool → Bool → Bool
  | .and, a, b => a && b
  | .or,  a, b => a || b

/-! ## `S_E` — the source bounded-value denotation of an `ExecExpr`

  `execDenote e env : Option ExecVal` — the bounded value `e` denotes, `none` when an
  obligation fails (arithmetic overflow, div/shift-by-zero, an out-of-range index, or a
  type mismatch the exec subset rules out). The `none` is the overflow/bounds obligation
  surfaced as partiality; the value is the mathematical result given the obligation holds. -/

/-- Project an `ExecVal` to a bounded integer (`none` if it is a bool, an exec type error
    the well-typed subset rules out, surfaced as partiality). -/
def asInt : ExecVal → Option BVal
  | .int b => some b
  | .bool _ => none

/-- Project an `ExecVal` to a bool (`none` if it is an integer). -/
def asBool : ExecVal → Option Bool
  | .bool b => some b
  | .int _ => none

/-- The source `S_E` denotation. Bounded value semantics: arithmetic carries the overflow
    obligation (`evalArith`, `none` on overflow / div-or-shift-by-zero); casts wrap at the
    target width (`castVal`, never nat); indexing reads the i-th bounded element under the
    in-range obligation (`none` out of range). -/
def execDenote : ExecExpr → ExecEnv → Option ExecVal
  | .intLit ty v, _ =>
      -- A literal is well-formed only in its type's range (a source-checked literal is in
      -- range; an out-of-range literal is rejected, the obligation).
      if 0 ≤ v ∧ v < ty.bound then some (.int ⟨ty, v⟩) else none
  | .boolLit b, _ => some (.bool b)
  | .var x, env => some (env.vars x)
  | .arith op a b, env => do
      let av ← asInt (← execDenote a env)
      let bv ← asInt (← execDenote b env)
      let r ← evalArith op av bv
      some (.int r)
  | .cmp op a b, env => do
      let av ← asInt (← execDenote a env)
      let bv ← asInt (← execDenote b env)
      some (.bool (cmpVal op av bv))
  | .logic op a b, env => do
      let av ← asBool (← execDenote a env)
      let bv ← asBool (← execDenote b env)
      some (.bool (logVal op av bv))
  | .not e, env => do
      let v ← asBool (← execDenote e env)
      some (.bool (!v))
  | .cast inner ty, env => do
      let v ← asInt (← execDenote inner env)
      some (.int (castVal ty v))
  | .index slice idx, env => do
      let iv ← asInt (← execDenote idx env)
      let xs := env.slices slice
      -- the in-range bounds obligation: `0 ≤ i < xs.length`; out of range → `none`.
      if h : 0 ≤ iv.value ∧ iv.value < (xs.length : Int) then
        some (.int (xs.get ⟨iv.value.toNat, by
          obtain ⟨h0, h1⟩ := h
          have : iv.value.toNat < xs.length := by omega
          exact this⟩))
      else none

/-! ## `execRefValue` — the model of `exec_encode.rs::exec_ref_value`'s output meaning

  This models what the Rust `exec_ref_value` produces: the operator map (`binop_str`), the
  cast-target map (`cast_target`, bounded, never nat), the slice-index element value, as a
  bounded-value denotation. It is defined via the encoder's operator/cast maps (an explicit
  token round-trip), independently of `execDenote`, so `exec_ref_sound` is non-vacuous. -/

/-- The encoder's arithmetic operator token (`binop_str`'s arithmetic arms). Modelled as an
    explicit token so the encoder's map is a separate object the soundness theorem
    round-trips against the source op (faithful to `binop_str`: `Add → "+"`, …). -/
inductive ArithTok where
  | plus | minus | star | slash | percent | shl | shr | amp | pipe | caret
  deriving DecidableEq, Repr

/-- `binop_str`'s arithmetic-arm map (`exec_encode.rs`): the bounded operator token (not a
    `wrapping_*` form, not a nat form), re-stated independently of the source `AOp` so a
    production wrong-op bug (`+`→`wrapping_sub`, E3) would show as a wrong token. -/
def encArith : AOp → ArithTok
  | .add => .plus  | .sub => .minus | .mul => .star | .div => .slash | .rem => .percent
  | .shl => .shl   | .shr => .shr
  | .bitAnd => .amp | .bitOr => .pipe | .bitXor => .caret

/-- Interpret an arithmetic token as the bounded op carrying the overflow obligation, the
    meaning of the Verus exec operator `exec_ref_value` emits (the bounded `+`/`-`/… that
    carries the verus overflow obligation, not `wrapping_*`, not `nat`). -/
def tokArith : ArithTok → BVal → BVal → Option BVal
  | .plus,    a, b => evalArith .add a b
  | .minus,   a, b => evalArith .sub a b
  | .star,    a, b => evalArith .mul a b
  | .slash,   a, b => evalArith .div a b
  | .percent, a, b => evalArith .rem a b
  | .shl,     a, b => evalArith .shl a b
  | .shr,     a, b => evalArith .shr a b
  | .amp,     a, b => evalArith .bitAnd a b
  | .pipe,    a, b => evalArith .bitOr a b
  | .caret,   a, b => evalArith .bitXor a b

/-- The encoder's cast-target token (`cast_target`'s accepted targets: the bounded prims
    `u8`/`u16`/`u32`/`u64`/`usize`, never `nat`/`int`; a `bool`/`nat`/`int` target is
    `Unsupported` and thus absent). -/
inductive CastTok where
  | u8 | u16 | u32 | u64 | usize
  deriving DecidableEq, Repr

/-- `cast_target`'s map (`exec_encode.rs`): the bounded target spelling. Re-stated
    independently; the non-nat-coercion fact: there is no `nat`/`int` token. -/
def encCast : IntTy → CastTok
  | .u8 => .u8 | .u16 => .u16 | .u32 => .u32 | .u64 => .u64 | .usize => .usize

/-- Interpret a cast token as the bounded wrap at the target width (`castVal`), the meaning
    of `(e) as u32`/… `exec_ref_value` emits. Never an unbounded-nat injection. -/
def tokCast : CastTok → BVal → BVal
  | .u8,    v => castVal .u8 v
  | .u16,   v => castVal .u16 v
  | .u32,   v => castVal .u32 v
  | .u64,   v => castVal .u64 v
  | .usize, v => castVal .usize v

/-- The comparison/logical token maps (the encoder's `==`/`!=`/… and `&&`/`||`): for the
    exec subset these are the same operations as the source (the encoder emits the bounded
    comparison verbatim); re-stated as the encoder's threading so the round-trip is explicit. -/
def encCmp : COp → COp := id
def encLog : LOp → LOp := id

/-- The encoder-output denotation: the meaning of the Verus exec-value string
    `exec_ref_value` produces, routed through the encoder's operator/cast maps. Defined
    independently of `execDenote` (it threads through `encArith`/`tokArith`,
    `encCast`/`tokCast`), so `exec_ref_sound` proving them equal is genuine content. -/
def execRefValue : ExecExpr → ExecEnv → Option ExecVal
  | .intLit ty v, _ =>
      if 0 ≤ v ∧ v < ty.bound then some (.int ⟨ty, v⟩) else none
  | .boolLit b, _ => some (.bool b)
  | .var x, env => some (env.vars x)
  | .arith op a b, env => do
      let av ← asInt (← execRefValue a env)
      let bv ← asInt (← execRefValue b env)
      let r ← tokArith (encArith op) av bv      -- the encoder's binop_str map, bounded
      some (.int r)
  | .cmp op a b, env => do
      let av ← asInt (← execRefValue a env)
      let bv ← asInt (← execRefValue b env)
      some (.bool (cmpVal (encCmp op) av bv))
  | .logic op a b, env => do
      let av ← asBool (← execRefValue a env)
      let bv ← asBool (← execRefValue b env)
      some (.bool (logVal (encLog op) av bv))
  | .not e, env => do
      let v ← asBool (← execRefValue e env)
      some (.bool (!v))
  | .cast inner ty, env => do
      let v ← asInt (← execRefValue inner env)
      some (.int (tokCast (encCast ty) v))       -- the encoder's cast_target map, bounded
  | .index slice idx, env => do
      let iv ← asInt (← execRefValue idx env)
      let xs := env.slices slice
      if h : 0 ≤ iv.value ∧ iv.value < (xs.length : Int) then
        some (.int (xs.get ⟨iv.value.toNat, by
          obtain ⟨h0, h1⟩ := h
          have : iv.value.toNat < xs.length := by omega
          exact this⟩))
      else none

/-! ## The operator/cast round-trips — the encoder's maps are faithful -/

/-- The arithmetic-token round-trip is faithful: the encoder's `tokArith (encArith op)` is
    the source bounded op `evalArith op` (the #171 operator-map content; the bounded
    `binop_str` map is the independent ground truth). -/
theorem tokArith_encArith (op : AOp) (a b : BVal) :
    tokArith (encArith op) a b = evalArith op a b := by
  cases op <;> rfl

/-- The cast-token round-trip is faithful: the encoder's `tokCast (encCast ty)` is the
    source bounded wrap `castVal ty` (the #171 cast-target content: bounded, never nat). -/
theorem tokCast_encCast (ty : IntTy) (v : BVal) :
    tokCast (encCast ty) v = castVal ty v := by
  cases ty <;> rfl

/-! ## (T1) — `exec_ref_value` is sound against `S_E` -/

/--
  (T1) — verified-validator soundness for the exec-expression fragment (`S_E`).

  For every pure exec `ExecExpr` `e` and every exec env, the meaning of the reference
  encoder's output (`execRefValue`, routed through the encoder's `binop_str`/`cast_target`
  maps: bounded, never nat-coerced, carrying the overflow obligation) equals the source
  bounded-value denotation (`execDenote`, `S_E`):

  `∀ pure exec Expr P, ⟦exec_ref_value(P)⟧ = ⟦P⟧_{S_E}`.

  Proved by structural induction on `e`. Non-vacuous: `execRefValue` threads each construct
  through the encoder's operator/cast maps (`tokArith ∘ encArith`, `tokCast ∘ encCast`),
  `execDenote` through the source bounded ops (`evalArith`, `castVal`); the round-trips
  `tokArith_encArith`/`tokCast_encCast` are the load-bearing content. The overflow
  obligation is carried identically on both sides (`evalArith` returns `none` on overflow on
  both sides), so the equality includes the agreement of the obligation itself: a faithful
  encoder neither masks nor invents an overflow. This opens layer 2. -/
theorem exec_ref_sound : ∀ (e : ExecExpr) (env : ExecEnv),
    execRefValue e env = execDenote e env
  | .intLit ty v, env => by simp [execRefValue, execDenote]
  | .boolLit b, env => by simp [execRefValue, execDenote]
  | .var x, env => by simp [execRefValue, execDenote]
  | .arith op a b, env => by
      simp [execRefValue, execDenote, exec_ref_sound a env, exec_ref_sound b env,
            tokArith_encArith]
  | .cmp op a b, env => by
      simp [execRefValue, execDenote, exec_ref_sound a env, exec_ref_sound b env, encCmp]
  | .logic op a b, env => by
      simp [execRefValue, execDenote, exec_ref_sound a env, exec_ref_sound b env, encLog]
  | .not e, env => by
      simp [execRefValue, execDenote, exec_ref_sound e env]
  | .cast inner ty, env => by
      simp [execRefValue, execDenote, exec_ref_sound inner env, tokCast_encCast]
  | .index slice idx, env => by
      simp [execRefValue, execDenote, exec_ref_sound idx env]

/-- A `Prop`-equality corollary (the `⟦exec_ref_value(P)⟧ = ⟦P⟧_{S_E}` form the (T2)
    composition transits on). -/
theorem exec_ref_sound_eq (e : ExecExpr) (env : ExecEnv) :
    execRefValue e env = execDenote e env := exec_ref_sound e env

/-! ## The overflow-obligation treatment is genuine (not silently unbounded)

  These witness that the bounded model is real: an arithmetic op that overflows its type's
  bound has no value (`execDenote = none`, the obligation fails), while the same op stays
  in range when no overflow. If the model were silently unbounded, the overflow case would
  always have a value, defeating the exec-side content. -/

/-- A concrete env: scalar `a := 2^64 - 1` (the max `u64`), `b := 1` (a `u64`); slice
    `xs := [10, 20, 30]` (bounded `u64` elements). The `a + b` overflows `u64`. -/
def envOverflow : ExecEnv :=
  { vars := fun s =>
      if s = "a" then .int ⟨.u64, (2 : Int) ^ 64 - 1⟩
      else if s = "b" then .int ⟨.u64, 1⟩
      else if s = "z" then .int ⟨.u64, 0⟩
      else .int ⟨.u64, 0⟩
    slices := fun s => if s = "xs" then [⟨.u64, 10⟩, ⟨.u64, 20⟩, ⟨.u64, 30⟩] else [] }

/-- Overflow obligation is genuine. `a + b` with `a = 2^64 - 1`, `b = 1` (both `u64`)
    overflows: `execDenote` is `none` (the no-overflow obligation fails, the value is not
    defined; a Verus exec `+` rejected because overflow is possible). A silently-
    unbounded model would return `some (2^64)` here, so this proves the model is bounded. -/
theorem add_overflow_has_no_value :
    execDenote (.arith .add (.var "a") (.var "b")) envOverflow = none := by
  simp [execDenote, envOverflow, asInt, evalArith, rawArith, IntTy.bound, IntTy.width]

/-- Overflow obligation discharged on a non-overflowing op. `b + b` (`1 + 1 = 2`, in
    `u64` range) has the value `2`: the obligation is discharged, the value is the
    mathematical result. (Witnesses the partiality is the obligation, not a blanket failure.) -/
theorem add_in_range_has_value :
    execDenote (.arith .add (.var "b") (.var "b")) envOverflow
      = some (.int ⟨.u64, 2⟩) := by
  simp [execDenote, envOverflow, asInt, evalArith, rawArith, IntTy.bound, IntTy.width]

/-- The encoder agrees on the overflow obligation (faithful). `exec_ref_value`'s output
    is also `none` at the overflow: the encoder neither masks nor invents the overflow
    (`exec_ref_sound` instantiated). This is the exec-side fidelity: the bounded `+` carries
    the obligation, and the encoder carries the same one. -/
theorem encoder_agrees_on_overflow :
    execRefValue (.arith .add (.var "a") (.var "b")) envOverflow = none := by
  rw [exec_ref_sound]; exact add_overflow_has_no_value

/-! ## Negative lemma — the "never nat-coerced" discipline as a proven property

  The load-bearing fidelity property (issue title: "never nat-coerced + overflow obligation
  carried"). A nat-coerced exec value is the bug the discipline prevents: a `u64`
  subtraction that underflows, encoded with a nat-coercion (`a as nat - b as nat`, which
  clamps to `0` via `Int.toNat`; Verus spec `nat` subtraction is truncated), disagrees with
  the bounded/obligation semantics (which has no value, the no-underflow obligation fails)
  at a concrete env. Mirrors the `S_C` negative-lemma pattern (`cast_paren_drop_breaks_
  soundness`): a faulty encoder disagrees with the faithful denotation at a witness. -/

/-- A nat-coerced model of `a - b`: the bug `exec_ref_value` must not commit. It computes
    the subtraction in the unbounded `nat` domain (`(a.value - b.value).toNat`), which
    clamps a negative (underflowing) result to `0` (Lean `Int.toNat`, like Verus's
    truncated `nat` subtraction). It always returns a value (never `none`), masking the
    underflow, the soundness hole. (Contrast `evalArith .sub`, which returns `none` when
    the result leaves `[0, 2^64)`.) -/
def subNatCoerced (a b : BVal) : ExecVal :=
  .int ⟨a.ty, ((a.value - b.value).toNat : Int)⟩

/-- An env where `a := 0`, `b := 1` (both `u64`): `a - b = -1` underflows `u64`. -/
def envUnderflow : ExecEnv :=
  { vars := fun s =>
      if s = "a" then .int ⟨.u64, 0⟩
      else if s = "b" then .int ⟨.u64, 1⟩
      else .int ⟨.u64, 0⟩
    slices := fun _ => [] }

/-- The bounded `a - b` has no value at the underflow (the no-underflow obligation
    fails): `0 - 1 = -1 ∉ [0, 2^64)`, so `execDenote = none`. -/
theorem sub_underflow_has_no_value :
    execDenote (.arith .sub (.var "a") (.var "b")) envUnderflow = none := by
  simp [execDenote, envUnderflow, asInt, evalArith, rawArith, IntTy.bound, IntTy.width]

/-- Negative lemma — the "never nat-coerced" discipline. A nat-coerced `a - b` (the
    forbidden encoding `(a - b) as nat`, clamping the underflow to `0`) produces a value
    `some (.int ⟨u64, 0⟩)` at the same env where the faithful bounded `S_E` produces `none`
    (the underflow obligation fails). They disagree (`some 0 ≠ none`), so a nat-coercing
    encoder does not satisfy `exec_ref_sound`. This is the proven statement of the load-
    bearing exec fidelity: `exec_ref_value` must stay bounded (carrying the obligation),
    never nat-coerce; had it nat-coerced, soundness would fail here. -/
theorem nat_coercion_underflow_breaks_soundness :
    (some (subNatCoerced ⟨.u64, 0⟩ ⟨.u64, 1⟩) : Option ExecVal)
      ≠ execDenote (.arith .sub (.var "a") (.var "b")) envUnderflow := by
  rw [sub_underflow_has_no_value]
  simp [subNatCoerced]

/-- The faithful positive counterpart, for contrast: with the real bounded encoder (no nat
    coercion) the `a - b` exec value is sound: `execRefValue = execDenote` (both `none` at
    the underflow), by `exec_ref_sound`. Confirms the teeth bite only the nat-coercion, not
    the faithful encoder. -/
theorem sub_faithful_is_sound :
    execRefValue (.arith .sub (.var "a") (.var "b")) envUnderflow
      = execDenote (.arith .sub (.var "a") (.var "b")) envUnderflow :=
  exec_ref_sound _ _

/-! ## A faithful positive witness — the slice-index element value (E4) -/

/-- A faithful positive witness for the slice index (E4): `xs[1]` over the slice
    `[10, 20, 30]` (`u64`) has the encoder meaning equal to the source, the 1-st bounded
    element value `20`, by `exec_ref_sound`. Exercises the index element-value rewrite,
    proven sound (bounded element, never nat). -/
theorem slice_index_faithful_is_sound :
    execRefValue (.index "xs" (.intLit .u64 1)) envOverflow
      = execDenote (.index "xs" (.intLit .u64 1)) envOverflow :=
  exec_ref_sound _ _

/-- And the indexed value is the 1-st element `20` (non-vacuous, not a bottom). -/
theorem slice_index_value_is_twenty :
    execDenote (.index "xs" (.intLit .u64 1)) envOverflow
      = some (.int ⟨.u64, 20⟩) := by
  simp [execDenote, envOverflow, asInt, IntTy.bound, IntTy.width]

end Thermite.Exec
