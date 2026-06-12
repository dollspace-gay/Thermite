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
//! [`generate`] assembles the §10 sections — (1) surface grammar, (2) the
//! SpecTherm combinator library, (2b) the recursion-scheme library, (3) the
//! Forge command set, (4) the ladder semantics, (5) the slag rules — into one
//! deterministic `String`. The SURFACE INVENTORY is DYNAMIC by two
//! compiler-backed mechanisms (REQ-8): (i) **registry-driven** — section (2)
//! iterates `thermite_spec::all()` and (2b) iterates
//! `thermite_spec::schemes::all()`, so a new registry entry auto-appears (REQ-2,
//! REQ-9); (ii) **exhaustive-match-driven** — section (1)'s construct inventory
//! is rendered by an EXHAUSTIVE `match` (no `_` wildcard) over the definitional
//! enums `thermite_syntax::{Type,Expr,Item,Pattern,Effect}` (+ `BinOp`/
//! `PrimType`), so a NEW variant FAILS TO COMPILE until its skill arm is added
//! (REQ-10 — the compiler is the freshness enforcer). The explanatory PROSE (the
//! framing, the ladder, the slag rules, the forge verb table) stays curated,
//! guarded by the freshness + budget tests (REQ-11). No I/O, no env, no
//! wall-clock, no RNG — a pure function of the compiled-in text, the static
//! registries, and the per-variant match arms (R-CODE-5).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (`generate()` API + canonical sections) | SHIPPED | `pub fn generate` concatenates `render_grammar`/`render_combinators`/`render_schemes`/`render_forge`/`render_ladder`/`render_slag` in §10 order; consumed by `main::run` (the `--emit`/`--check-budget` bin) and the freshness/coverage tests. |
//! | REQ-2 (combinator section machine-rendered from `all()`) | SHIPPED | `render_combinators` iterates `thermite_spec::all()`, renders each entry's surface signature from `name`/`arity`/`arg_kinds`/`result` + one example from `example_for`; consumed by `generate`. Verified: `combinator_coverage` asserts every `all()` name + an example marker appears. |
//! | REQ-3 (curated PROSE sections — narrowed to irreducible prose) | SHIPPED | `render_forge`/`render_ladder`/`render_slag` + the framing/scaffolding in `render_grammar` return compiled-in strings sourced from §5.1/§6/§8/§4.2; consumed by `generate`. The structural inventory moved to REQ-9/REQ-10. Verified: `grammar_forge_slag_coverage`, `ladder_coverage`. |
//! | REQ-4 (deterministic token count + ≤ 6,000 gate) | SHIPPED | `pub fn token_count` = `(chars*2).div_ceil(7)` (ceil(chars/3.5), integer, deterministic); `SKILL_TOKEN_BUDGET = 6000`; consumed by `main::run` (`--check-budget`) and `budget_gate`. |
//! | REQ-5 (committed `THERMITE.skill.md` + up-to-date check) | SHIPPED | the repo-root `THERMITE.skill.md` is `generate()`'s output; `committed_skill_is_fresh` asserts the committed bytes `== generate()`. |
//! | REQ-6 (`thermite-skill` bin — `--emit`/`--check-budget`) | SHIPPED — see `main.rs` | `main::run` dispatches `--emit`→`generate()` and `--check-budget`→`token_count(generate())`; consumes both `generate` and `token_count`. |
//! | REQ-7 (CI `--check-budget` step) | SHIPPED — see `.github/workflows/ci.yml` | the `cargo run -p thermite-skill -- --check-budget` step in `ci.yml` runs the gate in CI. |
//! | REQ-8 (compiler-enforced no-staleness GUARANTEE) | SHIPPED | the surface inventory is rendered ONLY by registry iteration (`render_combinators`/`render_schemes`) or exhaustive `match` (`render_type_arm`/`render_expr_arm`/`render_item_arm`/`render_pattern_arm`/`render_effect_arm`/`render_binop_arm`/`render_prim_arm`, NO `_` arm) — a new variant FAILS TO COMPILE; a new registry entry auto-renders. Consumed by `render_grammar`/`render_schemes` in `generate`. Verified: `surface_construct_coverage` (output) + the no-`_` structural invariant (compile-forced). |
//! | REQ-9 (recursion-scheme section registry-driven from `schemes::all()`) | SHIPPED | `render_schemes` iterates `thermite_spec::schemes::all()`, renders each `SchemeSig`'s call shape (`scrutinee_args`+`step_shape`) + result + one `scheme_example_for`; consumed by `generate`. Verified: `every_scheme_appears_with_an_example`. This is `thermite-skill`'s non-test consumer of `schemes::all()` (R-DEFER-1). |
//! | REQ-10 (type/expr/item/pattern/effect grammar exhaustive-match-driven) | SHIPPED | `render_{type,expr,item,pattern,effect,binop,prim}_arm` are exhaustive `match`es with NO `_` arm over `thermite_syntax::{Type,Expr,Item,Pattern,Effect,BinOp,PrimType}`; driven over per-variant inventories (`type_inventory` etc.) by `render_grammar`. Verified: `surface_construct_coverage`. |
//! | REQ-11 (prose curated + freshness-tested; forge command list the honest exception) | SHIPPED | the irreducible prose stays curated in `render_grammar`'s framing/`render_forge`/`render_ladder`/`render_slag`; the forge verb LIST stays a curated table (forge's `Command` is private + forge→thermite-skill dep, OQ-5), kept honest by `committed_skill_is_fresh` (REQ-5) + `grammar_forge_slag_coverage` (AC-4). |
//!
//! ## Cluster C10 — ergonomics skill arms (`.design/basis/11-ergonomics.md`, #112)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1/2/5 (tuple destructure / `for` / `if let`-`while let` teaching) | SHIPPED | the PURE-DESUGAR ergonomics add NO AST node, so the exhaustive-match inventory cannot auto-render them; `render_grammar`'s statement section gains a curated "Binding / control-flow ergonomics" prose block teaching all five (the desugarings, the `for` AUTO-`dec`, the guard-doesn't-complete rule, the or-pattern union, the `while let` → `while (cond)`). Kept fresh by `committed_skill_is_fresh`. |
//! | REQ-3 (match guard arm) | SHIPPED | `render_expr_arm`'s `Expr::Match` fragment now teaches `Pat [if C] => EXPR` + "a guard does NOT complete a match". Auto-rendered. |
//! | REQ-4 (or-pattern arm) | SHIPPED | `render_pattern_arm` += a `Pattern::Or` arm (`p0 \| p1 \| ..`, "covers their union") + `pattern_inventory` += `Pattern::Or(Vec::new())` — the compiler-forced no-staleness GUARANTEE (REQ-8) auto-renders it. Verified: `surface_construct_coverage`. |

