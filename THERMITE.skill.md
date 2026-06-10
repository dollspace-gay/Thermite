# THERMITE.skill.md

The complete Thermite v0.1 surface language and toolchain, in one file. This is
the canonical language definition (`thermite-design.md` §10): an agent reads it
at session start and holds the entire language in context. It is GENERATED — do
not edit by hand. Regenerate with:

    cargo run -p thermite-skill -- --emit > THERMITE.skill.md

Budget: this file must stay under 6,000 tokens (a hard CI gate, design §2.2).

## How to read this file — the 60-second workflow

You write verified code. The loop: (1) write a `fn` CONTRACT-FIRST — `req`/
`ens`/`fx`, THEN the body (§1); the contract is mandatory, no implicit defaults.
(2) `forge check <file>` (§3) returns a PER-OBLIGATION result — each goal is
discharged or `Failed` with a CONCRETE counterexample (e.g. `lo=3, hi=3`), never
a bare "verification failed". (3) Fix the body/contract and re-check; or, if the
body is not yet known, drop a HOLE `?0` in its place (§1) and work `forge goal`/
`forge fill` — a holed item never certifies until every hole is filled.

Map: §1 grammar (what you may write), §2/§2b combinators + recursion schemes (the
ONLY way to quantify/recurse in a spec), §3 Forge verbs, §4 the assurance ladder
(what a certificate means), §5 the `#[slag]` proof escape hatch.

## 1. Surface grammar

Every `fn` is contract-first, body-second. v0.1 has four top-level item forms —
`fn`, `spec fn`, `struct`, and `enum` (plus the `#[slag(...)]` / `#[boundary]`
attributes) — and no others (no `impl`/`trait`/`use`/`mod`/macros).

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

Loops: both `loop { }` and `while EXPR { }` carry one-or-more `inv EXPR` then
exactly one `dec EXPR`, then the body (missing `inv`/`dec` is a parse error).
Termination is proved by default; divergence requires `fx diverge`. `break ;`
exits and `continue ;` restarts: each `inv` must hold at every `break`/`continue`,
and in a terminating loop a `continue` must also decrease `dec`. An `fx diverge`
loop makes no termination claim, so `break`/`continue` are unconstrained by `dec`
(the event-loop shape `while true { … if k == quit { break; } … }`).

Statements: `let mut? NAME : TYPE = EXPR ;`, assignment `LVALUE = EXPR ;`,
`return EXPR? ;`, the `if`/`else` statement, the loop-control statements
`break ;` / `continue ;` (valid only inside a `loop`/`while` body, labelless and
value-less — no `break EXPR`), and expression-statements. A block `{ }` is
statements plus an optional trailing tail expression (no `;`) that is the
block's value. There is ONE member-access call syntax (postfix `.`); there is no
UFCS. Comparisons are non-associative (`a < b < c` is an error).

Holes: `?0` (a `?` followed by a digit run) is a HOLE — an open goal placeholder
valid ONLY in exec-`fn`-body statement position (not in a spec clause, `spec fn`,
or expression). A `fn` with any open hole is well-formed but NEVER certifies (it
is L0 until every hole is filled). You work holes with the goal-state REPL:
`forge goal <fn>` shows the open holes as `?N`, and `forge fill <fn>.?N <code>`
splices code at that hole and re-checks (the fill may surface new holes).

Binding / control-flow ergonomics (sugar over the proven core — one desugaring,
always explicit):

- Tuple destructuring `let (x, y) = e;` binds each element by projection
  (`let x = e.0; let y = e.1;`). Use `_` to drop an element; sub-patterns are
  flat names only.
- `for i in lo..hi inv EXPR { B }` is a bounded-range loop: you write the loop
  `inv` (mandatory, one-or-more, like `while`); the `dec` is AUTOMATIC
  (`hi - i`), so you write no `dec`. It desugars to
  `let mut i = lo; while i < hi inv EXPR dec hi - i { B; i = i + 1; }`. Only an
  exclusive integer range `lo..hi` (step +1) is admitted.
