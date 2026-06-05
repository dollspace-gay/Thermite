//! The `THERMITE.skill.md` generator + the deterministic token-count heuristic
//! that backs the ≤ 6,000-token CI budget gate.
//!
//! Governing design: `.design/skill/skill-generator.md` (REQ-1..REQ-6).
//! Thesis: `thermite-design.md` §2.2 (the ≤ 6,000-token hard budget),
//! §10 (the skill IS the spec; the combinator section is regenerated from the
//! registry; one example per combinator; "no version skew"), §4/§4.2/§4.4
//! (the surface grammar), §6 (the ladder, incl. the L0/slag clarification),
//! §8 (the slag rules), Appendix B (the Forge command surface).
//!
//! [`generate`] assembles the five §10 sections — (1) surface grammar,
//! (2) the SpecTherm combinator library, (3) the Forge command set, (4) the
//! ladder semantics, (5) the slag rules — into one deterministic `String`.
//! Section (2) is **machine-rendered** from `thermite_spec::all()` (the frozen
//! registry), so adding/removing a combinator auto-appears/auto-drops in the
//! skill (§10 anti-drift). Sections (1)/(3)/(4)/(5) are curated, templated
//! strings sourced from the design and versioned with the toolchain. No I/O, no
//! env, no wall-clock, no RNG — a pure function of the compiled-in curated text
//! and the static registry (R-CODE-5).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (`generate()` API + five canonical sections) | SHIPPED | `pub fn generate` concatenates `render_grammar`/`render_combinators`/`render_forge`/`render_ladder`/`render_slag` in §10 order; consumed by `main::run` (the `--emit`/`--check-budget` bin) and the freshness/coverage tests. |
//! | REQ-2 (combinator section machine-rendered from `all()`) | SHIPPED | `render_combinators` iterates `thermite_spec::all()`, renders each entry's surface signature from `name`/`arity`/`arg_kinds`/`result` + one example from `example_for`; consumed by `generate`. Verified: `combinator_coverage` asserts every `all()` name + an example marker appears. |
//! | REQ-3 (curated grammar/forge/ladder/slag sections) | SHIPPED | `render_grammar`/`render_forge`/`render_ladder`/`render_slag` return compiled-in strings sourced from §4/§4.2/§4.4 + Appendix B + §6 + §8; consumed by `generate`. Verified: `grammar_forge_slag_coverage`, `ladder_coverage`. |
//! | REQ-4 (deterministic token count + ≤ 6,000 gate) | SHIPPED | `pub fn token_count` = `(chars*2).div_ceil(7)` (ceil(chars/3.5), integer, deterministic); `SKILL_TOKEN_BUDGET = 6000`; consumed by `main::run` (`--check-budget`) and `budget_gate`. |
//! | REQ-5 (committed `THERMITE.skill.md` + up-to-date check) | SHIPPED | the repo-root `THERMITE.skill.md` is `generate()`'s output; `committed_skill_is_fresh` asserts the committed bytes `== generate()`. |
//! | REQ-6 (`thermite-skill` bin — `--emit`/`--check-budget`) | SHIPPED — see `main.rs` | `main::run` dispatches `--emit`→`generate()` and `--check-budget`→`token_count(generate())`; consumes both `generate` and `token_count`. |
//! | REQ-7 (CI `--check-budget` step) | SHIPPED — see `.github/workflows/ci.yml` | the `cargo run -p thermite-skill -- --check-budget` step in `ci.yml` runs the gate in CI. |

use thermite_spec::{ArgKind, CombinatorSig, ResultKind};

/// The hard token budget for `THERMITE.skill.md` (`thermite-design.md` §2.2:
/// "≤ 6,000 tokens … This is a hard budget, enforced in CI"). The design's
/// symbolic constant, not a value read back from the generator (R-CHAR-3).
pub const SKILL_TOKEN_BUDGET: usize = 6000;