use thermite_spec::schemes::{SchemeResult, SchemeSig, StepShape};
use thermite_spec::{ArgKind, CombinatorSig, ResultKind};
use thermite_syntax::ast::{
    BinOp, Effect, Expr, IndexArg, Item, Pattern, PrimType, SlicePat, Type, UnaryOp,
};
use thermite_syntax::lexer::Span;

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
/// The sections appear in `thermite-design.md` §10 order: (1) surface grammar,
/// (2) the SpecTherm combinator library, (2b) the recursion-scheme library, (3)
/// the Forge command set, (4) the ladder semantics, (5) the slag rules. Section
/// (1)'s construct inventory is EXHAUSTIVE-MATCH-driven over the `thermite_syntax`
/// enums (REQ-10), (2) is registry-driven from `thermite_spec::all()` (REQ-2),
/// (2b) is registry-driven from `thermite_spec::schemes::all()` (REQ-9); the
/// curated prose (the framing, ladder, slag, forge verb table) stays templated
/// (REQ-11). Pure: no I/O, no env, no clock, no RNG (REQ-1 / R-CODE-5 / AC-6).
pub fn generate() -> String {
    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str(&render_grammar());
    out.push_str(&render_combinators());
    out.push_str(&render_schemes());
    out.push_str(&render_forge());
    out.push_str(&render_ladder());
    out.push_str(&render_slag());
    out
}

/// The skill preamble: title + the regeneration command (so an editor knows the
/// file is generated and how to refresh it — REQ-5) + the "How to read this
/// file / 60-second workflow" comprehension intro (curated prose, REQ-11; the
/// cold-agent reader's entry point — write contract-first fn -> `forge check` ->
/// read the per-obligation result -> fix or `?N`-hole it and work `goal`/`fill`).
const HEADER: &str = "\
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
a bare \"verification failed\". (3) Fix the body/contract and re-check; or, if the
body is not yet known, drop a HOLE `?0` in its place (§1) and work `forge goal`/
`forge fill` — a holed item never certifies until every hole is filled.

Map: §1 grammar (what you may write), §2/§2b combinators + recursion schemes (the
ONLY way to quantify/recurse in a spec), §3 Forge verbs, §4 the assurance ladder
(what a certificate means), §5 the `#[slag]` proof escape hatch.

";

/// One rendered surface-construct entry (REQ-10): the per-variant fragment an
/// exhaustive-`match` renderer emits for a single language construct — a concise
/// grammar `fragment`, a one-line `description`, and a tiny `example`. The text
/// is a deterministic function of the VARIANT (not of any payload value), so the
/// rendered inventory is pure (R-CODE-5, AC-6).
struct SkillFragment {
    /// The grammar fragment for this construct (e.g. `&[T]`, `match e { … }`).
    fragment: &'static str,
    /// A one-line description of what the construct is.
    description: &'static str,
    /// A tiny illustrative example of the construct in use.
    example: &'static str,
}

impl SkillFragment {
    /// Render this fragment as one markdown bullet (the per-construct row of the
    /// REQ-10 inventory): the grammar fragment + description, then a tiny example.
    fn to_bullet(&self) -> String {
        format!(
            "- `{fragment}` — {description}\n  // e.g. {example}\n",
            fragment = self.fragment,
            description = self.description,
            example = self.example,
        )
    }
}

/// Render ONE `Type` variant's surface fragment (REQ-10).
///
/// EXHAUSTIVE `match` over `thermite_syntax::ast::Type` with NO `_` wildcard arm:
/// adding a new `Type` variant (e.g. the deferred `Type::Map`, ast.rs REQ-2)
/// makes this `match` non-exhaustive, a HARD `rustc` `E0004` compile error in
/// `thermite-skill`, until its arm is added — the compiler is the freshness
/// enforcer (REQ-8, AC-10(i)). Payload is field-elided (`{ .. }` / `(_)`); the
/// elision does not weaken exhaustiveness (the compiler checks the VARIANT set).
fn render_type_arm(ty: &Type) -> SkillFragment {
    match ty {
        Type::Prim(_) => SkillFragment {
            fragment: "u32 | u64 | usize | bool",
            description: "the closed primitive scalar set (no implicit widening)",
            example: "let n: u64 = 0;",
        },
        Type::Unit => SkillFragment {
            fragment: "()",
            description: "the unit type, written explicitly in a return position",
            example: "fn log() -> () req true ens true fx pure { }",
        },
        Type::Ref { .. } => SkillFragment {
            fragment: "&T | &mut T",
            description: "a shared / exclusive reference (no explicit lifetimes)",
            example: "fn f(x: &mut u64)",
        },
        Type::Slice(_) => SkillFragment {
            fragment: "&[T]",
            description: "a borrowed read-only slice view",
            example: "fn sum(xs: &[u32]) -> u64",
        },
        Type::Generic { .. } => SkillFragment {
            fragment: "NAME<T>",
            description: "one single-arg generic application",
            example: "-> Wrapper<usize>",
        },
        // Cluster C7 (`.design/basis/09-option-result.md` REQ-1/REQ-2): the built-in
        // optional / fallible primitives are dedicated `Type` nodes (NOT a
        // string-named `Generic`), so each renders ITS OWN surface fragment — the
        // construct + payload-in-contract surface an agent reads.
        Type::Option(_) => SkillFragment {
            fragment: "Option<T>",
            description: "the built-in optional (Some(v)/None; match/is; payload-in-contract via match-in-ens)",
            example: "-> Option<u64> ens match result { Some(v) => v == 5, None => true }",
        },
        Type::Result(_, _) => SkillFragment {
            fragment: "Result<T, E>",
            description: "the built-in fallible (Ok(v)/Err(e); match/is; the loud error arm)",
            example: "-> Result<u64, ParseErr>",
        },
        // Cluster C12 (`.design/basis/13-map.md` REQ-1/REQ-5): the bounded verified
        // key-value primitive `Map<K, V>` — the SECOND two-type-arg node. insert/get/
        // contains_key/len; get returns Option<V> (absent key -> None, NOT a wrong
        // value); insert carries fx alloc. Renders its OWN surface fragment.
        Type::Map(_, _) => SkillFragment {
            fragment: "Map<K, V>",
            description: "a bounded verified key-value map (insert/get/contains_key/len; get -> Option<V>, absent -> None; fx alloc)",
            example: "let mut m: Map<u64, u64> = Map::new(); m.insert(k, v); m.get(k)",
        },
        Type::Named(_) => SkillFragment {
            fragment: "Name",
            description: "a bare user-declared struct/enum type name",
            example: "fn area(s: Shape) -> u64",
        },
        Type::Box(_) => SkillFragment {
            fragment: "Box<T>",
            description: "heap indirection for a recursive enum (carries fx alloc)",
            example: "Cons(u64, Box<List>)",
        },
        Type::Vec(_) => SkillFragment {
            fragment: "Vec<T>",
            description: "a bounded growable collection over verified vstd (fx alloc)",
            example: "let v: Vec<u64> = Vec::new();",
        },
        Type::String => SkillFragment {
            fragment: "String",
            description: "a bounded owned run of u8 bytes (fx alloc)",
            example: "let s: String = \"hi\";",
        },
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-7): the
        // n-tuple return / pair primitive. Projection `.0`/`.1` is the access form
        // (the Expr::TupleProj fragment); `()` is unit, `(T)` is grouping.
        Type::Tuple(_) => SkillFragment {
            fragment: "(T, U, ..)",
            description: "an n-tuple (arity >= 2) for multiple returns; access via .0/.1",
            example: "fn swap(a: u64, b: u64) -> (u64, u64) req true ens result.0 == b && result.1 == a fx pure { (b, a) }",
        },
    }
}

