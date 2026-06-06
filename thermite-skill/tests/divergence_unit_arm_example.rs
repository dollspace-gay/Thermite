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
//! Un-ignore when fixed: the fixer corrects the `Type::Unit` arm example (e.g.
//! `fn log() -> () req true ens true fx pure { }`) AND regenerates
//! `THERMITE.skill.md`. The assertion then passes.

use thermite_skill::generate;
use thermite_syntax::parser::parse;

/// Every example line the skill renders for a top-level-item-shaped construct
/// must be a program the parser accepts. The `Type::Unit` arm's example is a
/// bare `fn … ens … fx …` with no `req`, which the parser rejects.
#[test]
#[ignore = "blocker #85 — un-ignore when fixed"]
fn rendered_fn_examples_parse_clean() {
    let skill = generate();

    // The exact misleading example string the `Type::Unit` arm renders. We assert
    // it is PRESENT in the skill (so this test tracks that arm specifically) and
    // that it is NOT accepted by the parser. Authority: the skill's own
    // mandatory-clause prose + `thermite_syntax::parser`.
    let unit_example = "fn log() -> () ens true fx pure { }";
    assert!(
        skill.contains(unit_example),
        "precondition for this pin: the Type::Unit arm renders `{unit_example}`; \
         if the arm text changed, re-derive this pin"
    );

    let result = parse(unit_example);
    assert!(
        result.is_clean(),
        "the skill teaches `{unit_example}` (Type::Unit arm) but the parser \
         rejects it ({} error(s): {}). The skill's own prose says `req` is \
         mandatory — `absence of any is a parse error`. A taught example MUST \
         parse clean (design §10: the skill IS the spec).",
        result.errors.len(),
        result
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; "),
    );
}
