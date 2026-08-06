---
rfc: 6
title: Full words for clause keywords, and the effect row on the arrow
status: draft
supersedes: []
introduces:
  - REQ-SYNTAX-CLAUSE-FULL-WORDS
  - REQ-SYNTAX-CLAUSE-ORDER
  - REQ-SYNTAX-CLAUSE-CONJUNCTS
  - REQ-SYNTAX-CLAUSE-TRIVIAL
  - REQ-SYNTAX-EFFECT-ROW-LEADING
  - REQ-SYNTAX-ADDRESS-CLAUSE-SEGMENTS
---

# Full words for clause keywords, and the effect row on the arrow

Kind: rename and reorder. This RFC adds no expressive power, no obligation, no
type, and no metatheory. Every existing program keeps its meaning, and every one
of them changes.

## The change

| from | to |
|---|---|
| `req P` | `requires P` |
| `ens P` | `ensures P` |
| `inv P` | `keeps P` |
| `dec E` | `measures E` |
| `fx E` | `! E`, moved to the head of the contract |

Plus three things that follow from those.

**Clause order becomes** the effect row, then the bare clauses, then `measures`
last. Today `parse_contract` enforces `req` ×1, `ens` ×1+, `fx` ×1, and a
recursive `fn` carries its `dec` after the row.

**A clause body may be a block of conjuncts**, with the bare single-expression
form as sugar:

```thermite
requires {
  cpu < 64;
  (s.expected >> cpu) & 1 == 1;
}
```

**`requires nothing`** is sugar for `requires true`.

That is the entire proposal.

## Why

Thermite is designed to be written principally by language models. That makes
**semantic overlap with pretraining worth more than token economy**.

The failure mode of abbreviation is not that a model cannot learn it. It is that
abbreviations misdirect: `fx` reads as audio/visual effects, `dec` as
declare/decimal/decrement, `inv` as inverse or inventory. Those are wrong priors
rather than absent ones. In a language where a misread clause yields a vacuous
proof rather than a compile error, that is a safety property rather than an
ergonomic one.

**The evidence is already paid, in this repository.** A `guar`/`ens` collision
consumed several design cycles in a proposal for this language, and it happened
because both were abbreviated. `guarantees` and `ensures` do not collide. The
abbreviation destroyed the information that would have prevented the clash.

The current spelling is also a departure from both parents. Verus uses full words
heavily — in vstd, `ensures` 804, `requires` 465, `decreases` 161, `invariant`
117. Rust is mixed, abbreviating only its most ubiquitous tokens: `fn`, `mut`,
`pub`. `req`/`ens`/`fx`/`inv`/`dec` is neither.

The counter-argument, that idiosyncrasy makes fine-tuning more specifying, is
real. It is judged to lose against the loss of semantic overlap.

## Why these particular words

`requires` and `ensures` are Verus's, and need no defence.

`keeps` and `measures` are not, so they do:

> **Every clause is a third-person-singular verb whose subject is the item.** A
> clause is a sentence with the subject elided, and the item supplies it.

```
f          requires  n < 100
f          ensures   result == n * 2
f          measures  p.count
the loop   keeps     acked & !expected == 0
Grant      keeps     base + len <= MAX_PHYS
```

`requires` and `ensures` already obey this. `inv` is a noun in a verb slot, which
is why it never sat right beside them, and `dec` names the expression's property
rather than the clause's purpose.

`measures` also fixes a mechanical problem. A clause keyword is a
semantic-address segment, and `validate_segments` matches a fixed allowlist after
splitting on `.`, so a clause keyword must be one word. A two-word spelling such
as `terminates by` is rejected as malformed before any lookup:

```
double.ens              → no such address     (segment well-formed)
double.terminates by    → malformed address   (rejected before lookup)
```

## Why the row moves

`fx E` is a noun phrase sitting among verb phrases, and it is not a claim about
behaviour. It is part of the type: `() ! pure` and `() ! write(shootdown)` are
different types to the prover, so the row belongs to the arrow. `!` follows
Koka's `-> B ! e`.

```thermite
fn allocate(pages: u64) -> Result<Grant, u64>
  ! write(heap)
  requires  pages > 0 && pages <= 1024
  ensures   ...
```