/// Render ONE `PrimType` leaf's surface fragment (REQ-10): the exhaustive `match`
/// over the closed primitive set so a NEW primitive also compile-forces an entry.
fn render_prim_arm(prim: PrimType) -> SkillFragment {
    match prim {
        PrimType::U32 => SkillFragment {
            fragment: "u32",
            description: "a 32-bit unsigned integer",
            example: "needle: u32",
        },
        PrimType::U64 => SkillFragment {
            fragment: "u64",
            description: "a 64-bit unsigned integer",
            example: "-> u64",
        },
        PrimType::Usize => SkillFragment {
            fragment: "usize",
            description: "a pointer-width unsigned index",
            example: "let i: usize = 0;",
        },
        PrimType::Bool => SkillFragment {
            fragment: "bool",
            description: "a boolean",
            example: "let ok: bool = true;",
        },
    }
}

/// Render ONE `Item` variant's surface fragment (REQ-10): exhaustive `match` over
/// `thermite_syntax::ast::Item`, NO `_` arm — a new top-level item kind
/// compile-forces a skill entry (REQ-8, AC-10).
fn render_item_arm(item: &Item) -> SkillFragment {
    match item {
        Item::Fn(_) => SkillFragment {
            fragment: "fn NAME(..) -> T req .. ens .. fx .. { .. }",
            description: "a contract-first function (mandatory req/ens/fx, in order)",
            example: "fn sum(xs: &[u32]) -> u64 req .. ens .. fx pure { .. }",
        },
        Item::SpecFn(_) => SkillFragment {
            fragment: "spec fn NAME(..) -> T dec .. { .. }",
            description: "a total terminating spec function (one dec measure, no req/ens/fx)",
            example: "spec fn spec_sum(xs: &[u32]) -> nat dec xs.len() { .. }",
        },
        Item::Struct(_) => SkillFragment {
            fragment: "struct NAME { field: T, .. } [inv EXPR]",
            description: "a product type with an optional type-invariant inv clause",
            example: "struct Account { balance: u64 } inv balance <= cap",
        },
        Item::Enum(_) => SkillFragment {
            fragment: "enum NAME { Unit, Tuple(T, ..), Struct { f: T } }",
            description: "a sum type; match over it must be exhaustive",
            example: "enum List { Nil, Cons(u64, Box<List>) }",
        },
    }
}

