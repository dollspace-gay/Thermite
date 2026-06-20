/-
  Thermite/Strat/Graph.lean — the sort graph (E1 ∪ E2), its acyclicity check, and the
  graph-theoretic soundness theorem `acyclic_iff_no_cycle`.

  Governing design: `.design/stage2-stratified-cage.md` REQ-3 / AC-3 (spec of record:
  the stage-2 metatheory sketch, GH issue #2, §3 "the sort graph and the admission
  predicate").

  §3.1, two edge sources, computed from a formula IN NEGATION NORMAL FORM (so every
  binder's polarity is syntactic — `Strat/Nnf.lean`):

  * **E1 (alternation):** `S → T` for every existential binder of sort `T` in the scope
    of a universal binder of sort `S`.
  * **E2 (function flow):** `S → T` for every function occurrence `g : … S … → T`
    whose `S`-position argument contains a universally bound variable — including
    `Read : SeqS T × usize → T` (edge `usize → T`), `Cast` (edge `from → to`), `Len`
    (edge `SeqS T → usize`, inert), and declared spec fns (`app1`, edge `arg → res`).

  THE ACYCLICITY DECISION (core-Lean-only, no Mathlib).  `acyclic` is the Roy–Warshall
  transitive-closure check: `reach allowed a b` is true iff there is a walk `a → b`
  whose internal vertices all lie in `allowed`, computed by the one-vertex-at-a-time
  recursion `reach (v::vs) a b = reach vs a b ∨ (reach vs a v ∧ reach vs v b)`.  Run with
  `allowed = G.nodes` it decides the full transitive closure, and the soundness proof
  `acyclic_iff_no_cycle` is a clean STRUCTURAL induction (the Warshall vertex peel) — no
  pigeonhole / no list-`Nodup` machinery, which is what keeps it Mathlib-free.  The
  declarative cycle notion is the inductive transitive closure `TC` (≡ a `Chain` with an
  explicit internal-vertex list); `reach_iff_TC` ties the two together, using only that
  the graph is well-formed (`Wf`: every edge endpoint is a node).
-/
import Thermite.Strat.Nnf

namespace Thermite.Strat.Cls

/-! ## Graphs over sorts -/

/-- A directed graph over sorts: an explicit node list and edge list. -/
structure Graph where
  nodes : List Sort₂
  edges : List (Sort₂ × Sort₂)
  deriving Repr

/-- Decidable edge membership as a `Bool`. -/
def hasEdge (G : Graph) (a b : Sort₂) : Bool := decide ((a, b) ∈ G.edges)

theorem hasEdge_iff {G : Graph} {a b : Sort₂} : hasEdge G a b = true ↔ (a, b) ∈ G.edges := by
  simp [hasEdge]

/-- Well-formedness: every edge endpoint is a declared node. The Warshall completeness
    direction needs it (a walk's internal vertices must lie in `allowed = nodes`). -/
def Wf (G : Graph) : Prop := ∀ p ∈ G.edges, p.1 ∈ G.nodes ∧ p.2 ∈ G.nodes

/-! ## The declarative cycle notion: transitive closure -/

/-- The transitive closure (length ≥ 1) — the declarative "there is a path". -/
inductive TC (G : Graph) : Sort₂ → Sort₂ → Prop
  | base {a b} : hasEdge G a b = true → TC G a b
  | step {a b c} : hasEdge G a b = true → TC G b c → TC G a c

theorem TC.trans {G a b c} (h1 : TC G a b) (h2 : TC G b c) : TC G a c := by
  induction h1 with
  | base hab => exact TC.step hab h2
  | step hab _ ih => exact TC.step hab (ih h2)

/-- A path with its explicit internal-vertex list — the bridge to the Warshall recursion. -/
inductive Chain (G : Graph) : Sort₂ → Sort₂ → List Sort₂ → Prop
  | nil {a b} : hasEdge G a b = true → Chain G a b []
  | cons {a m b ms} : hasEdge G a m = true → Chain G m b ms → Chain G a b (m :: ms)

theorem TC_of_Chain {G a b ms} (h : Chain G a b ms) : TC G a b := by
  induction h with
  | nil hab => exact TC.base hab
  | cons ham _ ih => exact TC.step ham ih

theorem Chain_of_TC {G a b} (h : TC G a b) : ∃ ms, Chain G a b ms := by
  induction h with
  | @base a b hab => exact ⟨[], Chain.nil hab⟩
  | @step a b c hab _ ih => obtain ⟨ms, hms⟩ := ih; exact ⟨b :: ms, Chain.cons hab hms⟩

/-! ## The Roy–Warshall reachability decision -/

/-- `reach allowed a b` — is there a walk `a → b` with internal vertices ⊆ `allowed`? -/
def reach (G : Graph) : List Sort₂ → Sort₂ → Sort₂ → Bool
  | [],      a, b => hasEdge G a b
  | v :: vs, a, b => reach G vs a b || (reach G vs a v && reach G vs v b)

/-- Soundness: a positive `reach` yields a genuine transitive-closure path. -/
theorem reach_sound {G : Graph} : ∀ (allowed : List Sort₂) (a b : Sort₂),
    reach G allowed a b = true → TC G a b
  | [],      a, b, h => TC.base h
  | v :: vs, a, b, h => by
      simp only [reach, Bool.or_eq_true, Bool.and_eq_true] at h
      rcases h with h | ⟨h1, h2⟩
      · exact reach_sound vs a b h
      · exact TC.trans (reach_sound vs a v h1) (reach_sound vs v b h2)

/-- Cut a chain at the FIRST occurrence of `v`: a prefix `a → v` not re-using `v`. -/
theorem chain_prefix {G : Graph} {a b ms} (h : Chain G a b ms) :
    ∀ v, v ∈ ms → ∃ ms1, Chain G a v ms1 ∧ v ∉ ms1 ∧ (∀ x ∈ ms1, x ∈ ms) := by
  induction h with
  | nil hab => intro v hv; cases hv
  | @cons a m b ms' ham hmb ih =>
      intro v hv
      by_cases hvm : v = m
      · subst hvm
        exact ⟨[], Chain.nil ham, by simp, by intro x hx; cases hx⟩
      · have hv' : v ∈ ms' := (List.mem_cons.mp hv).resolve_left hvm
        obtain ⟨ms1, hc, hni, hsub⟩ := ih v hv'
        refine ⟨m :: ms1, Chain.cons ham hc, ?_, ?_⟩
        · intro hmem
          rcases List.mem_cons.mp hmem with h | h
          · exact hvm h
          · exact hni h
        · intro x hx
          rcases List.mem_cons.mp hx with rfl | h
          · exact List.mem_cons_self ..
          · exact List.mem_cons_of_mem _ (hsub x h)

/-- Cut a chain at the LAST occurrence of `v`: a suffix `v → b` not re-using `v`. -/
theorem chain_suffix {G : Graph} {a b ms} (h : Chain G a b ms) :
    ∀ v, v ∈ ms → ∃ ms2, Chain G v b ms2 ∧ v ∉ ms2 ∧ (∀ x ∈ ms2, x ∈ ms) := by
  induction h with
  | nil hab => intro v hv; cases hv
  | @cons a m b ms' ham hmb ih =>
      intro v hv
      by_cases hvms' : v ∈ ms'
      · obtain ⟨ms2, hc, hni, hsub⟩ := ih v hvms'
        exact ⟨ms2, hc, hni, fun x hx => List.mem_cons_of_mem _ (hsub x hx)⟩
      · have hvm : v = m := (List.mem_cons.mp hv).resolve_right hvms'
        subst hvm
        exact ⟨ms', hmb, hvms', fun x hx => List.mem_cons_of_mem _ hx⟩

/-- Completeness: a chain whose internal vertices lie in `allowed` is found by `reach`. -/
theorem reach_complete {G : Graph} : ∀ (allowed : List Sort₂) (a b : Sort₂) (ms : List Sort₂),
    Chain G a b ms → (∀ x ∈ ms, x ∈ allowed) → reach G allowed a b = true
  | [], a, b, ms, hc, hsub => by
      cases ms with
      | nil => cases hc with | nil hab => simpa [reach] using hab
      | cons x xs => exact absurd (hsub x (List.mem_cons_self ..)) (by simp)
  | v :: vs, a, b, ms, hc, hsub => by
      by_cases hvms : v ∈ ms
      · obtain ⟨ms1, hc1, hni1, hsub1⟩ := chain_prefix hc v hvms
        obtain ⟨ms2, hc2, hni2, hsub2⟩ := chain_suffix hc v hvms
        have h1 : reach G vs a v = true := reach_complete vs a v ms1 hc1 (fun x hx =>
          (List.mem_cons.mp (hsub x (hsub1 x hx))).resolve_left (fun h => hni1 (h ▸ hx)))
        have h2 : reach G vs v b = true := reach_complete vs v b ms2 hc2 (fun x hx =>
          (List.mem_cons.mp (hsub x (hsub2 x hx))).resolve_left (fun h => hni2 (h ▸ hx)))
        simp only [reach, Bool.or_eq_true, Bool.and_eq_true]
        exact Or.inr ⟨h1, h2⟩
      · have hsub' : ∀ x ∈ ms, x ∈ vs := fun x hx =>
          (List.mem_cons.mp (hsub x hx)).resolve_left (fun h => hvms (h ▸ hx))
        simp only [reach, Bool.or_eq_true]
        exact Or.inl (reach_complete vs a b ms hc hsub')

/-- Every internal vertex of a chain in a well-formed graph is a node. -/
theorem chain_mem_nodes {G : Graph} (hwf : Wf G) :
    ∀ {a b ms}, Chain G a b ms → ∀ x ∈ ms, x ∈ G.nodes := by
  intro a b ms h
  induction h with
  | nil hab => intro x hx; cases hx
  | @cons a m b ms' ham hmb ih =>
      intro x hx
      rcases List.mem_cons.mp hx with rfl | hx'
      · exact (hwf (a, x) (hasEdge_iff.mp ham)).2
      · exact ih x hx'

/-- The bridge: over the node set, `reach` decides the transitive closure exactly. -/
theorem reach_iff_TC {G : Graph} (hwf : Wf G) {a b : Sort₂} :
    reach G G.nodes a b = true ↔ TC G a b := by
  constructor
  · exact reach_sound G.nodes a b
  · intro h
    obtain ⟨ms, hms⟩ := Chain_of_TC h
    exact reach_complete G.nodes a b ms hms (chain_mem_nodes hwf hms)

/-! ## Acyclicity -/

/-- The acyclicity check: no node reaches itself. -/
def acyclic (G : Graph) : Bool := G.nodes.all (fun s => ! reach G G.nodes s s)

/-- The declarative cycle: a node with a transitive edge to itself. -/
def HasCycle (G : Graph) : Prop := ∃ s, s ∈ G.nodes ∧ TC G s s

/-- The graph-theoretic soundness theorem: the DFS/Warshall `acyclic` coincides with the
    absence of a transitive-closure cycle. -/
theorem acyclic_iff_no_cycle {G : Graph} (hwf : Wf G) : acyclic G = true ↔ ¬ HasCycle G := by
  simp only [acyclic, List.all_eq_true, HasCycle, not_exists]
  constructor
  · intro h s hcontra
    obtain ⟨hs, htc⟩ := hcontra
    have hr : reach G G.nodes s s = true := (reach_iff_TC hwf).mpr htc
    have hh := h s hs
    rw [hr] at hh
    exact absurd hh (by decide)
  · intro h s hs
    cases hrr : reach G G.nodes s s with
    | false => rfl
    | true => exact absurd ((reach_iff_TC hwf).mp hrr) (fun htc => h s ⟨hs, htc⟩)

/-! ## The sort graph (E1 ∪ E2), by structural recursion on NNF

    A typing context `ctx` records, per de Bruijn binder level (head = innermost), whether
    the binder is universal and its sort. -/

/-- Is de Bruijn index `i` bound by a universal binder in `ctx`? -/
def varUniv (ctx : List (Bool × Sort₂)) (i : Nat) : Bool :=
  match ctx[i]? with
  | some (u, _) => u
  | none        => false

/-- Does a term mention a universally bound variable? -/
def hasUnivVar (ctx : List (Bool × Sort₂)) : Tm → Bool
  | .var _ i      => varUniv ctx i
  | .lit _        => false
  | .read _ sq ix => hasUnivVar ctx sq || hasUnivVar ctx ix
  | .len sq       => hasUnivVar ctx sq
  | .cast _ t     => hasUnivVar ctx t
  | .idxOp t _    => hasUnivVar ctx t
  | .mul t u      => hasUnivVar ctx t || hasUnivVar ctx u
  | .app1 _ _ _ t => hasUnivVar ctx t

/-- The E2 edges contributed by a term: a function occurrence whose `S`-position argument
    contains a universally bound variable, plus the edges of its subterms. -/
def edgesTm (ctx : List (Bool × Sort₂)) : Tm → List (Sort₂ × Sort₂)
  | .var _ _      => []
  | .lit _        => []
  | .read elem sq ix =>
      (if hasUnivVar ctx ix then [(usizeS, elem)] else []) ++ edgesTm ctx sq ++ edgesTm ctx ix
  | .len sq       => (if hasUnivVar ctx sq then [(sq.sortOf, usizeS)] else []) ++ edgesTm ctx sq
  | .cast to t    => (if hasUnivVar ctx t then [(t.sortOf, to)] else []) ++ edgesTm ctx t
  | .idxOp t _    => edgesTm ctx t
  | .mul t u      => edgesTm ctx t ++ edgesTm ctx u
  | .app1 arg res _ t => (if hasUnivVar ctx t then [(arg, res)] else []) ++ edgesTm ctx t

/-- The E2 edges contributed by an atom (the `qfree` leaf is opaque — no sorts/edges). -/
def edgesAtom (ctx : List (Bool × Sort₂)) : Atom → List (Sort₂ × Sort₂)
  | .rel _ t u => edgesTm ctx t ++ edgesTm ctx u
  | .qfree _   => []

/-- The universal sorts currently in scope. -/
def univSorts (ctx : List (Bool × Sort₂)) : List Sort₂ :=
  (ctx.filter (·.1)).map (·.2)

/-- The full edge set (E1 ∪ E2) of a formula under a binder context.  The `ex` case adds
    the E1 alternation edges `S → T` for every enclosing universal sort `S`. -/
def edgesFrm (ctx : List (Bool × Sort₂)) : Frm → List (Sort₂ × Sort₂)
  | .atom a   => edgesAtom ctx a
  | .neg φ    => edgesFrm ctx φ
  | .conj φ ψ => edgesFrm ctx φ ++ edgesFrm ctx ψ
  | .disj φ ψ => edgesFrm ctx φ ++ edgesFrm ctx ψ
  | .imp φ ψ  => edgesFrm ctx φ ++ edgesFrm ctx ψ
  | .all s φ  => edgesFrm ((true, s) :: ctx) φ
  | .ex s φ   => (univSorts ctx).map (fun S => (S, s)) ++ edgesFrm ((false, s) :: ctx) φ

/-- The node set: every endpoint of every edge (so the graph is closed — `Wf`). -/
def nodesOf (es : List (Sort₂ × Sort₂)) : List Sort₂ :=
  es.flatMap (fun p => [p.1, p.2])

/-- The sort graph of a formula (computed on NNF by the classifier; `Strat/Fragment.lean`). -/
def sortGraph (φ : Frm) : Graph :=
  let es := edgesFrm [] φ
  { nodes := nodesOf es, edges := es }

/-- `sortGraph` is well-formed: its node set is COMPLETE for its edges (every edge
    endpoint is a node), so `acyclic`'s Warshall closure over `nodes` is exhaustive.

    This is the kernel realisation of metatheory §5's `sortGraph_complete`: the E2 edges
    are computed DIRECTLY (the Skolemisation-closure the sketch describes is exactly the
    E2 function-flow edges, so no Skolemisation pass is needed), and the graph is closed
    under its own endpoints — the property `acyclic_iff_no_cycle` consumes. -/
theorem sortGraph_complete (φ : Frm) : Wf (sortGraph φ) := by
  intro p hp
  refine ⟨?_, ?_⟩
  · exact List.mem_flatMap.mpr ⟨p, hp, by simp⟩
  · exact List.mem_flatMap.mpr ⟨p, hp, by simp⟩

end Thermite.Strat.Cls
