# Rust↔Lean Encoder Correspondence — the arm-by-arm audit-by-inspection

<!--
tier: 3-component
status: draft
governs: thermite-tv/src/{ref_encode,exec_encode,exec_stmt_encode}.rs  (the Rust reference
         encoders that RUN in the per-run TV) ↔ lean/Thermite/{RefEncode,Exec}.lean +
         lean/Thermite/Exec/Stmt.lean (the kernel-proven Lean MODELS of those encoders).
         This doc is NOT production code; it is the audit artifact closing the named
         trust-base residual "Rust↔Lean correspondence" at the audit-by-inspection tier.
         No .rs is added or changed by this doc.
thesis-refs:
  - thermite-design.md §1 (trust relocated: "a skeptical third party can audit in minutes")
  - thermite-design.md §4.1 (contract-first functions; the exec body they guard)
  - thermite-design.md §4.2 (the frozen SpecTherm combinator cage + frozen triggers)
  - thermite-design.md §6 (the verification ladder; L3 SMT; L1 bounded exec values)
  - thermite-design.md §13 (roadmap; verified-microkernel convergence)
anchor-doc:
  - .design/verified/thermite-semantics.md REQ-6 (the Rust↔Lean encoder-correspondence
    residual — "the Rust thermite-tv code matching the Lean-proved algorithm"; this doc
    CLOSES that residual at the inspection tier; the extraction-bridge tier stays a named
    future option, per the reduced-trusted-base table item #3)
epic: crosslink #169 (lowering-soundness step 3)
blocker: crosslink #185 (increment 4b — Rust↔Lean correspondence)
prior-arc:
  - .design/verified/contract-tv.md (#139 — the contract reference encoder R_C)
  - .design/verified/exec-tv.md (#151 — the exec-expression reference encoder R_E)
  - .design/verified/exec-stmt-tv.md (#158 — the straight-line body state-transformer R_B)
-->

## Summary

The Lean proof spine proves the reference-encoder *algorithm* is denotation-faithful
(`Thermite.ref_sound`, `Thermite.Exec.exec_ref_sound`, `Thermite.Exec.body_ref_sound`,
composed in `Thermite.lowering_faithful`). But Lean proves the **Lean** definitions sound;
that the **Rust** encoders in `thermite-tv/` (which actually run in the per-run TV) implement
the *same* algorithm is a separate claim, discharged today by inspection. This document makes
that inspection rigorous and complete: an arm-by-arm map, every row showing the actual Rust
match arm / format string beside the actual Lean definition arm, plus the Verus-meaning bridge
and the Lean theorem (and negative lemma) that pins it. It also enumerates honestly what the
inspection does and does NOT cover, and what the stronger extraction-bridge tier would add.

## The claim being audited

> **(CORR)** For every construct in the frozen subset, the Rust encoder's emitted Verus text
> has — under Verus's documented expression semantics — exactly the denotation the corresponding
> Lean model assigns to that construct.

The trust reduction this closes:

```
  {Lean-proven encoder ALGORITHM}      — ref_sound / exec_ref_sound / body_ref_sound
+ {this Rust↔Lean correspondence}      — CORR, by arm-by-arm inspection (THIS DOC)
+ {per-run Z3 translation validation}  — h_tv: ⟦lower(P)⟧ = ⟦ref(P)⟧, discharged per run
= the UNIVERSAL faithfulness               per Thermite.lowering_faithful (Faithfulness.lean)
```

`lowering_faithful` consumes a `FnTvWitness` whose `h_tv_contract`/`h_tv_body` are the Z3
attestations that the **Rust** `ref(P)` agrees with the production lowering. The theorem then
composes that with the Lean (T1) theorems. CORR is the bridge that lets the theorem about the
**Lean** `refDenote`/`execRefValue`/`bodyRefState` carry over to the **Rust** `ref_contract_pred`
/`exec_ref_value`/`body_ref_state` that produced the string Z3 actually saw.

## Audited commits (PINNED — re-audit required on any change to these)

| Artifact | File | Pinned commit |
|---|---|---|
| Rust contract encoder | `thermite-tv/src/ref_encode.rs` | `579d3d48` (#150) |
| Rust exec-expr encoder | `thermite-tv/src/exec_encode.rs` | `43c9a6c8` (#152) |
| Rust exec-body encoder | `thermite-tv/src/exec_stmt_encode.rs` | `21b84c5f` (#163; was `b9dc22fd` #165 — re-pinned, see Amendment 2026-06-10) |
| Frozen combinator registry | `thermite-spec/src/combinators.rs` | `c0b1d8a3` (#4) |
| Lean spine | `lean/Thermite/**` | `65504c18` (was `7c85da25` — re-pinned, see Amendment 2026-06-10) |

Lean toolchain: `leanprover/lean4:v4.29.0` (downgraded from v4.30.0 by the #184 Z3-demotion probe — `lean/lakefile.toml` now `[[require]]`s Lean-SMT + Mathlib; this is OUTSIDE the `lean/Thermite/**` audited-spine scope and the entire audited spine still builds green and `sorry`-free on v4.29.0 — see `.design/verified/z3-demotion.md` and Amendment 2026-06-10).
Verified `sorry`-free by inspection: every `sorry` token in the tree is inside a comment, never in
a proof term (the proofs close by `simp`/`omega`/`decide`/`rfl`/structural induction). The spine's
axiom footprint is the standard `{propext, Classical.choice, Quot.sound}` (per the #182/#174 commit
messages and the `Faithfulness.lean` header). **Any edit to a pinned encoder file invalidates the
corresponding table section and requires re-audit (see "Drift" below).**

> **Amendment 2026-06-10 (re-pin, crosslink #200) — VERIFIED additive-only, NO re-audit of the arm
> tables needed.** The deep-audit drift tripwire (`scripts/audit.sh` check [4], commit `a0d8ea64`)
> correctly fired: two pinned SHAs were stale because the loop-TV work (#163) landed AFTER the
> arm-by-arm audit. The drift was VERIFIED additive-only against the actual diffs before re-pinning,
> NOT rubber-stamped:
> - **`thermite-tv/src/exec_stmt_encode.rs` `b9dc22fd` → `21b84c5f`** — `git diff` shows 396
>   insertions, 1 deletion; the single deletion is the `use` line, EXTENDED only
>   (`{BinOp, Block, Expr, IndexArg, Stmt}` → `{BinOp, Block, Clause, Expr, IndexArg, LoopKind, LoopNode, Stmt}`,
>   adding `Clause`/`LoopKind`/`LoopNode` for the new loop arms). No AUDITED arm changed:
>   `thread_stmt`/`body_ref_state`/`encode_block_tail`/`body_ref_state_ensures` (Table 3) are byte-for-byte
>   the same. The additions are the new loop arms `loop_ref_obligations`/`recognize_v1_loop`.
> - **`lean/Thermite/**` `7c85da25` → `65504c18`** — `git diff --name-status -- lean/Thermite/` shows
>   exactly TWO ADDED files and ZERO modified: `Exec/Loop.lean` (#163, the new `while`-loop semantics
>   + `while_rule`/`tv_meta_loop`) and `SmtDemo.lean` (#184, the Z3-demotion PoC — this SHA range
>   straddles both #163 and #184). Every AUDITED spine file — `RefEncode.lean`, `Denote.lean`,
>   `Exec.lean`, `Exec/Stmt.lean`, `Soundness.lean`, `Faithfulness.lean` — is UNCHANGED, so all of
>   Tables 1–3 and the cited (T1) theorems (`ref_sound`/`exec_ref_sound`/`body_ref_sound`/`lowering_faithful`)
>   + every negative lemma stand re-audit-free. (The `lean/Thermite.lean` import-aggregator gained two
>   `import` lines and `lean/lakefile.toml`/`lean-toolchain`/`lake-manifest.json` changed for #184 —
>   all OUTSIDE the `lean/Thermite/**` audited scope, all purely additive.)
>
> Verification verdict: **additive-only — the new loop arms `loop_ref_obligations`/`recognize_v1_loop`
> + the new `Exec/Loop.lean` (and the #184 `SmtDemo.lean`); no audited arm changed.** The arm tables of
> THIS doc are unchanged; the new loop arms are NOT absorbed here — see the loop cross-reference under
> "What this inspection does NOT cover."


## Requirements

- **REQ-1 (the arm-by-arm correspondence map)** — for every arm of each Rust reference encoder,
  exhibit the Rust source (the match arm / format string), the corresponding Lean model arm, the
  one-line Verus-meaning bridge, and the pinning Lean theorem (+ negative lemma where one exists).
  Derived from `thermite-semantics.md` REQ-6 (the correspondence residual). The deliverable IS this
  doc's tables.
- **REQ-2 (the extraction bridge — the stronger tier)** — a mechanized Lean→Rust extraction (or a
  Rust-side proof) that would make the Rust encoder equal the Lean model by construction rather than
  by inspection. NOT in this doc's scope; the named future closure of the same residual.

## Acceptance criteria

- **AC-1 (completeness)** — every match arm of `ref_contract_pred`/`exec_ref_value`/`body_ref_state`
  (and the combinator registry `verus_l3` forms the encoders reference) appears as a row, OR is
  recorded as an explicit out-of-Lean-scope residual in "What this inspection does NOT cover."
  The `ref_encode.rs::encode` dispatch is enumerated exhaustively in Table 1H (all 15 arms). Two of
  its live arms — `Expr::Field` (struct-field access `result.x`) and `Expr::TupleProj` (tuple
  projection `result.0`) — have NO Lean `Expr` constructor and are recorded as the explicit
  out-of-Lean-scope residual **Discrepancy D6** (inspection-only, like the Map accessor D3 and the
  guard-arm path D4); they are NOT correspondence rows, because no correspondence exists to claim.
  The leaf arms (`IntLit`/`BoolLit`/`Path`) ARE rowed (Table 1H) with their Lean `intLit`/`boolLit`/
  `var` counterparts. The catch-all `Err` arm is the faithful out-of-`S_C` boundary. So every live
  `encode` arm is now a row OR an explicitly-listed residual — AC-1 is true, not merely asserted.
- **AC-2 (groundedness)** — every row quotes the actual Rust arm and the actual Lean arm (short exact
  excerpts), never a paraphrase. A row that cannot be grounded is a discrepancy, recorded in
  "Discrepancies found."
- **AC-3 (the bridge assumptions are enumerated)** — the inspection's own trust items (Verus
  semantics, fuel↔well-founded unfolding, the symbolic-env↔big-step-transformer assumption) are
  listed.
- **AC-4 (the residuals are honest)** — string-level formatting, the production lowerer (out of
  scope — that is what TV is for), loops (#163), and the extraction-bridge tier are named as
  uncovered.

## How to read a correspondence row

Each row has four cells:

1. **Rust arm** — `file` symbol + the quoted match arm or `format!` string.
2. **Lean arm** — `file` symbol + the quoted Lean definition arm.
3. **Verus-meaning bridge** — one line: why, under Verus's documented semantics, the emitted string
   denotes what the Lean arm says.
4. **Pinned by** — the Lean theorem that proves the Lean arm sound, and the negative lemma (if one
   exists) proving the choice is load-bearing.

A skeptic verifies a row by reading the two quoted arms and confirming the emitted token/shape
matches the Lean datum, then checking the bridge line against Verus's precedence/operator rules.

---

## Table 1 — `thermite-tv/src/ref_encode.rs` ↔ `lean/Thermite/RefEncode.lean`

### 1A. The binary-operator map (`binop_str`) ↔ `encOp`/`encLog`/`encArith` + `encode_unary`

Rust `binop_str` (`ref_encode.rs`) is a single 18-arm `match`. Lean splits it into three total
maps (`encOp` comparisons, `encLog` logical, `encArith` arithmetic) plus `encode_unary`→`neg`.
The token each emits is interpreted by `tokRel`/`tokConn`/`tokArith` (RefEncode.lean).

| # | Rust arm (`binop_str`) | Lean arm | Bridge | Pinned by |
|---|---|---|---|---|
| Eq | `BinOp::Eq => "=="` | `encOp \| CmpOp.eq => VerusCmpTok.eqTok`; `tokRel eqTok x y => x = y` | Verus `==` on two ints is `=` | `ref_sound` (cmp case); neg: `eq_le_infidelity_breaks_soundness` |
| Ne | `BinOp::Ne => "!="` | `encOp \| CmpOp.ne => neTok`; `tokRel neTok => x ≠ y` | Verus `!=` is `≠` | `ref_sound` |
| Lt | `BinOp::Lt => "<"` | `encOp \| CmpOp.lt => ltTok`; `tokRel ltTok => x < y` | Verus `<` is `<` | `ref_sound` |
| Le | `BinOp::Le => "<="` | `encOp \| CmpOp.le => leTok`; `tokRel leTok => x ≤ y` | Verus `<=` is `≤` | `ref_sound`; neg: `eq_le_infidelity_breaks_soundness` |
| Gt | `BinOp::Gt => ">"` | `encOp \| CmpOp.gt => gtTok`; `tokRel gtTok => x > y` | Verus `>` is `>` | `ref_sound` |
| Ge | `BinOp::Ge => ">="` | `encOp \| CmpOp.ge => geTok`; `tokRel geTok => x ≥ y` | Verus `>=` is `≥` | `ref_sound` |
| And | `BinOp::And => "&&"` | `encLog \| LogOp.and => andTok`; `tokConn andTok p q => p ∧ q` | Verus `&&` is `∧` | `ref_sound` (logic case) |
| Or | `BinOp::Or => "\|\|"` | `encLog \| LogOp.or => orTok`; `tokConn orTok => p ∨ q` | Verus `\|\|` is `∨` | `ref_sound` |
| Not | `encode_unary`: `UnaryOp::Not => Ok(format!("(!{i})"))` | `refDenote \| Expr.neg e => ¬ refDenote …` | Verus `(!p)`, parenthesized, is `¬` | `ref_sound` (neg case) |
| Add | `BinOp::Add => "+"` | `encArith \| add => plusTok`; `tokArith plusTok => arithDenote add` | Verus `+` (spec int) is `+` | `ref_sound` (via `refVal_eq`); `tokArith_encArith` |
| Sub | `BinOp::Sub => "-"` | `encArith \| sub => minusTok` → `arithDenote sub` | `-` | `tokArith_encArith` |
| Mul | `BinOp::Mul => "*"` | `encArith \| mul => starTok` → `arithDenote mul` | `*` | `tokArith_encArith` |
| Div | `BinOp::Div => "/"` | `encArith \| div => slashTok` → `arithDenote div` | `/` (nonzero-divisor precond, source-side) | `tokArith_encArith` |
| Rem | `BinOp::Rem => "%"` | `encArith \| rem => percentTok` → `arithDenote rem` | `%` (nonzero precond) | `tokArith_encArith` |
| Shl | `BinOp::Shl => "<<"` | `encArith \| shl => shlTok` → `arithDenote shl` | `<<` = `* 2^k` | `tokArith_encArith` |
| Shr | `BinOp::Shr => ">>"` | `encArith \| shr => shrTok` → `arithDenote shr` | `>>` = `/ 2^k` | `tokArith_encArith` |
| BitAnd | `BinOp::BitAnd => "&"` | `encArith \| bitAnd => ampTok` → `arithDenote bitAnd` | `&` (Nat.land on bounded operands) | `tokArith_encArith` |
| BitOr | `BinOp::BitOr => "\|"` | `encArith \| bitOr => pipeTok` → `arithDenote bitOr` | `\|` (Nat.lor) | `tokArith_encArith` |
| BitXor | `BinOp::BitXor => "^"` | `encArith \| bitXor => caretTok` → `arithDenote bitXor` | `^` (Nat.xor) | `tokArith_encArith` |

Both sides wholly parenthesize a binary: Rust `encode_binary` returns `format!("({l} {} {r})", binop_str(op))`;
the Lean `refIntVal Expr.arith` is `tokArith (encArith op) (refIntVal a) (refIntVal b)` (operands bound
first, so the parens make the Rust string parse to that same AST). This is the bridge for every binary row.

**The Eq nat-coercion via `encode_binary`'s `is_nat_valued`.** Rust `encode_binary` applies the
`as nat` coercion **only on `Eq`** (`let coerce = op == BinOp::Eq;` then `is_nat_valued(rhs)` /
`is_nat_valued(lhs)`). `is_nat_valued` is `matches!(expr, Expr::Call { callee, .. } if … Expr::Path(_))`
— a nat-returning spec-fn/`count_where` call. The Lean side does NOT model the `as nat` coercion
string explicitly; it routes both operands through `refIntVal`/`castDenote` over the shared `Int`
domain, where `castDenote CastTy.nat v = (v.toNat : Int)` and the `≥ 0` source frame makes the clamp
the identity. **This is the one place the Lean model abstracts a Rust string detail** — see
Discrepancy D1 (it is a sound abstraction, not a mismatch).

### 1B. Casts — `encode_cast`/`cast_target` ↔ `encCast`/the `cast` arm (#122 / #146)

| Construct | Rust arm | Lean arm | Bridge | Pinned by |
|---|---|---|---|---|
| `as u32` | `cast_target`: `Type::Prim(PrimType::U32) => Ok("u32")` | `encCast \| CastTy.u32 => u32Tok`; `tokCast u32Tok => castDenote CastTy.u32` (identity on int domain) | `as u32` on spec int (no-overflow frame) is value-preserving | `tokCast_encCast`; `ref_sound` (via `refVal_eq` cast) |
| `as u64` | `Type::Prim(PrimType::U64) => Ok("u64")` | `encCast \| CastTy.u64 => u64Tok` → `castDenote u64` | value-preserving | `tokCast_encCast` |
| `as usize` | `Type::Prim(PrimType::Usize) => Ok("usize")` | `encCast \| CastTy.usize => usizeTok` → `castDenote usize` | value-preserving | `tokCast_encCast` |
| `as nat` | `Type::Named(n) if n == "nat" \| n == "int" => Ok(n.clone())` (nat) | `encCast \| CastTy.nat => natTok` → `castDenote nat = (v.toNat:Int)` | `as nat` injects into ℕ; `≥0` source frame makes clamp the identity | `tokCast_encCast` |
| `as int` | same arm (int) | `encCast \| CastTy.int => intTok` → `castDenote int = v` | `as int` is the spec int, identity | `tokCast_encCast` |
| cast→bool | `PrimType::Bool => Err(Unsupported("cast to bool …"))` | (absent — `CastTy` has no `bool`) | OUT of `S_C`; honest `Err` | n/a (out of scope, faithfully) |
| **#122 inner-paren** | `encode_cast`: `Ok(format!("({e}) as {target}"))` — inner wrapped UNCONDITIONALLY | `refIntVal Expr.cast inner ty => tokCast (encCast ty) (refIntVal fuel inner env)` — cast binds the WHOLE inner | the `({e})` paren makes a compound inner (`a + b`) bind as a unit so `(a+b) as nat` ≠ `a + (b as nat)` | neg: `cast_paren_drop_breaks_soundness` (Soundness.lean) |
| **#146 outer-paren** | `is_lt_leading`: `matches!(op, BinOp::Lt \| Le \| Shl)`; `encode_binary_operand` wraps a `Cast` left of such op: `if is_left && matches!(operand, Expr::Cast{..}) && is_lt_leading(op) { Ok(format!("({s})")) }` | (modelled at the AST level: the Lean cast denotes the whole inner; the outer paren is a *parse-safety* property of the string, not a denotation change) | `(x as u32) < 33` avoids the `u32<` generic-args mis-parse; both sides denote the same `<` over the cast value | `tokCast_encCast` + the #122 neg lemma; the #146 outer-paren is a parse-safety guarantee (see Bridge Assumption A1 / Discrepancy D2) |

The `#122` negative lemma `cast_paren_drop_breaks_soundness` constructs `castInnerFaithful` vs
`castInnerParenDropped` and proves the dropped-paren encoder disagrees with the source at a witness
env — proving the Rust `format!("({e}) as {target}")` paren is load-bearing.

### 1C. Slice/index/ref rewrites — `encode_slice_arg`/`encode_index`/`encode_ref` ↔ `refSeqVal` arms (#178)

| Construct | Rust arm | Lean arm | Bridge | Pinned by |
|---|---|---|---|---|
| slice `@`-view | `encode_slice_arg`: bare `Expr::Path` not seq-bound → `Ok(format!("{}@", segments[0]))`; seq-bound → identity `encode(arg)` | `refSeqVal \| Expr.seqVar x, env => env.seqs x` (the `@`-view is identity on the value) | `xs@` is the `Seq` view of slice `xs`; same element sequence | `refSeqVal_eq_seqVal`; `ref_sound` |
| single index | `encode_index` `IndexArg::Single(i)`: `Ok(format!("{recv}[{idx}]"))` over `encode_receiver`'s `@`-view; `idx = encode_index_value(i)` (`<p> as int` / bare literal) | `refIntVal \| Expr.idx base i => byteView encByteAt (refSeqVal base) (refIntVal i)` (`byteView specByteAt s i = seqIdx s i`) | `xs@[i as int]` is the i-th element of the view | `byteView_encByteAt`; `ref_sound`; neg: `byteview_wrong_index_breaks_soundness` |
| `&xs[..i]` (RangeTo) | `encode_index` `RangeTo(hi)`: `Ok(format!("{recv}.subrange(0, {h})"))` (via `encode_ref`→`encode_index`) | `refSeqVal \| Expr.subrange base (RangeArg.rangeTo hi) => seqSub s 0 (refIntVal hi)` | `xs@.subrange(0,i)` is the prefix `seqSub 0 i` | `refSeqVal_eq_seqVal`; `subrange_index_faithful_matches_source` |
| `&xs[a..b]` (Range) | `Range(lo,hi)`: `Ok(format!("{recv}.subrange({l}, {h})"))` | `RangeArg.range lo hi => seqSub s (refIntVal lo) (refIntVal hi)` | `xs@.subrange(a,b)` = `seqSub a b` | `refSeqVal_eq_seqVal` |
| `&xs[a..]` (RangeFrom) | `RangeFrom(lo)`: `Ok(format!("{recv}.subrange({l}, {recv}.len() as int)"))` | `RangeArg.rangeFrom lo => seqSub s (refIntVal lo) (s.length : Int)` | `xs@.subrange(a, xs@.len())` = `seqSub a |s|` | `refSeqVal_eq_seqVal` |
| bare `&xs` | `encode_ref`: `Expr::Path(_) => encode_slice_arg(inner)` | (the slice `@`-view above) | `&xs` over a spec slice is the `@`-view (no Verus `&Seq`) | `refSeqVal_eq_seqVal` |
| `&e` (other) | `encode_ref` `other => Err(Unsupported)` | (absent) | OUT of `S_C`; honest `Err` | n/a |

### 1D. String byte-view — `encode_string_byteview`/`encode_method_call` ↔ `byteView`/`encByteAt`/`encLen` (#127)

| Construct | Rust arm | Lean arm | Bridge | Pinned by |
|---|---|---|---|---|
| `s.byte_at(i)` (String) | `encode_string_byteview` `"byte_at"`: `Ok(format!("{recv}.spec_byte_at({idx})"))` (literal bare; else `as int`) | `refIntVal \| Expr.byteAt base i => byteView encByteAt (refSeqVal base) (refIntVal i)`; `encByteAt = specByteAt`; `byteView specByteAt s i = seqIdx s i` | `s.spec_byte_at(i)` is the i-th byte | `byteView_encByteAt`; neg: `byteview_misdispatch_breaks_soundness`, `byteview_wrong_index_breaks_soundness` |
| `s.len()` (String) | `encode_string_byteview` `"len"`: `Ok(format!("{recv}.spec_len()"))` | `refIntVal \| Expr.seqLen base => byteView encLen (refSeqVal base) 0`; `encLen = specLen`; `byteView specLen s _ = (s.length:Int)` | `s.spec_len()` is the byte-sequence length | `byteView_encLen` |
| `xs.len()` (slice) | `encode_method_call` `"len"`: `recv = encode_len_receiver(receiver)` (BARE path) → `Ok(format!("{recv}.len()"))` | same `Expr.seqLen` arm (length of view) | a slice `.len()` in spec position is the seq length | `byteView_encLen` |
| `xs[i]` byte-view (Seq recv) | `encode_method_call` `"byte_at"` (non-string recv): `recv = encode_receiver(..)` (@-view); `Ok(format!("{recv}[{idx}]"))` | `Expr.byteAt`/`Expr.idx` (same `seqIdx`) | `recv@[i]` is the i-th element | `byteView_encByteAt` |
| `s.slice(lo,hi)` | `encode_method_call` `"slice"`: `Ok(format!("{recv}.subrange({lo}, {hi})"))` | `Expr.subrange`/`seqSub` (see 1C) | `recv@.subrange(lo,hi)` | `refSeqVal_eq_seqVal` |
| `.slice` on String | `encode_string_byteview` `other => Err(Unsupported)` | (absent) | OUT; `TString` has no `spec_slice` | n/a (faithful absence) |
| other spec method | `encode_method_call` `other => Err(Unsupported)` | (absent) | OUT of frozen byte-view set | n/a |

The #127 negative lemma `byteview_misdispatch_breaks_soundness` proves routing `byte_at` to the
length spec-fn (the name-collision bug) disagrees with the faithful denotation — pinning that the
Rust dispatch CHOICE (`byte_at`→`spec_byte_at`, `len`→`spec_len`) is the load-bearing content.

The **Map accessor** (`encode_map_accessor`: `contains_key`→`spec_contains_key`, `len`→`len`) and
the **Option/Result frame** that #150 added to `RefCtx` are **NOT modelled in Lean** — see
Discrepancy D3 (Map membership is a residual; it is not in the Lean `S_C` fragment).

### 1E. The 8 combinators — `encode_combinator_call`/`encode_combinator_arg`/`encode_pred_arg`/`encode_index_value` ↔ the `comb` arms + `countWhereVal`/`permEq`

The Rust `encode_combinator_call` REUSES the registry name and re-encodes args **per
`CombinatorSig.arg_kinds`** (the frozen `thermite-spec/src/combinators.rs` `verus_l3` is the shared
ground truth on both Rust and production sides). Lean `refDenote Expr.comb` reproduces each
combinator's frozen `verus_l3` quantifier body directly.

| Combinator | Frozen `verus_l3` (combinators.rs) | Lean arm (`refDenote`/`refIntVal`) | Pinned by |
|---|---|---|---|
| `forall_in(s,p)` | `forall\|i:int\| 0<=i<s.len() ==> #[trigger] p(s[i])` | `CombName.forallIn => ∀ i, (0≤i ∧ i<s.length) → p i` | `ref_sound` (comb); neg: `wrong_combinator_breaks_soundness` |
| `exists_in(s,p)` | `exists\|i:int\| 0<=i<s.len() && #[trigger] p(s[i])` | `CombName.existsIn => ∃ i, (0≤i ∧ i<s.length) ∧ p i` | `ref_sound`; neg: `wrong_combinator_breaks_soundness` |
| `sorted(s)` | `forall\|i,j\| 0<=i<=j<s.len() ==> s[i]<=s[j]` | `CombName.sorted => ∀ i j, (0≤i ∧ i≤j ∧ j<s.length) → seqIdx s i ≤ seqIdx s j` | `ref_sound` |
| `forall_below(s,n,p)` | `forall\|i\| 0<=i<n && i<s.len() ==> #[trigger] p(s[i])` | `CombName.forallBelow => ∀ i, (0≤i ∧ i<n ∧ i<s.length) → p i` | `ref_sound`; neg: `index_argkind_slice_view_breaks_soundness` (#145) |
| `forall_from(s,n,p)` | `forall\|i\| n<=i<s.len() ==> #[trigger] p(s[i])` | `CombName.forallFrom => ∀ i, (n≤i ∧ i<s.length) → p i` | `ref_sound` |
| `disjoint(a,b)` | `forall\|i,j\| (0<=i<a.len() && 0<=j<b.len()) ==> a[i]!=b[j]` | `CombName.disjoint => ∀ i j, (… ) → seqIdx s i ≠ seqIdx s2 j` | `ref_sound` |
| `count_where(s,p)` | `if s.len()==0 {0} else {(if p(s[0]){1}else{0})+count_where(s.drop_first(),p)}` | `refIntVal Expr.comb countWhere … => countWhereVal p s`; `countWhereVal (x::xs) = (ite (p x) 1 0) + countWhereVal p xs` | `count_where_*` lemmas; neg: `count_where_wrong_pred_breaks_soundness`, `count_where_off_by_one_breaks_soundness` |
| `permutation_of(a,b)` | `a.to_multiset() == b.to_multiset()` | `CombName.permutationOf => permEq s s2`; `permEq a b = ∀ x, a.count x = b.count x` | `permutation_*`; neg: `permutation_set_model_breaks_soundness` |

The three **arg-kinds** (the `encode_combinator_arg` dispatch on `thermite_spec::ArgKind`):

| ArgKind | Rust arm (`encode_combinator_arg`) | Lean threading | Pinned by |
|---|---|---|---|
| `Slice` | `ArgKind::Slice => encode_slice_arg(arg)` (the `@`-view) | `let s := refSeqVal fuel seq env` | `refSeqVal_eq_seqVal` |
| `Index` | `ArgKind::Index => encode_index_value(arg)` (scalar `<p> as int`, NEVER `@`-view — #145) | `let n := match idx with \| some e => refIntVal fuel e env` | neg: `index_argkind_slice_view_breaks_soundness` |
| `Pred` | `ArgKind::Pred => encode_pred_arg(arg)`: `Ok(format!("\|{}: u32\| {body_s}", params[0]))` | `let p := fun i => refDenote fuel body (env.bindInt bound (seqIdx s i))` | `ref_sound` (recursive IH on body) |
| `Value` | `ArgKind::Value => encode(arg)` | (no combinator in the frozen set uses `Value`; absent) | n/a |

The Rust `encode_pred_arg` binds the closure param at `u32` (`format!("|{}: u32| {body_s}")`); Lean
`Pred.mk bound body` binds via `Env.bindInt bound (seqIdx s i)`. Both apply the body at the i-th
element. The `wrong_combinator_breaks_soundness` (forallIn↔existsIn) and the `#145`
`index_argkind_slice_view_breaks_soundness` are the load-bearing negative lemmas here.

### 1F. Match / `is` — `encode_match`/`encode_pattern`/`is_builtin_variant` + the `Expr::Is` arm ↔ `denoteArms`/`refDenoteArms`/`is_` (#180)

| Construct | Rust arm | Lean arm | Bridge | Pinned by |
|---|---|---|---|---|
| `match scrut { … }` | `encode_match`: `format!("match {s} {{\n")` … each arm `{pat} => {body}` (scrutinee + each body by `encode`) | `refDenote Expr.match_ scrut arms => refDenoteArms fuel (scrutVal scrut env) arms env` | Verus `match` selects the arm by variant (encoder reuses Verus match) | `ref_sound`+`ref_sound_arms`; neg: `match_arm_swap_breaks_soundness` |
| arm select+bind | `encode_match` arm loop; pattern via `encode_pattern` | `refDenoteArms \| MatchArm.mk variant binder body :: rest => if scrut.variant = variant then (match binder …) else refDenoteArms … rest` | arm chosen by variant; payload bound | `ref_sound_arms` |
| `Some(x)`/`Ok(x)`/`Err(e)`/`None` patterns | `encode_pattern` `Pattern::Enum` + `is_builtin_variant(head)`: `matches!(head, "Some"\|"None"\|"Ok"\|"Err")` | `Variant.{some_,none_,ok,err}` + `MatchArm.mk variant binder body` | built-in Option/Result, unqualified in Verus | `ref_sound_arms`; `match_result_faithful_is_sound` |
| user variant | `encode_pattern`: `if !is_builtin_variant(head) { Err(Unsupported) }` | (absent — `Variant` has only the 4 built-ins) | OUT of `S_C`; honest `Err` | n/a (faithful absence) |
| guard / wildcard / nested | `encode_pattern` `other => Err(Unsupported)`; `encode_match` guard arm `{pat} if {g} => …` | (Lean models only the 2-arm exhaustive built-in form; no guard arm) | guard arm is OUT of the modelled C7 fragment | see Discrepancy D4 |
| `scrut is Variant` | `Expr::Is` arm (`encode` in `ref_encode.rs`): `Ok(format!("({s} is {})", variant.join("::")))` | `refDenote Expr.is_ scrut variant => ((scrutVal scrut env).isVariant variant = true)` | Verus `(e is V)` discriminant test | `ref_sound` (is_ case); neg: `is_wrong_variant_breaks_soundness` |

### 1G. Spec-fn calls — `encode_call` case (3) ↔ the `specCall` arm + the fuel-indexed registry (#181)

| Construct | Rust arm | Lean arm | Bridge | Pinned by |
|---|---|---|---|---|
| `old(x)` | `encode_call` case (1): `if name == "old" { … Ok(format!("old_{mangled}")) }` | (modelled as a free `var` — `old(x)` binds a distinct obligation param) | the obligation binds `old_x` as a value param | `ref_sound` (var case) |
| frozen combinator | `encode_call` case (2): `if thermite_spec::lookup(&name).is_some() { encode_combinator_call(..) }` | the `Expr.comb` arms (Table 1E) | dispatched to the registry `verus_l3` | (Table 1E) |
| named spec-fn call | `encode_call` case (3): `Ok(format!("{name}({})", encoded_args.join(", ")))` (NOT inlined) | `refDenote \| fuel+1, Expr.specCall name args, env => match env.specs name with \| some fn => refDenote fuel fn.body (env.bindParams fn.params (refIntValArgs (fuel+1) args env)) \| none => True` | the call is a Verus `spec fn` call; the body is lowered ONCE as its own `spec fn` | `ref_sound` (specCall case); neg: `specfn_arg_order_breaks_soundness`, `specfn_wrong_resolution_breaks_soundness` |
| per-arg encoding | `encode_call_arg`: `Closure => encode_pred_arg; other => encode_slice_arg` | `refIntValArgs \| a::rest => refIntVal fuel a env :: refIntValArgs fuel rest env` | each arg re-encoded (slice `@`-view / closure form) | `refIntValArgs_eq` |

The fuel index is the Lean modelling of Verus's well-founded `spec fn` unfolding (every Thermite
spec fn carries a mandatory `dec` measure, §4.2). The Rust encoder does NOT inline (it emits a call),
so there is no recursion to bound on the Rust side; the fuel models the *meaning* of the resulting
recursive spec-fn definition. See Bridge Assumption A2.

### 1H. The `encode` dispatch inventory (AC-1 completeness for `ref_encode.rs`)

The `ref_contract_pred`→`encode` `match` (`ref_encode.rs`) has 15 arms. AC-1 requires each to be a
row above OR an explicit residual. The full enumeration:

| `encode` arm | Disposition |
|---|---|
| `Expr::IntLit { value, .. } => Ok(value.to_string())` | leaf — Lean `Expr.intLit value`; the integer-literal denotation (`refIntVal Expr.intLit => value`). Pinned by `ref_sound` (intLit case). Benign, rowed here. |
| `Expr::BoolLit(b) => Ok(b.to_string())` | leaf — Lean `Expr.boolLit value`; the bool-literal denotation. Pinned by `ref_sound` (boolLit case). Benign, rowed here. |
| `Expr::Path(segments) => encode_path(..)` | the var leaf — Lean `Expr.var`/`Expr.seqVar`/`Expr.strVar`/`Expr.optResVar` (a free obligation param; `result`/`old(x)` are free names). Pinned by `ref_sound` (var case); the `old(x)` form is Table 1G row 1. |
| `Expr::Binary { op, lhs, rhs }` | Table 1A (the binop map + the Eq nat-coercion). |
| `Expr::Unary { op, expr }` | Table 1A (`Not`). |
| `Expr::Call { callee, args }` | Tables 1E (combinators) + 1G (`old`/named spec-fn calls). |
| `Expr::MethodCall { .. }` | Table 1D (byte-view / Map / slice `.len()`). |
| `Expr::Index { base, index }` | Table 1C. |
| `Expr::Ref { expr: inner, .. }` | Table 1C (the `&xs[..i]` / bare `&xs` slice-view rewrite). |
| `Expr::Cast { expr, ty }` | Table 1B. |
| `Expr::Field { receiver, name }` | **RESIDUAL — Discrepancy D6.** No Lean `Expr` constructor (`field`); struct-field access is OUTSIDE the Lean `S_C` fragment. Inspection-only. |
| `Expr::TupleProj { receiver, index }` | **RESIDUAL — Discrepancy D6.** No Lean `Expr` constructor (`tupleProj`); tuple-projection access is OUTSIDE the Lean `S_C` fragment. Inspection-only. |
| `Expr::Is { scrutinee, variant }` | Table 1F (last row). |
| `Expr::Match { scrutinee, arms }` | Table 1F. |
| `other => Err(RefEncodeError::Unsupported(node_kind(other)))` | the honest catch-all — every `Expr` variant NOT above (`Closure` outside a pred slot, `If`, `StructLit`, `Deref`, `StrLit`, `Tuple`) is a real `Err`, never a silent wrong encoding (faithful absence; out of `S_C`). |

Every live `encode` arm is now a row (Tables 1A–1G + the leaf rows here) OR an explicit residual
(`Field`/`TupleProj` → D6), and the catch-all `Err` is the faithful out-of-`S_C` boundary. AC-1 holds
for `ref_encode.rs`.

---

## Table 2 — `thermite-tv/src/exec_encode.rs` ↔ `lean/Thermite/Exec.lean`

The exec encoder is the BOUNDED dual: values carry the overflow obligation, casts WRAP at the target
width, and there is **no `nat`/`int`**. Lean `execDenote` is `Option ExecVal` (`none` = obligation
fails); `execRefValue` is the encoder model; `exec_ref_sound` proves them equal.

### 2A. The exec operator map (`binop_str`) ↔ `encArith`/`encCmp`/`encLog`

| Op class | Rust arm (`exec_encode.rs::binop_str`) | Lean arm | Bridge | Pinned by |
|---|---|---|---|---|
| arithmetic (10) | `Add=>"+"` … `BitXor=>"^"` (identical 10 arms to Table 1A) | `encArith : AOp → ArithTok`; `tokArith tok a b : Option BVal` (overflow → `none` via `evalArith`) | exec `+` is the bounded add carrying the overflow obligation, NOT `wrapping_*` | `exec_ref_sound`; `tokArith_encArith`; neg: `nat_coercion_underflow_breaks_soundness` |
| comparison (6) | `Eq=>"=="` … `Ge=>">="` | `encCmp : COp → COp := id`; `cmpVal op a b : Bool` | exec comparison → `bool` | `exec_ref_sound` |
| logical (2) | `And=>"&&"`, `Or=>"\|\|"` | `encLog : LOp → LOp := id`; `logVal` | `&&`/`\|\|` over bool | `exec_ref_sound` |
| Not | `encode_unary`: `UnaryOp::Not => Ok(format!("(!{i})"))` | `ExecExpr.not e`; `execDenote .not => !v` | `(!b)` | `exec_ref_sound` |

The overflow obligation is GENUINE: `evalArith op a b` is `none` when the math result leaves
`[0, ty.bound)`. The negative lemma `nat_coercion_underflow_breaks_soundness` proves a nat-coercing
`a - b` (clamping the underflow to `0`) DISAGREES with `execDenote = none` at `a=0,b=1:u64` — pinning
that the Rust encoder must stay bounded (its `binop_str` emits `-`, never a `wrapping_sub`/`as nat`).

### 2B. Bounded cast targets — `cast_target` ↔ `encCast`/`castVal` (no nat/int)

| Target | Rust arm (`exec_encode.rs::cast_target`) | Lean arm | Bridge | Pinned by |
|---|---|---|---|---|
| u32 | `Type::Prim(PrimType::U32) => Ok("u32")` | `encCast \| IntTy.u32 => …`; `castVal u32 v = v.value % 2^32` | `as u32` wraps at width 32 | `tokCast_encCast`; `exec_ref_sound` |
| u64 | `Type::Prim(PrimType::U64) => Ok("u64")` | `IntTy.u64`; `castVal u64 = % 2^64` | wrap at 64 | `tokCast_encCast` |
| usize | `Type::Prim(PrimType::Usize) => Ok("usize")` | `IntTy.usize` (width 64); `% 2^64` | wrap at usize=64 | `tokCast_encCast` |
| u8 | `Type::Named(n) if matches!(n, "u8"\|"u16") => Ok(n.clone())` (u8) | `IntTy.u8`; `castVal u8 = % 2^8` | narrowing wrap at 8 | `tokCast_encCast` |
| u16 | same arm (u16) | `IntTy.u16`; `% 2^16` | wrap at 16 | `tokCast_encCast` |
| bool / nat / int | `PrimType::Bool => Err(…)`; `other => Err(Unsupported("… NEVER nat/int"))` | (absent — `IntTy` has NO nat/int) | OUT; the exec encoder NEVER nat-coerces | neg: `nat_coercion_underflow_breaks_soundness` (the whole point) |

The #122 inner-paren (`encode_cast`: emits `{e} as {target}`, relying on `encode_binary`/`encode_unary`
having already wrapped a Binary/Unary inner — pinned by the in-Rust `e1_cast_inner_paren` test
`(n - 1) as u8`) and the #146 outer-paren (`is_lt_leading`: `matches!(op, Lt|Le|Shl)`,
`encode_binary_operand` wraps a `Cast` left operand — in-Rust `e2_cast_lt_outer_paren`
`((x as u32) < 33)`) match Table 1B's discipline on the bounded side.

### 2C. Index + overflow framing — `encode_index` ↔ `ExecExpr.index` / `evalArith`

| Construct | Rust arm | Lean arm | Bridge | Pinned by |
|---|---|---|---|---|
| `xs[i]` (slice) | `encode_index` `IndexArg::Single(i)` over slice-bound base: `Ok(format!("{}[{idx}]", segments[0]))`; `idx = encode_index_value(i)` (`<p> as int`) | `execDenote .index slice idx => … if 0 ≤ iv.value ∧ iv.value < xs.length then some (.int (xs.get …)) else none` | `xs[i as int]` is the bounded i-th element; out-of-range → obligation `none` | `exec_ref_sound`; `slice_index_value_is_twenty` |
| range index | `encode_index` non-`Single` → `Err(Unsupported("slice-range … not a scalar"))` | (absent — `ExecExpr.index` is single only) | a sub-slice is not a scalar exec value | n/a (faithful) |
| non-slice base | `encode_index`: `Err(Unsupported("index over a non-slice base"))` | (absent) | OUT of frozen exec subset | n/a |

The overflow-obligation framing is the key 2A/2B match: Rust `exec_ref_value` emits bounded ops
whose Verus exec semantics carry the no-overflow VC; Lean `execDenote` returns `none` exactly there
(`evalArith` range check) — `add_overflow_has_no_value` + `encoder_agrees_on_overflow` pin both
sides agreeing.

---

## Table 3 — `thermite-tv/src/exec_stmt_encode.rs` ↔ `lean/Thermite/Exec/Stmt.lean`

`body_ref_state` threads a big-step environment (`Env = BTreeMap<String, Expr>`); Lean
`bodyRefState`/`refStmt` thread a `State` (an `ExecEnv` + an in-scope `scope` set). `body_ref_sound`
proves `bodyRefState = bodyDenote`. The per-RHS value at each position is delegated to
`execRefValue` (Table 2), exactly as Rust delegates to `exec_ref_value`.

### 3A. `thread_stmt` arms ↔ `refStmt` arms

| Construct | Rust arm (`thread_stmt`) | Lean arm (`refStmt`) | Bridge | Pinned by |
|---|---|---|---|---|
| `let n = rhs` | `Stmt::Let`: re-shadow guard `if env.contains_key(name) { Err }`; else `substitute(init,env); env.insert(name, …)` | `.letS name init => if st.scope name then none else do let v ← execRefValue init st.env; some ((st.setVar name v).bind name)` | a `let` binds the RHS substituted under the current env; re-shadow → `none` (encoder `Err`) | `body_ref_sound` (via `refStmt_eq_stmtDenote`) |
| re-shadow Err | `Err(Unsupported("re-shadowed binding …"))` | `if st.scope name then none` | flat env cannot hold two cells | `refStmt_eq_stmtDenote` (letS) |
| `n = rhs` (assign) | `Stmt::Assign`: target must be bare `Path[1]`; cell must be in env (`if !env.contains_key(&name) Err`); `substitute(value,env); env.insert` | `.assign name value => if st.scope name then do let v ← execRefValue value st.env; some (st.setVar name v) else none` | order-sensitive rebind of an in-scope cell; unbound → `none` | `refStmt_eq_stmtDenote` (assign); B2 `b2_mutation_order_matters` |
| non-scalar/non-bare target | `Stmt::Assign` non-`Path[1]` target → `Err(Unsupported("… indexed / field / projection … OUT"))` | (absent — `Stmt.assign` takes a `name : String`) | `xs[i]=e` is OUT (v2 sequence theory) | n/a (faithful absence) |
| unbound assign | `if !env.contains_key(&name) { Err }` | `else none` | malformed body | `refStmt_eq_stmtDenote` |
| expr-stmt | `Stmt::Expr(e)`: `let _ = substitute(e, env)?; Ok(())` (encode-and-discard) | `.exprS e => do let _ ← execRefValue e st.env; some st` | no state effect; surfaces a value error | `refStmt_eq_stmtDenote` (exprS) |
| `Stmt::If` | `Stmt::If { cond, then, else_ }`: `let mut then_env = env.clone(); thread_branch(then, &mut then_env)?; … else_env = env.clone(); …; let cell_names = env.keys().cloned().collect(); for name in cell_names { … compose Expr::If … }` | `.ifElse cond thenB elseB => do let c ← asBool (← execRefValue cond st.env); let branch ← (if c then refBlockThread thenB st else refBlockThread elseB st); some (st.restoreScope branch)` | branch on cond; recompose pre-`if` cells; discard branch-local `let` | `refStmt_eq_stmtDenote` (ifElse); see note below |
| `xs[i]=e` gate | (the non-bare-target `Err` above) | (absent) | `Unsupported` | n/a |
| early `return` (non-tail) | `Stmt::Return(_) => Err(Unsupported("early return in non-tail …"))` | (absent — no `Stmt.return`) | multi-exit CPS, OUT of v1 | n/a (faithful) |
| `Loop`/`Break`/`Continue` | each `=> Err(Unsupported("… step 2.2.2"))` | (absent — no loop `Stmt`) | loops kernel-gated (#163) | n/a (faithful) |

**The `Stmt::If` env.clone + env.keys() ↔ `restoreScope`.** Rust threads each branch into a CLONE
of the pre-`if` env (`let mut then_env = env.clone()`), then recomposes only the cells in
`env.keys()` (the pre-`if` cell set), so a branch-local `let` does not leak. Lean models this as
`State.restoreScope pre branch`:

```
def State.restoreScope (pre branch : State) : State :=
  { env := { vars := fun s => if pre.scope s then branch.env.vars s else pre.env.vars s
             slices := pre.env.slices }
    scope := pre.scope }
```

— an in-scope (pre-`if`) cell takes the branch's value, a non-pre-`if` name keeps the pre value, and
the post-`if` scope is the pre scope. This is precisely the Rust `env.keys()` recomposition. **This
arm was the #186 divergence**: an earlier Lean `ifElse` leaked branch-local scope; the ACToR loop
found it (`0256fd1c`), fixed it to match the `env.clone()` discipline (`6050b4cb`), and re-verified.
`StmtDivergence.lean` records the divergence as a kernel-checked artifact, not a `sorry`.

### 3B. The tail value + multi-cell projection ↔ `bodyRefState`/`bodyDenote`

| Construct | Rust arm | Lean arm | Bridge | Pinned by |
|---|---|---|---|---|
| tail value | `encode_block_tail`: thread stmts, then `Some(tail) => encode_value(tail, env)` | `bodyRefState b st = do let stf ← refBlockThread b st; match b.blkTail with \| some t => execRefValue t stf.env \| none => none` | tail evaluated in the final env | `body_ref_sound` |
| no tail | `None => Err(Unsupported("… no tail value …"))` | `\| none => none` | body-refinement compares a result value | `body_ref_sound` |
| if-expr tail | `encode_value` `Expr::If` arm: composes branch tails into `if {c} { {t} } else { {e} }` (with int-unify) | `bodyDenote`/`execDenote` if-expr (via `execRefValue`) — Lean models the if at the value level | both compose the taken branch | `b3_if_branch_taken` |
| tuple tail (multi-cell) | `body_ref_state_ensures` `Expr::Tuple` arm: per-projection `result.{i} == {cell}` conjunction | (Lean `bodyDenote` returns the threaded tail value; the multi-cell projection is the obligation SHAPE in `body_ref_state_ensures`) | each cell compared at the bounded type | partially — see Discrepancy D5 |

The B1–B4 in-Rust tests (`b1_let_chain_state`, `b2_mutation_order_state`, `b3_if_branch_state`,
`b4_multi_cell_tuple_state`) and the Lean B1–B3 theorems (`b1_let_chain_threads`,
`b2_mutation_order_matters`, `b3_if_branch_taken`) cross-check the same threading on both sides.

---

## The bridge assumptions (the inspection's own trust items)

This audit's correctness rests on three named assumptions — they are the residual trust the
inspection tier carries (the extraction tier would discharge A2/A3 mechanically; A1 is irreducible
on any tier that targets Verus text).

- **A1 (Verus expression semantics).** The emitted strings MEAN what we say under Verus's documented
  expression semantics — operator precedence, and the parenthesization discipline making the strings
  PARSE to the intended ASTs. The #122 inner-paren and #146 outer-paren are the load-bearing cases:
  the negative lemma `cast_paren_drop_breaks_soundness` proves the inner-paren is denotation-critical,
  and the #146 `is_lt_leading` outer-paren is a PARSE-SAFETY guarantee (without it `x as u32 < 33`
  mis-parses as a generic-arg list — a hard parse error in Verus and Rust, surfaced as
  "Unverifiable," not a wrong meaning). A1 is the irreducible Verus-target trust (the
  `thermite-semantics.md` reduced-trusted-base table item #1, the target semantics).
- **A2 (fuel ↔ Verus well-founded unfolding).** The Lean fuel index on `refDenote`/`refIntVal`
  /`denote` models Verus's well-founded `spec fn` unfolding (every Thermite spec fn carries a
  mandatory `dec` measure, §4.2 ⟹ termination ⟹ a well-founded fixpoint). The Rust `encode_call`
  case (3) does not inline (it emits a call), so the fuel models the MEANING of the recursive spec-fn
  definition, not a Rust recursion bound. The soundness is proved for ALL fuel and both sides share
  the fuel + registry, so it is fuel-uniform (the `Denote.lean`/`RefEncode.lean` headers state this;
  not a fuel-cap dodge).
- **A3 (symbolic closed-form env ↔ big-step transformer).** The Rust `exec_stmt_encode.rs` threads a
  symbolic `Env = BTreeMap<String, Expr>` of closed-form value EXPRESSIONS, and its module doc
  asserts this represents the big-step state transformer. The Lean `Stmt.lean` models the same
  threading as a `State` transformer over `ExecVal`s (`refStmt`/`refBlockThread`). The inspection
  trusts that the Rust symbolic substitution (`substitute`) and the Lean concrete threading denote
  the same transformer — `body_ref_sound` + the B1–B4 cross-checks are the evidence; the equivalence
  of "substitute-closed-forms-then-encode" vs "thread-concrete-values" is the assumption.

## What this inspection does NOT cover (honest residuals)

- **String-level formatting / whitespace.** The audit maps the emitted AST shape and tokens, NOT
  byte-exact strings. The Rust `encode_match` brace/indent layout, the if-tail
  `strip_one_enclosing_paren` cosmetic strip, and the int-unify `as int` coercion in
  `encode_value`'s if-arm are formatting choices Verus parses identically; they are not modelled in
  Lean (which works at the AST level). A formatting bug that still parses to the same AST would not be
  caught by Lean — but would be caught by the per-run Z3 TV (which sees the real string) and by the
  golden lowering files.
- **The production lowerer (`thermite-lower`).** NOT in scope. The whole architecture exists because
  the production lowerer is NOT verified — it is checked PER RUN by Z3 TV against the reference
  encoder. This doc audits the REFERENCE encoder ↔ its Lean model; the production lowerer ↔ reference
  link is the Z3 `h_tv` premise (`Faithfulness.lean`), not this inspection.
- **Loops (`while`/`loop`/`break`/`continue`, #163) — now covered ELSEWHERE, cross-referenced here,
  NOT absorbed into this doc's arm tables.** This doc's Tables 1–3 audit the STRAIGHT-LINE `S_B`
  fragment and remain loop-free; the v1 `while`-loop correspondence is a SEPARATE audit artifact and
  stays under its own authority. As of the #163 loop-TV arc, the Rust loop arm `loop_ref_obligations`
  (`thermite-tv/src/exec_stmt_encode.rs` @ `21b84c5f`) produces the three per-run reference pieces, and
  the Lean side proves the partial-correctness `while_rule` + its TV meta-theorem `tv_meta_loop`
  (`lean/Thermite/Exec/Loop.lean` @ `65504c18`). The correspondence between the three Rust obligations
  and the Lean `while_rule`/`tv_meta_loop` premises was fidelity-audited in the #163 ACToR arc (the
  loop-TV critic verified the Lean premises match the Rust obligations) and lives under the authority of
  **`.design/verified/loop-tv.md`** — named here as a cross-reference, deliberately NOT silently
  absorbed into Tables 1–3. The straight-line encoders still honestly `Err` on a loop OUTSIDE the v1
  frozen `while` subset; that residual is unchanged.
- **The Map accessor + Option/Result frame (#150).** `encode_map_accessor`
  (`contains_key`→`spec_contains_key`, `len`→`len`) and the `RefCtx` Map/Option frame are in the Rust
  encoder but NOT in the Lean `S_C` fragment (which covers Option/Result via `match`/`is` but not Map
  membership) — see Discrepancy D3.
- **Struct-field / tuple-projection access in contract position.** The `ref_encode.rs::encode`
  `Expr::Field` arm (`result.x` → `{r}.{name}`) and `Expr::TupleProj` arm (`result.0` → `{r}.{index}`)
  are live in the Rust encoder but have NO Lean `Expr` constructor — the Lean `S_C` fragment does not
  model member access. Reachable in a struct-field / tuple-projection `ens`; inspection-only (no T1
  theorem) — see Discrepancy D6.
- **The extraction-bridge tier (REQ-2, the named stronger closure).** A Lean→Rust extraction (or a
  Rust-side proof) would make the Rust encoder equal the Lean model BY CONSTRUCTION, discharging A2/A3
  and the inspection entirely. There is no Lean→Rust extraction tooling for this encoder shape today
  (the encoders are hand-written Rust producing Verus strings, not extracted from Lean). The
  audit-by-inspection tier is the accepted interim per `thermite-semantics.md` REQ-6 / the
  reduced-trusted-base table item #3.
- **Drift.** This doc pins the audited commits (above). Any edit to a pinned encoder file invalidates
  the corresponding table section and requires re-audit. **Recommended future work: a CI guard** that
  fails when `thermite-tv/src/{ref_encode,exec_encode,exec_stmt_encode}.rs` or
  `thermite-spec/src/combinators.rs` change without a matching update to the pinned SHAs here (a
  blocker-tracked enhancement, not a v0.1 kernel item).

## Discrepancies found

Every arm was checked. The correspondence is CLEAN at the denotation level for every arm in the
frozen subset. The items below are NOT meaning mismatches — they are places where the Lean model
ABSTRACTS a Rust string detail (sound), or where the Rust encoder covers MORE than the Lean fragment
(a residual, not a bug). They are recorded for honesty so a re-auditor knows exactly where the two
sides are not arm-for-arm identical.

- **D1 (the `Eq` nat-coercion string is abstracted, not modelled — SOUND).** Rust `encode_binary`
  emits `result as nat == spec_sum(xs)` (the `as nat` on the bounded operand, Eq-only,
  `is_nat_valued`-gated). Lean does NOT emit an `as nat` token in the `cmp` arm; it routes both
  operands through `refIntVal`/`castDenote` over the shared `Int` domain, where `castDenote nat` under
  the `≥0` source frame is the identity. The Lean model is therefore a SOUND ABSTRACTION of the Rust
  coercion (the meaning is the same int comparison), but it does NOT pin the Eq-ONLY gating or the
  `is_nat_valued` detection — those are pinned only by the Rust unit/teeth tests + the per-run Z3 TV,
  not by a Lean theorem. A re-auditor should treat the Eq-only coercion gating as inspection-only.
- **D2 (#146 outer-paren is a parse-safety property, not a Lean denotation theorem).** The Rust
  `is_lt_leading` outer-paren is pinned in Lean only indirectly (the cast value is the same; the paren
  prevents a mis-parse). There is no Lean lemma "dropping the #146 outer-paren breaks soundness"
  analogous to the #122 `cast_paren_drop_breaks_soundness`, because dropping it yields an UNPARSEABLE
  string (Unverifiable), not a wrong meaning. This is faithful (the in-Rust `e2_cast_lt_outer_paren`
  test pins it), but it lives under Bridge Assumption A1, not a Lean theorem.
- **D3 (Map membership is a Rust-encoder residual, not in Lean `S_C`).** `encode_map_accessor`
  (`contains_key`→`spec_contains_key`, `len`→`len`) was added by #150; the Lean `S_C` fragment models
  Option/Result (`match`/`is`) but NOT Map membership. So the Rust Map arm has NO Lean counterpart.
  This is a residual (the Lean fragment is a subset of what the Rust encoder admits), not a mismatch —
  the Map arm's correspondence is currently inspection-only (its faithfulness rests on the in-Rust
  `forge/tests/contract_tv_conformance.rs` `map_kv` corpus entry + per-run Z3, not a Lean theorem).
  A future Lean increment could embed Map membership.
- **D4 (guard arms / non-2-arm matches are Rust-encoder territory not modelled in Lean).** Rust
  `encode_match` emits a guard arm (`{pat} if {g} => {body}`); the Lean `MatchArm` has no guard, and
  `denoteArms` models exhaustive 2-arm `Some/None`/`Ok/Err` selection. The corpus matches are
  exhaustive 2-arm, so the modelled fragment matches the EXERCISED encoder behavior; the guard-arm
  path of `encode_match` is inspection-only (no corpus clause uses it, no Lean theorem pins it).
- **D5 (multi-cell tuple projection: the obligation SHAPE is Rust-only).** `body_ref_state_ensures`
  builds the per-projection `result.{i} == {cell}` conjunction for a tuple tail. Lean `bodyDenote`
  returns the threaded tail VALUE (including a tuple), proved sound by `body_ref_sound`; the
  per-projection obligation SHAPE (`result.0 == … && result.1 == …`) is the Rust obligation
  construction, cross-checked by the in-Rust `b4_multi_cell_tuple_state` test, not by a distinct Lean
  theorem. The state denotation (the cells' closed forms) IS pinned; the obligation packaging is
  inspection-only.
- **D6 (struct-field / tuple-projection ACCESS in contract position is a Rust-encoder residual, not
  in Lean `S_C`).** The `ref_encode.rs::encode` dispatch has two live arms that emit a Verus
  projection-access string and have NO corresponding Lean `Expr` constructor:

  ```rust
  Expr::Field { receiver, name } => {
      let r = encode(receiver, ctx)?;
      Ok(format!("{r}.{name}"))          // struct-field access  result.x
  }
  Expr::TupleProj { receiver, index } => {
      let r = encode(receiver, ctx)?;
      Ok(format!("{r}.{index}"))         // tuple projection     result.0
  }
  ```

  They emit the Verus member-access token `<receiver>.<name>` / `<receiver>.<index>` (the receiver
  recursively re-encoded). They are reachable in the frozen contract subset for a struct-field `ens`
  (`result.x == 0`) and a tuple-projection `ens` (`result.0 == a`) — the multi-cell/struct CONTRACT
  surface. (This is DISTINCT from D5: D5 covers the EXEC-BODY tuple obligation SHAPE built by
  `body_ref_state_ensures`, i.e. the `result.{i} == {cell}` conjunction the body-refinement emits;
  D6 covers these `ref_encode.rs::encode` CONTRACT-position projection arms.) The Lean `Expr`
  inductive (constructors `intLit/boolLit/var/cmp/logic/neg/arith/cast/seqVar/strVar/idx/subrange/`
  `seqLen/byteAt/comb/optResVar/match_/is_/specCall`, `lean/Thermite/Ast.lean`) has NO `field` or
  `tupleProj` constructor, so the Lean `S_C` fragment does not model struct-field / tuple-projection
  access at all. These two arms therefore have NO Lean counterpart and NO (T1) theorem backing —
  exactly like the Map accessor (D3) and the guard-arm path (D4): a Rust-encoder arm covering MORE
  than the Lean fragment, not a meaning mismatch.

  **Consequence.** A contract that uses `result.x` (struct field) or `result.0` (tuple projection)
  relies on the per-run Z3 translation validation alone for that arm, WITHOUT the Lean T1
  (`ref_sound`) backing the rest of the contract subset enjoys: the per-run `h_tv` attestation still
  checks the production lowering against this reference arm, but CORR for this arm rests on the
  inspection of the two quoted lines above (the emitted `{r}.{name}` / `{r}.{index}` IS the Verus
  member-access denotation), not on a kernel-checked Lean theorem.

  **Future brick.** Extending Lean `S_C` with a `field`/`tupleProj` constructor (a value projection
  over a struct/tuple `ExecVal`/`RefVal`, denoting member access) is a candidate next increment —
  the SAME shape as the proven rewrites (a structural arm with a `ref_sound` case + a negative lemma
  pinning the projection choice), which would promote D6 from inspection-only to T1-backed.

None of D1–D6 is a denotation mismatch. The arm-by-arm correspondence holds at the meaning level for
every construct in the frozen subset; D3/D4/D6 are Rust-encoder arms that lie OUTSIDE the Lean
fragment (inspection-only, no Lean counterpart), enumerated so a re-auditor knows exactly where the
two sides are not arm-for-arm identical.

## Verification

This doc is the audit artifact; its "verification" is the groundedness of every row (AC-2) and the
existence of the cited Lean theorems. The Lean spine builds clean and `sorry`-free at `65504c18`:

- `lake build` (Lean `v4.29.0` since the #184 probe; the audited spine builds core-only-equivalent) — the spine compiles; `#print axioms lowering_faithful`
  shows `{propext, Classical.choice, Quot.sound}` (standard).
- The cited (T1) theorems: `Thermite.ref_sound` / `ref_sound_eq` (Soundness.lean),
  `Thermite.Exec.exec_ref_sound` (Exec.lean), `Thermite.Exec.body_ref_sound` (Exec/Stmt.lean),
  composed in `Thermite.lowering_faithful` (Faithfulness.lean).
- The negative lemmas cited per row are theorems in Soundness.lean / Exec.lean (e.g.
  `eq_le_infidelity_breaks_soundness`, `cast_paren_drop_breaks_soundness`,
  `byteview_misdispatch_breaks_soundness`, `index_argkind_slice_view_breaks_soundness`,
  `match_arm_swap_breaks_soundness`, `is_wrong_variant_breaks_soundness`,
  `specfn_arg_order_breaks_soundness`, `count_where_wrong_pred_breaks_soundness`,
  `permutation_set_model_breaks_soundness`, `nat_coercion_underflow_breaks_soundness`).
- The Rust encoders' own teeth/unit tests: `thermite-tv/tests/{teeth,exec_teeth,body_teeth}.rs`
  (F1–F4 / E1–E4 / B1–B4 against real `verus`) and the in-module `#[test]`s pin the Rust output the
  Lean models mirror.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (the arm-by-arm correspondence map) | SHIPPED | This doc IS the deliverable. Every arm of `ref_contract_pred`/`exec_ref_value`/`body_ref_state` and the 8 combinator `verus_l3` forms is EITHER a row in Tables 1–3 OR an explicitly-listed out-of-Lean-scope residual (the `ref_encode.rs::encode` dispatch is enumerated exhaustively in Table 1H — its `Expr::Field`/`Expr::TupleProj` arms are residual D6, no Lean counterpart). Each row quotes the actual Rust arm (`thermite-tv/src/{ref_encode,exec_encode,exec_stmt_encode}.rs` @ `579d3d48`/`43c9a6c8`/`21b84c5f`; `thermite-spec/src/combinators.rs` @ `c0b1d8a3`) beside the actual Lean arm (`lean/Thermite/{RefEncode,Denote,Exec}.lean` + `Exec/Stmt.lean` @ `65504c18`), the Verus-meaning bridge, and the pinning Lean theorem + negative lemma. Bridge assumptions A1–A3 enumerated; residuals + discrepancies D1–D6 recorded honestly. Closes the `thermite-semantics.md` REQ-6 correspondence residual at the audit-by-inspection tier. |
| REQ-2 (the extraction bridge — Lean→Rust extraction or a Rust-side proof) | NOT-STARTED | open prereq blocker #185 (this doc's blocker tracks both tiers; the inspection tier is REQ-1 SHIPPED, the extraction tier stays open). Gap: there is no Lean→Rust extraction tooling for this encoder shape — the encoders are hand-written Rust producing Verus STRINGS, not Lean-extracted code, so the inspection (this doc) is the accepted interim per `thermite-semantics.md` REQ-6 / the reduced-trusted-base table item #3. The named stronger closure (extraction or a Rust-side proof making the Rust encoder equal the Lean model by construction) is future work; until then A2/A3 stay inspection-trusted and a CI drift-guard (recommended above) is the cheap interim hardening. |