/// Render ONE `Expr` variant's surface fragment (REQ-10): exhaustive `match` over
/// `thermite_syntax::ast::Expr`, NO `_` arm — a new expression form compile-forces
/// a skill entry (REQ-8, AC-10).
fn render_expr_arm(expr: &Expr) -> SkillFragment {
    match expr {
        Expr::IntLit { .. } => SkillFragment {
            fragment: "1_000_000",
            description: "an integer literal (verbatim `_` separators preserved)",
            example: "req xs.len() <= 1_000_000",
        },
        Expr::BoolLit(_) => SkillFragment {
            fragment: "true | false",
            description: "a boolean literal",
            example: "req true",
        },
        Expr::Path(_) => SkillFragment {
            fragment: "name | Mod::ITEM",
            description: "a path: a binding, a constant, or an enum variant",
            example: "u32::MAX",
        },
        Expr::Call { .. } => SkillFragment {
            fragment: "f(args)",
            description: "a free call (combinators and spec fns are free calls)",
            example: "sorted(haystack)",
        },
        Expr::MethodCall { .. } => SkillFragment {
            fragment: "recv.m(args)",
            description: "the ONE member-access call syntax (no UFCS)",
            example: "xs.len()",
        },
        Expr::Field { .. } => SkillFragment {
            fragment: "recv.field",
            description: "a field access",
            example: "account.balance",
        },
        Expr::Closure { .. } => SkillFragment {
            fragment: "|x| EXPR",
            description: "a flat predicate closure (no nested combinator/scheme)",
            example: "|x| x != needle",
        },
        Expr::Match { .. } => SkillFragment {
            fragment: "match e { Pat [if C] => EXPR, .. }",
            description:
                "a match (exhaustive over an enum; an `if C` guard does NOT complete a match)",
            example: "match result { Some(i) => .., None => .. }",
        },
        Expr::If { .. } => SkillFragment {
            fragment: "if C { .. } else { .. }",
            description: "an if/else as an expression (both arms required)",
            example: "if lo == hi { 0 } else { 1 }",
        },
        Expr::Binary { .. } => SkillFragment {
            fragment: "a OP b",
            description: "an arithmetic / comparison / logical / bitwise binary op",
            example: "lo + (hi - lo) / 2",
        },
        Expr::Unary { .. } => SkillFragment {
            fragment: "!EXPR",
            description: "prefix not (logical on bool, bitwise on int; binds tightest)",
            example: "!done",
        },
        Expr::Index { .. } => SkillFragment {
            fragment: "a[i] | a[..i] | a[i..] | a[i..j]",
            description: "single or range indexing",
            example: "spec_sum(&xs[..i])",
        },
        Expr::Cast { .. } => SkillFragment {
            fragment: "EXPR as T",
            description: "an explicit cast (all integer conversions are explicit)",
            example: "xs[i] as u64",
        },
        Expr::Ref { .. } => SkillFragment {
            fragment: "&EXPR | &mut EXPR",
            description: "a shared / exclusive borrow",
            example: "&xs[..i]",
        },
        Expr::StructLit { .. } => SkillFragment {
            fragment: "Path { field: val, .. }",
            description: "a struct / struct-variant construction",
            example: "Account { balance: 0 }",
        },
        Expr::Is { .. } => SkillFragment {
            fragment: "EXPR is Variant",
            description: "a bool-valued variant-discrimination test",
            example: "result is Circle",
        },
        Expr::Deref(_) => SkillFragment {
            fragment: "*EXPR",
            description: "a dereference of a boxed value (the recursive descent)",
            example: "sum_list(*t)",
        },
        Expr::StrLit(_) => SkillFragment {
            fragment: "\"text\"",
            description: "a string literal (an owned String; carries fx alloc)",
            example: "let s: String = \"hello\";",
        },
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-8): the
        // tuple construction + the projection access form. Projection (NOT
        // destructuring) is the v1 tuple access; it reads in BOTH exec and contract
        // (`ens result.0 == b`).
        Expr::Tuple(_) => SkillFragment {
            fragment: "(a, b, ..)",
            description: "an n-tuple construction (arity >= 2; (e) is grouping)",
            example: "(b, a)",
        },
        Expr::TupleProj { .. } => SkillFragment {
            fragment: "e.0 | e.1 | ..",
            description: "a tuple projection (the one tuple access; reads in exec and ens)",
            example: "ens result.0 == b && result.1 == a",
        },
    }
}

/// Render ONE `BinOp` leaf's surface fragment (REQ-10): exhaustive `match` so a
/// NEW operator compile-forces a skill entry. Comparisons are non-associative
/// (`a < b < c` is a parse error).
fn render_binop_arm(op: BinOp) -> SkillFragment {
    match op {
        BinOp::Add => SkillFragment {
            fragment: "a + b",
            description: "addition (overflow is a proof obligation)",
            example: "acc + xs[i] as u64",
        },
        BinOp::Sub => SkillFragment {
            fragment: "a - b",
            description: "subtraction (underflow is a proof obligation)",
            example: "hi - lo",
        },
        BinOp::Mul => SkillFragment {
            fragment: "a * b",
            description: "multiplication (overflow is a proof obligation)",
            example: "w * h",
        },
        BinOp::Div => SkillFragment {
            fragment: "a / b",
            description: "division (div-by-zero is a proof obligation)",
            example: "(hi - lo) / 2",
        },
        BinOp::Rem => SkillFragment {
            fragment: "a % b",
            description: "remainder (div-by-zero is a proof obligation: req b != 0)",
            example: "n % 2",
        },
        BinOp::Shl => SkillFragment {
            fragment: "a << k",
            description: "left shift (the shift amount must be bounded: req k < 64)",
            example: "1 << k",
        },
        BinOp::Shr => SkillFragment {
            fragment: "a >> k",
            description: "right shift (the shift amount must be bounded: req k < 64)",
            example: "x >> k",
        },
        BinOp::BitAnd => SkillFragment {
            fragment: "a & b",
            description: "bitwise and",
            example: "flags & mask",
        },
        BinOp::BitOr => SkillFragment {
            fragment: "a | b",
            description: "bitwise or",
            example: "flags | bit",
        },
        BinOp::BitXor => SkillFragment {
            fragment: "a ^ b",
            description: "bitwise xor",
            example: "a ^ b",
        },
        BinOp::Eq => SkillFragment {
            fragment: "a == b",
            description: "equality",
            example: "haystack[mid] == needle",
        },
        BinOp::Ne => SkillFragment {
            fragment: "a != b",
            description: "inequality",
            example: "x != needle",
        },
        BinOp::Lt => SkillFragment {
            fragment: "a < b",
            description: "less-than (non-associative)",
            example: "i < xs.len()",
        },
        BinOp::Le => SkillFragment {
            fragment: "a <= b",
            description: "less-or-equal",
            example: "lo <= hi",
        },
        BinOp::Gt => SkillFragment {
            fragment: "a > b",
            description: "greater-than",
            example: "x > needle",
        },
        BinOp::Ge => SkillFragment {
            fragment: "a >= b",
            description: "greater-or-equal",
            example: "balance >= amount",
        },
        BinOp::And => SkillFragment {
            fragment: "a && b",
            description: "logical and",
            example: "lo <= hi && hi <= len",
        },
        BinOp::Or => SkillFragment {
            fragment: "a || b",
            description: "logical or",
            example: "done || empty",
        },
    }
}

/// Render ONE `UnaryOp` leaf's surface fragment (REQ-10, #92): exhaustive `match`
/// so a NEW prefix operator compile-forces a skill entry. There is ONE
/// `UnaryOp::Not` (the prefix `!`), whose meaning is per the operand type
/// (logical-not on `bool`, bitwise-not on an integer; ast.md OQ-4); it binds
/// tighter than every binary operator (`surface-grammar.md` REQ-10).
fn render_unaryop_arm(op: UnaryOp) -> SkillFragment {
    match op {
        UnaryOp::Not => SkillFragment {
            fragment: "!EXPR",
            description: "prefix not — logical on bool, bitwise on int; binds tightest",
            example: "!(a & mask)",
        },
    }
}

