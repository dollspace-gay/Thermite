# Exporter Surface Correspondence — the arm-by-arm drift-tripwired contract

<!--
tier: 3-component
status: draft
governs: forge/src/lean_export.rs
audited-sha: 8978ecc950df30b58c00fe6df06f1fc5b4c56691 (re-pinned 2026-06-16 for stage-1 increment 2e, REQ-7: re-inspected the exporter surface for the new export_lemma. It reuses the EXACT tier-(a) fn-contract machinery (encode_expr + build_registry + R_item + the `Thermite.denote 0 … {v with specs := R_item}` framing) MINUS the body/result binding — a lemma is the pure `∀ params, req → ens` proposition with no body/result. The existing arms' correspondence is unchanged; the lemma goal is the same denote-framing the fn req/ens arms already certify, so no new soundness claim is introduced.)
audited-content-sha256: abfa33bb083d691b2450d202ac11276fb88c6b7659e5ba99411d635a6ddac506
             tone-pass that closed increment 0; increment 1 does NOT modify lean_export.rs,
             so this pin stays valid after the foundation commit)
thesis-refs:
  - thermite-design.md §4 (the pure-contract certification class; the hard gate; the
    frozen spine subset `S_C`/`S_B` the exporter targets)
  - thermite-design.md §4.1 (exec-body bridge; the `stateOf` env→State emission)
  - thermite-design.md §6.1 (the three-tier theorem: fuel-free auto tiers (a)/(b),
    stabilized interactive tier (c))
  - thermite-design.md §13 (roadmap; verified-microkernel convergence)
anchor-doc:
  - .design/verified/proof-backends.md REQ-6/REQ-7/REQ-10 (the exporter's top-level
    authority; the REQ status table there is the binding obligation record)
  - .design/verified/rust-lean-correspondence.md Table 4 / Table 4B (the earlier
    arm-by-arm tables this doc extends; pinned at forge/src/lean_export.rs @ 3373215e)
related-finding:
  - external-trust-audit.md F10 ("re-inspect the freshest code" — lean_export.rs is the
    freshest and largest trust surface; this doc is the durable form of that re-inspection)
  - external-trust-audit.md F3 (no mechanized extraction bridge — this doc does NOT close
    F3; see "Scope and limits" below)
epic: crosslink #240 (the exporter chain)
-->

## Summary

`forge/src/lean_export.rs` is the forge's front door: it serializes a checked Thermite item
into a self-contained Lean file that instantiates the kernel-proven spine
(`lean/Thermite/Ast.lean` + `Denote.lean` + `Stabilize.lean` + `Exec/Stmt.lean`). Nothing
in that serialization is verified by construction — the gap between "the Lean spine is
proven" and "the emitted Lean source faithfully targets the spine" is bridged by inspection.

This document makes that inspection rigorous and standing: an arm-by-arm correspondence
table for every piece of the exporter surface, drift-tripwired so that any future edit to
`lean_export.rs` fires the gate and demands a re-audit. It extends and supersedes
`.design/verified/rust-lean-correspondence.md` Table 4 / Table 4B for the exporter file,
which was pinned at the earlier `3373215e` commit; the pin here advances to the
current `b60b75a4` tone-pass commit.

## Scope and limits