/// Count the (conservative, deterministic) token estimate of `s`.
///
/// The estimate is `ceil(char_count / 3.5)`, computed in integer arithmetic as
/// `(chars * 2).div_ceil(7)` to avoid any float non-determinism (R-CODE-5).
/// `char_count` is `str::chars().count()` (Unicode scalar values — stable across
/// runs and platforms).
///
/// This is a HEURISTIC, not a model-backed BPE tokenizer: it has no dependency
/// and no committed model blob, so it is trivially reproducible. The `/3.5`
/// divisor OVER-counts relative to a real cl100k tokenizer (markdown + code +
/// identifier-heavy text typically lands near 3.5–4.5 chars/token), so the gate
/// fails EARLY — a skill this heuristic passes is comfortably under a real
/// tokenizer's 6,000. The method is swappable for an exact tokenizer behind this
/// one function without touching the gate or the budget constant
/// (`.design/skill/skill-generator.md` REQ-4 DECISION / OQ-1).
pub fn token_count(s: &str) -> usize {
    (s.chars().count() * 2).div_ceil(7)
}

/// Assemble the canonical `THERMITE.skill.md` as one deterministic `String`.
///
/// The five sections appear in `thermite-design.md` §10 order: (1) surface
/// grammar, (2) the SpecTherm combinator library (one example each), (3) the
/// Forge command set, (4) the ladder semantics, (5) the slag rules. Section (2)
/// is machine-rendered from `thermite_spec::all()` (REQ-2); the rest are curated
/// (REQ-3). Pure: no I/O, no env, no clock, no RNG (REQ-1 / R-CODE-5 / AC-6).
pub fn generate() -> String {
    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str(&render_grammar());
    out.push_str(&render_combinators());
    out.push_str(&render_forge());
    out.push_str(&render_ladder());
    out.push_str(&render_slag());
    out
}

/// The skill preamble: title + the regeneration command (so an editor knows the
/// file is generated and how to refresh it — REQ-5).
const HEADER: &str = "\
# THERMITE.skill.md

The complete Thermite v0.1 surface language and toolchain, in one file. This is
the canonical language definition (`thermite-design.md` §10): an agent reads it
at session start and holds the entire language in context. It is GENERATED — do
not edit by hand. Regenerate with:

    cargo run -p thermite-skill -- --emit > THERMITE.skill.md

Budget: this file must stay under 6,000 tokens (a hard CI gate, design §2.2).

";

/// Section (1) — the surface grammar. CURATED from `thermite-design.md`
/// §4/§4.2/§4.4 and `.design/syntax/surface-grammar.md` (REQ-3): the three item
/// forms, the mandatory clause order, mandatory loop `inv`/`dec`, the one call
/// syntax, the expression/pattern/type/effect grammar, the flat-closure rule,
/// and the "removed from Rust" table.
fn render_grammar() -> String {
    String::from(
        "\
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

",
    )
}

/// Render the surface type a single argument `ArgKind` presents in a usage
/// signature (REQ-2: `Slice`→`&[u32]`, `Index`→`usize`, `Pred`→a flat predicate
/// closure, `Value`→a scalar).
fn render_arg_kind(kind: ArgKind) -> &'static str {
    match kind {
        ArgKind::Slice => "&[u32]",
        ArgKind::Index => "usize",
        ArgKind::Pred => "|x| -> bool",
        ArgKind::Value => "u32",
    }
}

/// Render the surface result type a combinator yields (REQ-2: `Bool`→`bool`,
/// `Usize`→`usize`).
fn render_result_kind(kind: ResultKind) -> &'static str {
    match kind {
        ResultKind::Bool => "bool",
        ResultKind::Usize => "usize",
    }
}