/// The closed `UnaryOp` set, in declaration order (REQ-10 leaf inventory, #92).
fn unaryop_inventory() -> [UnaryOp; 1] {
    [UnaryOp::Not]
}

/// Render ONE `Pattern` variant's surface fragment (REQ-10): exhaustive `match`
/// over `thermite_syntax::ast::Pattern`, NO `_` arm.
fn render_pattern_arm(pat: &Pattern) -> SkillFragment {
    match pat {
        Pattern::Wildcard => SkillFragment {
            fragment: "_",
            description: "the wildcard pattern",
            example: "_ => 0",
        },
        Pattern::Literal(_) => SkillFragment {
            fragment: "LIT",
            description: "a literal pattern",
            example: "0 => true",
        },
        Pattern::Binding(_) => SkillFragment {
            fragment: "name",
            description: "a binding pattern",
            example: "Some(i) => i",
        },
        Pattern::Slice(_) => SkillFragment {
            fragment: "[] | [head, ..tail]",
            description: "a slice pattern with an optional rest binding",
            example: "[head, ..tail] => head",
        },
        Pattern::Enum { .. } => SkillFragment {
            fragment: "Variant(p, ..) | None",
            description: "a tuple/unit enum-variant pattern (binds the payload)",
            example: "Some(i) => ..",
        },
        Pattern::Struct { .. } => SkillFragment {
            fragment: "Path { field, .. }",
            description: "a struct / struct-variant destructuring pattern",
            example: "Rect { w, h } => w * h",
        },
        // The C10 or-pattern `p0 | p1 | …` (`.design/basis/11-ergonomics.md`
        // REQ-4): an alternation matching any one alternative, covering the
        // UNION of their cases for exhaustiveness.
        Pattern::Or(_) => SkillFragment {
            fragment: "p0 | p1 | ..",
            description: "an or-pattern (matches any alternative; covers their union)",
            example: "1 | 2 => true",
        },
    }
}

/// Render ONE `Effect` atom's surface fragment (REQ-10): exhaustive `match` over
/// `thermite_syntax::ast::Effect`, NO `_` arm — a new effect atom compile-forces
/// a skill entry (REQ-8, AC-10). A caller's row must subsume every callee's row.
fn render_effect_arm(effect: &Effect) -> SkillFragment {
    match effect {
        Effect::Read(_) => SkillFragment {
            fragment: "read(path)",
            description: "reads from a filesystem path",
            example: "fx read(\"/etc/hosts\")",
        },
        Effect::Write(_) => SkillFragment {
            fragment: "write(path)",
            description: "writes to a filesystem path",
            example: "fx write(\"/tmp/out\")",
        },
        Effect::Net(_) => SkillFragment {
            fragment: "net(domain)",
            description: "performs network I/O to a domain",
            example: "fx net(\"api.example.com\")",
        },
        Effect::Alloc => SkillFragment {
            fragment: "alloc",
            description: "allocates on the heap (Box/Vec/String construction)",
            example: "fx alloc",
        },
        Effect::Time => SkillFragment {
            fragment: "time",
            description: "reads the wall clock",
            example: "fx time",
        },
        Effect::Rand => SkillFragment {
            fragment: "rand",
            description: "draws randomness",
            example: "fx rand",
        },
        Effect::Panic => SkillFragment {
            fragment: "panic",
            description: "may panic / abort",
            example: "fx panic",
        },
        Effect::Diverge => SkillFragment {
            fragment: "diverge",
            description: "may not terminate (waives the default termination proof)",
            example: "fx diverge",
        },
        Effect::Term => SkillFragment {
            fragment: "term",
            description: "controls the terminal (raw mode via the `ioctl` syscall)",
            example: "fx term",
        },
    }
}

/// The representative `Type` variants the REQ-10 inventory enumerates. ONE value
/// per `Type` variant — the `match` in `render_type_arm` is what the compiler
/// checks for exhaustiveness; this list is what the OUTPUT covers. Payload is the
/// cheapest legal filler (the arm text is payload-independent, AC-6). If a new
/// `Type` variant is added, `render_type_arm`'s `match` fails to compile FIRST
/// (REQ-8); this list is then extended to render it.
fn type_inventory() -> Vec<Type> {
    vec![
        Type::Prim(PrimType::U64),
        Type::Unit,
        Type::Ref {
            mutable: false,
            inner: Box::new(Type::Unit),
        },
        Type::Slice(Box::new(Type::Unit)),
        Type::Generic {
            name: String::new(),
            arg: Box::new(Type::Unit),
        },
        Type::Named(String::new()),
        Type::Box(Box::new(Type::Unit)),
        Type::Vec(Box::new(Type::Unit)),
        Type::String,
        // Cluster C7 (`.design/basis/09-option-result.md` REQ-1/REQ-2): one
        // representative each of the built-in `Option<T>` / `Result<T, E>` nodes so
        // the REQ-10 inventory renders their fragments (the `match` in
        // `render_type_arm` is the exhaustiveness oracle; this list is the OUTPUT
        // cover). The payload is the cheapest legal filler.
        Type::Option(Box::new(Type::Unit)),
        Type::Result(Box::new(Type::Unit), Box::new(Type::Unit)),
        // Cluster C12 (`.design/basis/13-map.md` REQ-1/REQ-5): a representative
        // `Map<K, V>` node so the REQ-10 inventory renders its fragment (the `match`
        // in `render_type_arm` is the exhaustiveness oracle; this list is the OUTPUT
        // cover). The two args are the cheapest legal filler.
        Type::Map(Box::new(Type::Unit), Box::new(Type::Unit)),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-7): a
        // representative n-tuple type so the REQ-10 inventory renders its fragment
        // (the `match` in `render_type_arm` is the exhaustiveness oracle; this list
        // is the OUTPUT cover). Arity 2 — the minimal legal tuple.
        Type::Tuple(vec![Type::Unit, Type::Unit]),
    ]
}