- Match guards: `Pat if COND => EXPR`. A guard does NOT complete a match — a
  guarded-only arm leaves its variant uncovered, so a `_`/full-variant arm is
  still required for exhaustiveness.
- Or-patterns: `p0 | p1 => EXPR` matches any alternative and covers their UNION
  (`Some(_) | None` is exhaustive over an `Option`). v0.1 alternatives are
  payload-free (they bind the same — empty — set of names).
- `if let Pat = e { T } else { E }` desugars to `match e { Pat => T, _ => E }`
  (the `else` is required — both branches produce a value). `while let
  Variant(_) = e inv EXPR dec EXPR { B }` desugars to the canonical
  `while (e is Variant) inv EXPR dec EXPR { B }` (you write `inv`/`dec` as for
  any `while`).

The CONSTRUCT INVENTORY below is GENERATED by an exhaustive match over the
toolchain's own `Item`/`Type`/`Expr`/`BinOp`/`Pattern`/`Effect` enums, so it can
never silently fall behind the language.

### Item forms
- `fn NAME(..) -> T req .. ens .. fx .. { .. }` — a contract-first function (mandatory req/ens/fx, in order)
  // e.g. fn sum(xs: &[u32]) -> u64 req .. ens .. fx pure { .. }
- `spec fn NAME(..) -> T dec .. { .. }` — a total terminating spec function (one dec measure, no req/ens/fx)
  // e.g. spec fn spec_sum(xs: &[u32]) -> nat dec xs.len() { .. }
- `struct NAME { field: T, .. } [inv EXPR]` — a product type with an optional type-invariant inv clause
  // e.g. struct Account { balance: u64 } inv balance <= cap
- `enum NAME { Unit, Tuple(T, ..), Struct { f: T } }` — a sum type; match over it must be exhaustive
  // e.g. enum List { Nil, Cons(u64, Box<List>) }

**Types**

- `u32 | u64 | usize | bool` — the closed primitive scalar set (no implicit widening)
  // e.g. let n: u64 = 0;
- `()` — the unit type, written explicitly in a return position
  // e.g. fn log() -> () req true ens true fx pure { }
- `&T | &mut T` — a shared / exclusive reference (no explicit lifetimes)
  // e.g. fn f(x: &mut u64)
- `&[T]` — a borrowed read-only slice view
  // e.g. fn sum(xs: &[u32]) -> u64
- `NAME<T>` — one single-arg generic application
  // e.g. -> Wrapper<usize>
- `Name` — a bare user-declared struct/enum type name
  // e.g. fn area(s: Shape) -> u64
- `Box<T>` — heap indirection for a recursive enum (carries fx alloc)
  // e.g. Cons(u64, Box<List>)
- `Vec<T>` — a bounded growable collection over verified vstd (fx alloc)
  // e.g. let v: Vec<u64> = Vec::new();
- `String` — a bounded owned run of u8 bytes (fx alloc)
  // e.g. let s: String = "hi";
- `Option<T>` — the built-in optional (Some(v)/None; match/is; payload-in-contract via match-in-ens)
  // e.g. -> Option<u64> ens match result { Some(v) => v == 5, None => true }
- `Result<T, E>` — the built-in fallible (Ok(v)/Err(e); match/is; the loud error arm)
  // e.g. -> Result<u64, ParseErr>
- `Map<K, V>` — a bounded verified key-value map (insert/get/contains_key/len; get -> Option<V>, absent -> None; fx alloc)
  // e.g. let mut m: Map<u64, u64> = Map::new(); m.insert(k, v); m.get(k)
- `(T, U, ..)` — an n-tuple (arity >= 2) for multiple returns; access via .0/.1
  // e.g. fn swap(a: u64, b: u64) -> (u64, u64) req true ens result.0 == b && result.1 == a fx pure { (b, a) }

