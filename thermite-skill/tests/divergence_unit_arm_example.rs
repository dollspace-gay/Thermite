//! Divergence pin (critic, crosslink #84 audit of commit `e0ee523`).
//!
//! REQ-10 / REQ-8 (`.design/skill/skill-generator.md`) make the surface
//! inventory's per-construct text — each `render_*_arm`'s `{ fragment,
//! description, example }` — the agent-facing description of the REAL language
//! surface. `thermite-design.md` §10 ("the skill IS the spec, no version skew")
//! requires that text to be ACCURATE: an example the skill teaches must be a
//! program the toolchain accepts. The compile-force mechanism (REQ-8) guarantees
//! every variant HAS an arm, but it does NOT guarantee the arm's hand-written
//! example is valid surface — and one is not.
//!
//! DIVERGENCE: `render_type_arm`'s `Type::Unit` arm in
//! `thermite-skill/src/generate.rs` emits the example
//!   `fn log() -> () ens true fx pure { }`
//! which OMITS the mandatory `req` clause. The skill's OWN curated grammar prose
//! (`render_grammar`) states: "mandatory clauses in this exact order … absence of
//! any is a parse error" and lists `req`/`ens`/`fx`. The parser
//! (`thermite_syntax::parser::parse`) rejects this example with
//!   `clause `req` is out of order in `log``
//! (verified: the same fn WITH `req true` parses clean). So the skill teaches an
//! agent an example program that the toolchain itself refuses to parse — a
//! §10 version-skew lie of exactly the kind REQ-8/REQ-10 exist to eliminate, just
//! relocated from a curated grammar string into a per-variant arm example.
//!
//! Authority: `thermite-design.md` §10 (skill == spec); `.design/skill/
//! skill-generator.md` REQ-10 (per-construct fragment+example) / REQ-8 (no
//! version skew); the corpus shape (`conformance/string_demo.th` carries
//! `req true` even for a trivial precondition). The expected value is "the
//! skill's examples parse clean" — derived from the parser + the skill's own
//! mandatory-clause prose, NOT copied from generate.rs (R-CHAR-3).
//!
//! Tracking: crosslink #85.
//!
//! FIXED (crosslink #85): the `Type::Unit` arm now renders
//! `fn log() -> () req true ens true fx pure { }` (the mandatory `req` is
//! present, clauses in `req`->`ens`->`fx` order) and `THERMITE.skill.md` is
//! regenerated. This test is now the PERMANENT regression guard: EVERY complete
//! `fn`/`spec fn` example the skill renders must parse clean, so a rendered
//! un-parseable example can never ship again (§10: the skill IS the spec).

use thermite_skill::generate;
use thermite_syntax::parser::parse;

/// Extract the example text from each skill bullet's `// e.g. <example>` line —
/// the per-construct example a `SkillFragment` renders (`render_*_arm`). Returns
/// every example string verbatim.
fn rendered_examples(skill: &str) -> Vec<&str> {
    skill
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("// e.g. "))
        .collect()
}

/// Is `example` a COMPLETE top-level item (a `fn`/`spec fn` with a body), as
/// opposed to a signature snippet or a `..`-placeholder grammar fragment? Only
/// complete items are meant to be standalone-parseable programs; fragments
/// (`fn sum(..) -> u64 req .. ens .. fx pure { .. }`, bare signatures like
/// `fn f(x: &mut u64)`) deliberately are not.
fn is_complete_item(example: &str) -> bool {
    let e = example.trim();
    (e.starts_with("fn ") || e.starts_with("spec fn ")) && !e.contains("..") && e.ends_with('}')
}

/// Every COMPLETE `fn`/`spec fn` example the skill renders must be a program the
/// parser accepts. This is the regression guard for crosslink #85: the
/// `Type::Unit` arm previously rendered `fn log() -> () ens true fx pure { }`,
/// which OMITS the mandatory `req` and so the parser rejected with
/// `clause `req` is out of order`. The corrected arm renders the example WITH
/// `req true`. Authority: the skill's own mandatory-clause prose
/// (`req`->`ens`->`fx`, "absence of any is a parse error") + `thermite_syntax::
/// parser`; the expected value ("the skill's examples parse clean") is derived
/// from the parser, never copied from `generate.rs` (R-CHAR-3).
#[test]
fn rendered_fn_examples_parse_clean() {
    let skill = generate();

    // The corrected `Type::Unit` arm example must be present (so this test still
    // tracks that arm specifically) AND must be a complete item we then parse.
    let unit_example = "fn log() -> () req true ens true fx pure { }";
    assert!(
        skill.contains(unit_example),
        "the Type::Unit arm should render the corrected `{unit_example}` \
         (mandatory `req`, clauses in order); if the arm text changed, re-derive \
         this pin"
    );

    let complete: Vec<&str> = rendered_examples(&skill)
        .into_iter()
        .filter(|e| is_complete_item(e))
        .collect();

    // The sweep must actually find complete examples (the Type::Unit one at
    // minimum) — a guard against the extractor silently matching nothing.
    assert!(
        complete.iter().any(|e| e.trim() == unit_example),
        "the rendered-example sweep did not pick up the Type::Unit example"
    );

    for example in complete {
        let result = parse(example);
        assert!(
            result.is_clean(),
            "the skill teaches `{example}` but the parser rejects it \
             ({} error(s): {}). The skill's own prose says the mandatory \
             clauses (`req`->`ens`->`fx`) must all be present and in order — \
             `absence of any is a parse error`. A taught example MUST parse \
             clean (design §10: the skill IS the spec).",
            result.errors.len(),
            result
                .errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; "),
        );
    }
}