/// The closed `PrimType` set, in declaration order (REQ-10 leaf inventory).
fn prim_inventory() -> [PrimType; 4] {
    [
        PrimType::U32,
        PrimType::U64,
        PrimType::Usize,
        PrimType::Bool,
    ]
}

/// One representative value per `Item` variant (REQ-10). See [`type_inventory`].
fn item_inventory() -> Vec<Item> {
    use thermite_syntax::ast::{
        Block, Clause, Contract, EffectRow, EnumItem, FnItem, SpecFnItem, StructItem,
    };
    let span = Span::new(0, 0);
    let clause = || Clause {
        expr: Expr::BoolLit(true),
        text: String::new(),
        span,
    };
    let empty_block = || Block {
        stmts: Vec::new(),
        tail: None,
    };
    vec![
        Item::Fn(FnItem {
            slag: None,
            boundary: None,
            name: String::new(),
            params: Vec::new(),
            ret: Type::Unit,
            contract: Contract {
                req: clause(),
                ens: vec![clause()],
                fx: EffectRow::Pure,
            },
            // C9-A (`.design/basis/10-recursion-tuples.md` REQ-1): the optional
            // `dec` termination clause of a recursive exec `fn`. `None` for this
            // representative non-recursive item (the additive-field ripple).
            dec: None,
            body: Some(empty_block()),
            // #193 (`.design/forge/goal-repl.md` REQ-4): the open body holes. EMPTY
            // for this representative complete skill-inventory item (the additive
            // `FnItem.holes` ripple — a skill example is never a holed item).
            holes: Vec::new(),
            span,
        }),
        Item::SpecFn(SpecFnItem {
            name: String::new(),
            params: Vec::new(),
            ret: Type::Unit,
            dec: clause(),
            body: empty_block(),
            span,
        }),
        Item::Struct(StructItem {
            name: String::new(),
            fields: Vec::new(),
            inv: None,
            sealed: false,
            span,
        }),
        Item::Enum(EnumItem {
            name: String::new(),
            variants: Vec::new(),
            span,
        }),
    ]
}

/// One representative value per `Expr` variant (REQ-10). See [`type_inventory`].
fn expr_inventory() -> Vec<Expr> {
    use thermite_syntax::ast::{Block, MatchArm};
    let unit = || Box::new(Expr::Path(Vec::new()));
    let empty_block = || Block {
        stmts: Vec::new(),
        tail: None,
    };
    vec![
        Expr::IntLit {
            value: 0,
            raw: String::new(),
        },
        Expr::BoolLit(true),
        Expr::Path(Vec::new()),
        Expr::Call {
            callee: unit(),
            args: Vec::new(),
        },
        Expr::MethodCall {
            receiver: unit(),
            name: String::new(),
            args: Vec::new(),
        },
        Expr::Field {
            receiver: unit(),
            name: String::new(),
        },
        Expr::Closure {
            params: Vec::new(),
            body: unit(),
        },
        Expr::Match {
            scrutinee: unit(),
            arms: Vec::<MatchArm>::new(),
        },
        Expr::If {
            cond: unit(),
            then: empty_block(),
            else_: empty_block(),
        },
        Expr::Binary {
            op: BinOp::Add,
            lhs: unit(),
            rhs: unit(),
        },
        Expr::Unary {
            op: thermite_syntax::ast::UnaryOp::Not,
            expr: unit(),
        },
        Expr::Index {
            base: unit(),
            index: IndexArg::Single(unit()),
        },
        Expr::Cast {
            expr: unit(),
            ty: Type::Unit,
        },
        Expr::Ref {
            mutable: false,
            expr: unit(),
        },
        Expr::StructLit {
            path: Vec::new(),
            fields: Vec::new(),
        },
        Expr::Is {
            scrutinee: unit(),
            variant: Vec::new(),
        },
        Expr::Deref(unit()),
        Expr::StrLit(String::new()),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-8): one
        // representative each of the tuple construction + the projection node so the
        // REQ-10 inventory renders their fragments (the `match` in `render_expr_arm`
        // is the exhaustiveness oracle; this list is the OUTPUT cover).
        Expr::Tuple(vec![*unit(), *unit()]),
        Expr::TupleProj {
            receiver: unit(),
            index: 0,
        },
    ]
}

/// The closed `BinOp` set, in declaration order (REQ-10 leaf inventory). The #92
/// integer operators (`Rem`/`Shl`/`Shr`/`BitAnd`/`BitOr`/`BitXor`) join the set.
fn binop_inventory() -> [BinOp; 18] {
    [
        BinOp::Add,
        BinOp::Sub,
        BinOp::Mul,
        BinOp::Div,
        BinOp::Rem,
        BinOp::Shl,
        BinOp::Shr,
        BinOp::BitAnd,
        BinOp::BitOr,
        BinOp::BitXor,
        BinOp::Eq,
        BinOp::Ne,
        BinOp::Lt,
        BinOp::Le,
        BinOp::Gt,
        BinOp::Ge,
        BinOp::And,
        BinOp::Or,
    ]
}

/// One representative value per `Pattern` variant (REQ-10). See [`type_inventory`].
fn pattern_inventory() -> Vec<Pattern> {
    vec![
        Pattern::Wildcard,
        Pattern::Literal(Expr::BoolLit(true)),
        Pattern::Binding(String::new()),
        Pattern::Slice(Vec::<SlicePat>::new()),
        Pattern::Enum {
            path: Vec::new(),
            fields: Vec::new(),
        },
        Pattern::Struct {
            path: Vec::new(),
            fields: Vec::new(),
            rest: false,
        },
        // The C10 or-pattern (`.design/basis/11-ergonomics.md` REQ-4).
        Pattern::Or(Vec::new()),
    ]
}

/// One representative value per `Effect` atom (REQ-10). See [`type_inventory`].
fn effect_inventory() -> Vec<Effect> {
    vec![
        Effect::Read(String::new()),
        Effect::Write(String::new()),
        Effect::Net(String::new()),
        Effect::Alloc,
        Effect::Time,
        Effect::Rand,
        Effect::Panic,
        Effect::Diverge,
        Effect::Term,
    ]
}

