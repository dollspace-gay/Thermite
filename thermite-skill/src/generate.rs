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
//! Forge command set, (4) the ladder semantics, (5) the slag rules, (6) the
//! Stage-1 forge tier (the seven verdicts, per-clause routing, covenant authoring,
//! and the forge-tier verbs + burn receipt; `.design/stage1-forge-tier.md`) — into
//! one deterministic `String`. The surface INVENTORY is DYNAMIC by two
//! compiler-backed mechanisms (REQ-8): (i) **registry-driven** — section (2)
//! iterates `thermite_spec::all()` and (2b) iterates
//! `thermite_spec::schemes::all()`, so a new registry entry auto-appears (REQ-2,
//! REQ-9); (ii) **exhaustive-match-driven** — section (1)'s construct inventory
//! is rendered by an exhaustive `match` (no `_` wildcard) over the definitional
//! enums `thermite_syntax::{Type,Expr,Item,Pattern,Effect}` (+ `BinOp`/
//! `PrimType`), so a new variant fails TO compile until its skill arm is added
//! (REQ-10 — the compiler is the freshness enforcer). Section (3) iterates the
//! shared [`ForgeMethod`] registry. The explanatory prose (the framing, ladder,
//! and slag rules) stays curated,
//! guarded by the freshness + budget tests (REQ-11). No I/O, no env, no
//! wall-clock, no RNG — a pure function of the compiled-in text, the static
//! registries (including [`ForgeMethod::ALL`]), and the per-variant match arms
//! (R-CODE-5).
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=thermite-skill-generator-core-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SKILL-GENERATOR-BIN | shipped | `thermite-skill/src/generate.rs` | Skill generator CLI |  |
//! | REQ-SKILL-GENERATOR-BUDGET | shipped | `thermite-skill/src/generate.rs` | Deterministic skill token budget |  |
//! | REQ-SKILL-GENERATOR-CANONICAL-SECTIONS | shipped | `thermite-skill/src/generate.rs` | Skill generator canonical sections |  |
//! | REQ-SKILL-GENERATOR-CI-BUDGET | shipped | `thermite-skill/src/generate.rs` | CI skill budget gate |  |
//! | REQ-SKILL-GENERATOR-COMBINATOR-SECTION | shipped | `thermite-skill/src/generate.rs` | Combinator section from registry |  |
//! | REQ-SKILL-GENERATOR-COMMITTED-FRESH | shipped | `thermite-skill/src/generate.rs` | Committed skill freshness |  |
//! | REQ-SKILL-GENERATOR-CURATED-PROSE | shipped | `thermite-skill/src/generate.rs` | Curated prose sections |  |
//! | REQ-SKILL-GENERATOR-GRAMMAR-EXHAUSTIVE | shipped | `thermite-skill/src/generate.rs` | Exhaustive grammar inventory |  |
//! | REQ-SKILL-GENERATOR-NO-STALENESS | shipped | `thermite-skill/src/generate.rs` | Compiler-enforced skill surface freshness |  |
//! | REQ-SKILL-GENERATOR-PROSE-FRESHNESS | shipped | `thermite-skill/src/generate.rs` | Curated prose freshness |  |
//! | REQ-SKILL-GENERATOR-SCHEMES | shipped | `thermite-skill/src/generate.rs` | Scheme section from registry |  |
//! <!-- /generated:reqs -->
//!
//! ## Cluster C10 — ergonomics skill arms (`.design/basis/11-ergonomics.md`, #112)
//!
//! <!-- generated:reqs view=thermite-skill-generator-ergonomics-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SKILL-ERGONOMICS-DESUGAR | shipped | `thermite-skill/src/generate.rs` | Skill text for pure-desugar ergonomics |  |
//! | REQ-SKILL-ERGONOMICS-MATCH-GUARD | shipped | `thermite-skill/src/generate.rs` | Skill match-guard arm |  |
//! | REQ-SKILL-ERGONOMICS-OR-PATTERN | shipped | `thermite-skill/src/generate.rs` | Skill or-pattern arm |  |
//! <!-- /generated:reqs -->

use thermite_spec::schemes::{SchemeResult, SchemeSig, StepShape};
use thermite_spec::{ArgKind, CombinatorSig, ResultKind};
use thermite_syntax::ast::{
    BinOp, Effect, Expr, IndexArg, Item, Pattern, PlatformDomain, PrimType, SlicePat, Type, UnaryOp,
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

/// A public Forge method and the documentation shared by the CLI and skill.
///
/// The registry is declared once by `forge_methods!`. Forge resolves its first
/// argument through [`ForgeMethod::parse`] and exhaustively dispatches the
/// result; this generator iterates [`ForgeMethod::ALL`]. A newly registered
/// method therefore becomes visible in help and the skill in the same build.
macro_rules! forge_methods {
    (
        $(
            $variant:ident {
                name: $name:literal,
                usage: $usage:literal,
                purpose: $purpose:literal,
            }
        )+
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ForgeMethod {
            $($variant,)+
        }

        impl ForgeMethod {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }

            pub const fn usage(self) -> &'static str {
                match self {
                    $(Self::$variant => $usage,)+
                }
            }

            pub const fn purpose(self) -> &'static str {
                match self {
                    $(Self::$variant => $purpose,)+
                }
            }

            pub fn parse(name: &str) -> Option<Self> {
                Self::ALL
                    .iter()
                    .copied()
                    .find(|method| method.name() == name)
            }
        }
    };
}