**Primitive scalars**

- `u32` — a 32-bit unsigned integer
  // e.g. needle: u32
- `u64` — a 64-bit unsigned integer
  // e.g. -> u64
- `usize` — a pointer-width unsigned index
  // e.g. let i: usize = 0;
- `bool` — a boolean
  // e.g. let ok: bool = true;

**Expressions**

- `1_000_000` — an integer literal (verbatim `_` separators preserved)
  // e.g. req xs.len() <= 1_000_000
- `true | false` — a boolean literal
  // e.g. req true
- `name | Mod::ITEM` — a path: a binding, a constant, or an enum variant
  // e.g. u32::MAX
- `f(args)` — a free call (combinators and spec fns are free calls)
  // e.g. sorted(haystack)
- `recv.m(args)` — the ONE member-access call syntax (no UFCS)
  // e.g. xs.len()
- `recv.field` — a field access
  // e.g. account.balance
- `|x| EXPR` — a flat predicate closure (no nested combinator/scheme)
  // e.g. |x| x != needle
- `match e { Pat [if C] => EXPR, .. }` — a match (exhaustive over an enum; an `if C` guard does NOT complete a match)
  // e.g. match result { Some(i) => .., None => .. }
- `if C { .. } else { .. }` — an if/else as an expression (both arms required)
  // e.g. if lo == hi { 0 } else { 1 }
- `a OP b` — an arithmetic / comparison / logical / bitwise binary op
  // e.g. lo + (hi - lo) / 2
- `!EXPR` — prefix not (logical on bool, bitwise on int; binds tightest)
  // e.g. !done
- `a[i] | a[..i] | a[i..] | a[i..j]` — single or range indexing
  // e.g. spec_sum(&xs[..i])
- `EXPR as T` — an explicit cast (all integer conversions are explicit)
  // e.g. xs[i] as u64
- `&EXPR | &mut EXPR` — a shared / exclusive borrow
  // e.g. &xs[..i]
- `Path { field: val, .. }` — a struct / struct-variant construction
  // e.g. Account { balance: 0 }
- `EXPR is Variant` — a bool-valued variant-discrimination test
  // e.g. result is Circle
- `*EXPR` — a dereference of a boxed value (the recursive descent)
  // e.g. sum_list(*t)
- `"text"` — a string literal (an owned String; carries fx alloc)
  // e.g. let s: String = "hello";
- `(a, b, ..)` — an n-tuple construction (arity >= 2; (e) is grouping)
  // e.g. (b, a)
- `e.0 | e.1 | ..` — a tuple projection (the one tuple access; reads in exec and ens)
  // e.g. ens result.0 == b && result.1 == a

**Binary operators**

- `a + b` — addition (overflow is a proof obligation)
  // e.g. acc + xs[i] as u64
- `a - b` — subtraction (underflow is a proof obligation)
  // e.g. hi - lo
- `a * b` — multiplication (overflow is a proof obligation)
  // e.g. w * h
- `a / b` — division (div-by-zero is a proof obligation)
  // e.g. (hi - lo) / 2
- `a % b` — remainder (div-by-zero is a proof obligation: req b != 0)
  // e.g. n % 2
- `a << k` — left shift (the shift amount must be bounded: req k < 64)
  // e.g. 1 << k
- `a >> k` — right shift (the shift amount must be bounded: req k < 64)
  // e.g. x >> k
- `a & b` — bitwise and
  // e.g. flags & mask
- `a | b` — bitwise or
  // e.g. flags | bit
- `a ^ b` — bitwise xor
  // e.g. a ^ b
- `a == b` — equality
  // e.g. haystack[mid] == needle
- `a != b` — inequality
  // e.g. x != needle
- `a < b` — less-than (non-associative)
  // e.g. i < xs.len()
- `a <= b` — less-or-equal
  // e.g. lo <= hi