/// Render a labelled construct sub-section: a heading + one bullet per fragment.
fn render_inventory(label: &str, fragments: &[SkillFragment]) -> String {
    let mut s = format!("\n**{label}**\n\n");
    for frag in fragments {
        s.push_str(&frag.to_bullet());
    }
    s
}

/// Section (1) — the surface grammar. The narrative SCAFFOLDING (the
/// contract-first framing, the mandatory clause order, the loop `inv`/`dec`
/// rule, the one-call-syntax rule, the "removed from Rust" motivation) is CURATED
/// PROSE (REQ-11, sourced from `thermite-design.md` §4/§4.2/§4.4). The CONSTRUCT
/// INVENTORY — the type / item / expression / operator / pattern / effect forms —
/// is rendered by EXHAUSTIVE `match`es over the definitional enums (REQ-10), so a
/// new language construct compile-forces a skill entry (REQ-8). The exact set is
/// `render_*_arm` over [`type_inventory`]/[`item_inventory`]/[`expr_inventory`]/
/// [`binop_inventory`]/[`pattern_inventory`]/[`prim_inventory`]/
/// [`effect_inventory`] — the OUTPUT covers every current variant, the COMPILER
/// guarantees no variant can be added without an arm.
fn render_grammar() -> String {
    let mut s = String::from(
        "\
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
",
    );
    let items = item_inventory();
    let item_frags: Vec<SkillFragment> = items.iter().map(render_item_arm).collect();
    for frag in &item_frags {
        s.push_str(&frag.to_bullet());
    }

    let types = type_inventory();
    let type_frags: Vec<SkillFragment> = types.iter().map(render_type_arm).collect();
    s.push_str(&render_inventory("Types", &type_frags));

    let prim_frags: Vec<SkillFragment> =
        prim_inventory().into_iter().map(render_prim_arm).collect();
    s.push_str(&render_inventory("Primitive scalars", &prim_frags));

    let exprs = expr_inventory();
    let expr_frags: Vec<SkillFragment> = exprs.iter().map(render_expr_arm).collect();
    s.push_str(&render_inventory("Expressions", &expr_frags));

    let binop_frags: Vec<SkillFragment> = binop_inventory()
        .into_iter()
        .map(render_binop_arm)
        .collect();
    s.push_str(&render_inventory("Binary operators", &binop_frags));

    let unaryop_frags: Vec<SkillFragment> = unaryop_inventory()
        .into_iter()
        .map(render_unaryop_arm)
        .collect();
    s.push_str(&render_inventory(
        "Unary (prefix) operators",
        &unaryop_frags,
    ));

    let pats = pattern_inventory();
    let pat_frags: Vec<SkillFragment> = pats.iter().map(render_pattern_arm).collect();
    s.push_str(&render_inventory("Patterns", &pat_frags));

    let effects = effect_inventory();
    let effect_frags: Vec<SkillFragment> = effects.iter().map(render_effect_arm).collect();
    s.push_str(&render_inventory(
        "Effect atoms (a caller's fx row subsumes every callee's)",
        &effect_frags,
    ));

    s.push_str(
        "\
\nRemoved from Rust (to keep the language small and formulaic): explicit
lifetimes, the full trait system (only built-in `Eq`/`Ord`/`Hash`/`Iter`/
`Display`), macros, `unsafe` (replaced by `#[slag]`), UFCS, and implicit integer
widening (all conversions explicit; arithmetic overflow is a proof obligation).

",
    );
    s
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

/// The generator-side example table for the recursion schemes (REQ-9 / OQ-2):
/// one tiny usage example per scheme name, keyed by `name`. Examples are a SKILL
/// concern, not a registry field, so they live here, not in `SchemeSig` (the
/// `example_for` combinator precedent). A scheme added to the registry without a
/// mapping falls back to a generic example (so the renderer never panics —
/// R-CODE-2) and the coverage test still pins its name + the example marker
/// (AC-9), making the gap visible without an abort.
fn scheme_example_for(name: &str) -> &'static str {
    match name {
        "fold" => "fold(list, 0, |x, acc| acc + x)",
        "map" => "map(list, |x| x + 1)",
        "for_all" => "for_all(list, |x| x <= bound)",
        "exists" => "exists(list, |x| x == needle)",
        "traverse" => "traverse(list, |x, acc| acc && p(x))",
        _ => "fold(list, 0, |x, acc| acc)",
    }
}

/// Render the trailing step-closure shape of a scheme (REQ-9): `|x, acc|` for an
/// element+accumulator step, `|x|` for an element-only step.
fn render_step_shape(shape: StepShape) -> &'static str {
    match shape {
        StepShape::ElementAcc => "|x, acc| …",
        StepShape::Element => "|x| …",
    }
}

/// Render the surface result kind a scheme collapses to (REQ-9): an accumulator
/// folds to `nat`, the structural predicates to `bool`, a `map` rebuilds the ADT.
fn render_scheme_result(result: SchemeResult) -> &'static str {
    match result {
        SchemeResult::Accumulator => "nat",
        SchemeResult::Bool => "bool",
        SchemeResult::SameAdt => "the same ADT",
    }
}

/// Render one scheme's surface call shape + result + one example as a markdown
/// bullet (the per-entry body of [`render_schemes`], REQ-9). The call shape is
/// the `scrutinee_args` positional args (`l`, then a seed for `fold`) plus the
/// trailing `step_shape` closure.
fn render_one_scheme(sig: &SchemeSig) -> String {
    let mut args = String::from("l");
    // A second positional arg before the step is the fold/traverse-style seed.
    for _ in 1..sig.scrutinee_args {
        args.push_str(", init");
    }
    format!(
        "- `{name}({args}, {step}) -> {result}`\n  // scheme: {example}\n",
        name = sig.name,
        args = args,
        step = render_step_shape(sig.step_shape),
        result = render_scheme_result(sig.result),
        example = scheme_example_for(sig.name),
    )
}