forge_methods! {
    New {
        name: "new",
        usage: "forge new <name>",
        purpose: "Create a pinned project.",
    }
    Check {
        name: "check",
        usage: "forge check <file> [--json] [--level l2|l3] [--rlimit FLOAT] [--mutation-floor FLOAT] [--engine auto|verus|lean|nlsat|forge|bv]",
        purpose: "Certify; auto-routes eligible BV and EPR clauses.",
    }
    Audit {
        name: "audit",
        usage: "forge audit <file> [--json] [--meaning] [--metrics]",
        purpose: "Show assurance, boundaries, meaning, and metrics.",
    }
    Repair {
        name: "repair",
        usage: "forge repair <file> [item] [--json]",
        purpose: "Retry timeout-lowered items.",
    }
    Review {
        name: "review",
        usage: "forge review <file> [item] [--json] [--reviewer <cmd>]",
        purpose: "Emit contracts for intent review.",
    }
    Build {
        name: "build",
        usage: "forge build <file-or-package> [--level l1|l3] [--export <fn>] [--compose-export <fn> --compose-shell <file.rs> [--primitive-registry <registry.json>]] [--crate-name <name>] [--entry <fn>] [--out <path>] [--target std|kernel] [--json] [--no-sandbox] [--sandbox-self-test]",
        purpose: "Build L1 checked Rust or an exact-source L3 link/composition bundle.",
    }
    VerifyBuild {
        name: "verify-build",
        usage: "forge verify-build <bundle-dir> [--replay] [--json]",
        purpose: "Validate or replay a correspondence-backed L3 build receipt.",
    }
    Tv {
        name: "tv",
        usage: "forge tv <file> [--generated [N]] [--seed <u64>] [--json]",
        purpose: "Validate contract lowering.",
    }
    ExecTv {
        name: "exec-tv",
        usage: "forge exec-tv <file> [--generated [N]] [--no-generated] [--json]",
        purpose: "Validate expression lowering.",
    }
    StratTv {
        name: "strat-tv",
        usage: "forge strat-tv [--generated N] [--seed <u64>] [--json]",
        purpose: "Compare Rust and Lean cage classifiers.",
    }
    StratFaithfulTv {
        name: "strat-faithful-tv",
        usage: "forge strat-faithful-tv [--generated N] [--seed <u64>] [--json]",
        purpose: "Run two-phase stratified validation.",
    }
    G2Gate {
        name: "g2-gate",
        usage: "forge g2-gate --axiom-probe 0|1 --doc-drift 0|1 --differential 0|1 --two-phase 0|1 [--json]",
        purpose: "Combine the Stage 2 gate results.",
    }
    BodyTv {
        name: "body-tv",
        usage: "forge body-tv <file> [--json]",
        purpose: "Validate statement and loop lowering.",
    }
    Goal {
        name: "goal",
        usage: "forge goal <file> [item] [--proof]",
        purpose: "Show goals, witnesses, and holes.",
    }
    Battery {
        name: "battery",
        usage: "forge battery <file> [item]",
        purpose: "Show vacuity and mutation results.",
    }
    Edit {
        name: "edit",
        usage: "forge edit <file> <addr> --replace <code> | forge edit --restratify [--json]",
        purpose: "Edit by address or demonstrate restratification.",
    }
    Fill {
        name: "fill",
        usage: "forge fill <file> <hole-addr> <code>",
        purpose: "Fill a body or proof hole.",
    }
    SmtExport {
        name: "smt-export",
        usage: "forge smt-export [<file>] [--out <path>]",
        purpose: "Export Rust-to-Lean SMT obligations.",
    }
    Skill {
        name: "skill",
        usage: "forge skill [--claude] [--write <path> | --check <path>]",
        purpose: "Print, write, or check this reference.",
    }
}

/// Render the full Forge usage banner from the shared method registry.
pub fn forge_usage() -> String {
    let mut out = String::from("usage:\n");
    for method in ForgeMethod::ALL {
        out.push_str("  ");
        out.push_str(method.usage());
        out.push('\n');
    }
    out
}

/// Render a Claude Code skill by adding the required frontmatter to the
/// canonical, toolchain-matched reference.
pub fn generate_claude() -> String {
    let mut out = String::from(
        "---\n\
name: thermite\n\
description: Use when writing, checking, building, or reviewing Thermite programs with Forge.\n\
---\n\n",
    );
    out.push_str(&generate());
    out
}

/// Assemble the canonical `THERMITE.skill.md` as one deterministic `String`.
///
/// The sections appear in `thermite-design.md` §10 order: (1) surface grammar,
/// (2) the SpecTherm combinator library, (2b) the recursion-scheme library, (3)
/// the Forge command set, (4) the ladder semantics, (5) the slag rules, (6) the
/// Stage-1 forge tier. Section (1)'s construct inventory is exhaustive-MATCH-driven
/// over the `thermite_syntax` enums (REQ-10), (2) is registry-driven from
/// `thermite_spec::all()` (REQ-2), (2b) is registry-driven from
/// `thermite_spec::schemes::all()` (REQ-9); the curated prose (the framing, ladder,
/// slag, and §6 forge-tier semantics) stays templated (REQ-11).
/// Pure: no I/O, no env, no clock, no RNG (REQ-1 / R-CODE-5 / AC-6).
pub fn generate() -> String {
    let mut out = String::new();
    out.push_str(HEADER);
    out.push_str(&render_grammar());
    out.push_str(&render_combinators());
    out.push_str(&render_schemes());
    out.push_str(&render_forge());
    out.push_str(&render_ladder());
    out.push_str(&render_slag());
    out.push_str(&render_forge_tier());
    out
}

