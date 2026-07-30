/-
  Thermite/Strat/Cls/Wire.lean — the differential battery's Lean entry point
  (`.design/stage2-stratified-cage.md` REQ-4 / AC-4, the M2b classifier differential).

  This module is the LEAN HALF of the stratified-cage classifier differential. The Rust
  classifier (`thermite-spec/src/classifier.rs`) and the SplitMix64 generator
  (`thermite-tv/src/gen.rs`) emit each well-sorted formula in a compact, parenthesized
  S-expression WIRE format; here a `partial` recursive-descent parser turns one wire line
  back into the kernel-defined `Thermite.Strat.Cls.Frm`, and `main` runs the KERNEL
  classifier `Thermite.Strat.Cls.admitted` on it (the exact REQ-3 `admitted`, NOT a
  re-implementation), printing one verdict line per input line. The forge differential
  harness (`forge strat-tv` / `forge/tests/strat_differential.rs`) pipes the generated
  formulas to `lake env lean --run Thermite/Strat/Cls/Wire.lean` on stdin and holds the
  Lean verdicts byte-equal to the Rust ones; any disagreement is a hard CI failure.

  This is a TOOL module (an `IO` driver), not part of the proof spine: it carries no
  theorems and the parser is `partial` (its termination is not needed — a malformed line
  is reported as `parse-error`, never trusted). It is added to the Lean CI build target
  set (`scripts/lean-axiom-probe.sh`) only so a compile break is caught, not because it
  contributes an axiom-gated theorem.

  THE WIRE GRAMMAR (tokens are `(`, `)`, and maximal non-paren non-space runs; mirrors
  `thermite-spec/src/classifier.rs`'s `to_wire`/`parse_frm`):
  - sort := `(m WIDTH)` | `(s SORT)` | `(o NAT)`   (WIDTH ∈ u8 u16 u32 u64 usize bool)
  - tm   := `(v SORT INT)` | `(l SORT)` | `(rd SORT TM TM)` | `(ln TM)`
          | `(ct SORT TM)` | `(ix TM INT)` | `(ml TM TM)` | `(a1 SORT SORT NAT TM)`
  - atom := `(r REL TM TM)` | `(qf NAT)`           (REL ∈ eq ne lt le gt ge)
  - frm  := `(at ATOM)` | `(ng FRM)` | `(cj FRM FRM)` | `(dj FRM FRM)`
          | `(im FRM FRM)` | `(al SORT FRM)` | `(ex SORT FRM)`

  A `(qf id)` leaf maps to a distinct closed `Atom.qfree` placeholder. The
  classifier never inspects the payload, but retaining the ID catches wire
  round-trip mistakes and mirrors the production bridge's stable source-leaf
  identity.
-/
import Thermite.Strat.Fragment

namespace Thermite.Strat.Cls.Wire

open Thermite.Strat.Cls

/-! ## Tokenizer -/

/-- Split a wire string into tokens: each `(`/`)` is its own token, runs of
    non-paren non-whitespace characters are atom tokens, whitespace separates. -/
def tokenize (s : String) : List String :=
  let rec go : List Char → List Char → List String → List String
    | [],        cur, acc => if cur.isEmpty then acc else acc ++ [String.ofList cur.reverse]
    | c :: rest, cur, acc =>
      if c == '(' || c == ')' then
        let acc := if cur.isEmpty then acc else acc ++ [String.ofList cur.reverse]
        go rest [] (acc ++ [String.ofList [c]])
      else if c == ' ' || c == '\t' || c == '\n' || c == '\r' then
        let acc := if cur.isEmpty then acc else acc ++ [String.ofList cur.reverse]
        go rest [] acc
      else
        go rest (c :: cur) acc
  go s.toList [] []

/-! ## Leaf decoders -/

/-- A machine-width token → `Mach`. -/
def machOfWire : String → Option Mach
  | "u8"    => some .u8
  | "u16"   => some .u16
  | "u32"   => some .u32
  | "u64"   => some .u64
  | "usize" => some .usize
  | "bool"  => some .bool
  | _       => none

/-- A relation token → `Rel`. -/
def relOfWire : String → Option Rel
  | "eq" => some .eq
  | "ne" => some .ne
  | "lt" => some .lt
  | "le" => some .le
  | "gt" => some .gt
  | "ge" => some .ge
  | _    => none

/-! ## Recursive-descent parser

    Each parser consumes a `( tag … )` node and returns the value plus the unconsumed
    token suffix, or `none` on a malformed node. `partial` because termination on the
    shrinking token list is not needed for an `IO` tool (a bad line is reported, never
    trusted). -/

mutual

partial def parseSort : List String → Option (Sort₂ × List String)
  | "(" :: tag :: rest =>
    match tag with
    | "m" =>
      match rest with
      | w :: r2 =>
        match machOfWire w, r2 with
        | some m, ")" :: r3 => some (Sort₂.mach m, r3)
        | _, _ => none
      | _ => none
    | "s" =>
      match parseSort rest with
      | some (inner, ")" :: r3) => some (Sort₂.seq inner, r3)
      | _ => none
    | "o" =>
      match rest with
      | k :: r2 =>
        match k.toNat?, r2 with
        | some n, ")" :: r3 => some (Sort₂.opaque n, r3)
        | _, _ => none
      | _ => none
    | _ => none
  | _ => none

partial def parseTm : List String → Option (Tm × List String)
  | "(" :: tag :: rest =>
    match tag with
    | "v" =>
      match parseSort rest with
      | some (s, i :: r2) =>
        match i.toNat?, r2 with
        | some n, ")" :: r3 => some (Tm.var s n, r3)
        | _, _ => none
      | _ => none
    | "c" =>
      match parseSort rest with
      | some (s, i :: r2) =>
        match i.toNat?, r2 with
        | some n, ")" :: r3 => some (Tm.const s n, r3)
        | _, _ => none
      | _ => none
    | "l" =>
      match parseSort rest with
      | some (s, "(" :: kind :: value :: ")" :: ")" :: r3) =>
        match kind with
        | "i" => value.toInt?.map fun n => (Tm.lit s (.int n), r3)
        | "b" =>
          match value with
          | "0" => some (Tm.lit s (.bool false), r3)
          | "1" => some (Tm.lit s (.bool true), r3)
          | _ => none
        | _ => none
      | _ => none
    | "rd" =>
      match parseSort rest with
      | some (elem, r2) =>
        match parseTm r2 with
        | some (sq, r3) =>
          match parseTm r3 with
          | some (ix, ")" :: r4) => some (Tm.read elem sq ix, r4)
          | _ => none
        | _ => none
      | _ => none
    | "ln" =>
      match parseTm rest with
      | some (sq, ")" :: r3) => some (Tm.len sq, r3)
      | _ => none
    | "ct" =>
      match parseSort rest with
      | some (to, r2) =>
        match parseTm r2 with
        | some (t, ")" :: r3) => some (Tm.cast to t, r3)
        | _ => none
      | _ => none
    | "ix" =>
      match parseTm rest with
      | some (t, k :: r2) =>
        match k.toInt?, r2 with
        | some z, ")" :: r3 => some (Tm.idxOp t z, r3)
        | _, _ => none
      | _ => none
    | "ml" =>
      match parseTm rest with
      | some (t, r2) =>
        match parseTm r2 with
        | some (u, ")" :: r3) => some (Tm.mul t u, r3)
        | _ => none
      | _ => none
    | "a1" =>
      match parseSort rest with
      | some (arg, r2) =>
        match parseSort r2 with
        | some (res, f :: r3) =>
          match f.toNat? with
          | some fn =>
            match parseTm r3 with
            | some (a, ")" :: r4) => some (Tm.app1 arg res fn a, r4)
            | _ => none
          | none => none
        | _ => none
      | _ => none
    | _ => none
  | _ => none

partial def parseAtom : List String → Option (Atom × List String)
  | "(" :: tag :: rest =>
    match tag with
    | "r" =>
      match rest with
      | rl :: r2 =>
        match relOfWire rl with
        | some ρ =>
          match parseTm r2 with
          | some (t, r3) =>
            match parseTm r3 with
            | some (u, ")" :: r4) => some (Atom.rel ρ t u, r4)
            | _ => none
          | _ => none
        | none => none
      | _ => none
    | "qf" =>
      match rest with
      | id :: ")" :: r2 =>
        id.toNat?.map fun n =>
          (Atom.qfree n (Thermite.Expr.boolVar s!"__s2_qfree_{n}"), r2)
      | _ => none
    | _ => none
  | _ => none

partial def parseFrm : List String → Option (Frm × List String)
  | "(" :: tag :: rest =>
    match tag with
    | "at" =>
      match parseAtom rest with
      | some (a, ")" :: r3) => some (Frm.atom a, r3)
      | _ => none
    | "ng" =>
      match parseFrm rest with
      | some (φ, ")" :: r3) => some (Frm.neg φ, r3)
      | _ => none
    | "cj" =>
      match parseFrm rest with
      | some (φ, r2) =>
        match parseFrm r2 with
        | some (ψ, ")" :: r3) => some (Frm.conj φ ψ, r3)
        | _ => none
      | _ => none
    | "dj" =>
      match parseFrm rest with
      | some (φ, r2) =>
        match parseFrm r2 with
        | some (ψ, ")" :: r3) => some (Frm.disj φ ψ, r3)
        | _ => none
      | _ => none
    | "im" =>
      match parseFrm rest with
      | some (φ, r2) =>
        match parseFrm r2 with
        | some (ψ, ")" :: r3) => some (Frm.imp φ ψ, r3)
        | _ => none
      | _ => none
    | "al" =>
      match parseSort rest with
      | some (s, r2) =>
        match parseFrm r2 with
        | some (φ, ")" :: r3) => some (Frm.all s φ, r3)
        | _ => none
      | _ => none
    | "ex" =>
      match parseSort rest with
      | some (s, r2) =>
        match parseFrm r2 with
        | some (φ, ")" :: r3) => some (Frm.ex s φ, r3)
        | _ => none
      | _ => none
    | _ => none
  | _ => none

end

/-- Parse a whole wire line to a `Frm` (the entire token stream must be consumed). -/
def parseLine (line : String) : Option Frm :=
  match parseFrm (tokenize line) with
  | some (φ, []) => some φ
  | _            => none

/-- The verdict string for one input line: `true`/`false` from the KERNEL `admitted`, or
    `parse-error` for a malformed line (never silently dropped — the harness escalates a
    `parse-error` as classifier-suspect). -/
def runLine (line : String) : String :=
  match parseLine line with
  | some φ => if admitted φ then "true" else "false"
  | none   => "parse-error"

/-- Read wire lines from stdin until EOF, printing one verdict per line. `getLine`
    returns `""` only at EOF (a blank input line is `"\n"`, non-empty), so every
    non-EOF line yields exactly one output line — the harness matches output count to
    input count. -/
partial def loop (stdin : IO.FS.Stream) : IO Unit := do
  let line ← stdin.getLine
  if line.isEmpty then
    pure ()
  else
    IO.println (runLine line)
    loop stdin

/-- The differential battery driver: classify every stdin wire line with the kernel
    `Thermite.Strat.Cls.admitted`, printing one verdict per line. -/
def run : IO Unit := do
  let stdin ← IO.getStdin
  loop stdin

end Thermite.Strat.Cls.Wire

/-- Root-level entry point for `lake env lean --run Thermite/Strat/Cls/Wire.lean`
    (the interpreter resolves the `main` declaration at the root namespace). Pipes the
    generated wire formulas on stdin to the kernel classifier. -/
def main : IO Unit := Thermite.Strat.Cls.Wire.run