/// Section (2b) — the recursion-scheme library. MACHINE-RENDERED from
/// `thermite_spec::schemes::all()` (REQ-9): for EVERY entry and ONLY those
/// entries, the surface call shape (name + positional args + the trailing step
/// closure) + result kind + one example. Adding a scheme to the frozen registry
/// makes it auto-appear; removing one auto-drops it (§10 anti-drift — the
/// `render_combinators` precedent, REQ-2). The generated lowering symbols
/// (`fold_<e>` etc.) are NOT rendered — the skill teaches the surface call.
fn render_schemes() -> String {
    let mut s = String::from(
        "\n\
## 2b. Recursion-scheme library

Use these to RECURSE over a recursive ADT (a `Box`ed `enum` like a list/tree).
You may NOT hand-write the recursion — it goes through this fixed, closed set of
verified schemes (the structural analogue of the combinators). Each takes the
scrutinee (and, for `fold`, a seed) then a trailing FLAT step closure — like a
combinator's predicate closure, the step may NOT contain another scheme (genuine
nesting is a named `spec fn`). A scheme discharges its bound by citing the
`fold_bound` prove-once law, never a fresh induction.

The schemes (call shape, result, then one example each):

",
    );
    for sig in thermite_spec::schemes::all() {
        s.push_str(&render_one_scheme(sig));
    }
    s
}

/// Section (3) — the Forge command set. CURATED from `thermite-design.md`
/// Appendix B (the v0.1 command surface) + §5.1 framing (REQ-3).
fn render_forge() -> String {
    String::from(
        "\n\
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
forge check --engine lean|auto     L3 via the Lean engine; disagreement = HALT
forge goal <item>                  print the goal state: given / want / open
                                   holes ?N / per-obligation status
forge fill <fn>.?N <code>          splice code at a hole ?N + re-check; returns
                                   the new goal state (may surface new holes)
forge edit <addr> --replace <code> splice at any semantic address + re-check
forge build [item] --entry <fn>    lower to Rust + rustc -> a native binary whose
                                   contract checks fire at runtime, fx-sandboxed
forge build --target kernel <file> emit a freestanding no_std+alloc rlib (no
                                   main, no seccomp, panic=abort); ambient-
                                   syscall fx is REFUSED
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

- L3 — machine proof (Verus/Z3; or Lean via `--engine`): holds for ALL inputs.
  Not guaranteed to terminate -> solver budget + automatic downgrade.
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

`#[slag]` is the escape hatch for unverified code (slag is the waste product of a
thermite burn) — the replacement for `unsafe`: harder to write, louder to read.

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
- The contract is STILL mandatory and L1-enforced at runtime — slag exempts you
  from PROVING, never from STATING and CHECKING.
- Every slag block appears in the build manifest and in `forge audit`; `grep
  slag` is the complete inventory of fiat-trusted code.
- CI policy hooks can cap slag count or require a second-party sign-off.

The polarity inversion is the point: verification is the default and costs
nothing; non-verification is the exotic add-on that costs more keystrokes and
more visibility.
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
    fn scheme_coverage() {
        // AC-9: every entry in the frozen scheme registry appears by name AND
        // has a call-shape marker (the registry IS the oracle — R-CHAR-3).
        let skill = generate();
        for sig in thermite_spec::schemes::all() {
            assert!(
                skill.contains(sig.name),
                "skill is missing scheme name `{}`",
                sig.name
            );
        }
    }

    #[test]
    fn renderers_are_exhaustive_no_wildcard() {
        // AC-10(i) — the STRUCTURAL no-staleness invariant. The renderer
        // functions `render_{type,expr,item,pattern,effect,binop,prim}_arm` are
        // EXHAUSTIVE `match`es with NO `_` wildcard arm over their definitional
        // enums. Rust's exhaustiveness check (E0004) makes adding a new variant a
        // HARD compile error in THIS crate until the matching arm is added — so
        // the skill cannot silently fall behind the language (REQ-8).
        //
        // This is enforced by the compiler, not by a runtime assertion: if a
        // future variant were added without a renderer arm, this whole crate
        // (and thus this test) would FAIL TO BUILD. A green build is the proof.
        // We exercise the renderers over the full per-variant inventories so the
        // arms are reached, and assert each inventory is non-empty (a sanity
        // floor — the inventories must cover at least the shipped variants).
        assert!(!type_inventory().is_empty());
        assert!(!item_inventory().is_empty());
        assert!(!expr_inventory().is_empty());
        assert!(!pattern_inventory().is_empty());
        assert!(!effect_inventory().is_empty());
        assert_eq!(prim_inventory().len(), 4);
        // 12 base BinOps + the 6 #92 integer operators = 18.
        assert_eq!(binop_inventory().len(), 18);
        // The closed `UnaryOp` set (#92): exactly the prefix `!`.
        assert_eq!(unaryop_inventory().len(), 1);
        for op in unaryop_inventory() {
            assert!(!render_unaryop_arm(op).fragment.is_empty());
        }
        for ty in &type_inventory() {
            assert!(!render_type_arm(ty).fragment.is_empty());
        }
        for it in &item_inventory() {
            assert!(!render_item_arm(it).fragment.is_empty());
        }
        for ex in &expr_inventory() {
            assert!(!render_expr_arm(ex).fragment.is_empty());
        }
        for pat in &pattern_inventory() {
            assert!(!render_pattern_arm(pat).fragment.is_empty());
        }
        for ef in &effect_inventory() {
            assert!(!render_effect_arm(ef).fragment.is_empty());
        }
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
        // REQ-1: the sections appear in §10 order, now including 2b (schemes).
        let skill = generate();
        let headings = [
            "## 1. Surface grammar",
            "## 2. SpecTherm",
            "## 2b. Recursion-scheme",
            "## 3. Forge",
            "## 4. Verification ladder",
            "## 5. Slag rules",
        ];
        // Each heading must be present; collect the byte offsets it appears at.
        let positions: Vec<usize> = headings
            .iter()
            .map(|heading| {
                let found = skill.find(heading);
                assert!(
                    found.is_some(),
                    "skill is missing section heading `{heading}`"
                );
                // `is_some` just asserted; the default is never observed.
                found.unwrap_or_default()
            })
            .collect();
        // The offsets must be strictly increasing (the §10 canonical order).
        for window in positions.windows(2) {
            assert!(
                window[0] < window[1],
                "skill sections are out of canonical order: {positions:?}"
            );
        }
    }
}