/// The short entry point for a reader who has not used Thermite before.
const HEADER: &str = "\
# Thermite language and Forge reference

This generated file matches the toolchain that produced it. Do not edit it by
hand; refresh it with `forge skill --write THERMITE.skill.md`. The canonical
file stays below the 6,000-token CI budget.

Start with a contract-first `fn`: write `req`, `ens`, and `fx`, then its body.
Run `forge check <file>`. Fix any concrete counterexample, or leave a `?0` hole
and use `forge goal` and `forge fill`. A function with a hole never certifies.

Sections: grammar (§1), bounded combinators and recursion schemes (§2/§2b),
Forge methods (§3), assurance levels (§4), `#[slag]` (§5), and the forge proof
tier (§6).

";

/// One rendered surface-construct entry (REQ-10): the per-variant fragment an
/// exhaustive-`match` renderer emits for a single language construct — a concise
/// grammar `fragment`, a one-line `description`, and a tiny `example`. The text
/// is a deterministic function of the variant (not of any payload value), so the
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
    /// Render this fragment as one markdown bullet with its example (the per-construct
    /// row of the REQ-10 inventory): the grammar fragment + description, then a tiny
    /// example. Used (via [`render_inventory_complete_examples`]) for the `Type` arms
    /// whose example is a complete, copy-pasteable item — chiefly the `fn log() -> ()
    /// req true ens true fx pure { }` that the §10 parse-clean pin guards. Every other
    /// inventory (items, expressions, primitive scalars, operators, patterns, effects)
    /// renders via [`to_bullet_terse`](SkillFragment::to_bullet_terse) to stay under
    /// the §2.2 token budget.
    fn to_bullet(&self) -> String {
        format!(
            "- `{fragment}` — {description}\n  // e.g. {example}\n",
            fragment = self.fragment,
            description = self.description,
            example = self.example,
        )
    }

    /// Render this fragment as one markdown bullet without its example — the
    /// fragment + description only. The budget-tightening form (`thermite-design.md`
    /// §2.2: the ≤ 6,000-token hard gate) for the leaf inventories whose `fragment`
    /// already shows the surface syntax (`a + b`, `read(path)`, `[head, ..tail]`), so
    /// a worked example adds little. The `example` field is still authored on every
    /// arm and rendered by [`to_bullet`](SkillFragment::to_bullet) for the complete
    /// `Type` examples, so it is not dead.
    fn to_bullet_terse(&self) -> String {
        format!(
            "- `{fragment}` — {description}\n",
            fragment = self.fragment,
            description = self.description,
        )
    }
}