This table is INSPECTION-TIER throughout. It does NOT close audit finding F3 (the
trust-audit's "no mechanized extraction bridge" finding). There is no Lean→Rust extraction
tooling for this encoder shape — `lean_export.rs` is hand-written Rust emitting Lean source
text, not code extracted from a Lean model. The correspondence between what the function
emits and what the spine's inductive constructors mean is discharged today by the
arm-by-arm inspection this document records, plus the live regression oracle suite
(`forge/src/engine.rs` / `forge/tests/lean_engine.rs` / `forge/tests/lean_while.rs`).

Until a mechanized bridge exists (an extraction or a Rust-side proof making the emitter
equal the spine by construction), F3 stays open and this document is the durable
audit-by-inspection record demanded by F10.

## Requirements

- **REQ-1 (the exporter surface correspondence)** — for every structural arm of the
  exporter (`encode_expr`, `build_registry`, `emit_theorem` tiers, `emit_state_of`,
  `ExportRefusal` inventory), exhibit the Rust source symbol, the targeted spine
  constructor or spine theorem, and the inspection basis (what makes the mapping correct).
  Derived from `thermite-design.md §4` (the exporter's faithfulness obligation, EXP).
- **REQ-2 (drift-tripwire discipline)** — the correspondence table is pinned to the
  audited commit of `lean_export.rs`; any subsequent edit fires `doc-drift.py` and
  requires re-audit before the edit can land. This is the standing enforcement of F10's
  "re-inspect the freshest code" finding.

## Acceptance criteria

- **AC-1** — every arm of `encode_expr` appears as a row OR as an explicit out-of-fragment
  residual, with no silent omissions.
- **AC-2** — every `ExportRefusal` variant appears in the inventory table.
- **AC-3** — `build_registry`'s hard gate is documented with its two independent directions
  (AST-side coverage check + `build_registry` closure check).
- **AC-4** — the three theorem tiers of `emit_theorem` (FuelFreeAuto, StaticUnfoldAuto,
  RecursiveInteractive) are documented with the spine theorems they rely on.
- **AC-5** — `emit_state_of`'s three param-sort branches and `scope := false` invariant
  are documented with the per-param correspondence `rfl`-lemma pattern.
- **AC-6** — this document exists, is drift-tripwired (the `audited-sha:` pin above), and
  the `doc-drift.py` gate reports CURRENT on `forge/src/lean_export.rs`.

---

## Table EXP-1 — `encode_expr` arms ↔ `lean/Thermite/Ast.lean` constructors

`encode_expr` maps each frozen-subset Thermite `Expr` arm to its `Ast.lean` constructor
term. An out-of-spine construct returns `ExportRefusal::OutOfFragment` (an honest skip).
The table extends `rust-lean-correspondence.md` Table 4 (the earlier EXP pin); the mapping
logic is unchanged from that audit — this table advances the pin and records the as-shipped
arms at `b60b75a4`.

| # | Thermite `Expr` arm | `encode_expr` output | Targeted `Ast.lean` constructor | Inspection basis |
|---|---|---|---|---|
| intLit | `Expr::IntLit { value, .. }` | `(Thermite.Expr.intLit {value})` | `Expr.intLit (value : Int)`; `intVal _ (intLit n) _ = n` | direct — the integer literal is emitted as-is; the spine defines `intLit` as the constant denotation |
| boolLit | `Expr::BoolLit(b)` | `(Thermite.Expr.boolLit {b})` | `Expr.boolLit`; `denote _ (boolLit b) _ = (b = true)` | direct |
| var | `Expr::Path([name])` (not in seq/string/optres frame) | `(Thermite.Expr.var {lean_str(name)})` | `Expr.var`; `intVal _ (var x) env = env.ints x` | direct — the name is sorted by the `EncodeCtx` coercion frame |
| seqVar | `Expr::Path([name])` (name in `ctx.seq_params`) | `(Thermite.Expr.seqVar {lean_str(name)})` | `Expr.seqVar`; `seqVal _ (seqVar x) env = env.seqs x` | direct — the coercion-frame sort: slice params resolve to `seqVar` |
| strVar | `Expr::Path([name])` (name in `ctx.string_params`) | `(Thermite.Expr.strVar {lean_str(name)})` | `Expr.strVar`; the String view | direct |
| optResVar | `Expr::Path([name])` (name in `ctx.optres_params`) | `(Thermite.Expr.optResVar {lean_str(name)})` | `Expr.optResVar`; `env.optres` lookup | direct |
| cmp | `Expr::Binary { op ∈ Eq/Ne/Lt/Le/Gt/Ge, lhs, rhs }` | `(Thermite.Expr.cmp Thermite.CmpOp.{eq,ne,lt,le,gt,ge} l r)` | `Expr.cmp`; `denote (cmp op a b)` | direct — same `CmpOp` map as Table 4/Table 1A |
| logic | `Expr::Binary { op ∈ And/Or, lhs, rhs }` | `(Thermite.Expr.logic Thermite.LogOp.{and,or} l r)` | `Expr.logic`; `denote (logic op a b)` | direct |
| neg | `Expr::Unary { op: UnaryOp::Not, expr }` | `(Thermite.Expr.neg {encode_expr(expr)})` | `Expr.neg`; `denote (neg e) = ¬ denote e` | direct (Prop operand; an integer bitwise-not has no spine arm → `OutOfFragment`) |
| arith | `Expr::Binary { op ∈ Add/Sub/Mul/Div/Rem/Shl/Shr/BitAnd/BitOr/BitXor }` | `(Thermite.Expr.arith Thermite.ArithOp.{add,…,bitXor} l r)` | `Expr.arith`; `intVal (arith op a b) = arithDenote op …` | direct — same `ArithOp` map as Table 4/Table 1A |
| cast | `Expr::Cast { expr, ty ∈ u64/u32/usize/nat/int }` | `(Thermite.Expr.cast {inner} Thermite.CastTy.{u64,u32,usize,nat,int})` | `Expr.cast`; `intVal (cast inner ty) = castDenote ty …` | direct — `encode_cast_target` maps the five target types; any other target → `OutOfFragment` |
| idx | `Expr::Index { base, IndexArg::Single(i) }` | `(Thermite.Expr.idx {b} {encode_expr(i)})` | `Expr.idx`; `seqIdx` | direct |
| subrange/rangeTo | `Expr::Index { base, IndexArg::RangeTo(hi) }` | `(Thermite.Expr.subrange {b} (Thermite.RangeArg.rangeTo {hi}))` | `Expr.subrange` + `RangeArg.rangeTo` | direct |
| subrange/range | `Expr::Index { base, IndexArg::Range(lo, hi) }` | `(Thermite.Expr.subrange {b} (Thermite.RangeArg.range {lo} {hi}))` | `RangeArg.range` | direct |
| subrange/rangeFrom | `Expr::Index { base, IndexArg::RangeFrom(lo) }` | `(Thermite.Expr.subrange {b} (Thermite.RangeArg.rangeFrom {lo}))` | `RangeArg.rangeFrom` | direct |
| seqLen | `Expr::MethodCall { name: "len", args: [] }` | `(Thermite.Expr.seqLen {recv})` | `Expr.seqLen` | direct |
| byteAt | `Expr::MethodCall { name: "byte_at", args: [i] }` | `(Thermite.Expr.byteAt {recv} {encode_expr(i)})` | `Expr.byteAt` | direct |
| other method | any other `MethodCall` | `ExportRefusal::OutOfFragment` | (absent) | faithful absence — `S_C` admits only `.len()` / `.byte_at(i)` |
| old(x) | `Expr::Call { callee: Path(["old"]), args: [Path([x])] }` | `(Thermite.Expr.var {lean_str("old(x)")})` | `Expr.var` (a free pre-state name) | direct — the `old(x)` pre-state name is a distinct free `var` |
| comb | `Expr::Call { callee: Path([name ∈ frozen set]), args }` via `encode_combinator` | `(Thermite.Expr.comb Thermite.CombName.{…} {seq} {seq2} {idx} {pred})` | `Expr.comb` + `CombName` + `Pred` | direct — eight frozen combinators (Table EXP-2); arity mismatch / non-single-param predicate → `OutOfFragment` |
| specCall | `Expr::Call { callee: Path([name]), args }` (non-combinator, non-`old`) | `(Thermite.Expr.specCall {lean_str(name)} [{args}])` | `Expr.specCall`; fuel-indexed `Registry`; `R_item` populated by the hard-gate closure | direct + the hard gate (Table EXP-3) guarantees the name is registered |
| match_/is_ | `Expr::Match { Some/None/Ok/Err arms }` / `Expr::Is { variant }` | `(Thermite.Expr.match_ {scrut} [{MatchArm.mk Variant binder body}])` / `(Thermite.Expr.is_ {scrut} {Variant})` | `Expr.match_` + `MatchArm` + `Expr.is_` + `Variant` | direct (Table 1F in rust-lean-correspondence.md); user-ADT variant / guarded arm → `OutOfFragment` |
| Ref(Index) | `Expr::Ref { expr: Expr::Index { .. } }` | routed to `encode_index` (the subrange form above) | same as idx/subrange | direct — a range borrow `&xs[lo..hi]` is the `subrange` form |
| Ref(other) | `Expr::Ref { expr: other }` | `encode_expr(other)` (identity on the value) | the inner expression's constructor | direct — a bare `&e` of a non-index passes through to the inner |
| **OUT-of-S_C residuals** | `Expr::Field` / `Expr::TupleProj` / `Expr::StructLit` / `Expr::Deref` / `Expr::StrLit` / `Expr::Tuple` / `Expr::If` / `Expr::Closure` (bare) / qualified `Path` | `ExportRefusal::OutOfFragment(desc)` | (absent — no Lean `Expr` constructor) | faithful absence — each carries a human description of the offending construct |

### Table EXP-2 — the eight frozen combinators ↔ `CombName`

| Combinator | `encode_combinator` match arm | `CombName` constructor | Inspection basis |
|---|---|---|---|
| `forall_in(s, p)` | `("forall_in", [s, p])` → seq=`s`, pred=`p` | `CombName.forallIn` | same frozen `verus_l3` as Table 1E; the `CombName.forallIn` denotation is `∀ i, (0≤i ∧ i<s.length) → p i` |
| `exists_in(s, p)` | `("exists_in", [s, p])` → seq=`s`, pred=`p` | `CombName.existsIn` | `∃ i, (0≤i ∧ i<s.length) ∧ p i` |
| `count_where(s, p)` | `("count_where", [s, p])` → seq=`s`, pred=`p` | `CombName.countWhere` | `countWhereVal` |
| `sorted(s)` | `("sorted", [s])` → seq=`s` | `CombName.sorted` | `∀ i j, (0≤i ∧ i≤j ∧ j<s.length) → seqIdx s i ≤ seqIdx s j` |
| `disjoint(a, b)` | `("disjoint", [a, b])` → seq=`a`, seq2=`some b` | `CombName.disjoint` | `∀ i j, … seqIdx a i ≠ seqIdx b j` |
| `permutation_of(a, b)` | `("permutation_of", [a, b])` → seq=`a`, seq2=`some b` | `CombName.permutationOf` | `permEq a b` (multiset equality) |
| `forall_below(s, n, p)` | `("forall_below", [s, n, p])` → seq=`s`, idx=`some n`, pred=`p` | `CombName.forallBelow` | `∀ i, (0≤i ∧ i<n ∧ i<s.length) → p i` |
| `forall_from(s, n, p)` | `("forall_from", [s, n, p])` → seq=`s`, idx=`some n`, pred=`p` | `CombName.forallFrom` | `∀ i, (n≤i ∧ i<s.length) → p i` |

Predicate encoding: `encode_pred` in `encode_combinator` — a `Expr::Closure { params: [x], body }` →
`(some (Thermite.Pred.mk {lean_str(x)} {encode_expr(body, inner_ctx)}))` where `inner_ctx` binds `x`
as an integer name. A non-single-param closure → `OutOfFragment`.

---

## Table EXP-3 — `build_registry` hard gate ↔ `R_item` population

`build_registry` in `lean_export.rs` is the §4 hard gate. Its faithfulness is in two
independent directions, both enforced before any emission.

| Gate direction | Rust mechanism | Spine target | Inspection basis |
|---|---|---|---|
| (i) Undefined-callee gate (`collect_all_call_names`) | Collects every spec-call position callee in `req ∪ ens ∪ body ∪ dec` WITHOUT the `decls.contains_key` filter (the Pin G fix). Any name with no in-program definition is an `undefined` name → `ExportRefusal::IncompleteRegistry(undefined)` BEFORE emission. | `Thermite.Registry = String → Option SpecFn`; an undefined callee would bottom to the `intVal` Int-`0` and self-certify | The two-direction check: `collect_all_call_names` (unfiltered) feeds the `undefined` set; `build_registry`'s own `missing` check catches the obligation's `called` closure omissions |
| (ii) Called-but-omitted gate (`build_registry` `missing`) | For every name in the obligation's `called` closure, `build_registry` checks `decls.contains_key`. A name in `called` with no in-program definition → `ExportRefusal::IncompleteRegistry(missing)` | Same as (i) — a reachable-but-unregistered call bottoms to `0` | The Pin B/C/E/F mirror: the bottom-poisoning discharge is unreachable because the export refuses before emission |
| `R_item` body faithfulness | For each registered name, `encode_expr` encodes the spec-fn's REAL body tail expression arm-by-arm (not a summary) via `spec_fn_body_expr` → `encode_expr(body_expr, ctx)`. A wrong body → an unsound certification the gate cannot catch | `R_item "f" = some ⟨[params], real_body_term⟩`; `Env.specs "f" = some (SpecFn.mk params body)` | EXP body-faithfulness (§4): every `R_item` entry binds the REAL Denote-encoded body, not a placeholder |
| Per-name `decide` resolution lemma | `lemmas.push(format!("example : R_item {} ≠ none := by decide", lean_str(name)))` — emitted alongside the `R_item` def | `R_item "f" ≠ none` — kernel-checked; if the exporter ever omits a called spec-fn this lemma fails to compile | §4 mechanism 2 — a compile-time tripwire that a missing arm makes visible to the kernel |
| Independent re-check (Pin G fix, `emitted_spec_call_names`) | After emission, `emitted_spec_call_names(&theorem)` scans the emitted Lean text for `Expr.specCall "NAME"` occurrences and demands each `NAME ∈ registry_names`. Independent of the AST-side gate: inspects the bytes handed to the kernel | Same as (i) — a `specCall` in the goal whose name is not registered would bottom | The Pin G blind-spot fix: a future encoder bug that emitted a `specCall` for a name the AST gate never saw is caught here |

---

## Table EXP-4 — `emit_theorem` three tiers ↔ spine theorems

`emit_theorem` in `lean_export.rs` is the §4/§6.1 form.

| Tier | `ExportTier` variant | `tier_of` condition | Emitted theorem shape | Spine theorems consumed | Inspection basis |
|---|---|---|---|---|---|
| (a) Fuel-free auto | `FuelFreeAuto` | `req`/`ens`/`body` all specCall-free (no `Expr.specCall` in any expression) | `theorem T (v : Thermite.Env) : Thermite.denote 0 req {v with specs:=R_item} → Thermite.denote 0 ens ((…).bindInt "result" (Thermite.intVal 0 body …)) := by {auto_tactic_battery}` | `Thermite.stabilizes_iff_intVal_zero` / `Thermite.stabilizesProp_iff_denote_zero` (Stabilize.lean): for a specCall-free `e`, `stabilizesProp e env ↔ denote 0 e env` — the fuel-free goal is equivalent to the §4 stabilized form | The `intVal 0`/`denote 0` shapes are the definitions applied at fuel 0; the spine corollaries prove equivalence to the stabilized form for the non-recursive case |
| (b) Static-unfold auto | `StaticUnfoldAuto` | spec-calls present, `registry_is_recursive` false (finite DAG) | Same shape as tier (a) but the expressions passed to `emit_theorem` are the `unfold_spec_calls`-substituted versions (specCall-free after unfolding) | Same as tier (a) — the unfolded exprs are specCall-free, so the same corollaries apply | `unfold_spec_calls` iterates up to `decls.len() + 1` times, substituting each spec-fn call with its body arm-by-arm (EXP: the unfolded `Expr` equals the spec-fn's real body substituted — a wrong unfolding is a soundness gap the EXP inspector catches). Terminates because the registry is a finite DAG (tier (b)'s precondition) |
| (c) Recursive-registry interactive | `RecursiveInteractive` | `registry_is_recursive` true (a cycle in the call graph) | `theorem T (v : Thermite.Env) (r : Int) : Thermite.stabilizes body {v with specs:=R_item} r → Thermite.stabilizesProp req … → Thermite.stabilizesProp ens (….bindInt "result" r) := by sorry -- INTERACTIVE` | `Thermite.stabilization_exists` (the `RegistryTerminating` hypothesis); `Thermite.stabilizes_unique` | The `sorry` marks this as an interactive skeleton; the engine returns `Unknown("interactive-only")` and does NOT invoke lake for tier (c). The per-env `∃N` witness needs induction, not the auto battery |

The auto tactic battery (`auto_tactic_battery`): `intro hreq; simp only [Thermite.Env.bindInt, …]; first | decide | omega | simp_all | exact hreq | (revert hreq; decide) | (revert hreq; omega)`. This is the §6.1(a)/(b) z3-demotion battery; a closed-form QF goal is `decide`d, a linear-arith goal falls to `omega`.

`tier_of` detects tier (a) by checking `any_spec_call` across all four expression positions (`req`, `ens`, `body`, `dec`); if none, tier (a). If spec-calls are present, `registry_is_recursive` does a DFS cycle detection over the call graph (`callees` via `collect_block_calls`/`collect_expr_calls`) — a back edge is a cycle (tier (c)); otherwise tier (b).

### The `result` binding mode (REQ-6a, increment 2d — anti-Goodhart defense (a))

`emit_theorem` takes a `ResultMode` (`forge/src/lean_export.rs`). The shipped mode is
`BodyDenotation` — the auto-tier rows above, where `result` binds to the body's stabilized
value `(Thermite.intVal 0 body {v with specs:=R_item})`. The `Arbitrary` mode
(`export_arbitrary_result_harness`) is identical EXCEPT it adds a fresh `(r : Int)` theorem
binder and binds `result` to that `r`, dropping the body denotation:

| mode | theorem (auto tiers (a)/(b)) | correspondence / soundness |
|---|---|---|
| `BodyDenotation` (shipped) | `… .bindInt "result" (Thermite.intVal 0 body {v with specs:=R_item})` | the rows above — `result` is what the body computes |
| `Arbitrary` (REQ-6a) | `theorem T (v : Thermite.Env) (r : Int) : … → Thermite.denote 0 ens (….bindInt "result" r) := by {battery}` | the goal is `∀ r, req → ens@r` — the `ens` for an ARBITRARY result. If it kernel-accepts, the `ens` holds independent of the body → a body-ignoring tautology, which `check::gate_arbitrary_result_tautology` REJECTS (`SemanticTautology`). NOT a certification obligation — a tautology DETECTOR; the existing tiers' soundness correspondence is unchanged, since the `Arbitrary` goal is strictly WEAKER (∀ r) than the real obligation (`r` = the body value), so its acceptance is sound to read as "the contract over-claims nothing about the result". The Lean counterpart of `vacuity_solver.rs::build_tautology_harness`'s arbitrary `result` proof-fn param. |

`Arbitrary` mode is the pure-contract auto path only in increment 2d; a straight-line/while-body item is a structured `OutOfFragment` skip (the result-substitution on `bodyDenote`/`stateOf` is a residual). The battery, `R_item`, and `req`/`ens` encoding are byte-identical to the real obligation — only the `result` binder differs.

---

## Table EXP-5 — `emit_state_of` env→State emission ↔ `Exec.State`

`emit_state_of` emits `def stateOf (v : Thermite.Env) : Thermite.Exec.State` — the param-to-cell
correspondence that bridges the S_C free-variable env to the S_B operational state.

| Param sort | `emit_state_of` branch | Emitted cell | Per-param correspondence lemma | Inspection basis |
|---|---|---|---|---|
| Int scalar (`u32`/`u64`/`usize`) | `exec_int_ty(&p.ty) = Some(ctor)` | `vars s = if s = {ls} then Exec.ExecVal.int ⟨{ctor}, v.ints {ls}⟩` | `example (v : Thermite.Env) : Exec.asInt ((stateOf v).env.vars {ls}) = some ⟨{ctor}, v.ints {ls}⟩ := rfl` | The `rfl`-lemma is the §4.1.4 compile-time tripwire: if the cell encoding drifts from the `asInt` expectation, `rfl` fails to elaborate at kernel-check time |
| Bool scalar | `param_scalar_kind(&p.ty) = ScalarKind::Bool` | `vars s = if s = {ls} then Exec.ExecVal.bool (v.bools {ls})` | `example (v : Thermite.Env) : Exec.asBool ((stateOf v).env.vars {ls}) = some (v.bools {ls}) := rfl` | same `rfl` discipline |
| Slice (`&[uW]`) | `slice_elem_ctor(&p.ty) = Some(elem_ctor)` | `slices s = if s = {ls} then (v.seqs {ls}).map (fun n => ⟨{elem_ctor}, n⟩)` | `example (v : Thermite.Env) : ((stateOf v).env.slices {ls}).map Exec.BVal.value = v.seqs {ls} := rfl` | The map unwraps the `BVal` wrapper back to the integer sequence; `rfl` confirms the round-trip |
| `scope := fun _ => false` | always (no branch) | `scope := fun _ => false` | (no rfl-lemma for scope; verified by the EXP `scope := false` faithfulness argument: a body `assign` to a param is `none` on both the spine side (`scope p = false` → `Stmt.assign` checks `scope name` → `none`) and the reference encoder side (`body_ref_state`'s empty initial env); the `PinExecStateMisMap.lean` pin records this) | The `inputState` exemplar: params are free inputs, not `let`-bound cells; a body `assign` to a param is `none`/`Err` on both sides, matching the spine's `Stmt.assign` guard |

`InRangeParams` predicate: for each int param, `(0 ≤ v.ints {ls} ∧ v.ints {ls} < {ctor}.bound)`; for each slice param, `(∀ n ∈ v.seqs {ls}, 0 ≤ n ∧ n < {elem_ctor}.bound)`. The `emit_body_theorems` caller includes this as the first hypothesis of the HYPOTHESIZE theorem, scoping the well-typedness premise.

---

## Table EXP-6 — `ExportRefusal` inventory

Every `ExportRefusal` variant is an honest skip — the `LeanEngine` maps it to `Unknown` (a
skip), never to `Proven`/`Refuted`.

| Variant | Trigger | Consumer mapping | Inspection basis |
|---|---|---|---|
| `OutOfFragment(String)` | An `Expr` construct outside the frozen `S_C`/`S_B` spine subset (Field / TupleProj / StructLit / Deref / StrLit / Tuple / bare If-expr / bare Closure / qualified Path / unsupported cast target / unknown method / combinator arity mismatch / capture-unsafe unfolding substitution) | `LeanEngine::discharge` → `Unknown("out-of-fragment")` | The catch-all honest boundary — every construct NOT in the frozen subset is a structured refusal, never a silent wrong encoding |
| `NotPureContract(String)` | A boundary fn (no in-language body), or a spec-fn body that is not a single pure tail expression; today these are routed to the exec-body or while-body exporter so the only remaining triggers are boundary fns and spec-fns with statement bodies | `LeanEngine` → `Unknown` | §4 scope: the pure-contract class requires a single tail `Expr`; statement bodies route to the exec exporter |
| `IncompleteRegistry(Vec<String>)` | (i) An undefined callee (`calledSpecFns ⊄ dom(declared)`) — Pin G; (ii) a defined name omitted from the obligation's `called` closure — Pin B/C/E/F; (iii) a post-emission re-check finding a `specCall` in the theorem whose name is not in `registry_names` — the independent bytes-level gate | `LeanEngine` → `Unknown` (the bottom-poisoned discharge is unreachable — the export refuses before emission) | The hard gate's three independent sub-checks; carries the missing name(s) for diagnostics |
| `NonIntResult(String)` | A pure-contract fn/spec-fn whose declared result type is not an integer sort (`u32`/`u64`/`usize`/`int`/`nat`); `bool`/unit/ADT would bottom `intVal` to `0` and make a contract and its negation both certify | `LeanEngine` → `Unknown` (the Pin H result-sort gate — an `-> bool` item routes to the exec exporter instead; only spec-fns and unsupported ADT results land here) | §4 pure-contract scope: `intVal`-denoting bodies only |
| `OpenHole(String)` | An `Item::Fn` carrying `f.holes.is_empty() = false` — a body with an unresolved `?N` hole | `LeanEngine` → `Unknown` | §8 OUT set: an open hole is short-circuited at L0 before any engine |
| `LoopBody(String)` | A `Stmt::Loop`/`Break`/`Continue`/mid-body `Stmt::Return`/non-scalar `Stmt::Assign` target in a straight-line body (§4.1.7 — `S_B` mechanizes NO loop form for the straight-line exporter; while bodies recognized by `recognize_while_body` are routed to the while-body exporter before this) | `LeanEngine` → `Unknown("loop-body")` | `encode_exec_stmt` returns `LoopBody` for each of these; the while-body exporter (`export_while_body`) handles the `recognize_v1_loop` class |
| `OptResResult(String)` | A straight-line body whose declared result type is `Option<_>` or `Result<_,_>` — `ExecVal` has no optres variant; blocker #254 | `LeanEngine` → `Unknown` | §4.1.3 — the spine extension (an `ExecVal` optres variant) is the filed follow-on; until then an optres-typed straight-line body is a structured skip |

---

## Verification

This doc is the inspection artifact; "verification" is the groundedness of every row (each
row quotes the actual Rust source symbol and the actual spine target, not a paraphrase) and
the live regression oracle suite.

**Drift tripwire.** `python3 tooling/doc-drift.py` must exit 0 (CURRENT) on
`forge/src/lean_export.rs` against the `b60b75a4` pin. If it exits 1, the pin is stale
and this table requires re-audit before the edit can land.

**Live oracle suite (the Rust mirror of the kernel-checked pins).** The four spine critic
pins (`PinIntBottom`/`PinStabilization`/`PinBodyRegistry`/`PinDecMeasure`/`PinRegistryTerminating`)
gate the SPINE; the exporter's mirror is:

- `forge/src/engine.rs::live_scalar_correct_contract_is_proven` — a CORRECT contract
  kernel-accepts.
- `engine::tests::live_wrong_contract_is_unknown_never_refuted` — a WRONG contract yields
  `Unknown`, NEVER `Refuted`.
- `engine::tests::omitted_registry_obligation_refuses_export` — the Pin B/C/E/F mirror:
  bottom-poisoned discharge is unreachable because the export refuses.
- `engine::tests::recursive_registry_is_interactive_unknown` — a recursive registry is
  tier-(c) interactive.
- `forge/tests/lean_engine.rs::live_spine_elaborates_emitted_shape_correct_proves_wrong_fails` —
  live `lake env lean` kernel check of the emitted shape.
- `engine::tests::live_straight_line_body_is_proven` — a straight-line int body
  kernel-accepts including the OVERFLOW conjunct.
- `engine::tests::live_bool_result_body_is_proven_via_bindbool` — a bool-result body
  kernel-accepts via the `bindBool` bridge.
- `engine::tests::live_always_overflow_body_is_not_proven` — an always-overflow body's
  vacuous `ens` is NOT `Proven` because the OVERFLOW conjunct fails
  (PinExecOverflowVacuity mirror).
- `engine::tests::while_body_item_refuses_export` — a while body refuses (`LoopBody`).
- `engine::tests::optres_result_item_refuses_export` — an optres result refuses.
- `forge/tests/lean_while.rs::count_certifies_l3_via_lean_auto` — the L1 linear count
  family certifies via lean-auto (all 5+2 obligations kernel-accept).

---

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (the exporter surface correspondence) | SHIPPED | This doc IS the deliverable. Tables EXP-1 through EXP-6 enumerate every `encode_expr` arm (with explicit OUT-of-S_C residuals), every `ExportRefusal` variant, `build_registry`'s two-direction hard gate, the three `emit_theorem` tiers with their spine theorems, and `emit_state_of`'s three param-sort branches. Each row quotes the actual Rust source symbol (`pub fn encode_expr in lean_export.rs`, `fn build_registry in lean_export.rs`, `fn emit_theorem in lean_export.rs`, `fn emit_state_of in lean_export.rs`, `pub enum ExportRefusal in lean_export.rs`) beside the targeted spine constructor or theorem. Non-test consumer: `engine::LeanEngine::{fragment,discharge,admits_auto}` calls `export_item` (which calls all of the above) on the live discharge path. Inspection tier — see "Scope and limits." |
| REQ-2 (drift-tripwire discipline) | SHIPPED | `audited-sha: b60b75a49a3d8de99a4b7ed98fe42124e1b808fb` in this doc's header; the `forge/src/lean_export.rs` route in `tooling/spec-routes.toml` points at this doc; `python3 tooling/doc-drift.py` exits 0 (CURRENT). Any future edit to `lean_export.rs` fires the gate and demands re-audit. |