/// The generator-side example table (REQ-2 / OQ-2): one usage example per
/// combinator name, keyed by `name`. The corpus-grounded four
/// (`sorted`/`forall_in`/`forall_below`/`forall_from`) take their examples from
/// the `binary_search` contract (`thermite-design.md` §4.1); the §4.2-named four
/// (`exists_in`/`count_where`/`permutation_of`/`disjoint`) take a hand-written
/// illustrative example. Examples are a SKILL concern, not a registry field, so
/// they live here, not in `CombinatorSig`. A combinator added to the registry
/// without a mapping falls back to a generic example (so the renderer never
/// panics — R-CODE-2) and the coverage test still pins its name + the example
/// marker (AC-2), making the gap visible without an abort.
fn example_for(name: &str) -> &'static str {
    match name {
        "sorted" => "req sorted(haystack)",
        "forall_in" => "ens forall_in(haystack, |x| x != needle)",
        "exists_in" => "ens exists_in(haystack, |x| x == needle)",
        "count_where" => "ens count_where(xs, |x| x == 0) <= xs.len()",
        "permutation_of" => "ens permutation_of(result, input)",
        "disjoint" => "req disjoint(lefts, rights)",
        "forall_below" => "inv forall_below(haystack, lo, |x| x < needle)",
        "forall_from" => "inv forall_from(haystack, hi, |x| x > needle)",
        _ => "ens forall_in(xs, |x| true)",
    }
}

/// Section (2) — the SpecTherm combinator library. MACHINE-RENDERED from
/// `thermite_spec::all()` (REQ-2): for EVERY entry and ONLY those entries, the
/// surface signature (name + arg-kinds + result) + one usage example. Adding a
/// combinator to the frozen registry makes it auto-appear here; removing one
/// auto-drops it (§10 anti-drift). The verbose Verus(L3)/L1 bodies the registry
/// also carries are NOT rendered — the skill teaches the surface signature, not
/// the lowering bodies.
fn render_combinators() -> String {
    let mut s = String::from(
        "\
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

",
    );
    for sig in thermite_spec::all() {
        s.push_str(&render_one_combinator(sig));
    }
    s
}

/// Render one combinator's surface signature + one example as a markdown bullet
/// (the per-entry body of [`render_combinators`], REQ-2).
fn render_one_combinator(sig: &CombinatorSig) -> String {
    let mut args = String::new();
    for (i, kind) in sig.arg_kinds.iter().enumerate() {
        if i > 0 {
            args.push_str(", ");
        }
        args.push_str(render_arg_kind(*kind));
    }
    format!(
        "- `{name}({args}) -> {result}`\n  // example: {example}\n",
        name = sig.name,
        args = args,
        result = render_result_kind(sig.result),
        example = example_for(sig.name),
    )
}

/// Section (3) — the Forge command set. CURATED from `thermite-design.md`
/// Appendix B (the v0.1 command surface) + §5.1 framing (REQ-3).
fn render_forge() -> String {
    String::from(
        "\n\
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
forge battery [item]               run vacuity battery + mutation scoring
forge audit                        full slag + boundary + assurance inventory
forge skill                        emit the canonical THERMITE.skill.md
forge repair [item]                background L1/L2 -> L3 upgrade loop
```

Items and blocks have stable semantic addresses (`binary_search.loop#1.inv#2`);
edits take addresses, not string matches.

",
    )
}

/// Section (4) — the ladder semantics. CURATED from `thermite-design.md` §6
/// (REQ-3), INCLUDING the L0/slag clarification (slag → L1 with `slag: true`;
/// L0 is the body-proof aspect).
fn render_ladder() -> String {
    String::from(
        "\
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
is likewise always enforced, independent of the proof level.

",
    )
}