/// Render one `Type` variant's surface fragment (REQ-10).
///
/// exhaustive `match` over `thermite_syntax::ast::Type` with no `_` wildcard arm:
/// adding a new `Type` variant (e.g. the deferred `Type::Map`, ast.rs REQ-2)
/// makes this `match` non-exhaustive, a hard `rustc` `E0004` compile error in
/// `thermite-skill`, until its arm is added — the compiler is the freshness
/// enforcer (REQ-8, AC-10(i)). Payload is field-elided (`{ .. }` / `(_)`); the
/// elision does not weaken exhaustiveness (the compiler checks the variant set).
fn render_type_arm(ty: &Type) -> SkillFragment {
    match ty {
        Type::Prim(_) => SkillFragment {
            fragment: "u8 | u16 | u32 | u64 | usize | bool",
            description: "the closed primitive scalar set (no implicit widening)",
            example: "let n: u64 = 0;",
        },
        Type::Unit => SkillFragment {
            fragment: "()",
            description: "the unit type, written explicitly in a return position",
            example: "fn log() -> () req true ens true fx pure { }",
        },
        Type::Array { .. } => SkillFragment {
            fragment: "[T; N]",
            description: "an owned allocation-free fixed array with a literal or declared usize capacity",
            example: "let mut slots: [u64; CAP] = [0; CAP];",
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
        // optional / fallible primitives are dedicated `Type` nodes (not a
        // string-named `Generic`), so each renders ITS own surface fragment — the
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
        // key-value primitive `Map<K, V>` — the second two-type-arg node. insert/get/
        // contains_key/len; get returns Option<V> (absent key -> None, not a wrong
        // value); insert carries fx alloc. Renders its own surface fragment.
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

/// Render one `PrimType` leaf's surface fragment (REQ-10): the exhaustive `match`
/// over the closed primitive set so a new primitive also compile-forces an entry.
fn render_prim_arm(prim: PrimType) -> SkillFragment {
    match prim {
        PrimType::U8 => SkillFragment {
            fragment: "u8",
            description: "an 8-bit unsigned integer",
            example: "byte: u8",
        },
        PrimType::U16 => SkillFragment {
            fragment: "u16",
            description: "a 16-bit unsigned integer",
            example: "port: u16",
        },
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

/// Render one `Item` variant's surface fragment (REQ-10): exhaustive `match` over
/// `thermite_syntax::ast::Item`, no `_` arm — a new top-level item kind
/// compile-forces a skill entry (REQ-8, AC-10).
fn render_item_arm(item: &Item) -> SkillFragment {
    match item {
        Item::Const(_) => SkillFragment {
            fragment: "const NAME: usize = INTEGER;",
            description: "a package-visible compile-time fixed-array capacity",
            example: "const CAP: usize = 64;",
        },
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
        // Forge-tier item (stage1-forge-tier.md REQ-3): parse-only surface in this
        // increment; emit a descriptive fragment mirroring the ADT-decl arms (no
        // inert/None option exists in this render match).
        Item::Forge(_) => SkillFragment {
            fragment: "prop fn NAME(..) -> bool { .. } | lemma NAME(..) req .. ens .. { .. } | proof for NAME { .. } | witness NAME { .. }",
            description: "a forge-tier surface item (prop fn / lemma / proof for / witness)",
            example: "prop fn nonneg(x: i64) -> bool { x >= 0 }",
        },
    }
}

/// Render one `Expr` variant's surface fragment (REQ-10): exhaustive `match` over
/// `thermite_syntax::ast::Expr`, no `_` arm — a new expression form compile-forces
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
        Expr::Array(_) => SkillFragment {
            fragment: "[a, b, ..] | []",
            description: "an exact allocation-free fixed-array initializer",
            example: "let bytes: [u8; 3] = [1, 2, 3];",
        },
        Expr::ArrayRepeat { .. } => SkillFragment {
            fragment: "[value; N]",
            description: "a repeated allocation-free fixed-array initializer",
            example: "let slots: [u64; CAP] = [0; CAP];",
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
        // tuple construction + the projection access form. Projection (not
        // destructuring) is the v1 tuple access; it reads in both exec and contract
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
        // Stage-2 (`.design/stage2-stratified-cage.md` REQ-0): the raw quantifier
        // binder over a named sorted carrier. Distinct from the `forall_in`/`sorted`
        // COMBINATOR free calls (those are `Expr::Call`); the body extends greedily
        // (parenthesize to bound it). `in` is contextual (not a reserved word).
        Expr::Quantifier { .. } => SkillFragment {
            fragment: "forall (x : S) in DOM. BODY | exists (x : S) in DOM. BODY",
            description: "a raw quantifier over a named sorted carrier (the body is greedy)",
            example: "forall (i : Idx) in haystack. haystack[i] != needle",
        },
    }
}

/// Render one `BinOp` leaf's surface fragment (REQ-10): exhaustive `match` so a
/// new operator compile-forces a skill entry. Comparisons are non-associative
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

/// Render one `UnaryOp` leaf's surface fragment (REQ-10, #92): exhaustive `match`
/// so a new prefix operator compile-forces a skill entry. There is one
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

/// Render one `Pattern` variant's surface fragment (REQ-10): exhaustive `match`
/// over `thermite_syntax::ast::Pattern`, no `_` arm.
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

/// Render one `Effect` atom's surface fragment (REQ-10): exhaustive `match` over
/// `thermite_syntax::ast::Effect`, no `_` arm — a new effect atom compile-forces
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
        Effect::Platform(domain) => match domain {
            PlatformDomain::Boot => platform_fragment("platform(boot)"),
            PlatformDomain::Memory => platform_fragment("platform(memory)"),
            PlatformDomain::Mmio => platform_fragment("platform(mmio)"),
            PlatformDomain::Pio => platform_fragment("platform(pio)"),
            PlatformDomain::Irq => platform_fragment("platform(irq)"),
            PlatformDomain::Cpu => platform_fragment("platform(cpu)"),
            PlatformDomain::Atomic => platform_fragment("platform(atomic)"),
            PlatformDomain::Smp => platform_fragment("platform(smp)"),
            PlatformDomain::Dma => platform_fragment("platform(dma)"),
            PlatformDomain::Clock => platform_fragment("platform(clock)"),
            PlatformDomain::Entropy => platform_fragment("platform(entropy)"),
            PlatformDomain::Power => platform_fragment("platform(power)"),
        },
    }
}

fn platform_fragment(fragment: &'static str) -> SkillFragment {
    SkillFragment {
        fragment,
        description: "uses one frozen kernel platform authority domain",
        example: "fx platform(memory)",
    }
}

/// The representative `Type` variants the REQ-10 inventory enumerates. one value
/// per `Type` variant — the `match` in `render_type_arm` is what the compiler
/// checks for exhaustiveness; this list is what the output covers. Payload is the
/// cheapest legal filler (the arm text is payload-independent, AC-6). If a new
/// `Type` variant is added, `render_type_arm`'s `match` fails to compile first
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
        // `render_type_arm` is the exhaustiveness oracle; this list is the output
        // cover). The payload is the cheapest legal filler.
        Type::Option(Box::new(Type::Unit)),
        Type::Result(Box::new(Type::Unit), Box::new(Type::Unit)),
        // Cluster C12 (`.design/basis/13-map.md` REQ-1/REQ-5): a representative
        // `Map<K, V>` node so the REQ-10 inventory renders its fragment (the `match`
        // in `render_type_arm` is the exhaustiveness oracle; this list is the output
        // cover). The two args are the cheapest legal filler.
        Type::Map(Box::new(Type::Unit), Box::new(Type::Unit)),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-7): a
        // representative n-tuple type so the REQ-10 inventory renders its fragment
        // (the `match` in `render_type_arm` is the exhaustiveness oracle; this list
        // is the output cover). Arity 2 — the minimal legal tuple.
        Type::Tuple(vec![Type::Unit, Type::Unit]),
    ]
}

