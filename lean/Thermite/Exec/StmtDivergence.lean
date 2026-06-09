/-
  Thermite/Exec/StmtDivergence.lean — CRITIC divergence pins for increment 2b
  (`Thermite.Exec.Stmt`, commit `3b53d5aa`, crosslink #172, epic #169).

  These are ADVERSARIAL pins: each `theorem` states what the REAL Rust reference
  encoder `thermite-tv/src/exec_stmt_encode.rs::body_ref_state` PRODUCES (the
  authority being modeled), and shows the Lean `S_B` model (`bodyDenote` /
  `bodyRefState`) DISAGREES with it — a FIDELITY gap between the mechanized
  state-transformer and the encoder's actual operational model. A pin compiles
  (the disagreement is PROVEN), so it is a positive obligation that the generator
  must close (by making the `ifElse` arm faithful to the encoder's `Stmt::If`
  branch-env CLONE semantics), NOT a `sorry`/`#[ignore]`.

  Authority: `.design/verified/exec-stmt-tv.md` REQ-1/REQ-2 (the `if`-statement
  state-transformer; "v1 assumes each `let` introduces a distinct name") +
  `thermite-tv/src/exec_stmt_encode.rs` `thread_stmt`'s `Stmt::If` arm (the
  `then_env = env.clone()` branch-env discipline — a branch-local `let` lives only
  in the branch-env clone and is DISCARDED past the `if`). R-DEFER-9 / R-CHAR-3.
-/
import Thermite.Exec.Stmt

namespace Thermite.Exec

/-! ## DIVERGENCE D1 — the `ifElse` state transformer LEAKS branch-local `let`
    scope past the `if`; the REAL `body_ref_state` DISCARDS it (branch-env clone).

  REAL ENCODER (confirmed against `thermite-tv` at commit `3b53d5aa`): for the
  straight-line body

      { let mut r = x;
        if x < 10 { let k = 1; r = r + k; } else { r = r + 2; }
        let k = 5;
        k }

  `body_ref_state` returns `Ok("5")` — a DEFINED result. The `Stmt::If` arm threads
  each branch into its OWN `env.clone()` (`then_env`/`else_env`); the branch-local
  `let k = 1` lives only in that clone and is DISCARDED when the `if` recomposes the
  pre-`if` cells (`env.keys()`). So the post-`if` `let k = 5` is a FRESH binding —
  NOT a re-shadow — and the body has a closed-form tail value `5`.

  LEAN MODEL (`Thermite.Exec.Stmt`): the `ifElse` arm of `stmtDenote`/`refStmt` runs
  the taken branch as `blockThread thenB st` and returns the branch's FULL final
  state, INCLUDING the `let k`-introduced binding (`stmtDenote (.letS "k" ..)` does
  `(st.setVar "k" v).bind "k"`, marking `scope "k" = true`, and that propagates out
  of the `ifElse`). The post-`if` `let k = 5` then hits the re-shadow guard
  `if st.scope "k" then none`, so `bodyDenote = none`.

  So the Lean models `none` where the REAL encoder produces a DEFINED result: the
  mechanized `S_B` is NOT faithful to `body_ref_state`'s `Stmt::If` branch-env CLONE
  scoping. `body_ref_sound` remains internally true (both Lean sides leak the same
  way) — but it certifies the WRONG state transformer. This is the 1g-style scoping
  fidelity make-or-break, surfaced as a divergence.

  THE PIN: the encoder produces a result (`Ok`, i.e. the Lean model OUGHT to be
  `(·).isSome`), but the Lean model is `none`. We assert `isSome = true` (the
  authority) — it FAILS because the Lean is `none`. The `decide` below PROVES the
  Lean model is `none` (the disagreement), so the pin is the proof obligation. -/

/-- The divergent body (the REAL `body_ref_state` returns `Ok("5")`). -/
def d1_branch_local_then_rebind : Block :=
  .mk
    [ .letS "r" (.var "x"),
      .ifElse (.cmp .lt (.var "x") (.intLit .u64 10))
        (.mk [ .letS "k" (.intLit .u64 1),
               .assign "r" (.arith .add (.var "r") (.var "k")) ] none)
        (.mk [ .assign "r" (.arith .add (.var "r") (.intLit .u64 2)) ] none),
      .letS "k" (.intLit .u64 5) ]
    (some (.var "k"))

/-- **D1a — RESOLVED: the Lean `S_B` model now AGREES with the REAL `body_ref_state`,
    producing the tail value `5`.** The `ifElse` arm threads each branch over the
    pre-`if` state and PROJECTS the result back onto the pre-`if` cell/scope set
    (`State.restoreScope` — the encoder's `then_env = env.clone()` + `env.keys()`
    recomposition). The branch-local `let k = 1` lives only in the branch projection
    and is DISCARDED past the `if`, so the post-`if` `let k = 5` is a FRESH bind (NOT
    a re-shadow) and the body has the closed-form tail value `5` — exactly the REAL
    `body_ref_state`'s `Ok("5")`. (This is the fidelity gap, now CLOSED.) -/
theorem d1_lean_model_is_none :
    bodyDenote d1_branch_local_then_rebind inputState = some (.int ⟨.u64, 5⟩) := by
  simp only [d1_branch_local_then_rebind, bodyDenote, blockThread, stmtDenote,
        inputState, State.setVar, State.bind, State.restoreScope, execDenote, asInt,
        asBool, evalArith, rawArith, cmpVal, IntTy.bound, IntTy.width]
  decide

/-- **D1 — DIVERGENCE PIN (now PASSES — the divergence is CLOSED).** The authority
    (the REAL `body_ref_state`, confirmed `Ok("5")` at commit `3b53d5aa`) produces a
    DEFINED result, so a FAITHFUL `S_B` model MUST also be defined here:
    `(bodyDenote …).isSome = true`. After the `ifElse` arm threads each branch over a
    SCOPE-RESTORED copy (the encoder's `env.clone()` discipline — a branch-local `let`
    does not leak past the `if`), the Lean model HAS the result `5`
    (`d1_lean_model_is_none`), so `isSome = true` holds. The branch-local-`let`-scope
    -leak fidelity gap is closed. -/
theorem d1_faithful_model_should_have_result :
    (bodyDenote d1_branch_local_then_rebind inputState).isSome = true := by
  rw [d1_lean_model_is_none]
  rfl

end Thermite.Exec