/// Section (5) — the slag rules. CURATED from `thermite-design.md` §8 (REQ-3):
/// mandatory non-empty `reason`/`owner`/`review`, contract still enforced at L1,
/// `grep slag` as the complete inventory, the polarity inversion.
fn render_slag() -> String {
    String::from(
        "\
## 5. Slag rules

`#[slag]` is the escape hatch for unverified code (slag is the waste product of
a thermite burn). It is the replacement for `unsafe`: harder to write, louder to
read.

```thermite
#[slag(reason = \"vendored SIMD intrinsics; contract checked at boundary by L1\",
       owner  = \"agent:forge-7/session-2026-06-04\",
       review = \"required\")]
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
",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_gate() {
        // AC-1: the §2.2 symbolic budget (SKILL_TOKEN_BUDGET), not a value read
        // back from the generator (R-CHAR-3).
        let count = token_count(&generate());
        assert!(
            count <= SKILL_TOKEN_BUDGET,
            "skill is {count} tokens, over the {SKILL_TOKEN_BUDGET} budget"
        );
    }

    #[test]
    fn token_count_is_ceil_chars_over_3_5() {
        // The heuristic is integer ceil(chars / 3.5) == (chars*2).div_ceil(7).
        assert_eq!(token_count(""), 0);
        // 7 chars -> 14/7 = 2 exactly.
        assert_eq!(token_count("abcdefg"), 2);
        // 1 char -> ceil(2/7) = 1 (conservative, never zero for nonempty).
        assert_eq!(token_count("a"), 1);
        // 8 chars -> ceil(16/7) = 3.
        assert_eq!(token_count("abcdefgh"), 3);
    }

    #[test]
    fn combinator_coverage() {
        // AC-2: every entry in the frozen registry appears by name AND has an
        // example marker. Expected source is the registry itself (R-CHAR-3 — the
        // anti-drift contract is "the skill mirrors all()").
        let skill = generate();
        for sig in thermite_spec::all() {
            assert!(
                skill.contains(sig.name),
                "skill is missing combinator name `{}`",
                sig.name
            );
        }
        // One `// example:` line per registry entry.
        let example_lines = skill.matches("// example:").count();
        assert_eq!(
            example_lines,
            thermite_spec::all().len(),
            "expected one example per combinator"
        );
    }

    #[test]
    fn ladder_coverage() {
        // AC-3: all four ladder labels + the L0/slag clarification.
        let skill = generate();
        for level in ["L0", "L1", "L2", "L3"] {
            assert!(
                skill.contains(level),
                "skill is missing ladder level {level}"
            );
        }
        assert!(skill.contains("slag: true"));
        assert!(skill.contains("exempts PROVING, never STATING and CHECKING"));
    }

    #[test]
    fn grammar_forge_slag_coverage() {
        // AC-4: forge verbs, slag fields, grammar keywords (expected strings
        // derived from Appendix B / §8 / §4).
        let skill = generate();
        for verb in [
            "forge new",
            "forge goal",
            "forge fill",
            "forge edit",
            "forge check",
            "forge battery",
            "forge audit",
            "forge skill",
            "forge repair",
        ] {
            assert!(skill.contains(verb), "skill is missing `{verb}`");
        }
        for field in ["reason", "owner", "review"] {
            assert!(
                skill.contains(field),
                "skill is missing slag field `{field}`"
            );
        }
        for kw in ["req", "ens", "fx", "inv", "dec", "spec fn", "#[slag]"] {
            assert!(skill.contains(kw), "skill is missing grammar marker `{kw}`");
        }
    }

    #[test]
    fn determinism() {
        // AC-6: pure function — and no wall-clock content.
        assert_eq!(generate(), generate());
        let skill = generate();
        // No ISO date / time pattern leaked into the output (the only date is
        // the static §8 slag example owner string, which is curated content).
        assert!(!skill.contains("2026-06-04T"));
    }

    #[test]
    fn sections_in_canonical_order() {
        // REQ-1: the five sections appear in §10 order.
        let skill = generate();
        let g = skill.find("## 1. Surface grammar").expect("section 1");
        let c = skill.find("## 2. SpecTherm").expect("section 2");
        let f = skill.find("## 3. Forge").expect("section 3");
        let l = skill.find("## 4. Verification ladder").expect("section 4");
        let s = skill.find("## 5. Slag rules").expect("section 5");
        assert!(g < c && c < f && f < l && l < s, "sections out of order");
    }
}