/// The closed `PrimType` set, in declaration order (REQ-10 leaf inventory).
fn prim_inventory() -> [PrimType; 6] {
    [
        PrimType::U8,
        PrimType::U16,
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
        bv: None,
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
            // #193 (`.design/forge/goal-repl.md` REQ-4): the open body holes. empty
            // for this representative complete skill-inventory item (the additive
            // `FnItem.holes` ripple — a skill example is never a holed item).
            holes: Vec::new(),
            refinements: Vec::new(),
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
        // is the exhaustiveness oracle; this list is the output cover).
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
        Effect::Platform(PlatformDomain::Boot),
        Effect::Platform(PlatformDomain::Memory),
        Effect::Platform(PlatformDomain::Mmio),
        Effect::Platform(PlatformDomain::Pio),
        Effect::Platform(PlatformDomain::Irq),
        Effect::Platform(PlatformDomain::Cpu),
        Effect::Platform(PlatformDomain::Atomic),
        Effect::Platform(PlatformDomain::Smp),
        Effect::Platform(PlatformDomain::Dma),
        Effect::Platform(PlatformDomain::Clock),
        Effect::Platform(PlatformDomain::Entropy),
        Effect::Platform(PlatformDomain::Power),
    ]
}

/// Render a labelled construct sub-section in the TERSE form — a heading + one
/// fragment+description bullet per construct, no worked example
/// ([`SkillFragment::to_bullet_terse`]). The budget-tightening renderer
/// (`thermite-design.md` §2.2) for the leaf inventories (primitive scalars,
/// expressions, operators, patterns, effect atoms) whose `fragment` already shows
/// the syntax.
fn render_inventory_terse(label: &str, fragments: &[SkillFragment]) -> String {
    let mut s = format!("\n**{label}**\n\n");
    for frag in fragments {
        s.push_str(&frag.to_bullet_terse());
    }
    s
}

/// Render a labelled inventory that keeps a fragment's worked example only when the
/// example is a complete, standalone item — a `fn`/`spec fn` with a body and no `..`
/// placeholder ([`is_complete_example`]) — and renders every other (snippet) example
/// terse. This is the budget-aware middle ground for the `Type` inventory: it keeps
/// the copy-pasteable, parse-clean `fn log() -> () req true ens true fx pure { }`
/// (the Type::Unit example the §10 parse-clean pin guards) while dropping the
/// low-value type-snippet examples (`let n: u64 = 0;`, `-> Wrapper<usize>`, …). The
/// `example` field is therefore still rendered for the complete arms, so it is not
/// dead.
fn render_inventory_complete_examples(label: &str, fragments: &[SkillFragment]) -> String {
    let mut s = format!("\n**{label}**\n\n");
    for frag in fragments {
        if is_complete_example(frag.example) {
            s.push_str(&frag.to_bullet());
        } else {
            s.push_str(&frag.to_bullet_terse());
        }
    }
    s
}

/// Is `example` a complete, standalone item — a `fn`/`spec fn` with a body and no
/// `..` placeholder? Mirrors the §10 parse-clean pin's `is_complete_item`
/// (`thermite-skill/tests/divergence_unit_arm_example.rs`): only such examples are
/// standalone-parseable programs worth a full worked bullet; signature snippets and
/// `..`-placeholder fragments are not. Deterministic, payload-free (R-CODE-5).
fn is_complete_example(example: &str) -> bool {
    let e = example.trim();
    (e.starts_with("fn ") || e.starts_with("spec fn ")) && !e.contains("..") && e.ends_with('}')
}