The line the position draws is worth stating, because it is the one that decides
where a future feature belongs:

> An effect propagates up the call graph by construction. A clause is proved at
> the item.

`!` needs no new token: it already lexes as `TokKind::Bang`. The row is
unambiguous by position, checked rather than assumed. Migrating the corpus and
scanning every line gives **142 lines opening with a row and zero lines opening
with `!` that are not one**. The remaining three of the lexer's 145 `fx` tokens
are accounted for: one row sits inline in `conformance/review/vacuous.th`, and
two are in `conformance/parse/recover_per_item.th`, which does not parse by
design. Clause expressions that open with a negation are unaffected, because
there the `!` follows a keyword and never opens a line.

## An argument the migration turned up

After the rename, Thermite's `requires` and Verus's `requires` are the same word,
so **lowering becomes identity rather than translation** for four of the five
clauses. Emitted Verus reads against its Thermite source directly, and a reader
comparing the two no longer holds a mapping in their head.

That was not why the change was proposed, and it is a real argument for it.

## Scope

Measured by the pinned lexer across the 67 `.th` files tracked at `84d276e7`. A
`req` token is a clause and an identifier spelled `req` is not, which is the
distinction a textual count cannot make.

| | sites |
|---|---|
| `ens` | 205 |
| `req` | 152 |
| `fx` | 145 |
| `dec` | 26 |
| `inv` | 19 |
| **total** | **547**, across 144 contracts |
| `req true` | 100 |
| `ens true` | 2 |

The volume is not there. **1,527 further clause sites live inside Rust string
literals**, across 450 `.th` fragments in 111 test files. Three quarters of the
migration is in the test suite.

## The compiler change

Scoped against the tree at `84d276e7` rather than estimated, and built as a spike
to check the estimate.

| | |
|---|---|
| `keyword_kind` entries (`lexer.rs:217`) | 5 |
| `TokKind` variants | 5, keeping their names, so 53 downstream references do not move |
| `parse_contract` (`parser.rs:1420`) | accept the row first, one or more `requires`, `measures` last |
| `validate_segments` (`address.rs:331`, `:347`) | 2 lines |
| clause names in diagnostic strings | several in `parser.rs`, user-facing |

**The spike measured 63 insertions and 62 deletions across five files** in
`thermite-syntax` — lexer, parser, addresses, AST and lib. `fx` is removed as a
keyword entirely, since the row is `!` and `Bang` already lexed.

Conjunct blocks are the one part that adds a production rather than renaming a
token. They desugar to repeated clauses, so nothing downstream of the AST needs
to know about them, which is the argument for including them here.

**Correcting a claim in this proposal's own history:** `fx` is not last today. A
recursive function carries `dec` after it — `examples/editor/editor.th` reads
`ens … fx pure … dec end - i` — so the new order is a genuine move rather than a
no-op.

## Migration

**It is mechanical.** A rename plus a fixed reorder is a deterministic
source-to-source rewrite: nothing about it depends on what a program means. The
rewriter edits spans rather than reprinting files, so comments, blank lines,
expression text and alignment survive untouched. A formatter would produce a diff
nobody can review, and this change does not need one.

**The front end drives it, so the hard case is exact.** The hard case is a
contract written on one line, which is most of the test corpus:

```thermite
fn id(x: u32) -> u32 req true ens result == x fx pure { x }
```

Moving the row to the front means knowing where the contract ends and the body
begins, and that is parsing rather than matching. A rewriter linking
`thermite-syntax` takes item boundaries from `parse` and every offset from
`tokenize`; two facts from the grammar then settle it. A clause keyword is a
reserved token, so `TokKind::Req` is a clause and an identifier spelled `req`
cannot be one. And `parse_effect_row` is a closed grammar containing no brace, so
the row ends at the first token that cannot continue it and the body's `{` is
whatever follows.

Measured on a `git archive` export of the pin:

| | |
|---|---|
| `.th` corpus | **66 of 67 migrate**, with no clause keyword surviving |
| the one decline | `conformance/parse/recover_per_item.th`, whose purpose is to not parse |
| `.th` fragments in Rust literals | **450 migrate**, carrying 1,527 clause sites |
| declined, carrying no clause keyword | 340, where declining costs nothing |
| declined and clause-bearing | **43**: `format!` templates, assertion prose, and fixtures invalid on purpose |