- `a > b` — greater-than
  // e.g. x > needle
- `a >= b` — greater-or-equal
  // e.g. balance >= amount
- `a && b` — logical and
  // e.g. lo <= hi && hi <= len
- `a || b` — logical or
  // e.g. done || empty

**Unary (prefix) operators**

- `!EXPR` — prefix not — logical on bool, bitwise on int; binds tightest
  // e.g. !(a & mask)

**Patterns**

- `_` — the wildcard pattern
  // e.g. _ => 0
- `LIT` — a literal pattern
  // e.g. 0 => true
- `name` — a binding pattern
  // e.g. Some(i) => i
- `[] | [head, ..tail]` — a slice pattern with an optional rest binding
  // e.g. [head, ..tail] => head
- `Variant(p, ..) | None` — a tuple/unit enum-variant pattern (binds the payload)
  // e.g. Some(i) => ..
- `Path { field, .. }` — a struct / struct-variant destructuring pattern
  // e.g. Rect { w, h } => w * h
- `p0 | p1 | ..` — an or-pattern (matches any alternative; covers their union)
  // e.g. 1 | 2 => true

**Effect atoms (a caller's fx row subsumes every callee's)**

- `read(path)` — reads from a filesystem path
  // e.g. fx read("/etc/hosts")
- `write(path)` — writes to a filesystem path
  // e.g. fx write("/tmp/out")
- `net(domain)` — performs network I/O to a domain
  // e.g. fx net("api.example.com")
- `alloc` — allocates on the heap (Box/Vec/String construction)
  // e.g. fx alloc
- `time` — reads the wall clock
  // e.g. fx time
- `rand` — draws randomness
  // e.g. fx rand
- `panic` — may panic / abort
  // e.g. fx panic
- `diverge` — may not terminate (waives the default termination proof)
  // e.g. fx diverge
- `term` — controls the terminal (raw mode via the `ioctl` syscall)
  // e.g. fx term

Removed from Rust (to keep the language small and formulaic): explicit
lifetimes, the full trait system (only built-in `Eq`/`Ord`/`Hash`/`Iter`/
`Display`), macros, `unsafe` (replaced by `#[slag]`), UFCS, and implicit integer
widening (all conversions explicit; arithmetic overflow is a proof obligation).

## 2. SpecTherm combinator library

Use these to QUANTIFY in a contract. You may NOT write a raw `forall`/`exists` in
a `req`/`ens`/`inv` — quantification is available ONLY through this fixed, closed
library of bounded combinators (SpecTherm, a deliberately weak total language),
each with a hand-tuned frozen SMT trigger so the proof goes through. A combinator
joins this set only via a slow budget-gated RFC — never a user abstraction.

Flat-closure rule (§4.2): a combinator's predicate closure (`|x| ...`) is a FLAT
predicate — comparisons, arithmetic, boolean/logical ops, field/index access, and
calls to NAMED `spec fn`s — but it may NOT contain another combinator. Genuine
nested quantification is a named `spec fn` (with its own `dec` measure).

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

## 2b. Recursion-scheme library

Use these to RECURSE over a recursive ADT (a `Box`ed `enum` like a list/tree).
You may NOT hand-write the recursion — it goes through this fixed, closed set of
verified schemes (the structural analogue of the combinators). Each takes the
scrutinee (and, for `fold`, a seed) then a trailing FLAT step closure — like a
combinator's predicate closure, the step may NOT contain another scheme (genuine
nesting is a named `spec fn`). A scheme discharges its bound by citing the
`fold_bound` prove-once law, never a fresh induction.

The schemes (call shape, result, then one example each):

- `fold(l, init, |x, acc| …) -> nat`
  // scheme: fold(list, 0, |x, acc| acc + x)
- `traverse(l, |x, acc| …) -> bool`
  // scheme: traverse(list, |x, acc| acc && p(x))