/// Section (1) — the surface grammar. The narrative SCAFFOLDING (the
/// contract-first framing, the mandatory clause order, the loop `inv`/`dec`
/// rule, the one-call-syntax rule, the "removed from Rust" motivation) is curated
/// prose (REQ-11, sourced from `thermite-design.md` §4/§4.2/§4.4). The CONSTRUCT
/// INVENTORY — the type / item / expression / operator / pattern / effect forms —
/// is rendered by exhaustive `match`es over the definitional enums (REQ-10), so a
/// new language construct compile-forces a skill entry (REQ-8). The exact set is
/// `render_*_arm` over [`type_inventory`]/[`item_inventory`]/[`expr_inventory`]/
/// [`binop_inventory`]/[`pattern_inventory`]/[`prim_inventory`]/
/// [`effect_inventory`] — the output covers every current variant, the COMPILER
/// guarantees no variant can be added without an arm.
fn render_grammar() -> String {
    let mut s = String::from(
        "\
## 1. Surface grammar

Every `fn` is contract-first, body-second. v0.1 has four top-level item forms —
`fn`, `spec fn`, `struct`, `enum` (plus the `#[slag(...)]` / `#[boundary]`
attributes) — and no others (no `impl`/`trait`/`use`/`mod`/macros).

A `fn` signature is followed by mandatory clauses in this exact order (absence of any
is a parse error, never an implicit default):

- `req EXPR` — precondition (write `req true` if none).
- `ens EXPR` — postcondition, one-or-more; must mention `result` unless the return
  type is `()`.
- `fx EFFECTROW` — effect row, exactly one.

A `spec fn` carries exactly one `dec EXPR` (a decreases-measure), not `req`/`ens`/`fx`;
spec functions are total, terminating, executable.

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
Termination is proved by default; `fx diverge` waives it. `break ;` exits,
`continue ;` restarts; each `inv` must hold at both, and in a terminating loop a
`continue` must decrease `dec` (an `fx diverge` loop makes no termination claim, so
neither is `dec`-bound).

Statements: `let mut? NAME : TYPE = EXPR ;`, assignment `LVALUE = EXPR ;`, `return
EXPR? ;`, the `if`/`else` statement, the loop-control `break ;` / `continue ;`
(inside a `loop`/`while` body only, labelless + value-less — no `break EXPR`), and
expression-statements. A block `{ }` is statements plus an optional tail expression
(no `;`), its value. ONE member-access call syntax (postfix `.`, no UFCS).
Comparisons are non-associative (`a < b < c` is an error).

Holes: `?0` (a `?` + a digit run) is a body HOLE — an open-goal placeholder valid
ONLY in exec-`fn`-body statement position (not a spec clause / `spec fn` /
expression). A `fn` with any open hole is well-formed but NEVER certifies (L0 until
every hole is filled). Work holes with `forge goal <fn>` + `forge fill <fn>.?N
<code>` (§3); the proof-hole `?pN` analogue is §6.

Binding / control-flow ergonomics (sugar over the proven core — one explicit
desugaring each):

- `let (x, y) = e;` — tuple destructuring by projection (`let x = e.0; …`); `_`
  drops an element; sub-patterns are flat names only.
- `for i in lo..hi inv EXPR { B }` — a bounded exclusive-range loop (step +1); you
  write the `inv` (mandatory, like `while`), the `dec` is AUTOMATIC (`hi - i`).
- Match guards `Pat if COND => EXPR` do NOT complete a match — a guarded-only arm
  leaves its variant uncovered, so a `_`/full-variant arm is still required.
- Or-patterns `p0 | p1 => EXPR` match any alternative and cover their UNION
  (`Some(_) | None` is exhaustive over `Option`); v0.1 alternatives are payload-free.
- `if let Pat = e { T } else { E }` desugars to `match e { Pat => T, _ => E }` (the
  `else` is required). `while let V(_) = e inv .. dec .. { B }` desugars to
  `while (e is V) inv .. dec .. { B }`.

The CONSTRUCT INVENTORY below is GENERATED by an exhaustive match over the
toolchain's `Item`/`Type`/`Expr`/`BinOp`/`Pattern`/`Effect` enums, so it never falls
behind the language.

### Item forms
",
    );
    let items = item_inventory();
    let item_frags: Vec<SkillFragment> = items.iter().map(render_item_arm).collect();
    for frag in &item_frags {
        s.push_str(&frag.to_bullet_terse());
    }

    let types = type_inventory();
    let type_frags: Vec<SkillFragment> = types.iter().map(render_type_arm).collect();
    s.push_str(&render_inventory_complete_examples("Types", &type_frags));

    let prim_frags: Vec<SkillFragment> =
        prim_inventory().into_iter().map(render_prim_arm).collect();
    s.push_str(&render_inventory_terse("Primitive scalars", &prim_frags));

    let exprs = expr_inventory();
    let expr_frags: Vec<SkillFragment> = exprs.iter().map(render_expr_arm).collect();
    s.push_str(&render_inventory_terse("Expressions", &expr_frags));

    let binop_frags: Vec<SkillFragment> = binop_inventory()
        .into_iter()
        .map(render_binop_arm)
        .collect();
    s.push_str(&render_inventory_terse("Binary operators", &binop_frags));

    let unaryop_frags: Vec<SkillFragment> = unaryop_inventory()
        .into_iter()
        .map(render_unaryop_arm)
        .collect();
    s.push_str(&render_inventory_terse(
        "Unary (prefix) operators",
        &unaryop_frags,
    ));

    let pats = pattern_inventory();
    let pat_frags: Vec<SkillFragment> = pats.iter().map(render_pattern_arm).collect();
    s.push_str(&render_inventory_terse("Patterns", &pat_frags));

    let effects = effect_inventory();
    let effect_frags: Vec<SkillFragment> = effects.iter().map(render_effect_arm).collect();
    s.push_str(&render_inventory_terse(
        "Effect atoms (a caller's fx row subsumes every callee's)",
        &effect_frags,
    ));

    s.push_str(
        "\
\nRemoved from Rust: explicit lifetimes, the trait system (only built-in
`Eq`/`Ord`/`Hash`/`Iter`/`Display`), macros, `unsafe` (→ `#[slag]`), UFCS, implicit
widening (casts explicit; overflow is a proof obligation).

Kernel-authoring library: `forge build` also accepts a receipt-bound
`*.thpkg.json` manifest. The vendorable
`stdlib/kernel-primitives/atomics.thpkg.json` package provides sealed
bool/u32/u64/usize atomic declarations, ordering/history specifications, and
pure proof probes—no kernel or machine implementation. Atomic calls require an
exact `AtomicOrdering::Relaxed|Acquire|Release|AcqRel|SeqCst` literal; illegal
load/store/fence/CAS pairs reject before lowering. A consumer must supply and
directly refine the exact machine boundary; registry v1 intentionally certifies
only sequential safe-Rust wrappers.

`ownership.thpkg.json` provides a generation ledger; `collections.thpkg.json`
provides fixed bitset, vector, FIFO-ring, and collision-explicit direct-map
mechanics. Opacity, packing, aggregate TV, and mint refinement remain.

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
/// illustrative example. Examples are a skill concern, not a registry field, so
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
/// `thermite_spec::all()` (REQ-2): for every entry and only those entries, the
/// surface signature (name + arg-kinds + result) + one usage example. Adding a
/// combinator to the frozen registry makes it auto-appear here; removing one
/// auto-drops it (§10 anti-drift). The verbose Verus(L3)/L1 bodies the registry
/// also carries are not rendered — the skill teaches the surface signature, not
/// the lowering bodies.
fn render_combinators() -> String {
    let mut s = String::from(
        "\
## 2. SpecTherm combinator library

Use these to QUANTIFY in a contract. You may NOT write a raw `forall`/`exists` in a
`req`/`ens`/`inv` — quantification is ONLY through this fixed, closed library of
bounded combinators (SpecTherm, a deliberately weak total language), each with a
frozen SMT trigger. A combinator joins only via a slow budget-gated RFC.

Flat-closure rule: a combinator's predicate closure (`|x| ...`) is FLAT —
comparisons, arithmetic, boolean ops, field/index access, calls to NAMED `spec fn`s
— but may NOT contain another combinator (genuine nesting is a named `spec fn` with
its own `dec`).

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
/// one tiny usage example per scheme name, keyed by `name`. Examples are a skill
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
/// `thermite_spec::schemes::all()` (REQ-9): for every entry and only those
/// entries, the surface call shape (name + positional args + the trailing step
/// closure) + result kind + one example. Adding a scheme to the frozen registry
/// makes it auto-appear; removing one auto-drops it (§10 anti-drift — the
/// `render_combinators` precedent, REQ-2). The generated lowering symbols
/// (`fold_<e>` etc.) are not rendered — the skill teaches the surface call.
fn render_schemes() -> String {
    let mut s = String::from(
        "\n\
## 2b. Recursion-scheme library

Use these to RECURSE over a recursive ADT (a `Box`ed `enum`). You may NOT hand-write
the recursion — it goes through this fixed, closed set of verified schemes. Each
takes the scrutinee (and, for `fold`, a seed) then a trailing FLAT step closure that
may NOT contain another scheme (genuine nesting is a named `spec fn`). A scheme
discharges its bound by citing the `fold_bound` prove-once law, never fresh induction.

The schemes (call shape, result, then one example each):

",
    );
    for sig in thermite_spec::schemes::all() {
        s.push_str(&render_one_scheme(sig));
    }
    s
}

/// Section (3) — the Forge method set from the shared registry.
fn render_forge() -> String {
    let mut out = String::from(
        "\n\
## 3. Forge methods

Use `check` for normal certification, `goal`/`fill` for open holes, and `build`
for artifacts. Failures carry witnesses; timeouts remain named resource events.
Plain `check` automatically routes eligible fixed-width and finite EPR clauses
through checked reconstruction.

",
    );
    for method in ForgeMethod::ALL {
        out.push_str("- `");
        out.push_str(method.usage());
        out.push_str("` — ");
        out.push_str(method.purpose());
        out.push('\n');
    }
    out.push_str(
        "\nItems and blocks have stable semantic addresses such as \
`binary_search.loop#1.inv#2`; holes use `<fn>.?N` or `<fn>.?pN`.\n\n",
    );
    out
}

/// Section (4) — the ladder semantics. curated from `thermite-design.md` §6
/// (REQ-3), INCLUDING the L0/slag clarification (slag → L1 with `slag: true`;
/// L0 is the body-proof aspect).
fn render_ladder() -> String {
    String::from(
        "\
## 4. Verification ladder

Every function targets L3; downgrades are automatic, logged, and surfaced in the
build manifest; upgrades are a standing background task. The certificate lists every
function's level — this manifest IS the deliverable's trust statement.

- L4 — admitted, decidable clauses with checked reconstruction: nonlinear
  relaxation, fixed-width BV, and finite EPR relation/array clauses. Failures
  carry a real, bit-pattern, or finite-structure witness.
- L3 — machine proof (Verus/Z3, or Lean via `--engine`): holds for ALL inputs. Not
  guaranteed to terminate -> solver budget + automatic downgrade.
- L2 — bounded model check (Kani/CBMC): holds for all inputs UP TO a bound (stated
  explicitly in the manifest; L2 and L3 are always distinct).
- L1 — runtime contract checks: violations detected at the call site, in every build
  profile (not just debug).
- L0 — `#[slag]`: nothing is proved about the body. Trusted by fiat.

The Thermite -> Verus lowering behind L3 is not a trusted black box: each checked
item is translation-validated per run (Z3 proves the lowered contract equivalent to
an independent reference encoding, itself proven denotation-faithful by a
kernel-checked Lean spine). `make audit` re-derives the L3 claim on a skeptic's machine.

L0 / slag: the level rates the BODY only. A `#[slag]` fn's CONTRACT is still
mandatory and L1-checked at the call site, so its cert is L1 with `slag: true`. Slag
exempts PROVING, never STATING and CHECKING. The `fx` row is enforced independent of
level: caller/callee subsumption at compile time, plus — in a `forge build` binary —
a seccomp sandbox that kills code exceeding its declared effects at the syscall
boundary.

",
    )
}

/// Section (5) — the slag rules. curated from `thermite-design.md` §8 (REQ-3):
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
  ens result == spec_sum(xs)          // contract still mandatory — L1-enforced
  fx  pure
{ ... }
```

Rules:

- `reason`, `owner`, `review` are mandatory and non-empty (checked).
- The contract is STILL mandatory and L1-enforced at runtime (slag exempts PROVING,
  not STATING/CHECKING).
- Every slag block appears in the build manifest and `forge audit`; `grep slag` is
  the complete inventory of fiat-trusted code.
- CI policy hooks can cap slag count or require second-party sign-off.

The polarity inversion is the point: verification is the default and free;
non-verification is the exotic add-on that costs more keystrokes and visibility.
",
    )
}

/// Section (6) — the Stage-1 forge tier. curated from the SHIPPED forge code
/// (`.design/stage1-forge-tier.md`): the seven cert-level verdicts
/// ([`forge::verdict::CertVerdict`]) + the agent action per verdict; the per-clause
/// relax/in-cage/lemma routing ([`forge::relax::classify_fn`], the `nlsat`/`verus`/
/// `lean` engines, the L4/L3 attribution); covenant authoring (the
/// `witness { inhabit; falsify N }` covenant-before-burn gate,
/// [`forge::covenant_engine`]); and the forge-tier verbs + the L3/L4 burn receipt
/// ([`forge::burn::BurnReceipt`]). Curated prose (REQ-11), guarded by the budget +
/// the v2 coverage test (`forge_tier_markers_present`). The seven verdict names are
/// the closed `CertVerdict` set — a new verdict there is caught by that test.
fn render_forge_tier() -> String {
    String::from(
        "\
## 6. Forge tier (Stage-1)

The forge tier proves propositions as `prop fn`, `lemma`, or `proof for f`
items. A `witness` block gives its covenant.

### 6.1 The seven verdicts

Every clause receives one final verdict:

- **Proved** — holds at the stated level.
- **Counterexample** — concrete failing inputs; fix code or contract.
- **RealWitness** — false over the reals but possibly true over integers; add the
  missing integrality `req`.
- **CovenantRefuted** — `falsify` found a contract violation.
- **Stuck** — Lean left a residual `⊢ goal`; add the named bridge.
- **KernelBudget** — Lean exhausted its budget; split or shrink the proof.
- **Timeout** — SMT exhausted its rlimit; simplify or raise `--rlimit`.

### 6.2 Routing + per-clause attribution

Certificates record `engine`, `trust`, and `verdict` per clause:

- relaxable integer polynomials → **nlsat** plus the kernel-checked real-to-integer
  bridge, at **L4**;
- an `@bvN` fixed-width clause → QF_BV solving plus Lean replay of the actual
  theorem, at **L4**; false returns a bit pattern;
- an admitted finite S₂.0 relation/sequence clause → grounded SAT plus LRAT and
  Lean replay, at **L4**; false returns a finite model;
- an ordinary in-cage contract → **verus**, at **L3**;
- a `lemma` or `proof for` item → **lean**, at **L3**.

Plain `forge check` uses automatic routing. `--engine
auto|nlsat|verus|lean|forge|bv` selects a diagnostic override.

### 6.3 Covenant authoring

`witness { inhabit (args); falsify N; }` follows the function it covenants.
At least one well-typed `inhabit` tuple must satisfy `req`. `falsify N` checks a
deterministic sample; any violation yields **CovenantRefuted** and blocks proof.

### 6.4 Forge-tier verbs + the burn receipt

- `forge goal <f> --proof` shows hypotheses, the goal, and open `?pN` holes.
- `forge fill <f>.?pN <proof>` fills a proof hole and re-checks.
- A closed L3/L4 goal gets a **burn receipt** recording proof size and cited
  lemmas. This metadata never changes the verdict.
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
        // AC-2: every entry in the frozen registry appears by name and has an
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
        // AC-9: every entry in the frozen scheme registry appears by name and
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
        // AC-10(i) — the structural no-staleness invariant. The renderer
        // functions `render_{type,expr,item,pattern,effect,binop,prim}_arm` are
        // exhaustive `match`es with no `_` wildcard arm over their definitional
        // enums. Rust's exhaustiveness check (E0004) makes adding a new variant a
        // hard compile error in this crate until the matching arm is added — so
        // the skill cannot silently fall behind the language (REQ-8).
        //
        // This is enforced by the compiler, not by a runtime assertion: if a
        // future variant were added without a renderer arm, this whole crate
        // (and thus this test) would fail TO build. A green build is the proof.
        // We exercise the renderers over the full per-variant inventories so the
        // arms are reached, and assert each inventory is non-empty (a sanity
        // floor — the inventories must cover at least the shipped variants).
        assert!(!type_inventory().is_empty());
        assert!(!item_inventory().is_empty());
        assert!(!expr_inventory().is_empty());
        assert!(!pattern_inventory().is_empty());
        assert!(!effect_inventory().is_empty());
        assert_eq!(prim_inventory().len(), 6);
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
        // AC-4: every shared Forge method, slag fields, and grammar keywords.
        let skill = generate();
        for method in ForgeMethod::ALL {
            assert!(
                skill.contains(method.usage()),
                "skill is missing `{}`",
                method.usage()
            );
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
