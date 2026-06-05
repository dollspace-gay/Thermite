# THERMITE.skill.md

The complete Thermite v0.1 surface language and toolchain, in one file. This is
the canonical language definition (`thermite-design.md` §10): an agent reads it
at session start and holds the entire language in context. It is GENERATED — do
not edit by hand. Regenerate with:

    cargo run -p thermite-skill -- --emit > THERMITE.skill.md

Budget: this file must stay under 6,000 tokens (a hard CI gate, design §2.2).

## 1. Surface grammar

Every `fn` is contract-first, body-second. Three top-level item forms exist and
no others (no `struct`/`impl`/`trait`/`use`/`mod`/macros in v0.1): `fn`,
`spec fn`, and `#[slag(...)] fn`.

A `fn` signature is followed by mandatory clauses in this exact order — absence
of any is a parse error, never an implicit default:

- `req EXPR` — precondition (write `req true` if there is none).
- `ens EXPR` — postcondition, one-or-more. Must mention `result` unless the
  return type is `()`.
- `fx EFFECTROW` — effect row, exactly one.

A `spec fn` carries exactly one `dec EXPR` (a decreases-measure), not
`req`/`ens`/`fx`. Spec functions are total, terminating, and executable.

```thermite
fn binary_search(haystack: &[u32], needle: u32) -> Option<usize>
  req sorted(haystack)
  ens match result {
        Some(i) => i < haystack.len() && haystack[i] == needle,
        None    => forall_in(haystack, |x| x != needle),
      }
  fx  pure
{
  let mut lo: usize = 0;
  let mut hi: usize = haystack.len();
  loop
    inv lo <= hi && hi <= haystack.len()
    inv forall_below(haystack, lo, |x| x < needle)
    inv forall_from(haystack, hi, |x| x > needle)
    dec hi - lo
  {
    if lo == hi { return None; }
    let mid = lo + (hi - lo) / 2;
    if haystack[mid] == needle { return Some(mid); }
    if haystack[mid] < needle  { lo = mid + 1; } else { hi = mid; }
  }
}
```

Loops: both `loop { }` and `while EXPR { }` carry one-or-more `inv EXPR` clauses
then exactly one `dec EXPR`, then the body. Missing `inv` or `dec` is a parse
error. Termination is proved by default; divergence requires `fx diverge`.

Statements: `let mut? NAME : TYPE = EXPR ;`, assignment `LVALUE = EXPR ;`,
`return EXPR? ;`, the `if`/`else` statement, and expression-statements. A block
`{ }` is statements plus an optional trailing tail expression (no `;`) that is
the block's value.

Expressions: integer literals (`1_000_000`), `bool` literals, paths (`lo`,
`u32::MAX`, `Some`, `None`), free call `f(args)`, ONE call syntax for member
access (postfix `.`: `xs.len()` is a method call; there is no UFCS), closure
`|x| EXPR`, `match`, `if/else` as an expression, arithmetic `+ - * /`,
comparison `== != < <= > >=` (non-associative — `a < b < c` is an error),
logical `&& ||`, indexing `a[i]` / range index `a[..i]`, cast `EXPR as TYPE`,
references `&EXPR` / `&mut EXPR`, and parenthesized grouping.

Patterns: `_`, literals, bindings, slice patterns `[]` / `[head, ..t]`, and
enum/tuple-struct patterns `Some(i)` / `None`.

Types: `u32`, `u64`, `usize`, `bool`, shared slice `&[T]`, references `&T` /
`&mut T`, one generic application `NAME<T>` (`Option<usize>`). No user generics,
no lifetimes. A `()` return type is written explicitly.

Effect rows: `pure`, or a set drawn from `read(path)`, `write(path)`,
`net(domain)`, `alloc`, `time`, `rand`, `panic`, `diverge`. A caller's row must
subsume every callee's row (compile-time check).

Removed from Rust (to keep the language small and formulaic): explicit
lifetimes, the full trait system (only built-in `Eq`/`Ord`/`Hash`/`Iter`/
`Display`), macros, `unsafe` (replaced by `#[slag]`), UFCS, `match`-ergonomics
special cases, and implicit integer widening (all conversions explicit;
arithmetic overflow is a proof obligation).

## 2. SpecTherm combinator library

Contracts are written in SpecTherm, a deliberately weak total language. There
are NO general quantifiers: quantification is only available through this fixed,
closed library of bounded combinators, each with a hand-tuned frozen SMT
trigger. A combinator becomes part of this set only through the slow,
budget-gated RFC process — never a user abstraction.

Flat-closure rule (`.design/spec/spectherm-combinators.md` REQ-6, design §4.2):
a combinator's predicate-closure body (`|x| ...`) is a FLAT predicate — it may
use comparisons, arithmetic, boolean/logical operators, field/index access, and
calls to NAMED `spec fn`s, but it may NOT contain another combinator. Genuine
nested quantification is written as a named `spec fn` (which carries its own
`dec` measure). Every quantifier is a bounded combinator with a frozen trigger;
composition happens only through named `spec fn`s, never anonymous nested
quantifiers.