Those 43 are a hand-reviewable list rather than a residue, and they are listed in
the implementation PR rather than here.

**A note on checking, because it generalises.** An earlier rewriter matched a
clause keyword at the head of a line and proved itself by round-trip —
`to_v2(to_v3(x)) == x`, byte for byte, on 382 of 382 files. That check is silent
about text a tool never touches, because untouched text is restored perfectly,
and the silence hid every one-line contract and 17 `@bv`-tagged clauses across 10
files. A migrated corpus would have carried `ens@bv64` into a front end with no
`ens` keyword. Reversibility is worth having and does not measure coverage.
Parsing both sides and comparing ASTs is the replacement, and it checks meaning
rather than text.

**The one non-mechanical part is optional.** A naive rewriter emits
`requires true` rather than `requires nothing`. That stays legal, since `true`
remains legal inside expressions and the sugar is clause-level only, so adopting
it is a second pass over 100 sites rather than a correctness condition.

**Certificates survive**, and this was checked rather than assumed. The
`.cert.json` oracle subset is `item` / `level` / `tautology` /
`vacuous_precondition` / `effects` / `slag`. Clause names appear nowhere in it, so
no oracle is invalidated and the migration is source-only.

**And the migrated corpus certifies identically**, which is the check a
round-trip cannot make. Baseline is `forge check` at `84d276e7` on the unmigrated
file; migrated is the patched front end on the rewriter's output:

| file | items at L3, before | after | exit, before / after |
|---|---|---|---|
| `parse_u64.th` | 1 | 1 | 0 / 0 |
| `list_sum.th` | 2 | 2 | 0 / 0 |
| `option_result.th` | 5 | 5 | 0 / 0 |
| `multi_adt.th` | 5 | 5 | 0 / 0 |
| `map_kv.th` | 1 of 4 | 1 of 4 | 1 / 1 |
| `bytes_eq_demo.th` | 4 | 4 | 0 / 0 |

Eighteen items at L3, same levels and the same exit status on every file, against
Verus `0.2026.05.24.ecee80a`. `map_kv.th` exits 1 in both directions: it carries
an `ens true` that §7.1(a) rejects as `EnsIsTrivial`, before the rename and after
it. The rename preserves meaning to the prover rather than only information in
the text.

## What is not in this proposal

Everything that would add capability, listed so that this one can be short:

```
survives · interleaves { asks / promises } · resource · forget / forgets(r)
opaque · by unfold(…) · shared declarations and checked effect rows
lock / owns / holding · protocol types · ensures on spec fn · blocks · cost(E)
handlers { } · an effect algebra
```

Each of those adds an obligation, a type, or a check.

**Two abbreviations are deliberately left alone.** `alloc` and `rand` are
abbreviations the principle above would rename. They are untouched because a
plausible later proposal turns them into `write(heap)` and `write(entropy)`
anyway, and renaming the same token twice is churn. Flagged so it reads as a
decision rather than an oversight.

## What this asks

A review and a CI run. The corpus certifies before and after, the migration is a
tool rather than a hand edit, and no proof obligation anywhere changes.

The migration and the parser change land as **one PR rather than two**, because
no front end accepts the new surface until the parser moves, so a migrated corpus
cannot certify on its own.

It is also a **version-number event**, which makes it a concrete test case for
[RFC-4](0004-versioning.md): a breaking change to every source file, with an
automated migration, and no change to what any program proves.

## Provenance

This proposal comes from outside the project. It was written while porting a
kernel subsystem to Thermite, where the clause vocabulary was a repeated source
of misreading, and the counts above come from measuring this repository at
`84d276e7` rather than from reading its documentation. The spike, the migration
tooling and the certification table are attached to the implementation PR.

Every gap that motivated it was found by attempting something rather than by
reading the reference, and several contradicted the documentation in both
directions. That buys evidence rather than standing: the proposal earns its way
on the design and the reproduction, and a maintainer's judgement on `keeps` and
`measures` is the deciding one.