- `map(l, |x| …) -> the same ADT`
  // scheme: map(list, |x| x + 1)
- `for_all(l, |x| …) -> bool`
  // scheme: for_all(list, |x| x <= bound)
- `exists(l, |x| …) -> bool`
  // scheme: exists(list, |x| x == needle)

## 3. Forge command set

Forge is your interface — a goal-state REPL. Every reply inlines the source,
returns a CONCRETE counterexample (a witness) rather than an adjective when an
obligation fails, and DEGRADES (L3 -> L2 -> L1) rather than blocks on a solver
timeout. Day-to-day verbs: `check` (does it verify?), `goal`/`fill` (work open
holes), `build` (lower to a runnable binary).

```
forge new <name>                   create project (manifest, lockfile, skill pin)
forge check [item]                 run the ladder; per-obligation results +
                                   counterexamples (your primary verb)
forge goal <item>                  print the goal state: given / want / open
                                   holes ?N / per-obligation status
forge fill <fn>.?N <code>          splice code at a hole ?N + re-check; returns
                                   the new goal state (may surface new holes)
forge edit <addr> --replace <code> splice at any semantic address + re-check
forge build [item] --entry <fn>    lower to Rust + rustc -> a native binary whose
                                   contract checks fire at runtime, fx-sandboxed
forge build --target kernel <file> emit a freestanding no_std+alloc rlib (no
                                   main, no seccomp, panic=abort) for a verified
                                   microkernel; ambient-syscall fx
                                   (read/write/net/term/time/rand) is REFUSED
forge battery [item]               run vacuity battery + mutation scoring
forge audit                        full slag + boundary + assurance inventory
forge review <file> [item]         pluggable spec-intent review slot
forge tv <file>                    translation-validate each item's CONTRACT
                                   lowering against the independent reference
                                   encoder (Z3 equivalence; off-corpus generator)
forge exec-tv <file>               translation-validate exec EXPRESSION lowering
forge body-tv <file>               translation-validate the exec BODY state
                                   (straight-line + v1 while-loop obligations);
                                   Faithful/Divergent/Unverifiable/Skipped
forge skill                        emit the canonical THERMITE.skill.md
forge repair [item]                background L1/L2 -> L3 upgrade loop
```

Items and blocks have stable semantic addresses (`binary_search.loop#1.inv#2`,
a hole is `<fn>.?N`); `edit`/`fill` take addresses, not string matches.

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

The Thermite -> Verus lowering behind L3 is not a trusted black box: each
checked item is translation-validated per run (Z3 proves the lowered contract
equivalent to an independent reference encoding, `thermite-tv`, itself proven
denotation-faithful by a kernel-checked Lean spine). `make audit` re-derives the
L3 claim from source on a skeptic's machine.

L0 / slag clarification (§6): the level rates the BODY only. A `#[slag]` fn's
CONTRACT is still mandatory and L1-checked at the call site, so its cert is L1
with `slag: true` (L1 = contract checked, slag = body unproven). Slag
exempts PROVING, never STATING and CHECKING. The `fx` row is enforced independent of
level: caller/callee subsumption at compile time, plus — in a `forge build`
binary — a seccomp sandbox that kills code exceeding its declared effects at the
syscall boundary (slag/boundary bodies included).

## 5. Slag rules

`#[slag]` is the escape hatch for unverified code (slag is the waste product of a
thermite burn) — the replacement for `unsafe`: harder to write, louder to read.

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
- The contract is STILL mandatory and L1-enforced at runtime — slag exempts you
  from PROVING, never from STATING and CHECKING.
- Every slag block appears in the build manifest and in `forge audit`; `grep
  slag` is the complete inventory of fiat-trusted code.
- CI policy hooks can cap slag count or require a second-party sign-off.

The polarity inversion is the point: verification is the default and costs
nothing; non-verification is the exotic add-on that costs more keystrokes and
more visibility.