The combinators (signature, then one example each):

- `sorted(&[u32]) -> bool`
  // example: req sorted(haystack)
- `forall_in(&[u32], |x| -> bool) -> bool`
  // example: ens forall_in(haystack, |x| x != needle)
- `exists_in(&[u32], |x| -> bool) -> bool`
  // example: ens exists_in(haystack, |x| x == needle)
- `count_where(&[u32], |x| -> bool) -> usize`
  // example: ens count_where(xs, |x| x == 0) <= xs.len()
- `permutation_of(&[u32], &[u32]) -> bool`
  // example: ens permutation_of(result, input)
- `disjoint(&[u32], &[u32]) -> bool`
  // example: req disjoint(lefts, rights)
- `forall_below(&[u32], usize, |x| -> bool) -> bool`
  // example: inv forall_below(haystack, lo, |x| x < needle)
- `forall_from(&[u32], usize, |x| -> bool) -> bool`
  // example: inv forall_from(haystack, hi, |x| x > needle)

## 3. Forge command set

Forge is the agent's interface — a goal-state REPL. The unit of progress is
discharging a goal; every Forge message is a structured prompt with the relevant
source inline, returns counterexamples (concrete witnesses) rather than
adjectives when an obligation fails, and degrades rather than blocks on a solver
timeout.

```
forge new <name>                   create project (manifest, lockfile, skill pin)
forge goal <item>                  print goal state for an item
forge fill <hole-addr> <code>      fill a hole; returns new goal state
forge edit <addr> --replace <code> semantic edit by stable address
forge check [item]                 run the ladder; per-obligation results
forge build [item] --entry <fn>    lower to Rust + rustc -> a native binary whose
                                   contract checks fire at runtime, fx-sandboxed
forge battery [item]               run vacuity battery + mutation scoring
forge audit                        full slag + boundary + assurance inventory
forge skill                        emit the canonical THERMITE.skill.md
forge repair [item]                background L1/L2 -> L3 upgrade loop
```

Items and blocks have stable semantic addresses (`binary_search.loop#1.inv#2`);
edits take addresses, not string matches.

## 4. Verification ladder

Every function targets L3; downgrades are automatic, logged, and surfaced in the
build manifest; upgrades are a standing background task. The certificate lists
every function's level — this manifest IS the deliverable's trust statement.

- L3 — SMT proof (Verus/Z3): the contract holds for ALL inputs. Not guaranteed
  to terminate -> solver budget + automatic downgrade.
- L2 — bounded model check (Kani/CBMC): holds for all inputs UP TO a bound. The
  manifest states the bound explicitly; L2 and L3 are always distinct.
- L1 — runtime contract checks: violations are detected at the call site, in
  every build profile (not just debug).
- L0 — `#[slag]`: nothing is proved about the body. Trusted by fiat.

L0 / slag clarification (design §6): the L0 row measures assurance about the
BODY only. A `#[slag]` function's CONTRACT is still mandatory and enforced at
runtime, so its certificate carries level L1 with a `slag: true` flag — L1
because the contract is L1-checked at the call site, slag because the body is
unproven. Slag exempts PROVING, never STATING and CHECKING. The `fx` effect row
is enforced two ways, independent of the proof level: caller/callee subsumption
at compile time, and — in a `forge build` binary — a seccomp syscall sandbox
derived from the row, so code that exceeds its declared effects is killed at the
syscall boundary (a `#[slag]`/boundary body included).

## 5. Slag rules

`#[slag]` is the escape hatch for unverified code (slag is the waste product of
a thermite burn). It is the replacement for `unsafe`: harder to write, louder to
read.

```thermite
#[slag(reason = "vendored SIMD intrinsics; contract checked at boundary by L1",
       owner  = "agent:forge-7/session-2026-06-04",
       review = "required")]
fn simd_sum(xs: &[u32]) -> u64
  req xs.len() <= u32::MAX as usize
  ens result == spec_sum(xs)          // contract still mandatory — enforced at L1
  fx  pure
{ ... }
```

Rules:

- `reason`, `owner`, and `review` fields are mandatory and non-empty (checked).
- The contract is STILL mandatory and is enforced at L1 (runtime) — slag exempts
  you from PROVING, never from STATING and CHECKING.
- Every slag block appears in the build manifest and in `forge audit`. `grep
  slag` over a codebase is the complete inventory of fiat-trusted code.
- CI policy hooks can cap slag count or require a second-party sign-off.

The polarity inversion is the point: verification is the default and costs
nothing; non-verification is the exotic add-on and costs more keystrokes, more
metadata, and more visibility.
