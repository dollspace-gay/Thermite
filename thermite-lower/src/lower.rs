//! L3 emission: lower a validated `thermite-syntax` `Program` to a single
//! Verus-annotated Rust source `String` whose `requires`/`ensures`/`invariant`/
//! `decreases` annotations ARE the Thermite contract and whose body is the
//! lowered Thermite body. Forge (#5/#6) hands the emitted file to the `verus`
//! binary; a `0 errors` result is the L3 certificate
//! (`.design/lower/verus-lowering.md`; `thermite-design.md` §3/§4.1/§4.2/§6).
//!
//! Governing design: `.design/lower/verus-lowering.md`.
//! Reference (verus-verified, hand-authored): `tests/golden/lower/sum.verus.rs`,
//! `tests/golden/lower/binary_search.verus.rs`.
//!
//! ## Two lowering contexts (the central finding, REQ-5)
//!
//! Verus distinguishes EXEC code (`fn` bodies) from SPEC code
//! (`requires`/`ensures`/`invariant`/`decreases` and `spec fn` bodies). The same
//! Thermite expression lowers differently per context: a `&[T]` slice `xs` is
//! plain `xs` in exec position but `xs@` (a `vstd` `Seq<T>`) in spec position;
//! `xs[i]` is `xs[i]` in exec but `xs@[i as int]` in spec; `&xs[..i]` is
//! `&xs[..i]` in exec but `xs@.subrange(0, i as int)` in spec. A `spec fn` over a
//! slice takes `Seq<T>` (NOT `&[T]`) and recurses on `xs.drop_first()`
//! (verus-lowering.md REQ-5; the naive `&[u32]` spec-fn form fails `verus`).
//!
//! ## Proof aids are SHAPE-keyed, never program-keyed (REQ-7)
//!
//! Where a corpus program does not verify from its bare annotations, the lowerer
//! derives the needed proof aids from the program's AST/contract SHAPE — never
//! from its identity (no `if name == "binary_search"`). The shape keys are
//! documented at each template's emission site (`push_lemma_for`,
//! `nonlinear_overflow_assert`, `lift_immutable_preconds`, `extensionality_at_exit`,
//! `complementary_coverage_split`). This is the load-bearing honesty boundary
//! (`goal.md` "THE HONEST MANDATE", R-DEFER-9).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (file frame + `fn`/`spec fn` signature) | SHIPPED | `lower` emits the `use vstd::prelude::*; verus! { .. } fn main() {}` frame; `lower_fn`/`lower_spec_fn`; verified by `lower_conformance::sum_emitted_verifies`. |
//! | REQ-2 (type lowering) | SHIPPED | `lower_type`; consumer `lower_fn`/`lower_spec_fn`; asserted by `corpus_node_substrings`. |
//! | REQ-3 (expression lowering — exec) | SHIPPED | `lower_expr` with `Ctx::exec()`; consumer `lower_block`; asserted by corpus verification. |
//! | REQ-4 (statement + loop lowering) | SHIPPED | `lower_stmt`/`lower_loop` emit every `inv`→`invariant` + `dec`→`decreases`; consumer `lower_block`. |
//! | REQ-5 (spec-context `Seq` lowering) | SHIPPED | `lower_expr` with `Ctx::spec_seq()` (`xs@`/`subrange`/`@[i as int]`); `spec_sum` recursion via `lower_spec_fn` Seq form. |
//! | REQ-6 (combinator Verus(L3) defs) | SHIPPED | `emit_combinator_defs` reads `thermite_spec::CombinatorSig.verus_l3`; closes OQ-2 (R-DEFER-1 consumer of the #2 registry seam). |
//! | REQ-7 (proof-aid emission, shape-keyed) | SHIPPED | `push_lemma_for`/`nonlinear_overflow_assert`/`lift_immutable_preconds`/`extensionality_at_exit`/`complementary_coverage_split`; each keys on AST/contract shape, documented at site. |
//! | REQ-8 (golden-file contract — VERIFY) | SHIPPED | emitted output run through real `verus` in `lower_conformance.rs`; contracts asserted equivalent to the corpus (no weakening). |
//! | REQ-9 (`LowerError`, no panics) | SHIPPED | `enum LowerError` (span-bearing, `Display`); `lower` returns `Result`; no `unwrap`/`expect`/`panic!` in this file. |
//! | REQ-EQ (equivalent-mutant equivalence-obligation seam, #101) | SHIPPED | `pub fn lower_equivalence_obligation` (`.design/forge/equivalent-mutants.md` REQ-1 / `.design/lower/verus-lowering.md` REQ-EQ) renders `f`'s real body + a survivor mutant's body into the GROUNDED `spec fn equiv_real_<n>`/`spec fn equiv_mut_<n>` + `proof fn equiv_check_<n> requires <req> ensures mut == real {}` Verus unit, REUSING `lower_expr` + the design-grounded `(expr) as <ret>` exec coercion (`render_body_as_spec_value`/`coerce_obligation_expr`) — no hand-emitted Verus (R-CHAR-3). SCALAR-only (`scalar_obligation_type`, OQ-1): a non-scalar/non-forced-output shape returns `LowerError::Unsupported` so the survivor stays counted. Consumer: `forge::check::equivalence_proves_equal`. Verified: `thermite-lower/tests/equivalence_obligation.rs` (real verus). |
//!
//! ## #52 §9 boundary-composition arm (`.design/lower/boundary-composition.md`)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | composition REQ-1 (assumable-signature emission, boundary/slag only) | SHIPPED | `lower_fn` dispatches a `f.boundary.is_some() \|\| f.slag.is_some()` fn to `lower_external_body_fn`, which emits `#[verifier::external_body]` + the SAME `lower_fn_signature` (unweakened `requires`/`ensures`) + a synthetic `{ unimplemented!() }` body verus never checks. THE HONESTY GATE: external_body iff the syntactic `#[boundary]`/`#[slag]` flag — a regular fn ALWAYS takes the fully-proved-body arm. The 2-bool decision is DELEGATED to the Verus-verified `thermite_verified::should_emit_external_body` (epic #60, REQ-9 / `.design/verified/self-verification.md` Target C, mechanism (c)): its `ensures` proves the disjunction AND the §9 corollary `(!boundary && !slag) ==> !r`, anchored by the OBSERVABLE-dispatch test `tests/boundary_gate_verified.rs` (the emitted `#[verifier::external_body]` substring over the 4 (boundary,slag) combos). Consumer: `forge::check::item_subprogram` weaves a boundary/slag dep through this arm. Verified: `forge`'s `composition_conformance::direct_boundary_caller_verifies_through_the_contract` (caller L3) + `lying_regular_fn_is_caught_never_laundered_to_l3` (a regular lie is CAUGHT — verus `postcondition not satisfied`). The `#[verifier::external_body]` lives in the lowered verus STRING (a generated foreign-fn artifact), never in this `.rs` source. |
//!
//! ## Basis Stage 1c ADT arm (`.design/basis/01-adts.md`)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-8 (struct → Verus struct; type-invariant → enforced predicate) | SHIPPED | `lower_struct` emits a `pub struct` + `pub` fields + `impl { pub open spec fn well_formed(&self) -> bool { <inv with self.field> } }` (`lower_inv_expr`); OQ-3 RESOLVED — automatic threading: `lower_fn_signature` weaves `<param>.well_formed()` / `result.well_formed()` into `requires`/`ensures` for every invariant-bearing struct param/return (`inv_structs`). The `pub` visibility tier is the recorded grounding finding (a `pub open spec fn` body needs `pub` struct+fields). Consumer: `lower` (`Item::Struct` arm) + `lower_fn`. Verified: real verus `1 verified, 0 errors` on the emitted `bank_account` lowering, the cert oracle (`conformance/bank_account.cert.json` L3/pure/non-vacuous) — `tests/adt_lower_conformance.rs::bank_account_lowers_struct_invariant_and_verifies_l3` + `deposit_matches_cert_oracle_stable_subset`. |
//! | REQ-9 (enum → Verus enum; `match` → Verus `match`; `is` → variant test) | SHIPPED | `lower_enum` emits a Verus `enum` (unit/tuple/struct variants); `lower_match`/`lower_pattern` emit ENUM-QUALIFIED arms via the program `variants` map (`qualify_variant_path`) incl. `Pattern::Struct` (`Rect { w, h }`/`..`); `Expr::Is` → the Verus-native `(s is Circle)` discriminant. Consumer: `lower` (`Item::Enum` arm) + `lower_fn`. Verified: real verus `1 verified, 0 errors` on the emitted `shape` lowering + the cert oracle (`conformance/shape.cert.json` L3/pure/non-vacuous) — `shape_lowers_enum_match_is_and_verifies_l3` + `is_circle_matches_cert_oracle_stable_subset`. |
//! | REQ-10 (recursive type → Verus recursive enum; `Box`; structural `decreases`) | SHIPPED | `lower_enum` emits `Cons(u64, Box<List>)` (`lower_type` `Type::Box`→`Box<…>`); a `spec fn` matching the ADT-fold-sum shape (`is_adt_fold_sum`) lowers `-> nat` with `decreases l` over the datatype VALUE (Verus's built-in structural order) and `Expr::Deref`→`*t`, casts coerced `as nat` (`Ctx::nat_ret`). Consumer: `lower` (`Item::SpecFn`/`Item::Enum`). Verified: real verus `1 verified, 0 errors` on the emitted `list_sum` lowering — `list_sum_lowers_recursive_box_and_verifies_l3`. |
//! | REQ-11 (`LowerError`/no panics) | SHIPPED | the ADT arms reuse the existing `LowerError` (`Unsupported`/`TooDeep`); no new variant needed (the validator #65 owns the reject cases); no `unwrap`/`expect`/`panic!` added. Verified: `cargo clippy --workspace -D warnings` + the anti-pattern-gate. |
//! | REQ-12 (handled-or-loud — compile-time tooth) | SHIPPED | the exhaustiveness mechanism it names is the #65 validator (`SpecError::NonExhaustiveMatch`); this stage's L3/L1 lowering of the accepted `match` preserves it (every arm is emitted; a non-exhaustive match never reaches the lowerer). No regression to the compile-time tooth. |
//!
//! ## REQ status — 02-recursion-schemes.md (Basis Stage 2c, issue #70)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-6 (scheme → generated Verus recursive `spec fn` + `decreases <value>`) | SHIPPED | `emit_scheme_defs` (called from `lower` after `emit_combinator_defs`) GENERATES, per (ADT, scheme) the program uses (`collect_scheme_uses` resolving the scrutinee path → the param's `enum` type), the recursive `spec fn fold_<e>`/`for_all_<e>`/`map_<e>`/… (`emit_scheme_spec_fn`, `decreases l`, `*tail` deref, `Box::new` for `map`) reusing the Stage-1 recursive-fold shape. A scheme CALL lowers (`lower_expr` `Call` arm → `lower_scheme_call`) to a call of the generated `fold_<e>` with the step `Expr::Closure` lowered to a TYPED Verus `spec_fn` (`lower_step_closure`: `|x: u64, acc: nat| (<body>) as nat` for `fold`, `|x: u64| <body>` for `for_all`). Consumer: `lower`. Verified: real `verus --no-cheating` `verified, 0 errors` on the emitted `list_fold.th` (len_list/sum_list/all_positive → L3) — `tests/adt_schemes_conformance.rs::list_fold_lowers_to_generated_schemes_and_verifies_l3`. |
//! | REQ-7 (induction-discharged-once — the multiplier) | SHIPPED | `emit_fold_bound_law` GENERATES `fold_bound_<e>` (a `proof fn` parametric in the step `f` + a per-node premise, carrying the SINGLE `decreases l` induction, proving `fold_<e>(l, init, f) <= <e>_len(l) * b`) per ADT a `fold` folds over; `emit_len_measure` generates the structural measure `<e>_len`. An instance bound is proven by CITING the law with NO fresh induction. Consumer: `lower`. Verified: `verus --no-cheating` `verified, 0 errors` on the law + the GROUNDED `sum_list_bounded` instance (cites `fold_bound_list`, NO `decreases`) — `multiplier_instance_cites_the_generated_law_no_fresh_induction`; the NEGATIVE CONTROL (per-node premise removed) FAILS verus — `negative_control_premise_removed_fails_verus` (the induction is real, R-DEFER-9). |
//! | REQ-9 (`LowerError` extension, no panics) | SHIPPED | the scheme lowering reuses the existing `LowerError::Unsupported` (a scheme over a non-ADT value, an un-resolvable scrutinee) and `TooDeep`; no new variant needed. The DEC NUANCE is resolved: a scheme-CALL instance body (non-recursive — the recursion is in the generated `fold_<e>`) lowers WITHOUT a spurious `decreases` (`lower_spec_fn` suppresses it for `is_scheme_call_body`), while the generated `fold_<e>`/law carry their own `decreases l`. No `unwrap`/`expect`/`panic!` added (R-CODE-2 / R-APG-1). |
//! | REQ-3 (exec form — MONOMORPHIZED, OQ-2) | NOT-STARTED | epic **#62** Stage 2c. The SPEC scheme (the verified engine — the generated higher-order `fold_<e>` with the step passed as a `spec_fn`) is SHIPPED above. The MONOMORPHIZED EXEC mirror (an inlined `decreases`-bearing loop, the `conformance/sum.th` while-loop shape) is NOT implemented: the v0.1 corpus `list_fold.th` is SPEC-ONLY (all three items are `spec fn`), so no exec scheme is exercised yet — `collect_scheme_uses` collects spec-fn uses only. The exec mirror lands when a corpus exec fn folds an ADT (no blocker filed; #62 Stage 2c owns it). |
//!
//! ## REQ status — 04-collections.md (Basis Stage 4, issue #73)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-5 (`Vec<T>` → vstd-`Vec` newtype; push/get/len; `fx alloc`; backing-agnostic) | SHIPPED | `lower_type` maps `Type::Vec(elem)` → `tvec_name` (`Vec<u64>` → `TVecU64`); `emit_vec_wrappers` (called from `lower` after `emit_scheme_defs`) materializes ONCE per element type the GROUNDED `TVec<elem>` newtype over `vstd::vec::Vec<elem>` with `well_formed` (`len() <= CAP`), spec `len`/`spec_get`, the no-OOB exec `get` (`req i < len`), and the capacity-preserving exec `push` (`req well_formed && len < CAP`, `ens final(self)...` — the `final(self)` &mut grounding finding). Spec-position `v.get(i)` lowers to `v.spec_get(i as int)` (`lower_expr` MethodCall arm). `fx alloc` emits no verus annotation (effects are a Thermite-level row; `push`'s allocation is the Stage-1 `Effect::Alloc`, accepted by effect-subsumption since `push` is an intrinsic, not a declared callee). Consumer: `lower`. Verified: real `verus --no-cheating` on the emitted `vec_demo.th` — `checked_get`/`push_one` `verified, 0 errors` (`thermite-lower/tests/collections_conformance.rs`); the no-`req` `get` reject FAILS (non-vacuity, L0). BACKING-AGNOSTIC (#62): the surface contract names `len`/`get`/`push` over `v@`, never `vstd::vec::Vec`; a later custom-backing decouple swaps `TVec`'s `data` field WITHOUT changing user `.th` code. |
//! | REQ-6 (`Map<K,V>` → vstd `Map` wrapper) | NOT-STARTED | epic **#62** Stage 4 (OQ-3 thin-first-cut, v1.1). No `Type::Map` node / no `Map` lowering; the v1 corpus oracle (`conformance/vec_demo.th`) is `Vec`-only. Modeled on `vstd::map::Map`, deferred to a Stage-4 follow-up under #62. |
//! | REQ-7 (`LowerError` extension, no panics) | SHIPPED | the `Vec` lowering reuses the existing `LowerError::Unsupported` (`tvec_name` on a non-primitive element type) — no new variant needed; no `unwrap`/`expect`/`panic!` added (R-CODE-2 / R-APG-1). |
//!
//! ## REQ status — 04-collections.md cluster C6 (Vec completeness, issue #98)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-8 (`pop_last`/`last`/`insert`/`remove`/`contains` — tuple-free missing ops) | SHIPPED | #98. `emit_one_vec_wrapper` (called per element type from `emit_vec_wrappers`) emits all five ops on the `TVec<elem>` impl: `pop_last`/`insert`/`remove` are `&mut` with `final(self)` (the REQ-5 grounding finding), `last` is `&self`-reading, `contains` is the exec linear scan with the `forall|k| 0<=k<i ==> v@[k]!=x` invariant + `decreases len-i`. The `insert` carries the load-bearing `i <= len` no-OOB guard. Spec-position `v.last()` lowers to `v.spec_get((v.len()-1) as int)` (`lower_expr` MethodCall arm). Consumer: `lower`. Verified: `forge/tests/vec_completeness_conformance.rs::vec_u64_ops_certify_l3` — real `verus --no-cheating` `9 verified, 0 errors`; the unguarded `insert` FAILS `8 verified, 1 errors` (`vec_insert_without_oob_guard_fails_verus_l0`, non-vacuity, R-DEFER-9). |
//! | REQ-9 (`Vec<T>` non-Copy elements — `Vec<String>`/`Vec<struct>`/nested via borrow-`get`) | SHIPPED | #98. `tvec_name` `match` EXTENDS to a `String` element (→ `TVecTString`), a `Named` struct/enum element (→ `TVec<Name>`), and a nested `Vec(inner)` element (→ recursive `tvec_name(inner)` suffix, `Vec<Vec<u64>>` → `TVecTVecU64`). `elem_is_copy` classifies the element: a Copy element keeps the by-value `get -> T`/`last -> T` + `contains`; a NON-Copy element gets the BORROW `get -> &T`/`last -> &T` (`&self.data[i]`, `ens *result == v@[i]`) — vstd's index MOVES a non-Copy element out (`E0507`), so the borrow is the load-bearing fix; `push(x: T)` consumes the owned element (no Copy needed). Consumer: `lower`. Verified: `vec_completeness_conformance.rs` — `Vec<String>` `17 verified, 0 errors` (the make-or-break), `Vec<struct>` `7 verified`, nested `Vec<Vec<u64>>` `15 verified`, all `0 errors` (the GROUNDED `4 verified, 0 errors` op-subset within each, the by-value form FAILS `E0507`). |
//! | REQ-10 (element-type wrapper/decl woven before the `TVec` — #68/#86 pattern) | SHIPPED | #98. `collect_vec_elem_types`'s `note_vec_elems` notes a nested `Vec` element INNER-FIRST, so `emit_vec_wrappers` emits the inner `TVecU64` before the outer `TVecTVecU64` (REQ-10 emission order for the two-wrapper case). For a `String`/struct element the wrapper/decl is woven via the CONSUMED `program_uses_string` (`ty_reaches_string` recurses `Type::Vec`) / `forge::collect_type_adt_refs` (recurses `Type::Vec`) — verus resolves references within the `verus!` block order-independently (the 17/0 + 7/0 verifies confirm). Consumer: `lower`. Verified: `vec_completeness_conformance.rs` (the element wrapper/decl present + the whole program verifies L3). |
//! | REQ-11 (`Vec::new()`-no-param wrapper-reachability fix — the #86 analog) | SHIPPED | #98. `collect_vec_elem_types` EXTENDS its reachability closure from fn/spec-fn param+return to ALSO walk `struct`/`enum`-variant FIELD types and `fn`-body local `let` type annotations (`note_block_vec_elems`/`note_stmt_vec_elems`, the #86 String-reachability analog keyed on `Type::Vec(inner)`). A body-local `let v: Vec<u64> = Vec::new();` now emits `TVecU64`. `lower_stmt`'s `Stmt::Let` arm rewrites a `Vec`-typed `Vec::new()` init (`is_vec_new`) to the wrapper construction `<TVec> { data: Vec::new() }` (a bare `Vec::new()` cannot inhabit the newtype, `E0308`). Consumer: `lower`. Verified: `vec_completeness_conformance.rs::local_vec_new_no_param_certifies_l3` (a fn whose only Vec is a body-local `Vec::new()` certifies L3 — NOT `E0425`). |
//! | REQ-12 (`last`/`contains` in `BUILTIN_METHODS`; non-Copy `tvec_name` extension; no panics) | SHIPPED | #98. `last`/`contains` ADDED to `BUILTIN_METHODS` (`thermite-spec/src/validator.rs`) so an `ens result == v.last()` / `v.contains(x)` validates inside the §4.2 cage; `pop_last`/`insert`/`remove` stay EXEC-only. `tvec_name` extends to String/`Named`/nested `Vec` elements (REQ-9). A still-unlowerable element (`Unit`/`Ref`/`Slice`/`Generic`) is the existing `LowerError::Unsupported` — no new variant, no `unwrap`/`expect`/`panic!` (R-CODE-2 / R-APG-1). Consumer: `lower` / `validate`. Verified: `vec_completeness_conformance.rs` + the validator tests. |
//!
//! ## REQ status — 07-strings.md cluster C4 (Basis Stage 7, issue #94)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-4 (String-SCANNING `spec fn` — `byte_at`/`len` over a `&String` param; #126) | SHIPPED | The exec `lower_fn` signature path threaded a fn's `&String` params via `.with_strings(..)` (`lower_fn_signature`), but the `spec fn` body path did NOT — so a `&String`-param `byte_at(i)` in a `spec fn` body stayed bare against the `usize`-typed exec accessor (E0308). FIX: `lower_spec_fn_body` + `lower_spec_fn_body_with_schemes` + `spec_dec` now collect the spec fn's `String`/`&String` params (`string_param_names` / `is_string_ty`) and add `.with_strings(..)` to the `Ctx::spec_seq()`, so a `&String`-param `byte_at(i)` rewrites to `spec_byte_at(i as int)` and a `dec s.len()` to `s.spec_len()`. Two narrowings for Verus's UNBOUNDED-`int` spec arithmetic: a recursive spec-fn-call arithmetic arg `i + 1` → `(i + 1) as u64`, and a contract `s.len()` arg → `s.spec_len() as u64` (`lower_expr` `Call` arm, gated on `plain_user_spec_call` — a user spec fn's surface integer param is `u64`); the `s -> s.data@` byte-view is gated on `callee_takes_string_byteview` so a `&String`-param spec fn's self-call passes `s` (`&TString`) through, NOT `s.data@` (`Seq<u8>`, the GENERATED byte-view fns only). Consumer: `lower` (the editor's `spec_line_start` twin pins `cursor_col` exactly). Verified: `forge/tests/spec_fn_string_param.rs` (real verus — a String-scanning spec fn + exec twin + contract-naming fn all L3; a broken body rejects below L3) + `forge/tests/editor_runs.rs` (`spec_line_start` L3, `cursor_col` 4/4). |
//! | REQ-7 (`push_byte`/`from_byte` — verified byte-builder; `fx alloc`) | SHIPPED | `emit_string_wrapper` emits the `from_byte(b: u64) -> TString` 1-byte constructor (`ens len == 1 && data@[0] == b as u8`) and the `push_byte(&self, b: u64) -> TString` copy-then-append (`req len < CAP`, `ens len == old+1 && data@[old] == b as u8` + the element-frame `forall|j| 0 <= j < old ==> result@[j] == self@[j]`) — the GROUNDED byte-builder over vstd's verified `Vec::push` (the `u64` byte zero-extends, the SAME convention as `byte_at -> u64`). Owned-result form (no `&mut`/`final`). Consumer: `lower` (`emit_string_wrapper`). Verified: `forge/tests/string_format_conformance.rs` (real verus L3 / `effects: [alloc]`). |
//! | REQ-8 (`u64_to_string` — decimal formatting, ROUND-TRIP contract; `fx alloc`) | SHIPPED | `emit_numfmt_defs` emits the `pow10`/`parse_le`/`parse_be`/`seq_reverse` spec fns + the `lemma_parse_push` append lemma + the display-bridge lemmas `lemma_parse_be_push`/`lemma_parse_be_reverse` (`parse_be(seq_reverse(s)) == parse_le(s)`, proved by induction, `=~=` extensionality + `by(nonlinear_arith)`) + the `u64_to_string(n) -> TString` exec fn (the divide/mod-by-10 digit loop builds LSB-first — invariant `parse_le(data@) + m*pow10(data.len()) == n`, `decreases m` — THEN a reverse loop to the human-readable MSB-first display order, blocker #96). Materialized once when the program uses `n.to_string()` / names `parse_be` (`program_uses_numfmt`). The method `n.to_string()` lowers to `u64_to_string(n)` (`lower_expr` MethodCall exec arm); a contract `parse_be(result)` lowers to `parse_be(result.data@)` (`lower_spec_arg` String-view rule) with the round-trip `ens parse_be(result.data@) == n as nat` (the `as nat` coercion via `nat_fns += parse_be`). GROUNDED `17 verified, 0 errors`; an overclaimed round-trip FAILS (R-DEFER-9). UPPER-BOUND `ens result.data.len() <= 20` (blocker #105): a u64 is `< 10^20`, so at most 20 digits — PROVED via the build-loop invariant `data.len() <= 20` + `lemma_pow10_le`/`lemma_pow10_20_gt_u64max` (`pow10(20) > u64::MAX`, `reveal_with_fuel` + `by(compute)`), carried through the reverse loop; this lets a caller's bounded `concat` discharge the §4.2 CAP when an operand is `n.to_string()`. The struct-invariant signature weave (`named_struct_param`) now sees through a single `&` borrow so a `&Buffer`-style ref param carries `well_formed()` (without it `b.cursor + 1` cannot be proved non-overflowing). Consumer: `lower`. Verified: `forge/tests/string_format_conformance.rs` + `forge/tests/divergence_numfmt_display_order.rs` (the MSB-first round-trip certifies L3, the formatter builds+runs 42→[52,50]=="42", 100→"100", 7→"7") + `forge/tests/editor_runs.rs` (the #90 editor's `render_frame` certifies L3 — the upper bound + ref-weave discharge the bounded `concat`). |
//! | REQ-9 (`parse_u64` — `String`→`u64`, PARTIAL) | SHIPPED | #95 cluster C7. `emit_parse_defs` emits the `is_digit`/`all_digits`/`parse_be` spec fns + `parse_u64(s: &TString) -> Option<u64>` (the Horner-accumulate loop + the BE partial-value invariant + the three handled-or-loud `None` arms) with the round-trip success `ens match result { Some(v) => all_digits(s.data@) && s.data.len() >= 1 && parse_be(s.data@) == v as nat, None => true }`; materialized when `program_uses_parse`; `parse_be` deduped against the numfmt round-trip. Consumer: `lower`. Verified: `forge/tests/option_result_conformance.rs::ac4_parse_u64_lowering_verifies_under_real_verus` (real verus `5 verified, 0 errors`) + the broken-`Some(0)` non-vacuity. |
//! | REQ-13 (`contains`/`starts_with`/`ends_with` — boolean substring predicates; `pure`) | SHIPPED | #102 cluster C5. `emit_string_search_methods` (called from `emit_string_wrapper` when `program_uses_string_search`) emits the inner `matches_at` helper + `starts_with`/`ends_with`/`contains` byte scans (`ens result == occurs_at(self.data@, p.data@, ..)` / `contains_sub(..)`); `emit_string_search_defs` emits the `occurs_at`/`contains_sub` spec fns. `contains` is RECEIVER-TYPE-dispatched: `TString::contains` (substring) vs `TVec::contains` (membership) — DISTINCT inherent methods, no clobber. Consumer: `lower` (`emit_string_wrapper`). Verified: `forge/tests/string_search_conformance.rs` (real verus L3 pure — a true AND a false case; a broken `starts_with` FAILS, non-vacuous). GROUNDED `14 verified, 0 errors`. |
//! | REQ-14 (`find` — first occurrence → `Option<u64>`; `pure`) | SHIPPED | #102 cluster C5. `emit_string_search_methods` emits the `find(&self, p: &TString) -> Option<u64>` occurrence scan with the C7 spec-`match`-in-`ens` (`Some(at) => occurs_at(..), None => !contains_sub(..)`), reusing C7's `Type::Option` lowering. The `occurs_at` offset arg is cast `as int` (the `lower_expr` `Call` `occurs_fn` arm). Consumer: `lower`. Verified: `forge/tests/string_search_conformance.rs` (real verus L3; the PINNED Some case proves `result is Some`, the always-`None` mutant FAILS — #101 trap avoided). |
//! | REQ-15 (`split` — split on a separator byte → `Vec<String>`; `fx alloc`) | SHIPPED | #102 cluster C5. `emit_string_search_methods` emits the `split(&self, sep: u8) -> TVecTString` push-loop (the count partial + sep-free invariant); `emit_string_search_defs` emits the `count_sep`/`sep_free` spec fns + the `lemma_count_push` induction proof. `collect_vec_elem_types` weaves the `Vec<String>` element (→ `TVecTString`) when a C5 op is used so `split`'s result wrapper is always in scope. The surface `u64` `sep` is cast `as u8` at the call site (exec) + `as u8` in the `count_sep`/`sep_free` contract arg (spec). `count_sep` joins `nat_fns`. Consumer: `lower`. Verified: `forge/tests/string_search_conformance.rs` (real verus — the count-bound + sep-free floor `7 verified, 0 errors`; a `split`-drop mutant FAILS, non-vacuous). The count-bound is the STRONGEST proved contract (NOT a reconstruct-round-trip). |
//! | REQ-16 (`trim` — strip leading/trailing ASCII whitespace → `String`; `fx alloc`) | SHIPPED | #102 cluster C5. `emit_string_search_methods` emits the `trim(&self) -> TString` forward/backward whitespace scan + bounded copy (the subrange invariant `out@ == self.data@.subrange(lo, i)`); `emit_string_search_defs` emits the `is_space` spec fn. The whitespace test is inlined in the exec loop (`is_space` is a spec fn). Consumer: `lower`. Verified: `forge/tests/string_search_conformance.rs` (real verus — the length floor + subrange content `8 verified, 0 errors`). |
//!
//! ## REQ status — 13-map.md cluster C12 (bounded verified Map<K,V>, issue #114/#123)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-4 (`Map<K,V>` → Vec-of-pairs `TMap` wrapper + spec view; `insert`/`get`/`contains_key`/`len`; `fx alloc`) | SHIPPED | `lower_type` maps `Type::Map(k, v)` → `tmap_name` (`Map<u64,u64>` → `TMapU64U64`); `emit_map_wrappers` (called from `lower` after `emit_vec_wrappers`) materializes ONCE per `(K,V)` pair the GROUNDED `TMapU64U64` newtype over `vstd::vec::Vec<(u64,u64)>` with `spec_dom`/`well_formed` (capacity `data.len() <= MAP_CAP` + key-uniqueness)/`spec_contains_key`/`len` spec view, the exec linear-scan `contains_key` (`ens result == spec_contains_key(k)`), the no-OOB / handled-or-loud `get -> Option<V>` (absent → None, NOT a wrong value), and the append-under-`!contains_key` `insert` (`ens final(self)...`, the `final(self)` &mut grounding finding). Spec-position `m.contains_key(k)` lowers to `m.spec_contains_key(k)` (`lower_expr` MethodCall arm); a `Map`-typed param weaves `well_formed()` (`is_map_param_ty`, the inv-struct/String weave precedent); `Map::new()` `let`-init rewrites to `<TMap> { data: Vec::new() }` (`is_map_new`). `fx alloc` accepted by effect-subsumption (`insert` is an intrinsic). Consumer: `lower`. Verified: real `verus --no-cheating` on the GROUNDED `TMapU64U64` + the emitted `map_kv.th` (`forge/tests/map_conformance.rs` — `9 verified, 0 errors`: insert-then-get round-trip → `Some(v)`, absent → `None`, contains_key both branches; the broken `Some(0)`-for-absent FAILS `verified, 1 errors`, non-vacuity R-DEFER-9). BACKING-AGNOSTIC (#62/#114): the surface contract names `len`/`get`/`contains_key`/`insert` over the spec map abstraction, never `vstd::vec::Vec<(K,V)>`. |
//! | REQ-5 (`Type::Map` exhaustive-match ripple) | SHIPPED | the new two-arg `Type::Map` variant rippled to every exhaustive `match Type`: `lower_type` (the wrapper-name arm), `tmap_name`/`tmap_type_suffix`/`collect_map_kv_types`/`note_map_kv` (the monomorphization + reachability), `note_vec_elems` (recurses both Map args), `ty_reaches_string` (recurses both args), `l1.rs::lower_type` + `emit_map_runtime_l1` (the L1 runnable mirror), `l2.rs` `type_label`, `forge/src/{check.rs (collect_type_adt_refs both args — the #68 ADT weave), review.rs (render_type)}`, `thermite-skill/src/generate.rs` (the `Map<K,V>` fragment) — honest arms, no `_`/panic. `mutation.rs`'s `zero_value_for`/`zero_desc` legitimately route a `Map`-returning fn to the no-scalar-zero `_` catch-all (like `Result`). Consumer: `lower`/`lower_l1`. Verified: `cargo build --workspace` (exhaustiveness) + `forge/tests/map_conformance.rs`. |
//! | REQ-6 (`LowerError` extension, no panics) | SHIPPED | the `Map` lowering reuses the existing `LowerError::Unsupported` (`tmap_name`/`tmap_type_suffix` on a non-Copy key / unsupported key-value type — v1 grounds `Map<u64,u64>` Copy keys, OQ-4) — no new variant; no `unwrap`/`expect`/`panic!` added (R-CODE-2 / R-APG-1). |
//!
//! ## REQ status — 09-option-result.md cluster C7 (built-in Option/Result, issue #95)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-4 (`Option`/`Result` → Verus types; construct/match/`is`/spec-match lower) | SHIPPED | `lower_type` gains `Type::Option(T)` → `Option<T>` and `Type::Result(T, E)` → `Result<T, E>` (the Verus-native generics, no wrapper); `qualify_variant_path` leaves the built-in `Some`/`Ok`/`Err`/`None` UNQUALIFIED (not in the user `variants` map → bare names Verus's prelude carries). The spec-`match`-in-`ens` lowers via the EXISTING `lower_expr` `Match` arm. Consumer: `lower`. Verified: `forge/tests/option_result_conformance.rs` AC-1/AC-2/AC-4 (real verus L3). |
//! | REQ-5 (`parse_u64` emission) | SHIPPED | `emit_parse_defs` + `program_uses_parse` (see 07-strings.md REQ-9 above). |
//!
//! ## REQ status — 10-recursion-tuples.md cluster C9-A (plain-`fn` recursion, issue #108)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-3 (`fn` `decreases` lowering) | SHIPPED | `lower_fn` emits `decreases <spec_dec(f.dec, f.params)>` AFTER the signature's `requires`/`ensures` block and BEFORE the body when `f.dec.is_some()` — the SAME `spec_dec` helper + position the recursive `spec fn` uses (`lower_spec_fn`). A non-recursive fn (`dec = None`) emits NO `decreases` (byte-stable for the existing corpus — AC-7, `sum`/`binary_search` lower unchanged). The self-call lowers as an ordinary `Expr::Call` (no special node); Verus discharges termination from the emitted `decreases`. The `fx diverge` exemption (`fn_is_diverge` → `#[verifier::exec_allows_no_decreases_clause]`, #88) lets a diverge fn recurse WITHOUT `dec` (L1-capped). Consumer: `lower` (`Item::Fn`). Verified: `forge/tests/recursion_conformance.rs` — real verus: `count_down(n-1)` `dec n` → L3; recurse-on-`n` → L0 ("could not prove termination"); a `fx diverge` recursive fn → L1; the recursive fn BUILDS + RUNS at L1. |
//! | REQ-4 (termination bites) | SHIPPED | GROUNDED end-to-end (`forge/tests/recursion_conformance.rs`): the `decreases` is the ONLY thing between the fn and L0 — non-decreasing → L0, no-`dec` → `MissingDecreases` (the `thermite-spec` validator, never reaching L3), diverge → L1-capped. No proof cheat (R-DEFER-9). Consumer: `forge::check` ladder. |
//!
//! ## REQ status — 11-ergonomics.md cluster C10 (binding/control-flow ergonomics, issue #112)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1/REQ-2/REQ-5 (tuple destructure / `for` / `if let`-`while let`) | SHIPPED | PURE-DESUGAR in the parser — these emit NO new AST node; the desugar targets (`Expr::TupleProj`, `LoopNode`(`While`), `Expr::Match`, `Expr::Is`) are the SHIPPED constructs `lower_expr`/`lower_loop`/`lower_match` already emit unchanged. The `for` AUTO-`dec hi - i` is a real `dec` `Clause` (`lower_loop` emits it as `decreases` — no proof cheat, R-DEFER-9). Verified: `forge/tests/ergonomics_conformance.rs::{req1_tuple_destructuring_certifies_l3, req2_for_range_certifies_l3, req2_bad_for_inv_is_l0, req5_if_let_certifies_l3, req5_while_let_certifies_l3}` (real verus). |
//! | REQ-3 (match guard lowering) | SHIPPED | `lower_match` emits the Verus-NATIVE guarded arm `pat if <guard> => body` when `arm.guard.is_some()` (the proven core — Verus supports guards). The guard `Expr` is also walked by `collect_combinators_in_expr` + `each_subexpr` (a combinator/effect in a guard is captured). Consumer: `lower` / `lower_fn`. Verified: `forge/tests/ergonomics_conformance.rs::req3_guarded_match_certifies_l3` (L3, non-vacuous). |
//! | REQ-4 (or-pattern lowering) | SHIPPED | `lower_pattern` += a `Pattern::Or` arm emitting the Verus-NATIVE `p0 \| p1 \| …` (each alternative enum-qualified). The new variant rippled to every exhaustive `match Pattern` in this crate (`lower_pattern`, `l1::lower_pattern_exec`) with honest arms. Consumer: `lower_match`. Verified: `forge/tests/ergonomics_conformance.rs::req4_or_pattern_certifies_l3` (L3). |

use std::fmt::Write as _;

use thermite_syntax::ast::{
    BinOp, Block, Clause, EnumItem, Expr, FnItem, IndexArg, Item, MatchArm, Param, Pattern,
    PrimType, Program, SlicePat, SpecFnItem, Stmt, Type, UnaryOp, VariantDef, VariantShape,
};
use thermite_syntax::lexer::Span;

/// The maximum recursive-descent emission depth before `lower` returns
/// `LowerError::TooDeep`. The lowerer recurses over the AST (expressions,
/// blocks, statements, types, patterns); like `thermite-syntax`'s parser guard
/// (its `MAX_RECURSION_DEPTH`, the #29/#31/#32 lesson) a single shared counter
/// bounds EVERY recursive family here so a pathological (or adversarial,
/// post-recovery) AST cannot overflow the native stack and abort the process.
/// Fixed constant (determinism, `goal.md` R-CODE-5). Set well above any
/// human-authored nesting; `thermite-syntax` itself caps parse nesting at 64, so
/// a well-formed AST cannot exceed that — this is a defensive backstop.
const MAX_EMIT_DEPTH: usize = 256;

/// `thermite-lower`'s own error type — born here with this crate's first
/// fallible function (`.design/scaffold/workspace.md` REQ-3). Span-bearing
/// (reusing `thermite_syntax::lexer::Span`) and `Display`-able. No panics
/// (`goal.md` R-CODE-2 / R-APG-1): an un-lowerable construct is an `Err`, never
/// an `unwrap`/`expect`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    /// A combinator call whose callee path is not in the `thermite-spec`
    /// registry. Validation (#2) should have caught this; the lowerer re-checks
    /// defensively (verus-lowering.md REQ-9).
    UnknownCombinator { name: String, span: Span },
    /// An expression/type/statement nested past `MAX_EMIT_DEPTH` — surfaced
    /// structurally so input can never overflow the C stack (REQ-9, R-CODE-2).
    TooDeep { limit: usize, span: Span },
    /// A construct the v0.1 lowering does not cover (e.g. a `Type` or `Expr`
    /// shape outside the corpus mapping tables). Carries a human description.
    Unsupported { what: String, span: Span },
    /// A call site where the caller's `fx` row does NOT subsume the callee's
    /// (`.design/lower/effect-subsumption.md` REQ-4; `thermite-design.md` §4.1
    /// "a caller's row must subsume every callee's row"). `missing` names the
    /// atomic effects the callee has that the caller's row lacks
    /// (`effects(callee) \ effects(caller)`), so the diagnostic tells the agent
    /// exactly which effect to add to the caller's row (or remove from the
    /// callee). Produced by `effects::check_effects`; NEVER a panic (R-CODE-2).
    EffectNotSubsumed {
        caller: String,
        callee: String,
        missing: Vec<thermite_syntax::ast::Effect>,
        span: Span,
    },
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LowerError::UnknownCombinator { name, span } => write!(
                f,
                "unknown combinator `{name}` at byte {}..{} (not in the SpecTherm registry)",
                span.start,
                span.end()
            ),
            LowerError::TooDeep { limit, span } => write!(
                f,
                "expression nested past the lowerer's depth limit of {limit} at byte {}..{}",
                span.start,
                span.end()
            ),
            LowerError::Unsupported { what, span } => write!(
                f,
                "unsupported construct for L3 lowering: {what} at byte {}..{}",
                span.start,
                span.end()
            ),
            LowerError::EffectNotSubsumed {
                caller,
                callee,
                missing,
                span,
            } => {
                let atoms: Vec<String> = missing.iter().map(effect_atom_name).collect();
                write!(
                    f,
                    "effect row of `{caller}` does not subsume callee `{callee}` at byte {}..{}: \
                     missing effect(s) [{}] (add them to `{caller}`'s `fx` row or remove them from `{callee}`)",
                    span.start,
                    span.end(),
                    atoms.join(", ")
                )
            }
        }
    }
}

/// The surface atom name of an `Effect` for an `EffectNotSubsumed` diagnostic
/// (REQ-4). v0.1 subsumption is path-insensitive (`.design/lower/effect-subsumption.md`
/// OQ-1), so the carrier atoms (`read`/`write`/`net`) are reported by KIND
/// without their (empty) path argument — the agent's fix is to add the effect
/// kind to the caller's row.
fn effect_atom_name(effect: &thermite_syntax::ast::Effect) -> String {
    use thermite_syntax::ast::Effect;
    match effect {
        Effect::Read(_) => "read".to_string(),
        Effect::Write(_) => "write".to_string(),
        Effect::Net(_) => "net".to_string(),
        Effect::Alloc => "alloc".to_string(),
        Effect::Time => "time".to_string(),
        Effect::Rand => "rand".to_string(),
        Effect::Panic => "panic".to_string(),
        Effect::Diverge => "diverge".to_string(),
        Effect::Term => "term".to_string(),
    }
}

impl std::error::Error for LowerError {}

/// Lowering position: spec (`requires`/`ensures`/`invariant`/`decreases` and
/// `spec fn` bodies) vs exec (`fn` bodies). Drives the slice→`Seq` rewrite
/// (REQ-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pos {
    Exec,
    Spec,
}

/// Lowering context: the position plus the set of in-scope slice-typed
/// parameter names. In SPEC position a bare slice-param path `xs` becomes the
/// `vstd` view `xs@` (a `Seq<T>`) — REQ-5. The set is computed per item from the
/// parameter types (a SHAPE-derived fact, not a name list), so the `@` rewrite
/// generalizes to any slice-typed parameter.
#[derive(Debug, Clone, Copy)]
struct Ctx<'a> {
    pos: Pos,
    slices: &'a [&'a str],
    /// Names of `spec fn`s lowered with a `nat` return type (the head-fold-sum
    /// shape — OQ-1). An `Eq` between a `u64`-valued scalar and a call to one of
    /// these coerces the scalar with `as nat`, since `nat` and `u64` are not the
    /// same Verus type. Computed program-wide, SHAPE-derived.
    nat_fns: &'a [&'a str],
    /// The program's `(variant_name, enum_name)` map (REQ-9): a `match` arm /
    /// pattern over a user enum variant lowers to the Verus-required ENUM-QUALIFIED
    /// path `Enum::Variant` (verus rejects a bare `Nil`/`Circle`). `Some`/`None`
    /// and slice patterns are NOT in this map, so they lower unqualified (Verus
    /// knows the `Option` built-in) — the qualification is keyed on membership.
    variants: &'a [(&'a str, &'a str)],
    /// True inside the body of a `nat`-returning spec fn (REQ-10): an integer
    /// cast (`h as u64`) coerces to `as nat` so the fold's arithmetic stays `nat`
    /// (no overflow obligation in spec context), the GROUNDED `sum_list` form.
    nat_ret: bool,
    /// Basis Stage 2 (`.design/basis/02-recursion-schemes.md` REQ-6): the
    /// recursion-scheme bindings IN SCOPE for the spec fn currently being lowered
    /// — one per (scheme name → resolved generated fn + element/result types) the
    /// fn's scrutinee resolves to. A scheme CALL `fold(l, 0, |x, acc| …)` lowers
    /// (in `lower_expr`'s `Call` arm) to a CALL of the generated `fold_<e>` with
    /// the step closure lowered to a typed `spec_fn`. EMPTY for a non-scheme fn
    /// (byte-stable for the existing corpus).
    schemes: &'a [SchemeBinding],
    /// Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): the names that denote
    /// a `String` value IN SCOPE for the spec context currently being lowered —
    /// every `String`/`&String` parameter plus `result` when the return type is
    /// `String`. A `String` receiver's `.len()` / `.byte_at(i)` in SPEC position
    /// rewrites to the wrapper's spec fns `.spec_len()` / `.spec_byte_at(i as int)`
    /// (the exec `len`/`byte_at` return `u64` and cannot be named in a contract; a
    /// Verus spec index is `int`). Keyed on the receiver being a `String`-named
    /// path so a `Vec` receiver's `.len()` (whose wrapper spec fn IS named `len`)
    /// is UNCHANGED — the rewrite is `String`-specific. EMPTY for a non-`String`
    /// fn (byte-stable for the existing corpus).
    strings: &'a [&'a str],
    /// Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): the program-wide set of
    /// FIELD names whose declared type reaches `String` (the editor core `Buf {
    /// text: String }`). A spec-position method call whose receiver is a FIELD
    /// access `<x>.<field>` where `<field>` is in this set rewrites `.len()`/
    /// `.byte_at(i)` to the wrapper SPEC fns `.spec_len()`/`.spec_byte_at(i as int)`
    /// — the field analog of `strings` (which keys a bare `String` VALUE path). A
    /// contract `b.text.len()` / `result.text.len()` over a `String` field needs the
    /// spec accessor (the exec `len`/`byte_at` cannot be named in a contract). EMPTY
    /// for a program with no `String` field (byte-stable for the existing corpus).
    string_fields: &'a [&'a str],
    /// Cluster C7 (`.design/basis/09-option-result.md` REQ-5, issue #95/#100): the
    /// names of OWNED `String` (`Type::String`, NOT `&String`) parameters in scope
    /// for the EXEC body currently being lowered. The generated `parse_u64` takes
    /// `&TString` (a read-only borrow), so an exec call `parse_u64(s)` whose arg `s`
    /// is an OWNED `String` param must lower to `parse_u64(&s)` (a `&String` param is
    /// already a borrow and passes through unchanged). Keyed on the param SHAPE
    /// (owned vs reference), not on the name. EMPTY for a fn with no owned `String`
    /// param (byte-stable for the existing corpus).
    owned_strings: &'a [&'a str],
    /// Basis Stage 7 (`.design/basis/07-strings.md` REQ-4, issue #127): the
    /// program-wide names of USER `spec fn`s that declare a `String`/`&String`
    /// parameter (the #126 String-scanning shape). The byte-view dispatch
    /// (`callee_takes_string_byteview`) is SHAPE-keyed off this: the GENERATED
    /// byte-view spec fns (`parse_le`/`is_digit`/`occurs_at`/… — declared over
    /// `Seq<u8>`) take a `String` arg as its `.data@` view; a USER spec fn declares
    /// a `&TString` param, so a `String` arg to it passes the REFERENCE through
    /// (`s`, NOT `s.data@` — else `Seq<u8>` vs `&TString`, E0308). A user spec fn
    /// NAMED like a generated one (`spec fn is_digit(s: &String, ..)`) lives in the
    /// user namespace and SHADOWS the generated name, so it must be excluded from
    /// the byte-view set — keyed on the callee's PARAM SHAPE (it takes `&String`),
    /// not on the name. EMPTY for a program with no String-param user spec fn
    /// (byte-stable — the generated fns are not in this set, so they still byteview).
    user_string_spec_fns: &'a [&'a str],
}

/// A resolved recursion-scheme binding in scope while lowering a spec fn body
/// (REQ-6): the surface scheme name (`fold`), the generated Verus fn name it
/// lowers to (`fold_list`), the ADT element type (`u64` — the step's element
/// parameter type), and the scheme's result kind (drives the step's accumulator
/// type + the `as nat` coercion of the step body).
#[derive(Debug, Clone)]
struct SchemeBinding {
    scheme_name: &'static str,
    gen_name: String,
    elem_ty: String,
    result: thermite_spec::SchemeResult,
}

const NO_SCHEMES: &[SchemeBinding] = &[];

const NO_SLICES: &[&str] = &[];
const NO_VARIANTS: &[(&str, &str)] = &[];

impl<'a> Ctx<'a> {
    fn exec() -> Ctx<'static> {
        Ctx {
            pos: Pos::Exec,
            slices: NO_SLICES,
            nat_fns: NO_SLICES,
            variants: NO_VARIANTS,
            nat_ret: false,
            schemes: NO_SCHEMES,
            strings: NO_SLICES,
            string_fields: NO_SLICES,
            owned_strings: NO_SLICES,
            user_string_spec_fns: NO_SLICES,
        }
    }
    fn spec(slices: &'a [&'a str], nat_fns: &'a [&'a str]) -> Ctx<'a> {
        Ctx {
            pos: Pos::Spec,
            slices,
            nat_fns,
            variants: NO_VARIANTS,
            nat_ret: false,
            schemes: NO_SCHEMES,
            strings: NO_SLICES,
            string_fields: NO_SLICES,
            owned_strings: NO_SLICES,
            user_string_spec_fns: NO_SLICES,
        }
    }
    /// A spec context with no slice-view names — for positions where every
    /// slice value is already a `Seq` (spec-fn bodies, whose slice params are
    /// `Seq<T>`) or where no slice appears (scalar predicates, literals).
    fn spec_seq() -> Ctx<'static> {
        Ctx {
            pos: Pos::Spec,
            slices: NO_SLICES,
            nat_fns: NO_SLICES,
            variants: NO_VARIANTS,
            nat_ret: false,
            schemes: NO_SCHEMES,
            strings: NO_SLICES,
            string_fields: NO_SLICES,
            owned_strings: NO_SLICES,
            user_string_spec_fns: NO_SLICES,
        }
    }
    /// This context with the enum-variant map attached (REQ-9 — variant-pattern
    /// qualification). Carried through `match`/pattern lowering.
    fn with_variants(mut self, variants: &'a [(&'a str, &'a str)]) -> Ctx<'a> {
        self.variants = variants;
        self
    }
    /// This context marked as a `nat`-returning spec-fn body (REQ-10 — integer
    /// casts coerce to `as nat`).
    fn with_nat_ret(mut self, nat_ret: bool) -> Ctx<'a> {
        self.nat_ret = nat_ret;
        self
    }
    /// This context with the recursion-scheme bindings in scope (REQ-6 — scheme
    /// CALL lowering). Carried into the spec-fn body so `lower_expr` rewrites a
    /// scheme call to a call of the generated `fold_<e>`.
    fn with_schemes(mut self, schemes: &'a [SchemeBinding]) -> Ctx<'a> {
        self.schemes = schemes;
        self
    }
    /// This context with the `String`-named values in scope (REQ-4 — a `String`
    /// receiver's spec-position `.len()`/`.byte_at(i)` rewrite). Carried into the
    /// signature `requires`/`ensures` lowering so a contract over a `String` param
    /// or `result` names the wrapper's spec fns.
    fn with_strings(mut self, strings: &'a [&'a str]) -> Ctx<'a> {
        self.strings = strings;
        self
    }
    /// This context with the program-wide `String`-typed FIELD names in scope
    /// (REQ-4 — a `String` FIELD receiver's spec-position `.len()`/`.byte_at(i)`
    /// rewrite). The field analog of [`Ctx::with_strings`].
    fn with_string_fields(mut self, string_fields: &'a [&'a str]) -> Ctx<'a> {
        self.string_fields = string_fields;
        self
    }
    /// This context with the OWNED `String` parameter names in scope (REQ-5 — the
    /// exec `parse_u64(s)` borrow-rewrite). Carried into the EXEC body so an owned
    /// `String` arg to the generated `parse_u64` (which takes `&TString`) lowers to
    /// `parse_u64(&s)`. The field analog of [`Ctx::with_strings`], EXEC-side.
    fn with_owned_strings(mut self, owned_strings: &'a [&'a str]) -> Ctx<'a> {
        self.owned_strings = owned_strings;
        self
    }
    /// This context with the program-wide USER String-param `spec fn` names in
    /// scope (REQ-4, #127 — the byte-view dispatch's SHAPE key). Carried into every
    /// spec/exec body so `callee_takes_string_byteview` can exclude a user spec fn
    /// that NAMES a generated byte-view fn but declares a `&String` param.
    fn with_user_string_spec_fns(mut self, names: &'a [&'a str]) -> Ctx<'a> {
        self.user_string_spec_fns = names;
        self
    }
    /// True if `name` is a USER `spec fn` declaring a `String`/`&String` param (the
    /// #126 String-scanning shape). Such a callee takes its `String` arg as a
    /// `&TString` REFERENCE, not the `.data@` byte view — so it SHADOWS any
    /// generated byte-view fn of the same name (#127). Keyed on the PARAM SHAPE.
    fn is_user_string_spec_fn(&self, name: &str) -> bool {
        self.user_string_spec_fns.contains(&name)
    }
    /// True if `name` denotes an OWNED `String` (`Type::String`, not `&String`)
    /// parameter in scope (drives the exec `parse_u64(s)` → `parse_u64(&s)` borrow,
    /// REQ-5). An owned value must be borrowed to satisfy the `&TString` param.
    fn is_owned_string(&self, name: &str) -> bool {
        self.owned_strings.contains(&name)
    }
    /// True if `name` denotes a `String` value in scope (drives the spec-position
    /// `.len()`→`.spec_len()` / `.byte_at(i)`→`.spec_byte_at(i as int)` rewrite).
    fn is_string(&self, name: &str) -> bool {
        self.strings.contains(&name)
    }
    /// True if `name` is a program field whose type reaches `String` (drives the
    /// spec-position `<x>.<field>.len()`→`<x>.<field>.spec_len()` rewrite). REQ-4.
    fn is_string_field(&self, name: &str) -> bool {
        self.string_fields.contains(&name)
    }
    /// The in-scope scheme binding for a callee `name` (REQ-6), or `None` if
    /// `name` is not a scheme call resolved for the current fn.
    fn scheme_binding(&self, name: &str) -> Option<&'a SchemeBinding> {
        self.schemes.iter().find(|b| b.scheme_name == name)
    }
    fn is_spec(&self) -> bool {
        self.pos == Pos::Spec
    }
    /// True if `name` is an in-scope slice-typed parameter (gets `@` in spec).
    fn is_slice(&self, name: &str) -> bool {
        self.slices.contains(&name)
    }
    /// True if `name` is a `nat`-returning spec fn (drives `as nat` coercion).
    fn is_nat_fn(&self, name: &str) -> bool {
        self.nat_fns.contains(&name)
    }
    /// The enum name a user variant belongs to (REQ-9), or `None` if `name` is not
    /// a declared user variant (`Some`/`None`/a binding/literal — left unqualified).
    fn enum_of_variant(&self, name: &str) -> Option<&'a str> {
        self.variants
            .iter()
            .find(|(v, _)| *v == name)
            .map(|(_, e)| *e)
    }
    /// A clone of this spec context keeping its name sets (for recursing).
    fn keep(&self) -> Ctx<'a> {
        *self
    }
}

/// A span pointing at the very start of the source, used when an AST node we are
/// lowering does not itself carry a `Span` (the emitter recurses into spanless
/// sub-`Expr` nodes; the enclosing item's span is the best locus we have, and is
/// threaded down). Errors prefer the nearest enclosing span the caller passes.
fn zero_span() -> Span {
    Span::new(0, 0)
}

/// Lower a whole `Program` to a single Verus source file (REQ-1). Emits the
/// fixed prelude, a `verus! { .. }` block holding (1) the `spec fn` definitions
/// of every combinator the program's contracts reference, (2) the lowered items
/// in source order with their shape-derived proof aids, and (3) a trailing
/// `fn main() {}`.
pub fn lower(program: &Program) -> Result<String, LowerError> {
    let mut out = String::new();
    out.push_str("use vstd::prelude::*;\n");
    out.push_str("verus! {\n");

    // (1) combinator spec-fn definitions used anywhere in the program (REQ-6).
    let combinator_defs = emit_combinator_defs(program)?;
    out.push_str(&combinator_defs);

    // (1b) Basis Stage 2 (`.design/basis/02-recursion-schemes.md` REQ-6/REQ-7):
    // the GENERATED per-(ADT, scheme) Verus recursive `spec fn`s
    // (`fold_<e>`/`for_all_<e>`/…) + the structural measure `<e>_len` + the
    // induction-discharged-once law `fold_bound_<e>`, materialized ONCE BEFORE
    // their first use (a scheme call lowers to a CALL of `fold_<e>`). EMPTY when
    // the program uses no scheme (byte-stable for the non-scheme corpus).
    let scheme_defs = emit_scheme_defs(program)?;
    out.push_str(&scheme_defs);

    // (1c) Basis Stage 4 (`.design/basis/04-collections.md` REQ-5): the
    // bounded-`Vec` wrapper struct + its verified `len`/`spec_get`/`get`/`push`
    // impl, materialized ONCE per element type the program uses (a `Vec<u64>`
    // param/return → `TVecU64`), BEFORE any fn references it. EMPTY when the
    // program uses no `Vec` (byte-stable for the existing corpus — no regression).
    // The GROUNDED `BVec`-over-`vstd::vec::Vec<u64>` form (verus `verified, 0
    // errors`): the `well_formed` capacity invariant, the no-OOB `get`, the
    // capacity-preserving `push` with the `final(self)` &mut postcondition.
    let vec_wrappers = emit_vec_wrappers(program)?;
    out.push_str(&vec_wrappers);

    // (1c.5) Cluster C12 (`.design/basis/13-map.md` REQ-4): the bounded verified
    // `Map<K, V>` wrapper struct `TMap<K,V>` over a `vstd::vec::Vec<(K, V)>`-of-pairs
    // backing + the spec abstraction view (`spec_dom`/`spec_contains_key`/`len`) +
    // its verified `contains_key`/`get`/`insert` ops, materialized ONCE per `(K, V)`
    // pair the program uses, BEFORE any fn references it. EMPTY when the program uses
    // no `Map` (byte-stable for the existing corpus — no regression). The GROUNDED
    // `TMapU64U64`-over-`vstd::vec::Vec<(u64,u64)>` form (verus `9 verified, 0
    // errors`): the `well_formed` capacity + key-uniqueness invariant, the no-OOB /
    // handled-or-loud `get -> Option<V>` (absent → None), the append-under-
    // `!contains_key` `insert` with the `final(self)` &mut postcondition.
    let map_wrappers = emit_map_wrappers(program)?;
    out.push_str(&map_wrappers);

    // (1d) Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): the bounded
    // `String` wrapper struct `TString` over `vstd::vec::Vec<u8>` + its verified
    // `well_formed`/`spec_len`/`len`/`spec_byte_at`/`byte_at`/`concat`/`slice`
    // impl, materialized ONCE when the program uses `String`, BEFORE any fn
    // references it. EMPTY when the program uses no `String` (byte-stable for the
    // existing corpus — no regression). The GROUNDED `TString`-over-
    // `vstd::vec::Vec<u8>` form (verus `verified, 0 errors`): the `well_formed`
    // capacity invariant, the no-OOB `byte_at` (`req i < len`), the bounded
    // `concat`/`slice` with the `final`-free owned-value construction.
    let string_wrapper = emit_string_wrapper(program)?;
    out.push_str(&string_wrapper);

    // (1d.5) Cluster C5 (`.design/basis/07-strings.md` REQ-13..16, issue #102): the
    // string search/transform module-scope definitions — the `occurs_at`/
    // `contains_sub`/`count_sep`/`sep_free`/`is_space` spec fns + the
    // `lemma_count_push` proof fn — materialized ONCE when the program uses a C5 op
    // (`program_uses_string_search`). The `TString` search METHODS (emitted by
    // `emit_string_wrapper`'s `emit_string_search_methods`) NAME these in their
    // contracts; verus resolves them order-independently within the single `verus!`
    // block. EMPTY otherwise (byte-stable — no regression). The GROUNDED forms (no
    // `assume`/`admit`/`external_body`, R-DEFER-9; `lemma_count_push` is a REAL
    // induction proof).
    let string_search_defs = emit_string_search_defs(program)?;
    out.push_str(&string_search_defs);

    // (1e) Cluster C4 (`.design/basis/07-strings.md` REQ-8, issue #94): the
    // generated `u64`→decimal-`String` round-trip definitions — the `pow10`/
    // `parse_le` spec fns, the `lemma_parse_push` append lemma, and the
    // `u64_to_string` exec fn (the divide/mod-by-10 digit-extraction loop with the
    // round-trip invariant `parse_le(data@) + m*pow10(len) == n` + `decreases m`) —
    // materialized ONCE when the program uses `n.to_string()`, BEFORE any fn
    // references them. EMPTY otherwise (byte-stable for the existing corpus — no
    // regression). The emitted form is EXACTLY the GROUNDED `16 verified, 0 errors`
    // round-trip (no `assume`/`external_body`, R-DEFER-9). Emitted AFTER the
    // `TString` wrapper because `u64_to_string` returns a `TString`.
    let numfmt_defs = emit_numfmt_defs(program)?;
    out.push_str(&numfmt_defs);

    // (1f) Cluster C7 (`.design/basis/09-option-result.md` REQ-5, issue #95): the
    // generated `String`→`u64` PARTIAL parser `parse_u64` (the C4 07-strings.md
    // REQ-9 payoff) — the `is_digit`/`all_digits` spec fns + the `parse_u64` exec fn
    // (the Horner-accumulate loop `acc = acc*10 + digit`, the three handled-or-loud
    // `None` arms — empty / non-digit / overflow — and the round-trip success
    // contract `ens match result { Some(v) => all_digits(s.data@) && s.data.len() >=
    // 1 && parse_be(s.data@) == v, None => true }`). Materialized ONCE when the
    // program calls `parse_u64` / names `all_digits`/`is_digit` (`program_uses_-
    // parse`), BEFORE any fn references it. EMPTY otherwise (byte-stable). The
    // emitted form is EXACTLY the GROUNDED `5 verified, 0 errors` parse (no
    // `assume`/`external_body`/`admit`, R-DEFER-9 — the round-trip is a REAL proof;
    // a broken `Some(0)` FAILS). `parse_be` is shared with the numfmt round-trip; it
    // is emitted here ONLY when numfmt did not already emit it (dedup).
    let parse_defs = emit_parse_defs(program)?;
    out.push_str(&parse_defs);

    // The program-wide set of `nat`-returning spec fns (the head-fold-sum shape,
    // OQ-1) — SHAPE-derived, used to coerce `u64`/`nat` equalities (`as nat`). An
    // ADT match-fold-sum spec fn (`sum_list`, REQ-10) joins this set: it too
    // returns `nat` so its integer arithmetic stays `nat` (no overflow obligation
    // in spec context), exactly as the slice head-fold does.
    // Basis Stage 2 (`.design/basis/02-recursion-schemes.md` REQ-6): a `fold`
    // scheme-CALL instance (the only `nat`-result scheme — `Accumulator`) also
    // returns `nat`, so it joins the `nat_fns` set exactly as a hand-written
    // ADT-fold-sum does (an `Eq` against it coerces `as nat`). Detected by SHAPE:
    // the body tail is a `Call` whose callee path resolves to the `fold` scheme.
    let mut nat_fns: Vec<&str> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::SpecFn(s)
                if is_head_fold_sum(&s.body)
                    || is_adt_fold_sum(&s.body)
                    || is_fold_scheme_call_body(&s.body) =>
            {
                Some(s.name.as_str())
            }
            _ => None,
        })
        .collect();
    // Cluster C4 (`.design/basis/07-strings.md` REQ-8, issue #94): the generated
    // round-trip spec fns `parse_le`/`pow10` return `nat`, so when the program names
    // them they join `nat_fns` — an `Eq` against `parse_le(...)` (the round-trip
    // `ens parse_le(result) == n`) coerces the scalar `u64` side `as nat` exactly as
    // a hand-written ADT-fold-sum does. Added only when numfmt is in use (byte-stable
    // for the non-numfmt corpus).
    if program_uses_numfmt(program) {
        for name in GENERATED_NUMFMT_SPEC_FNS {
            nat_fns.push(name);
        }
    }
    // Cluster C5 (`.design/basis/07-strings.md` REQ-15, issue #102): the generated
    // `count_sep` spec fn returns `nat`, so when the program uses a C5 op it joins
    // `nat_fns` — `split`'s `ens result.len() == 1 + count_sep(s@, sep)` coerces the
    // scalar `result.len()` side `as nat` exactly as a hand-written ADT-fold-sum does.
    // (The other C5 spec fns — `occurs_at`/`contains_sub`/`sep_free`/`is_space` —
    // return `bool`, so they do NOT join `nat_fns`.) Added only when a C5 op is in use
    // (byte-stable for the non-C5 corpus).
    if program_uses_string_search(program) {
        nat_fns.push("count_sep");
    }

    // The program-wide set of `struct` names that carry a type-invariant (REQ-8,
    // OQ-3 automatic threading): every `fn` taking or returning such a struct gets
    // the `<param>.well_formed()` / `result.well_formed()` conjunct woven into its
    // `requires`/`ensures` so Verus enforces the invariant at construction + use.
    let inv_structs: Vec<&str> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(s) if s.inv.is_some() => Some(s.name.as_str()),
            _ => None,
        })
        .collect();

    // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): the program-wide set of
    // FIELD names whose declared type reaches `String` — the editor core's `Buf {
    // text: String, .. }`. A contract reading `b.text.len()` / `result.text.len()`
    // (a String FIELD access receiver) must rewrite `.len()`/`.byte_at(i)` to the
    // wrapper SPEC fns `.spec_len()`/`.spec_byte_at(i as int)` (the exec `len`/
    // `byte_at` return `u64` and cannot be named in a contract — the same rule the
    // bare-`String`-value rewrite applies). Threaded into every fn's spec `Ctx`
    // (sorted+deduped for determinism, R-CODE-5). A field name is keyed alone (no
    // struct qualifier): v0.1 has no field-name overload across a `String` field and
    // a non-`String` field of the same name in scope, and the rewrite is inert
    // unless the method is `len`/`byte_at`.
    let mut string_field_names: Vec<&str> = program
        .items
        .iter()
        .flat_map(|item| -> Box<dyn Iterator<Item = &str>> {
            match item {
                Item::Struct(s) => Box::new(
                    s.fields
                        .iter()
                        .filter(|fd| ty_reaches_string(&fd.ty))
                        .map(|fd| fd.name.as_str()),
                ),
                Item::Enum(e) => Box::new(e.variants.iter().flat_map(|v| {
                    let fields: &[thermite_syntax::ast::FieldDef] = match &v.shape {
                        thermite_syntax::ast::VariantShape::Struct(fds) => fds,
                        _ => &[],
                    };
                    fields
                        .iter()
                        .filter(|fd| ty_reaches_string(&fd.ty))
                        .map(|fd| fd.name.as_str())
                })),
                _ => Box::new(std::iter::empty()),
            }
        })
        .collect();
    string_field_names.sort_unstable();
    string_field_names.dedup();

    // The program-wide `(variant_name, enum_name)` map (REQ-9): drives the
    // ENUM-QUALIFIED `Enum::Variant` lowering of a `match` arm / pattern over a
    // user enum value (verus rejects a bare `Nil`/`Circle`). Built once, threaded
    // through every `fn`/`spec fn` body's match lowering.
    let variants: Vec<(&str, &str)> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(e) => Some(e),
            _ => None,
        })
        .flat_map(|e| {
            e.variants
                .iter()
                .map(move |v| (v.name.as_str(), e.name.as_str()))
        })
        .collect();

    // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4, issue #127): the
    // program-wide names of USER `spec fn`s that declare a `String`/`&String` param
    // — the byte-view dispatch's SHAPE key. A USER spec fn (the #126 String-scanning
    // shape) lowers its param to `&TString`, so a `String` arg to it (e.g. a
    // recursive self-call `is_digit(s, i+1, ..)`) must pass the REFERENCE through,
    // NOT the `.data@` byte view the GENERATED `Seq<u8>` byte-view fns want. A user
    // spec fn NAMED like a generated one (`spec fn is_digit(s: &String, ..)`) lives
    // in the user namespace and SHADOWS the generated name — so `is_user_string_spec_fn`
    // excludes it from the byte-view set (`callee_takes_string_byteview`), keyed on
    // its PARAM SHAPE, not its name (#127). The generated fns are synthesized and so
    // are NOT in `program.items`, hence not in this set — they still byte-view.
    // Sorted+deduped for determinism (R-CODE-5). EMPTY for a program with no
    // String-param user spec fn (byte-stable for the existing corpus).
    let user_string_spec_fns: Vec<&str> = user_string_spec_fn_names(program);

    // (2) the lowered items, in source order (determinism, §5.3). A `fn` whose
    // loop carries an accumulator-fold invariant pulls in the auto-generated
    // push lemma for the folded spec fn (REQ-7 template a); the lemma def is
    // emitted at file scope right before the `fn` that uses it, deduped.
    let mut emitted_lemmas: Vec<String> = Vec::new();
    for item in &program.items {
        let item_src = match item {
            Item::SpecFn(s) => lower_spec_fn(s, &variants, &user_string_spec_fns, program)?,
            Item::Fn(f) if f.boundary.is_some() || f.slag.is_some() => {
                // A boundary/slag fn is woven as a `#[verifier::external_body]`
                // signature (its body is never lowered, REQ-1), so it needs no
                // accumulator-fold push lemmas — skip the lemma collection that a
                // fully-proved fn body drives.
                lower_fn(
                    f,
                    &nat_fns,
                    &inv_structs,
                    &string_field_names,
                    &variants,
                    &user_string_spec_fns,
                )?
            }
            Item::Fn(f) => {
                for lemma_def in push_lemma_defs_for_fn(f)? {
                    let name_line = lemma_def.lines().next().unwrap_or("").to_string();
                    if emitted_lemmas.iter().any(|n| n == &name_line) {
                        continue;
                    }
                    out.push('\n');
                    out.push_str(&lemma_def);
                    out.push('\n');
                    emitted_lemmas.push(name_line);
                }
                lower_fn(
                    f,
                    &nat_fns,
                    &inv_structs,
                    &string_field_names,
                    &variants,
                    &user_string_spec_fns,
                )?
            }
            // Basis Stage 1c (`.design/basis/01-adts.md` REQ-8/REQ-10): a
            // `struct` lowers to a Verus `pub struct` + the `well_formed`
            // type-invariant predicate (REQ-8); a (recursive) `enum` lowers to a
            // Verus `enum` with `Box<T>` at the recursive occurrence (REQ-10).
            Item::Struct(s) => lower_struct(s)?,
            Item::Enum(e) => lower_enum(e)?,
        };
        out.push('\n');
        out.push_str(&item_src);
        out.push('\n');
    }

    out.push_str("\n}\nfn main() {}\n");
    Ok(out)
}

// ---------------------------------------------------------------------------
// REQ-8/REQ-9/REQ-10: ADT item lowering (struct, enum, recursive enum).
// ---------------------------------------------------------------------------

/// Lower a `StructItem` to a Verus `pub struct` plus, when it carries an `inv`
/// clause, the `well_formed` type-invariant predicate (REQ-8). The GROUNDED form
/// (`.design/basis/01-adts.md` "Struct + type invariant", verus `0 errors`):
///
/// ```verus
/// pub struct Account { pub balance: u64 }
/// impl Account {
///     pub open spec fn well_formed(&self) -> bool { self.balance <= 1000000 }
/// }
/// ```
///
/// VISIBILITY TIER (the recorded finding, REQ-8): a `pub open spec fn` body may
/// refer only to `pub` items, so the struct, ITS FIELDS, and the predicate are
/// all emitted `pub` — otherwise verus rejects with `field expression for a
/// non-visible datatype`. The `inv` expression is lowered with bare field-name
/// paths rewritten to `self.<field>` (the predicate's receiver), the
/// data-invariant the corpus `inv balance <= 1_000_000` denotes.
fn lower_struct(s: &thermite_syntax::ast::StructItem) -> Result<String, LowerError> {
    let mut out = String::new();
    writeln!(out, "pub struct {} {{", s.name).ok();
    for field in &s.fields {
        let ty = lower_type(&field.ty)?;
        writeln!(out, "    pub {}: {ty},", field.name).ok();
    }
    out.push_str("}\n");

    // The type-invariant predicate (REQ-8), when an `inv` clause is present. A
    // struct WITHOUT an invariant is a plain `pub struct` (no predicate, nothing
    // to thread — the OQ-3 threading in `lower_fn_signature` keys on `inv_structs`
    // which is exactly the invariant-bearing set).
    if let Some(inv) = &s.inv {
        let field_names: Vec<&str> = s.fields.iter().map(|f| f.name.as_str()).collect();
        // The subset of fields whose type reaches `String` (REQ-4): a `String`
        // field's `<field>.len()` / `<field>.byte_at(i)` inside the spec-position
        // `well_formed` predicate must name the wrapper SPEC fns
        // `.spec_len()`/`.spec_byte_at(i as int)` (the exec `len`/`byte_at` return
        // `u64` and cannot be named in a contract — the same rule the fn-signature
        // String rewrite applies). The editor core `inv cursor <= text.len()`.
        let string_fields: Vec<&str> = s
            .fields
            .iter()
            .filter(|f| ty_reaches_string(&f.ty))
            .map(|f| f.name.as_str())
            .collect();
        let body = lower_inv_expr(&inv.expr, &field_names, &string_fields, 0, s.span)?;
        writeln!(out, "\nimpl {} {{", s.name).ok();
        out.push_str("    pub open spec fn well_formed(&self) -> bool {\n");
        writeln!(out, "        {body}").ok();
        out.push_str("    }\n}\n");
    }
    Ok(out)
}

/// Lower an `inv` expression to the `well_formed(&self)` predicate body (REQ-8):
/// a bare single-segment path that names a declared field is rewritten to
/// `self.<field>` (the invariant `balance <= 1_000_000` is about `self.balance`).
/// Everything else lowers in spec position via the shared `lower_expr` — but the
/// field rewrite must happen on the AST, so this walks the expression itself.
fn lower_inv_expr(
    expr: &Expr,
    field_names: &[&str],
    string_fields: &[&str],
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    if depth >= MAX_EMIT_DEPTH {
        return Err(LowerError::TooDeep {
            limit: MAX_EMIT_DEPTH,
            span,
        });
    }
    let d = depth + 1;
    match expr {
        // A bare field-name path becomes a `self.<field>` field access; any other
        // single/multi-segment path lowers normally (a `spec const` like a CAP,
        // or a `::`-qualified path, stays as written).
        Expr::Path(segs) => {
            if segs.len() == 1 && field_names.contains(&segs[0].as_str()) {
                Ok(format!("self.{}", segs[0]))
            } else {
                Ok(segs.join("::"))
            }
        }
        Expr::Binary { op, lhs, rhs } => {
            let l = lower_inv_operand(lhs, *op, true, field_names, string_fields, d, span)?;
            let r = lower_inv_operand(rhs, *op, false, field_names, string_fields, d, span)?;
            Ok(format!("{l} {} {r}", binop(*op)))
        }
        Expr::Field { receiver, name } => {
            let r = lower_inv_expr(receiver, field_names, string_fields, d, span)?;
            Ok(format!("{r}.{name}"))
        }
        // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): a method call inside
        // the spec-position `well_formed` predicate — the editor core `inv cursor <=
        // text.len()`. The receiver's bare field name is rewritten to `self.<field>`
        // (recursively, so a nested field receiver works too). When the receiver is a
        // `String`-typed field, `.len()`/`.byte_at(i)` rewrite to the wrapper SPEC fns
        // `.spec_len()`/`.spec_byte_at(i as int)` — the exec `len`/`byte_at` return
        // `u64` and CANNOT be named in a contract (the same rule the fn-signature
        // String rewrite applies; `lower_expr` MethodCall spec arm). A non-`String`
        // field's method call (e.g. a `Vec` field's `.len()`, whose wrapper spec fn IS
        // `len`) keeps the method name unchanged. Without this arm `text.len()` fell to
        // the catch-all `lower_expr`, which lowered the bare receiver `text` with NO
        // `self.` rewrite (`error[E0425]: cannot find value text`).
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            let r = lower_inv_expr(receiver, field_names, string_fields, d, span)?;
            let recv_is_string_field = matches!(
                receiver.as_ref(),
                Expr::Path(segs) if segs.len() == 1 && string_fields.contains(&segs[0].as_str())
            );
            if recv_is_string_field {
                if name == "len" && args.is_empty() {
                    return Ok(format!("{r}.spec_len()"));
                }
                if name == "byte_at" && args.len() == 1 {
                    // `spec_byte_at(i: int)`: an integer literal flows into the `int`
                    // parameter directly (Verus coerces a literal); a non-literal
                    // index gets the explicit `as int` Verus requires in spec
                    // position (the same split as the fn-signature byte_at rewrite).
                    let idx = if matches!(&args[0], Expr::IntLit { .. }) {
                        lower_inv_expr(&args[0], field_names, string_fields, d, span)?
                    } else {
                        lower_index_arg(&args[0], Ctx::spec_seq(), d, span)?
                    };
                    return Ok(format!("{r}.spec_byte_at({idx})"));
                }
            }
            let mut parts = Vec::with_capacity(args.len());
            for a in args {
                parts.push(lower_inv_expr(a, field_names, string_fields, d, span)?);
            }
            Ok(format!("{r}.{name}({})", parts.join(", ")))
        }
        // A cast inside the `well_formed` predicate (`inv (x as u32) < cap`,
        // blocker #148): the cast INNER must recurse through `lower_inv_expr` so a
        // bare field name is rewritten to `self.<field>` (the catch-all
        // `lower_expr` would emit the bare `x as u32` — `cannot find value x`).
        // The #122 inner-paren discipline is preserved (a `Binary`/`Unary` inner is
        // parenthesized: `(a - b) as T`). The target type lowers via `lower_type`
        // (a struct invariant is `bool`-returning, never `nat_ret`).
        Expr::Cast { expr: inner, ty } => {
            let e = lower_inv_expr(inner, field_names, string_fields, d, span)?;
            let t = lower_type(ty)?;
            let e = if matches!(inner.as_ref(), Expr::Binary { .. } | Expr::Unary { .. }) {
                format!("({e})")
            } else {
                e
            };
            Ok(format!("{e} as {t}"))
        }
        // A literal / other leaf lowers exactly as the shared spec lowering would
        // (the field rewrite only matters for bare paths and their parents).
        _ => lower_expr(expr, Ctx::spec_seq(), depth, span),
    }
}

/// Parenthesize an `inv` binary operand the same way `lower_binary_operand` does,
/// but recursing through `lower_inv_expr` so nested field-name paths are rewritten
/// (REQ-8). Mirrors the precedence discipline of the exec/spec operand lowering.
fn lower_inv_operand(
    operand: &Expr,
    parent: BinOp,
    is_left: bool,
    field_names: &[&str],
    string_fields: &[&str],
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    let s = lower_inv_expr(operand, field_names, string_fields, depth, span)?;
    if let Expr::Binary { op: child, .. } = operand {
        let pp = precedence(parent);
        let cp = precedence(*child);
        let needs = cp < pp || (!is_left && cp == pp);
        if needs {
            return Ok(format!("({s})"));
        }
    }
    // Blocker #148 (the #146/#139/#142 cast-`<` class, on the struct
    // type-invariant path this parenthesizer missed): a `Cast` LEFT operand of a
    // `<`-leading op (`<`/`<=`/`<<`) is AMBIGUOUS — `self.x as u32 < self.cap`
    // mis-parses the `u32 <` as a generic-argument list (`u32<…>`, "expected
    // `,`"). Same guard, same `is_lt_leading` predicate as `lower_binary_operand`
    // (R-DEFER-8: the cast-`<` convention is uniform across every emission site).
    if is_left && matches!(operand, Expr::Cast { .. }) && is_lt_leading(parent) {
        return Ok(format!("({s})"));
    }
    Ok(s)
}

/// Lower an `EnumItem` to a Verus `enum` (REQ-9), recursive `Box<T>` and all
/// (REQ-10). The GROUNDED forms (`.design/basis/01-adts.md`, verus `0 errors`):
///
/// ```verus
/// enum Shape { Circle(u64), Rect { w: u64, h: u64 } }
/// enum List { Nil, Cons(u64, Box<List>) }
/// ```
///
/// A unit variant is the bare name; a tuple variant `Name(T, …)`; a struct
/// variant `Name { field: T, … }`. The recursive occurrence is a `Box<List>`
/// (`lower_type` emits `Box<…>` for `Type::Box`), the heap indirection Verus
/// dereferences with `*` (REQ-10). An enum is emitted WITHOUT `pub`: the corpus
/// `Shape`/`List` are used only from `fn`/`spec fn` in the same module, and a bare
/// `enum` matches the GROUNDED verified form (no `pub open spec fn` refers to it,
/// so no visibility-tier obligation as the struct invariant has).
fn lower_enum(e: &EnumItem) -> Result<String, LowerError> {
    let mut out = String::new();
    writeln!(out, "enum {} {{", e.name).ok();
    for variant in &e.variants {
        match &variant.shape {
            VariantShape::Unit => {
                writeln!(out, "    {},", variant.name).ok();
            }
            VariantShape::Tuple(tys) => {
                let mut parts = Vec::with_capacity(tys.len());
                for ty in tys {
                    parts.push(lower_type(ty)?);
                }
                writeln!(out, "    {}({}),", variant.name, parts.join(", ")).ok();
            }
            VariantShape::Struct(fields) => {
                let mut parts = Vec::with_capacity(fields.len());
                for field in fields {
                    parts.push(format!("{}: {}", field.name, lower_type(&field.ty)?));
                }
                writeln!(out, "    {} {{ {} }},", variant.name, parts.join(", ")).ok();
            }
        }
    }
    out.push_str("}\n");
    Ok(out)
}

// ---------------------------------------------------------------------------
// REQ-6: combinator Verus(L3) definitions, sourced from the #2 registry seam.
// ---------------------------------------------------------------------------

/// Collect (in deterministic source order, deduped) the combinator names the
/// program references anywhere in a contract/spec position, and emit each one's
/// frozen `verus_l3` `spec fn` definition from the `thermite-spec` registry
/// (REQ-6; closes the OQ-2 seam — this is the registry's #4 consumer per
/// R-DEFER-1). A referenced name with no registry entry is `UnknownCombinator`.
fn emit_combinator_defs(program: &Program) -> Result<String, LowerError> {
    let mut names: Vec<(String, Span)> = Vec::new();
    for item in &program.items {
        match item {
            Item::Fn(f) => {
                collect_combinators_in_expr(&f.contract.req.expr, f.span, &mut names);
                for ens in &f.contract.ens {
                    collect_combinators_in_expr(&ens.expr, f.span, &mut names);
                }
                // A boundary fn (ffi-boundary.md REQ-2) has `body: None` — its
                // `req`/`ens` combinators are collected above; no body to scan.
                if let Some(body) = &f.body {
                    collect_combinators_in_block_specs(body, f.span, &mut names);
                }
            }
            Item::SpecFn(s) => {
                collect_combinators_in_expr(&s.dec.expr, s.span, &mut names);
                collect_combinators_in_block_specs(&s.body, s.span, &mut names);
            }
            // Basis Stage 1a (`.design/basis/01-adts.md`): a `struct`/`enum`
            // item carries no contract clauses, so it references no combinators
            // — the neutral value for this collector is a no-op. (The item is
            // gated at the validator anyway; this arm is dead-in-1a.)
            Item::Struct(_) | Item::Enum(_) => {}
        }
    }

    let mut out = String::new();
    let mut emitted: Vec<&str> = Vec::new();
    for (name, span) in &names {
        if emitted.iter().any(|e| e == name) {
            continue;
        }
        let sig = thermite_spec::lookup(name).ok_or_else(|| LowerError::UnknownCombinator {
            name: name.clone(),
            span: *span,
        })?;
        out.push('\n');
        out.push_str(sig.verus_l3);
        out.push('\n');
        emitted.push(sig.name);
    }
    Ok(out)
}

/// Walk an expression collecting any callee path whose head segment is a
/// registered combinator name. Combinator calls are plain `Expr::Call` with a
/// `Path` callee (the frontend is registry-free — `ast.rs` module doc).
fn collect_combinators_in_expr(expr: &Expr, span: Span, acc: &mut Vec<(String, Span)>) {
    match expr {
        Expr::Call { callee, args } => {
            if let Expr::Path(segs) = callee.as_ref() {
                if let Some(last) = segs.last() {
                    if thermite_spec::lookup(last).is_some() {
                        acc.push((last.clone(), span));
                    }
                }
            }
            collect_combinators_in_expr(callee, span, acc);
            for a in args {
                collect_combinators_in_expr(a, span, acc);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_combinators_in_expr(receiver, span, acc);
            for a in args {
                collect_combinators_in_expr(a, span, acc);
            }
        }
        Expr::Field { receiver, .. } => collect_combinators_in_expr(receiver, span, acc),
        Expr::Closure { body, .. } => collect_combinators_in_expr(body, span, acc),
        Expr::Match { scrutinee, arms } => {
            collect_combinators_in_expr(scrutinee, span, acc);
            for arm in arms {
                // A C10 match guard is an `Expr` that may carry a combinator
                // (`.design/basis/11-ergonomics.md` REQ-3).
                if let Some(guard) = &arm.guard {
                    collect_combinators_in_expr(guard, span, acc);
                }
                collect_combinators_in_expr(&arm.body, span, acc);
            }
        }
        Expr::If { cond, then, else_ } => {
            collect_combinators_in_expr(cond, span, acc);
            collect_combinators_in_block_specs(then, span, acc);
            collect_combinators_in_block_specs(else_, span, acc);
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_combinators_in_expr(lhs, span, acc);
            collect_combinators_in_expr(rhs, span, acc);
        }
        Expr::Index { base, index } => {
            collect_combinators_in_expr(base, span, acc);
            match index {
                IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
                    collect_combinators_in_expr(e, span, acc)
                }
                IndexArg::Range(a, b) => {
                    collect_combinators_in_expr(a, span, acc);
                    collect_combinators_in_expr(b, span, acc);
                }
            }
        }
        Expr::Cast { expr, .. } | Expr::Ref { expr, .. } => {
            collect_combinators_in_expr(expr, span, acc)
        }
        // Basis Stage 1a (`.design/basis/01-adts.md`): the ADT expressions are
        // dead-in-1a (gated at the validator), but the honest collector value
        // is to descend into their sub-expressions — a combinator could in
        // principle appear in a struct-literal field value, an `is` scrutinee,
        // or a deref operand — so no referenced combinator is silently dropped.
        Expr::StructLit { fields, .. } => {
            for (_, value) in fields {
                collect_combinators_in_expr(value, span, acc);
            }
        }
        Expr::Is { scrutinee, .. } => collect_combinators_in_expr(scrutinee, span, acc),
        Expr::Deref(inner) => collect_combinators_in_expr(inner, span, acc),
        // The prefix `!` (#92): descend into the operand so a combinator inside
        // `!forall_in(...)` is still collected (recurse like the other unary arms).
        Expr::Unary { expr, .. } => collect_combinators_in_expr(expr, span, acc),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): a
        // combinator could appear in any tuple element or under a projection's
        // receiver — descend into both so no referenced combinator is dropped.
        Expr::Tuple(elems) => {
            for e in elems {
                collect_combinators_in_expr(e, span, acc);
            }
        }
        Expr::TupleProj { receiver, .. } => collect_combinators_in_expr(receiver, span, acc),
        // A string literal (`.design/basis/07-strings.md` REQ-1) references no
        // combinator — a value-carrying leaf, like an int/bool literal.
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
    }
}

/// Walk a block collecting combinators referenced in its SPEC positions: loop
/// `inv`/`dec` clauses. Body exec expressions never reference combinators in the
/// corpus, but loop invariants do (`binary_search`'s `forall_below`/`forall_from`).
fn collect_combinators_in_block_specs(block: &Block, span: Span, acc: &mut Vec<(String, Span)>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(l) => {
                for inv in &l.invs {
                    collect_combinators_in_expr(&inv.expr, span, acc);
                }
                collect_combinators_in_expr(&l.dec.expr, span, acc);
                collect_combinators_in_block_specs(&l.body, span, acc);
            }
            Stmt::If { then, else_, .. } => {
                collect_combinators_in_block_specs(then, span, acc);
                if let Some(e) = else_ {
                    collect_combinators_in_block_specs(e, span, acc);
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Basis Stage 2 (`.design/basis/02-recursion-schemes.md` REQ-6/REQ-7): the
// GENERATED per-(ADT, scheme) Verus recursive `spec fn`s + the discharged-once
// law. A scheme call `fold(l, init, step)` lowers to a CALL of the generated
// `fold_<e>`, materialized here ONCE before its first use (the OQ-1 (b)
// MATERIALIZED-items resolution — shared across instances, in the audit surface).
// ---------------------------------------------------------------------------

/// A single (recursion scheme, recursive ADT) pair the program uses, resolved by
/// SHAPE: a scheme call whose scrutinee path resolves to a `spec fn` parameter of
/// a declared `enum` type. The lowerer materializes the scheme's generated
/// `spec fn` over this ADT (`fold_<e>`/`for_all_<e>`/…) + the structural measure +
/// the `fold_bound_<e>` law (REQ-6/REQ-7).
struct SchemeUse {
    scheme: &'static thermite_spec::SchemeSig,
    /// The declared `enum` the scheme folds over (the scrutinee's type).
    enum_name: String,
    /// The element type of the ADT's recursive variant (the first non-`Box`
    /// field — `u64` for `Cons(u64, Box<List>)`), the step's element parameter
    /// type. The GROUNDED forms are all `u64`-element.
    elem_ty: String,
    /// The recursive variant's name (`Cons`) and the base (unit/value) variant(s),
    /// resolved from the enum decl, so the generated `match` is ENUM-QUALIFIED and
    /// recurses through the `Box`-deref'd recursive field.
    enum_item: EnumItem,
}

/// Emit the GENERATED scheme definitions for every (scheme, ADT) pair the program
/// uses (REQ-6/REQ-7), in a DETERMINISTIC order (R-CODE-5), deduped. For each ADT
/// that ANY scheme folds over, the structural measure `<e>_len` is emitted once;
/// for each used scheme over that ADT the recursive `spec fn` (`fold_<e>`/
/// `for_all_<e>`/…); and for each `fold` over that ADT the induction law
/// `fold_bound_<e>`. EMPTY when the program uses no scheme (the non-scheme corpus
/// is byte-stable — no regression). The forms reproduce the GROUNDED Verus
/// (`9 verified, 0 errors`) of `.design/basis/02-recursion-schemes.md`.
fn emit_scheme_defs(program: &Program) -> Result<String, LowerError> {
    let uses = collect_scheme_uses(program)?;
    if uses.is_empty() {
        return Ok(String::new());
    }

    let mut out = String::new();

    // (a) the structural measure `<e>_len` once per ADT any scheme folds over —
    // the `fold_bound_<e>` law's `len_<e>(l) * b` bound references it. Deduped by
    // enum name, deterministic source-order traversal.
    let mut measured: Vec<String> = Vec::new();
    for u in &uses {
        if measured.iter().any(|m| m == &u.enum_name) {
            continue;
        }
        out.push('\n');
        out.push_str(&emit_len_measure(u)?);
        out.push('\n');
        measured.push(u.enum_name.clone());
    }

    // (b) each used scheme's generated recursive `spec fn`, deduped by the
    // generated name (`fold_list`/`for_all_list`/…).
    let mut emitted: Vec<String> = Vec::new();
    for u in &uses {
        let name = u.scheme.generated_fn_name(&u.enum_name);
        if emitted.iter().any(|e| e == &name) {
            continue;
        }
        out.push('\n');
        out.push_str(&emit_scheme_spec_fn(u)?);
        out.push('\n');
        emitted.push(name);
    }

    // (c) the `fold_bound_<e>` induction-discharged-once law for each ADT a `fold`
    // folds over (REQ-7). Deduped by enum name.
    let mut lawed: Vec<String> = Vec::new();
    for u in &uses {
        if u.scheme.name != "fold" {
            continue;
        }
        if lawed.iter().any(|m| m == &u.enum_name) {
            continue;
        }
        out.push('\n');
        out.push_str(&emit_fold_bound_law(u)?);
        out.push('\n');
        lawed.push(u.enum_name.clone());
    }

    Ok(out)
}

/// Collect every (scheme, ADT) pair the program uses (REQ-6). A scheme call is an
/// `Expr::Call` whose callee `Path` resolves via `thermite_spec::schemes::lookup`;
/// the ADT is the type of its scrutinee (first) argument, resolved against the
/// enclosing `spec fn`/`fn`'s parameter types (the AST is untyped — OQ-3 — so the
/// scrutinee path → param-type resolution is the SHAPE-decidable mapping). A
/// scheme over a value whose type is not a declared `enum` is `Unsupported`
/// (REQ-9 — never a panic).
fn collect_scheme_uses(program: &Program) -> Result<Vec<SchemeUse>, LowerError> {
    // The declared enums, by name.
    let enums: std::collections::BTreeMap<&str, &EnumItem> = program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Enum(e) => Some((e.name.as_str(), e)),
            _ => None,
        })
        .collect();

    let mut uses: Vec<SchemeUse> = Vec::new();
    for item in &program.items {
        let (params, body, span) = match item {
            Item::SpecFn(s) => (&s.params, &s.body, s.span),
            // A scheme in an exec `fn` body is the MONOMORPHIZED exec form (OQ-2);
            // the v0.1 corpus is spec-only, so an exec scheme is out of the
            // grounded path — collect spec-fn scheme uses only here.
            _ => continue,
        };
        collect_scheme_uses_in_block(body, params, &enums, span, &mut uses)?;
    }
    Ok(uses)
}

/// Walk a `spec fn` body block collecting scheme uses (REQ-6).
fn collect_scheme_uses_in_block(
    block: &Block,
    params: &[Param],
    enums: &std::collections::BTreeMap<&str, &EnumItem>,
    span: Span,
    uses: &mut Vec<SchemeUse>,
) -> Result<(), LowerError> {
    for stmt in &block.stmts {
        if let Stmt::Expr(e) | Stmt::Return(Some(e)) | Stmt::Let { init: e, .. } = stmt {
            collect_scheme_uses_in_expr(e, params, enums, span, uses)?;
        }
    }
    if let Some(tail) = &block.tail {
        collect_scheme_uses_in_expr(tail, params, enums, span, uses)?;
    }
    Ok(())
}

/// Walk an expression collecting scheme uses (REQ-6). A scheme call's scrutinee
/// path is resolved to an enclosing parameter's `enum` type.
fn collect_scheme_uses_in_expr(
    expr: &Expr,
    params: &[Param],
    enums: &std::collections::BTreeMap<&str, &EnumItem>,
    span: Span,
    uses: &mut Vec<SchemeUse>,
) -> Result<(), LowerError> {
    if let Expr::Call { callee, args } = expr {
        if let Expr::Path(segs) = callee.as_ref() {
            if let Some(name) = segs.last() {
                if let Some(scheme) = thermite_spec::schemes::lookup(name) {
                    let enum_item = resolve_scheme_adt(scheme, args, params, enums, span)?;
                    let elem_ty = recursive_elem_type(&enum_item, span)?;
                    if !uses
                        .iter()
                        .any(|u| u.scheme.name == scheme.name && u.enum_name == enum_item.name)
                    {
                        uses.push(SchemeUse {
                            scheme,
                            enum_name: enum_item.name.clone(),
                            elem_ty,
                            enum_item,
                        });
                    }
                }
            }
        }
    }
    // Recurse sub-expressions (a scheme call may be nested in arithmetic — though
    // the validator caps the step to a flat closure, the instance body itself can
    // wrap the call, e.g. `fold(...) + 0`). The depth is bounded by the source
    // structure; the validator already enforced the contract limit.
    each_subexpr(expr, &mut |e| {
        collect_scheme_uses_in_expr(e, params, enums, span, uses)
    })
}

/// Apply `f` to each immediate sub-expression of `expr` (a shared structural
/// walk), short-circuiting on the first `Err`. Used by the scheme-use collector.
fn each_subexpr(
    expr: &Expr,
    f: &mut impl FnMut(&Expr) -> Result<(), LowerError>,
) -> Result<(), LowerError> {
    match expr {
        Expr::Call { callee, args } => {
            f(callee)?;
            for a in args {
                f(a)?;
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            f(receiver)?;
            for a in args {
                f(a)?;
            }
        }
        Expr::Field { receiver, .. } => f(receiver)?,
        Expr::Closure { body, .. } => f(body)?,
        Expr::Match { scrutinee, arms } => {
            f(scrutinee)?;
            for arm in arms {
                // A C10 match guard is a sub-expression too
                // (`.design/basis/11-ergonomics.md` REQ-3).
                if let Some(guard) = &arm.guard {
                    f(guard)?;
                }
                f(&arm.body)?;
            }
        }
        Expr::If { cond, then, else_ } => {
            f(cond)?;
            each_block_subexpr(then, f)?;
            each_block_subexpr(else_, f)?;
        }
        Expr::Binary { lhs, rhs, .. } => {
            f(lhs)?;
            f(rhs)?;
        }
        Expr::Index { base, index } => {
            f(base)?;
            match index {
                IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => f(e)?,
                IndexArg::Range(a, b) => {
                    f(a)?;
                    f(b)?;
                }
            }
        }
        Expr::Cast { expr, .. } | Expr::Ref { expr, .. } | Expr::Deref(expr) => f(expr)?,
        // The prefix `!` (#92): descend into its single operand.
        Expr::Unary { expr, .. } => f(expr)?,
        Expr::StructLit { fields, .. } => {
            for (_, v) in fields {
                f(v)?;
            }
        }
        Expr::Is { scrutinee, .. } => f(scrutinee)?,
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): the
        // immediate sub-expressions of a tuple are its elements; of a projection,
        // its receiver — a scheme use can hide in any of them.
        Expr::Tuple(elems) => {
            for e in elems {
                f(e)?;
            }
        }
        Expr::TupleProj { receiver, .. } => f(receiver)?,
        // A string literal (`.design/basis/07-strings.md` REQ-1) is a value-
        // carrying leaf with no sub-expression — like an int/bool literal.
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
    }
    Ok(())
}

/// Apply `f` to each sub-expression of a block (for `each_subexpr`'s `If` arms).
fn each_block_subexpr(
    block: &Block,
    f: &mut impl FnMut(&Expr) -> Result<(), LowerError>,
) -> Result<(), LowerError> {
    for stmt in &block.stmts {
        if let Stmt::Expr(e) | Stmt::Return(Some(e)) | Stmt::Let { init: e, .. } = stmt {
            f(e)?;
        }
    }
    if let Some(tail) = &block.tail {
        f(tail)?;
    }
    Ok(())
}

/// Resolve the declared `enum` a scheme call folds over (REQ-6): the scrutinee
/// (first) argument is a bare path naming a parameter whose type is `Named(E)`
/// for a declared `enum E`. An un-resolvable scrutinee (non-path, unknown param,
/// non-enum type) is `Unsupported` (REQ-9 — a scheme over a non-ADT value).
fn resolve_scheme_adt(
    scheme: &thermite_spec::SchemeSig,
    args: &[Expr],
    params: &[Param],
    enums: &std::collections::BTreeMap<&str, &EnumItem>,
    span: Span,
) -> Result<EnumItem, LowerError> {
    let scrutinee = args.first().ok_or_else(|| LowerError::Unsupported {
        what: format!(
            "recursion scheme `{}` with no scrutinee argument",
            scheme.name
        ),
        span,
    })?;
    let Expr::Path(segs) = scrutinee else {
        return Err(LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` scrutinee must be a bare value path",
                scheme.name
            ),
            span,
        });
    };
    let pname = segs.last().map(|s| s.as_str()).unwrap_or_default();
    let ty = params
        .iter()
        .find(|p| p.name == pname)
        .map(|p| &p.ty)
        .ok_or_else(|| LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` scrutinee `{pname}` is not a parameter",
                scheme.name
            ),
            span,
        })?;
    let Type::Named(enum_name) = ty else {
        return Err(LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` scrutinee `{pname}` is not a declared `enum` value",
                scheme.name
            ),
            span,
        });
    };
    enums
        .get(enum_name.as_str())
        .map(|e| (*e).clone())
        .ok_or_else(|| LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` over `{enum_name}`, which is not a declared `enum`",
                scheme.name
            ),
            span,
        })
}

/// The element type of an ADT's recursive variant — the first NON-`Box` field of
/// the variant that carries a `Box<Self>` recursive occurrence (`u64` for
/// `Cons(u64, Box<List>)`). The step's element parameter type (REQ-6). An enum
/// with no recursive `Box` variant is not a recursion-scheme target → `Unsupported`.
fn recursive_elem_type(e: &EnumItem, span: Span) -> Result<String, LowerError> {
    for variant in &e.variants {
        if let VariantShape::Tuple(tys) = &variant.shape {
            let has_box = tys.iter().any(|t| matches!(t, Type::Box(_)));
            if has_box {
                if let Some(elem) = tys.iter().find(|t| !matches!(t, Type::Box(_))) {
                    return lower_type(elem);
                }
            }
        }
    }
    Err(LowerError::Unsupported {
        what: format!(
            "recursion scheme over `{}`: no recursive `Box<{}>` variant with an element field",
            e.name, e.name
        ),
        span,
    })
}

/// The recursive variant (the one carrying a `Box<Self>`) and the variant set, so
/// the generated `match` is ENUM-QUALIFIED and recurses through the deref'd field.
/// Returns `(base_variants, recursive_variant_name)`. The base variants are every
/// non-recursive variant (unit `Nil` / value `Leaf(v)`).
fn enum_variant_split(e: &EnumItem) -> (Vec<&VariantDef>, Option<&VariantDef>) {
    let mut base = Vec::new();
    let mut recursive = None;
    for variant in &e.variants {
        let is_rec = matches!(&variant.shape, VariantShape::Tuple(tys) if tys.iter().any(|t| matches!(t, Type::Box(_))));
        if is_rec {
            recursive = Some(variant);
        } else {
            base.push(variant);
        }
    }
    (base, recursive)
}

/// Emit the structural measure `<e>_len(l: E) -> nat decreases l` (REQ-6/REQ-7):
/// the `len_list`-shaped count the `fold_bound_<e>` law multiplies. The recursive
/// arm counts `1 + <e>_len(*tail)`; each base arm contributes `0`. GROUNDED
/// (`list_len`, part of the `9 verified` run). The generated name is `<e>_len`
/// (e.g. `list_len`), DISTINCT from any surface `len_<e>` fold instance so the
/// two never collide (the corpus `len_list` is a `fold` instance).
fn emit_len_measure(u: &SchemeUse) -> Result<String, LowerError> {
    let e = &u.enum_item;
    let lname = format!("{}_len", e.name.to_ascii_lowercase());
    let (base, recursive) = enum_variant_split(e);
    let rec = recursive.ok_or_else(|| LowerError::Unsupported {
        what: format!("ADT `{}` has no recursive variant for a measure", e.name),
        span: zero_span(),
    })?;
    let mut out = String::new();
    writeln!(out, "spec fn {lname}(l: {}) -> nat", e.name).map_err(|_| fmt_err())?;
    out.push_str("    decreases l,\n{\n    match l {\n");
    for b in &base {
        writeln!(out, "        {}::{} => 0,", e.name, base_variant_pattern(b))
            .map_err(|_| fmt_err())?;
    }
    writeln!(
        out,
        "        {}::{}(x, tail) => 1 + {lname}(*tail),",
        e.name, rec.name
    )
    .map_err(|_| fmt_err())?;
    out.push_str("    }\n}\n");
    Ok(out)
}

/// The arm pattern for a BASE variant in a generated `match` (REQ-6): a unit
/// variant is its bare name (`Nil`); a value-carrying tuple base binds its field
/// (`Leaf(v)`). v0.1 grounded corpus is unit-base (`Nil`); the value-base shape
/// is supported for the `Tree` generalization.
fn base_variant_pattern(v: &VariantDef) -> String {
    match &v.shape {
        VariantShape::Unit => v.name.clone(),
        VariantShape::Tuple(tys) => {
            let binds: Vec<String> = (0..tys.len()).map(|i| format!("v{i}")).collect();
            format!("{}({})", v.name, binds.join(", "))
        }
        VariantShape::Struct(_) => v.name.clone(),
    }
}

/// Emit the generated recursive scheme `spec fn` over the ADT (REQ-6): `fold_<e>`
/// (`-> nat`, `decreases l`, applies the passed `spec_fn` step at each recursive
/// node), `for_all_<e>`/`exists_<e>`/`traverse_<e>` (`-> bool`), or `map_<e>`
/// (`-> E`, `Box::new`-reconstructing). GROUNDED forms (`fold_list`/`for_all_list`/
/// `map_list`, `decreases l`, `*tail`).
fn emit_scheme_spec_fn(u: &SchemeUse) -> Result<String, LowerError> {
    use thermite_spec::{SchemeResult, StepShape};
    let e = &u.enum_item;
    let elem = &u.elem_ty;
    let fname = u.scheme.generated_fn_name(&e.name);
    let (base, recursive) = enum_variant_split(e);
    let rec = recursive.ok_or_else(|| LowerError::Unsupported {
        what: format!(
            "ADT `{}` has no recursive variant for scheme `{}`",
            e.name, u.scheme.name
        ),
        span: zero_span(),
    })?;

    // The step `spec_fn` type + the seed/return type, by the scheme's result kind.
    let (step_ty, ret_ty, seed_param) = match (u.scheme.step_shape, u.scheme.result) {
        (StepShape::ElementAcc, SchemeResult::Accumulator) => (
            format!("spec_fn({elem}, nat) -> nat"),
            "nat".to_string(),
            Some(("init", "nat")),
        ),
        (StepShape::ElementAcc, SchemeResult::Bool) => (
            format!("spec_fn({elem}, bool) -> bool"),
            "bool".to_string(),
            Some(("init", "bool")),
        ),
        (StepShape::Element, SchemeResult::Bool) => {
            (format!("spec_fn({elem}) -> bool"), "bool".to_string(), None)
        }
        (StepShape::Element, SchemeResult::SameAdt) => {
            (format!("spec_fn({elem}) -> {elem}"), e.name.clone(), None)
        }
        // The remaining (shape, result) combinations are not in the frozen scheme
        // set (the registry pairs each shape with one result); unreachable for a
        // registered scheme, surfaced structurally rather than panicking.
        (shape, result) => {
            return Err(LowerError::Unsupported {
                what: format!(
                "scheme `{}` has an unmodeled (step-shape {shape:?}, result {result:?}) pairing",
                u.scheme.name
            ),
                span: zero_span(),
            })
        }
    };

    let mut out = String::new();
    write!(out, "spec fn {fname}(l: {}", e.name).map_err(|_| fmt_err())?;
    if let Some((sn, st)) = seed_param {
        write!(out, ", {sn}: {st}").map_err(|_| fmt_err())?;
    }
    let step_name = step_param_name(u.scheme);
    writeln!(out, ", {step_name}: {step_ty}) -> {ret_ty}").map_err(|_| fmt_err())?;
    out.push_str("    decreases l,\n{\n    match l {\n");

    // Base arm(s) + the recursive arm, per scheme.
    for b in &base {
        let value = scheme_base_value(u.scheme, b, seed_param.map(|(n, _)| n));
        writeln!(
            out,
            "        {}::{} => {value},",
            e.name,
            base_variant_pattern(b)
        )
        .map_err(|_| fmt_err())?;
    }
    let rec_arm = scheme_recursive_arm(
        u.scheme,
        &e.name,
        &rec.name,
        &fname,
        step_name,
        seed_param.map(|(n, _)| n),
    );
    writeln!(
        out,
        "        {}::{}(x, tail) => {rec_arm},",
        e.name, rec.name
    )
    .map_err(|_| fmt_err())?;
    out.push_str("    }\n}\n");
    Ok(out)
}

/// The step parameter name in the generated scheme `spec fn` (REQ-6): `f` for the
/// accumulator schemes (`fold`/`traverse`), `g` for `map`, `p` for the predicates
/// (`for_all`/`exists`) — matching the GROUNDED forms.
fn step_param_name(scheme: &thermite_spec::SchemeSig) -> &'static str {
    match scheme.name {
        "fold" | "traverse" => "f",
        "map" => "g",
        _ => "p",
    }
}

/// The base-arm value for a generated scheme `spec fn` (REQ-6): `fold` → the seed
/// `init`; `for_all`/`traverse` → `true`; `exists` → `false`; `map` → the empty
/// ADT (the unit base reconstructed, `E::Nil`).
fn scheme_base_value(
    scheme: &thermite_spec::SchemeSig,
    base: &VariantDef,
    seed: Option<&str>,
) -> String {
    match scheme.name {
        "fold" | "traverse" => seed.unwrap_or("init").to_string(),
        "exists" => "false".to_string(),
        "map" => base.name.clone(),
        // for_all (and any predicate base) is the identity `true`.
        _ => "true".to_string(),
    }
}

/// The recursive-arm body for a generated scheme `spec fn` (REQ-6), applying the
/// step at each `Cons`/`Node`. GROUNDED forms:
/// - `fold`: `f(x, fold_<e>(*tail, init, f))`
/// - `for_all`: `p(x) && for_all_<e>(*tail, p)`
/// - `exists`: `p(x) || exists_<e>(*tail, p)`
/// - `traverse`: `f(x, traverse_<e>(*tail, init, f))`
/// - `map`: `E::Cons(g(x), Box::new(map_<e>(*tail, g)))`
fn scheme_recursive_arm(
    scheme: &thermite_spec::SchemeSig,
    enum_name: &str,
    rec_variant: &str,
    fname: &str,
    step_name: &str,
    seed: Option<&str>,
) -> String {
    let seed = seed.unwrap_or("init");
    match scheme.name {
        "fold" | "traverse" => {
            format!("{step_name}(x, {fname}(*tail, {seed}, {step_name}))")
        }
        "for_all" => format!("{step_name}(x) && {fname}(*tail, {step_name})"),
        "exists" => format!("{step_name}(x) || {fname}(*tail, {step_name})"),
        "map" => format!(
            "{enum_name}::{rec_variant}({step_name}(x), Box::new({fname}(*tail, {step_name})))"
        ),
        // Unreachable for a registered scheme (the 5 are matched above); the
        // identity for safety.
        _ => format!("{fname}(*tail, {step_name})"),
    }
}

/// Emit the `fold_bound_<e>` induction-discharged-once law (REQ-7) — the
/// multiplier. A `proof fn` parametric in the step `f` + a per-node premise,
/// carrying the SINGLE `decreases l` structural induction, proving
/// `fold_<e>(l, init, f) <= <e>_len(l) * b` for `init == 0` and a per-node bound.
/// GROUNDED (`fold_bound_list`, single induction, `9 verified, 0 errors`; the
/// per-node-premise-removed negative control FAILS). Emitted ONLY for an ADT a
/// `fold` folds over.
fn emit_fold_bound_law(u: &SchemeUse) -> Result<String, LowerError> {
    let e = &u.enum_item;
    let elem = &u.elem_ty;
    let foldn = u.scheme.generated_fn_name(&e.name); // fold_<e>
    let lenn = format!("{}_len", e.name.to_ascii_lowercase());
    let lawn = format!("fold_bound_{}", e.name.to_ascii_lowercase());
    let (base, recursive) = enum_variant_split(e);
    let rec = recursive.ok_or_else(|| LowerError::Unsupported {
        what: format!("ADT `{}` has no recursive variant for the fold law", e.name),
        span: zero_span(),
    })?;

    let mut out = String::new();
    writeln!(
        out,
        "proof fn {lawn}(l: {}, init: nat, f: spec_fn({elem}, nat) -> nat, b: nat)",
        e.name
    )
    .map_err(|_| fmt_err())?;
    out.push_str("    requires\n        init == 0,\n");
    writeln!(
        out,
        "        forall|x: {elem}, acc: nat| #[trigger] f(x, acc) <= acc + b,"
    )
    .map_err(|_| fmt_err())?;
    writeln!(out, "    ensures").map_err(|_| fmt_err())?;
    writeln!(out, "        {foldn}(l, init, f) <= {lenn}(l) * b,").map_err(|_| fmt_err())?;
    out.push_str("    decreases l,\n{\n    match l {\n");
    for b in &base {
        writeln!(
            out,
            "        {}::{} => {{}}",
            e.name,
            base_variant_pattern(b)
        )
        .map_err(|_| fmt_err())?;
    }
    writeln!(out, "        {}::{}(x, tail) => {{", e.name, rec.name).map_err(|_| fmt_err())?;
    writeln!(out, "            {lawn}(*tail, init, f, b);").map_err(|_| fmt_err())?;
    writeln!(
        out,
        "            assert(({lenn}(*tail) + 1) * b == {lenn}(*tail) * b + b) by(nonlinear_arith);"
    )
    .map_err(|_| fmt_err())?;
    out.push_str("        }\n    }\n}\n");
    Ok(out)
}

// ---------------------------------------------------------------------------
// REQ-1: signature lowering.
// ---------------------------------------------------------------------------

/// Lower a `spec fn` (REQ-1/REQ-5). Slice params take `Seq<T>` (not `&[T]`); the
/// body lowers in spec context; `dec`→`decreases`. The return type uses the
/// `nat`-typed accumulator form when the body folds slice elements into a sum
/// (OQ-1: `u64`-valued `spec_sum` would re-introduce the overflow obligation).
fn lower_spec_fn(
    s: &SpecFnItem,
    variants: &[(&str, &str)],
    user_string_spec_fns: &[&str],
    program: &Program,
) -> Result<String, LowerError> {
    // Basis Stage 2 (`.design/basis/02-recursion-schemes.md` REQ-6): the
    // recursion-scheme bindings in scope for THIS spec fn — its scrutinee params
    // resolved to the generated `fold_<e>`/`for_all_<e>`. EMPTY for a non-scheme
    // fn (byte-stable for the existing corpus).
    let scheme_bindings = spec_fn_scheme_bindings(s, program)?;

    let mut out = String::new();
    // The return type: a scheme-CALL fold body returns the scheme's result kind
    // (`nat` for `fold`, `bool` for `for_all`/`exists`/`traverse`, the ADT for
    // `map`); else the existing head/ADT-fold-sum or declared-type lowering.
    let ret = lower_spec_fn_ret_with_schemes(&s.ret, &s.body, &scheme_bindings);
    write!(out, "spec fn {}(", s.name).ok();
    emit_params(&mut out, &s.params, Pos::Spec)?;

    // The DEC NUANCE (`.design/basis/02-recursion-schemes.md` step-lowering
    // resolution): a scheme-CALL instance body (`fold_list(l, 0, f)`) is
    // NON-recursive — the recursion lives in the GENERATED `fold_<e>`, which
    // carries its own `decreases l`. The surface instance still parses with a
    // mandatory `dec l`, but emitting `decreases l` on this non-recursive body is
    // spurious. We SUPPRESS it for a scheme-call body. (A hand-written recursive
    // `spec fn` — the `is_adt_fold_sum`/head-fold path — keeps its `decreases`.)
    if is_scheme_call_body(&s.body, &scheme_bindings) {
        writeln!(out, ") -> {ret}").ok();
    } else {
        write!(
            out,
            ") -> {ret}\n    decreases {}\n",
            spec_dec(&s.dec, &s.params)
        )
        .ok();
    }
    out.push_str(&lower_spec_fn_body_with_schemes(
        &s.body,
        &s.params,
        &ret,
        variants,
        user_string_spec_fns,
        &scheme_bindings,
    )?);
    Ok(out)
}

/// The recursion-scheme bindings in scope for a `spec fn` (REQ-6): for each
/// distinct scheme its body CALLS, the resolved generated fn name + element/result
/// types (from the scrutinee param's `enum` type). EMPTY for a non-scheme fn.
fn spec_fn_scheme_bindings(
    s: &SpecFnItem,
    program: &Program,
) -> Result<Vec<SchemeBinding>, LowerError> {
    let enums: std::collections::BTreeMap<&str, &EnumItem> = program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Enum(e) => Some((e.name.as_str(), e)),
            _ => None,
        })
        .collect();
    let mut uses: Vec<SchemeUse> = Vec::new();
    collect_scheme_uses_in_block(&s.body, &s.params, &enums, s.span, &mut uses)?;
    Ok(uses
        .into_iter()
        .map(|u| SchemeBinding {
            scheme_name: u.scheme.name,
            gen_name: u.scheme.generated_fn_name(&u.enum_name),
            elem_ty: u.elem_ty,
            result: u.scheme.result,
        })
        .collect())
}

/// True if the spec fn's body tail is a scheme CALL resolved by an in-scope
/// binding (REQ-6) — the instance whose `decreases` is suppressed (the recursion
/// lives in the generated `fold_<e>`).
fn is_scheme_call_body(body: &Block, bindings: &[SchemeBinding]) -> bool {
    scheme_call_result(body, bindings).is_some()
}

/// The result kind of a scheme-CALL body tail (REQ-6), or `None` if the body is
/// not a scheme call. Used to pick the instance's return type + suppress the
/// spurious `decreases`.
fn scheme_call_result(
    body: &Block,
    bindings: &[SchemeBinding],
) -> Option<thermite_spec::SchemeResult> {
    let tail = body.tail.as_ref()?;
    if let Expr::Call { callee, .. } = tail.as_ref() {
        if let Expr::Path(segs) = callee.as_ref() {
            if let Some(name) = segs.last() {
                return bindings
                    .iter()
                    .find(|b| b.scheme_name == name)
                    .map(|b| b.result);
            }
        }
    }
    None
}

/// The return type of a scheme-CALL instance spec fn (REQ-6): `nat` for `fold`,
/// `bool` for the predicate schemes, the ADT element-or-name for `map`. Falls
/// back to the existing `lower_spec_fn_ret` (head/ADT-fold-sum or declared type)
/// when the body is not a scheme call.
fn lower_spec_fn_ret_with_schemes(ret: &Type, body: &Block, bindings: &[SchemeBinding]) -> String {
    use thermite_spec::SchemeResult;
    match scheme_call_result(body, bindings) {
        Some(SchemeResult::Accumulator) => "nat".to_string(),
        Some(SchemeResult::Bool) => "bool".to_string(),
        Some(SchemeResult::SameAdt) => {
            // `map` returns the same ADT; the surface `ret` already names it.
            lower_type(ret).unwrap_or_else(|_| "bool".to_string())
        }
        None => lower_spec_fn_ret(ret, body),
    }
}

/// Lower a spec-fn body with the recursion-scheme bindings in scope (REQ-6) — a
/// scheme call in the body lowers to a call of the generated `fold_<e>`. Delegates
/// to the existing `lower_spec_fn_body` for the non-scheme paths (head-fold-sum,
/// ADT-fold-sum), threading the bindings through the spec context.
fn lower_spec_fn_body_with_schemes(
    body: &Block,
    params: &[Param],
    ret: &str,
    variants: &[(&str, &str)],
    user_string_spec_fns: &[&str],
    bindings: &[SchemeBinding],
) -> Result<String, LowerError> {
    if bindings.is_empty() {
        return lower_spec_fn_body(body, params, ret, variants, user_string_spec_fns);
    }
    // A scheme-call fn body lowers directly in spec context with the scheme
    // bindings (and variants) attached. The head/ADT-fold-sum shape predicates do
    // not match a scheme-call body (its tail is a `Call`, not a `Match`), so the
    // existing special-case lowering is bypassed — the scheme call is handled in
    // `lower_expr`'s `Call` arm via `lower_scheme_call`.
    // Thread the spec fn's `String`/`&String` params (Basis Stage 7 REQ-4) so a
    // String-scanning scheme-call body's `byte_at`/`slice` rewrites to the SPEC
    // accessors — the SAME `.with_strings` threading the non-scheme path below
    // uses, covering the whole spec-fn-body class (no scheme sibling left to
    // re-pin). EMPTY for a non-`String` spec fn (byte-stable).
    let strings = string_param_names(params);
    let ctx = Ctx::spec_seq()
        .with_variants(variants)
        .with_nat_ret(ret == "nat")
        .with_schemes(bindings)
        .with_strings(&strings)
        .with_user_string_spec_fns(user_string_spec_fns);
    let mut out = String::from("{\n");
    let b = lower_block_inner(body, ctx, 1, zero_span())?;
    out.push_str(&b);
    out.push_str("}\n");
    Ok(out)
}

/// The slice-typed parameter names of an item (the SHAPE-derived set whose bare
/// paths get `@` in spec position — REQ-5).
fn slice_param_names(params: &[Param]) -> Vec<&str> {
    params
        .iter()
        .filter_map(|p| match &p.ty {
            Type::Ref { inner, .. } if matches!(inner.as_ref(), Type::Slice(_)) => {
                Some(p.name.as_str())
            }
            _ => None,
        })
        .collect()
}

/// The `String`/`&String` parameter names of a `spec fn` (Basis Stage 7,
/// `.design/basis/07-strings.md` REQ-4): the SHAPE-derived set whose
/// spec-position `.len()`/`.byte_at(i)`/`.slice(..)` rewrite to the wrapper's
/// SPEC accessors (`.spec_len()`/`.spec_byte_at(i as int)`). A `spec fn` carries
/// only `Param`s (no synthetic `result` — a spec fn body is an expression, not a
/// `req`/`ens` contract), so this is the param-level analog of
/// [`string_value_names`] used to thread `.with_strings(..)` into the spec-fn
/// body context (`lower_spec_fn_body`/`lower_spec_fn_body_with_schemes`). Without
/// it a `&String`-param body's `byte_at(i)` does NOT rewrite to
/// `spec_byte_at(i as int)` and hits the `usize`-typed exec accessor (E0308 —
/// `usize` vs `int`). EMPTY for a non-`String` spec fn (byte-stable for the
/// existing corpus). Sees through a single `&` borrow via [`is_string_ty`].
fn string_param_names(params: &[Param]) -> Vec<&str> {
    params
        .iter()
        .filter(|p| is_string_ty(&p.ty))
        .map(|p| p.name.as_str())
        .collect()
}

/// The program-wide names of USER `spec fn`s that declare a `String`/`&String`
/// parameter (Basis Stage 7, `.design/basis/07-strings.md` REQ-4, issue #127). The
/// byte-view dispatch's SHAPE key: such a user spec fn (the #126 String-scanning
/// shape) lowers its param to `&TString`, so a `String` arg to it passes the
/// REFERENCE, NOT the `.data@` byte view — AND, because it lives in the user
/// namespace, it SHADOWS any GENERATED byte-view/parse def of the same name. The
/// generated emission gates (`program_uses_parse`/`program_uses_numfmt`/
/// `program_uses_string_search`) exclude these names so a user `spec fn is_digit(s:
/// &String, ..)` does not spuriously materialize the generated `is_digit(b: u8)`
/// (which would be a duplicate definition — E0428). Sorted+deduped for determinism
/// (R-CODE-5). EMPTY for a program with no String-param user spec fn (byte-stable).
fn user_string_spec_fn_names(program: &Program) -> Vec<&str> {
    let mut names: Vec<&str> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::SpecFn(s) if s.params.iter().any(|p| is_string_ty(&p.ty)) => {
                Some(s.name.as_str())
            }
            _ => None,
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// True iff `ty` is `String` or `&String` (sees through a single borrow) — the
/// SHAPE test shared by [`string_param_names`] and [`string_value_names`].
fn is_string_ty(ty: &Type) -> bool {
    match ty {
        Type::String => true,
        Type::Ref { inner, .. } => is_string_ty(inner),
        _ => false,
    }
}

/// The `String`-named values in scope for a fn's contract (REQ-4): every
/// `String`/`&String` parameter, plus the synthetic `result` when the return type
/// is `String`. A contract over a `String` names the wrapper's spec fns
/// (`.spec_len()`/`.spec_byte_at(i as int)`); this is the SHAPE-derived set the
/// `Ctx::is_string` rewrite keys on. A `&String` (the `str`-view role) is a
/// `String` value too — a read-only param's `.len()`/`.byte_at` are the same spec
/// fns. EMPTY for a non-`String` fn (byte-stable for the existing corpus).
fn string_value_names(f: &FnItem) -> Vec<&str> {
    let mut names: Vec<&str> = f
        .params
        .iter()
        .filter(|p| is_string_ty(&p.ty))
        .map(|p| p.name.as_str())
        .collect();
    if is_string_ty(&f.ret) {
        names.push("result");
    }
    names
}

/// The OWNED `String` (`Type::String`, NOT `&String`) parameter names of a fn
/// (Cluster C7, `.design/basis/09-option-result.md` REQ-5, #100): the exec body
/// borrows an owned `String` arg when it calls the generated `parse_u64` (which
/// takes `&TString`). A `&String` param is already a borrow and is EXCLUDED — it
/// passes through unchanged. EMPTY for a fn with no owned `String` param
/// (byte-stable for the existing corpus).
fn owned_string_value_names(f: &FnItem) -> Vec<&str> {
    f.params
        .iter()
        .filter(|p| matches!(p.ty, Type::String))
        .map(|p| p.name.as_str())
        .collect()
}

/// Lower a `fn` (REQ-1). `-> (result: RET)` binder so `ens` can mention
/// `result`; `req`→`requires`, each `ens`→`ensures`, `fx pure`→nothing.
///
/// THE BOUNDARY/SLAG COMPOSITION ARM (`.design/lower/boundary-composition.md`
/// REQ-1, §9/§8): when `f.boundary.is_some() || f.slag.is_some()` the fn is a
/// DECLARED trust boundary — a `#[boundary]` fn has a FOREIGN body (`body: None`)
/// and a `#[slag]` fn a fiat-trusted body, both body-UNPROVEN by §8/§9. As a
/// woven dependency of a caller's sub-program it is emitted as a
/// `#[verifier::external_body]` SIGNATURE — its `requires`/`ensures` lowered
/// exactly as a regular fn's (no weakening), with the body SUPPRESSED to a
/// synthetic `{ unimplemented!() }` verus never checks — so the caller's proof
/// resolves the callee and discharges against its ASSUMED `ensures`. The
/// exemption is gated STRICTLY on the syntactic `#[boundary]`/`#[slag]` flag
/// (the honesty gate, `goal.md` R-DEFER-9): a REGULAR fn (neither flag) ALWAYS
/// takes the fully-proved-body path below — a lying regular body is CAUGHT.
/// True iff the fn's effect row contains `diverge` (§4.1: "divergence requires
/// `fx diverge` in the row"). Keyed on the SHAPE of the effect row — a `pure`
/// row never diverges; a `Set` row diverges iff it lists [`Effect::Diverge`].
/// This is the SINGLE source of truth for the §4.1 termination exemption (the
/// fn attribute in [`lower_fn`] and the loop-`decreases` suppression in
/// [`lower_loop`] both gate on it), so the exemption is applied uniformly and
/// ONLY to a diverge fn (a non-diverge loop still proves termination).
fn fn_is_diverge(f: &FnItem) -> bool {
    use thermite_syntax::ast::{Effect, EffectRow};
    matches!(&f.contract.fx, EffectRow::Set(es) if es.contains(&Effect::Diverge))
}

fn lower_fn(
    f: &FnItem,
    nat_fns: &[&str],
    inv_structs: &[&str],
    string_fields: &[&str],
    variants: &[(&str, &str)],
    user_string_spec_fns: &[&str],
) -> Result<String, LowerError> {
    // THE HONESTY GATE: external_body iff a declared trust boundary
    // (`#[boundary]`/`#[slag]`), NEVER a regular fn. Emitted only into a CALLER's
    // sub-program as a woven dependency (forge's `item_subprogram`). The 2-bool
    // decision is DELEGATED to the Verus-verified `should_emit_external_body`
    // (epic #60, REQ-9 / Target C, mechanism (c)): its `ensures` proves the
    // disjunction AND the §9 soundness corollary `(!boundary && !slag) ==> !r` —
    // a regular fn is NEVER laundered into an assumed-L3 external_body signature
    // (`goal.md` R-DEFER-9). `boundary_gate_verified.rs` anchors this OBSERVABLE
    // dispatch (the emitted `#[verifier::external_body]` substring) to the proof.
    if thermite_verified::should_emit_external_body(f.boundary.is_some(), f.slag.is_some()) {
        return lower_external_body_fn(
            f,
            nat_fns,
            inv_structs,
            string_fields,
            user_string_spec_fns,
        );
    }

    let mut out = String::new();
    // §4.1: "Termination is proved by default; divergence requires `fx diverge`."
    // A `fx diverge` fn (an event loop, `examples/editor/editor.th`'s `run`) is
    // honestly NON-terminating: its loop's `decreases` is suppressed below
    // (`lower_loop`), and Verus would then DEMAND a termination proof for the
    // bare loop unless the fn carries this exemption. The attribute scopes the
    // exemption to THIS fn (it does not weaken termination for any other fn): a
    // diverge fn proves PARTIAL correctness (the loop INVARIANTS) only, which is
    // the honest L1 verdict — termination is not claimed. A non-diverge fn never
    // emits this, so the termination default stands unweakened (gap-3 is
    // diverge-ONLY; a normal loop without `dec` still fails to verify).
    if fn_is_diverge(f) {
        out.push_str("#[verifier::exec_allows_no_decreases_clause]\n");
    }
    out.push_str(&lower_fn_signature(
        f,
        nat_fns,
        inv_structs,
        string_fields,
        user_string_spec_fns,
    )?);
    // `fx pure` emits no annotation (Verus `fn` is pure by default; §4.1).

    // C9-A (`.design/basis/10-recursion-tuples.md` REQ-3): a RECURSIVE exec `fn`
    // carries an optional `dec <measure>` clause; emit the Verus `decreases
    // <measure>` AFTER the signature's `requires`/`ensures` block and BEFORE the
    // body `{` — the SAME position + the SAME `spec_dec` measure-lowering helper
    // the recursive `spec fn` uses (`lower_spec_fn`). Verus discharges termination
    // of the self-recursion from this measure: a non-decreasing measure → L0
    // ("could not prove termination", REQ-4); the self-call itself lowers as an
    // ordinary `Expr::Call` in the body (no special node). A NON-recursive fn has
    // `dec = None` and emits NO `decreases` — byte-stable for the entire existing
    // corpus (AC-7; `sum`/`binary_search` lower unchanged). The `fx diverge`
    // exemption above (`#[verifier::exec_allows_no_decreases_clause]`, #88) lets a
    // diverge fn recurse WITHOUT a `dec` (L1-capped, partial correctness only);
    // such a fn carries `dec = None`, so this block correctly emits nothing.
    if let Some(dec) = &f.dec {
        let measure = spec_dec(dec, &f.params);
        writeln!(out, "    decreases {measure}").ok();
    }

    // Body, with shape-derived proof aids threaded through the loop lowering. The
    // variant map flows into the exec body so an enum `match` (e.g. `is_circle`'s)
    // lowers to ENUM-QUALIFIED arms (REQ-9).
    // NOTE: the exec fn BODY is not threaded the user-String-spec-fn set: the
    // byte-view dispatch (`lower_spec_arg`) only fires in SPEC position (`ctx.is_spec()`),
    // and an exec body lowers in `Ctx::exec()` — a recursive self-call's `String` arg
    // passes the reference verbatim regardless (no `.data@` view in exec). Only the
    // exec SIGNATURE's `requires`/`ensures` (lowered in spec context above) and a
    // `spec fn` body (`lower_spec_fn`) reach the byte-view dispatch (#127).
    let body = lower_fn_body(f, nat_fns, string_fields, variants)?;
    out.push_str(&body);
    Ok(out)
}

/// Emit a `fn`'s signature up to and including its `requires`/`ensures` block
/// (everything before the body): `fn name(<params>) -> (result: RET)` then
/// `requires <req>,` (omitted when literal-`true`) and each `ens` in source
/// order. Shared by the regular fully-proved arm ([`lower_fn`]) and the
/// boundary/slag external_body arm ([`lower_external_body_fn`]) so the contract
/// lowering is IDENTICAL across both (REQ-1 — the assumed signature carries the
/// exact unweakened contract).
fn lower_fn_signature(
    f: &FnItem,
    nat_fns: &[&str],
    inv_structs: &[&str],
    string_fields: &[&str],
    user_string_spec_fns: &[&str],
) -> Result<String, LowerError> {
    let mut out = String::new();
    let ret = lower_type(&f.ret)?;
    write!(out, "fn {}(", f.name).ok();
    emit_params(&mut out, &f.params, Pos::Exec)?;
    writeln!(out, ") -> (result: {ret})").ok();

    let slices = slice_param_names(&f.params);
    // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): the `String`-named
    // values in scope for this fn's contract — every `String`/`&String` param plus
    // `result` when the return is `String`. A `String` receiver's spec-position
    // `.len()`/`.byte_at(i)` rewrites to `.spec_len()`/`.spec_byte_at(i as int)`.
    let strings = string_value_names(f);
    let spec = Ctx::spec(&slices, nat_fns)
        .with_strings(&strings)
        .with_string_fields(string_fields)
        .with_user_string_spec_fns(user_string_spec_fns);

    // requires: the single `req` clause (REQ-1), plus the woven `well_formed()`
    // conjunct for every parameter whose type is an invariant-bearing `struct`
    // (REQ-8, OQ-3 automatic threading) so Verus has the type-invariant of each
    // incoming value in scope. The author writes neither conjunct: the invariant
    // is a property of the TYPE, implicit at every use.
    // Cluster C5 (`.design/basis/07-strings.md` REQ-13..16, #102): does THIS fn use a
    // string search/transform op (a C5 method call or a C5 spec-fn in its contract)?
    // If so its `String` params need the woven `well_formed()` precondition (below).
    let fn_uses_string_search = contract_uses_string_search(&f.contract, user_string_spec_fns)
        || f.body
            .as_ref()
            .map(|b| block_uses_string_search(b, user_string_spec_fns))
            .unwrap_or(false);
    let mut woven_reqs: Vec<String> = Vec::new();
    for p in &f.params {
        // The invariant-bearing-struct `well_formed()` weave sees through a single
        // `&` borrow (blocker #105): `b: Buffer` and `b: &Buffer` both weave
        // `b.well_formed()`, so a borrowed receiver carries its type invariant
        // (`cursor <= text.len()`) into scope — without it `b.cursor + 1` cannot be
        // proved non-overflowing. (Field access auto-derefs in both Rust and Verus.)
        if let Some(name) = named_struct_param(&p.ty) {
            if inv_structs.contains(&name) {
                woven_reqs.push(format!("{}.well_formed()", p.name));
            }
        }
        // Cluster C5 (`.design/basis/07-strings.md` REQ-13..16, issue #102): a
        // `String`/`&String` param is BOUNDED by its type invariant `well_formed()`
        // (`data.len() <= CAP`, the §4.2 cage), exactly like an invariant-bearing
        // struct. The C5 search/transform methods REQUIRE `self.well_formed()` /
        // `p.well_formed()`, so weave the `well_formed()` conjunct for every
        // `String`-typed param (a bare `String` or a `&String` borrow — `is_string_ty`
        // sees through the `Ref`) so a caller of `s.starts_with(p)` / `s.split(sep)`
        // discharges the method's precondition. The author writes neither conjunct —
        // the invariant is implicit at every use, the SAME automatic threading the
        // inv-bearing struct gets. Woven ONLY when the program uses a C5 op (no golden
        // churn for the pre-C5 string corpus — `string_demo`'s `join`/`first_byte`
        // discharge `well_formed` from their own `req`, so they need no weave). Keyed
        // on the param TYPE reaching `String` directly (not a `Vec<String>`/struct
        // field — those carry their own invariant), deduped against the struct weave.
        if fn_uses_string_search && is_string_param_ty(&p.ty) {
            let conj = format!("{}.well_formed()", p.name);
            if !woven_reqs.contains(&conj) {
                woven_reqs.push(conj);
            }
        }
        // Cluster C12 (`.design/basis/13-map.md` REQ-4): a `Map<K, V>`-typed param
        // (bare or `&Map`) is BOUNDED by its type invariant `well_formed()` (the
        // capacity + key-uniqueness invariant, the §4.2 cage) — the SAME automatic
        // threading an invariant-bearing struct / a `String` param gets. The `TMap`
        // wrapper's `contains_key`/`get`/`insert` all `require self.well_formed()`,
        // so weave the `well_formed()` conjunct for every `Map`-typed param so a
        // caller of `m.contains_key(k)` / `m.get(k)` / `m.insert(k, v)` discharges
        // the method's precondition. The author writes no conjunct — the invariant
        // is implicit at every use. Deduped against the struct/String weaves.
        if is_map_param_ty(&p.ty) {
            let conj = format!("{}.well_formed()", p.name);
            if !woven_reqs.contains(&conj) {
                woven_reqs.push(conj);
            }
        }
    }
    let req = lower_expr(&f.contract.req.expr, spec, 0, f.span)?;
    if woven_reqs.is_empty() {
        // No woven invariant conjunct: keep the existing single-line
        // `requires <req>,` form byte-for-byte (no golden churn for the non-ADT
        // corpus — `sum`/`binary_search` lower UNCHANGED). Omit a literal-`true`.
        if req != "true" {
            writeln!(out, "    requires {req},").ok();
        }
    } else {
        // An invariant-bearing struct param weaves its `well_formed()` conjunct;
        // emit the multi-line `requires` block (the woven conjuncts first, then
        // the author's `req` unless it is literal-`true`).
        out.push_str("    requires\n");
        for r in &woven_reqs {
            writeln!(out, "        {r},").ok();
        }
        if req != "true" {
            writeln!(out, "        {req},").ok();
        }
    }

    // ensures: the woven `result.well_formed()` conjunct FIRST when the return
    // type is an invariant-bearing struct (REQ-8 — Verus proves the constructed
    // return value satisfies the invariant), then every `ens` clause in source
    // order (no weakening — R-DEFER-9).
    out.push_str("    ensures\n");
    if let Type::Named(name) = &f.ret {
        if inv_structs.contains(&name.as_str()) {
            out.push_str("        result.well_formed(),\n");
        }
    }
    for ens in &f.contract.ens {
        let e = lower_expr(&ens.expr, spec, 0, f.span)?;
        writeln!(out, "        {e},").ok();
    }
    Ok(out)
}

/// Lower ONE contract-position [`Expr`] to its PRODUCTION Verus predicate
/// (`.design/verified/contract-tv.md` REQ-5 prerequisite; epic crosslink #139 /
/// blocker #144). This is `P_production` for a single contract clause — the
/// artifact-under-test the contract-faithfulness translation-validation (TV)
/// engine compares against the INDEPENDENT reference encoding
/// `thermite_tv::ref_encode::ref_contract_pred`.
///
/// It REUSES the EXACT spec-context [`lower_expr`] path `lower_fn_signature` uses
/// for a fn's `requires`/`ensures` clauses (and the loop `inv`/`dec` lowering
/// uses), threaded with the SAME spec context inputs: `slices` (the `&[T]` param
/// names whose use sites take the `@`-view), `nat_fns` (the `nat`-returning spec
/// fn names that drive the `as nat` coercion of a compared scalar), `strings`
/// (the `String`/`&String` value names whose `.len()`/`.byte_at(i)` rewrite to
/// the wrapper spec fns), `string_fields` (the `String`-typed FIELD names), and
/// `user_string_spec_fns` (the #127 byte-view-dispatch shape key). Passing the
/// SAME context the signature path computes (`slice_param_names`/
/// `string_value_names`/the program-wide field + user-spec-fn sets) makes
/// `lower_contract_expr(clause.expr, …)` produce a clause's predicate
/// BYTE-IDENTICAL to the line `lower_fn_signature` emits for it — so the forge TV
/// phase checks the REAL production lowering, not a re-derivation. (The forge
/// phase binds slice params directly as `Seq<T>` in the obligation and so passes
/// an EMPTY `slices`, matching the reference encoder's seq-bound identity `@`-view
/// — the per-clause obligation's coercion-matching contract, contract-tv.md
/// Architecture.)
///
/// This is a thin per-clause re-entry into the lowerer (NOT a new lowering rule);
/// `forge::contract_tv` is the non-test consumer (R-DEFER-1). Returns a
/// [`LowerError`] (never a panic — R-CODE-2) for a construct the spec-context
/// lowering does not cover.
#[allow(
    clippy::too_many_arguments,
    reason = "the spec-context inputs mirror lower_fn_signature's threaded ctx \
        (slices/nat_fns/strings/string_fields/user_string_spec_fns) — a struct \
        would obscure the 1-to-1 correspondence with the signature path this \
        re-enters"
)]
pub fn lower_contract_expr(
    expr: &Expr,
    slices: &[&str],
    nat_fns: &[&str],
    strings: &[&str],
    string_fields: &[&str],
    user_string_spec_fns: &[&str],
) -> Result<String, LowerError> {
    let ctx = Ctx::spec(slices, nat_fns)
        .with_strings(strings)
        .with_string_fields(string_fields)
        .with_user_string_spec_fns(user_string_spec_fns);
    lower_expr(expr, ctx, 0, zero_span())
}

/// Lower ONE body-position (EXEC) expression to its PRODUCTION Verus EXEC
/// expression text (`.design/verified/exec-tv.md` REQ-2 prerequisite, epic
/// crosslink #151, blocker #152). This is the EXEC dual of [`lower_contract_expr`]:
/// where the contract entry re-enters `lower_expr` in SPEC context (`Ctx::spec`,
/// where slices/casts/indexing rewrite to `@`/`nat`/`int`), this re-enters in EXEC
/// context (`Ctx::exec()`, `Pos::Exec`), where arithmetic stays BOUNDED `u64`/
/// `usize` with the always-active runtime overflow checks, an index `xs[i]` lowers
/// to the bounded Rust access `xs[i]` (not the spec `xs@[i as int]`), and a cast
/// `(n - 1) as u8` carries the #122 inner-paren + the #146 cast-`<` outer-paren
/// discipline (`lower_binary_operand`/`is_lt_leading`). It is the `P_production`
/// the exec-TV obligation wraps as `fn tv_exec_wrap(..) ensures result == <ref> {
/// <this> }`.
///
/// THE EXEC `Ctx` IS REACHABLE FOR A STANDALONE EXPR (the #1 feasibility unknown
/// the design flagged): `Ctx::exec()` is a `Ctx<'static>` constructed with NO
/// surrounding-fn context (empty `slices`/`nat_fns`/`variants`/`schemes`/…) — a
/// pure exec EXPRESSION (`thermite-design.md` §4.1 arithmetic/cast/comparison/call/
/// index subset) lowers with NO surrounding-fn frame (no `let`/loop/mutation, those
/// are step 2.2). The body params the expr reads are bound by the OBLIGATION's
/// signature (`thermite_tv::ExecObligationFrame`), not by this lowering. So a
/// standalone exec expr lowers verbatim through the SAME `lower_expr` exec path the
/// body uses (REUSE, not a re-derivation — `goal.md` R-CHAR-3).
///
/// This is a thin per-expr re-entry into the lowerer (NOT a new lowering rule);
/// `forge::exec_tv` (the #156 next dispatch) is the eventual non-test consumer.
/// Returns a [`LowerError`] (never a panic — R-CODE-2) for a construct the EXEC
/// lowering does not cover.
pub fn lower_exec_expr(expr: &Expr) -> Result<String, LowerError> {
    lower_expr(expr, Ctx::exec(), 0, zero_span())
}

/// Lower a STRAIGHT-LINE exec BODY (a [`Block`]) to its PRODUCTION Verus EXEC body
/// text (`.design/verified/exec-stmt-tv.md` REQ-3, blocker #161; epic crosslink
/// #158). This is the per-BODY analogue of [`lower_exec_expr`]: where the per-expr
/// entry re-enters `lower_expr` in EXEC context for a single value, this re-enters
/// the EXISTING [`lower_block_inner`] / [`lower_stmt`] EXEC path for a whole
/// straight-line block (its `let`/`mut`-let / assignment / `if`-statement /
/// `Stmt::Expr` / tail), threading the SAME `Ctx::exec()` frame. It is the
/// `P_production` the body-TV obligation (`thermite_tv::body_equivalence_obligation`)
/// wraps as `fn tv_body_wrap(..) ensures result == <body_ref_state(source)> {
/// <this> }`.
///
/// THE BODY EXEC `Ctx` IS REACHABLE FOR A STANDALONE BLOCK (the #161 feasibility
/// the design flagged — `lower_block_inner` is private + fn-context-bound).
/// RESOLVED exactly as step 2.1 resolved `lower_exec_expr`: the body's FREE VARS
/// (the fn params) are bound by the OBLIGATION's signature
/// (`thermite_tv::BodyObligationFrame`), NOT by this lowering, so a straight-line
/// body needs NO surrounding-fn aids (no `slices`/`nat_fns`/`variants`/`schemes`/…
/// — those drive contract/spec rewrites and loop proof-aids, none of which a
/// straight-line EXEC body uses). The minimal `Ctx::exec()` frame (a `Ctx<'static>`
/// with every aid empty) is therefore the correct per-body entry; the block lowers
/// VERBATIM through the SAME `lower_block_inner` exec path the fn body uses (REUSE,
/// not a re-derivation — `goal.md` R-CHAR-3). The result is the lowered statement
/// sequence followed by the tail expression (the body's final-state projection),
/// one statement per line, matching production's fn-body emission.
///
/// FROZEN-SUBSET HONESTY (`.design/verified/exec-stmt-tv.md` REQ-1): a body
/// containing a `Stmt::Loop` (step 2.2.2, OUT of 2.2.1) is NOT silently lowered —
/// `lower_stmt`'s `Stmt::Loop` arm returns [`LowerError::Unsupported`] (a standalone
/// loop needs the fn-aid loop proof-aid context — `lower_loop`), so an
/// out-of-frozen-subset body is an honest `Err` here, never a wrong lowering. The
/// in-frozen-subset constructs (`Let`/`Assign`/`If`/`Expr`/tail-`Return`) lower
/// through `lower_stmt`/`lower_block_inner` exactly as in a fn body.
///
/// Returns a [`LowerError`] (never a panic — R-CODE-2) for a construct the EXEC body
/// lowering does not cover.
pub fn lower_exec_body(block: &Block) -> Result<String, LowerError> {
    lower_block_inner(block, Ctx::exec(), 0, zero_span())
}

/// Lower a `#[boundary]`/`#[slag]` fn as a `#[verifier::external_body]` assumable
/// SIGNATURE (`.design/lower/boundary-composition.md` REQ-1, §9/§8). The verus
/// `#[verifier::external_body]` attribute makes the body OPAQUE: verus assumes
/// the `requires`/`ensures` at every call site and NEVER checks the body
/// (grounded harness (1): a caller proves L3 through the assumed `ensures`). The
/// signature + contract are lowered by the SAME [`lower_fn_signature`] a regular
/// fn uses (no weakening — REQ-1), and the body is a synthetic
/// `{ unimplemented!() }` verus never examines (the foreign/fiat body the caller
/// trusts by declaration; §8/§9).
///
/// This is THE HONEST modeling of a foreign function, not a proof cheat
/// (`goal.md` R-DEFER-9): it is emitted ONLY for a fn ALREADY classified
/// `#[boundary]`/`#[slag]` (the §16/§8 `gate_fn` L1 path) and woven into a
/// CALLER's sub-program — the caller still proves its OWN body and discharges
/// the callee's `req` at the call site (harnesses (2)/(3)). The
/// `#[verifier::external_body]` lives in the lowered verus STRING (a generated
/// artifact describing a foreign function), NEVER in the toolchain's own `.rs`
/// source — categorically distinct from the gate-forbidden `#[verifier::external]`
/// proof-dodge of code we wrote (the doc's emitted-verus vs our-Rust distinction).
fn lower_external_body_fn(
    f: &FnItem,
    nat_fns: &[&str],
    inv_structs: &[&str],
    string_fields: &[&str],
    user_string_spec_fns: &[&str],
) -> Result<String, LowerError> {
    let mut out = String::new();
    out.push_str("#[verifier::external_body]\n");
    out.push_str(&lower_fn_signature(
        f,
        nat_fns,
        inv_structs,
        string_fields,
        user_string_spec_fns,
    )?);
    // The body is SUPPRESSED: verus does not check an external_body body, so the
    // synthetic `{ unimplemented!() }` stands in for the foreign/fiat body the
    // caller trusts by declaration (§8/§9). The real `f.body` (None for a
    // boundary fn, a fiat body for slag) is NEVER lowered here — re-lowering a
    // slag body would re-introduce the obligation §8 exempts (OQ-2).
    out.push_str("{\n    unimplemented!()\n}\n");
    Ok(out)
}

// ---------------------------------------------------------------------------
// The equivalent-mutant equivalence-obligation seam
// (`.design/forge/equivalent-mutants.md` REQ-1, crosslink #101).
// ---------------------------------------------------------------------------

/// Lower the §7 equivalent-mutant EQUIVALENCE OBLIGATION for one survivor
/// (`.design/forge/equivalent-mutants.md` REQ-1): given a fn `f` (its `req`,
/// params, return type, and REAL body `f.body`) and a surviving mutant's body
/// `mutant_body`, emit a complete Verus source file that asks Verus to PROVE
/// that, under `f`'s `req`, the mutant body's observable result equals the real
/// body's result FOR ALL inputs. A `verus` run that VERIFIES (`0 errors`) is a
/// PROOF of observable equivalence (the survivor is a true equivalent mutant →
/// dropped from the kill-ratio denominator, REQ-2); a counterexample
/// (`postcondition not satisfied`) means a distinguishing input exists (the
/// survivor STAYS counted, REQ-3).
///
/// The emitted shape is the GROUNDED form pinned in the design (verus
/// `2 verified, 0 errors` for the equivalent case, `0 verified, 1 errors` for the
/// distinguishing case):
///
/// ```verus
/// spec fn equiv_real_<name>(x: u64) -> u64 { let y: u64 = (x + 0) as u64; y }
/// spec fn equiv_mut_<name>(x: u64)  -> u64 { 0 }
/// proof fn equiv_check_<name>(x: u64)
///     requires x == 0,
///     ensures equiv_mut_<name>(x) == equiv_real_<name>(x),
/// {}
/// ```
///
/// REUSE, NOT a hand-emitted Verus duplicate (`goal.md` R-CHAR-3): each body is
/// rendered through the SAME `lower_expr` the L3 path uses, in a scalar spec
/// context, with the EXEC arithmetic coercion (`(expr) as <ret>`) the design
/// grounded — a naive spec rendering of `x + 0` over a `u64` return fails
/// `verus` with `expected u64, found int`, exactly the seam-need the prior
/// builder grounded. The `req` is lowered by `lower_expr` in the same spec
/// context (the obligation's `requires`).
///
/// SCOPE (OQ-1, sound-but-incomplete): only SCALAR (`u32`/`u64`/`usize`/`bool`)
/// params and return are rendered — observable equality is value equality. A
/// non-scalar param/return, a non-pure body, or a body whose statements are not
/// the simple let-chain-plus-tail / leading-early-return shape returns
/// `LowerError::Unsupported`; the caller treats an un-renderable obligation as NO
/// proof, so the survivor STAYS counted (the natural conservative fallback —
/// never a laundered exclusion, R-DEFER-9).
pub fn lower_equivalence_obligation(f: &FnItem, mutant_body: &Block) -> Result<String, LowerError> {
    let real_body = f.body.as_ref().ok_or_else(|| LowerError::Unsupported {
        what: "equivalence obligation reached a bodyless (boundary) fn; a boundary \
               fn is never mutation-scored (equivalent-mutants.md OQ-2)"
            .to_string(),
        span: f.span,
    })?;

    // SCOPE gate (OQ-1): every param + the return must be a scalar primitive so
    // observable equality is value equality. Any other shape → Unsupported → the
    // survivor stays counted (sound-but-incomplete).
    let ret_spelling = scalar_obligation_type(&f.ret).ok_or_else(|| LowerError::Unsupported {
        what: format!(
            "equivalence obligation supports only scalar (u32/u64/usize/bool) \
             returns; `{}` returns a non-scalar type (equivalent-mutants.md OQ-1)",
            f.name
        ),
        span: f.span,
    })?;
    for p in &f.params {
        if scalar_obligation_type(&p.ty).is_none() {
            return Err(LowerError::Unsupported {
                what: format!(
                    "equivalence obligation supports only scalar params; `{}`'s \
                     param `{}` is non-scalar (equivalent-mutants.md OQ-1)",
                    f.name, p.name
                ),
                span: f.span,
            });
        }
    }

    // The scalar spec context: no slice-view names (every param is a scalar), no
    // nat-fns. The SAME context the L3 spec path uses for a scalar predicate.
    let ctx = Ctx::spec(NO_SLICES, NO_SLICES);

    let params = obligation_param_list(&f.params)?;
    let real_value = render_body_as_spec_value(real_body, &ret_spelling, ctx, f.span)?;
    let mut_value = render_body_as_spec_value(mutant_body, &ret_spelling, ctx, f.span)?;
    let req = lower_expr(&f.contract.req.expr, ctx, 0, f.span)?;

    let name = &f.name;
    let mut out = String::new();
    out.push_str("use vstd::prelude::*;\n");
    out.push_str("verus! {\n");
    writeln!(
        out,
        "spec fn equiv_real_{name}({params}) -> {ret_spelling} {{ {real_value} }}"
    )
    .map_err(|_| fmt_err())?;
    writeln!(
        out,
        "spec fn equiv_mut_{name}({params}) -> {ret_spelling} {{ {mut_value} }}"
    )
    .map_err(|_| fmt_err())?;
    writeln!(out, "proof fn equiv_check_{name}({params})").map_err(|_| fmt_err())?;
    // Omit a literal-`true` precondition (the obligation holds for ALL inputs).
    if req != "true" {
        writeln!(out, "    requires {req},").map_err(|_| fmt_err())?;
    }
    let arg_names = obligation_arg_names(&f.params);
    writeln!(
        out,
        "    ensures equiv_mut_{name}({arg_names}) == equiv_real_{name}({arg_names}),"
    )
    .map_err(|_| fmt_err())?;
    out.push_str("{}\n");
    out.push_str("}\n");
    out.push_str("fn main() {}\n");
    Ok(out)
}

/// The Verus spelling of a SCALAR primitive type, or `None` for any non-scalar
/// type (the equivalence-obligation scope gate, OQ-1). `bool` is included: a
/// `bool`-returning forced-output fn's observable result is its boolean value.
fn scalar_obligation_type(ty: &Type) -> Option<String> {
    match ty {
        Type::Prim(PrimType::U32) => Some("u32".to_string()),
        Type::Prim(PrimType::U64) => Some("u64".to_string()),
        Type::Prim(PrimType::Usize) => Some("usize".to_string()),
        Type::Prim(PrimType::Bool) => Some("bool".to_string()),
        _ => None,
    }
}

/// `true` iff the obligation arithmetic-coerces to `ty` — an integer scalar gets
/// the `(expr) as <ty>` coercion the design grounded; a `bool` result is left
/// bare (Verus has no `as bool`).
fn obligation_coerces(ty: &str) -> bool {
    matches!(ty, "u32" | "u64" | "usize")
}

/// The `name: <ty>` parameter list for an equivalence-obligation spec/proof fn —
/// the SCALAR param types verbatim (the gate already rejected non-scalars).
fn obligation_param_list(params: &[Param]) -> Result<String, LowerError> {
    let mut parts = Vec::with_capacity(params.len());
    for p in params {
        let ty = scalar_obligation_type(&p.ty).ok_or_else(|| LowerError::Unsupported {
            what: format!("equivalence obligation param `{}` is non-scalar", p.name),
            span: zero_span(),
        })?;
        parts.push(format!("{}: {ty}", p.name));
    }
    Ok(parts.join(", "))
}

/// The comma-separated argument names for the two spec-fn calls in the proof
/// `ensures` (`equiv_mut_f(x, y) == equiv_real_f(x, y)`).
fn obligation_arg_names(params: &[Param]) -> String {
    params
        .iter()
        .map(|p| p.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Render an exec body as the spec-fn body of an equivalence-obligation
/// (REQ-1). The observable result of:
///
/// - a body whose FIRST statement is `return <e>;` (the early-return mutant) is
///   `<e>` (coerced) — the rest of the body is dead;
/// - a body of `let`s ending in a tail is the let-chain (each init coerced to
///   its declared/return type) plus the coerced tail (the `{ let y = x+0; y }`
///   forced-output shape).
///
/// Any other statement shape (an `Assign`, a nested `Loop`/`If`, a `Stmt::Expr`)
/// → `Unsupported` (out of the scalar forced-output scope, OQ-1) so the survivor
/// stays counted.
fn render_body_as_spec_value(
    body: &Block,
    ret: &str,
    ctx: Ctx,
    span: Span,
) -> Result<String, LowerError> {
    // The early-return mutant: a leading `return <e>;` pins the observable result
    // to `<e>` regardless of the dead tail (`mutation::early_return_value`).
    if let Some(Stmt::Return(ret_expr)) = body.stmts.first() {
        let e = ret_expr.as_ref().ok_or_else(|| LowerError::Unsupported {
            what: "equivalence obligation: a value-less `return;` has no observable \
                   result to compare"
                .to_string(),
            span,
        })?;
        let lowered = lower_expr(e, ctx, 0, span)?;
        return Ok(coerce_obligation_expr(&lowered, ret));
    }

    // A let-chain plus tail: render each `let` (init coerced to its declared type,
    // else the return type) and the coerced tail.
    let mut out = String::new();
    for stmt in &body.stmts {
        match stmt {
            Stmt::Let {
                mutable: false,
                name,
                ty,
                init,
            } => {
                let decl = match ty {
                    Some(t) => {
                        scalar_obligation_type(t).ok_or_else(|| LowerError::Unsupported {
                            what: format!("equivalence obligation `let {name}` is non-scalar"),
                            span,
                        })?
                    }
                    None => ret.to_string(),
                };
                let init_lowered = lower_expr(init, ctx, 0, span)?;
                let init_coerced = coerce_obligation_expr(&init_lowered, &decl);
                write!(out, "let {name}: {decl} = {init_coerced}; ").map_err(|_| fmt_err())?;
            }
            other => {
                return Err(LowerError::Unsupported {
                    what: format!(
                        "equivalence obligation supports only a leading early-return \
                         or an immutable let-chain-plus-tail body; found {}",
                        stmt_kind(other)
                    ),
                    span,
                });
            }
        }
    }
    let tail = body.tail.as_ref().ok_or_else(|| LowerError::Unsupported {
        what: "equivalence obligation body has no tail value to compare".to_string(),
        span,
    })?;
    let tail_lowered = lower_expr(tail, ctx, 0, span)?;
    out.push_str(&coerce_obligation_expr(&tail_lowered, ret));
    Ok(out)
}

/// Apply the EXEC arithmetic coercion the design grounded: an integer-result
/// expression is wrapped `(expr) as <ty>` so the spec-position arithmetic (which
/// Verus types as unbounded `int`) matches the bounded scalar return — without it
/// `verus` rejects `x + 0` as `expected u64, found int`. A `bool` result is left
/// bare (no `as bool`). Already-`as`-suffixed and bare-literal forms still take
/// the wrap (it is idempotent for verification).
fn coerce_obligation_expr(expr: &str, ret: &str) -> String {
    if obligation_coerces(ret) {
        format!("({expr}) as {ret}")
    } else {
        expr.to_string()
    }
}

/// A short human label for an unexpected statement kind in an equivalence-
/// obligation body (the `Unsupported` diagnostic).
fn stmt_kind(stmt: &Stmt) -> &'static str {
    match stmt {
        Stmt::Let { mutable: true, .. } => "a mutable `let`",
        Stmt::Let { .. } => "a `let`",
        Stmt::Assign { .. } => "an assignment",
        Stmt::Return(_) => "a non-leading `return`",
        Stmt::If { .. } => "an `if` statement",
        Stmt::Loop(_) => "a loop",
        Stmt::Break => "a `break`",
        Stmt::Continue => "a `continue`",
        Stmt::Expr(_) => "an expression statement",
    }
}

/// Emit the comma-separated parameter list. In spec context a slice param is the
/// `Seq` view (REQ-5); in exec context it is the plain `&[T]`.
fn emit_params(out: &mut String, params: &[Param], pos: Pos) -> Result<(), LowerError> {
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let ty = if pos == Pos::Spec {
            spec_param_type(&p.ty)?
        } else {
            lower_type(&p.ty)?
        };
        write!(out, "{}: {ty}", p.name).ok();
    }
    Ok(())
}

/// A `spec fn` parameter type: a `&[T]` slice becomes `Seq<T>` (REQ-5 — the
/// naive `&[u32]` form fails `verus` with `the trait bound &[u32]: Integer is
/// not satisfied`). Other types lower normally.
fn spec_param_type(ty: &Type) -> Result<String, LowerError> {
    if let Type::Ref { inner, .. } = ty {
        if let Type::Slice(elem) = inner.as_ref() {
            let e = lower_type(elem)?;
            return Ok(format!("Seq<{e}>"));
        }
    }
    lower_type(ty)
}

/// The return type of a `spec fn`. A slice-folding spec fn (one whose body sums
/// `elem as TY` over the slice — the `spec_sum` shape) returns `nat` so the
/// fold cannot overflow the spec relation (OQ-1). Detected by SHAPE: a `Match`
/// or `if/else` whose recursive arm adds a cast slice head to a recursive call.
fn lower_spec_fn_ret(ret: &Type, body: &Block) -> String {
    if is_head_fold_sum(body) || is_adt_fold_sum(body) {
        return "nat".to_string();
    }
    lower_type(ret).unwrap_or_else(|_| "bool".to_string())
}

/// Detect the GENERAL ADT structural-fold shape (`.design/basis/01-adts.md`
/// REQ-10 + the recorded structural-recursion finding): a `spec fn` over a
/// recursive ADT value whose body `match`es that value and whose arm(s) RECURSE
/// on the dereferenced recursive field — `f(*t)`, the `Box`-deref of REQ-3.
///
/// This is a STRUCTURAL predicate, NOT fitted to a base-arm shape (the #69
/// divergence): it does NOT require a literal-`0` unit base. Both folds detect:
///
/// - `sum_list`/`len` — literal-`0` unit base (`Nil => 0`) + a cons arm
///   `Cons(h, t) => <cast h> + f(*t)`;
/// - `tree_sum` — a VALUE-carrying base (`Leaf(v) => v as u64`) + a
///   binary-recursive arm `Node(l, r) => f(*l) + f(*r)`.
///
/// The single distinguishing signal is the presence of a recursive `f(*x)`
/// call ANYWHERE in some arm body (`expr_has_deref_call_arg`, a full-tree walk),
/// over a `match` of the function's `dec` value. Such a fold is lowered with a
/// `nat` return so EVERY arm's integer arithmetic stays `nat` and the arms
/// type-check uniformly (the GROUNDED form; without the `nat` return a base arm
/// like `v as u64` is `u64` while the recursive arm is `int`, and verus rejects
/// the `match` with `match arms have incompatible types`).
/// True if a spec-fn body is a `fold` SCHEME CALL (`.design/basis/02-recursion-
/// schemes.md` REQ-6): the body tail is an `Expr::Call` whose callee path is the
/// `fold` scheme. Such an instance returns `nat` (the `Accumulator` result), so
/// it joins `nat_fns` exactly as a hand-written ADT-fold-sum does. SHAPE check
/// (the callee path is `fold` and `fold` is a registered scheme), never a name
/// check; only `fold` is the `nat`-result scheme.
fn is_fold_scheme_call_body(body: &Block) -> bool {
    let Some(tail) = &body.tail else { return false };
    let Expr::Call { callee, .. } = tail.as_ref() else {
        return false;
    };
    let Expr::Path(segs) = callee.as_ref() else {
        return false;
    };
    segs.last().map(|s| s.as_str()) == Some("fold")
        && thermite_spec::schemes::lookup("fold").is_some()
}

fn is_adt_fold_sum(body: &Block) -> bool {
    let Some(tail) = &body.tail else { return false };
    let Expr::Match { arms, .. } = tail.as_ref() else {
        return false;
    };
    // GENERAL: a recursive structural fold has at least one arm that recurses
    // through a `Box`-deref'd field (`f(*x)`). The base arm(s) are whatever
    // remains — a literal `0`, a value-carrying `Leaf(v) => v`, etc. — and are
    // coerced to `nat` uniformly with the recursive arm by the `nat` return.
    arms.iter().any(|arm| expr_has_deref_call_arg(&arm.body))
}

/// True if a recursive structural-fold call `f(*x)` (a `Call` with a `Deref`
/// argument, the `*t` of REQ-3's `Box`-deref recursion) appears ANYWHERE in the
/// expression tree of an arm body — not just at its top level. A full-tree walk
/// so `Node(l, r) => f(*l) + f(*r)` (the recursive call nested under an `Add`)
/// and `Cons(h, t) => h as T + f(*t)` (nested under an `Add` rhs) are both
/// detected. SHAPE check, never a name check.
fn expr_has_deref_call_arg(expr: &Expr) -> bool {
    match expr {
        Expr::Call { callee, args } => {
            args.iter().any(|a| matches!(a, Expr::Deref(_)))
                || expr_has_deref_call_arg(callee)
                || args.iter().any(expr_has_deref_call_arg)
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_has_deref_call_arg(receiver) || args.iter().any(expr_has_deref_call_arg)
        }
        Expr::Field { receiver, .. } => expr_has_deref_call_arg(receiver),
        Expr::Binary { lhs, rhs, .. } => {
            expr_has_deref_call_arg(lhs) || expr_has_deref_call_arg(rhs)
        }
        Expr::Cast { expr, .. } => expr_has_deref_call_arg(expr),
        Expr::Ref { expr, .. } | Expr::Deref(expr) => expr_has_deref_call_arg(expr),
        Expr::Index { base, .. } => expr_has_deref_call_arg(base),
        Expr::Is { scrutinee, .. } => expr_has_deref_call_arg(scrutinee),
        Expr::Closure { body, .. } => expr_has_deref_call_arg(body),
        Expr::Match { scrutinee, arms } => {
            expr_has_deref_call_arg(scrutinee)
                || arms.iter().any(|a| expr_has_deref_call_arg(&a.body))
        }
        Expr::StructLit { fields, .. } => fields.iter().any(|(_, v)| expr_has_deref_call_arg(v)),
        // The prefix `!` (#92): a deref'd recursive call could sit under `!`,
        // so descend into the operand (the honest full-tree walk).
        Expr::Unary { expr, .. } => expr_has_deref_call_arg(expr),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): a
        // deref'd recursive call could sit in any tuple element or under a
        // projection's receiver — the honest full-tree walk descends into both.
        Expr::Tuple(elems) => elems.iter().any(expr_has_deref_call_arg),
        Expr::TupleProj { receiver, .. } => expr_has_deref_call_arg(receiver),
        Expr::IntLit { .. }
        | Expr::BoolLit(_)
        | Expr::Path(_)
        | Expr::StrLit(_)
        | Expr::If { .. } => false,
    }
}

// ---------------------------------------------------------------------------
// REQ-5: spec-fn body lowering — the slice match → Seq recursion.
// ---------------------------------------------------------------------------

/// Detect the head-fold-sum shape (`spec_sum`): a `match xs { [] => 0,
/// [head, ..t] => head as T + f(t) }` — an empty-slice base case of `0` and a
/// cons arm adding the (cast) head to a recursive call on the tail. This is a
/// SHAPE predicate over the AST, not a name check.
fn is_head_fold_sum(body: &Block) -> bool {
    let Some(tail) = &body.tail else { return false };
    let Expr::Match { arms, .. } = tail.as_ref() else {
        return false;
    };
    let mut has_empty_zero = false;
    let mut has_cons_add = false;
    for arm in arms {
        match &arm.pattern {
            Pattern::Slice(pats) if pats.is_empty() => {
                if matches!(&arm.body, Expr::IntLit { value: 0, .. }) {
                    has_empty_zero = true;
                }
            }
            Pattern::Slice(pats) if is_head_rest(pats) => {
                if let Expr::Binary { op: BinOp::Add, .. } = &arm.body {
                    has_cons_add = true;
                }
            }
            _ => {}
        }
    }
    has_empty_zero && has_cons_add
}

/// `[head, ..t]` shape: a binding then a rest.
fn is_head_rest(pats: &[SlicePat]) -> bool {
    matches!(
        pats,
        [SlicePat::Pat(Pattern::Binding(_)), SlicePat::Rest(_)]
    )
}

/// Lower a `spec fn` body. For the head-fold-sum shape, emit the verified `Seq`
/// recursion `if xs.len() == 0 { 0 } else { xs[0] as nat + f(xs.drop_first()) }`
/// (REQ-5). The recursion is reconstructed from the match arms' SHAPE: the base
/// arm's value, the head-element cast, and the recursive callee name.
fn lower_spec_fn_body(
    body: &Block,
    params: &[Param],
    ret: &str,
    variants: &[(&str, &str)],
    user_string_spec_fns: &[&str],
) -> Result<String, LowerError> {
    if is_head_fold_sum(body) {
        if let Some(slice) = first_slice_param(params) {
            if let Some(tail) = &body.tail {
                if let Expr::Match { arms, .. } = tail.as_ref() {
                    return seq_fold_body(slice, arms, ret);
                }
            }
        }
    }
    // Fallback: lower the block in spec context directly. An ADT fold (`sum_list`,
    // REQ-10) flows through HERE — its `match l { … }` lowers via `lower_match`
    // with the enum-variant map attached (ENUM-QUALIFIED arms) and, when the
    // return is `nat`, with `nat_ret` set so integer casts coerce to `as nat`
    // (the GROUNDED form's `h as nat + sum_list(*t)`).
    //
    // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): thread the spec fn's
    // `String`/`&String` param names so a String-SCANNING spec-fn body (a `&String`
    // param's `byte_at(i)`/`slice(lo,hi)` — e.g. `spec_line_start`, the spec twin
    // of an exec `\n`-scan) rewrites to the wrapper's SPEC accessors
    // (`spec_byte_at(i as int)`/`spec_slice(..)`) the SAME way the signature
    // `requires`/`ensures` context does (`lower_fn_signature`'s `.with_strings`).
    // WITHOUT this the body's `byte_at(i)` stays bare and hits the `usize`-typed
    // EXEC accessor in spec position (E0308 — `usize` vs `int`); the rewrite at
    // `lower_expr`'s String-receiver arm is gated on `ctx.is_string`. EMPTY for a
    // non-`String` spec fn (byte-stable for the existing corpus — `spec_sum`/ADT
    // folds carry no `String` param, so the set is empty and nothing changes).
    let strings = string_param_names(params);
    let ctx = Ctx::spec_seq()
        .with_variants(variants)
        .with_nat_ret(ret == "nat")
        .with_strings(&strings)
        .with_user_string_spec_fns(user_string_spec_fns);
    let mut out = String::from("{\n");
    let b = lower_block_inner(body, ctx, 1, zero_span())?;
    out.push_str(&b);
    out.push_str("}\n");
    Ok(out)
}

/// The name of the first slice (`&[T]`) parameter, used as the `Seq` recursion
/// subject.
fn first_slice_param(params: &[Param]) -> Option<&str> {
    params.iter().find_map(|p| match &p.ty {
        Type::Ref { inner, .. } if matches!(inner.as_ref(), Type::Slice(_)) => {
            Some(p.name.as_str())
        }
        _ => None,
    })
}

/// Build the `Seq` head-fold body from the match arms (REQ-5). `[] => B` becomes
/// `if xs.len() == 0 { B }`; `[head, ..t] => head as T + rec(t)` becomes
/// `else { xs[0] as nat + rec(xs.drop_first()) }`.
fn seq_fold_body(slice: &str, arms: &[MatchArm], ret: &str) -> Result<String, LowerError> {
    let mut base = String::from("0");
    let mut rec_name = String::new();
    let head_cast: String = if ret == "nat" {
        "nat".to_string()
    } else {
        ret.to_string()
    };
    for arm in arms {
        match &arm.pattern {
            Pattern::Slice(pats) if pats.is_empty() => {
                base = lower_expr(&arm.body, Ctx::spec_seq(), 0, zero_span())?;
            }
            Pattern::Slice(pats) if is_head_rest(pats) => {
                // The cons arm is `head as T + rec(t)`: pull the recursive callee.
                if let Expr::Binary { rhs, .. } = &arm.body {
                    if let Expr::Call { callee, .. } = rhs.as_ref() {
                        if let Expr::Path(segs) = callee.as_ref() {
                            if let Some(last) = segs.last() {
                                rec_name = last.clone();
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if rec_name.is_empty() {
        return Err(LowerError::Unsupported {
            what: "head-fold spec fn without a recursive tail call".to_string(),
            span: zero_span(),
        });
    }
    Ok(format!(
        "{{\n    if {slice}.len() == 0 {{ {base} }} else {{ {slice}[0] as {head_cast} + {rec_name}({slice}.drop_first()) }}\n}}\n"
    ))
}

// ---------------------------------------------------------------------------
// Basis Stage 2 (`.design/basis/02-recursion-schemes.md` REQ-6): scheme-CALL
// lowering — a scheme call → a call of the generated `fold_<e>`.
// ---------------------------------------------------------------------------

/// Lower a recursion-scheme CALL to a CALL of the generated scheme `spec fn`
/// (REQ-6): `fold(l, 0, |x, acc| x + acc)` → `fold_list(l, 0, |x: u64, acc: nat|
/// (x + acc) as nat)`; `for_all(l, |x| x > 0)` → `for_all_list(l, |x: u64| x >
/// 0)`. The scrutinee/seed args lower plainly; the trailing STEP closure is
/// lowered to a TYPED Verus `spec_fn` — element parameter `x: <elem>`, the
/// accumulator parameter `acc: <acc-ty>` for `fold`/`traverse`, and for an
/// accumulator (`fold`) the step body is coerced `as nat` (a `u64`/`nat` mixed
/// body is `int` in spec; the GROUNDED step is `(x + acc) as nat`). The validator
/// (Stage 2b) has already checked the call arity + the flat step.
fn lower_scheme_call(
    binding: &SchemeBinding,
    args: &[Expr],
    ctx: Ctx,
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    // The trailing argument is the step closure; everything before it is a
    // scrutinee/seed argument lowered plainly. The validator guaranteed the
    // closure shape, but be defensive (no panic, REQ-9): a missing step is
    // `Unsupported`.
    let Some((step, head_args)) = args.split_last() else {
        return Err(LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` with no arguments",
                binding.scheme_name
            ),
            span,
        });
    };

    let mut parts: Vec<String> = Vec::with_capacity(args.len());
    for a in head_args {
        parts.push(lower_expr(a, ctx, depth, span)?);
    }

    let Expr::Closure { params, body } = step else {
        return Err(LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` step must be a closure",
                binding.scheme_name
            ),
            span,
        });
    };

    // Lower the step body in spec context. For an accumulator scheme the body is
    // coerced `as nat` (the GROUNDED `(x + acc) as nat`); for a predicate / map
    // scheme the body stays as written (the closure result type already matches).
    let lowered_body = lower_expr(body, ctx.keep(), depth, span)?;
    let step_src = lower_step_closure(binding, params, &lowered_body, span)?;
    parts.push(step_src);

    Ok(format!("{}({})", binding.gen_name, parts.join(", ")))
}

/// Lower a scheme STEP closure to a typed Verus `spec_fn` literal (REQ-6). The
/// element parameter is typed `<elem>`; an accumulator scheme adds the `acc: nat`
/// (or `acc: bool` for `traverse`) parameter and coerces the body `as nat`. The
/// parameter NAMES are the surface closure's (`x`/`acc`), so the body's path
/// references resolve.
fn lower_step_closure(
    binding: &SchemeBinding,
    params: &[String],
    lowered_body: &str,
    span: Span,
) -> Result<String, LowerError> {
    use thermite_spec::SchemeResult;
    let elem = &binding.elem_ty;
    match binding.result {
        // fold: `|x: <elem>, acc: nat| (<body>) as nat`
        SchemeResult::Accumulator => {
            let (x, acc) = two_params(params, binding, span)?;
            Ok(format!("|{x}: {elem}, {acc}: nat| ({lowered_body}) as nat"))
        }
        // traverse: `|x: <elem>, acc: bool| <body>` (the body is already `bool`)
        SchemeResult::Bool if binding.scheme_name == "traverse" => {
            let (x, acc) = two_params(params, binding, span)?;
            Ok(format!("|{x}: {elem}, {acc}: bool| {lowered_body}"))
        }
        // for_all/exists: `|x: <elem>| <body>` (the body is already `bool`)
        SchemeResult::Bool => {
            let x = one_param(params, binding, span)?;
            Ok(format!("|{x}: {elem}| {lowered_body}"))
        }
        // map: `|x: <elem>| <body>` (the body returns `<elem>`)
        SchemeResult::SameAdt => {
            let x = one_param(params, binding, span)?;
            Ok(format!("|{x}: {elem}| {lowered_body}"))
        }
    }
}

/// The two step-closure parameter names for an accumulator scheme (REQ-6),
/// defensively erroring (no panic) if the validator's shape check was bypassed.
fn two_params<'p>(
    params: &'p [String],
    binding: &SchemeBinding,
    span: Span,
) -> Result<(&'p str, &'p str), LowerError> {
    match params {
        [x, acc] => Ok((x.as_str(), acc.as_str())),
        _ => Err(LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` step must have 2 parameters (`|x, acc|`)",
                binding.scheme_name
            ),
            span,
        }),
    }
}

/// The single step-closure parameter name for an element scheme (REQ-6).
fn one_param<'p>(
    params: &'p [String],
    binding: &SchemeBinding,
    span: Span,
) -> Result<&'p str, LowerError> {
    match params {
        [x] => Ok(x.as_str()),
        _ => Err(LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` step must have 1 parameter (`|x|`)",
                binding.scheme_name
            ),
            span,
        }),
    }
}

// ---------------------------------------------------------------------------
// REQ-2: type lowering.
// ---------------------------------------------------------------------------

/// Lower a `Type` to its Verus/Rust spelling (REQ-2). No lifetimes (§4.4).
fn lower_type(ty: &Type) -> Result<String, LowerError> {
    match ty {
        Type::Prim(PrimType::U32) => Ok("u32".to_string()),
        Type::Prim(PrimType::U64) => Ok("u64".to_string()),
        Type::Prim(PrimType::Usize) => Ok("usize".to_string()),
        Type::Prim(PrimType::Bool) => Ok("bool".to_string()),
        Type::Unit => Ok("()".to_string()),
        Type::Ref { mutable, inner } => {
            let i = lower_type(inner)?;
            if *mutable {
                Ok(format!("&mut {i}"))
            } else {
                Ok(format!("&{i}"))
            }
        }
        Type::Slice(inner) => {
            let i = lower_type(inner)?;
            Ok(format!("[{i}]"))
        }
        Type::Generic { name, arg } => {
            let a = lower_type(arg)?;
            Ok(format!("{name}<{a}>"))
        }
        // Basis Stage 1c (`.design/basis/01-adts.md` REQ-1/REQ-2/REQ-10): a
        // user-defined `struct`/`enum` type is its bare name (`Account`, `Shape`,
        // `List`) — the type-side complement of the lowered `Item::Struct`/
        // `Item::Enum`. `Box<T>` is the heap-indirection primitive emitted as a
        // Verus `Box<…>` (the recursive occurrence `Box<List>`, REQ-10), which
        // Verus models natively for a recursive datatype.
        Type::Named(name) => Ok(name.clone()),
        Type::Box(inner) => {
            let i = lower_type(inner)?;
            Ok(format!("Box<{i}>"))
        }
        // Basis Stage 4 (`.design/basis/04-collections.md` REQ-5): a bounded
        // `Vec<T>` lowers to the Thermite-runtime newtype `TVec<elem>` over
        // `vstd::vec::Vec<T>` (the GROUNDED `BVec`-over-`Vec<u64>` form). The
        // wrapper struct + its verified `len`/`spec_get`/`get`/`push` impl are
        // materialized ONCE per element type by `emit_vec_wrappers`; this arm
        // names the type (`Vec<u64>` → `TVecU64`). The wrapper carries the
        // `well_formed` capacity invariant + the no-OOB `get` + capacity-preserving
        // `push`, the §4.2-decidable bounded structure. BACKING-AGNOSTIC SURFACE
        // (#62): the surface contract names `len`/`get`/`push` over `v@`, never
        // `vstd::vec::Vec`; v1 IMPLEMENTS it by wrapping vstd's verified `Vec`.
        Type::Vec(inner) => Ok(tvec_name(inner)?),
        // Basis Stage 7 (`.design/basis/07-strings.md` REQ-2/REQ-4): a bounded
        // `String` lowers to the Thermite-runtime newtype `TString` over
        // `vstd::vec::Vec<u8>` (the GROUNDED `TString`-over-`Vec<u8>` form,
        // `verified, 0 errors`). The wrapper struct + its verified
        // `well_formed`/`len`/`byte_at`/`concat`/`slice` impl are materialized
        // ONCE by `emit_string_wrapper`; this arm names the type. The element
        // type is FIXED to `u8` (the char model is bytes for v1), so — unlike
        // `Type::Vec(elem)` — there is no per-element monomorphization. BACKING-
        // AGNOSTIC SURFACE (#62): the surface contract names `len`/`byte_at` over
        // the byte view `s@`, never `vstd::vec::Vec<u8>`; v1 IMPLEMENTS it by
        // wrapping vstd's verified `Vec<u8>`.
        Type::String => Ok("TString".to_string()),
        // Cluster C7 (`.design/basis/09-option-result.md` REQ-4, issue #95): the
        // built-in `Option<T>` / `Result<T, E>` lower to the Verus-NATIVE generic
        // types — Verus's prelude carries `Option`/`Result` and their constructors
        // `Some`/`None`/`Ok`/`Err` (GROUNDED `5 verified, 0 errors`), so there is NO
        // wrapper to emit (unlike `TString`/`TVec`) and the constructors fall
        // through `qualify_variant_path` to the BARE name (they are not user-enum
        // variants, so they are never enum-qualified — exactly what Verus wants).
        // The element/error types lower recursively.
        Type::Option(inner) => {
            let i = lower_type(inner)?;
            Ok(format!("Option<{i}>"))
        }
        Type::Result(ok, err) => {
            let o = lower_type(ok)?;
            let e = lower_type(err)?;
            Ok(format!("Result<{o}, {e}>"))
        }
        // Cluster C12 (`.design/basis/13-map.md` REQ-4): a bounded `Map<K, V>` lowers
        // to the Thermite-runtime newtype `TMap<K,V>` over a `vstd::vec::Vec<(K, V)>`-
        // of-pairs backing (C6 `Vec<tuple>` + C9 `(K,V)` pair). The wrapper struct +
        // its verified `spec_dom`/`spec_contains_key`/`len`/`contains_key`/`get`/
        // `insert` impl are materialized ONCE per `(K, V)` pair by `emit_map_wrappers`;
        // this arm names the type (`Map<u64, u64>` → `TMapU64U64`). The wrapper carries
        // the `well_formed` capacity + key-uniqueness invariant + the no-OOB /
        // handled-or-loud `get -> Option<V>` (absent → None). BACKING-AGNOSTIC SURFACE
        // (#62/#114): the surface contract names `len`/`get`/`contains_key`/`insert`
        // over a spec map abstraction, never `vstd::vec::Vec<(K,V)>`; v1 IMPLEMENTS it
        // by wrapping the Vec-of-pairs (a later vstd-hash-map decouple swaps the
        // backing WITHOUT changing user `.th` code).
        Type::Map(k, v) => Ok(tmap_name(k, v)?),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-7/REQ-8,
        // #109): an n-tuple type `(T, U, …)` lowers to the Verus-NATIVE tuple type
        // `(<t0>, <t1>, …)` (Verus tuples are native, GROUNDED at arity 2 and 3 —
        // `verified, 0 errors`); each element type lowers recursively. There is no
        // wrapper to emit (like `Option`/`Result`, unlike `TString`/`TVec`).
        Type::Tuple(elems) => {
            let parts = elems
                .iter()
                .map(lower_type)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("({})", parts.join(", ")))
        }
    }
}

/// The generated wrapper struct name for `Vec<elem>` — `TVec` plus an
/// UpperCamelCase suffix derived from the element type's Verus spelling
/// (`Vec<u64>` → `TVecU64`, `Vec<u32>` → `TVecU32`, `Vec<usize>` → `TVecUsize`)
/// (`.design/basis/04-collections.md` REQ-5 / REQ-9 / REQ-12). A per-element-type
/// concrete wrapper (not a generic `TVec<T>`) is the GROUNDED form: vstd's
/// `Vec<T>` index `self.data[i]` moves the element out (`E0507` for non-`Copy`
/// `T`), so the verified accessor is monomorphized per element type.
///
/// CLUSTER C6 (#98, REQ-9/REQ-12): the suffix `match` EXTENDS from Copy primitives
/// to the NON-`Copy` element types — a `String` element (→ `TVecTString`, the
/// Stage-7 string-wrapper element name), a user `struct`/`enum` element (→
/// `TVec<StructName>`, the bare decl name), and a NESTED `Vec<Vec<_>>` element (→
/// the RECURSIVE `T` + inner `tvec_name`, so `Vec<Vec<u64>>` → `TVecTVecU64`). A
/// non-Copy element's wrapper emits the BORROW-returning `get -> &T` (REQ-9); a
/// Copy element keeps the by-value `get -> T` (the byte-stable `vec_demo.th` form)
/// — classified by [`elem_is_copy`]. A still-unlowerable element (`Unit`/`Ref`/
/// `Slice`/`Generic`) is the existing `LowerError::Unsupported` (no panic, REQ-12).
pub(crate) fn tvec_name(elem: &Type) -> Result<String, LowerError> {
    let suffix = match elem {
        Type::Prim(PrimType::U32) => "U32".to_string(),
        Type::Prim(PrimType::U64) => "U64".to_string(),
        Type::Prim(PrimType::Usize) => "Usize".to_string(),
        Type::Prim(PrimType::Bool) => "Bool".to_string(),
        // Cluster C6 (REQ-9): a `String` element → the Stage-7 wrapper name
        // `TString`, so `Vec<String>` → `TVecTString` (the GROUNDED non-Copy form).
        Type::String => "TString".to_string(),
        // Cluster C6 (REQ-9): a user `struct`/`enum` element → its bare decl name,
        // so `Vec<Point>` → `TVecPoint`. The decl is woven before the wrapper
        // (REQ-10, the #68 `collect_type_adt_refs` recursion through `Type::Vec`).
        Type::Named(name) => name.clone(),
        // Cluster C6 (REQ-9, nested): a `Vec<Vec<_>>` element → the RECURSIVE inner
        // wrapper name as the suffix, so `Vec<Vec<u64>>` → `TVec` + `TVecU64` =
        // `TVecTVecU64` (the inner `tvec_name` already yields `TVecU64`; the outer
        // prepends `TVec`). The inner `TVec*` wrapper is emitted before the outer
        // (REQ-10). The recursion is bounded by `Type` nesting (finite).
        Type::Vec(inner) => tvec_name(inner)?,
        other => {
            return Err(LowerError::Unsupported {
                what: format!(
                    "Vec element type {:?} (v1 wraps a Copy primitive, a String, a \
                     user struct/enum, or a nested Vec element)",
                    lower_type(other).unwrap_or_else(|_| "<unlowerable>".to_string())
                ),
                span: zero_span(),
            });
        }
    };
    Ok(format!("TVec{suffix}"))
}

/// The generated wrapper struct name for `Map<K, V>` — `TMap` plus the
/// UpperCamelCase suffix of the KEY type's wrapper-suffix and the VALUE type's
/// (`Map<u64, u64>` → `TMapU64U64`) (`.design/basis/13-map.md` REQ-4). A
/// per-`(K,V)`-pair concrete wrapper (not a generic `TMap<K,V>`) MIRRORS
/// [`tvec_name`]'s monomorphization: the GROUNDED `TMapU64U64`-over-
/// `vstd::vec::Vec<(u64,u64)>` form. v1 grounds `Map<u64, u64>` (Copy keys, OQ-4);
/// the suffix reuses `tmap_type_suffix` so a future `Map<String, u64>` /
/// `Map<u64, Account>` composes by the SAME rule the SHIPPED `Vec<String>`/
/// `Vec<struct>` proved. A still-unlowerable key/value type is the existing
/// `LowerError::Unsupported` (no panic, REQ-6).
pub(crate) fn tmap_name(key: &Type, val: &Type) -> Result<String, LowerError> {
    let ks = tmap_type_suffix(key)?;
    let vs = tmap_type_suffix(val)?;
    Ok(format!("TMap{ks}{vs}"))
}

/// The UpperCamelCase wrapper-name suffix for a single `Map` key/value type
/// (`.design/basis/13-map.md` REQ-4 / OQ-4). MIRRORS the suffix arm of
/// [`tvec_name`] (a Copy primitive, a `String` → `TString`, a user `struct`/`enum`
/// → its bare decl name, a nested `Vec` → its `tvec_name`), so a `(K, V)` pair
/// monomorphizes by the SAME rule the SHIPPED `Vec` element does. A still-
/// unlowerable type is the existing `LowerError::Unsupported` (no panic, REQ-6).
fn tmap_type_suffix(ty: &Type) -> Result<String, LowerError> {
    Ok(match ty {
        Type::Prim(PrimType::U32) => "U32".to_string(),
        Type::Prim(PrimType::U64) => "U64".to_string(),
        Type::Prim(PrimType::Usize) => "Usize".to_string(),
        Type::Prim(PrimType::Bool) => "Bool".to_string(),
        Type::String => "TString".to_string(),
        Type::Named(name) => name.clone(),
        Type::Vec(inner) => tvec_name(inner)?,
        other => {
            return Err(LowerError::Unsupported {
                what: format!(
                    "Map key/value type {:?} (v1 grounds Map<u64, u64>; the suffix \
                     mirrors a Vec element — a Copy primitive, a String, a user \
                     struct/enum, or a nested Vec)",
                    lower_type(other).unwrap_or_else(|_| "<unlowerable>".to_string())
                ),
                span: zero_span(),
            });
        }
    })
}

/// True iff `expr` is the bounded-`Map` no-param constructor `Map::new()`
/// (`.design/basis/13-map.md` REQ-4, mirroring [`is_vec_new`]): an `Expr::Call`
/// whose callee path is `Map::new` with no arguments. Drives the `let`-init
/// rewrite to the wrapper construction `<TMap> { data: Vec::new() }`.
pub(crate) fn is_map_new(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call { callee, args }
            if args.is_empty()
                && matches!(callee.as_ref(), Expr::Path(segs)
                    if segs.len() == 2 && segs[0] == "Map" && segs[1] == "new")
    )
}

/// True iff a `Map` key type is `Copy` (`.design/basis/13-map.md` OQ-4): v1
/// grounds `Map<u64, u64>` (Copy key, by-value `get -> Option<V>`). A non-Copy key
/// (`Map<String, _>`) needs the borrow-comparison rule (the REQ-9 finding), flagged
/// as the breadth v1 does not exercise. Used to gate the by-value value return in
/// the emitted `get`.
fn map_key_is_copy(key: &Type) -> bool {
    elem_is_copy(key)
}

/// True iff a `Vec` element type is `Copy` (`.design/basis/04-collections.md`
/// REQ-9): a Copy element (`u32`/`u64`/`usize`/`bool`) keeps the by-value
/// accessor `get -> T` / `last -> T` (the byte-stable `vec_demo.th` form, vstd's
/// index `self.data[i]` copies); a NON-Copy element (`String`/struct/enum/nested
/// `Vec`) MUST use the BORROW accessor `get -> &T` / `last -> &T` (`&self.data[i]`),
/// because vstd's index MOVES a non-Copy element out of the backing `Vec` (`E0507`,
/// the Stage-4 finding the borrow resolves). A `contains` (element `==`) is emitted
/// only for a Copy element — `==` on a `String`/struct in exec position is not a
/// v1 built-in (it joins when a corpus program needs it, REQ-1 frozen-set).
/// True iff `expr` is the bounded-`Vec` no-param constructor `Vec::new()`
/// (`.design/basis/04-collections.md` REQ-11): an `Expr::Call` whose callee path
/// is `Vec::new` (or a bare `new` on `Vec`) with no arguments. Drives the
/// `let`-init rewrite to the wrapper construction `<TVec> { data: Vec::new() }`.
pub(crate) fn is_vec_new(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call { callee, args }
            if args.is_empty()
                && matches!(callee.as_ref(), Expr::Path(segs)
                    if segs.len() == 2 && segs[0] == "Vec" && segs[1] == "new")
    )
}

pub(crate) fn elem_is_copy(elem: &Type) -> bool {
    matches!(
        elem,
        Type::Prim(PrimType::U32)
            | Type::Prim(PrimType::U64)
            | Type::Prim(PrimType::Usize)
            | Type::Prim(PrimType::Bool)
    )
}

// ---------------------------------------------------------------------------
// Basis Stage 4 (`.design/basis/04-collections.md` REQ-5): the bounded-`Vec`
// wrapper emission. A Thermite `Vec<T>` lowers to a newtype `TVec<elem>` over
// `vstd::vec::Vec<T>` with the verified `len`/`spec_get`/`get`/`push` impl — the
// GROUNDED `BVec`-over-`Vec<u64>` form. Materialized ONCE per element type, so a
// program using `Vec<u64>` in many fns emits a single `TVecU64`.
// ---------------------------------------------------------------------------

/// The bounded-`Vec` capacity constant `CAP` (`.design/basis/04-collections.md`
/// REQ-5 / the GROUNDED `BVec` `spec const CAP`): the SAME `1_000_000` bound the
/// corpus idiom uses (`conformance/sum.th` `req xs.len() <= 1_000_000`;
/// `conformance/vec_demo.th` `push_one` `req v.len() < 1_000_000`). A `Vec` is
/// bounded by design so the §4.2 cage never sees an unbounded sequence.
const VEC_CAP: u64 = 1_000_000;

/// Collect, in deterministic order and deduped, the element type of every
/// `Vec<T>` the program references ANYWHERE it is REACHABLE
/// (`.design/basis/04-collections.md` REQ-5/REQ-10/REQ-11). The wrapper struct is
/// materialized once per element type.
///
/// CLUSTER C6 (#98, REQ-11 — the `Vec::new()`-no-param reachability fix, the #86
/// String-reachability analog): a `Vec<T>` is REACHABLE not only in a `fn`/`spec
/// fn` PARAMETER or RETURN position (the original closure) but also in a
/// `struct`/`enum`-variant FIELD type and a `fn`-body local `let` type annotation —
/// a body-local `let mut v: Vec<u64> = Vec::new();` with no `Vec` param/return must
/// still emit `TVecU64` (else `E0425 cannot find type TVecU64`). The closure walks
/// the SAME reachability set as `program_uses_string` (`ty_reaches_string` over
/// param/return + field + local `let`), keyed on `Type::Vec(inner)`.
///
/// CLUSTER C6 (#98, REQ-10 — emission ORDER for nested): for a NESTED `Vec<Vec<_>>`
/// the outer wrapper (`TVecTVecU64`) references the inner wrapper (`TVecU64`), so
/// the inner element is noted BEFORE the outer — [`note`] recurses into a `Vec`
/// element first, then pushes the element itself, so the inner `TVec*` is emitted
/// before the outer (verus needs each in scope before the wrapper that names it).
pub(crate) fn collect_vec_elem_types(program: &Program) -> Vec<Type> {
    let mut elems: Vec<Type> = Vec::new();
    for item in &program.items {
        match item {
            Item::Fn(f) => {
                for p in &f.params {
                    note_vec_elems(&p.ty, &mut elems);
                }
                note_vec_elems(&f.ret, &mut elems);
                if let Some(b) = &f.body {
                    note_block_vec_elems(b, &mut elems);
                }
            }
            Item::SpecFn(s) => {
                for p in &s.params {
                    note_vec_elems(&p.ty, &mut elems);
                }
                note_vec_elems(&s.ret, &mut elems);
                note_block_vec_elems(&s.body, &mut elems);
            }
            // REQ-10/REQ-11: a `struct`/`enum`-variant FIELD typed `Vec<T>` reaches
            // the element wrapper exactly as a param does (a `Buf { items: Vec<u64> }`
            // or an enum payload). The #86 String-reachability analog over fields.
            Item::Struct(s) => {
                for fd in &s.fields {
                    note_vec_elems(&fd.ty, &mut elems);
                }
            }
            Item::Enum(e) => {
                for v in &e.variants {
                    match &v.shape {
                        VariantShape::Unit => {}
                        VariantShape::Tuple(tys) => {
                            for ty in tys {
                                note_vec_elems(ty, &mut elems);
                            }
                        }
                        VariantShape::Struct(fields) => {
                            for fd in fields {
                                note_vec_elems(&fd.ty, &mut elems);
                            }
                        }
                    }
                }
            }
        }
    }
    // Cluster C5 (`.design/basis/07-strings.md` REQ-15, issue #102): the emitted
    // `TString::split` method RETURNS a `TVecTString` (the `Vec<String>` wrapper), and
    // the `split` method is emitted whenever ANY C5 op is used
    // (`program_uses_string_search`) — including in a per-item subprogram (forge's
    // `item_subprogram`) whose own return type is NOT `Vec<String>`. So weave the
    // `Vec<String>` element (→ `TVecTString`) whenever the program uses a C5 op, so the
    // wrapper `split` references is ALWAYS in scope (the REQ-10 element-wrapper weave,
    // applied to `split`'s implicit result type). `note_vec_elems` dedups, so a
    // program that ALSO has an explicit `Vec<String>` does not double-emit.
    if program_uses_string_search(program) {
        note_vec_elems(&Type::Vec(Box::new(Type::String)), &mut elems);
    }
    elems
}

/// Note the `Vec` element type(s) REACHABLE in a single `Type` (REQ-10/REQ-11),
/// deduped, INNER-FIRST. A nested `Vec<Vec<u64>>` element is itself a `Type::Vec`,
/// so the inner element is noted BEFORE pushing the outer element — the inner
/// `TVec*` wrapper is emitted before the outer that references it (REQ-10 emission
/// order). Recurses through every type constructor (`Ref`/`Slice`/`Box`/`Generic`/
/// `Vec`) so a `Vec` nested under a reference/box is reached.
fn note_vec_elems(ty: &Type, elems: &mut Vec<Type>) {
    match ty {
        Type::Vec(inner) => {
            // INNER-FIRST: a `Vec<u64>` element of `Vec<Vec<u64>>` must be noted
            // (and emitted) before the outer (REQ-10). Recurse into the element so
            // its own nested `Vec`s are noted first, then push the element itself.
            note_vec_elems(inner, elems);
            let e = (**inner).clone();
            if !elems.contains(&e) {
                elems.push(e);
            }
        }
        Type::Ref { inner, .. }
        | Type::Slice(inner)
        | Type::Box(inner)
        | Type::Generic { arg: inner, .. }
        // Cluster C7 (`.design/basis/09-option-result.md` REQ-4): a `Vec` nested
        // under an `Option<Vec<_>>` is reached through the option's element type,
        // exactly as a `Box`/`Generic` inner is.
        | Type::Option(inner) => note_vec_elems(inner, elems),
        // A `Result<T, E>` reaches a `Vec` in EITHER type argument (the `T` ok type
        // or the `E` error type), so both are recursed (inner-first preserved).
        Type::Result(ok, err) => {
            note_vec_elems(ok, elems);
            note_vec_elems(err, elems);
        }
        // Cluster C12 (`.design/basis/13-map.md` REQ-5): a `Map<K, V>` reaches a
        // `Vec` in EITHER type argument (the key or the value), so both are recursed
        // (inner-first preserved, exactly as `Result`'s two arguments are). A `Map`
        // itself does NOT carry a `TVec` element type — its Vec-of-pairs backing is
        // emitted by `emit_map_wrappers`, not the `TVec` path — so the `Map` node is
        // not pushed as a `Vec` element; only its arguments are walked for nested
        // `Vec`s.
        Type::Map(k, v) => {
            note_vec_elems(k, elems);
            note_vec_elems(v, elems);
        }
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): a
        // tuple type `(T, U, …)` reaches a `Vec` in ANY of its element types, so
        // every element is recursed (inner-first preserved, exactly as `Result`'s
        // two arguments are).
        Type::Tuple(tys) => {
            for t in tys {
                note_vec_elems(t, elems);
            }
        }
        Type::Prim(_) | Type::Unit | Type::Named(_) | Type::String => {}
    }
}

/// Note every `Vec` element type reachable in a body-local `let` type annotation
/// (REQ-11 — the `Vec::new()`-no-param fix). Walks nested `if`/`loop` blocks, the
/// SAME body-walk shape as `block_has_string_local` (the #86 analog), keyed on the
/// `let`'s declared type via `note_vec_elems`.
fn note_block_vec_elems(block: &Block, elems: &mut Vec<Type>) {
    for stmt in &block.stmts {
        note_stmt_vec_elems(stmt, elems);
    }
}

fn note_stmt_vec_elems(stmt: &Stmt, elems: &mut Vec<Type>) {
    match stmt {
        Stmt::Let { ty, .. } => {
            if let Some(t) = ty {
                note_vec_elems(t, elems);
            }
        }
        Stmt::If { then, else_, .. } => {
            note_block_vec_elems(then, elems);
            if let Some(e) = else_ {
                note_block_vec_elems(e, elems);
            }
        }
        Stmt::Loop(l) => note_block_vec_elems(&l.body, elems),
        Stmt::Assign { .. } | Stmt::Return(_) | Stmt::Expr(_) | Stmt::Break | Stmt::Continue => {}
    }
}

/// Emit the `TVec<elem>` wrapper struct + its verified `len`/`spec_get`/`get`/
/// `push` impl for every element type the program uses (REQ-5), in deterministic
/// order. EMPTY when the program uses no `Vec` (byte-stable for the non-Vec
/// corpus). The emitted form is EXACTLY the GROUNDED `BVec` over `vstd::vec::Vec`
/// (`verified, 0 errors`):
///
/// ```verus
/// pub struct TVecU64 { pub data: Vec<u64> }
/// impl TVecU64 {
///     pub open spec fn well_formed(&self) -> bool { self.data.len() <= 1000000 }
///     pub open spec fn len(&self) -> nat { self.data.len() as nat }
///     pub open spec fn spec_get(&self, i: int) -> u64 { self.data@[i] }
///     pub fn get(&self, i: usize) -> (result: u64)
///         requires i < self.data.len(),
///         ensures result == self.data@[i as int],
///     { self.data[i] }
///     pub fn push(&mut self, x: u64)
///         requires old(self).well_formed(), old(self).data.len() < 1000000,
///         ensures
///             final(self).well_formed(),
///             final(self).data.len() == old(self).data.len() + 1,
///             final(self).data@[old(self).data.len() as int] == x,
///     { self.data.push(x) }
/// }
/// ```
///
/// THE `final(self)` FINDING (REQ-5 / the design's recorded migration note): verus
/// 0.2026.05.24 requires `final(self)` (NOT bare `self`) to disambiguate a `&mut`
/// receiver in a `push` postcondition. The `well_formed` capacity invariant + the
/// no-OOB `get` (`req i < len`) + the capacity-preserving `push` (`req len < CAP`)
/// are the Thermite-level additions threaded over vstd's verified `Vec::push`/
/// `Vec::index`/`Vec::len` (which carry the heap proof) — NO `assume`/`external_body`
/// (R-DEFER-9; the broken unguarded forms FAIL verus, the non-vacuity proof).
fn emit_vec_wrappers(program: &Program) -> Result<String, LowerError> {
    let elems = collect_vec_elem_types(program);
    if elems.is_empty() {
        return Ok(String::new());
    }
    let mut out = String::new();
    for elem in &elems {
        out.push_str(&emit_one_vec_wrapper(elem)?);
    }
    Ok(out)
}

/// Emit ONE `TVec<elem>` wrapper struct + its verified op `impl` for a single
/// element type (`.design/basis/04-collections.md` REQ-5/REQ-8/REQ-9). The
/// `well_formed`/`len`/`spec_get`/`push`/`pop_last`/`insert`/`remove` ops are
/// element-type-AGNOSTIC (they shift/drop indices, never moving an element OUT),
/// so they are identical for Copy and non-Copy elements. The element-RETURNING
/// accessors `get`/`last` and `contains` diverge by Copy-ness (REQ-9):
///
/// - **Copy element** (`u64` etc.): `get -> T` / `last -> T` by VALUE (vstd's
///   index copies, the byte-stable `vec_demo.th` form) + `contains` (element `==`).
/// - **NON-Copy element** (`String`/struct/nested `Vec`): `get -> &T` / `last ->
///   &T` by BORROW (`&self.data[i]`, `ens *result == v@[i]`), because vstd's index
///   MOVES a non-Copy element out (`E0507`, the Stage-4 finding); `contains` is
///   omitted (element `==` on a non-Copy type is not a v1 exec built-in).
///
/// GROUNDED (real `verus 0.2026.05.24`): `Vec<u64>` all ops `9 verified, 0 errors`;
/// `Vec<String>`/`Vec<struct>`/nested `Vec<Vec<u64>>` the borrow form
/// `4 verified, 0 errors`; the by-value `get` on a non-Copy element `E0507`. NO
/// `assume`/`external_body` (R-DEFER-9) — every contract is threaded over vstd's
/// verified `Vec::push`/`pop`/`insert`/`remove`/`index`/`len`. The `&mut`-mutating
/// ops (`push`/`pop_last`/`insert`/`remove`) use `final(self)` (the REQ-5
/// grounding: verus 0.2026.05.24 disambiguates a `&mut` postcondition with
/// `final`).
fn emit_one_vec_wrapper(elem: &Type) -> Result<String, LowerError> {
    let name = tvec_name(elem)?;
    let ety = lower_type(elem)?;
    let copy = elem_is_copy(elem);
    let mut out = String::new();
    out.push('\n');
    writeln!(out, "pub struct {name} {{ pub data: Vec<{ety}> }}").ok();
    writeln!(out, "impl {name} {{").ok();
    writeln!(
        out,
        "    pub open spec fn well_formed(&self) -> bool {{ self.data.len() <= {VEC_CAP} }}"
    )
    .ok();
    out.push_str("    pub open spec fn len(&self) -> nat { self.data.len() as nat }\n");
    writeln!(
        out,
        "    pub open spec fn spec_get(&self, i: int) -> {ety} {{ self.data@[i] }}"
    )
    .ok();
    // The no-OOB exec accessor `get` (REQ-5/REQ-9): `req i < len`. A Copy element
    // returns by VALUE (`result == v@[i]`, vstd's index copies); a non-Copy element
    // returns a BORROW `&T` (`*result == v@[i]`, `&self.data[i]` — vstd's index
    // would MOVE the element out, `E0507`, so the borrow is the load-bearing fix).
    if copy {
        writeln!(out, "    pub fn get(&self, i: usize) -> (result: {ety})").ok();
        out.push_str("        requires i < self.data.len(),\n");
        out.push_str("        ensures result == self.data@[i as int],\n");
        out.push_str("    { self.data[i] }\n");
    } else {
        writeln!(out, "    pub fn get(&self, i: usize) -> (result: &{ety})").ok();
        out.push_str("        requires i < self.data.len(),\n");
        out.push_str("        ensures *result == self.data@[i as int],\n");
        out.push_str("    { &self.data[i] }\n");
    }
    // The capacity-preserving exec mutator `push` (REQ-5): `req well_formed && len <
    // CAP`, `ens final(self).well_formed() && len' == len+1 && v@[old_len] == x`. The
    // `final(self)` &mut postcondition. `push(x: T)` CONSUMES the owned element (no
    // `Copy` needed — moves the value into the backing run), so it is identical for
    // Copy and non-Copy elements (REQ-9).
    writeln!(out, "    pub fn push(&mut self, x: {ety})").ok();
    out.push_str("        requires old(self).well_formed(), old(self).data.len() < ");
    writeln!(out, "{VEC_CAP},").ok();
    out.push_str("        ensures\n");
    out.push_str("            final(self).well_formed(),\n");
    out.push_str("            final(self).data.len() == old(self).data.len() + 1,\n");
    out.push_str("            final(self).data@[old(self).data.len() as int] == x,\n");
    // The element-preservation frame (REQ-5 GROUNDED `BVec::push` seed): `push`
    // APPENDS without disturbing the prior elements, so a caller can prove an
    // earlier `get(j)` still reads the originally-pushed element after a later
    // `push` (the accumulator soundness — a token list / editor buffer keeps its
    // earlier elements). Mirrors the `pop_last` kept-prefix frame below.
    out.push_str("            forall|j: int| 0 <= j < old(self).data.len()\n");
    out.push_str("                ==> final(self).data@[j] == old(self).data@[j],\n");
    out.push_str("    { self.data.push(x) }\n");
    // The tuple-free `pop_last` (REQ-8): drop the LAST element. `req len > 0`, `ens
    // len' == len-1` + the kept-prefix frame. `&mut`, `final(self)`. The companion
    // `last` reads the value (no tuple). Element-agnostic (vstd's `pop` drops an
    // index, never moving an element out as a result here).
    out.push_str("    pub fn pop_last(&mut self)\n");
    out.push_str("        requires old(self).data.len() > 0,\n");
    out.push_str("        ensures\n");
    out.push_str("            final(self).data.len() == old(self).data.len() - 1,\n");
    out.push_str("            forall|j: int| 0 <= j < final(self).data.len()\n");
    out.push_str("                ==> final(self).data@[j] == old(self).data@[j],\n");
    out.push_str("    { self.data.pop(); }\n");
    // The final-element accessor `last` (REQ-8): `req len > 0`, `ens result ==
    // v@[len-1]`. `&self`-reading (no `final`). Copy → by VALUE; non-Copy → BORROW
    // `&T` (the same no-move rule as `get`, REQ-9).
    if copy {
        writeln!(out, "    pub fn last(&self) -> (result: {ety})").ok();
        out.push_str("        requires self.data.len() > 0,\n");
        out.push_str("        ensures result == self.data@[self.data.len() - 1],\n");
        out.push_str("    { self.data[self.data.len() - 1] }\n");
    } else {
        writeln!(out, "    pub fn last(&self) -> (result: &{ety})").ok();
        out.push_str("        requires self.data.len() > 0,\n");
        out.push_str("        ensures *result == self.data@[self.data.len() - 1],\n");
        out.push_str("    { &self.data[self.data.len() - 1] }\n");
    }
    // The `insert` op (REQ-8): splice `x` at index `i`, shifting the suffix right.
    // `req well_formed && len < CAP && i <= len` (the `i <= len` is the no-OOB
    // safety — an insert AT `len` is an append, NOT `i < len`). `ens len' == len+1
    // && v'@ == v@.insert(i, x)`. `&mut`, `final(self)`. Element-agnostic.
    writeln!(out, "    pub fn insert(&mut self, i: usize, x: {ety})").ok();
    out.push_str("        requires old(self).well_formed(), old(self).data.len() < ");
    writeln!(out, "{VEC_CAP}, i <= old(self).data.len(),").ok();
    out.push_str("        ensures\n");
    out.push_str("            final(self).well_formed(),\n");
    out.push_str("            final(self).data.len() == old(self).data.len() + 1,\n");
    out.push_str("            final(self).data@ == old(self).data@.insert(i as int, x),\n");
    out.push_str("    { self.data.insert(i, x); }\n");
    // The `remove` op (REQ-8): delete the element at `i`, shifting the suffix left.
    // `req i < len`, `ens len' == len-1 && v'@ == v@.remove(i)`. `&mut`,
    // `final(self)`. Element-agnostic.
    out.push_str("    pub fn remove(&mut self, i: usize)\n");
    out.push_str("        requires i < old(self).data.len(),\n");
    out.push_str("        ensures\n");
    out.push_str("            final(self).data.len() == old(self).data.len() - 1,\n");
    out.push_str("            final(self).data@ == old(self).data@.remove(i as int),\n");
    out.push_str("    { self.data.remove(i); }\n");
    // The `contains` op (REQ-8): an EXEC linear scan with the standard
    // `forall|k| 0 <= k < i ==> v@[k] != x` invariant + `decreases len - i`. `req
    // well_formed`, `ens result == exists|k| 0<=k<len && v@[k]==x`. Emitted only for
    // a Copy element — `==` on a non-Copy element (`String`/struct) in exec position
    // is not a v1 built-in (REQ-9; it joins when a corpus program needs it).
    if copy {
        writeln!(
            out,
            "    pub fn contains(&self, x: {ety}) -> (result: bool)"
        )
        .ok();
        out.push_str("        requires self.well_formed(),\n");
        out.push_str(
            "        ensures result == (exists|k: int| 0 <= k < self.data.len() && self.data@[k] == x),\n",
        );
        out.push_str("    {\n");
        out.push_str("        let mut i: usize = 0;\n");
        out.push_str("        while i < self.data.len()\n");
        out.push_str("            invariant\n");
        out.push_str("                i <= self.data.len(),\n");
        out.push_str("                forall|k: int| 0 <= k < i ==> self.data@[k] != x,\n");
        out.push_str("            decreases self.data.len() - i,\n");
        out.push_str("        {\n");
        out.push_str("            if self.data[i] == x {\n");
        out.push_str("                return true;\n");
        out.push_str("            }\n");
        out.push_str("            i = i + 1;\n");
        out.push_str("        }\n");
        out.push_str("        false\n");
        out.push_str("    }\n");
    }
    out.push_str("}\n");
    Ok(out)
}

// ---------------------------------------------------------------------------
// Cluster C12 (`.design/basis/13-map.md` REQ-4): the bounded verified `Map<K, V>`
// wrapper emission. A Thermite `Map<K, V>` lowers to a newtype `TMap<K,V>` over a
// `vstd::vec::Vec<(K, V)>`-of-pairs backing (C6 `Vec<tuple>` + C9 `(K,V)` pair)
// with the verified `spec_dom`/`spec_contains_key`/`len`/`contains_key`/`get`/
// `insert` impl — the GROUNDED `TMapU64U64`-over-`vstd::vec::Vec<(u64,u64)>` form
// (`9 verified, 0 errors`). Materialized ONCE per `(K, V)` pair the program uses.
// ---------------------------------------------------------------------------

/// The bounded-`Map` capacity bound (`.design/basis/13-map.md` REQ-4 / the GROUNDED
/// `MAP_CAP`): the SAME `1_000_000` idiom as [`VEC_CAP`]. A `Map`'s `well_formed`
/// carries `data.len() <= MAP_CAP` so the §4.2 cage never sees an unbounded backing.
const MAP_CAP: u64 = 1_000_000;

/// Collect, in deterministic order and deduped, every `(K, V)` pair the program
/// uses in a `Map<K, V>` REACHABLE anywhere (a `fn`/`spec fn` param/return, a
/// `struct`/`enum`-variant FIELD, a `fn`-body local `let` annotation) — the SAME
/// reachability closure as [`collect_vec_elem_types`], keyed on `Type::Map(k, v)`.
/// The wrapper struct is materialized once per `(K, V)` pair.
pub(crate) fn collect_map_kv_types(program: &Program) -> Vec<(Type, Type)> {
    let mut pairs: Vec<(Type, Type)> = Vec::new();
    for item in &program.items {
        match item {
            Item::Fn(f) => {
                for p in &f.params {
                    note_map_kv(&p.ty, &mut pairs);
                }
                note_map_kv(&f.ret, &mut pairs);
                if let Some(b) = &f.body {
                    note_block_map_kv(b, &mut pairs);
                }
            }
            Item::SpecFn(s) => {
                for p in &s.params {
                    note_map_kv(&p.ty, &mut pairs);
                }
                note_map_kv(&s.ret, &mut pairs);
                note_block_map_kv(&s.body, &mut pairs);
            }
            Item::Struct(s) => {
                for fd in &s.fields {
                    note_map_kv(&fd.ty, &mut pairs);
                }
            }
            Item::Enum(e) => {
                for v in &e.variants {
                    match &v.shape {
                        VariantShape::Unit => {}
                        VariantShape::Tuple(tys) => {
                            for ty in tys {
                                note_map_kv(ty, &mut pairs);
                            }
                        }
                        VariantShape::Struct(fields) => {
                            for fd in fields {
                                note_map_kv(&fd.ty, &mut pairs);
                            }
                        }
                    }
                }
            }
        }
    }
    pairs
}

/// Note every `(K, V)` pair reachable in a single `Type` (REQ-5), deduped. Recurses
/// through every type constructor so a `Map` nested under a `Ref`/`Vec`/`Option`/
/// `Result`/`Tuple` is reached.
fn note_map_kv(ty: &Type, pairs: &mut Vec<(Type, Type)>) {
    match ty {
        Type::Map(k, v) => {
            // Recurse into the key/value first (a `Map<_, Map<_,_>>` inner pair must
            // be noted/emitted before the outer that names it), then push this pair.
            note_map_kv(k, pairs);
            note_map_kv(v, pairs);
            let pair = ((**k).clone(), (**v).clone());
            if !pairs.contains(&pair) {
                pairs.push(pair);
            }
        }
        Type::Ref { inner, .. }
        | Type::Slice(inner)
        | Type::Box(inner)
        | Type::Vec(inner)
        | Type::Generic { arg: inner, .. }
        | Type::Option(inner) => note_map_kv(inner, pairs),
        Type::Result(ok, err) => {
            note_map_kv(ok, pairs);
            note_map_kv(err, pairs);
        }
        Type::Tuple(tys) => {
            for t in tys {
                note_map_kv(t, pairs);
            }
        }
        Type::Prim(_) | Type::Unit | Type::Named(_) | Type::String => {}
    }
}

/// Note every `(K, V)` pair reachable in a body-local `let` type annotation (REQ-5,
/// the `Map::new()`-no-param reachability — the #86/REQ-11 analog). Walks nested
/// `if`/`loop` blocks, keyed on the `let`'s declared type via [`note_map_kv`].
fn note_block_map_kv(block: &Block, pairs: &mut Vec<(Type, Type)>) {
    for stmt in &block.stmts {
        note_stmt_map_kv(stmt, pairs);
    }
}

fn note_stmt_map_kv(stmt: &Stmt, pairs: &mut Vec<(Type, Type)>) {
    match stmt {
        Stmt::Let { ty, .. } => {
            if let Some(t) = ty {
                note_map_kv(t, pairs);
            }
        }
        Stmt::If { then, else_, .. } => {
            note_block_map_kv(then, pairs);
            if let Some(e) = else_ {
                note_block_map_kv(e, pairs);
            }
        }
        Stmt::Loop(l) => note_block_map_kv(&l.body, pairs),
        Stmt::Assign { .. } | Stmt::Return(_) | Stmt::Expr(_) | Stmt::Break | Stmt::Continue => {}
    }
}

/// Emit the `TMap<K,V>` wrapper struct + its verified op `impl` for every `(K, V)`
/// pair the program uses (REQ-4), in deterministic order. EMPTY when the program
/// uses no `Map` (byte-stable for the non-`Map` corpus — no regression).
fn emit_map_wrappers(program: &Program) -> Result<String, LowerError> {
    let pairs = collect_map_kv_types(program);
    if pairs.is_empty() {
        return Ok(String::new());
    }
    let mut out = String::new();
    for (k, v) in &pairs {
        out.push_str(&emit_one_map_wrapper(k, v)?);
    }
    Ok(out)
}

/// Emit ONE `TMap<K,V>` wrapper struct + its verified op `impl` for a single
/// `(K, V)` pair (`.design/basis/13-map.md` REQ-4). The emitted form is EXACTLY the
/// GROUNDED `TMapU64U64`-over-`vstd::vec::Vec<(u64,u64)>` (`9 verified, 0 errors`):
/// the `well_formed` capacity + key-uniqueness invariant, the `spec_dom`/
/// `spec_contains_key`/`len` spec abstraction view, the exec linear-scan
/// `contains_key` (`ens result == spec_contains_key(k)`), the no-OOB /
/// handled-or-loud `get -> Option<V>` (absent → `None`, NOT a wrong value), and the
/// append-under-`!contains_key` `insert` with the `final(self)` &mut postcondition.
/// NO `assume`/`external_body` (R-DEFER-9) — every contract is real verus map
/// reasoning threaded over vstd's verified `Vec::push`/`Vec::index`/`Vec::len`. v1
/// grounds Copy keys (`Map<u64,u64>`, OQ-4); a non-Copy key is the existing
/// `LowerError::Unsupported` via `tmap_name`.
fn emit_one_map_wrapper(key: &Type, val: &Type) -> Result<String, LowerError> {
    let name = tmap_name(key, val)?;
    let kty = lower_type(key)?;
    let vty = lower_type(val)?;
    if !map_key_is_copy(key) {
        // v1 grounds Copy keys only (OQ-4): a non-Copy key (`Map<String, _>`) needs
        // the borrow-comparison rule (the REQ-9 finding) not yet grounded. Refuse
        // loudly via the existing error enum — no panic, no silent wrong code.
        return Err(LowerError::Unsupported {
            what: format!(
                "Map key type {kty} (v1 grounds Copy keys — Map<u64, u64>; a non-Copy \
                 key needs the borrow-comparison rule, deferred per 13-map.md OQ-4)"
            ),
            span: zero_span(),
        });
    }
    let mut out = String::new();
    out.push('\n');
    writeln!(out, "pub struct {name} {{ pub data: Vec<({kty}, {vty})> }}").ok();
    writeln!(out, "impl {name} {{").ok();
    // The key-set abstraction `spec_dom` (the spec membership view; REQ-4). A named
    // spec fn over a `Set` comprehension with an explicit trigger (the §4.2 cage —
    // never an anonymous nested quantifier admitted raw).
    out.push_str("    pub open spec fn spec_dom(&self) -> Set<int> {\n");
    out.push_str("        Set::new(|kk: int| exists|j: int|\n");
    out.push_str(
        "            0 <= j < self.data.len() && #[trigger] self.data@[j].0 as int == kk)\n",
    );
    out.push_str("    }\n");
    // The `well_formed` capacity + key-uniqueness invariant (REQ-4): `data.len() <=
    // MAP_CAP` AND every distinct pair has a distinct key. The forall carries an
    // explicit dual-key trigger (both quantified vars covered).
    out.push_str("    pub open spec fn well_formed(&self) -> bool {\n");
    writeln!(out, "        &&& self.data.len() <= {MAP_CAP}").ok();
    out.push_str(
        "        &&& (forall|a: int, b: int| #![trigger self.data@[a].0, self.data@[b].0]\n",
    );
    out.push_str(
        "                0 <= a < self.data.len() && 0 <= b < self.data.len() && a != b\n",
    );
    out.push_str("                ==> self.data@[a].0 != self.data@[b].0)\n");
    out.push_str("    }\n");
    // The membership abstraction `spec_contains_key` (REQ-4): `exists|j| data@[j].0
    // == k`, an explicit single-var trigger.
    writeln!(
        out,
        "    pub open spec fn spec_contains_key(&self, k: {kty}) -> bool {{"
    )
    .ok();
    out.push_str(
        "        exists|j: int| 0 <= j < self.data.len() && #[trigger] self.data@[j].0 == k\n",
    );
    out.push_str("    }\n");
    out.push_str("    pub open spec fn len(&self) -> nat { self.data.len() as nat }\n");
    // The exec linear-scan `contains_key` (REQ-4): `req well_formed`, `ens result ==
    // spec_contains_key(k)`, the scan invariant + `decreases`. PURE.
    writeln!(
        out,
        "    pub fn contains_key(&self, k: {kty}) -> (result: bool)"
    )
    .ok();
    out.push_str("        requires self.well_formed(),\n");
    out.push_str("        ensures result == self.spec_contains_key(k),\n");
    out.push_str("    {\n");
    out.push_str("        let mut i: usize = 0;\n");
    out.push_str("        while i < self.data.len()\n");
    out.push_str("            invariant\n");
    out.push_str("                i <= self.data.len(),\n");
    out.push_str("                forall|j: int| 0 <= j < i ==> self.data@[j].0 != k,\n");
    out.push_str("            decreases self.data.len() - i,\n");
    out.push_str("        {\n");
    out.push_str("            if self.data[i].0 == k {\n");
    out.push_str("                assert(self.data@[i as int].0 == k);\n");
    out.push_str("                return true;\n");
    out.push_str("            }\n");
    out.push_str("            i = i + 1;\n");
    out.push_str("        }\n");
    out.push_str("        false\n");
    out.push_str("    }\n");
    // The no-OOB / handled-or-loud `get -> Option<V>` (REQ-4): `req well_formed`,
    // `ens match result { Some(v) => contains_key(k) && (the pair exists), None =>
    // !contains_key(k) }` — an ABSENT key returns None, NOT a wrong value (the C7
    // Option, the absent → None refusal). PURE. v1 grounds a Copy value (by value).
    writeln!(
        out,
        "    pub fn get(&self, k: {kty}) -> (result: Option<{vty}>)"
    )
    .ok();
    out.push_str("        requires self.well_formed(),\n");
    out.push_str("        ensures match result {\n");
    out.push_str("            Some(v) => self.spec_contains_key(k)\n");
    out.push_str("                && (exists|j: int| 0 <= j < self.data.len()\n");
    out.push_str("                       && self.data@[j].0 == k && self.data@[j].1 == v),\n");
    out.push_str("            None => !self.spec_contains_key(k),\n");
    out.push_str("        },\n");
    out.push_str("    {\n");
    out.push_str("        let mut i: usize = 0;\n");
    out.push_str("        while i < self.data.len()\n");
    out.push_str("            invariant\n");
    out.push_str("                i <= self.data.len(),\n");
    out.push_str("                forall|j: int| 0 <= j < i ==> self.data@[j].0 != k,\n");
    out.push_str("            decreases self.data.len() - i,\n");
    out.push_str("        {\n");
    out.push_str("            if self.data[i].0 == k {\n");
    writeln!(out, "                let v: {vty} = self.data[i].1;").ok();
    out.push_str(
        "                assert(self.data@[i as int].0 == k && self.data@[i as int].1 == v);\n",
    );
    out.push_str("                return Some(v);\n");
    out.push_str("            }\n");
    out.push_str("            i = i + 1;\n");
    out.push_str("        }\n");
    out.push_str("        None\n");
    out.push_str("    }\n");
    // The append-under-`!contains_key` `insert` (REQ-4, OQ-2 the v1 form): `req
    // well_formed && len < MAP_CAP && !contains_key(k)`, `ens final(self)...` (the
    // `final(self)` &mut postcondition — the SHIPPED Vec::push grounding finding) —
    // the new pair maps `k -> v`, capacity + uniqueness preserved, `len' == len + 1`.
    // Carries `fx alloc` at the surface (the Vec-push / Effect::Alloc rule). The
    // proof: the new pair witnesses the membership; uniqueness holds because the new
    // key was absent by precondition.
    writeln!(out, "    pub fn insert(&mut self, k: {kty}, v: {vty})").ok();
    out.push_str("        requires old(self).well_formed(), old(self).data.len() < ");
    writeln!(out, "{MAP_CAP},").ok();
    out.push_str("                 !old(self).spec_contains_key(k),\n");
    out.push_str("        ensures\n");
    out.push_str("            final(self).well_formed(),\n");
    out.push_str("            final(self).spec_contains_key(k),\n");
    out.push_str("            exists|j: int| 0 <= j < final(self).data.len()\n");
    out.push_str(
        "                && final(self).data@[j].0 == k && final(self).data@[j].1 == v,\n",
    );
    out.push_str("            final(self).data.len() == old(self).data.len() + 1,\n");
    out.push_str("    {\n");
    out.push_str("        let ghost old_len = self.data.len();\n");
    out.push_str("        self.data.push((k, v));\n");
    out.push_str(
        "        assert(self.data@[old_len as int].0 == k && self.data@[old_len as int].1 == v);\n",
    );
    out.push_str("        assert(self.spec_contains_key(k)) by {\n");
    out.push_str(
        "            assert(0 <= old_len < self.data.len() && self.data@[old_len as int].0 == k);\n",
    );
    out.push_str("        }\n");
    out.push_str("        assert(self.well_formed()) by {\n");
    out.push_str("            assert forall|a: int, b: int|\n");
    out.push_str(
        "                0 <= a < self.data.len() && 0 <= b < self.data.len() && a != b\n",
    );
    out.push_str("                implies self.data@[a].0 != self.data@[b].0 by {\n");
    out.push_str("                if a < old_len && b < old_len {\n");
    out.push_str("                } else if a == old_len {\n");
    out.push_str("                    assert(self.data@[b].0 != k);\n");
    out.push_str("                } else if b == old_len {\n");
    out.push_str("                    assert(self.data@[a].0 != k);\n");
    out.push_str("                }\n");
    out.push_str("            }\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    Ok(out)
}

// ---------------------------------------------------------------------------
// Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): the bounded-`String`
// wrapper emission. A Thermite `String` lowers to a newtype `TString` over
// `vstd::vec::Vec<u8>` with the verified `well_formed`/`len`/`byte_at`/`concat`/
// `slice` impl — the GROUNDED `TString`-over-`Vec<u8>` form. Materialized ONCE
// when the program references `String` (the element type is FIXED to `u8`, so —
// unlike the per-element `Vec` wrapper — there is exactly one `TString`).
// ---------------------------------------------------------------------------

/// True if the `String` type (`Type::String`) is REACHABLE anywhere in `ty` —
/// directly, or nested under a `Ref`/`Slice`/`Vec`/`Box`/`Generic` constructor
/// (a `&String` view, a `Vec<String>`, a `Box<String>`). The whole type-constructor
/// closure is walked so no String-bearing type position is missed (REQ-4).
/// True if `ty` is a `String` VALUE param — a bare `String` or a `&String` borrow
/// (the `str`-view role), but NOT a `Vec<String>`/`Box<String>`/struct field (those
/// carry their OWN wrapper invariant, named differently). Used to weave the `TString`
/// `well_formed()` precondition for a `String`-receiver/needle param of a C5
/// search/transform fn (`.design/basis/07-strings.md` REQ-13..16, issue #102): the
/// emitted method requires `self.well_formed()`/`p.well_formed()`, and a `String`
/// value is bounded by that invariant (the §4.2 cage), so the caller discharges it.
fn is_string_param_ty(ty: &Type) -> bool {
    match ty {
        Type::String => true,
        Type::Ref { inner, .. } => matches!(inner.as_ref(), Type::String),
        _ => false,
    }
}

/// True iff `ty` is a `Map<K, V>` VALUE param — a bare `Map` or a `&Map`/`&mut Map`
/// borrow (`.design/basis/13-map.md` REQ-4). Used to weave the `TMap`
/// `well_formed()` precondition for a `Map`-receiver param of a fn calling
/// `contains_key`/`get`/`insert` (those methods `require self.well_formed()`), the
/// SAME automatic threading [`is_string_param_ty`] gives a `String` param. Sees
/// through one borrow only (a `&&Map` is not an invariant-bearing receiver).
fn is_map_param_ty(ty: &Type) -> bool {
    match ty {
        Type::Map(_, _) => true,
        Type::Ref { inner, .. } => matches!(inner.as_ref(), Type::Map(_, _)),
        _ => false,
    }
}

/// The named-type of a param, SEEING THROUGH a single `&` borrow (REQ-8 automatic
/// threading, blocker #105): `Buffer` and `&Buffer` both yield `Some("Buffer")` so
/// the invariant-bearing-struct `well_formed()` weave fires for a borrowed receiver
/// exactly as for an owned one. Mirrors how `is_string_param_ty` sees through `Ref`
/// for the C5 String weave — the type invariant is a property of the TYPE, implicit
/// at every use whether the value is owned or borrowed. NOT a deref chain (a `&&T`
/// or `&Box<T>` is not an invariant-bearing struct receiver) — one borrow only.
fn named_struct_param(ty: &Type) -> Option<&str> {
    match ty {
        Type::Named(name) => Some(name.as_str()),
        Type::Ref { inner, .. } => match inner.as_ref() {
            Type::Named(name) => Some(name.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn ty_reaches_string(ty: &Type) -> bool {
    match ty {
        Type::String => true,
        Type::Ref { inner, .. }
        | Type::Slice(inner)
        | Type::Vec(inner)
        | Type::Box(inner)
        | Type::Generic { arg: inner, .. }
        // Cluster C7 (`.design/basis/09-option-result.md` REQ-4): a `String` nested
        // under an `Option<String>` is reached through the option's element type.
        | Type::Option(inner) => ty_reaches_string(inner),
        // A `Result<T, E>` reaches a `String` in EITHER type argument.
        Type::Result(ok, err) => ty_reaches_string(ok) || ty_reaches_string(err),
        // Cluster C12 (`.design/basis/13-map.md` REQ-5): a `Map<K, V>` reaches a
        // `String` in EITHER type argument (a `Map<String, _>` key or a
        // `Map<_, String>` value), so both are recursed (exactly as `Result`'s two
        // arguments are) — the `TString` wrapper must be in scope for the
        // Vec-of-pairs backing.
        Type::Map(k, v) => ty_reaches_string(k) || ty_reaches_string(v),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): a
        // tuple type reaches a `String` if ANY element does.
        Type::Tuple(tys) => tys.iter().any(ty_reaches_string),
        Type::Prim(_) | Type::Unit | Type::Named(_) => false,
    }
}

/// True if the program references the `String` type in ANY reachable type
/// position — a `fn`/`spec fn` parameter or return, a `struct`/`enum`-variant
/// FIELD, or a `fn`-body local `let` annotation — OR uses a string literal
/// anywhere (REQ-4). Every such position needs the `TString` wrapper in scope (a
/// struct field `text: String` lowers to `pub text: TString`; a literal
/// materializes a `TString`). The wrapper is emitted once iff this holds (EMPTY
/// otherwise — byte-stable for the non-`String` corpus).
///
/// CRITICAL for the per-item sub-program weave (forge `#86`): a `forge check`
/// per-item sub-program may be a STRUCT decl alone (`struct Buf { text: String,
/// cursor: u64 }`) whose only `String` reference is a FIELD type — so the struct
/// and enum field arms below are load-bearing, not a `continue`. Mirrors the way
/// `reachable_adt_deps` weaves the struct decls a String-bearing item reaches.
fn program_uses_string(program: &Program) -> bool {
    for item in &program.items {
        match item {
            Item::Fn(f) => {
                if f.params.iter().any(|p| ty_reaches_string(&p.ty)) || ty_reaches_string(&f.ret) {
                    return true;
                }
                if let Some(b) = &f.body {
                    if block_has_str_lit(b) || block_has_string_local(b) {
                        return true;
                    }
                }
            }
            Item::SpecFn(s) => {
                if s.params.iter().any(|p| ty_reaches_string(&p.ty))
                    || ty_reaches_string(&s.ret)
                    || block_has_str_lit(&s.body)
                    || block_has_string_local(&s.body)
                {
                    return true;
                }
            }
            Item::Struct(s) => {
                if s.fields.iter().any(|fd| ty_reaches_string(&fd.ty)) {
                    return true;
                }
            }
            Item::Enum(e) => {
                if e.variants.iter().any(variant_reaches_string) {
                    return true;
                }
            }
        }
    }
    false
}

/// True if any field/payload type of an enum variant reaches `String` (REQ-4).
fn variant_reaches_string(v: &thermite_syntax::ast::VariantDef) -> bool {
    match &v.shape {
        thermite_syntax::ast::VariantShape::Unit => false,
        thermite_syntax::ast::VariantShape::Tuple(tys) => tys.iter().any(ty_reaches_string),
        thermite_syntax::ast::VariantShape::Struct(fields) => {
            fields.iter().any(|fd| ty_reaches_string(&fd.ty))
        }
    }
}

/// True if a block contains a `let` whose type annotation reaches `String`
/// (REQ-4) — a `let s: String = …` local needs the `TString` wrapper even when no
/// param/return/field is typed `String`. Walks nested `if`/`loop` blocks.
fn block_has_string_local(block: &Block) -> bool {
    block.stmts.iter().any(stmt_has_string_local)
}

fn stmt_has_string_local(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { ty, .. } => ty.as_ref().map(ty_reaches_string).unwrap_or(false),
        Stmt::If { then, else_, .. } => {
            block_has_string_local(then)
                || else_.as_ref().map(block_has_string_local).unwrap_or(false)
        }
        Stmt::Loop(l) => block_has_string_local(&l.body),
        // break/continue declare no local (#93): no string-typed binding.
        Stmt::Assign { .. } | Stmt::Return(_) | Stmt::Expr(_) | Stmt::Break | Stmt::Continue => {
            false
        }
    }
}

/// True if a block contains a string-literal expression anywhere (REQ-1) — a
/// literal materializes a `TString`, so the wrapper must be emitted even when no
/// parameter/return is typed `String` (e.g. `literal_len()`'s `"hello".len()`).
fn block_has_str_lit(block: &Block) -> bool {
    block.stmts.iter().any(stmt_has_str_lit)
        || block.tail.as_deref().map(expr_has_str_lit).unwrap_or(false)
}

fn stmt_has_str_lit(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { init, .. } => expr_has_str_lit(init),
        Stmt::Assign { target, value } => expr_has_str_lit(target) || expr_has_str_lit(value),
        Stmt::Return(opt) => opt.as_ref().map(expr_has_str_lit).unwrap_or(false),
        Stmt::If {
            cond, then, else_, ..
        } => {
            expr_has_str_lit(cond)
                || block_has_str_lit(then)
                || else_.as_ref().map(block_has_str_lit).unwrap_or(false)
        }
        Stmt::Loop(l) => block_has_str_lit(&l.body),
        Stmt::Expr(e) => expr_has_str_lit(e),
        // break/continue carry no sub-expression (#93): no string literal.
        Stmt::Break | Stmt::Continue => false,
    }
}

/// True if a string literal appears anywhere in `expr` (a full-tree walk reusing
/// `each_subexpr`'s structural cases). REQ-1.
fn expr_has_str_lit(expr: &Expr) -> bool {
    if matches!(expr, Expr::StrLit(_)) {
        return true;
    }
    let mut found = false;
    let _ = each_subexpr(expr, &mut |e| {
        if expr_has_str_lit(e) {
            found = true;
        }
        Ok(())
    });
    found
}

/// Emit the `TString` wrapper struct + its verified `well_formed`/`spec_len`/
/// `len`/`spec_byte_at`/`byte_at`/`concat`/`slice` impl when the program uses
/// `String` (REQ-4), in deterministic order. EMPTY otherwise. The emitted form is
/// EXACTLY the GROUNDED `TString` over `vstd::vec::Vec<u8>` (`verified, 0
/// errors`):
///
/// ```verus
/// pub struct TString { pub data: Vec<u8> }
/// impl TString {
///     pub open spec fn well_formed(&self) -> bool { self.data.len() <= 1000000 }
///     pub open spec fn spec_len(&self) -> nat { self.data.len() as nat }
///     pub fn len(&self) -> (result: u64) ensures result == self.data.len(),
///         { self.data.len() as u64 }
///     pub open spec fn spec_byte_at(&self, i: int) -> u64 { self.data@[i] as u64 }
///     pub fn byte_at(&self, i: usize) -> (result: u64)
///         requires i < self.data.len(), ensures result == self.data@[i as int],
///         { self.data[i] as u64 }
///     pub fn concat(&self, b: TString) -> (result: TString) { … two-loop append … }
///     pub fn slice(&self, lo: usize, hi: usize) -> (result: TString) { … bounded copy … }
/// }
/// ```
///
/// THE BYTE CHAR MODEL (REQ-2): `byte_at` returns `u64` (the corpus oracle's
/// `first_byte -> u64` shape; a byte zero-extends into `u64`); the exec `len`
/// returns `u64` (the corpus `greeting_len -> u64`) while the SPEC fn is `spec_len`
/// (a contract names `spec_len`, the exec `len` cannot be named in a contract). The
/// no-OOB `byte_at` (`req i < self.data.len()`) is the editor's core safety — the
/// unguarded form FAILS verus (`0 verified, 1 errors`, the L0 demonstration,
/// R-DEFER-9). `concat`/`slice` carry the bounded length identity; `slice` requires
/// `self.well_formed()` so the copied run stays `<= CAP`. NO `assume`/`external_body`
/// (R-DEFER-9) — every Thermite-level contract is threaded over vstd's verified
/// `Vec<u8>::push`/`index`/`len` (which carry the heap proof).
fn emit_string_wrapper(program: &Program) -> Result<String, LowerError> {
    if !program_uses_string(program) {
        return Ok(String::new());
    }
    let cap = VEC_CAP;
    let mut out = String::new();
    out.push('\n');
    out.push_str("pub struct TString { pub data: Vec<u8> }\n");
    out.push_str("impl TString {\n");
    writeln!(
        out,
        "    pub open spec fn well_formed(&self) -> bool {{ self.data.len() <= {cap} }}"
    )
    .ok();
    out.push_str("    pub open spec fn spec_len(&self) -> nat { self.data.len() as nat }\n");
    out.push_str("    pub fn len(&self) -> (result: u64)\n");
    out.push_str("        ensures result == self.data.len(),\n");
    out.push_str("    { self.data.len() as u64 }\n");
    out.push_str(
        "    pub open spec fn spec_byte_at(&self, i: int) -> u64 { self.data@[i] as u64 }\n",
    );
    // The no-OOB exec accessor `byte_at` (REQ-4): `req i < len`, `ens result ==
    // self.data@[i as int]`. The verified vstd index `self.data[i]` zero-extended
    // to `u64` (the corpus `first_byte -> u64`).
    out.push_str("    pub fn byte_at(&self, i: usize) -> (result: u64)\n");
    out.push_str("        requires i < self.data.len(),\n");
    out.push_str("        ensures result == self.data@[i as int],\n");
    out.push_str("    { self.data[i] as u64 }\n");
    // The bounded constructing `concat` (REQ-4): a two-loop append, `req
    // self.well_formed() && b.well_formed() && len_a + len_b <= CAP`, `ens
    // result.well_formed() && result.len() == len_a + len_b`. `b` is by value to
    // match the corpus `a.concat(b)` (no `&` insertion needed). An owned-value
    // construction — no `&mut`/`final(self)` (the result is a fresh value).
    out.push_str("    pub fn concat(&self, b: TString) -> (result: TString)\n");
    out.push_str("        requires self.well_formed(), b.well_formed(),\n");
    writeln!(
        out,
        "                 self.data.len() + b.data.len() <= {cap},"
    )
    .ok();
    out.push_str("        ensures result.well_formed(),\n");
    out.push_str("                result.data.len() == self.data.len() + b.data.len(),\n");
    out.push_str("    {\n");
    out.push_str("        let mut out: Vec<u8> = Vec::new();\n");
    out.push_str("        let mut i: usize = 0;\n");
    out.push_str("        while i < self.data.len()\n");
    out.push_str("            invariant i <= self.data.len(), out.len() == i,\n");
    writeln!(
        out,
        "                      self.data.len() + b.data.len() <= {cap},"
    )
    .ok();
    out.push_str("            decreases self.data.len() - i,\n");
    out.push_str("        { out.push(self.data[i]); i = i + 1; }\n");
    out.push_str("        let mut j: usize = 0;\n");
    out.push_str("        while j < b.data.len()\n");
    out.push_str("            invariant j <= b.data.len(), out.len() == self.data.len() + j,\n");
    writeln!(
        out,
        "                      self.data.len() + b.data.len() <= {cap},"
    )
    .ok();
    out.push_str("            decreases b.data.len() - j,\n");
    out.push_str("        { out.push(b.data[j]); j = j + 1; }\n");
    out.push_str("        TString { data: out }\n");
    out.push_str("    }\n");
    // The bounded substring `slice` (REQ-4): a bounded copy, `req self.well_formed()
    // && lo <= hi && hi <= len`, `ens result.well_formed() && result.len() == hi -
    // lo`. The owned-copy form (OQ-4 RESOLVED — owned, not a borrowed view, so no
    // region/lifetime reasoning §4.4 defers). `self.well_formed()` keeps the copied
    // run <= CAP (the invariant carries the CAP bound).
    out.push_str("    pub fn slice(&self, lo: usize, hi: usize) -> (result: TString)\n");
    out.push_str("        requires self.well_formed(), lo <= hi, hi <= self.data.len(),\n");
    out.push_str("        ensures result.well_formed(), result.data.len() == hi - lo,\n");
    out.push_str("    {\n");
    out.push_str("        let mut out: Vec<u8> = Vec::new();\n");
    out.push_str("        let mut i: usize = lo;\n");
    out.push_str("        while i < hi\n");
    writeln!(
        out,
        "            invariant lo <= i, i <= hi, hi <= self.data.len(), self.data.len() <= {cap}, out.len() == i - lo,"
    )
    .ok();
    out.push_str("            decreases hi - i,\n");
    out.push_str("        { out.push(self.data[i]); i = i + 1; }\n");
    out.push_str("        TString { data: out }\n");
    out.push_str("    }\n");
    // Cluster C4 (`.design/basis/07-strings.md` REQ-7, issue #94): the verified
    // byte-builder. `from_byte(b)` builds a 1-byte `String`; `push_byte(b)` appends
    // one byte returning a FRESH owned `String` (the `&self`/owned-result form — no
    // `&mut`/`final(self)`, consistent with `concat`'s owned result). The surface
    // byte is a `u64` (the SAME zero-extended convention as `byte_at -> u64`), cast
    // to the `u8` backing element. GROUNDED `verified, 0 errors` (the copy loop with
    // the element-frame invariant `forall|j| 0 <= j < i ==> out@[j] == self.data@[j]`
    // + the new-byte placement `result@[old_len] == b`). All constructing (`fx alloc`).
    out.push_str("    pub fn from_byte(b: u64) -> (result: TString)\n");
    out.push_str("        ensures result.well_formed(), result.data.len() == 1,\n");
    out.push_str("                result.data@[0] == b as u8,\n");
    out.push_str("    {\n");
    out.push_str("        let mut data: Vec<u8> = Vec::new();\n");
    out.push_str("        data.push(b as u8);\n");
    out.push_str("        TString { data }\n");
    out.push_str("    }\n");
    out.push_str("    pub fn push_byte(&self, b: u64) -> (result: TString)\n");
    writeln!(
        out,
        "        requires self.well_formed(), self.data.len() < {cap},"
    )
    .ok();
    out.push_str("        ensures result.well_formed(),\n");
    out.push_str("                result.data.len() == self.data.len() + 1,\n");
    out.push_str("                result.data@[self.data.len() as int] == b as u8,\n");
    out.push_str("                forall|j: int| 0 <= j < self.data.len()\n");
    out.push_str("                    ==> result.data@[j] == self.data@[j],\n");
    out.push_str("    {\n");
    out.push_str("        let mut out: Vec<u8> = Vec::new();\n");
    out.push_str("        let mut i: usize = 0;\n");
    out.push_str("        while i < self.data.len()\n");
    out.push_str("            invariant i <= self.data.len(), out.len() == i,\n");
    writeln!(out, "                      self.data.len() < {cap},").ok();
    out.push_str(
        "                      forall|j: int| 0 <= j < i ==> #[trigger] out@[j] == self.data@[j],\n",
    );
    out.push_str("            decreases self.data.len() - i,\n");
    out.push_str("        { out.push(self.data[i]); i = i + 1; }\n");
    out.push_str("        out.push(b as u8);\n");
    out.push_str("        TString { data: out }\n");
    out.push_str("    }\n");
    // Cluster C5 (`.design/basis/07-strings.md` REQ-13..16, issue #102): the string
    // search/transform ops. Emitted only when the program uses a C5 op
    // (`program_uses_string_search`) so the non-C5 corpus is byte-unaffected (no
    // regression). The GROUNDED forms (`verus 0.2026.05.24`, no `assume`/`admit`/
    // `external_body` — R-DEFER-9): the predicate scans `14 verified, 0 errors`,
    // `split` `7 verified, 0 errors`, `trim` `8 verified, 0 errors`.
    if program_uses_string_search(program) {
        // Blocker #130: the search methods INTER-CALL the generated free fns
        // (`occurs_at`/`contains_sub`/`count_sep`/`sep_free`/`lemma_count_push`), so
        // build them into a local buffer, rewrite those references to the reserved
        // names, then append — matching the reserved-named defs (`emit_string_search_
        // defs`). The METHOD names themselves (`matches_at`/`split`/…) are inherent
        // (namespaced under the impl), so they are not reserved.
        let mut methods = String::new();
        emit_string_search_methods(&mut methods, cap);
        out.push_str(&reserve_generated_names(&methods));
    }
    out.push_str("}\n");
    Ok(out)
}

/// Emit the C5 string search/transform METHODS onto the open `TString` impl
/// (`.design/basis/07-strings.md` REQ-13..16, issue #102). Appended inside
/// `emit_string_wrapper`'s impl block (the open brace is still pending). The
/// contracts NAME the seeded spec fns `occurs_at`/`contains_sub`/`count_sep`/
/// `sep_free` (emitted at module scope by `emit_string_search_defs`); verus resolves
/// them order-independently within the single `verus!` block. The exact GROUNDED
/// forms — the inner `matches_at` helper + the byte-scan predicates (REQ-13), the
/// `find -> Option<u64>` occurrence scan (REQ-14, the C7 spec-`match`-in-`ens`), the
/// `split -> TVecTString` push-loop (REQ-15, reusing C6's `TVecTString`), and the
/// `trim -> TString` whitespace scan + bounded copy (REQ-16). `cap` is the §4.2
/// capacity bound (`VEC_CAP`). No `unwrap`/`expect`/`panic!` (R-CODE-2), no proof
/// cheat (the scans + split + the lemma are PROVED).
fn emit_string_search_methods(out: &mut String, cap: u64) {
    // The inner occurrence helper: does `p` occur at byte offset `at`? A scan over
    // `p`'s bytes with the prefix-match invariant; the `at <= len - plen` form keeps
    // the `at + plen` precondition from overflowing `usize` (the GROUNDED form).
    // Consumed by `starts_with`/`ends_with`/`contains`/`find`.
    out.push_str("    pub fn matches_at(&self, p: &TString, at: usize) -> (result: bool)\n");
    out.push_str("        requires self.well_formed(), p.well_formed(),\n");
    out.push_str("                 p.data.len() <= self.data.len(),\n");
    out.push_str("                 at <= self.data.len() - p.data.len(),\n");
    out.push_str("        ensures result == occurs_at(self.data@, p.data@, at as int),\n");
    out.push_str("    {\n");
    out.push_str("        let mut k: usize = 0;\n");
    out.push_str("        while k < p.data.len()\n");
    out.push_str("            invariant\n");
    out.push_str("                k <= p.data.len(),\n");
    out.push_str("                p.data.len() <= self.data.len(),\n");
    out.push_str("                at <= self.data.len() - p.data.len(),\n");
    out.push_str(
        "                forall|j: int| 0 <= j < k ==> self.data@[at + j] == p.data@[j],\n",
    );
    out.push_str("            decreases p.data.len() - k,\n");
    out.push_str("        {\n");
    out.push_str("            if self.data[at + k] != p.data[k] {\n");
    out.push_str("                assert(self.data@[at + k as int] != p.data@[k as int]);\n");
    out.push_str("                return false;\n");
    out.push_str("            }\n");
    out.push_str("            k = k + 1;\n");
    out.push_str("        }\n");
    out.push_str("        true\n");
    out.push_str("    }\n");
    // starts_with: `occurs_at(s@, needle@, 0)` (REQ-13). The empty-needle / oversized-
    // needle guard returns false BEFORE calling `matches_at` (its `req` would not hold).
    out.push_str("    pub fn starts_with(&self, p: &TString) -> (result: bool)\n");
    out.push_str("        requires self.well_formed(), p.well_formed(),\n");
    out.push_str("        ensures result == occurs_at(self.data@, p.data@, 0),\n");
    out.push_str("    {\n");
    out.push_str("        if p.data.len() > self.data.len() { return false; }\n");
    out.push_str("        self.matches_at(p, 0)\n");
    out.push_str("    }\n");
    // ends_with: `occurs_at(s@, needle@, (len - needle.len()))` (REQ-13).
    out.push_str("    pub fn ends_with(&self, p: &TString) -> (result: bool)\n");
    out.push_str("        requires self.well_formed(), p.well_formed(),\n");
    out.push_str(
        "        ensures result == occurs_at(self.data@, p.data@, (self.data.len() - p.data.len()) as int),\n",
    );
    out.push_str("    {\n");
    out.push_str("        if p.data.len() > self.data.len() { return false; }\n");
    out.push_str("        let off: usize = self.data.len() - p.data.len();\n");
    out.push_str("        self.matches_at(p, off)\n");
    out.push_str("    }\n");
    // contains: `contains_sub(s@, needle@)` — the outer occurrence-position scan
    // calling `matches_at`, with the no-match-so-far invariant + the
    // `assert forall .. !occurs_at .. by` blocks that prove `!contains_sub` on the
    // no-match exits (REQ-13). RECEIVER-TYPE-dispatched: this is `TString::contains`;
    // the C6 `TVec::contains` (membership) is a DISTINCT inherent method — Rust keys
    // method resolution on the receiver type, so the shared surface name `contains`
    // does not clobber (the design-flagged name-clash, RESOLVED at this layer).
    out.push_str("    pub fn contains(&self, p: &TString) -> (result: bool)\n");
    out.push_str("        requires self.well_formed(), p.well_formed(),\n");
    out.push_str("        ensures result == contains_sub(self.data@, p.data@),\n");
    out.push_str("    {\n");
    out.push_str("        if p.data.len() > self.data.len() {\n");
    out.push_str("            assert forall|at: int| !occurs_at(self.data@, p.data@, at) by {\n");
    out.push_str("                if 0 <= at && at + p.data.len() <= self.data.len() { }\n");
    out.push_str("            }\n");
    out.push_str("            return false;\n");
    out.push_str("        }\n");
    out.push_str("        let last: usize = self.data.len() - p.data.len();\n");
    out.push_str("        let mut at: usize = 0;\n");
    out.push_str("        while at <= last\n");
    out.push_str("            invariant\n");
    out.push_str("                self.well_formed(), p.well_formed(),\n");
    out.push_str("                p.data.len() <= self.data.len(),\n");
    out.push_str("                last == self.data.len() - p.data.len(),\n");
    out.push_str("                at <= last + 1,\n");
    out.push_str("                last + p.data.len() == self.data.len(),\n");
    out.push_str(
        "                forall|j: int| 0 <= j < at ==> !occurs_at(self.data@, p.data@, j),\n",
    );
    out.push_str("            decreases last + 1 - at,\n");
    out.push_str("        {\n");
    out.push_str("            assert(at <= self.data.len() - p.data.len());\n");
    out.push_str("            if self.matches_at(p, at) {\n");
    out.push_str("                assert(occurs_at(self.data@, p.data@, at as int));\n");
    out.push_str("                return true;\n");
    out.push_str("            }\n");
    out.push_str("            at = at + 1;\n");
    out.push_str("        }\n");
    out.push_str("        assert forall|j: int| !occurs_at(self.data@, p.data@, j) by {\n");
    out.push_str("            if 0 <= j && j + p.data.len() <= self.data.len() {\n");
    out.push_str("                assert(j <= last);\n");
    out.push_str("                assert(j < at);\n");
    out.push_str("            }\n");
    out.push_str("        }\n");
    out.push_str("        false\n");
    out.push_str("    }\n");
    // find -> Option<u64>: the SAME outer scan as `contains`, returning `Some(at)` on
    // the first hit (REQ-14, reuses C7's `Option`). The `Some` arm carries the honest
    // bound `at + plen <= slen` (NOT `at < slen`, false for an empty needle at
    // `at == len`); the `None` arm proves `!contains_sub` (the handled-or-loud tooth).
    out.push_str("    pub fn find(&self, p: &TString) -> (result: Option<u64>)\n");
    out.push_str("        requires self.well_formed(), p.well_formed(),\n");
    out.push_str("        ensures match result {\n");
    out.push_str(
        "            Some(at) => at + p.data.len() <= self.data.len() && occurs_at(self.data@, p.data@, at as int),\n",
    );
    out.push_str("            None => !contains_sub(self.data@, p.data@),\n");
    out.push_str("        },\n");
    out.push_str("    {\n");
    out.push_str("        if p.data.len() > self.data.len() {\n");
    out.push_str("            assert forall|at: int| !occurs_at(self.data@, p.data@, at) by {\n");
    out.push_str("                if 0 <= at && at + p.data.len() <= self.data.len() { }\n");
    out.push_str("            }\n");
    out.push_str("            return None;\n");
    out.push_str("        }\n");
    out.push_str("        let last: usize = self.data.len() - p.data.len();\n");
    out.push_str("        let mut at: usize = 0;\n");
    out.push_str("        while at <= last\n");
    out.push_str("            invariant\n");
    out.push_str("                self.well_formed(), p.well_formed(),\n");
    out.push_str("                p.data.len() <= self.data.len(),\n");
    out.push_str("                last == self.data.len() - p.data.len(),\n");
    out.push_str("                at <= last + 1,\n");
    out.push_str("                last + p.data.len() == self.data.len(),\n");
    out.push_str(
        "                forall|j: int| 0 <= j < at ==> !occurs_at(self.data@, p.data@, j),\n",
    );
    out.push_str("            decreases last + 1 - at,\n");
    out.push_str("        {\n");
    out.push_str("            assert(at <= self.data.len() - p.data.len());\n");
    out.push_str("            if self.matches_at(p, at) {\n");
    out.push_str("                assert(occurs_at(self.data@, p.data@, at as int));\n");
    out.push_str("                return Some(at as u64);\n");
    out.push_str("            }\n");
    out.push_str("            at = at + 1;\n");
    out.push_str("        }\n");
    out.push_str("        assert forall|j: int| !occurs_at(self.data@, p.data@, j) by {\n");
    out.push_str("            if 0 <= j && j + p.data.len() <= self.data.len() {\n");
    out.push_str("                assert(j <= last);\n");
    out.push_str("                assert(j < at);\n");
    out.push_str("            }\n");
    out.push_str("        }\n");
    out.push_str("        None\n");
    out.push_str("    }\n");
    // split -> Vec<String> (TVecTString): the parser core (REQ-15). The scan builds
    // `cur: Vec<u8>`; on a `sep` byte it pushes `TString { data: cur }` into the
    // `pieces: Vec<TString>` and resets `cur`; otherwise pushes the byte onto `cur`;
    // after the loop pushes the trailing `cur` (the `+1`). The loop invariant carries
    // the count partial (`pieces.len() == count_sep(prefix)`, maintained by
    // `lemma_count_push`), `sep_free(cur@)`, and every completed piece sep-free. The
    // surface `sep` is a `u64` (the `byte_at -> u64` convention); the exec param is a
    // `u8` (the backing element), so the lowerer casts the call arg `as u8`.
    out.push_str("    pub fn split(&self, sep: u8) -> (result: TVecTString)\n");
    out.push_str("        requires self.well_formed(),\n");
    out.push_str("        ensures\n");
    out.push_str("            result.data.len() >= 1,\n");
    out.push_str("            result.data.len() == 1 + count_sep(self.data@, sep),\n");
    out.push_str(
        "            forall|k: int| 0 <= k < result.data.len() ==> sep_free(#[trigger] result.data@[k].data@, sep),\n",
    );
    out.push_str("    {\n");
    out.push_str("        let mut pieces: Vec<TString> = Vec::new();\n");
    out.push_str("        let mut cur: Vec<u8> = Vec::new();\n");
    out.push_str("        let mut i: usize = 0;\n");
    out.push_str("        while i < self.data.len()\n");
    out.push_str("            invariant\n");
    out.push_str("                i <= self.data.len(),\n");
    out.push_str(
        "                pieces.len() == count_sep(self.data@.subrange(0, i as int), sep),\n",
    );
    out.push_str("                sep_free(cur@, sep),\n");
    out.push_str(
        "                forall|k: int| 0 <= k < pieces.len() ==> sep_free(#[trigger] pieces@[k].data@, sep),\n",
    );
    out.push_str("            decreases self.data.len() - i,\n");
    out.push_str("        {\n");
    out.push_str("            let b: u8 = self.data[i];\n");
    out.push_str("            let ghost old_pref = self.data@.subrange(0, i as int);\n");
    out.push_str("            proof {\n");
    out.push_str(
        "                assert(self.data@.subrange(0, (i + 1) as int) =~= old_pref.push(b));\n",
    );
    out.push_str("                lemma_count_push(old_pref, b, sep);\n");
    out.push_str("            }\n");
    out.push_str("            if b == sep {\n");
    out.push_str("                let piece = TString { data: cur };\n");
    out.push_str("                pieces.push(piece);\n");
    out.push_str("                cur = Vec::new();\n");
    out.push_str("                assert(sep_free(cur@, sep));\n");
    out.push_str("            } else {\n");
    out.push_str("                let ghost old_cur = cur@;\n");
    out.push_str("                cur.push(b);\n");
    out.push_str("                assert(cur@ =~= old_cur.push(b));\n");
    out.push_str(
        "                assert forall|j: int| 0 <= j < cur@.len() implies #[trigger] cur@[j] != sep by {\n",
    );
    out.push_str("                    if j < old_cur.len() { } else { assert(cur@[j] == b); }\n");
    out.push_str("                }\n");
    out.push_str("            }\n");
    out.push_str("            i = i + 1;\n");
    out.push_str("        }\n");
    out.push_str("        assert(self.data@.subrange(0, i as int) =~= self.data@);\n");
    out.push_str("        let final_piece = TString { data: cur };\n");
    out.push_str("        pieces.push(final_piece);\n");
    out.push_str("        TVecTString { data: pieces }\n");
    out.push_str("    }\n");
    // trim -> String: scan `lo` forward past leading whitespace, `hi` (exclusive)
    // backward past trailing whitespace, then copy `[lo, hi)` into a fresh `Vec<u8>`
    // with the subrange invariant `out@ == self.data@.subrange(lo, i)` (REQ-16). The
    // whitespace test is inlined in the exec loop condition (space/`\t`/`\n`/`\r` ==
    // 32/9/10/13) since `is_space` is a spec fn (not callable in exec position). The
    // content relation (`exists|lo,hi| result == s.subrange(lo,hi)`) pins the trimmed
    // bytes are a contiguous slice of the source. Constructing (`fx alloc`).
    out.push_str("    pub fn trim(&self) -> (result: TString)\n");
    out.push_str("        requires self.well_formed(),\n");
    out.push_str("        ensures\n");
    out.push_str("            result.well_formed(),\n");
    out.push_str("            result.data.len() <= self.data.len(),\n");
    out.push_str("            exists|lo: int, hi: int|\n");
    out.push_str(
        "                0 <= lo <= hi <= self.data.len() && result.data@ == self.data@.subrange(lo, hi),\n",
    );
    out.push_str("    {\n");
    out.push_str("        let n: usize = self.data.len();\n");
    out.push_str("        let mut lo: usize = 0;\n");
    out.push_str("        while lo < n && {\n");
    out.push_str("                let c = self.data[lo];\n");
    out.push_str("                c == 32 || c == 9 || c == 10 || c == 13\n");
    out.push_str("            }\n");
    out.push_str("            invariant lo <= n, n == self.data.len(),\n");
    out.push_str("            decreases n - lo,\n");
    out.push_str("        { lo = lo + 1; }\n");
    out.push_str("        let mut hi: usize = n;\n");
    out.push_str("        while hi > lo && {\n");
    out.push_str("                let c = self.data[hi - 1];\n");
    out.push_str("                c == 32 || c == 9 || c == 10 || c == 13\n");
    out.push_str("            }\n");
    out.push_str("            invariant lo <= hi, hi <= n, n == self.data.len(),\n");
    out.push_str("            decreases hi,\n");
    out.push_str("        { hi = hi - 1; }\n");
    out.push_str("        let mut out: Vec<u8> = Vec::new();\n");
    out.push_str("        let mut i: usize = lo;\n");
    out.push_str("        while i < hi\n");
    out.push_str("            invariant\n");
    out.push_str("                lo <= i, i <= hi, hi <= n, n == self.data.len(),\n");
    writeln!(out, "                self.data.len() <= {cap},").ok();
    out.push_str("                out@ == self.data@.subrange(lo as int, i as int),\n");
    out.push_str("            decreases hi - i,\n");
    out.push_str("        {\n");
    out.push_str("            let ghost old_out = out@;\n");
    out.push_str("            out.push(self.data[i]);\n");
    out.push_str(
        "            assert(out@ =~= self.data@.subrange(lo as int, (i + 1) as int)) by {\n",
    );
    out.push_str(
        "                assert(self.data@.subrange(lo as int, (i + 1) as int) =~= self.data@.subrange(lo as int, i as int).push(self.data@[i as int]));\n",
    );
    out.push_str("            }\n");
    out.push_str("            i = i + 1;\n");
    out.push_str("        }\n");
    out.push_str("        assert(out@ == self.data@.subrange(lo as int, hi as int));\n");
    out.push_str("        TString { data: out }\n");
    out.push_str("    }\n");
}

/// The C5 string search/transform method names (`.design/basis/07-strings.md`
/// REQ-13..16, issue #102). `contains` is INTENTIONALLY OMITTED from this trigger
/// set: it is shared with the C6 `Vec` membership op, so a bare `.contains(..)` does
/// not by itself imply a `String` receiver. The string-search defs (+ the substring
/// `TString::contains` method) are emitted when ANY of these UNAMBIGUOUS string ops
/// appears OR when a contract names a C5 spec fn (`occurs_at`/`contains_sub`/
/// `count_sep`/`sep_free`/`is_space`); a String `s.contains(needle)` in such a
/// program then resolves to `TString::contains` (receiver-type dispatch). A program
/// whose ONLY string op were a bare `contains` would still emit the method because it
/// names `contains_sub` in the contract (REQ-13's `ens result == contains_sub(..)`).
const STRING_SEARCH_METHODS: &[&str] = &["starts_with", "ends_with", "find", "split", "trim"];

/// The C5 GENERATED spec-fn names (mirrors `thermite-spec::validator::
/// GENERATED_SPEC_FNS`'s C5 additions): a contract naming any of these requires the
/// string-search defs emitted + drives the `<String> -> <String>.data@` byte-view
/// rewrite (the SAME mechanism `parse_le`/`parse_be` use).
const GENERATED_SEARCH_SPEC_FNS: &[&str] = &[
    "occurs_at",
    "contains_sub",
    "count_sep",
    "sep_free",
    "is_space",
];

/// True if the program uses a C5 string search/transform op (REQ-13..16, #102) — an
/// unambiguous string method (`STRING_SEARCH_METHODS`) in exec/spec position OR a
/// contract naming a C5 spec fn (`GENERATED_SEARCH_SPEC_FNS`). Either reference
/// requires the search methods + the generated spec fns + `lemma_count_push` in
/// scope. EMPTY otherwise (byte-stable for the non-C5 corpus, no regression). The
/// walk reuses the `each_subexpr` full-tree traversal (the same shape as
/// `program_uses_numfmt`), over every fn/spec-fn body + every contract clause.
pub(crate) fn program_uses_string_search(program: &Program) -> bool {
    // #127 — SHAPE key: a C5 spec-fn name (`occurs_at`/`contains_sub`/`count_sep`/
    // `sep_free`) SHADOWED by a user `spec fn` (with a `String`/`&String` param)
    // resolves to the user fn, not the generated search def — excluded so a
    // user-named collision does not materialize the generated def (E0428). The
    // METHOD-call surface (`.contains`/`.split`/…) is not name-collidable with a
    // spec fn, so it still triggers generation unconditionally.
    let shadow = user_string_spec_fn_names(program);
    program.items.iter().any(|item| match item {
        Item::Fn(f) => {
            contract_uses_string_search(&f.contract, &shadow)
                || f.body
                    .as_ref()
                    .map(|b| block_uses_string_search(b, &shadow))
                    .unwrap_or(false)
        }
        Item::SpecFn(s) => block_uses_string_search(&s.body, &shadow),
        Item::Struct(_) | Item::Enum(_) => false,
    })
}

/// True if a fn `Contract`'s `req`/`ens` uses a C5 construct (REQ-13..16).
fn contract_uses_string_search(contract: &thermite_syntax::ast::Contract, shadow: &[&str]) -> bool {
    expr_uses_string_search(&contract.req.expr, shadow)
        || contract
            .ens
            .iter()
            .any(|c| expr_uses_string_search(&c.expr, shadow))
}

/// True if a block uses a C5 construct anywhere (REQ-13..16).
fn block_uses_string_search(block: &Block, shadow: &[&str]) -> bool {
    block
        .stmts
        .iter()
        .any(|s| stmt_uses_string_search(s, shadow))
        || block
            .tail
            .as_deref()
            .map(|e| expr_uses_string_search(e, shadow))
            .unwrap_or(false)
}

fn stmt_uses_string_search(stmt: &Stmt, shadow: &[&str]) -> bool {
    match stmt {
        Stmt::Let { init, .. } => expr_uses_string_search(init, shadow),
        Stmt::Assign { target, value } => {
            expr_uses_string_search(target, shadow) || expr_uses_string_search(value, shadow)
        }
        Stmt::Return(opt) => opt
            .as_ref()
            .map(|e| expr_uses_string_search(e, shadow))
            .unwrap_or(false),
        Stmt::If {
            cond, then, else_, ..
        } => {
            expr_uses_string_search(cond, shadow)
                || block_uses_string_search(then, shadow)
                || else_
                    .as_ref()
                    .map(|b| block_uses_string_search(b, shadow))
                    .unwrap_or(false)
        }
        Stmt::Loop(l) => block_uses_string_search(&l.body, shadow),
        Stmt::Expr(e) => expr_uses_string_search(e, shadow),
        Stmt::Break | Stmt::Continue => false,
    }
}

/// True if `expr` references a C5 construct anywhere (REQ-13..16): an unambiguous
/// string-search `MethodCall` (`STRING_SEARCH_METHODS`) or a C5 spec-fn `Call`
/// (`GENERATED_SEARCH_SPEC_FNS`). A full-tree walk reusing `each_subexpr`. A C5
/// spec-fn name SHADOWED by a user `spec fn` (in `shadow`, #127) is excluded.
fn expr_uses_string_search(expr: &Expr, shadow: &[&str]) -> bool {
    match expr {
        Expr::MethodCall { name, .. } if STRING_SEARCH_METHODS.contains(&name.as_str()) => {
            return true;
        }
        Expr::Call { callee, .. } => {
            if let Expr::Path(segs) = callee.as_ref() {
                if let Some(last) = segs.last() {
                    if GENERATED_SEARCH_SPEC_FNS.contains(&last.as_str())
                        && !shadow.contains(&last.as_str())
                    {
                        return true;
                    }
                }
            }
        }
        _ => {}
    }
    let mut found = false;
    let _ = each_subexpr(expr, &mut |e| {
        if expr_uses_string_search(e, shadow) {
            found = true;
        }
        Ok(())
    });
    found
}

/// Emit the C5 string search/transform module-scope DEFINITIONS (REQ-13..16, #102):
/// the spec fns `occurs_at`/`contains_sub`/`count_sep`/`sep_free`/`is_space` + the
/// `lemma_count_push` proof fn (`split`'s count-invariant engine, proved by
/// induction). Emitted ONCE when the program uses a C5 op (`program_uses_string_
/// search`), in deterministic order. EMPTY otherwise (byte-stable). The emitted forms
/// are EXACTLY the GROUNDED `verus 0.2026.05.24` definitions (no `assume`/`admit`/
/// `external_body` — R-DEFER-9; `lemma_count_push` is a REAL induction proof). These
/// must be in scope for the `TString` search methods' contracts (which name them);
/// verus resolves references order-independently within the single `verus!` block.
fn emit_string_search_defs(program: &Program) -> Result<String, LowerError> {
    if !program_uses_string_search(program) {
        return Ok(String::new());
    }
    let mut out = String::new();
    out.push('\n');
    // occurs_at: needle occurs at byte offset `at` (a flat bounded `forall|k|` in the
    // NAMED spec-fn body — §4.2 composition through named spec fns).
    out.push_str("pub open spec fn occurs_at(s: Seq<u8>, needle: Seq<u8>, at: int) -> bool {\n");
    out.push_str("    0 <= at && at + needle.len() <= s.len()\n");
    out.push_str(
        "    && (forall|k: int| 0 <= k < needle.len() ==> #[trigger] s[at + k] == needle[k])\n",
    );
    out.push_str("}\n");
    // contains_sub: a flat single bounded existential (the §4.2 `exists`).
    out.push_str("pub open spec fn contains_sub(s: Seq<u8>, needle: Seq<u8>) -> bool {\n");
    out.push_str("    exists|at: int| occurs_at(s, needle, at)\n");
    out.push_str("}\n");
    // count_sep: the recursive separator count (split's result length).
    out.push_str("pub open spec fn count_sep(s: Seq<u8>, sep: u8) -> nat\n");
    out.push_str("    decreases s.len()\n");
    out.push_str("{ if s.len() == 0 { 0nat }\n");
    out.push_str(
        "  else { (if s[0] == sep { 1nat } else { 0nat }) + count_sep(s.subrange(1, s.len() as int), sep) } }\n",
    );
    // sep_free: no byte equals sep (each split piece).
    out.push_str("pub open spec fn sep_free(s: Seq<u8>, sep: u8) -> bool\n");
    out.push_str("{ forall|i: int| 0 <= i < s.len() ==> #[trigger] s[i] != sep }\n");
    // is_space: the ASCII-whitespace predicate (space/tab/LF/CR) trim strips. A
    // contract may name it; trim's exec inlines the byte test (a spec fn is not
    // callable in exec position).
    out.push_str(
        "pub open spec fn is_space(b: u8) -> bool { b == 32 || b == 9 || b == 10 || b == 13 }\n",
    );
    // lemma_count_push: appending a byte at the end adds (if b == sep {1} else {0}) to
    // the count — proved by induction on `s` (the count-invariant engine for split).
    out.push_str("pub proof fn lemma_count_push(s: Seq<u8>, b: u8, sep: u8)\n");
    out.push_str(
        "    ensures count_sep(s.push(b), sep) == count_sep(s, sep) + (if b == sep { 1nat } else { 0nat }),\n",
    );
    out.push_str("    decreases s.len(),\n");
    out.push_str("{\n");
    out.push_str("    let t = s.push(b);\n");
    out.push_str("    assert(t.len() == s.len() + 1);\n");
    out.push_str("    if s.len() == 0 {\n");
    out.push_str("        assert(t[0] == b);\n");
    out.push_str("        assert(t.subrange(1, t.len() as int) =~= Seq::<u8>::empty());\n");
    out.push_str("        assert(count_sep(t.subrange(1, t.len() as int), sep) == 0nat);\n");
    out.push_str("        assert(count_sep(t, sep) == (if t[0] == sep { 1nat } else { 0nat }));\n");
    out.push_str("    } else {\n");
    out.push_str("        assert(t[0] == s[0]);\n");
    out.push_str("        let ts = t.subrange(1, t.len() as int);\n");
    out.push_str("        let ss = s.subrange(1, s.len() as int);\n");
    out.push_str("        assert(ts =~= ss.push(b));\n");
    out.push_str("        lemma_count_push(ss, b, sep);\n");
    out.push_str(
        "        assert(count_sep(t, sep) == (if t[0] == sep { 1nat } else { 0nat }) + count_sep(ts, sep));\n",
    );
    out.push_str(
        "        assert(count_sep(s, sep) == (if s[0] == sep { 1nat } else { 0nat }) + count_sep(ss, sep));\n",
    );
    out.push_str("    }\n");
    out.push_str("}\n");
    // Blocker #130: the C5 search defs (`occurs_at`/`contains_sub`/`count_sep`/
    // `sep_free`/`is_space` + `lemma_count_push`) emit under the reserved namespace,
    // so a user `spec fn count_sep(&String, ..)` is a DISTINCT name from the
    // generated `count_sep(Seq<u8>, u8)` even when `s.split(sep)` genuinely pulls in
    // this module — no double definition (E0428).
    Ok(reserve_generated_names(&out))
}

/// True if the program uses `n.to_string()` anywhere (a `to_string` `MethodCall`)
/// OR names the generated `parse_le`/`pow10` spec fns in a contract (REQ-8, #94).
/// Either reference requires the generated `u64`→`String` round-trip definitions
/// in scope. EMPTY otherwise (byte-stable for the non-numfmt corpus). The walk
/// reuses the `each_subexpr` full-tree traversal (the same shape as
/// `expr_has_str_lit`), over every fn/spec-fn body + every contract clause.
fn program_uses_numfmt(program: &Program) -> bool {
    // #127 — SHAPE key: a `parse_le`/`parse_be`/`pow10` name SHADOWED by a user
    // `spec fn` (with a `String`/`&String` param) resolves to the user fn, not the
    // generated round-trip def — excluded so a user-named collision does not
    // materialize the generated def alongside it (E0428).
    let shadow = user_string_spec_fn_names(program);
    program.items.iter().any(|item| match item {
        Item::Fn(f) => {
            contract_uses_numfmt(&f.contract, &shadow)
                || f.body
                    .as_ref()
                    .map(|b| block_uses_numfmt(b, &shadow))
                    .unwrap_or(false)
        }
        Item::SpecFn(s) => block_uses_numfmt(&s.body, &shadow),
        Item::Struct(_) | Item::Enum(_) => false,
    })
}

/// True if a fn `Contract`'s `req`/`ens` names a numfmt construct (REQ-8).
fn contract_uses_numfmt(contract: &thermite_syntax::ast::Contract, shadow: &[&str]) -> bool {
    expr_uses_numfmt(&contract.req.expr, shadow)
        || contract
            .ens
            .iter()
            .any(|c| expr_uses_numfmt(&c.expr, shadow))
}

/// True if a block uses a numfmt construct anywhere (REQ-8) — a `to_string`
/// `MethodCall` in exec position or a `parse_le`/`pow10` call in a contract.
fn block_uses_numfmt(block: &Block, shadow: &[&str]) -> bool {
    block.stmts.iter().any(|s| stmt_uses_numfmt(s, shadow))
        || block
            .tail
            .as_deref()
            .map(|e| expr_uses_numfmt(e, shadow))
            .unwrap_or(false)
}

fn stmt_uses_numfmt(stmt: &Stmt, shadow: &[&str]) -> bool {
    match stmt {
        Stmt::Let { init, .. } => expr_uses_numfmt(init, shadow),
        Stmt::Assign { target, value } => {
            expr_uses_numfmt(target, shadow) || expr_uses_numfmt(value, shadow)
        }
        Stmt::Return(opt) => opt
            .as_ref()
            .map(|e| expr_uses_numfmt(e, shadow))
            .unwrap_or(false),
        Stmt::If {
            cond, then, else_, ..
        } => {
            expr_uses_numfmt(cond, shadow)
                || block_uses_numfmt(then, shadow)
                || else_
                    .as_ref()
                    .map(|b| block_uses_numfmt(b, shadow))
                    .unwrap_or(false)
        }
        Stmt::Loop(l) => block_uses_numfmt(&l.body, shadow),
        Stmt::Expr(e) => expr_uses_numfmt(e, shadow),
        Stmt::Break | Stmt::Continue => false,
    }
}

/// True if `expr` references a numfmt construct anywhere (REQ-8): a `to_string`
/// `MethodCall` (the surface `n.to_string()`) or a `parse_le`/`pow10` `Call` (a
/// contract naming the generated round-trip). A full-tree walk reusing
/// `each_subexpr` (the same structural traversal as `expr_has_str_lit`). A name
/// SHADOWED by a user `spec fn` (in `shadow`, #127) is excluded.
fn expr_uses_numfmt(expr: &Expr, shadow: &[&str]) -> bool {
    match expr {
        Expr::MethodCall { name, .. } if name == "to_string" => return true,
        Expr::Call { callee, .. } => {
            if let Expr::Path(segs) = callee.as_ref() {
                if let Some(last) = segs.last() {
                    if GENERATED_NUMFMT_SPEC_FNS.contains(&last.as_str())
                        && !shadow.contains(&last.as_str())
                    {
                        return true;
                    }
                }
            }
        }
        _ => {}
    }
    let mut found = false;
    let _ = each_subexpr(expr, &mut |e| {
        if expr_uses_numfmt(e, shadow) {
            found = true;
        }
        Ok(())
    });
    found
}

/// The GENERATED `u64`→`String` round-trip spec-fn names (mirrors
/// `thermite-spec::validator::GENERATED_SPEC_FNS`): a contract naming either
/// requires the generated definitions emitted + drives the `parse_le(result)` →
/// `parse_le(result.data@)` rewrite and the `as nat` equality coercion (REQ-8).
const GENERATED_NUMFMT_SPEC_FNS: &[&str] = &["parse_le", "parse_be", "pow10"];

/// Emit the generated `u64`→decimal-`String` round-trip definitions (REQ-8, #94)
/// when the program uses `n.to_string()` / names `parse_le`, in deterministic
/// order: the `pow10`/`parse_le` spec fns, the `lemma_parse_push` append lemma
/// (proved by induction), then the `u64_to_string` exec fn (the divide/mod-by-10
/// digit-extraction loop). EMPTY otherwise. The emitted form is EXACTLY the
/// GROUNDED `16 verified, 0 errors` round-trip: `u64_to_string` returns a `TString`
/// (so the surface `String` return type matches) with `ens parse_le(result.data@)
/// == n`; the loop invariant is the round-trip partial accumulator `parse_le(data@)
/// + m*pow10(data.len()) == n` with `decreases m`; the per-iteration step is
/// discharged by `lemma_parse_push` + `by(nonlinear_arith)`. NO `assume`/
/// `external_body`/`admit` (R-DEFER-9) — the round-trip is a REAL proof.
fn emit_numfmt_defs(program: &Program) -> Result<String, LowerError> {
    if !program_uses_numfmt(program) {
        return Ok(String::new());
    }
    let mut out = String::new();
    out.push('\n');
    // pow10 / parse_le — the decimal-weight + LSB-first decimal-value spec fns
    // (data[0] least significant — the divide/mod-by-10 construction order).
    out.push_str("pub open spec fn pow10(k: nat) -> nat\n");
    out.push_str("    decreases k\n");
    out.push_str("{ if k == 0 { 1 } else { 10 * pow10((k - 1) as nat) } }\n");
    out.push_str("pub open spec fn parse_le(s: Seq<u8>) -> nat\n");
    out.push_str("    decreases s.len()\n");
    out.push_str("{ if s.len() == 0 { 0 }\n");
    out.push_str(
        "  else { ((s[0] - 48) as nat) + 10 * parse_le(s.subrange(1, s.len() as int)) } }\n",
    );
    // parse_be — the MSB-first (human-readable) decimal value: the LAST byte is the
    // least significant. This is the DISPLAY-form parse the surface contract names
    // (REQ-8: "the displayed bytes round-trip against a big-endian parse"). The
    // reversed buffer `u64_to_string` returns round-trips against THIS parse.
    out.push_str("pub open spec fn parse_be(s: Seq<u8>) -> nat\n");
    out.push_str("    decreases s.len()\n");
    out.push_str("{ if s.len() == 0 { 0 }\n");
    out.push_str(
        "  else { parse_be(s.subrange(0, (s.len() - 1) as int)) * 10 + ((s[(s.len() - 1) as int] - 48) as nat) } }\n",
    );
    // seq_reverse — a recursively-defined reverse (head moves to the END), so the
    // display bridge `parse_be(seq_reverse(s)) == parse_le(s)` is a clean induction
    // mirroring `parse_le`'s recursion. (NOT vstd's index-based `Seq::reverse`, whose
    // `subrange` alignment would need extra lemmas; this self-contained form proves
    // GROUNDED `17 verified, 0 errors`.)
    out.push_str("pub open spec fn seq_reverse(s: Seq<u8>) -> Seq<u8>\n");
    out.push_str("    decreases s.len()\n");
    out.push_str("{ if s.len() == 0 { Seq::<u8>::empty() }\n");
    out.push_str("  else { seq_reverse(s.subrange(1, s.len() as int)).push(s[0]) } }\n");
    // lemma_parse_push — appending a digit at the end adds (d-48)*pow10(len) to the
    // value (proved by induction: base subrange==empty + pow10(0)==1; step subrange
    // recurse + pow10 fold + nonlinear_arith distribution + =~= extensionality).
    out.push('\n');
    out.push_str("proof fn lemma_parse_push(s: Seq<u8>, d: u8)\n");
    out.push_str(
        "    ensures parse_le(s.push(d)) == parse_le(s) + ((d - 48) as nat) * pow10(s.len()),\n",
    );
    out.push_str("    decreases s.len(),\n");
    out.push_str("{\n");
    out.push_str("    let sd = s.push(d);\n");
    out.push_str("    if s.len() == 0 {\n");
    out.push_str("        assert(sd.len() == 1);\n");
    out.push_str("        assert(sd[0] == d);\n");
    out.push_str("        assert(sd.subrange(1, sd.len() as int) =~= Seq::<u8>::empty());\n");
    out.push_str("        assert(parse_le(sd.subrange(1, sd.len() as int)) == 0);\n");
    out.push_str("        assert(parse_le(sd) == ((d - 48) as nat));\n");
    out.push_str("        assert(parse_le(s) == 0);\n");
    out.push_str("        assert(pow10(0) == 1);\n");
    out.push_str(
        "        assert(((d - 48) as nat) * pow10(0) == ((d - 48) as nat)) by(nonlinear_arith);\n",
    );
    out.push_str(
        "        assert(parse_le(sd) == parse_le(s) + ((d - 48) as nat) * pow10(s.len()));\n",
    );
    out.push_str("    } else {\n");
    out.push_str("        let t = s.subrange(1, s.len() as int);\n");
    out.push_str("        lemma_parse_push(t, d);\n");
    out.push_str("        assert(sd.len() == s.len() + 1);\n");
    out.push_str("        assert(sd[0] == s[0]);\n");
    out.push_str("        assert(sd.subrange(1, sd.len() as int) =~= t.push(d));\n");
    out.push_str("        assert(t.len() == s.len() - 1);\n");
    out.push_str("        assert(sd.subrange(1, sd.len() as int) == t.push(d));\n");
    out.push_str("        assert(parse_le(sd) == ((sd[0] - 48) as nat) + 10 * parse_le(sd.subrange(1, sd.len() as int)));\n");
    out.push_str(
        "        assert(parse_le(sd) == ((s[0] - 48) as nat) + 10 * parse_le(t.push(d)));\n",
    );
    out.push_str("        assert(parse_le(s) == ((s[0] - 48) as nat) + 10 * parse_le(t));\n");
    out.push_str("        assert(pow10(s.len()) == 10 * pow10(t.len()));\n");
    out.push_str("        assert(10 * (((d - 48) as nat) * pow10(t.len())) == ((d - 48) as nat) * pow10(s.len()))\n");
    out.push_str("            by(nonlinear_arith)\n");
    out.push_str("            requires pow10(s.len()) == 10 * pow10(t.len());\n");
    out.push_str("        assert(10 * (parse_le(t) + ((d - 48) as nat) * pow10(t.len()))\n");
    out.push_str("            == 10 * parse_le(t) + 10 * (((d - 48) as nat) * pow10(t.len()))) by(nonlinear_arith);\n");
    out.push_str("        assert(parse_le(t.push(d)) == parse_le(t) + ((d - 48) as nat) * pow10(t.len()));\n");
    out.push_str("        assert(10 * parse_le(t.push(d))\n");
    out.push_str("            == 10 * parse_le(t) + ((d - 48) as nat) * pow10(s.len()));\n");
    out.push_str(
        "        assert(parse_le(sd) == parse_le(s) + ((d - 48) as nat) * pow10(s.len()));\n",
    );
    out.push_str("    }\n");
    out.push_str("}\n");
    // lemma_parse_be_push — appending a byte at the END of a big-endian sequence
    // multiplies the value by 10 and adds the new least-significant digit. The
    // building block for the reverse loop's per-iteration step (proved by =~=
    // extensionality on the subrange).
    out.push('\n');
    out.push_str("proof fn lemma_parse_be_push(s: Seq<u8>, d: u8)\n");
    out.push_str("    ensures parse_be(s.push(d)) == parse_be(s) * 10 + ((d - 48) as nat),\n");
    out.push_str("{\n");
    out.push_str("    let sd = s.push(d);\n");
    out.push_str("    assert(sd.len() == s.len() + 1);\n");
    out.push_str("    assert(sd[(sd.len() - 1) as int] == d);\n");
    out.push_str("    assert(sd.subrange(0, (sd.len() - 1) as int) =~= s);\n");
    out.push_str("}\n");
    // lemma_parse_be_reverse — THE DISPLAY BRIDGE (REQ-8): the MSB-first parse of the
    // reversed buffer equals the LSB-first parse of the construction buffer, by
    // induction on s. Carries the round-trip proof from the LSB construction
    // (`parse_le(data@) == n`) onto the reversed display form
    // (`parse_be(seq_reverse(data@)) == n`).
    out.push('\n');
    out.push_str("proof fn lemma_parse_be_reverse(s: Seq<u8>)\n");
    out.push_str("    ensures parse_be(seq_reverse(s)) == parse_le(s),\n");
    out.push_str("    decreases s.len(),\n");
    out.push_str("{\n");
    out.push_str("    if s.len() == 0 {\n");
    out.push_str("        assert(seq_reverse(s) =~= Seq::<u8>::empty());\n");
    out.push_str("    } else {\n");
    out.push_str("        let t = s.subrange(1, s.len() as int);\n");
    out.push_str("        lemma_parse_be_reverse(t);\n");
    out.push_str("        lemma_parse_be_push(seq_reverse(t), s[0]);\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    // lemma_pow10_le — pow10 is monotone non-decreasing in its exponent (a <= b =>
    // pow10(a) <= pow10(b)), by induction peeling the top factor (each step multiplies
    // by 10 >= 1). The building block for the digit-count UPPER bound (REQ-8's `<= 20`):
    // it bounds pow10(data.len()) from below by pow10(20) once data.len() reaches 20.
    out.push('\n');
    out.push_str("proof fn lemma_pow10_le(a: nat, b: nat)\n");
    out.push_str("    requires a <= b,\n");
    out.push_str("    ensures pow10(a) <= pow10(b),\n");
    out.push_str("    decreases b,\n");
    out.push_str("{\n");
    out.push_str("    if a < b {\n");
    out.push_str("        lemma_pow10_le(a, (b - 1) as nat);\n");
    out.push_str("        assert(pow10(b) == 10 * pow10((b - 1) as nat));\n");
    out.push_str("        assert(pow10((b - 1) as nat) <= 10 * pow10((b - 1) as nat)) by(nonlinear_arith);\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    // lemma_pow10_20_gt_u64max — pow10(20) == 10^20 == 100_000_000_000_000_000_000 is
    // STRICTLY GREATER than u64::MAX (18_446_744_073_709_551_615). The literal-evaluated
    // anchor (reveal-with-fuel folds pow10(20) to the closed literal) for REQ-8's
    // digit-count cap: a u64 is < pow10(20), so its decimal has at most 20 digits.
    out.push('\n');
    out.push_str("proof fn lemma_pow10_20_gt_u64max()\n");
    out.push_str("    ensures pow10(20) > u64::MAX as nat,\n");
    out.push_str("{\n");
    out.push_str("    reveal_with_fuel(pow10, 21);\n");
    out.push_str("    assert(pow10(20) == 100_000_000_000_000_000_000nat) by(compute);\n");
    out.push_str("}\n");
    // u64_to_string — the divide/mod-by-10 digit-extraction loop builds the decimal
    // LSB-first (the GROUNDED `parse_le(data@) == n` form), THEN reverses to the
    // human-readable MSB-first display order (REQ-8); the surface round-trip ens is
    // `parse_be(result.data@) == n` — the display bridge carries the proof. The first
    // loop invariant is the round-trip partial accumulator (`decreases m`); the
    // reverse loop maintains `out@ == seq_reverse(<suffix of data@>)` (`decreases
    // data.len() - i`), and `lemma_parse_be_reverse` closes the contract.
    out.push('\n');
    out.push_str("pub fn u64_to_string(n: u64) -> (result: TString)\n");
    // The round-trip is the gold standard; `result.data.len() >= 1` is the HONEST
    // floor that contractually FORBIDS the empty string (every decimal has at least
    // one digit, including 0 -> "0"). The round-trip alone admits "" for 0
    // (`parse_be([]) == 0`), so the len floor is what catches a dropped zero-guard
    // (blocker #97; without the guard this `ens` FAILS verus, R-DEFER-9 — real teeth).
    out.push_str("    ensures\n");
    out.push_str("        parse_be(result.data@) == n as nat,\n");
    out.push_str("        result.data.len() >= 1,\n");
    // `result.data.len() <= 20` — the HONEST UPPER floor (REQ-8): a u64 is < 10^20
    // (pow10(20) > u64::MAX), so its decimal has AT MOST 20 digits. This bounds the
    // formatted-number length from ABOVE so a caller's bounded `concat` (the §4.2 cage
    // CAP precondition `self.len() + b.len() <= CAP`) discharges when one operand is
    // `n.to_string()` (e.g. the editor's render_frame cursor coordinate). PROVED, not
    // assumed (blocker #105): the digit-count cap rides the build loop's invariant.
    out.push_str("        result.data.len() <= 20,\n");
    out.push_str("{\n");
    out.push_str("    let mut data: Vec<u8> = Vec::new();\n");
    out.push_str("    let mut m: u64 = n;\n");
    out.push_str("    proof {\n");
    out.push_str("        assert(data@ =~= Seq::<u8>::empty());\n");
    out.push_str("        assert(parse_le(data@) == 0);\n");
    out.push_str("        assert(pow10(0) == 1);\n");
    out.push_str("        assert((n as nat) * pow10(0) == n as nat) by(nonlinear_arith);\n");
    out.push_str("    }\n");
    // Zero-guard (mirrors the L1 `if m == 0 { data.push(48u8); }`, blocker #97):
    // `while m > 0` never runs for `n == 0`, so without this the verified output is
    // the EMPTY seq while the built L1 binary prints "0" ([48]) — L3 != L1 and REQ-8
    // (the human-readable decimal of 0 is "0") is missed. Pushing 48 ('0') makes the
    // verified bytes match L1 byte-for-byte; `parse_le([48]) == 0 == n` keeps the
    // build-loop invariant intact for the `n == 0` entry.
    out.push_str("    if m == 0 {\n");
    out.push_str("        data.push(48u8);\n");
    out.push_str("        proof {\n");
    out.push_str("            assert(data@.len() == 1);\n");
    out.push_str("            assert(data@[0] == 48u8);\n");
    out.push_str(
        "            assert(data@.subrange(1, data@.len() as int) =~= Seq::<u8>::empty());\n",
    );
    out.push_str("            assert(parse_le(data@.subrange(1, data@.len() as int)) == 0);\n");
    out.push_str("            assert(parse_le(data@) == 0);\n");
    out.push_str("            assert((m as nat) == 0);\n");
    out.push_str(
        "            assert((m as nat) * pow10(data.len() as nat) == 0) by(nonlinear_arith)\n",
    );
    out.push_str("                requires (m as nat) == 0;\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    while m > 0\n");
    out.push_str("        invariant\n");
    out.push_str(
        "            parse_le(data@) + (m as nat) * pow10(data.len() as nat) == n as nat,\n",
    );
    // `data.len() >= 1 || m > 0`: at entry n>0 -> m>0; n==0 -> guard pushed so
    // data.len()>=1. On exit `m == 0`, so the disjunct forces data.len()>=1 (the
    // len floor's proof, blocker #97).
    out.push_str("            data.len() >= 1 || m > 0,\n");
    // `data.len() <= 20` — the digit-count cap (REQ-8 upper floor, blocker #105). It is
    // maintained because a 21st digit cannot exist: if data.len() reached 20 with m > 0
    // (the loop guard), then m*pow10(20) >= pow10(20) > u64::MAX >= n would contradict
    // the round-trip invariant parse_le(data@) + m*pow10(data.len()) == n. So at the top
    // of the body data.len() <= 19, and the single push keeps data.len() <= 20.
    out.push_str("            data.len() <= 20,\n");
    out.push_str("        decreases m,\n");
    out.push_str("    {\n");
    out.push_str("        let d: u8 = (m % 10) as u8 + 48u8;\n");
    out.push_str("        let ghost old_data = data@;\n");
    out.push_str("        let ghost old_m = m as nat;\n");
    out.push_str("        let ghost old_len = data.len() as nat;\n");
    // Prove data.len() <= 19 BEFORE the push (so the push keeps data.len() <= 20). If
    // data.len() == 20 then pow10(20) <= m*pow10(20) (m >= 1) <= n (the round-trip
    // invariant, parse_le >= 0) <= u64::MAX < pow10(20) — a contradiction.
    out.push_str("        proof {\n");
    out.push_str("            if data.len() == 20 {\n");
    out.push_str("                lemma_pow10_20_gt_u64max();\n");
    out.push_str(
        "                assert(pow10(20) <= (m as nat) * pow10(20)) by(nonlinear_arith)\n",
    );
    out.push_str("                    requires (m as nat) >= 1;\n");
    out.push_str("                assert((m as nat) * pow10(data.len() as nat) <= n as nat);\n");
    out.push_str("                assert(false);\n");
    out.push_str("            }\n");
    out.push_str("        }\n");
    out.push_str("        data.push(d);\n");
    out.push_str("        proof {\n");
    out.push_str("            lemma_parse_push(old_data, d);\n");
    out.push_str("            assert((m as nat) == 10 * ((m / 10) as nat) + ((m % 10) as nat)) by(nonlinear_arith);\n");
    out.push_str("            assert(pow10((old_len + 1) as nat) == 10 * pow10(old_len));\n");
    out.push_str("        }\n");
    out.push_str("        m = m / 10;\n");
    out.push_str("        proof {\n");
    out.push_str("            assert(old_m * pow10(old_len)\n");
    out.push_str("                == ((d - 48) as nat) * pow10(old_len) + (m as nat) * pow10((old_len + 1) as nat))\n");
    out.push_str("                by(nonlinear_arith)\n");
    out.push_str("                requires\n");
    out.push_str("                    old_m == 10 * (m as nat) + ((d - 48) as nat),\n");
    out.push_str("                    pow10((old_len + 1) as nat) == 10 * pow10(old_len);\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    // data@ now satisfies parse_le(data@) == n (m == 0, the LSB-first construction).
    // Reverse into `out` to the human-readable MSB-first display order (REQ-8); the
    // loop maintains `out@ == seq_reverse(<suffix of data@>)`, so at exit
    // `out@ == seq_reverse(data@)` and `lemma_parse_be_reverse` gives
    // `parse_be(out@) == parse_le(data@) == n`.
    // `data.len() >= 1` falls out of the build-loop invariant on exit; carry it as
    // `out.len() == i` through the reverse loop so the `result.data.len() >= 1`
    // ensures discharges (out.len() == data.len() >= 1 at exit; blocker #97).
    out.push_str("    assert(data.len() >= 1);\n");
    // `data.len() <= 20` falls out of the build-loop invariant on exit (the digit-count
    // cap, blocker #105). `data` is NOT mutated by the reverse loop, so carry it as a
    // reverse-loop invariant: at exit i == data.len() and out.len() == i, so
    // out.len() == data.len() <= 20 — the `result.data.len() <= 20` ens discharges.
    out.push_str("    assert(data.len() <= 20);\n");
    out.push_str("    let mut out: Vec<u8> = Vec::new();\n");
    out.push_str("    let mut i: usize = 0;\n");
    out.push_str("    while i < data.len()\n");
    out.push_str("        invariant\n");
    out.push_str("            i <= data.len(),\n");
    out.push_str("            data.len() <= 20,\n");
    out.push_str("            out.len() == i,\n");
    out.push_str("            out@ =~= seq_reverse(data@.subrange((data.len() - i) as int, data.len() as int)),\n");
    out.push_str("        decreases data.len() - i,\n");
    out.push_str("    {\n");
    out.push_str(
        "        let ghost prefix = data@.subrange((data.len() - i) as int, data.len() as int);\n",
    );
    out.push_str("        out.push(data[data.len() - 1 - i]);\n");
    out.push_str("        i = i + 1;\n");
    out.push_str("        proof {\n");
    out.push_str("            let lo = (data.len() - i) as int;\n");
    out.push_str("            let whole = data@.subrange(lo, data@.len() as int);\n");
    out.push_str("            assert(whole.len() > 0);\n");
    out.push_str("            assert(whole[0] == data@[lo]);\n");
    out.push_str("            assert(whole.subrange(1, whole.len() as int) =~= prefix);\n");
    out.push_str(
        "            assert(seq_reverse(whole) =~= seq_reverse(prefix).push(data@[lo]));\n",
    );
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    proof {\n");
    out.push_str("        assert(data@.subrange(0, data@.len() as int) =~= data@);\n");
    out.push_str("        lemma_parse_be_reverse(data@);\n");
    out.push_str("    }\n");
    out.push_str("    TString { data: out }\n");
    out.push_str("}\n");
    // Blocker #130: the C4 numfmt round-trip defs (`pow10`/`parse_le`/`parse_be`/
    // `seq_reverse` + the lemmas + `u64_to_string`) emit under the reserved
    // namespace, so a user `spec fn parse_be(&String, ..)` is a DISTINCT name from
    // the generated `parse_be(Seq<u8>)` even when `n.to_string()` genuinely pulls in
    // this module — no double definition (E0428).
    Ok(reserve_generated_names(&out))
}

/// The GENERATED `String`→`u64` parser spec-fn / fn names (Cluster C7,
/// `.design/basis/09-option-result.md` REQ-5, issue #95). A program calling
/// `parse_u64` (the surface partial parser) or naming `all_digits`/`is_digit` (a
/// contract over the parse witness) requires the generated definitions emitted.
/// `parse_be` is SHARED with the numfmt round-trip (`GENERATED_NUMFMT_SPEC_FNS`),
/// so it is NOT in this list — `emit_parse_defs` keys its own `parse_be` emission
/// off whether numfmt already emitted it (dedup).
const GENERATED_PARSE_FNS: &[&str] = &["parse_u64", "all_digits", "is_digit"];

/// The reserved prefix the lowerer mints its generated FREE-fn names under
/// (`.design/basis/07-strings.md` REQ-4 — the byte-view namespace is collision-free,
/// blocker #130). Every module-scope spec/exec/proof fn the lowerer SYNTHESIZES
/// (the C4 numfmt round-trip, the C7 parser, the C5 search/transform helpers, and
/// their proof lemmas) is emitted under this prefix, so a USER `spec fn`/`fn` whose
/// surface name coincides with a generated one (`spec fn is_digit(s: &String, ..)`,
/// `spec fn count_sep(s: &String, ..)`) is a DISTINCT name from the generated def —
/// no double definition (E0428), even when the generated module is GENUINELY pulled
/// in through a different, non-shadowed trigger (`parse_u64(s)` / `s.split(sep)`).
/// `thermite-spec/src/validator.rs` FORBIDS a user declaring a name with this prefix
/// (`SpecError::ReservedName`), so the reserved namespace is the lowerer's alone.
const THERMITE_RESERVED_PREFIX: &str = "__thermite_";

/// The complete set of GENERATED module-scope FREE-fn names the lowerer synthesizes
/// (the C4 numfmt, C7 parser, and C5 search/transform defs + their proof lemmas).
/// These are RESERVED-NAMED (`THERMITE_RESERVED_PREFIX`) at emission so they never
/// collide with a user `spec fn`/`fn` of the same surface name (blocker #130). The
/// `TString` METHODS (`matches_at`/`starts_with`/`split`/…) are NOT here: they are
/// inherent methods, namespaced under the `TString` impl, so they cannot collide
/// with a user free fn. `parse_u64`/`u64_to_string` ARE here — they are free fns the
/// user invokes at the surface (`parse_u64(s)`, `n.to_string()`), so their call
/// sites are rewritten to the reserved name (the dispatch keys on the SURFACE name,
/// which it still sees on the un-rewritten AST).
const GENERATED_FREE_FNS: &[&str] = &[
    // C7 parser (REQ-9) + C4 numfmt round-trip (REQ-8) spec fns
    "is_digit",
    "all_digits",
    "parse_be",
    "parse_le",
    "pow10",
    "seq_reverse",
    // C5 search/transform spec fns (REQ-13..16)
    "occurs_at",
    "contains_sub",
    "count_sep",
    "sep_free",
    "is_space",
    // generated exec fns (surface-invoked)
    "parse_u64",
    "u64_to_string",
    // generated proof lemmas
    "lemma_parse_push",
    "lemma_parse_be_push",
    "lemma_parse_be_reverse",
    "lemma_pow10_le",
    "lemma_pow10_20_gt_u64max",
    "lemma_count_push",
    "lemma_parse_be_prefix_le",
];

/// The reserved name for a generated free fn surface name (`is_digit` →
/// `__thermite_is_digit`). The single mint point for the reserved scheme.
fn reserved_name(surface: &str) -> String {
    format!("{THERMITE_RESERVED_PREFIX}{surface}")
}

/// True iff `name` is a generated free-fn surface name the lowerer reserves
/// (`GENERATED_FREE_FNS`). Used at a call site to decide whether to rewrite the
/// callee to its reserved name (`reserved_name`).
fn is_generated_free_fn(name: &str) -> bool {
    GENERATED_FREE_FNS.contains(&name)
}

/// Rewrite every WHOLE-IDENTIFIER occurrence of a generated free-fn name
/// (`GENERATED_FREE_FNS`) in a block of EMITTED generated source to its reserved
/// name (`THERMITE_RESERVED_PREFIX`). Applied to the lowerer's own synthesized
/// def/method blocks (the `emit_parse_defs`/`emit_numfmt_defs`/`emit_string_search_
/// defs` defs and the search METHODS), so the generated module both DEFINES and
/// INTER-CALLS the reserved names — never the bare surface names a user `spec fn`
/// (blocker #130). Whole-identifier matching (an ASCII-alphanumeric/`_` neighbour on
/// either side suppresses the rewrite) so `parse_be` is rewritten but
/// `parse_be_prefix_le` (matched separately as its own name) is not partially hit,
/// and a `.data@`-suffixed token is left intact. Operates ONLY on lowerer-generated
/// text (no user expression flows through here), so it is self-contained and
/// byte-stable. Deterministic: the names are scanned in `GENERATED_FREE_FNS` order
/// but whole-identifier matching makes the result order-independent.
fn reserve_generated_names(src: &str) -> String {
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < bytes.len() {
        // At an identifier boundary? (start of string or non-ident left neighbour)
        let at_boundary = i == 0 || !is_ident(bytes[i - 1]);
        let mut matched = false;
        if at_boundary && is_ident(bytes[i]) {
            for name in GENERATED_FREE_FNS {
                let nb = name.as_bytes();
                let end = i + nb.len();
                if end <= bytes.len()
                    && &bytes[i..end] == nb
                    && (end == bytes.len() || !is_ident(bytes[end]))
                {
                    out.push_str(THERMITE_RESERVED_PREFIX);
                    out.push_str(name);
                    i = end;
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            // Advance past a whole identifier (so a non-matching ident is not
            // re-scanned mid-token, which could false-match a generated name that is
            // a SUFFIX of a longer ident).
            if at_boundary && is_ident(bytes[i]) {
                let start = i;
                while i < bytes.len() && is_ident(bytes[i]) {
                    i += 1;
                }
                out.push_str(&src[start..i]);
            } else {
                // Push one full UTF-8 char (the source is ASCII generated code, but
                // stay char-safe — advance by the char's byte length).
                match src[i..].chars().next() {
                    Some(ch) => {
                        out.push(ch);
                        i += ch.len_utf8();
                    }
                    None => break,
                }
            }
        }
    }
    out
}

/// True if the program uses the `String`→`u64` parser anywhere (REQ-5): a
/// `parse_u64` call (the surface `parse_u64(s)`) or an `all_digits`/`is_digit`
/// reference in a contract. Either requires the generated parse definitions in
/// scope. EMPTY otherwise (byte-stable for the non-parse corpus). The walk reuses
/// the `each_subexpr` full-tree traversal exactly as `program_uses_numfmt`.
pub(crate) fn program_uses_parse(program: &Program) -> bool {
    // #127 — SHAPE key: a CALL whose name is a generated parse def name but which
    // resolves to a USER `spec fn` (declared with a `String`/`&String` param, the
    // user namespace) is NOT a use of the generated def — it would otherwise emit
    // the generated `is_digit(b: u8)` alongside the user's `is_digit(&String, ..)`
    // (E0428, the name defined twice). The shadow set excludes such names so a user
    // `spec fn is_digit` does not spuriously materialize the parse module.
    let shadow = user_string_spec_fn_names(program);
    program.items.iter().any(|item| match item {
        Item::Fn(f) => {
            contract_uses_parse(&f.contract, &shadow)
                || f.body
                    .as_ref()
                    .map(|b| block_uses_parse(b, &shadow))
                    .unwrap_or(false)
        }
        Item::SpecFn(s) => block_uses_parse(&s.body, &shadow),
        Item::Struct(_) | Item::Enum(_) => false,
    })
}

fn contract_uses_parse(contract: &thermite_syntax::ast::Contract, shadow: &[&str]) -> bool {
    expr_uses_parse(&contract.req.expr, shadow)
        || contract
            .ens
            .iter()
            .any(|c| expr_uses_parse(&c.expr, shadow))
}

fn block_uses_parse(block: &Block, shadow: &[&str]) -> bool {
    block.stmts.iter().any(|s| stmt_uses_parse(s, shadow))
        || block
            .tail
            .as_deref()
            .map(|e| expr_uses_parse(e, shadow))
            .unwrap_or(false)
}

fn stmt_uses_parse(stmt: &Stmt, shadow: &[&str]) -> bool {
    match stmt {
        Stmt::Let { init, .. } => expr_uses_parse(init, shadow),
        Stmt::Assign { target, value } => {
            expr_uses_parse(target, shadow) || expr_uses_parse(value, shadow)
        }
        Stmt::Return(opt) => opt
            .as_ref()
            .map(|e| expr_uses_parse(e, shadow))
            .unwrap_or(false),
        Stmt::If {
            cond, then, else_, ..
        } => {
            expr_uses_parse(cond, shadow)
                || block_uses_parse(then, shadow)
                || else_
                    .as_ref()
                    .map(|b| block_uses_parse(b, shadow))
                    .unwrap_or(false)
        }
        Stmt::Loop(l) => block_uses_parse(&l.body, shadow),
        Stmt::Expr(e) => expr_uses_parse(e, shadow),
        Stmt::Break | Stmt::Continue => false,
    }
}

/// True if `expr` references a parse construct anywhere (REQ-5): a `parse_u64` /
/// `all_digits` / `is_digit` `Call`. A full-tree walk reusing `each_subexpr`. A
/// name SHADOWED by a user `spec fn` (in `shadow`, #127) is excluded — the call
/// resolves to the user fn, not the generated parse def.
fn expr_uses_parse(expr: &Expr, shadow: &[&str]) -> bool {
    if let Expr::Call { callee, .. } = expr {
        if let Expr::Path(segs) = callee.as_ref() {
            if let Some(last) = segs.last() {
                if GENERATED_PARSE_FNS.contains(&last.as_str()) && !shadow.contains(&last.as_str())
                {
                    return true;
                }
            }
        }
    }
    let mut found = false;
    let _ = each_subexpr(expr, &mut |e| {
        if expr_uses_parse(e, shadow) {
            found = true;
        }
        Ok(())
    });
    found
}

/// Emit the generated `String`→`u64` PARTIAL parser definitions (Cluster C7,
/// `.design/basis/09-option-result.md` REQ-5, issue #95) when the program calls
/// `parse_u64`, in deterministic order: the `is_digit`/`all_digits`/`parse_be`
/// spec fns (the round-trip witnesses), then the `parse_u64` exec fn (the
/// Horner-accumulate loop with the BE partial-value invariant + the three
/// handled-or-loud `None` arms). EMPTY otherwise. The emitted form is EXACTLY the
/// GROUNDED `5 verified, 0 errors` parse: `parse_u64(s: &TString) -> Option<u64>`
/// with `ens match result { Some(v) => all_digits(s.data@) && s.data.len() >= 1 &&
/// parse_be(s.data@) == v as nat, None => true }`; the loop invariant is the BE
/// partial value over the consumed prefix (`parse_be(s.data@.subrange(0, i)) ==
/// acc`) + the all-digits prefix witness, `decreases s.data.len() - i`; the
/// overflow guard screams BEFORE `acc*10 + digit` would wrap. NO `assume`/
/// `external_body`/`admit` (R-DEFER-9). `parse_be` is emitted here ONLY when the
/// numfmt round-trip did not already emit it (shared dedup).
fn emit_parse_defs(program: &Program) -> Result<String, LowerError> {
    if !program_uses_parse(program) {
        return Ok(String::new());
    }
    let mut out = String::new();
    out.push('\n');
    // is_digit / all_digits — the per-byte digit predicate + the all-bytes-are-
    // digits witness (a bounded `forall` over the byte seq, the §4.2 cage form).
    out.push_str("pub open spec fn is_digit(b: u8) -> bool { 48 <= b && b <= 57 }\n");
    out.push_str("pub open spec fn all_digits(s: Seq<u8>) -> bool\n");
    out.push_str("{ forall|i: int| 0 <= i < s.len() ==> is_digit(#[trigger] s[i]) }\n");
    // parse_be — the MSB-first (read-order) decimal value. SHARED with the numfmt
    // round-trip (`emit_numfmt_defs`), so emit it HERE only when numfmt did not
    // (dedup — a program using BOTH `to_string` and `parse_u64` must not define
    // `parse_be` twice).
    if !program_uses_numfmt(program) {
        out.push_str("pub open spec fn parse_be(s: Seq<u8>) -> nat\n");
        out.push_str("    decreases s.len()\n");
        out.push_str("{ if s.len() == 0 { 0 }\n");
        out.push_str(
            "  else { parse_be(s.subrange(0, (s.len() - 1) as int)) * 10 + ((s[(s.len() - 1) as int] - 48) as nat) } }\n",
        );
    }
    // lemma_parse_be_prefix_le — parse_be is MONOTONE in the prefix length: the
    // big-endian value of a length-`k` prefix never exceeds the value of the whole
    // sequence (each appended digit multiplies the running value by 10 and adds a
    // non-negative digit). Proved by induction on the suffix (`decreases s.len() -
    // k`), `=~=` extensionality on the prefix-of-prefix + `by(nonlinear_arith)` for
    // the `* 10` growth. This carries the OVERFLOW `None` arm of `parse_u64`: when
    // the accumulator would overflow at byte `i+1`, the length-(i+1) prefix already
    // exceeds `u64::MAX`, so (if the whole input is all-digits) the FULL parse_be
    // exceeds `u64::MAX` — discharging the strengthened `result is None ==> ... ||
    // parse_be(s.data@) > u64::MAX`. NO `assume`/`external_body`/`admit`
    // (R-DEFER-9). GROUNDED `9 verified, 0 errors` with the two corpus callers.
    out.push('\n');
    out.push_str("proof fn lemma_parse_be_prefix_le(s: Seq<u8>, k: int)\n");
    out.push_str("    requires 0 <= k <= s.len(),\n");
    out.push_str("    ensures parse_be(s.subrange(0, k)) <= parse_be(s),\n");
    out.push_str("    decreases s.len() - k,\n");
    out.push_str("{\n");
    out.push_str("    if k == s.len() {\n");
    out.push_str("        assert(s.subrange(0, k) =~= s);\n");
    out.push_str("    } else {\n");
    out.push_str("        let m = (s.len() - 1) as int;\n");
    out.push_str("        assert(s.subrange(0, m).subrange(0, k) =~= s.subrange(0, k));\n");
    out.push_str("        lemma_parse_be_prefix_le(s.subrange(0, m), k);\n");
    out.push_str(
        "        assert(parse_be(s) == parse_be(s.subrange(0, m)) * 10 + ((s[m] - 48) as nat));\n",
    );
    out.push_str(
        "        assert(parse_be(s.subrange(0, m)) * 10 >= parse_be(s.subrange(0, m))) by(nonlinear_arith);\n",
    );
    out.push_str("    }\n");
    out.push_str("}\n");
    // parse_u64 — the Horner-accumulate loop. Takes `&TString` (the surface
    // `&String`), returns the Verus-native `Option<u64>`. The STRENGTHENED,
    // CALLER-USABLE contract (#100 — the C7 external oracle `parse_u64.cert.json`):
    //   (1) a valid in-range all-digit string is GUARANTEED `Some`
    //       (`(all_digits && len>=1 && parse_be<=MAX) ==> result is Some`), so a
    //       caller with that `req` discharges `ens result is Some` — `parse_valid`;
    //   (2) the round-trip on the success arm carries the `Some(v)` payload AND its
    //       digit/length witness (`Some(v) => all_digits(s.data@) && len>=1 &&
    //       parse_be == v`), so a caller proving `!all_digits` derives `result is
    //       None` (the `Some(v) => false` arm of `parse_rejects_nondigit`);
    //   (3) the handled-or-loud refusal (`result is None ==> !all_digits || len==0
    //       || parse_be > u64::MAX`) names EXACTLY the three reasons a parse fails.
    // The three partial cases take the LOUD `None` arm BEFORE corrupting `acc`; each
    // carries its proof (non-digit ⟹ !all_digits; overflow ⟹ prefix > MAX ⟹ (via
    // lemma_parse_be_prefix_le) full > MAX when all-digits). GROUNDED `9 verified, 0
    // errors` (with the two corpus callers); a broken `Some(0)` FAILS (non-vacuous).
    out.push('\n');
    out.push_str("pub fn parse_u64(s: &TString) -> (result: Option<u64>)\n");
    out.push_str("    ensures\n");
    out.push_str(
        "        (all_digits(s.data@) && s.data.len() >= 1 && parse_be(s.data@) <= u64::MAX) ==> result is Some,\n",
    );
    out.push_str("        match result {\n");
    out.push_str(
        "            Some(v) => all_digits(s.data@) && s.data.len() >= 1 && parse_be(s.data@) == v as nat,\n",
    );
    out.push_str("            None => true,\n");
    out.push_str("        },\n");
    out.push_str(
        "        result is None ==> (!all_digits(s.data@) || s.data.len() == 0 || parse_be(s.data@) > u64::MAX),\n",
    );
    out.push_str("{\n");
    out.push_str("    if s.data.len() == 0 { return None; }\n");
    out.push_str("    let mut acc: u64 = 0;\n");
    out.push_str("    let mut i: usize = 0;\n");
    out.push_str("    while i < s.data.len()\n");
    out.push_str("        invariant\n");
    out.push_str("            i <= s.data.len(),\n");
    out.push_str("            all_digits(s.data@.subrange(0, i as int)),\n");
    out.push_str("            parse_be(s.data@.subrange(0, i as int)) == acc as nat,\n");
    out.push_str("        decreases s.data.len() - i,\n");
    out.push_str("    {\n");
    out.push_str("        let b: u8 = s.data[i];\n");
    // non-digit → LOUD None; the offending byte witnesses `!all_digits(s.data@)`.
    out.push_str("        if b < 48 || b > 57 {\n");
    out.push_str("            assert(!is_digit(s.data@[i as int]));\n");
    out.push_str("            assert(!all_digits(s.data@));\n");
    out.push_str("            return None;\n");
    out.push_str("        }\n");
    out.push_str("        let digit: u64 = (b - 48) as u64;\n");
    // The subrange/index ghost glue: the prefix of length i+1 restricted to its
    // first i bytes is the length-i prefix, and its last byte is `b`; so the BE
    // value of the length-(i+1) prefix is `parse_be(prefix_i) * 10 + (b - 48)`.
    out.push_str("        let ghost old_i = i as int;\n");
    out.push_str(
        "        assert(s.data@.subrange(0, (i + 1) as int).subrange(0, old_i) =~= s.data@.subrange(0, old_i));\n",
    );
    out.push_str("        assert(s.data@.subrange(0, (i + 1) as int)[old_i] == b);\n");
    out.push_str(
        "        assert(parse_be(s.data@.subrange(0, (i + 1) as int)) == parse_be(s.data@.subrange(0, old_i)) * 10 + ((b - 48) as nat));\n",
    );
    // overflow → LOUD None, BEFORE the `acc*10 + digit` u64 arithmetic would wrap.
    // The length-(i+1) prefix value overflows u64::MAX; the monotonicity lemma
    // lifts that to the FULL parse_be when the whole input is all-digits, so the
    // strengthened `result is None ==> ... || parse_be > u64::MAX` arm discharges.
    out.push_str("        if acc > (u64::MAX - digit) / 10 {\n");
    out.push_str("            proof {\n");
    out.push_str("                assert(digit <= 9);\n");
    out.push_str(
        "                assert((acc as nat) * 10 + (digit as nat) > u64::MAX as nat) by(nonlinear_arith)\n",
    );
    out.push_str(
        "                    requires acc as nat > ((u64::MAX - digit) / 10) as nat, digit <= 9;\n",
    );
    out.push_str(
        "                assert(parse_be(s.data@.subrange(0, (i + 1) as int)) > u64::MAX);\n",
    );
    out.push_str("                if all_digits(s.data@) {\n");
    out.push_str("                    lemma_parse_be_prefix_le(s.data@, (i + 1) as int);\n");
    out.push_str("                }\n");
    out.push_str("            }\n");
    out.push_str("            return None;\n");
    out.push_str("        }\n");
    out.push_str("        acc = acc * 10 + digit;\n");
    out.push_str("        i = i + 1;\n");
    out.push_str("    }\n");
    // At exit i == s.data.len(), so the consumed prefix is the whole seq.
    out.push_str("    assert(s.data@.subrange(0, i as int) =~= s.data@);\n");
    out.push_str("    Some(acc)\n");
    out.push_str("}\n");
    // Blocker #130: emit the generated FREE fns under the reserved namespace so a
    // user `spec fn is_digit(&String, ..)` (a DISTINCT name) never double-defines
    // with the generated `is_digit(b: u8)` — even when `parse_u64(s)` (a non-shadowed
    // trigger) genuinely pulls this module in alongside the user fn.
    Ok(reserve_generated_names(&out))
}

// ---------------------------------------------------------------------------
// REQ-3/REQ-5: expression lowering (exec vs spec).
// ---------------------------------------------------------------------------

/// Lower an `Expr` in the given context (REQ-3 exec / REQ-5 spec). `depth`
/// bounds recursion (REQ-9). `span` is the nearest enclosing item span for error
/// loci.
fn lower_expr(expr: &Expr, ctx: Ctx, depth: usize, span: Span) -> Result<String, LowerError> {
    if depth >= MAX_EMIT_DEPTH {
        return Err(LowerError::TooDeep {
            limit: MAX_EMIT_DEPTH,
            span,
        });
    }
    let d = depth + 1;
    match expr {
        // Emit the numeric `value`, NOT `raw` (#37): the lowered output stays
        // byte-identical (`1_000_000` lowers to `1000000`); no golden churn.
        Expr::IntLit { value, .. } => Ok(value.to_string()),
        Expr::BoolLit(b) => Ok(b.to_string()),
        // Basis Stage 7 (`.design/basis/07-strings.md` REQ-1/REQ-4): a string
        // literal `"hello"` materializes into an owned `TString` whose bytes are
        // the literal's UTF-8, constructed by pushing each byte — the GROUNDED
        // `lit_hello` form (`{ let mut data = Vec::new(); data.push(104u8); …
        // TString { data } }`, `verified, 0 errors`). Emitted as an INLINE block
        // expression so it composes as a receiver (`"hello".len()` →
        // `({ … TString { data } }).len()`). It is a CONSTRUCTING op (it
        // allocates), so the enclosing fn carries `fx alloc` (the Stage-1
        // `Effect::Alloc`, accepted by effect-subsumption — `push` is an
        // intrinsic, no declared callee to subsume). The byte sequence is the
        // literal's UTF-8 (`str::as_bytes`), so a multi-byte codepoint pushes each
        // of its bytes (v1 indexes bytes, REQ-2 char model).
        Expr::StrLit(s) => {
            let mut block = String::from("({ let mut data: Vec<u8> = Vec::new();");
            for b in s.as_bytes() {
                write!(block, " data.push({b}u8);").ok();
            }
            block.push_str(" TString { data } })");
            Ok(block)
        }
        Expr::Path(segs) => {
            // Cluster C4 (`.design/basis/07-strings.md` REQ-7, issue #94): a
            // `String::`-qualified associated call (`String::from_byte(b)`) names the
            // surface type `String`, which lowers to the wrapper `TString` (the SAME
            // mapping `lower_type` applies to `Type::String`). Rewrite a leading
            // `String` path segment to `TString` so the associated constructor
            // resolves to the wrapper's `from_byte` (Verus would otherwise resolve
            // `String` to `std::string::String`, which has no `from_byte`). Keyed on
            // the leading segment being exactly `String` with a `::`-qualified tail
            // (a bare `String` is a TYPE position handled by `lower_type`, never an
            // expression path).
            if segs.len() >= 2 && segs[0] == "String" {
                let mut out = String::from("TString");
                for seg in &segs[1..] {
                    out.push_str("::");
                    out.push_str(seg);
                }
                return Ok(out);
            }
            // A plain path emits its segments joined by `::`. The slice→`xs@`
            // view (REQ-5) is applied at the point of USE (a spec-fn / combinator
            // argument position — `lower_spec_arg`), NOT here, because an `Index`
            // base must stay bare (`lower_index` appends the `@`) to avoid `xs@@`.
            Ok(segs.join("::"))
        }
        Expr::Call { callee, args } => {
            // Basis Stage 2 (`.design/basis/02-recursion-schemes.md` REQ-6): a
            // scheme CALL `fold(l, 0, |x, acc| …)` lowers to a CALL of the
            // generated `fold_<e>` with the step closure lowered to a typed Verus
            // `spec_fn`. Resolved through the in-scope scheme bindings (the
            // current spec fn's `with_schemes`); a non-scheme call falls through.
            if let Expr::Path(segs) = callee.as_ref() {
                if let Some(name) = segs.last() {
                    if let Some(binding) = ctx.scheme_binding(name) {
                        return lower_scheme_call(binding, args, ctx, d, span);
                    }
                }
            }
            // Cluster C7 (`.design/basis/09-option-result.md` REQ-5, #100): the
            // generated `parse_u64` takes `&TString` (a read-only borrow). An EXEC
            // call `parse_u64(s)` whose arg `s` is an OWNED `String` param
            // (`ctx.is_owned_string`) must borrow it — `parse_u64(&s)` — to satisfy
            // the `&TString` parameter (a surface `String` lowers to an owned
            // `TString`, not a reference). A `&String` param is ALREADY a borrow and
            // is NOT in `owned_strings`, so it passes through unchanged
            // (`parse_u64(s)` where `s: &TString` typechecks directly). Keyed on the
            // callee NAME `parse_u64` + the arg being an owned-`String` bare path.
            if !ctx.is_spec() {
                if let Expr::Path(csegs) = callee.as_ref() {
                    if csegs.last().map(|s| s.as_str()) == Some("parse_u64")
                        && !ctx.is_user_string_spec_fn("parse_u64")
                    {
                        if let [Expr::Path(asegs)] = args.as_slice() {
                            if asegs.len() == 1 && ctx.is_owned_string(&asegs[0]) {
                                // Blocker #130: the generated parser is reserved-named
                                // (`__thermite_parse_u64`); rewrite the surface call.
                                return Ok(format!(
                                    "{}(&{})",
                                    reserved_name("parse_u64"),
                                    asegs[0]
                                ));
                            }
                        }
                    }
                }
            }
            let mut c = lower_expr(callee, ctx, d, span)?;
            // Blocker #130: a call whose callee names a GENERATED free fn
            // (`GENERATED_FREE_FNS` — `parse_be`/`count_sep`/`occurs_at`/`parse_u64`/…)
            // resolves to the lowerer-synthesized def, which is emitted under the
            // reserved namespace (`THERMITE_RESERVED_PREFIX`). Rewrite the lowered
            // callee to the reserved name so a surface contract reference (`ens
            // parse_be(result) == n`, `ens result == contains_sub(s, p)`) or an exec
            // `parse_u64(s)` (a `&String` param, not the owned-borrow case above)
            // binds to the generated def. A USER `spec fn` of the same name
            // (`is_user_string_spec_fn` — declares a `String`/`&String` param) lives
            // in the user namespace and is NOT rewritten, so a user `spec fn
            // is_digit(&String, ..)` self-call passes through as `is_digit` (the
            // user's), distinct from `__thermite_is_digit`. Keyed on the SURFACE name
            // (the un-rewritten `callee` Expr the dispatch below also inspects).
            if let Expr::Path(csegs) = callee.as_ref() {
                if let Some(last) = csegs.last() {
                    if csegs.len() == 1
                        && is_generated_free_fn(last)
                        && !ctx.is_user_string_spec_fn(last)
                    {
                        c = reserved_name(last);
                    }
                }
            }
            // In spec position, a bare slice-param argument to a spec fn or a
            // combinator is passed as its `Seq` view `xs@` (REQ-5). Keyed on the
            // in-scope slice-param SHAPE set (`ctx.is_slice`), not on names. A
            // combinator `Index`-kind argument (per the registry `arg_kinds`)
            // that is a bare `usize` var is cast `as int` (the registry spec-fn
            // param is `int`) — keyed on the registry kind, not on the name.
            let arg_kinds = combinator_arg_kinds(callee);
            // Cluster C5 (`.design/basis/07-strings.md` REQ-15, issue #102): the
            // generated `count_sep(s: Seq<u8>, sep: u8)` / `sep_free(s: Seq<u8>, sep:
            // u8)` spec fns take a `u8` separator, but the Thermite surface separator
            // is a `u64` (the `byte_at -> u64` convention — a contract `ens
            // result.len() == 1 + count_sep(s, sep)` names a `u64` `sep`). The first
            // arg is the `String` byte-view (`s -> s.data@`, the existing `is_string`
            // rule); the SECOND arg is the `sep` byte, cast `as u8` to match the spec
            // fn's param (Verus does no implicit `u64 -> u8` in spec position). Keyed
            // on the callee NAME being a sep-taking C5 spec fn + the arg index (1),
            // mirroring `split`'s exec `as u8` coercion at the call site. A bare-int
            // literal flows in directly; an arg already `as u8` is left as-is.
            let sep_fn = ctx.is_spec()
                && matches!(
                    callee.as_ref(),
                    Expr::Path(segs) if segs.last().map(|s| matches!(s.as_str(), "count_sep" | "sep_free")).unwrap_or(false)
                );
            // Cluster C5 (`.design/basis/07-strings.md` REQ-13/REQ-14, issue #102): the
            // generated `occurs_at(s: Seq<u8>, needle: Seq<u8>, at: int)` spec fn takes
            // an `int` occurrence offset, but the surface offset is a `u64` (the
            // `find -> Option<u64>` payload, or a literal) — a contract `ens occurs_at(s,
            // p, i)` names a `u64` `i`. Args 0/1 are the `String` byte-views (the
            // `is_string` `.data@` rule); arg 2 is the `at` offset, cast `as int` (Verus
            // does no implicit `u64 -> int` in a spec-fn arg position). The SAME `as
            // int` coercion a combinator `Index`-kind arg gets, here keyed on the callee
            // NAME `occurs_at` + the arg index (2). A literal / already-`as int` arg
            // passes through (`lower_index_arg` avoids the double-cast).
            let occurs_fn = ctx.is_spec()
                && matches!(
                    callee.as_ref(),
                    Expr::Path(segs) if segs.last().map(|s| s.as_str()) == Some("occurs_at")
                );
            // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): does this callee
            // take a String argument as its byte `Seq<u8>` VIEW (`s -> s.data@`) or as
            // a `&TString` REFERENCE? The GENERATED byte-view spec fns
            // (`parse_be`/`parse_le`/`all_digits`/`is_digit`/`occurs_at`/
            // `contains_sub`/`count_sep`/`sep_free` — the C4/C5/C7 numfmt+search defs,
            // each declared over `Seq<u8>`) want the `.data@` view. A USER-DEFINED spec
            // fn that declares a `&String` param (the #126 `spec_line_start`/`count_x`
            // String-scanning twin) lowers that param to `&TString` and so wants the
            // reference PASSED THROUGH (`s`, not `s.data@`) — a recursive self-call
            // `count_x(s, i+1)` over a `&String` param would otherwise emit
            // `count_x(s.data@, ..)` (E0308 — `Seq<u8>` vs `&TString`). Keyed on the
            // FIXED generated-name set (`callee_takes_string_byteview`), so the view
            // applies for exactly the generated byte-view fns and the reference passes
            // through for every user String-param spec fn.
            let string_as_byteview = callee_takes_string_byteview(callee, ctx);
            // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4, #126): in SPEC
            // position Verus integer arithmetic is UNBOUNDED `int` — a `u64`-typed
            // `i + 1` evaluates to `int`, NOT `u64`. So a recursive spec-fn-call
            // argument that is an arithmetic expression over a `u64` index
            // (`spec_line_start(text, i + 1, …)` — the String-scanning twin's step)
            // must be narrowed back to the `u64` the lowered param expects, else
            // E0308 (`expected u64, found int`). The surface type set is
            // `u64`/`u32`/`usize`/`bool` (no `nat`/`int`), so a user spec fn's
            // integer param is ALWAYS `u64`-shaped and the `as u64` is the right
            // narrowing (Verus proves no truncation from the body's own bounds).
            // Applied ONLY to an arithmetic arg of a PLAIN user spec fn — NOT a
            // combinator (its arg kinds drive the `Index`/`as int` path above), NOT a
            // byte-view fn (its String args go to `.data@`), and NOT a non-arithmetic
            // arg (a bare path / literal flows in unchanged). EMPTY effect on the
            // existing corpus: no corpus spec-fn call passes an arithmetic index (the
            // folds pass slices `t`/`*t`; the scheme calls pass `l`/seeds) — byte-stable.
            let plain_user_spec_call = ctx.is_spec() && arg_kinds.is_none() && !string_as_byteview;
            let mut parts = Vec::new();
            for (i, a) in args.iter().enumerate() {
                let is_index = arg_kinds
                    .map(|ks| ks.get(i).copied() == Some(thermite_spec::ArgKind::Index))
                    .unwrap_or(false);
                if is_index && ctx.is_spec() {
                    parts.push(lower_index_arg(a, ctx, d, span)?);
                } else if sep_fn && i == 1 {
                    let lowered = lower_spec_arg(a, ctx, string_as_byteview, d, span)?;
                    if matches!(a, Expr::IntLit { .. }) || lowered.ends_with("as u8") {
                        parts.push(lowered);
                    } else {
                        parts.push(format!("{lowered} as u8"));
                    }
                } else if occurs_fn && i == 2 {
                    parts.push(lower_index_arg(a, ctx, d, span)?);
                } else if plain_user_spec_call
                    && matches!(a, Expr::Binary { .. } | Expr::Unary { .. })
                {
                    // `as` binds tighter than `+`/`-`, so the arithmetic inner is
                    // parenthesized (#122 precedent): `(i + 1) as u64`, never
                    // `i + 1 as u64` (= `i + (1 as u64)`).
                    let lowered = lower_spec_arg(a, ctx, string_as_byteview, d, span)?;
                    if lowered.ends_with("as u64") {
                        parts.push(lowered);
                    } else {
                        parts.push(format!("({lowered}) as u64"));
                    }
                } else if plain_user_spec_call && spec_arg_is_nat_len(a) {
                    // A `String`-`.len()` in a CONTRACT argument lowers to the SPEC
                    // accessor `.spec_len()`, which returns `nat`; a user spec fn's
                    // integer param is `u64` (the surface set), so narrow `as u64`
                    // (`spec_scan(s, 0, s.spec_len(), 0)` → `… s.spec_len() as u64 …`).
                    // Verus proves no truncation from `well_formed` (`len <= CAP`).
                    let lowered = lower_spec_arg(a, ctx, string_as_byteview, d, span)?;
                    if lowered.ends_with("as u64") {
                        parts.push(lowered);
                    } else {
                        parts.push(format!("{lowered} as u64"));
                    }
                } else {
                    parts.push(lower_spec_arg(a, ctx, string_as_byteview, d, span)?);
                }
            }
            Ok(format!("{c}({})", parts.join(", ")))
        }
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            // The receiver lowers plainly: a slice `.len()` in spec position is
            // accepted by Verus on the slice (`haystack.len()`), as the golden
            // references confirm; the `@` view is only needed where a `Seq`
            // operation (`subrange`/index) is required (handled in `lower_index`).
            let r = lower_expr(receiver, ctx, d, span)?;
            // Cluster C4 (`.design/basis/07-strings.md` REQ-8, issue #94): the
            // `u64`→decimal-`String` method `n.to_string()` lowers to a CALL of the
            // generated free fn `u64_to_string(n)` (emitted by `emit_numfmt_defs`,
            // returning a `TString` with the round-trip `ens parse_le(result.data@)
            // == n`). The method spelling is the surface (07-strings.md REQ-8 pins
            // `n.to_string()`); the free-fn call is the clean lowering — the generated
            // fn carries the proof, so the method-call site is a thin dispatch. EXEC
            // position only (a constructing op, `fx alloc`); `to_string` never appears
            // in a contract (the round-trip is named via `parse_le`, not `to_string`).
            if !ctx.is_spec() && name == "to_string" && args.is_empty() {
                // Blocker #130: the generated formatter is reserved-named
                // (`__thermite_u64_to_string`).
                return Ok(format!("{}({r})", reserved_name("u64_to_string")));
            }
            // Basis Stage 4 (`.design/basis/04-collections.md` REQ-5): in SPEC
            // position the bounded-`Vec` accessor `v.get(i)` (a contract naming the
            // accessed element, `ens result == v.get(i)`) lowers to the wrapper's
            // SPEC accessor `v.spec_get(i as int)` — the exec `get` returns `T` but
            // a contract needs the spec function (`self.data@[i]`), and a Verus spec
            // index is `int`. `v.len()` in spec position lowers to the wrapper's
            // `spec fn len(&self) -> nat` unchanged (`r.len()`). Keyed on the method
            // NAME `get` in spec position only; exec `get`/`push`/`len` (a fn body)
            // lower verbatim to the verified vstd-backed exec methods. The index
            // cast `as int` is appended exactly as `lower_index_arg` does for a
            // combinator index, avoiding a double-cast on an already-`as int` arg.
            if ctx.is_spec() && name == "get" && args.len() == 1 {
                let idx = lower_index_arg(&args[0], ctx, d, span)?;
                return Ok(format!("{r}.spec_get({idx})"));
            }
            // Cluster C6 (`.design/basis/04-collections.md` REQ-8/REQ-12): in SPEC
            // position the bounded-`Vec` final-element accessor `v.last()` (a
            // contract naming the last element, `ens result == v.last()`) lowers to
            // the wrapper's SPEC accessor over the last index `v.spec_get((v.len() -
            // 1) as int)` — the exec `last` returns `T`/`&T` but a contract needs the
            // spec function (`self.data@[len-1]`), and a Verus spec index is `int`.
            // The receiver's spec `len` is `nat`, so the `- 1` stays in `int`
            // arithmetic and is cast once. Keyed on the method NAME `last` (no args)
            // in spec position only; the exec `last` (a fn body) lowers verbatim to
            // the verified wrapper method. `contains` is NOT rewritten here: its spec
            // meaning is the `exists` its exec `ens` already states, so a contract
            // naming `v.contains(x)` is admitted (REQ-12) but is not a v1 spec-fn
            // rewrite target (no corpus contract names it; it joins by amendment).
            if ctx.is_spec() && name == "last" && args.is_empty() {
                return Ok(format!("{r}.spec_get(({r}.len() - 1) as int)"));
            }
            // Cluster C12 (`.design/basis/13-map.md` REQ-4): in SPEC position the
            // bounded-`Map` membership accessor `m.contains_key(k)` (a contract naming
            // key membership, `ens result == m.contains_key(k)`) lowers to the
            // wrapper's SPEC abstraction `m.spec_contains_key(k)` — the exec
            // `contains_key` returns `bool` but a contract needs the spec function
            // (the `exists|j| data@[j].0 == k` membership). Keyed on the method NAME
            // `contains_key` (one arg) in spec position only; the exec `contains_key`
            // (a fn body) lowers verbatim to the verified wrapper method. The key arg
            // lowers plainly (a Copy key value, no `as int` cast — `spec_contains_key`
            // takes the surface key type). `m.len()` in spec position lowers to the
            // wrapper's `spec fn len -> nat` unchanged (`r.len()`, the SAME spec fn as
            // the `Vec` wrapper, so the existing pass-through covers it). `m.get(k)`
            // is NOT spec-rewritten: the exec `get -> Option<V>` is named in a
            // contract only via the C7 spec-`match`-in-`ens` over the RESULT (the
            // `match result { Some(v) => …, None => … }` form), exactly as the
            // grounded `insert_then_get` round-trip threads — not a spec-fn rewrite.
            if ctx.is_spec() && name == "contains_key" && args.len() == 1 {
                let arg = lower_expr(&args[0], ctx, d, span)?;
                return Ok(format!("{r}.spec_contains_key({arg})"));
            }
            // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): in SPEC position
            // a `String` receiver's `.len()` / `.byte_at(i)` lowers to the wrapper's
            // SPEC fns `.spec_len()` / `.spec_byte_at(i as int)` — the exec `len`/
            // `byte_at` return `u64` and cannot be NAMED in a contract (a contract
            // needs the spec function), and a Verus spec index is `int`. Keyed on
            // the receiver being a `String`-named bare path (`ctx.is_string`) so a
            // `Vec` receiver's `.len()` (whose wrapper spec fn IS `len`) is
            // UNCHANGED — the rewrite is `String`-specific. A `String` `result`
            // (a `String`-returning fn) is in the set too, so `result.len()` in an
            // `ens` rewrites the same way. The receiver path lowered to `r`.
            // The receiver is a `String` either as a bare value PATH (`s`,
            // `result` — `ctx.is_string`) or as a struct FIELD access (`b.text`,
            // `result.text` — `Expr::Field` whose `name` is a `String` field,
            // `ctx.is_string_field`). The editor core `ens result.text.len() ==
            // t.len()` / `b.text.len()` exercises the field form; the corpus
            // `greeting_len` the bare form. Both rewrite `.len()`/`.byte_at(i)` the
            // SAME way (the spec accessors); only the receiver-classification
            // differs, so the whole `String`-receiver class is covered (no field
            // sibling left for a critic to re-pin).
            let recv_is_string = ctx.is_spec()
                && match receiver.as_ref() {
                    Expr::Path(segs) => segs.len() == 1 && ctx.is_string(&segs[0]),
                    Expr::Field { name, .. } => ctx.is_string_field(name),
                    _ => false,
                };
            if recv_is_string {
                if name == "len" && args.is_empty() {
                    return Ok(format!("{r}.spec_len()"));
                }
                if name == "byte_at" && args.len() == 1 {
                    // The spec accessor `spec_byte_at(i: int)` takes an `int` index.
                    // An integer LITERAL (`s.byte_at(0)` — the corpus `first_byte`
                    // ens) flows into the `int` parameter directly (Verus coerces a
                    // literal), so it is emitted plainly, matching the GROUNDED
                    // golden `tests/golden/lower/string_demo.verus.rs`
                    // (`s.spec_byte_at(0)`, `11 verified, 0 errors`). A non-literal
                    // index (a `usize`-typed variable) needs the explicit `as int`
                    // cast Verus requires (no implicit `usize`->`int` in spec
                    // position), so it goes through `lower_index_arg`.
                    let idx = if matches!(&args[0], Expr::IntLit { .. }) {
                        lower_expr(&args[0], ctx, d, span)?
                    } else {
                        lower_index_arg(&args[0], ctx, d, span)?
                    };
                    return Ok(format!("{r}.spec_byte_at({idx})"));
                }
            }
            // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): in EXEC position
            // the `String` wrapper's index accessors `byte_at(i: usize)` and
            // `slice(lo: usize, hi: usize)` take `usize` parameters (the `vstd::vec::Vec`
            // index type), but a Thermite surface index is commonly a `u64` (the
            // `Buf { cursor: u64 }` editor core, `s.slice(0, k)` with `k: u64`).
            // Verus performs NO implicit `u64 -> usize` narrowing, so each index
            // argument is coerced with an explicit `as usize` cast — the same
            // intrinsic-index coercion `byte_at`'s `usize` accessor needs, applied
            // uniformly across BOTH string index intrinsics so the whole op family
            // (no single triggering site left for a sibling to re-pin). Keyed on the
            // reserved built-in method NAME (`byte_at`/`slice` — there are no
            // user-defined methods in v0.1, so no misfire) and only on the positional
            // index args (`concat`'s by-value `TString` arg is NOT an index). An
            // integer LITERAL (`s.byte_at(0)`, `s.slice(0, ..)`) flows into the
            // `usize` parameter directly (Verus coerces a literal — the GROUNDED
            // golden `string_demo.verus.rs` `{ s.byte_at(0) }` stays byte-identical),
            // and an argument already written `as usize` is left as-is (no
            // double-cast); only a non-literal `u64`/`u32` index needs the explicit
            // narrowing. EXEC-only — the spec-position `.byte_at`/`.spec_*` rewrite
            // above handles a contract index.
            let coerce_usize = !ctx.is_spec() && matches!(name.as_str(), "byte_at" | "slice");
            // Cluster C5 (`.design/basis/07-strings.md` REQ-15, issue #102): the
            // `split(sep: u8)` exec accessor takes a `u8` separator (the backing byte
            // element), but the Thermite surface separator is a `u64` (the `byte_at ->
            // u64` zero-extension convention, the parser passes `,`/`\n` as `u64`).
            // Rust does NO implicit `u64 -> u8` narrowing, so the `split` arg is
            // coerced with an explicit `as u8` — the same byte-narrowing the wrapper's
            // `from_byte`/`push_byte` apply to their `b: u64` surface byte, here at the
            // call site because the emitted `split` param is `u8`. EXEC-only (a
            // contract names `sep` as a `u8` via the spec fns). An integer LITERAL
            // flows in directly (Verus/Rust coerce a literal) and an arg already `as
            // u8` is left as-is (no double-cast).
            let coerce_u8 = !ctx.is_spec() && name == "split";
            let mut parts = Vec::new();
            for a in args {
                let lowered = lower_expr(a, ctx, d, span)?;
                if coerce_usize && !matches!(a, Expr::IntLit { .. }) && !is_usize_cast(a) {
                    // Issue #122: `as` binds tighter than `+`/`-`/…, so a compound
                    // index `i - 1` MUST be parenthesized before the coercion —
                    // `i - 1 as usize` is `i - (1 as usize)` (`u64 - usize`, E0277).
                    // A simple arg never mis-binds, so the paren is added ONLY for a
                    // `Binary`/`Unary` index; an `as usize` arg already short-circuited.
                    if matches!(a, Expr::Binary { .. } | Expr::Unary { .. }) {
                        parts.push(format!("({lowered}) as usize"));
                    } else {
                        parts.push(format!("{lowered} as usize"));
                    }
                } else if coerce_u8
                    && !matches!(a, Expr::IntLit { .. })
                    && !lowered.ends_with("as u8")
                {
                    // Issue #122: same precedence-safety as the `as usize` arm —
                    // a binary/unary separator (`split(sep - 1)`) is parenthesized
                    // so `as u8` binds the whole inner, not just its last operand.
                    if matches!(a, Expr::Binary { .. } | Expr::Unary { .. }) {
                        parts.push(format!("({lowered}) as u8"));
                    } else {
                        parts.push(format!("{lowered} as u8"));
                    }
                } else {
                    parts.push(lowered);
                }
            }
            Ok(format!("{r}.{name}({})", parts.join(", ")))
        }
        Expr::Field { receiver, name } => {
            let r = lower_expr(receiver, ctx, d, span)?;
            Ok(format!("{r}.{name}"))
        }
        Expr::Closure { params, body } => {
            // Verus `spec_fn` literal `|x: u32| <body>` (REQ-6). The corpus
            // closures are all `u32`-typed slice-element predicates.
            let b = lower_expr(body, ctx.keep(), d, span)?;
            let ps: Vec<String> = params.iter().map(|p| format!("{p}: u32")).collect();
            Ok(format!("|{}| {b}", ps.join(", ")))
        }
        Expr::Match { scrutinee, arms } => lower_match(scrutinee, arms, ctx, d, span),
        Expr::If { cond, then, else_ } => {
            let c = lower_expr(cond, ctx, d, span)?;
            let t = lower_block_inner(then, ctx, d, span)?;
            let e = lower_block_inner(else_, ctx, d, span)?;
            Ok(format!("if {c} {{ {} }} else {{ {} }}", t.trim(), e.trim()))
        }
        Expr::Binary { op, lhs, rhs } => {
            // OQ-1 nat/u64 coercion: an `Eq` where one side calls a `nat`-typed
            // spec fn forces an `as nat` cast on the other (a `u64`-valued
            // scalar) side, since `nat != u64` in Verus. Keyed on the SHAPE
            // (a call to a known nat-spec-fn), not on names. Only in spec
            // position and only when the scalar side is not already a cast.
            if *op == BinOp::Eq && ctx.is_spec() {
                if let Some(s) = lower_nat_equality(lhs, rhs, ctx, d, span)? {
                    return Ok(s);
                }
            }
            // Precedence-preserving parenthesization: a child binary of strictly
            // lower precedence is wrapped (so `lo + (hi - lo) / 2` survives the
            // round-trip rather than degrading to `lo + hi - lo / 2`). The AST
            // already encodes grouping in its nesting; we only add the parens.
            let l = lower_binary_operand(lhs, *op, true, ctx, d, span)?;
            let r = lower_binary_operand(rhs, *op, false, ctx, d, span)?;
            Ok(format!("{l} {} {r}", binop(*op)))
        }
        Expr::Unary { op, expr: inner } => {
            // The prefix `!` (#92, ast.md REQ-10): Verus's `!` is TYPE-DIRECTED —
            // logical-not on `bool`, bitwise-not on an integer — so the lowering
            // emits the bare `!` and Verus resolves the meaning from the operand
            // type (ast.md OQ-4; the GROUNDED `!flag`/`!bits` both certify). The
            // operand is parenthesized when it is itself a binary (or another
            // unary) so the prefix binds only the operand: `!(a & b)` for a
            // grouped binary inner, never `!a & b`. A bare path/literal/call needs
            // no parens.
            let UnaryOp::Not = op;
            let inner_src = lower_expr(inner, ctx, d, span)?;
            let needs_parens = matches!(inner.as_ref(), Expr::Binary { .. });
            if needs_parens {
                Ok(format!("!({inner_src})"))
            } else {
                Ok(format!("!{inner_src}"))
            }
        }
        Expr::Index { base, index } => lower_index(base, index, ctx, d, span),
        Expr::Cast { expr, ty } => {
            let e = lower_expr(expr, ctx, d, span)?;
            // REQ-10: inside a `nat`-returning ADT-fold spec fn body, an integer
            // cast (`h as u64`) coerces to `as nat` so the fold's arithmetic stays
            // `nat` (the GROUNDED `sum_list` form `h as nat + sum_list(*t)`; a
            // `u64`-typed arm body is `int` in spec and verus rejects the match).
            // Keyed on the SHAPE (nat-ret spec body + an integer-target cast),
            // never a name. A `bool`/`()` cast is left as written.
            let t = if ctx.nat_ret && is_int_type(ty) {
                "nat".to_string()
            } else {
                lower_type(ty)?
            };
            // Issue #122: `as` binds TIGHTER than the binary/unary operators in
            // both Verus and Rust, so a cast over a binary/unary inner operand
            // (`(n - 1) as nat`) MUST parenthesize the inner — without the parens
            // `n - 1 as nat` parses as `n - (1 as nat)`, an `int`/`nat`
            // (or `u64`/`usize`) type mismatch → L0. The hand-authored golden
            // reference uses exactly this form (`tests/golden/lower/parse_u64.verus.rs`
            // `(k - 1) as nat`, `(s.len() - 1) as int`). A non-binary/unary inner
            // (`i as int`, `acc as nat`, the literal `0 as usize`) never mis-binds,
            // so the paren is added ONLY for a `Binary`/`Unary` inner — the simple
            // casts the corpus/goldens pin stay byte-identical (no regression).
            let e = if matches!(expr.as_ref(), Expr::Binary { .. } | Expr::Unary { .. }) {
                format!("({e})")
            } else {
                e
            };
            Ok(format!("{e} as {t}"))
        }
        Expr::Ref { mutable, expr } => {
            // In spec position `&xs[..i]` becomes `xs@.subrange(..)` (handled in
            // lower_index when the inner is an Index); a bare `&e` keeps the `&`.
            if ctx.is_spec() {
                if let Expr::Index { base, index } = expr.as_ref() {
                    return lower_index(base, index, ctx.keep(), d, span);
                }
            }
            let e = lower_expr(expr, ctx, d, span)?;
            if *mutable {
                Ok(format!("&mut {e}"))
            } else {
                Ok(format!("&{e}"))
            }
        }
        // Basis Stage 1c (`.design/basis/01-adts.md` REQ-8/REQ-9/REQ-10).
        Expr::StructLit { path, fields } => {
            // A struct / struct-variant construction `Path { field: val, … }`
            // (REQ-2/REQ-8): the struct name (or ENUM-QUALIFIED variant) followed
            // by `field: <value>` initializers in source order. The GROUNDED form
            // `Account { balance: a.balance + amount }`. A struct path stays as
            // written; a single-segment user enum VARIANT is qualified.
            let head = qualify_variant_path(path, ctx);
            let mut parts = Vec::with_capacity(fields.len());
            for (name, value) in fields {
                let v = lower_expr(value, ctx, d, span)?;
                parts.push(format!("{name}: {v}"));
            }
            Ok(format!("{head} {{ {} }}", parts.join(", ")))
        }
        Expr::Is { scrutinee, variant } => {
            // A variant-discrimination test `SCRUTINEE is Variant` (REQ-6/REQ-9).
            // In SPEC/contract position Verus's `is` operator is the proven core
            // (the GROUNDED `s is Circle`): emit `<scrutinee> is <Variant>` with
            // the BARE variant identifier (the scrutinee's type fixes the enum).
            // In EXEC position (the C10 `while let` desugar's loop CONDITION —
            // `.design/basis/11-ergonomics.md` REQ-5) Verus REJECTS `is` ("cannot
            // test variant in exec mode"), so emit the exec-valid discriminant
            // `matches!(<scrutinee>, <Qualified::Variant> { .. })` — the SAME shape
            // the L1 mirror (`l1::lower_expr_exec`) uses, ENUM-QUALIFIED for a user
            // variant (a built-in `Some`/`None` stays unqualified).
            let s = lower_expr(scrutinee, ctx, d, span)?;
            if ctx.pos == Pos::Exec {
                let head = qualify_variant_path(variant, ctx);
                Ok(format!("matches!({s}, {head} {{ .. }})"))
            } else {
                let v = variant.last().cloned().unwrap_or_default();
                Ok(format!("({s} is {v})"))
            }
        }
        Expr::Deref(inner) => {
            // A `Box` dereference `*EXPR` (REQ-3/REQ-10): the recursive-occurrence
            // read `*tail` Verus follows transparently through the `Box`. Lowers to
            // `*<inner>` in both contexts (the GROUNDED `sum_list(*t)`).
            let e = lower_expr(inner, ctx, d, span)?;
            Ok(format!("*{e}"))
        }
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-7/REQ-8,
        // #109): an n-tuple construction `(a, b, …)` lowers to the Verus-NATIVE
        // tuple `(<e0>, <e1>, …)` — the GROUNDED `swap` body `(b, a)`
        // (`verified, 0 errors`). Each element lowers recursively in the SAME ctx
        // (exec or spec). A trailing arity-1 single-element tuple cannot occur (the
        // parser only builds `Expr::Tuple` at arity ≥ 2).
        Expr::Tuple(elems) => {
            let parts = elems
                .iter()
                .map(|e| lower_expr(e, ctx, d, span))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("({})", parts.join(", ")))
        }
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-8, #109): a
        // tuple projection `e.0`/`e.1`/… lowers to the Verus-NATIVE projection
        // `<recv>.<index>` (the GROUNDED `ens result.0 == b` form `r.0 == b`).
        // Works in BOTH exec and spec/contract position. The projection is the v1
        // §2.3 "one way" tuple access (destructuring deferred).
        Expr::TupleProj { receiver, index } => {
            let r = lower_expr(receiver, ctx, d, span)?;
            Ok(format!("{r}.{index}"))
        }
    }
}

/// True if `ty` is an integer primitive (`u32`/`u64`/`usize`) — the cast targets
/// that coerce to `nat` inside a `nat`-returning ADT-fold spec fn (REQ-10). A
/// `bool`/`()`/reference/slice/user type is NOT coerced.
fn is_int_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Prim(PrimType::U32) | Type::Prim(PrimType::U64) | Type::Prim(PrimType::Usize)
    )
}

/// True if `expr` is already an `as usize` cast — so the Stage-7 string index
/// coercion (`.design/basis/07-strings.md` REQ-4, the `byte_at`/`slice` `usize`
/// accessors) does NOT double-cast an argument the source already wrote as
/// `... as usize`.
fn is_usize_cast(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Cast {
            ty: Type::Prim(PrimType::Usize),
            ..
        }
    )
}

/// Lower a spec-position call/combinator argument (REQ-5). A bare slice-param
/// path `xs` is passed as its `Seq` view `xs@`; everything else lowers normally.
/// Keyed on the in-scope slice SHAPE set, not on names.
fn lower_spec_arg(
    arg: &Expr,
    ctx: Ctx,
    string_as_byteview: bool,
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    if ctx.is_spec() {
        if let Expr::Path(segs) = arg {
            if let Some(name) = segs.last() {
                if segs.len() == 1 && ctx.is_slice(name) {
                    return Ok(format!("{name}@"));
                }
                // Cluster C4 (`.design/basis/07-strings.md` REQ-8, issue #94): a
                // `String`-named value passed to a GENERATED byte-view spec fn is
                // viewed as its byte `Seq<u8>` — `result.data@` — so the round-trip
                // `parse_be(result)` lowers to `parse_be(result.data@)` (the `TString`
                // wraps a `vstd::vec::Vec<u8>` whose `Seq` view is `.data@`). The SAME
                // view-at-use rule a slice param gets (`xs@`), specialized to the
                // `TString`'s backing field. Keyed on the in-scope `String` SHAPE set
                // (`ctx.is_string`) AND on the callee taking the byte VIEW
                // (`string_as_byteview`, #126): a USER-DEFINED spec fn with a `&String`
                // param takes the `&TString` REFERENCE, so its String arg passes
                // through unchanged (a recursive `count_x(s, ..)` over a `&String`
                // param stays `count_x(s, ..)`, not `count_x(s.data@, ..)` — E0308).
                if string_as_byteview && segs.len() == 1 && ctx.is_string(name) {
                    return Ok(format!("{name}.data@"));
                }
            }
        }
    }
    lower_expr(arg, ctx, depth, span)
}

/// True iff a spec-fn-call argument is a `.len()` method call (Basis Stage 7
/// REQ-4, #126): in a CONTRACT a String `.len()` lowers to the SPEC accessor
/// `.spec_len()`, which returns `nat`. Passed to a user spec fn's `u64` integer
/// param it needs an `as u64` narrowing (the contract
/// `spec_scan(s, 0, s.len(), 0)`). Keyed on the `len` method NAME with no args —
/// the only `nat`-producing accessor that appears as a spec-fn-call argument.
fn spec_arg_is_nat_len(arg: &Expr) -> bool {
    matches!(arg, Expr::MethodCall { name, args, .. } if name == "len" && args.is_empty())
}

/// True iff `callee` is a GENERATED byte-view spec fn that takes its
/// String-derived argument as a byte `Seq<u8>` VIEW (`s -> s.data@`) rather than
/// as a `&TString` reference (Basis Stage 7, `.design/basis/07-strings.md`
/// REQ-4/REQ-8, #126). This is the FIXED set of lowerer-emitted spec fns whose
/// String params are declared over `Seq<u8>`: the C4 numfmt round-trip
/// (`parse_le`/`parse_be`/`all_digits`/`is_digit`) and the C5 search defs
/// (`occurs_at`/`contains_sub`/`count_sep`/`sep_free`). A USER-DEFINED spec fn
/// declaring a `&String` param is NOT in this set, so its String argument passes
/// through as the `&TString` reference the lowered signature expects (the #126
/// String-scanning twin `spec_line_start`/`count_x`).
///
/// SHAPE-keyed, not name-keyed (#127): the generated-name match is consulted ONLY
/// after excluding a USER `spec fn` of the same name. A user `spec fn` declaring a
/// `String`/`&String` param (`ctx.is_user_string_spec_fn`) lives in the user
/// namespace and SHADOWS any generated byte-view fn of that name — its param is
/// `&TString`, so its `String` arg passes the REFERENCE, never the `.data@` view.
/// This refutes the prior assumption that the surface "reserves" the names: a user
/// `spec fn is_digit(s: &String, ..)` now certifies L3 (the divergence #127 closes),
/// while the generated `is_digit(Seq<u8>)` (not in `program.items`) still byteviews.
fn callee_takes_string_byteview(callee: &Expr, ctx: Ctx) -> bool {
    if let Expr::Path(segs) = callee {
        if let Some(name) = segs.last() {
            // #127 — SHAPE key: a USER `spec fn` declaring a `String`/`&String`
            // param SHADOWS any generated byte-view fn of the same name. Its param
            // is `&TString` (a reference), so a `String` self-call arg passes `s`
            // through, NOT `s.data@` (`Seq<u8>` vs `&TString`, E0308). Excluded
            // BEFORE the generated-name match, so a user `spec fn is_digit(s:
            // &String, ..)` is never byte-viewed (the divergence #127 closes).
            if ctx.is_user_string_spec_fn(name) {
                return false;
            }
            return matches!(
                name.as_str(),
                "parse_le"
                    | "parse_be"
                    | "all_digits"
                    | "is_digit"
                    | "occurs_at"
                    | "contains_sub"
                    | "count_sep"
                    | "sep_free"
            );
        }
    }
    false
}

/// The registry `arg_kinds` of a call whose callee path names a combinator, or
/// `None` if the callee is not a combinator. Used to apply `as int` to
/// `Index`-kind arguments in spec position (REQ-5/REQ-6).
fn combinator_arg_kinds(callee: &Expr) -> Option<&'static [thermite_spec::ArgKind]> {
    if let Expr::Path(segs) = callee {
        if let Some(name) = segs.last() {
            return thermite_spec::lookup(name).map(|sig| sig.arg_kinds);
        }
    }
    None
}

/// Lower a combinator `Index`-kind argument in spec position: a bare `usize`
/// path is cast `as int` (the registry spec fn takes `int`). A compound index
/// expression lowers normally then is cast. Keyed on the registry kind.
fn lower_index_arg(arg: &Expr, ctx: Ctx, depth: usize, span: Span) -> Result<String, LowerError> {
    let lowered = lower_expr(arg, ctx, depth, span)?;
    // Avoid double-casting if the surface already wrote `as int`.
    if lowered.ends_with("as int") {
        Ok(lowered)
    } else if matches!(arg, Expr::Binary { .. } | Expr::Unary { .. }) {
        // Issue #122: `as` binds tighter than the binary/unary operators, so a
        // compound combinator index (`forall_in(s, ..)` with an `i - 1`-shaped
        // bound) MUST parenthesize the inner — `i - 1 as int` is `i - (1 as int)`.
        // A simple path index (`i`) never mis-binds (the common case).
        Ok(format!("({lowered}) as int"))
    } else {
        Ok(format!("{lowered} as int"))
    }
}

/// OQ-1 `nat`/`u64` coercion for an `Eq`: if one operand is a call to a
/// `nat`-returning spec fn (`ctx.is_nat_fn`) and the other is a `u64`-valued
/// scalar (a plain path like `acc`/`result`), emit `<scalar> as nat == <call>`.
/// Returns `None` when neither side is a nat-spec-fn call (so the caller falls
/// back to the plain binary lowering). Keyed on SHAPE.
fn lower_nat_equality(
    lhs: &Expr,
    rhs: &Expr,
    ctx: Ctx,
    depth: usize,
    span: Span,
) -> Result<Option<String>, LowerError> {
    let lhs_nat = is_nat_fn_call(lhs, ctx);
    let rhs_nat = is_nat_fn_call(rhs, ctx);
    // Exactly one side is a nat-spec-fn call: coerce the OTHER (scalar) side.
    let (scalar, call) = match (lhs_nat, rhs_nat) {
        (false, true) => (lhs, rhs),
        (true, false) => (rhs, lhs),
        _ => return Ok(None),
    };
    // Only coerce a bare scalar path (`acc`, `result`); leave compound exprs.
    if let Expr::Path(_) = scalar {
        let s = lower_expr(scalar, ctx, depth, span)?;
        let c = lower_expr(call, ctx, depth, span)?;
        return Ok(Some(format!("{s} as nat == {c}")));
    }
    Ok(None)
}

/// True if `expr` is a direct call to a `nat`-returning spec fn (SHAPE check).
fn is_nat_fn_call(expr: &Expr, ctx: Ctx) -> bool {
    if let Expr::Call { callee, .. } = expr {
        if let Expr::Path(segs) = callee.as_ref() {
            if let Some(name) = segs.last() {
                return ctx.is_nat_fn(name);
            }
        }
    }
    false
}

/// Lower an `Index` expression across the four `IndexArg` forms (REQ-3/REQ-5).
/// In spec context: `xs[i]`→`xs@[i as int]`, `&xs[..i]`→`xs@.subrange(0, i as
/// int)`, `xs[i..]`→`xs@.subrange(i as int, xs@.len() as int)`,
/// `xs[i..j]`→`xs@.subrange(i as int, j as int)`. In exec context, plain Rust.
fn lower_index(
    base: &Expr,
    index: &IndexArg,
    ctx: Ctx,
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    let b = lower_expr(base, ctx, depth, span)?;
    match (ctx.pos, index) {
        (Pos::Spec, IndexArg::Single(i)) => {
            let idx = lower_expr(i, ctx, depth, span)?;
            Ok(format!("{b}@[{idx} as int]"))
        }
        (Pos::Spec, IndexArg::RangeTo(i)) => {
            let idx = lower_expr(i, ctx, depth, span)?;
            Ok(format!("{b}@.subrange(0, {idx} as int)"))
        }
        (Pos::Spec, IndexArg::RangeFrom(i)) => {
            let idx = lower_expr(i, ctx, depth, span)?;
            Ok(format!("{b}@.subrange({idx} as int, {b}@.len() as int)"))
        }
        (Pos::Spec, IndexArg::Range(i, j)) => {
            let lo = lower_expr(i, ctx, depth, span)?;
            let hi = lower_expr(j, ctx, depth, span)?;
            Ok(format!("{b}@.subrange({lo} as int, {hi} as int)"))
        }
        (Pos::Exec, IndexArg::Single(i)) => {
            let idx = lower_expr(i, ctx, depth, span)?;
            Ok(format!("{b}[{idx}]"))
        }
        (Pos::Exec, IndexArg::RangeTo(i)) => {
            let idx = lower_expr(i, ctx, depth, span)?;
            Ok(format!("{b}[..{idx}]"))
        }
        (Pos::Exec, IndexArg::RangeFrom(i)) => {
            let idx = lower_expr(i, ctx, depth, span)?;
            Ok(format!("{b}[{idx}..]"))
        }
        (Pos::Exec, IndexArg::Range(i, j)) => {
            let lo = lower_expr(i, ctx, depth, span)?;
            let hi = lower_expr(j, ctx, depth, span)?;
            Ok(format!("{b}[{lo}..{hi}]"))
        }
    }
}

/// Lower a `match` (REQ-3). Used in `ens` (the `binary_search` `Option` match)
/// and in spec-fn bodies (the `sum` slice match, handled separately).
fn lower_match(
    scrutinee: &Expr,
    arms: &[MatchArm],
    ctx: Ctx,
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    let s = lower_expr(scrutinee, ctx, depth, span)?;
    let mut out = format!("match {s} {{\n");
    for arm in arms {
        let pat = lower_pattern(&arm.pattern, ctx, depth, span)?;
        let body = lower_expr(&arm.body, ctx, depth, span)?;
        // A C10 match guard lowers to the Verus-NATIVE guarded arm
        // `pat if <guard> => body` (`.design/basis/11-ergonomics.md` REQ-3). The
        // guard is the proven core (Verus supports match guards); the
        // down-weighted exhaustiveness (a guard does not complete a match) is the
        // validator's job — here we just emit the `if`.
        match &arm.guard {
            Some(guard) => {
                let g = lower_expr(guard, ctx, depth, span)?;
                writeln!(out, "            {pat} if {g} => {body},").ok();
            }
            None => {
                writeln!(out, "            {pat} => {body},").ok();
            }
        }
    }
    out.push_str("        }");
    Ok(out)
}

/// Lower a pattern (REQ-7/REQ-9 node set). A user enum-variant pattern is
/// ENUM-QUALIFIED (`Circle(r)`→`Shape::Circle(r)`, `Nil`→`List::Nil`) via the
/// `ctx.variants` map (verus rejects a bare variant); `Some(i)`/`None` and
/// bindings/wildcards/literals are NOT in the map, so they lower unqualified.
/// `Pattern::Struct` (`Rect { w, h }` / `Rect { .. }`) is REQ-4's struct-variant
/// destructuring (REQ-9 lowering).
fn lower_pattern(pat: &Pattern, ctx: Ctx, depth: usize, span: Span) -> Result<String, LowerError> {
    if depth >= MAX_EMIT_DEPTH {
        return Err(LowerError::TooDeep {
            limit: MAX_EMIT_DEPTH,
            span,
        });
    }
    match pat {
        Pattern::Wildcard => Ok("_".to_string()),
        Pattern::Binding(name) => Ok(name.clone()),
        Pattern::Literal(e) => lower_expr(e, Ctx::spec_seq(), depth + 1, span),
        Pattern::Enum { path, fields } => {
            let head = qualify_variant_path(path, ctx);
            if fields.is_empty() {
                Ok(head)
            } else {
                let mut fs = Vec::new();
                for f in fields {
                    fs.push(lower_pattern(f, ctx, depth + 1, span)?);
                }
                Ok(format!("{head}({})", fs.join(", ")))
            }
        }
        Pattern::Struct { path, fields, rest } => {
            // `Rect { w, h }` / `Rect { .. }` (REQ-4/REQ-9): an ENUM-QUALIFIED
            // struct-variant (or struct) destructuring pattern. Each field is
            // `name: <subpat>`; the `rest` flag emits the `..` of `Rect { .. }`.
            let head = qualify_variant_path(path, ctx);
            let mut parts = Vec::with_capacity(fields.len());
            for (name, subpat) in fields {
                let sub = lower_pattern(subpat, ctx, depth + 1, span)?;
                // A field-shorthand `Rect { w, h }` (parsed to `(w, Binding(w))`)
                // lowers to the bare field name; an explicit `field: pat` keeps the
                // `name: pat` form.
                if matches!(subpat, Pattern::Binding(b) if b == name) {
                    parts.push(name.clone());
                } else {
                    parts.push(format!("{name}: {sub}"));
                }
            }
            if *rest {
                parts.push("..".to_string());
            }
            if parts.is_empty() {
                Ok(format!("{head} {{}}"))
            } else {
                Ok(format!("{head} {{ {} }}", parts.join(", ")))
            }
        }
        // An or-pattern `p0 | p1 | …` (`.design/basis/11-ergonomics.md` REQ-4)
        // lowers to the Verus-NATIVE or-pattern `p0 | p1 | …` — Verus supports it
        // as the proven core. Each alternative is enum-qualified independently;
        // the alternatives are joined with ` | `.
        Pattern::Or(alts) => {
            let mut parts = Vec::with_capacity(alts.len());
            for alt in alts {
                parts.push(lower_pattern(alt, ctx, depth + 1, span)?);
            }
            Ok(parts.join(" | "))
        }
        Pattern::Slice(_) => Err(LowerError::Unsupported {
            what: "slice pattern outside a head-fold spec fn".to_string(),
            span,
        }),
    }
}

/// ENUM-QUALIFY a variant-pattern path (REQ-9): a single-segment user variant
/// `["Circle"]` becomes `Shape::Circle` via the `ctx.variants` map; an already
/// `::`-qualified path, a built-in (`Some`/`None`), or an unknown name is joined
/// as-written (verus knows `Option`; a user variant must be qualified or it is
/// rejected). Keyed on map membership, never on a name pattern.
fn qualify_variant_path(path: &[String], ctx: Ctx) -> String {
    if path.len() == 1 {
        if let Some(enum_name) = ctx.enum_of_variant(&path[0]) {
            return format!("{enum_name}::{}", path[0]);
        }
    }
    path.join("::")
}

/// The Verus/Rust operator for a `BinOp` (REQ-3).
fn binop(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        // #92 integer operators → their Verus-native operators. `%`/`<<`/`>>` carry
        // the divide-by-zero / shift-bound PROOF obligation Verus raises at the
        // operator site (ast.md REQ-11); the lowering emits the BARE operator and
        // MUST NOT suppress it (no `external`/`assume` — R-DEFER-9).
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

/// Binding-power tier of a binary operator (higher binds tighter). Mirrors the
/// pinned standard-Rust precedence (`surface-grammar.md` REQ-10) closely enough to
/// decide parenthesization of nested binaries during emission (REQ-3 — preserve
/// the AST's grouping). The #92 tiers (modulo at `* /`, shifts, `&`, `^`, `|`)
/// slot between `+ -` and comparison exactly as the parser threads them.
fn precedence(op: BinOp) -> u8 {
    match op {
        BinOp::Or => 1,
        BinOp::And => 2,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 3,
        BinOp::BitOr => 4,
        BinOp::BitXor => 5,
        BinOp::BitAnd => 6,
        BinOp::Shl | BinOp::Shr => 7,
        BinOp::Add | BinOp::Sub => 8,
        BinOp::Mul | BinOp::Div | BinOp::Rem => 9,
    }
}

/// Lower an operand of a binary expression, wrapping it in parens when a child
/// binary's precedence is lower than (or, for the right child of a
/// left-associative operator, equal to) the parent's — so the AST's grouping is
/// preserved verbatim (`lo + (hi - lo) / 2`, not `lo + hi - lo / 2`). `is_left`
/// distinguishes the two children for associativity.
fn lower_binary_operand(
    operand: &Expr,
    parent: BinOp,
    is_left: bool,
    ctx: Ctx,
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    let s = lower_expr(operand, ctx, depth, span)?;
    if let Expr::Binary { op: child, .. } = operand {
        let pp = precedence(parent);
        let cp = precedence(*child);
        let needs = cp < pp || (!is_left && cp == pp);
        if needs {
            return Ok(format!("({s})"));
        }
    }
    // Blocker (#139/#142 off-corpus TV finding): a `Cast` operand IMMEDIATELY
    // followed by a `<`-leading operator (`<`, `<=`, or `<<`) is AMBIGUOUS in both
    // Verus's and Rust's grammar — `x as u32 < 33` parses the `u32 <` as the start
    // of a GENERIC argument list (`u32<33, …>`), a hard parse error ("expected
    // `,`"). The fix is the dual of the #122 cast-INNER paren: parenthesize the
    // WHOLE cast when it is the LEFT operand of a `<`-leading op (`(x as u32) <
    // 33`). Keyed on (left operand is a `Cast`) AND (parent op begins with `<`), so
    // the corpus/goldens — whose casts feed `==`/`*`/`+`/`-`/`>`(`acc as nat ==`,
    // `i as u64 * …`, `xs.len() as u64 * …`) — stay BYTE-IDENTICAL (no churn). The
    // independent `thermite_tv::ref_encode` already parenthesizes every cast, so
    // this aligns production with the reference for the whole class of
    // `cast < operand` clauses the generator surfaced.
    if is_left && matches!(operand, Expr::Cast { .. }) && is_lt_leading(parent) {
        return Ok(format!("({s})"));
    }
    Ok(s)
}

/// Is `op` a `<`-leading operator (`<`, `<=`, `<<`)? A `Cast` left-operand of such
/// an op MUST be parenthesized — `x as u32 < 33` mis-parses the `u32 <` as a
/// generic-argument list (the off-corpus TV finding, #139/#142). `>`/`>=`/`>>` and
/// `==`/`!=` do NOT trigger the generic-parse ambiguity, so they are excluded
/// (keeps the corpus/goldens byte-identical).
fn is_lt_leading(op: BinOp) -> bool {
    matches!(op, BinOp::Lt | BinOp::Le | BinOp::Shl)
}

// ---------------------------------------------------------------------------
// REQ-4: statement, block and loop lowering (exec body).
// ---------------------------------------------------------------------------

/// Lower a `fn` body, threading the shape-derived proof aids (REQ-7). The body
/// is emitted between `{` and `}`; the loop lowering injects per-loop aids and
/// the extensionality assert at exit.
fn lower_fn_body(
    f: &FnItem,
    nat_fns: &[&str],
    string_fields: &[&str],
    variants: &[(&str, &str)],
) -> Result<String, LowerError> {
    let mut out = String::from("{\n");
    // A boundary fn (ffi-boundary.md REQ-2/OQ-3) has `body: None` and is NEVER
    // lowered to Verus — `forge`'s `check.rs` routes it to the L1 boundary path
    // BEFORE `lower` ever sees it (the foreign body cannot be proved). Reaching
    // here with no body is a structured error (R-CODE-2), never an unwrap.
    let body = f.body.as_ref().ok_or_else(|| LowerError::Unsupported {
        what: "lower (L3 Verus) reached a bodyless (boundary) fn; a boundary fn \
               certifies at L1 and is never lowered to Verus (ffi-boundary.md OQ-3)"
            .to_string(),
        span: f.span,
    })?;
    let inner = lower_block_with_fn_aids(body, f, nat_fns, string_fields, variants, 1)?;
    out.push_str(&inner);
    out.push_str("}\n");
    Ok(out)
}

/// Lower a block with the enclosing `fn`'s contract in scope, so loop lowering
/// can lift immutable preconditions and emit accumulator/coverage aids (REQ-7).
/// The exec context carries the enum-variant map (REQ-9) so a `match` over a user
/// enum (e.g. `is_circle`'s body) lowers to ENUM-QUALIFIED arms.
fn lower_block_with_fn_aids(
    block: &Block,
    f: &FnItem,
    nat_fns: &[&str],
    string_fields: &[&str],
    variants: &[(&str, &str)],
    indent: usize,
) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    let owned = owned_string_value_names(f);
    let exec = Ctx::exec()
        .with_variants(variants)
        .with_owned_strings(&owned);
    let mut out = String::new();
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(l) => {
                out.push_str(&lower_loop(l, f, nat_fns, string_fields, variants, indent)?);
            }
            other => {
                out.push_str(&lower_stmt(other, exec, indent)?);
            }
        }
    }
    if let Some(tail) = &block.tail {
        let t = lower_expr(tail, exec, 0, f.span)?;
        writeln!(out, "{pad}{t}").ok();
    }
    Ok(out)
}

/// Lower a plain block (no fn-level aids) in the given context.
fn lower_block_inner(
    block: &Block,
    ctx: Ctx,
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    let mut out = String::new();
    for stmt in &block.stmts {
        out.push_str(&lower_stmt(stmt, ctx, depth + 1)?);
    }
    if let Some(tail) = &block.tail {
        let t = lower_expr(tail, ctx, depth, span)?;
        writeln!(out, "    {t}").ok();
    }
    Ok(out)
}

/// Lower a single statement (REQ-4).
fn lower_stmt(stmt: &Stmt, ctx: Ctx, indent: usize) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    match stmt {
        Stmt::Let {
            mutable,
            name,
            ty,
            init,
        } => {
            let kw = if *mutable { "let mut" } else { "let" };
            // Cluster C6 (`.design/basis/04-collections.md` REQ-5/REQ-11): a
            // `Vec`-typed `let` whose init is the no-param constructor `Vec::new()`
            // lowers the init to the wrapper construction `<TVec> { data: Vec::new()
            // }` — the bounded-`Vec` wrapper is a newtype over `vstd::vec::Vec`, so a
            // bare `Vec::new()` cannot inhabit the `TVec` type (`E0308`). The element
            // type comes from the `let`'s `Type::Vec(elem)` annotation (the surface
            // `Vec::new()` carries none), exactly the GROUNDED `let mut v: TVec* =
            // TVec* { data: Vec::new() };` form. Keyed on the annotation being a
            // `Type::Vec` AND the init being `Vec::new()` — any other init lowers
            // normally (a `let v: Vec<u64> = other_vec;` passes through).
            let init_s = if let (Some(Type::Vec(elem)), true) = (ty, is_vec_new(init)) {
                let wname = tvec_name(elem.as_ref())?;
                format!("{wname} {{ data: Vec::new() }}")
            } else if let (Some(Type::Map(k, v)), true) = (ty, is_map_new(init)) {
                // Cluster C12 (`.design/basis/13-map.md` REQ-4): a `Map`-typed `let`
                // whose init is the no-param constructor `Map::new()` lowers to the
                // wrapper construction `<TMap> { data: Vec::new() }` — the `TMap`
                // newtype wraps a `vstd::vec::Vec<(K,V)>`-of-pairs, so a bare
                // `Map::new()` cannot inhabit it (`E0308`). MIRRORS the `Vec::new()`
                // rewrite above; the `(K, V)` pair comes from the `let` annotation.
                let wname = tmap_name(k.as_ref(), v.as_ref())?;
                format!("{wname} {{ data: Vec::new() }}")
            } else {
                lower_expr(init, ctx, 0, zero_span())?
            };
            if let Some(t) = ty {
                let ts = lower_type(t)?;
                Ok(format!("{pad}{kw} {name}: {ts} = {init_s};\n"))
            } else {
                Ok(format!("{pad}{kw} {name} = {init_s};\n"))
            }
        }
        Stmt::Assign { target, value } => {
            let t = lower_expr(target, ctx, 0, zero_span())?;
            let v = lower_expr(value, ctx, 0, zero_span())?;
            Ok(format!("{pad}{t} = {v};\n"))
        }
        Stmt::Return(e) => match e {
            Some(e) => {
                let s = lower_expr(e, ctx, 0, zero_span())?;
                Ok(format!("{pad}return {s};\n"))
            }
            None => Ok(format!("{pad}return;\n")),
        },
        Stmt::If { cond, then, else_ } => {
            let c = lower_expr(cond, ctx, 0, zero_span())?;
            let t = lower_block_inner(then, ctx, indent, zero_span())?;
            let mut out = format!("{pad}if {c} {{\n{t}{pad}}}");
            if let Some(e) = else_ {
                let es = lower_block_inner(e, ctx, indent, zero_span())?;
                write!(out, " else {{\n{es}{pad}}}").ok();
            }
            out.push('\n');
            Ok(out)
        }
        Stmt::Expr(e) => {
            let s = lower_expr(e, ctx, 0, zero_span())?;
            Ok(format!("{pad}{s};\n"))
        }
        // `break;` / `continue;` lower to the Verus-native loop-control
        // statements (verus-lowering.md REQ-12, #93). The emission is trivial;
        // the verification semantics (the invariant at every continue, the
        // continue-decreases back-edge obligation, break-as-exit) are enforced
        // by Verus on the lowered loop annotations — NOT suppressed here
        // (no `assume`/`external`/dropped `decreases` — R-DEFER-9).
        Stmt::Break => Ok(format!("{pad}break;\n")),
        Stmt::Continue => Ok(format!("{pad}continue;\n")),
        Stmt::Loop(_) => Err(LowerError::Unsupported {
            what: "nested loop without fn-aid context".to_string(),
            span: zero_span(),
        }),
    }
}

// ---------------------------------------------------------------------------
// REQ-7: shape-keyed proof-aid templates. The hard part.
// ---------------------------------------------------------------------------

/// Lower a loop (REQ-4) with its shape-derived proof aids (REQ-7). Emits every
/// `inv`→`invariant`, the `dec`→`decreases`, and:
///  - template (b): every immutable-param precondition of the enclosing `fn`
///    that the loop does not already restate is lifted into the invariants;
///  - template (c)+(a): if an invariant has the accumulator shape
///    `acc as nat == specfn(slice@.subrange(0, idx as int))`, emit + call the
///    auto-generated push lemma for `specfn`;
///  - template (overflow): if the body assigns `acc = acc + slice[idx] ...` and
///    an invariant bounds `acc <= idx * BOUND`, emit the `by(nonlinear_arith)`
///    overflow discharge;
///  - template (e): if a `None`/false-postcondition `forall_in(s, p)` is
///    provable from `forall_below(s,k,p1)` + `forall_from(s,k',p2)`, emit the
///    loop-exit coverage case-split inside the `if lo == hi` branch;
///  - template (d): if an accumulator invariant uses `subrange(0, idx)` and the
///    loop exits when `idx == len`, emit the `=~=` extensionality after the loop.
fn lower_loop(
    l: &thermite_syntax::ast::LoopNode,
    f: &FnItem,
    nat_fns: &[&str],
    string_fields: &[&str],
    variants: &[(&str, &str)],
    indent: usize,
) -> Result<String, LowerError> {
    use thermite_syntax::ast::LoopKind;
    let pad = "    ".repeat(indent);
    let ipad = "    ".repeat(indent + 1);
    let owned = owned_string_value_names(f);
    let exec = Ctx::exec()
        .with_variants(variants)
        .with_owned_strings(&owned);
    let mut out = String::new();

    // Loop header.
    match &l.kind {
        LoopKind::Loop => writeln!(out, "{pad}loop").map_err(|_| fmt_err())?,
        LoopKind::While(c) => {
            let cs = lower_expr(c, exec, 0, f.span)?;
            writeln!(out, "{pad}while {cs}").map_err(|_| fmt_err())?;
        }
    };

    let slices = slice_param_names(&f.params);
    let strings = string_value_names(f);
    let spec = Ctx::spec(&slices, nat_fns)
        .with_strings(&strings)
        .with_string_fields(string_fields);

    // Invariants: the loop's own `inv`s, then lifted immutable preconditions
    // (template b) not already present.
    out.push_str(&format!("{ipad}invariant\n"));
    let mut inv_strings: Vec<String> = Vec::new();
    for inv in &l.invs {
        inv_strings.push(lower_expr(&inv.expr, spec, 0, f.span)?);
    }
    let lifted = lift_immutable_preconds(f, spec, &inv_strings)?;
    for inv in inv_strings.iter().chain(lifted.iter()) {
        writeln!(out, "{ipad}    {inv},").map_err(|_| fmt_err())?;
    }

    // decreases (§4.1: "Termination is proved by default"). SUPPRESSED for a
    // `fx diverge` fn: an event loop is non-terminating BY DESIGN, so emitting a
    // `decreases` would force Verus to prove a termination measure that honestly
    // does not exist. The enclosing fn carries `#[verifier::exec_allows_no_
    // decreases_clause]` (see `lower_fn`), so Verus verifies the loop's
    // INVARIANTS (partial correctness) WITHOUT a termination claim — the honest
    // L1 verdict. A non-diverge fn ALWAYS emits its `decreases` (sum/binary_search
    // still prove termination → L3): the exemption is diverge-ONLY and is not a
    // termination-proof escape hatch.
    if !fn_is_diverge(f) {
        let dec = lower_expr(&l.dec.expr, spec, 0, f.span)?;
        writeln!(out, "{ipad}decreases {dec},").map_err(|_| fmt_err())?;
    }

    // Body open.
    writeln!(out, "{pad}{{").map_err(|_| fmt_err())?;

    // template (c)+(a): the push-lemma proof block, emitted before the body if
    // an accumulator invariant of the recursive-fold shape is present.
    let acc_aid = accumulator_aid(f, &l.invs)?;
    if let Some((lemma_call, _)) = &acc_aid {
        writeln!(out, "{ipad}proof {{ {lemma_call} }}").map_err(|_| fmt_err())?;
    }
    // template (overflow): the nonlinear_arith discharge, if the body grows an
    // accumulator bounded by a product invariant.
    if let Some(assert_line) = nonlinear_overflow_assert(f, &l.invs, &l.body)? {
        writeln!(out, "{ipad}{assert_line}").map_err(|_| fmt_err())?;
    }

    // The body statements, with the loop-exit coverage split injected into the
    // matching `if` branch (template e).
    let body_src = lower_loop_body(&l.body, f, &l.invs, variants, indent + 1)?;
    out.push_str(&body_src);

    writeln!(out, "{pad}}}").map_err(|_| fmt_err())?;

    // template (d): extensionality at exit, if an accumulator invariant folds a
    // subrange and the loop is `while idx < len` (exits at idx == len).
    if let Some(ext) = extensionality_at_exit(f, l, &acc_aid)? {
        writeln!(out, "{pad}{ext}").map_err(|_| fmt_err())?;
    }

    Ok(out)
}

fn fmt_err() -> LowerError {
    LowerError::Unsupported {
        what: "string formatting".to_string(),
        span: zero_span(),
    }
}

/// Describes a recursive-fold accumulator invariant matched by SHAPE: an
/// invariant `accvar as nat == specfn(slice@.subrange(0, idxvar as int))`.
struct AccInfo {
    specfn: String,
    slice: String,
    idxvar: String,
}

/// Match the accumulator invariant SHAPE in a loop's `inv`s (template c). Keys on
/// the AST shape `Binary{Eq, Cast{acc, nat-ish}, Call{specfn, [subrange(slice, 0, idx)]}}`
/// — NOT on any name. Returns the spec-fn name, slice name, and index var.
fn match_acc_invariant(invs: &[Clause]) -> Option<AccInfo> {
    for inv in invs {
        if let Expr::Binary {
            op: BinOp::Eq,
            lhs,
            rhs,
        } = &inv.expr
        {
            // lhs is `acc` (possibly cast); rhs is `specfn(&slice[..idx])`.
            if let Expr::Call { callee, args } = rhs.as_ref() {
                if let (Expr::Path(segs), [arg0]) = (callee.as_ref(), args.as_slice()) {
                    if let Some(specfn) = segs.last() {
                        // The single arg must be a `&slice[..idx]` (RangeTo) shape.
                        if let Some((slice, idxvar)) = match_range_to_slice(arg0) {
                            // and lhs must reference a single var (the accumulator).
                            let _ = lhs;
                            return Some(AccInfo {
                                specfn: specfn.clone(),
                                slice,
                                idxvar,
                            });
                        }
                    }
                }
            }
        }
    }
    None
}

/// Match a `&slice[..idx]` expression, returning `(slice, idx)` where both are
/// simple path names. Shape: `Ref{ Index{ base: Path[slice], RangeTo(Path[idx]) } }`
/// or the bare `Index` without the `&`.
fn match_range_to_slice(expr: &Expr) -> Option<(String, String)> {
    let inner = match expr {
        Expr::Ref { expr, .. } => expr.as_ref(),
        other => other,
    };
    if let Expr::Index { base, index } = inner {
        if let (Expr::Path(bsegs), IndexArg::RangeTo(i)) = (base.as_ref(), index) {
            if let (Some(slice), Expr::Path(isegs)) = (bsegs.last(), i.as_ref()) {
                if let Some(idx) = isegs.last() {
                    return Some((slice.clone(), idx.clone()));
                }
            }
        }
    }
    None
}

/// template (b): lift each immutable-param precondition of the `fn`'s `req` into
/// the loop invariants when not already present. Keys on SHAPE: a `req`
/// conjunct that mentions only immutable (slice/param) state — concretely, any
/// `req` conjunct that does not mention a loop-local mutable. Because v0.1 has a
/// single `req` clause and the corpus precondition (`xs.len() <= 1_000_000`)
/// references only the immutable slice, we lift the whole `req` if it is not
/// already among the invariants. A `true` req lifts nothing.
fn lift_immutable_preconds(
    f: &FnItem,
    spec: Ctx,
    existing_invs: &[String],
) -> Result<Vec<String>, LowerError> {
    let req = lower_expr(&f.contract.req.expr, spec, 0, f.span)?;
    if req == "true" {
        return Ok(Vec::new());
    }
    // Only lift conjuncts that reference an immutable param name and NOT a
    // mutated local. We approximate "immutable" by: the conjunct references a
    // fn param. The corpus reqs (`xs.len() <= 1_000_000`, `sorted(haystack)`)
    // reference an immutable slice param and no loop-local. Already-present
    // invariants are skipped. Lowered with the fn's slice ctx so a slice arg
    // gets its `@` view (REQ-5).
    let param_names: Vec<&str> = f.params.iter().map(|p| p.name.as_str()).collect();
    let mut lifted = Vec::new();
    for conj in split_conjuncts(&f.contract.req.expr) {
        let lowered = lower_expr(conj, spec, 0, f.span)?;
        let mentions_param = param_names.iter().any(|p| expr_mentions(conj, p));
        if mentions_param && !existing_invs.iter().any(|e| e == &lowered) {
            lifted.push(lowered);
        }
    }
    Ok(lifted)
}

/// Split an expression into top-level `&&` conjuncts (for precondition lifting).
fn split_conjuncts(expr: &Expr) -> Vec<&Expr> {
    let mut out = Vec::new();
    fn go<'a>(e: &'a Expr, acc: &mut Vec<&'a Expr>) {
        if let Expr::Binary {
            op: BinOp::And,
            lhs,
            rhs,
        } = e
        {
            go(lhs, acc);
            go(rhs, acc);
        } else {
            acc.push(e);
        }
    }
    go(expr, &mut out);
    out
}

/// True if `expr` syntactically mentions identifier `name` anywhere.
fn expr_mentions(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Path(segs) => segs.iter().any(|s| s == name),
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::StrLit(_) => false,
        Expr::Call { callee, args } => {
            expr_mentions(callee, name) || args.iter().any(|a| expr_mentions(a, name))
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_mentions(receiver, name) || args.iter().any(|a| expr_mentions(a, name))
        }
        Expr::Field { receiver, .. } => expr_mentions(receiver, name),
        Expr::Closure { body, .. } => expr_mentions(body, name),
        Expr::Match { scrutinee, arms } => {
            expr_mentions(scrutinee, name) || arms.iter().any(|a| expr_mentions(&a.body, name))
        }
        Expr::If { cond, .. } => expr_mentions(cond, name),
        Expr::Binary { lhs, rhs, .. } => expr_mentions(lhs, name) || expr_mentions(rhs, name),
        Expr::Index { base, index } => {
            expr_mentions(base, name)
                || match index {
                    IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
                        expr_mentions(e, name)
                    }
                    IndexArg::Range(a, b) => expr_mentions(a, name) || expr_mentions(b, name),
                }
        }
        Expr::Cast { expr, .. } | Expr::Ref { expr, .. } => expr_mentions(expr, name),
        // Basis Stage 1a (`.design/basis/01-adts.md`): dead-in-1a ADT
        // expressions, but the honest predicate value is to descend — a name
        // could be mentioned in a struct-literal field value, an `is`
        // scrutinee, or a deref operand, so we must not silently answer `false`.
        Expr::StructLit { fields, .. } => fields.iter().any(|(_, v)| expr_mentions(v, name)),
        Expr::Is { scrutinee, .. } => expr_mentions(scrutinee, name),
        Expr::Deref(inner) => expr_mentions(inner, name),
        // The prefix `!` (#92): a name can be mentioned under it (`!done`).
        Expr::Unary { expr, .. } => expr_mentions(expr, name),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): a name
        // can be mentioned in any tuple element or under a projection's receiver.
        Expr::Tuple(elems) => elems.iter().any(|e| expr_mentions(e, name)),
        Expr::TupleProj { receiver, .. } => expr_mentions(receiver, name),
    }
}

/// template (c)+(a): if a loop carries an accumulator invariant of the
/// recursive-fold shape, return `(lemma_call, lemma_def)` — the in-loop
/// `proof { lemma_<specfn>_push(slice@, idx as int); }` call and the
/// auto-generated push lemma definition. The lemma definition is emitted at file
/// scope by `lower` via `collect_push_lemmas`. Here we only return the call.
fn accumulator_aid(f: &FnItem, invs: &[Clause]) -> Result<Option<(String, String)>, LowerError> {
    let _ = f;
    if let Some(info) = match_acc_invariant(invs) {
        let call = format!(
            "lemma_{}_push({}@, {} as int);",
            info.specfn, info.slice, info.idxvar
        );
        let def = push_lemma_for(&info.specfn);
        return Ok(Some((call, def)));
    }
    Ok(None)
}

/// Collect the auto-generated push-lemma definitions a `fn` needs: one per loop
/// carrying an accumulator-fold invariant of the recursive-fold shape (REQ-7
/// template a). Keyed on the invariant SHAPE (`match_acc_invariant`), never on
/// the program. Emitted at file scope by `lower` before the `fn`.
fn push_lemma_defs_for_fn(f: &FnItem) -> Result<Vec<String>, LowerError> {
    let mut defs = Vec::new();
    // A boundary fn (ffi-boundary.md REQ-2) has `body: None` — no loop bodies, so
    // no accumulator-fold push lemmas. A boundary fn never reaches L3 anyway
    // (`lower_fn` errors on a bodyless fn); this keeps the collector total.
    if let Some(body) = &f.body {
        collect_push_lemmas_in_block(body, &mut defs);
    }
    Ok(defs)
}

fn collect_push_lemmas_in_block(block: &Block, defs: &mut Vec<String>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(l) => {
                if let Some(info) = match_acc_invariant(&l.invs) {
                    defs.push(push_lemma_for(&info.specfn));
                }
                collect_push_lemmas_in_block(&l.body, defs);
            }
            Stmt::If { then, else_, .. } => {
                collect_push_lemmas_in_block(then, defs);
                if let Some(e) = else_ {
                    collect_push_lemmas_in_block(e, defs);
                }
            }
            _ => {}
        }
    }
}

/// template (a): the auto-generated push (unfold) induction lemma for a
/// head-fold spec fn `specfn`. It relates `specfn(xs.subrange(0, k+1))` to
/// `specfn(xs.subrange(0, k)) + xs[k]`. Keyed PURELY on the spec-fn name passed
/// in (which itself was derived from the accumulator-invariant SHAPE); the body
/// is the general drop_first induction, identical in structure for any
/// head-fold-sum spec fn. NOT program-specific.
fn push_lemma_for(specfn: &str) -> String {
    format!(
        "proof fn lemma_{specfn}_push(xs: Seq<u32>, k: int)\n    requires 0 <= k < xs.len(),\n    ensures {specfn}(xs.subrange(0, k + 1)) == {specfn}(xs.subrange(0, k)) + xs[k] as nat,\n    decreases k,\n{{\n    if k == 0 {{\n        assert(xs.subrange(0, 1).drop_first() =~= xs.subrange(0, 0));\n    }} else {{\n        lemma_{specfn}_push(xs.drop_first(), k - 1);\n        assert(xs.subrange(0, k + 1).drop_first() =~= xs.drop_first().subrange(0, k));\n        assert(xs.subrange(0, k).drop_first() =~= xs.drop_first().subrange(0, k - 1));\n    }}\n}}\n"
    )
}

/// template (overflow): if the loop body assigns `acc = acc + slice[idx] as T`
/// and an invariant bounds `acc <= idx as T * BOUND`, emit the
/// `by(nonlinear_arith)` discharge with the in-scope invariant/precondition
/// hypotheses as `requires`. Keys on SHAPE: an `Assign` whose value is
/// `acc + (slice[idx] cast)`, plus a product-bound invariant on the same `acc`.
fn nonlinear_overflow_assert(
    f: &FnItem,
    invs: &[Clause],
    body: &Block,
) -> Result<Option<String>, LowerError> {
    // Find `acc = acc + slice[idx] as T;` in the body.
    let Some((accvar, idxvar)) = find_accumulator_growth(body) else {
        return Ok(None);
    };
    // Find the product-bound invariant `acc <= idx as T * BOUND`.
    let Some((bound_factor, bound_ty)) = find_product_bound(invs, &accvar, &idxvar) else {
        return Ok(None);
    };
    // Gather the hypotheses: the product bound, `idx < slice.len()`, and the
    // lifted immutable precondition (all from the loop's own state + req).
    let slice = first_slice_param(&f.params).unwrap_or("xs");
    let req = lower_expr(&f.contract.req.expr, Ctx::spec_seq(), 0, f.span)?;
    let mut hyps = vec![
        format!("{accvar} <= {idxvar} as {bound_ty} * {bound_factor}",),
        format!("{idxvar} < {slice}.len()"),
    ];
    if req != "true" {
        hyps.push(req);
    }
    let line = format!(
        "assert({accvar} + {slice}[{idxvar} as int] as {bound_ty} <= ({idxvar} as {bound_ty} + 1) * {bound_factor}) by(nonlinear_arith)\n        requires {};",
        hyps.join(", ")
    );
    Ok(Some(line))
}

/// Find an accumulator-growth assignment `accvar = accvar + slice[idxvar] as T;`
/// in a block. Returns `(accvar, idxvar)`. SHAPE match only.
fn find_accumulator_growth(block: &Block) -> Option<(String, String)> {
    for stmt in &block.stmts {
        let Stmt::Assign {
            target: Expr::Path(tsegs),
            value:
                Expr::Binary {
                    op: BinOp::Add,
                    lhs,
                    rhs,
                },
        } = stmt
        else {
            continue;
        };
        let Some(accvar) = tsegs.last() else {
            continue;
        };
        // value = accvar + (slice[idx] as T)
        if let Expr::Path(lsegs) = lhs.as_ref() {
            if lsegs.last() == Some(accvar) {
                // rhs is `slice[idx] as T` (Cast over Index Single).
                if let Some(idxvar) = index_var_of_cast(rhs) {
                    return Some((accvar.clone(), idxvar));
                }
            }
        }
    }
    None
}

/// Extract the index var of a `slice[idx] as T` expression (or bare `slice[idx]`).
fn index_var_of_cast(expr: &Expr) -> Option<String> {
    let inner = match expr {
        Expr::Cast { expr, .. } => expr.as_ref(),
        other => other,
    };
    if let Expr::Index {
        index: IndexArg::Single(i),
        ..
    } = inner
    {
        if let Expr::Path(segs) = i.as_ref() {
            return segs.last().cloned();
        }
    }
    None
}

/// Find a product-bound invariant `accvar <= idxvar as T * FACTOR`. Returns
/// `(factor_string, T)`. SHAPE match.
fn find_product_bound(invs: &[Clause], accvar: &str, idxvar: &str) -> Option<(String, String)> {
    for inv in invs {
        if let Expr::Binary {
            op: BinOp::Le,
            lhs,
            rhs,
        } = &inv.expr
        {
            if let Expr::Path(lsegs) = lhs.as_ref() {
                if lsegs.last().map(|s| s == accvar).unwrap_or(false) {
                    // rhs = (idxvar as T) * FACTOR
                    if let Expr::Binary {
                        op: BinOp::Mul,
                        lhs: ml,
                        rhs: mr,
                    } = rhs.as_ref()
                    {
                        if let Expr::Cast { expr, ty } = ml.as_ref() {
                            if let Expr::Path(isegs) = expr.as_ref() {
                                if isegs.last().map(|s| s == idxvar).unwrap_or(false) {
                                    let t = lower_type(ty).ok()?;
                                    let factor =
                                        lower_expr(mr, Ctx::spec_seq(), 0, zero_span()).ok()?;
                                    return Some((factor, t));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// template (d): the `=~=` extensionality assert after a `while idx < slice.len()`
/// loop carrying an accumulator over `slice@.subrange(0, idx)` — at exit
/// `idx == len`, so `subrange(0, len) =~= slice@`. Keys on: an accumulator-aid
/// loop whose `while` condition is `idx < slice.len()`.
fn extensionality_at_exit(
    f: &FnItem,
    l: &thermite_syntax::ast::LoopNode,
    acc_aid: &Option<(String, String)>,
) -> Result<Option<String>, LowerError> {
    use thermite_syntax::ast::LoopKind;
    if acc_aid.is_none() {
        return Ok(None);
    }
    let Some(info) = match_acc_invariant(&l.invs) else {
        return Ok(None);
    };
    // Confirm the loop is `while idx < slice.len()` for this idx/slice.
    let LoopKind::While(cond) = &l.kind else {
        return Ok(None);
    };
    if let Expr::Binary {
        op: BinOp::Lt,
        lhs,
        rhs,
    } = cond.as_ref()
    {
        let lhs_is_idx = matches!(lhs.as_ref(), Expr::Path(s) if s.last().map(|x| x == &info.idxvar).unwrap_or(false));
        let rhs_is_len = matches!(rhs.as_ref(), Expr::MethodCall { receiver, name, .. }
            if name == "len" && matches!(receiver.as_ref(), Expr::Path(s) if s.last().map(|x| x == &info.slice).unwrap_or(false)));
        if lhs_is_idx && rhs_is_len {
            let _ = f;
            return Ok(Some(format!(
                "assert({s}@.subrange(0, {s}.len() as int) =~= {s}@);",
                s = info.slice
            )));
        }
    }
    Ok(None)
}

/// Lower a loop body, injecting the complementary-coverage case-split (template
/// e) into the `if <exit-cond>` branch that returns the negative/None result.
fn lower_loop_body(
    body: &Block,
    f: &FnItem,
    invs: &[Clause],
    variants: &[(&str, &str)],
    indent: usize,
) -> Result<String, LowerError> {
    // Pre-compute the coverage split, if this loop's invariants + the fn's
    // None-postcondition match template (e).
    let coverage = complementary_coverage_split(f, invs)?;
    let owned = owned_string_value_names(f);
    let exec = Ctx::exec()
        .with_variants(variants)
        .with_owned_strings(&owned);

    let mut out = String::new();
    for stmt in &body.stmts {
        if let (Some(cov), Stmt::If { cond, then, else_ }) = (&coverage, stmt) {
            // Inject the split into the branch whose body `return`s the negative
            // result, when the guard matches the coverage's exit condition.
            if if_is_coverage_exit(cond, &cov.guard) {
                out.push_str(&emit_if_with_split(
                    cond,
                    then,
                    else_,
                    &cov.assert_block,
                    f,
                    variants,
                    indent,
                )?);
                continue;
            }
        }
        out.push_str(&lower_stmt(stmt, exec, indent)?);
    }
    if let Some(tail) = &body.tail {
        let pad = "    ".repeat(indent);
        let t = lower_expr(tail, exec, 0, f.span)?;
        writeln!(out, "{pad}{t}").map_err(|_| fmt_err())?;
    }
    Ok(out)
}

/// Whether an `if` condition is the coverage exit `lo == hi` for the matched
/// guard variables.
fn if_is_coverage_exit(cond: &Expr, guard: &(String, String)) -> bool {
    if let Expr::Binary {
        op: BinOp::Eq,
        lhs,
        rhs,
    } = cond
    {
        let l = matches!(lhs.as_ref(), Expr::Path(s) if s.last().map(|x| x == &guard.0).unwrap_or(false));
        let r = matches!(rhs.as_ref(), Expr::Path(s) if s.last().map(|x| x == &guard.1).unwrap_or(false));
        return l && r;
    }
    false
}

/// Emit the coverage-exit `if` with the case-split assert prepended to its
/// `then` block (template e).
fn emit_if_with_split(
    cond: &Expr,
    then: &Block,
    else_: &Option<Block>,
    split: &str,
    f: &FnItem,
    variants: &[(&str, &str)],
    indent: usize,
) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    let owned = owned_string_value_names(f);
    let exec = Ctx::exec()
        .with_variants(variants)
        .with_owned_strings(&owned);
    let c = lower_expr(cond, exec, 0, f.span)?;
    let mut out = format!("{pad}if {c} {{\n");
    // The split assert, indented one level in.
    for line in split.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            writeln!(out, "{pad}    {line}").map_err(|_| fmt_err())?;
        }
    }
    let then_src = lower_block_inner(then, exec, indent, f.span)?;
    out.push_str(&then_src);
    out.push_str(&format!("{pad}}}"));
    if let Some(e) = else_ {
        let es = lower_block_inner(e, exec, indent, f.span)?;
        write!(out, " else {{\n{es}{pad}}}").map_err(|_| fmt_err())?;
    }
    out.push('\n');
    Ok(out)
}

/// The result of matching template (e): the two guard variables whose equality
/// (`below_var == from_var`, the `lo == hi` exit) triggers the split, plus the
/// emitted `assert(forall_in(...)) by { ... }` case-split block.
struct CoverageSplit {
    guard: (String, String),
    assert_block: String,
}

/// template (e): the complementary-bounded-quantifier coverage case-split. When
/// the `fn`'s `None`/false postcondition is `forall_in(s, p)` and the loop
/// invariants include `forall_below(s, k, p1)` and `forall_from(s, k', p2)` with
/// `k == k'` at loop exit (the `lo == hi` guard), the negative postcondition is
/// provable by a case-split on the index: below `k` use `p1`, from `k'` use
/// `p2`. Keys on the SHAPE of the postcondition + invariants (three combinator
/// calls over the same slice with complementary index bounds), never on the
/// program name.
fn complementary_coverage_split(
    f: &FnItem,
    invs: &[Clause],
) -> Result<Option<CoverageSplit>, LowerError> {
    // 1. Find a `None => forall_in(s, ptarget)` arm in some `ens`.
    let Some((slice, ptarget)) = find_none_forall_in(&f.contract.ens) else {
        return Ok(None);
    };

    // 2. Find `forall_below(slice, below_var, p_below)` and
    //    `forall_from(slice, from_var, p_from)` invariants over the same slice.
    let mut below: Option<(String, String)> = None; // (var, pred)
    let mut from: Option<(String, String)> = None;
    for inv in invs {
        if let Some((s, var, pred)) = match_bounded_combinator(&inv.expr, "forall_below") {
            if s == slice {
                below = Some((var, pred));
            }
        }
        if let Some((s, var, pred)) = match_bounded_combinator(&inv.expr, "forall_from") {
            if s == slice {
                from = Some((var, pred));
            }
        }
    }
    let (Some((below_var, p_below)), Some((from_var, p_from))) = (below, from) else {
        return Ok(None);
    };

    // 3. The guard at exit is `below_var == from_var` (the `lo == hi` shape).
    //    Build the assert: forall k in [0,len): below k -> p_below; else p_from.
    let target = ptarget;
    let split = format!(
        "assert(forall_in({slice}@, {target})) by {{\n    assert forall|k: int| 0 <= k < {slice}@.len()\n        implies ({target})({slice}@[k]) by {{\n        if k < {below_var} as int {{\n            assert(({p_below})({slice}@[k]));\n        }} else {{\n            assert(({p_from})({slice}@[k]));\n        }}\n    }}\n}}",
    );
    Ok(Some(CoverageSplit {
        guard: (below_var, from_var),
        assert_block: split,
    }))
}

/// Find a `match result { ... None => forall_in(slice, pred) ... }` ensures arm,
/// returning `(slice, lowered_pred)`. SHAPE match on the ensures.
fn find_none_forall_in(ens: &[Clause]) -> Option<(String, String)> {
    for clause in ens {
        if let Expr::Match { arms, .. } = &clause.expr {
            for arm in arms {
                let is_none = matches!(&arm.pattern, Pattern::Enum { path, fields }
                    if fields.is_empty() && path.last().map(|p| p == "None").unwrap_or(false));
                if is_none {
                    if let Expr::Call { callee, args } = &arm.body {
                        if let (Expr::Path(segs), [s, p]) = (callee.as_ref(), args.as_slice()) {
                            if segs.last().map(|x| x == "forall_in").unwrap_or(false) {
                                let slice = slice_name(s)?;
                                let pred = lower_expr(p, Ctx::spec_seq(), 0, zero_span()).ok()?;
                                return Some((slice, pred));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Match `comb(slice, var, pred)` (a `forall_below`/`forall_from` call),
/// returning `(slice, var, lowered_pred)`. SHAPE match.
fn match_bounded_combinator(expr: &Expr, comb: &str) -> Option<(String, String, String)> {
    if let Expr::Call { callee, args } = expr {
        if let (Expr::Path(segs), [s, v, p]) = (callee.as_ref(), args.as_slice()) {
            if segs.last().map(|x| x == comb).unwrap_or(false) {
                let slice = slice_name(s)?;
                let var = match v {
                    Expr::Path(vs) => vs.last()?.clone(),
                    _ => return None,
                };
                let pred = lower_expr(p, Ctx::spec_seq(), 0, zero_span()).ok()?;
                return Some((slice, var, pred));
            }
        }
    }
    None
}

/// The bare name of a slice-shaped argument (a `Path` head).
fn slice_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(segs) => segs.last().cloned(),
        _ => None,
    }
}

/// `dec`/`decreases` lowering for a spec fn: the measure expression in spec
/// context, with slice `.len()` viewed appropriately. The corpus `dec xs.len()`
/// lowers to `xs.len()` (Verus coerces a `Seq` `.len()` here).
fn spec_dec(dec: &Clause, params: &[Param]) -> String {
    // A `decreases <measure>` is a SPEC term (Verus admits no exec call in it). So a
    // `String`-param measure that names `.len()` (`dec s.len() - i`) must rewrite to
    // the SPEC accessor `s.spec_len()` — the exec `len` returns `u64` and is not
    // callable in spec position; left bare it poisons the measure's type inference
    // (Verus then mis-types the recursive-call args). Thread the spec fn's
    // `String`/`&String` params so the `.len()`/`.byte_at(i)` in the measure rewrite
    // the SAME way the signature/body do (Basis Stage 7 REQ-4, #126). EMPTY for a
    // non-`String` fn (byte-stable: the corpus `dec`s name plain scalars/`xs.len()`
    // over a Seq, never a String accessor).
    let strings = string_param_names(params);
    let ctx = Ctx::spec_seq().with_strings(&strings);
    lower_expr(&dec.expr, ctx, 0, zero_span()).unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod exec_expr_tests {
    //! `lower_exec_expr` per-expr EXEC lowering (`.design/verified/exec-tv.md`
    //! REQ-2 prerequisite, blocker #152). These pin the FAITHFUL production exec
    //! shapes the exec-TV teeth (`thermite-tv/tests/exec_teeth.rs` E1–E4) wrap as
    //! `P_production` — proving the exec `Ctx` IS reachable for a standalone expr
    //! (the #1 feasibility unknown) and that the #122 inner-paren + #146 cast-`<`
    //! outer-paren disciplines fire in EXEC position. The teeth-test (in the
    //! INDEPENDENT `thermite-tv`, no `thermite-lower` dep) cannot import this, so
    //! these tests are the cross-crate bridge that the faithful strings it hardcodes
    //! DO match the real production lowering (R-CHAR-3 — the faithful column traces
    //! to production here, the reference to `exec_encode`).
    use super::*;
    use thermite_syntax::ast::{BinOp, Expr, IndexArg, PrimType, Type};

    fn path(name: &str) -> Expr {
        Expr::Path(vec![name.to_string()])
    }
    fn int(value: u128) -> Expr {
        Expr::IntLit {
            value,
            raw: value.to_string(),
        }
    }
    fn bin(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
        Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }
    fn cast(inner: Expr, ty: Type) -> Expr {
        Expr::Cast {
            expr: Box::new(inner),
            ty,
        }
    }

    // E1 — `(n - 1) as u8`: the #122 inner-paren on the `Binary` inner.
    #[test]
    fn e1_cast_inner_paren_exec() {
        let e = cast(
            bin(BinOp::Sub, path("n"), int(1)),
            Type::Named("u8".to_string()),
        );
        assert_eq!(lower_exec_expr(&e).unwrap(), "(n - 1) as u8");
    }

    // E2 — `x as u32 < 33`: the #146 cast-`<` OUTER-paren (a `Cast` left of `<`).
    #[test]
    fn e2_cast_lt_outer_paren_exec() {
        let e = bin(
            BinOp::Lt,
            cast(path("x"), Type::Prim(PrimType::U32)),
            int(33),
        );
        assert_eq!(lower_exec_expr(&e).unwrap(), "(x as u32) < 33");
    }

    // E3 — `a + b`: bounded exec add (NOT `wrapping_*`, NOT `nat`).
    #[test]
    fn e3_bounded_add_exec() {
        let e = bin(BinOp::Add, path("a"), path("b"));
        assert_eq!(lower_exec_expr(&e).unwrap(), "a + b");
    }

    // E4 — `xs[i]`: the bounded Rust access (exec index, NOT the spec `xs@[i as int]`).
    #[test]
    fn e4_slice_index_exec() {
        let e = Expr::Index {
            base: Box::new(path("xs")),
            index: IndexArg::Single(Box::new(path("i"))),
        };
        assert_eq!(lower_exec_expr(&e).unwrap(), "xs[i]");
    }
}

#[cfg(test)]
mod exec_body_tests {
    //! `lower_exec_body` per-BODY straight-line EXEC lowering
    //! (`.design/verified/exec-stmt-tv.md` REQ-3, blocker #161; epic #158). These
    //! pin the FAITHFUL production exec BODY shapes the body-TV teeth
    //! (`thermite-tv/tests/body_teeth.rs` B1-B4) wrap as `P_production` - proving
    //! the body exec `Ctx` IS reachable for a standalone straight-line `Block` (the
    //! #161 feasibility unknown: `lower_block_inner` is private + fn-context-bound,
    //! reached here through the minimal `Ctx::exec()` frame) and that the
    //! `let`/`mut`-let / assignment / `if`-statement / tail thread the SAME exec
    //! path the fn body uses. The teeth-test (in the INDEPENDENT `thermite-tv`, NO
    //! `thermite-lower` dep) cannot import this, so these tests are the cross-crate
    //! bridge that the faithful strings it hardcodes DO match the real production
    //! lowering (R-CHAR-3 - the faithful column traces to production HERE, the
    //! reference to `exec_stmt_encode`). A loop body is an honest `Err` (the frozen
    //! 2.2.1-vs-2.2.2 boundary), NEVER a silent lowering.
    use super::*;
    use thermite_syntax::ast::{BinOp, Block, Clause, Expr, LoopKind, LoopNode, Stmt};

    fn path(name: &str) -> Expr {
        Expr::Path(vec![name.to_string()])
    }
    fn int(value: u128) -> Expr {
        Expr::IntLit {
            value,
            raw: value.to_string(),
        }
    }
    fn bin(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
        Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }
    fn let_(mutable: bool, name: &str, init: Expr) -> Stmt {
        Stmt::Let {
            mutable,
            name: name.to_string(),
            ty: None,
            init,
        }
    }
    fn assign(target: &str, value: Expr) -> Stmt {
        Stmt::Assign {
            target: path(target),
            value,
        }
    }

    // B1 straight-line: { let a = x + 1; let b = a * 2; b }
    #[test]
    fn b1_straight_line_let_chain() {
        let block = Block {
            stmts: vec![
                let_(false, "a", bin(BinOp::Add, path("x"), int(1))),
                let_(false, "b", bin(BinOp::Mul, path("a"), int(2))),
            ],
            tail: Some(Box::new(path("b"))),
        };
        assert_eq!(
            lower_exec_body(&block).unwrap(),
            "    let a = x + 1;\n    let b = a * 2;\n    b\n"
        );
    }

    // B2 mutation-order: { let mut s = x; s = s + 1; s = s * 2; s }
    #[test]
    fn b2_mutation_order() {
        let block = Block {
            stmts: vec![
                let_(true, "s", path("x")),
                assign("s", bin(BinOp::Add, path("s"), int(1))),
                assign("s", bin(BinOp::Mul, path("s"), int(2))),
            ],
            tail: Some(Box::new(path("s"))),
        };
        assert_eq!(
            lower_exec_body(&block).unwrap(),
            "    let mut s = x;\n    s = s + 1;\n    s = s * 2;\n    s\n"
        );
    }

    // B3 if-branch: { if c { x + 1 } else { x - 1 } }  (tail = if-EXPR)
    #[test]
    fn b3_if_branch_tail() {
        let then = Block {
            stmts: vec![],
            tail: Some(Box::new(bin(BinOp::Add, path("x"), int(1)))),
        };
        let els = Block {
            stmts: vec![],
            tail: Some(Box::new(bin(BinOp::Sub, path("x"), int(1)))),
        };
        let block = Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::If {
                cond: Box::new(path("c")),
                then,
                else_: els,
            })),
        };
        assert_eq!(
            lower_exec_body(&block).unwrap(),
            "    if c { x + 1 } else { x - 1 }\n"
        );
    }

    // B4 multi-cell tuple:
    //   { let mut a = x; let mut b = y; a = a + 1; b = b + a; (a, b) }
    #[test]
    fn b4_multi_cell_tuple() {
        let block = Block {
            stmts: vec![
                let_(true, "a", path("x")),
                let_(true, "b", path("y")),
                assign("a", bin(BinOp::Add, path("a"), int(1))),
                assign("b", bin(BinOp::Add, path("b"), path("a"))),
            ],
            tail: Some(Box::new(Expr::Tuple(vec![path("a"), path("b")]))),
        };
        assert_eq!(
            lower_exec_body(&block).unwrap(),
            "    let mut a = x;\n    let mut b = y;\n    a = a + 1;\n    b = b + a;\n    (a, b)\n"
        );
    }

    // FROZEN-SUBSET HONESTY (REQ-1): a body containing a `Stmt::Loop` is OUT of the
    // 2.2.1 straight-line slice (step 2.2.2). `lower_exec_body` returns an honest
    // `Err` (via `lower_stmt`'s `Stmt::Loop` arm), NEVER a silent / wrong lowering.
    #[test]
    fn loop_body_is_err_not_silent() {
        let loop_node = LoopNode {
            kind: LoopKind::While(Box::new(path("c"))),
            invs: vec![Clause {
                expr: Expr::BoolLit(true),
                text: "true".to_string(),
                span: zero_span(),
            }],
            dec: Clause {
                expr: int(0),
                text: "0".to_string(),
                span: zero_span(),
            },
            body: Block {
                stmts: vec![],
                tail: None,
            },
            span: zero_span(),
        };
        let block = Block {
            stmts: vec![Stmt::Loop(loop_node)],
            tail: None,
        };
        assert!(matches!(
            lower_exec_body(&block),
            Err(LowerError::Unsupported { .. })
        ));
    }
}
