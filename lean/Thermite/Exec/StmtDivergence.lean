/-
  Thermite/Exec/StmtDivergence.lean — critic divergence pins for increment 2b
  (`Thermite.Exec.Stmt`, commit `3b53d5aa`, crosslink #172, epic #169).

  These are adversarial pins: each `theorem` states what the Rust reference
  encoder `thermite-tv/src/exec_stmt_encode.rs::body_ref_state` produces (the
  authority being modeled), and shows the Lean `S_B` model (`bodyDenote` /
  `bodyRefState`) disagrees with it — a fidelity gap between the mechanized
  state-transformer and the encoder's operational model. A pin compiles
  (the disagreement is proven), so it is a positive obligation that the generator
  must close by making the `ifElse` arm faithful to the encoder's `Stmt::If`
  branch-env clone semantics, rather than a `sorry`/`#[ignore]`.

  Authority: `.design/verified/exec-stmt-tv.md` REQ-1/REQ-2 (the `if`-statement
  state-transformer; "v1 assumes each `let` introduces a distinct name") plus
  `thermite-tv/src/exec_stmt_encode.rs` `thread_stmt`'s `Stmt::If` arm (the
  `then_env = env.clone()` branch-env discipline: a branch-local `let` lives only
  in the branch-env clone and is discarded past the `if`). R-DEFER-9 / R-CHAR-3.
-/
import Thermite.Exec.Stmt

namespace Thermite.Exec

/-! ## Divergence D1 — the `ifElse` state transformer leaks branch-local `let`
    scope past the `if`; `body_ref_state` discards it (branch-env clone).

  Encoder (confirmed against `thermite-tv` at commit `3b53d5aa`): for the
  straight-line body

      { let mut r = x;
        if x < 10 { let k = 1; r = r + k; } else { r = r + 2; }
        let k = 5;
        k }

  `body_ref_state` returns `Ok("5")`, a defined result. The `Stmt::If` arm threads
  each branch into its own `env.clone()` (`then_env`/`else_env`); the branch-local
  `let k = 1` lives only in that clone and is discarded when the `if` recomposes the
  pre-`if` cells (`env.keys()`). So the post-`if` `let k = 5` is a fresh binding
  rather than a re-shadow, and the body has a closed-form tail value `5`.

  Lean model (`Thermite.Exec.Stmt`): the `ifElse` arm of `stmtDenote`/`refStmt` runs
  the taken branch as `blockThread thenB st` and returns the branch's full final
  state, including the `let k`-introduced binding (`stmtDenote (.letS "k" ..)` does
  `(st.setVar "k" v).bind "k"`, marking `scope "k" = true`, and that propagates out
  of the `ifElse`). The post-`if` `let k = 5` then hits the re-shadow guard
  `if st.scope "k" then none`, so `bodyDenote = none`.

  So the Lean models `none` where the encoder produces a defined result: the
  mechanized `S_B` is not faithful to `body_ref_state`'s `Stmt::If` branch-env clone
  scoping. `body_ref_sound` remains internally true (both Lean sides leak the same
  way), but it certifies the wrong state transformer. This is the 1g-style scoping
  fidelity make-or-break, surfaced as a divergence.

  The pin: the encoder produces a result (`Ok`, i.e. the Lean model ought to be
  `(·).isSome`), but the Lean model is `none`. We assert `isSome = true` (the
  authority); it fails because the Lean is `none`. The `decide` below proves the
  Lean model is `none` (the disagreement), so the pin is the proof obligation. -/

/-- The divergent body (`body_ref_state` returns `Ok("5")`). -/
def d1_branch_local_then_rebind : Block :=
  .mk
    [ .letS "r" (.var "x"),
      .ifElse (.cmp .lt (.var "x") (.intLit .u64 10))
        (.mk [ .letS "k" (.intLit .u64 1),
               .assign "r" (.arith .add (.var "r") (.var "k")) ] none)
        (.mk [ .assign "r" (.arith .add (.var "r") (.intLit .u64 2)) ] none),
      .letS "k" (.intLit .u64 5) ]
    (some (.var "k"))

/-- D1a — resolved: the Lean `S_B` model now agrees with `body_ref_state`,
    producing the tail value `5`. The `ifElse` arm threads each branch over the
    pre-`if` state and projects the result back onto the pre-`if` cell/scope set
    (`State.restoreScope`, the encoder's `then_env = env.clone()` plus `env.keys()`
    recomposition). The branch-local `let k = 1` lives only in the branch projection
    and is discarded past the `if`, so the post-`if` `let k = 5` is a fresh bind
    rather than a re-shadow, and the body has the closed-form tail value `5`, matching
    `body_ref_state`'s `Ok("5")`. The fidelity gap is now closed. -/
theorem d1_lean_model_is_none :
    bodyDenote d1_branch_local_then_rebind inputState = some (.int ⟨.u64, 5⟩) := by
  simp only [d1_branch_local_then_rebind, bodyDenote, blockThread, stmtDenote,
        inputState, State.setVar, State.bind, State.restoreScope, execDenote, asInt,
        asBool, evalArith, rawArith, cmpVal, IntTy.bound, IntTy.width]
  decide

/-- D1 — divergence pin (now passes; the divergence is closed). The authority
    (`body_ref_state`, confirmed `Ok("5")` at commit `3b53d5aa`) produces a
    defined result, so a faithful `S_B` model is also defined here:
    `(bodyDenote …).isSome = true`. After the `ifElse` arm threads each branch over a
    scope-restored copy (the encoder's `env.clone()` discipline, under which a
    branch-local `let` does not leak past the `if`), the Lean model has the result `5`
    (`d1_lean_model_is_none`), so `isSome = true` holds. The branch-local-`let`-scope
    -leak fidelity gap is closed. -/
theorem d1_faithful_model_should_have_result :
    (bodyDenote d1_branch_local_then_rebind inputState).isSome = true := by
  rw [d1_lean_model_is_none]
  rfl

end Thermite.Exec
