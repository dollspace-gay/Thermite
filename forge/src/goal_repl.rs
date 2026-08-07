//! `forge/src/goal_repl.rs` — the Lean-style goal-state REPL surface
//! (`thermite-design.md` §5/§5.1, Appendix B). v1 ships all three increments:
//! `forge goal <file> [item]` and `forge battery <file> [item]` (pure views over
//! the shipped `check::check_file` cert collection), `forge edit <file> <addr>
//! --replace <code>` (a semantic-address source-text splice over the
//! `thermite_syntax::address` machinery, then a re-check), and increment (iii),
//! #193, `forge fill <file> <hole-addr> <code>`: the `?N` body-hole fill loop.
//! A `?N` hole lexes/parses in `thermite_syntax` (fn-body statement position only)
//! and is recorded on `FnItem.holes`; a holed item does not certify (`check`
//! short-circuits it to an `OpenHole` L0 cert before lowering, REQ-5); `forge
//! goal` renders the open holes as the §5.1 `holes:` section; `forge fill` splices
//! `code` at the hole's span (reusing the (ii) splice machinery) and re-checks,
//! surfacing any new holes the fill introduced (the §5.1 loop).
//!
//! Each verb's path argument is a single `.th` source or a canonical
//! `.thpkg.json` manifest, resolved through `thermite_package::load_source`. On a
//! package the addressed item's span is module-local, so `edit`/`fill` splice
//! into the module that declares it and leave the manifest and every sibling
//! module byte-identical (`.design/build/kernel-primitives.md`, "Modules,
//! packages, and receipts").
//!
//! These verbs add no verification: `goal`/`battery` are renders over the existing
//! per-item `Vec<Certificate>` (`goal` reads `cert.obligations` + the re-parsed AST
//! contract for given/want; `battery` reads `cert.contract_quality`, which already
//! carries the §7 vacuity + mutation verdicts the gate computed: a view, no
//! accessor needed, AC-1). `edit` resolves the address, splices the replacement at
//! the addressed node's byte span in the file, re-emits, and re-runs `check_file`.
//!
//! Governing design: `.design/forge/goal-repl.md`.
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=forge-goal-repl-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-GOAL-BATTERY-VIEW | shipped | `forge/src/goal_repl.rs` | forge battery view |  |
//! | REQ-FORGE-GOAL-DETERMINISM | shipped | `forge/src/goal_repl.rs` | Goal REPL determinism and result discipline |  |
//! | REQ-FORGE-GOAL-EDIT | shipped | `forge/src/goal_repl.rs` | forge edit semantic-address splice |  |
//! | REQ-FORGE-GOAL-FILL | shipped | `forge/src/goal_repl.rs` | forge fill hole splice loop |  |
//! | REQ-FORGE-GOAL-GOAL-VIEW | shipped | `forge/src/goal_repl.rs` | forge goal state render |  |
//! | REQ-FORGE-GOAL-HOLE-PARSER-CONSUMER | shipped | `forge/src/goal_repl.rs` | Body-position hole rendering and addressing |  |
//! | REQ-FORGE-GOAL-OPEN-HOLE-VALIDATOR | shipped | `forge/src/goal_repl.rs` | Open-hole validator integration |  |
//! <!-- /generated:reqs -->

use std::path::Path;

use thermite_syntax::address::{self, AddrKind, AddressError};
use thermite_syntax::{
    Block, Clause, ClauseSelector, Contract, FnItem, ForgeItem, Item, LemmaItem, Param, Program,
    ProofItem, Span, Stmt, Type,
};

use crate::check;
use crate::cli::ForgeError;
use crate::manifest::{Certificate, Level, ObligationStatus};

/// Render the §5.1 goal state (REQ-2) for `file`, optionally restricted to one
/// `item`. A view over the `check::check_file` cert collection + the
/// re-parsed AST contract (the `given`/`want` source text the cert does not
/// carry). Adds no verification.
pub fn render_goal(file: &Path, item: Option<&str>) -> Result<String, ForgeError> {
    let certs = check::check_file(file)?;
    let program = parse_program(file)?;
    let selected = select_certs(&certs, item)?;

    let mut out = String::new();
    for cert in selected {
        out.push_str(&render_goal_item(cert, &program));
    }
    Ok(out)
}

/// Render the §7 anti-Goodhart battery (REQ-1) for `file`, optionally restricted
/// to one `item`. A pure view over each cert's `contract_quality` block: the
/// vacuity + mutation verdicts the gate already computed inside `check_file`
/// (AC-1: a view, not a re-derivation; no accessor needed because the cert
/// already carries them separably).
pub fn render_battery(file: &Path, item: Option<&str>) -> Result<String, ForgeError> {
    let certs = check::check_file(file)?;
    let selected = select_certs(&certs, item)?;

    let mut out = String::new();
    for cert in selected {
        out.push_str(&render_battery_item(cert));
    }
    Ok(out)
}

/// Render the §5.1 proof view (`.design/stage1-forge-tier.md` REQ-7 / AC-11) for the
/// forge-routed goals in `file`, optionally restricted to one `item`. A forge-routed
/// goal is a `lemma` or a `proof for f` obligation — a goal the forge discharges at L3.
/// Unlike [`render_goal`] (the v1 exec-fn goal state) the proof view renders the goal
/// with ITS HYPOTHESES IN scope: the typed parameter binders + the `req` precondition
/// the proof may assume, then the `⊢ goal` to discharge, then any open `?pN` proof
/// holes (the `forge fill` operands). The hypothesis context is derived structurally
/// from the contract — the same data the Lean discharge binds (params as free inputs,
/// `req` as the assumed precondition) — so the view needs no live elaborator. Adds no
/// verification (a pure view over the parsed program, R-CODE-5). A file with no
/// forge-routed goal (or a named `item` that is not one) is a structured Usage error,
/// not an empty render.
pub fn render_proof(file: &Path, item: Option<&str>) -> Result<String, ForgeError> {
    let program = parse_program(file)?;

    let mut out = String::new();
    let mut matched = false;
    for it in &program.items {
        let Item::Forge(forge) = it else { continue };
        match forge {
            // A `lemma` is a self-contained forge-routed goal: its `req`/params are the
            // hypotheses, its `ens` clause(s) the goal(s), its proof block the discharge.
            ForgeItem::Lemma(l) if item.is_none_or(|name| name == l.name) => {
                out.push_str(&render_lemma_proof(l));
                matched = true;
            }
            // A `proof for f` obligation discharges a specific `ens#k` clause of `f`'s
            // contract; the hypotheses are `f`'s params + `req` (resolved from `f`).
            ForgeItem::Proof(p) if item.is_none_or(|name| name == p.target) => {
                out.push_str(&render_proof_for(p, &program));
                matched = true;
            }
            // A `prop fn` is a definition and a `witness` is a covenant block — neither
            // is a goal-with-hypotheses, so neither is a proof-view target (REQ-7).
            _ => {}
        }
    }

    if !matched {
        let forge_goals: Vec<&str> = program
            .items
            .iter()
            .filter_map(|it| match it {
                Item::Forge(ForgeItem::Lemma(l)) => Some(l.name.as_str()),
                Item::Forge(ForgeItem::Proof(p)) => Some(p.target.as_str()),
                _ => None,
            })
            .collect();
        return Err(ForgeError::Usage(match item {
            Some(name) => format!(
                "no forge-routed goal named `{name}` in this file; `forge goal --proof` renders \
                 `lemma`/`proof for` goals, and the file declares: [{}]",
                forge_goals.join(", ")
            ),
            None => "this file declares no forge-routed goal (`lemma` / `proof for f`); \
                     `forge goal --proof` is the proof view for forge-tier items (REQ-7)"
                .to_string(),
        }));
    }
    Ok(out)
}

/// Render one `lemma`'s proof view (REQ-7): its parameters + `req` as the hypotheses
/// in scope, each `ens` clause as a goal, and the proof block's open `?pN` holes as
/// the `forge fill` operands.
fn render_lemma_proof(l: &LemmaItem) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "PROOF VIEW — {} (lemma, forge-routed \u{2192} L3)\n",
        l.name
    ));
    out.push_str(&render_hypotheses(&l.params, &l.req));

    // Each `ens` clause is a goal the proof must discharge.
    for (i, ens) in l.ens.iter().enumerate() {
        let tag = if l.ens.len() > 1 {
            format!(" #{}", i + 1)
        } else {
            String::new()
        };
        out.push_str(&format!("  \u{22a2} goal{tag}: {}\n", ens.text));
    }

    render_proof_holes(
        &mut out,
        &l.proof.holes,
        &format!("{}.proof", l.name),
        l.proof.text.trim().is_empty(),
    );
    out
}

/// Render the proof view for a `proof for f` item (REQ-7): one block per obligation,
/// each resolving its `ens#k` goal against `f`'s contract and binding `f`'s
/// params + `req` as the hypotheses in scope. A `proof for` whose target `f` is not a
/// `fn` in the file (or whose clause selector resolves no clause) renders an
/// "unresolved" note rather than a fabricated goal (R-CODE-2 — surface the gap).
fn render_proof_for(p: &ProofItem, program: &Program) -> String {
    let mut out = String::new();
    let target_fn = fn_of(program, &p.target);

    for ob in &p.obligations {
        let clause_label = clause_label(&ob.clause);
        out.push_str(&format!(
            "PROOF VIEW — proof for {}.{} (forge-routed \u{2192} L3)\n",
            p.target, clause_label
        ));

        match target_fn {
            Some(f) => {
                out.push_str(&render_hypotheses(&f.params, &f.contract.req));
                match resolve_clause(&f.contract, &ob.clause) {
                    Some(goal) => {
                        out.push_str(&format!("  \u{22a2} goal: {}\n", goal.text));
                    }
                    None => out.push_str(&format!(
                        "  \u{22a2} goal: <unresolved — `{}` names no clause of `{}`'s contract>\n",
                        clause_label, p.target
                    )),
                }
            }
            None => out.push_str(&format!(
                "  hypotheses in scope: <unresolved — `proof for {}` names no `fn` in this file>\n",
                p.target
            )),
        }

        render_proof_holes(
            &mut out,
            &ob.proof.holes,
            &format!("{}.proof.{}", p.target, clause_label),
            ob.proof.text.trim().is_empty(),
        );
    }
    out
}

/// Render the "hypotheses in scope" section shared by the lemma + proof-for views
/// (REQ-7): one typed binder per parameter, then the `req` precondition as an assumed
/// hypothesis (`h_req`) — omitted when `req` is the trivial `true` (it assumes
/// nothing). The same context the Lean discharge binds: params as free inputs, `req`
/// as the assumption.
fn render_hypotheses(params: &[Param], req: &Clause) -> String {
    let mut out = String::from("  hypotheses in scope:\n");
    if params.is_empty() {
        out.push_str("    (no parameters)\n");
    }
    for p in params {
        out.push_str(&format!("    {} : {}\n", p.name, type_spelling(&p.ty)));
    }
    if req.text.trim() != "true" {
        out.push_str(&format!(
            "    h_req : {}    (the `req` precondition, assumed)\n",
            req.text
        ));
    }
    out
}

/// Render the open `?pN` proof holes as the `forge fill` operands (REQ-7 / AC-11), or
/// — when the proof block is non-empty with no open hole — a "proof authored"
/// committed line. An empty hole-free proof block is an "no proof yet" note.
fn render_proof_holes(
    out: &mut String,
    holes: &[thermite_syntax::Hole],
    block_addr: &str,
    empty: bool,
) {
    if !holes.is_empty() {
        out.push_str("  proof holes:\n");
        for hole in holes {
            out.push_str(&format!(
                "    ?p{n} : open — fill with `forge fill {block_addr}.?p{n} \"<tactics>\"`\n",
                n = hole.number,
            ));
        }
    } else if empty {
        out.push_str("  proof: <empty — author tactics or open a `?pN` hole>\n");
    } else {
        out.push_str("  proof: authored (no open holes) \u{2713}\n");
    }
}

/// The clause-selector's address label (`ens#k` / `req`), the spelling
/// `address.rs` uses for the `f.proof.<clause>` address (REQ-3).
fn clause_label(sel: &ClauseSelector) -> String {
    match sel.index {
        Some(k) => format!("{}#{}", sel.keyword, k),
        None => sel.keyword.clone(),
    }
}

/// Resolve a [`ClauseSelector`] against a function's contract (REQ-7). `req` names the
/// (single) precondition; `ens#k` names the `k`-th ensures clause 0-based in source
/// order (`ens#0` is the first `ens` — the convention the forge-tier proof-obligation
/// corpus already uses, `thermite-syntax/tests/forge_items.rs`: `ens#0 by { … } ens#1
/// by { … }`). Returns `None` for an out-of-range / unknown selector (rendered as an
/// explicit "unresolved" goal rather than a fabricated one).
fn resolve_clause<'c>(contract: &'c Contract, sel: &ClauseSelector) -> Option<&'c Clause> {
    match sel.keyword.as_str() {
        "req" => Some(&contract.req),
        "ens" => contract.ens.get(sel.index? as usize),
        _ => None,
    }
}

/// The named `fn` item in `program`, if any (the `proof for f` target lookup).
fn fn_of<'p>(program: &'p Program, name: &str) -> Option<&'p FnItem> {
    program.items.iter().find_map(|i| match i {
        Item::Fn(f) if f.name == name => Some(f),
        _ => None,
    })
}

/// A human-readable spelling of a [`Type`] for the proof-view hypothesis binders
/// (REQ-7). Total over the `Type` enum (every variant has a spelling), deterministic
/// (R-CODE-5). A render helper only — it has no semantic role.
fn type_spelling(ty: &Type) -> String {
    use thermite_syntax::PrimType;
    match ty {
        Type::Prim(PrimType::U8) => "u8".to_string(),
        Type::Prim(PrimType::U16) => "u16".to_string(),
        Type::Prim(PrimType::U32) => "u32".to_string(),
        Type::Prim(PrimType::U64) => "u64".to_string(),
        Type::Prim(PrimType::Usize) => "usize".to_string(),
        Type::Prim(PrimType::Bool) => "bool".to_string(),
        Type::Unit => "()".to_string(),
        Type::Array { elem, len } => {
            let len = match len {
                thermite_syntax::ArrayLen::Literal { raw, .. } => raw,
                thermite_syntax::ArrayLen::Const(name) => name,
            };
            format!("[{}; {len}]", type_spelling(elem))
        }
        Type::Ref { mutable, inner } => {
            let m = if *mutable { "mut " } else { "" };
            format!("&{m}{}", type_spelling(inner))
        }
        Type::Slice(inner) => format!("[{}]", type_spelling(inner)),
        Type::Generic { name, arg } => format!("{name}<{}>", type_spelling(arg)),
        Type::Named(name) => name.clone(),
        Type::Box(inner) => format!("Box<{}>", type_spelling(inner)),
        Type::Vec(inner) => format!("Vec<{}>", type_spelling(inner)),
        Type::String => "String".to_string(),
        Type::Option(inner) => format!("Option<{}>", type_spelling(inner)),
        Type::Result(ok, err) => {
            format!("Result<{}, {}>", type_spelling(ok), type_spelling(err))
        }
        Type::Map(k, val) => format!("Map<{}, {}>", type_spelling(k), type_spelling(val)),
        Type::Tuple(elems) => {
            let parts: Vec<String> = elems.iter().map(type_spelling).collect();
            format!("({})", parts.join(", "))
        }
    }
}

/// Resolve `addr` against `file`, splice the replacement source text at the
/// addressed node's byte span, write the declaring module back, re-parse,
/// re-check the affected item, and return the new goal state render (REQ-3). The
/// splice is a pure function of the span + replacement text (R-CODE-5); a
/// bad/unresolvable address is a structured `ForgeError::Usage` carrying the
/// `AddressError` (R-CODE-2, not a panic). For a package the declaring module is
/// the file the manifest binds for the addressed item, so the manifest and every
/// sibling module stay byte-identical.
pub fn edit_file(file: &Path, addr: &str, replacement: &str) -> Result<String, ForgeError> {
    let resolved = crate::thermite_package::load_source(file)?;
    let program = resolved.program();

    // Resolve the address through the resolver (a bad address →
    // structured AddressError, surfaced as a Usage error, not a panic).
    address::resolve(program, addr).map_err(address_usage)?;

    // The resolver confirms the address exists; the byte span is found by walking
    // the AST (the addressing namespace v1 `edit` operates on: a `fn` root, a
    // `loop#N`, an `inv#M`, or a `dec` — semantic-addressing.md REQ-1..REQ-4).
    // The walk also returns the declaring item, because a package's spans are
    // module-local: the splice belongs to that module's file, never to the
    // manifest and never to a sibling module.
    let (item_index, span) = located_span_of_address(program, addr).ok_or_else(|| {
        ForgeError::Usage(format!(
            "address `{addr}` resolves but names no `edit`-able span in v1 (editable forms: \
             a loop `inv`/`dec` clause); a `spec fn` measure / a `struct`/`enum` is not yet \
             splice-addressable"
        ))
    })?;
    let declaring = resolved.declaring_module(item_index)?;

    // Splice the replacement text at the addressed span (the pure splice: prefix
    // + replacement + suffix over byte offsets). The spans are byte offsets into
    // the declaring module's source (lexer::Span), so this is UTF-8-boundary safe.
    let spliced = splice(&declaring.source, span, replacement);

    // Re-emit the declaring module in place, then re-parse + re-check the affected
    // item (the v0.1 whole-item check; the per-item proof cache §5.3 keeps
    // unaffected items cheap). A re-parse failure after the splice is a real,
    // reported error.
    write_file(&declaring.path, &spliced)?;

    let root = address_root(addr);
    render_goal(file, Some(root))
}

/// Fill the hole named by `addr` with `code`, re-check the affected item, and return
/// the new state render (REQ-6 / `.design/stage1-forge-tier.md` REQ-7; the §5.1 fill
/// loop). `forge fill` is a specialization of `edit` whose address names a hole: it
/// splices the replacement source at the hole token's span (reusing the increment-(ii)
/// splice machinery), re-parses, and re-checks. Two hole kinds:
///
/// - a body hole `<fn>.?N` (#193): splices into a `fn` body, re-renders the goal state.
/// - a proof hole `<lemma>.proof.?pN` / `<fn>.proof.<clause>.?pN` (stage-1 REQ-7, 2e):
///   splices into a forge-tier proof block, re-renders the proof view + the re-check
///   verdict (the frozen battery refuses an unlisted tactic — REQ-5/2c — and the
///   discharge produces the forge-tier cert with the burn receipt — REQ-7).
///
/// The filled `code` may itself contain new holes, which the re-parse records and the
/// new view surfaces (the §5.1 "fill ?0 … introducing ?1 ?2"). A non-hole address (a
/// `loop`/`inv`/`dec`/`fn`/`prop fn` node, an `edit` target rather than a `fill`
/// target) is a `ForgeError::Usage` (use `forge edit` for those); a bad/unresolvable
/// hole address is a structured error, not a panic (R-CODE-2).
pub fn fill_hole(file: &Path, addr: &str, code: &str) -> Result<String, ForgeError> {
    let resolved = crate::thermite_package::load_source(file)?;
    let program = resolved.program();

    // Resolve the address (bad address → structured AddressError, not a panic).
    let entry = address::resolve(program, addr).map_err(address_usage)?;

    // `fill` targets a hole only — a body `?N` or a proof `?pN`. A non-hole address is
    // the `edit` surface; reject it with an actionable message rather than silently
    // splicing (the two verbs have distinct contracts, REQ-3 vs REQ-6/REQ-7).
    let is_proof_hole = match entry.kind {
        AddrKind::Hole => false,
        AddrKind::ProofHole => true,
        other => {
            return Err(ForgeError::Usage(format!(
                "address `{addr}` is not a hole (it names a {other:?} node); `forge fill` targets \
                 a `?N` body hole or a `?pN` proof hole — use `forge edit {addr} --replace \
                 <code>` to splice a non-hole node"
            )));
        }
    };

    // The hole token's span is the splice target (mirroring `edit`'s span walk; the
    // `?pN` proof-hole span resolves through the same `located_span_of_address`),
    // together with the item whose declaring module owns that span.
    let (item_index, span) = located_span_of_address(program, addr).ok_or_else(|| {
        ForgeError::Usage(format!(
            "hole address `{addr}` resolves but names no hole span (internal: the hole is recorded \
             on its item but its span was not found)"
        ))
    })?;
    let declaring = resolved.declaring_module(item_index)?;

    // Splice the fill code at the hole's position (the pure splice, R-CODE-5), re-emit
    // the declaring module in place, then re-check the affected item. A re-parse
    // failure after the splice (malformed fill code) is a reported error, not swallowed.
    let spliced = splice(&declaring.source, span, code);
    write_file(&declaring.path, &spliced)?;
    // Surface a malformed-fill parse error here rather than as a confusing downstream
    // render failure (the filled code may not parse).
    let _ = parse_program(file)?;

    let root = address_root(addr);
    if is_proof_hole {
        render_proof_after_fill(file, root)
    } else {
        render_goal(file, Some(root))
    }
}

/// Re-render after a `?pN` proof-hole fill (REQ-7): the updated proof view (remaining
/// open holes the fill introduced, or the authored-no-holes line), then the re-check
/// verdict for the affected item — a frozen-battery refusal (REQ-5/2c: "cite-unlisted
/// → refused") or the forge-tier discharge cert (carrying the burn receipt, REQ-7).
/// The re-check runs the same `check::check_file` the body-hole path runs.
fn render_proof_after_fill(file: &Path, root: &str) -> Result<String, ForgeError> {
    let mut out = render_proof(file, Some(root))?;
    // The forge-tier re-check runs on the LEAN path (`check_file_lean`): a closed
    // forge-tier goal certifies + carries the burn receipt there (REQ-7 / AC-11), where
    // the Verus-only default `check_file` would skip the lemma. The frozen-battery
    // refusal + open-hole short-circuit still fire (they are on the shared base path).
    for cert in check::check_file_lean(file)? {
        if cert.item == root {
            out.push_str(&render_proof_cert_status(&cert));
        }
    }
    Ok(out)
}

/// The re-check status line for a forge-tier item after a proof-hole fill (REQ-7): a
/// reject (the frozen-battery refusal or an open-proof-hole short-circuit) names its
/// cause; a certified discharge names its level + the burn receipt (committed proof
/// tokens + cited lemmas) the cert now carries.
fn render_proof_cert_status(cert: &Certificate) -> String {
    if let Some(reject) = &cert.reject {
        return format!(
            "  re-check: NOT CERTIFIED — {} ({})\n",
            reject.cause, reject.detail
        );
    }
    let mut out = format!("  re-check: certified {} \u{2713}\n", level_str(cert.level));
    if let Some(burn) = &cert.burn {
        out.push_str(&format!("  burn: {} proof token(s)", burn.proof_tokens));
        if !burn.cited_lemmas.is_empty() {
            out.push_str(&format!("; cited lemmas: {}", burn.cited_lemmas.join(", ")));
        }
        if let Some(authoring) = burn.authoring_tokens {
            out.push_str(&format!("; authoring tokens: {authoring}"));
        }
        out.push('\n');
    }
    out
}

/// Render one item's goal state (REQ-2; §5.1). The `given` is the `req` clause
/// text; the `want` is the `ens` clause texts; then each obligation as discharged
/// or failed-with-witness; a clean cert renders `all goals discharged` + the level
/// + the battery line.
fn render_goal_item(cert: &Certificate, program: &Program) -> String {
    let mut out = String::new();
    out.push_str(&format!("GOAL STATE — {}\n", cert.item));

    // given / want from the re-parsed contract (the cert does not carry the clause
    // source text; the AST does — semantic-addressing.md AC-1 keeps verbatim
    // `text` on every clause).
    if let Some(contract) = contract_of(program, &cert.item) {
        out.push_str(&format!("  given: {}\n", contract.req.text));
        for (i, ens) in contract.ens.iter().enumerate() {
            let label = if i == 0 { "want " } else { "     " };
            out.push_str(&format!("  {label}: {}\n", ens.text));
        }
    }

    // Open holes (`?N`) render as the §5.1 `holes:` section: the open goals the
    // agent must fill (`.design/forge/goal-repl.md` REQ-5; the §5.1 `holes: ?0 :
    // body` line). A holed item does not certify (its cert is the `OpenHole` reject),
    // so the holes line is the goal-state's actionable next move. Listed in document
    // order, by `<fn>.?N` address (the `forge fill` operand).
    let holes = holes_of(program, &cert.item);
    if !holes.is_empty() {
        out.push_str("  holes:\n");
        for hole in holes {
            out.push_str(&format!(
                "    ?{n} : body — fill with `forge fill {item}.?{n} <code>`\n",
                n = hole.number,
                item = cert.item,
            ));
        }
    }

    // A rejected cert (a §6/§13 vacuity/slag reject, or a #193 open-hole reject:
    // Level::L0 with a `reject` cause) is reported as the obligation-blocking cause,
    // not silently dropped. For an `OpenHole` reject the `holes:` section above is
    // the actionable view; the status line names the cert verdict.
    if let Some(reject) = &cert.reject {
        out.push_str(&format!(
            "  status: NOT CERTIFIED — {} ({})\n",
            reject.cause, reject.detail
        ));
        return out;
    }

    // Per-obligation status (§5.1 property 2: a failure carries its concrete
    // witness, not a bare adjective).
    let any_failed = cert
        .obligations
        .iter()
        .any(|o| o.status == ObligationStatus::Failed);
    if cert.level == Level::L3 && !any_failed {
        out.push_str(&format!(
            "  ALL GOALS DISCHARGED \u{2713}  {} certified {}\n",
            cert.item,
            level_str(cert.level)
        ));
    } else {
        for ob in &cert.obligations {
            match ob.status {
                ObligationStatus::Discharged => {
                    out.push_str(&format!("  \u{2713} discharged: {}\n", ob.name));
                }
                ObligationStatus::Failed => {
                    out.push_str(&format!("  \u{2717} open — obligation: {}\n", ob.name));
                    if let Some(loc) = &ob.location {
                        out.push_str(&format!("        at {loc}\n"));
                    }
                    if let Some(diag) = &ob.diagnostic {
                        out.push_str(&format!("        counterexample: {diag}\n"));
                    }
                }
            }
        }
        out.push_str(&format!(
            "  status: {} (not all goals discharged)\n",
            level_str(cert.level)
        ));
    }

    // §5.1 "contract score" line — the battery verdict inline (the same view
    // `forge battery` renders standalone).
    out.push_str(&format!("  contract score: {}\n", battery_line(cert)));
    out
}

/// Render one item's §7 battery view (REQ-1). The vacuity verdict + the mutation
/// kill-ratio (+ the surviving mutant, if any), read straight off the cert's
/// `contract_quality`, not recomputed.
fn render_battery_item(cert: &Certificate) -> String {
    let mut out = String::new();
    out.push_str(&format!("battery — {}\n", cert.item));

    // A gate-rejected cert (a §7.1 vacuity / §13 slag reject: Level::L0 with a
    // `reject` cause) keeps `contract_quality` at `forward_declared()` placeholder
    // `false`s, not a clean verdict. Surface the gate's reject cause, mirroring
    // `render_goal_item`, rather than the placeholder non-vacuous line or `mutants
    // killed 0/0`. (REQ-1: a view re-defines no verdict; the pipeline's verdict
    // for a triage-rejected item is the reject.)
    if let Some(reject) = &cert.reject {
        out.push_str(&format!(
            "  vacuity: VACUOUS — {} ({})\n",
            reject.cause, reject.detail
        ));
        return out;
    }

    let q = &cert.contract_quality;
    out.push_str(&format!(
        "  vacuity: {}\n",
        if q.tautology || q.vacuous_precondition {
            vacuity_reject_phrase(cert)
        } else {
            "non-vacuous (tautology=false, vacuous_precondition=false)".to_string()
        }
    ));
    out.push_str(&format!("  mutants killed: {}\n", q.mutants_killed));
    if let Some(survivor) = &q.survivor {
        out.push_str(&format!("  survivor: {survivor}\n"));
    }
    out
}

/// The one-line battery summary (§5.1 "contract score" — reused by the goal
/// render). `non-vacuous ✓, mutants killed 17/18`.
fn battery_line(cert: &Certificate) -> String {
    let q = &cert.contract_quality;
    let vac = if q.tautology || q.vacuous_precondition {
        "VACUOUS"
    } else {
        "non-vacuous \u{2713}"
    };
    format!("{vac}, mutants killed {}", q.mutants_killed)
}

/// Phrase the vacuity rejection (tautology vs vacuous precondition).
fn vacuity_reject_phrase(cert: &Certificate) -> String {
    let q = &cert.contract_quality;
    if q.tautology {
        "VACUOUS — the `ens` is a tautology (holds for any body)".to_string()
    } else if q.vacuous_precondition {
        "VACUOUS — the `req` is unsatisfiable (no input reaches the body)".to_string()
    } else {
        "non-vacuous".to_string()
    }
}

/// Select the certs to render: all of them, or the single named `item` (a name
/// that matches no checked item is a Usage error, not an empty render).
fn select_certs<'c>(
    certs: &'c [Certificate],
    item: Option<&str>,
) -> Result<Vec<&'c Certificate>, ForgeError> {
    match item {
        None => Ok(certs.iter().collect()),
        Some(name) => {
            let matched: Vec<&Certificate> = certs.iter().filter(|c| c.item == name).collect();
            if matched.is_empty() {
                let known: Vec<&str> = certs.iter().map(|c| c.item.as_str()).collect();
                return Err(ForgeError::Usage(format!(
                    "no checked item named `{name}`; the file declares: [{}]",
                    known.join(", ")
                )));
            }
            Ok(matched)
        }
    }
}

/// The contract of the named `fn` item in `program`, if any (the source of the
/// `given`/`want` lines; a `spec fn`/`struct`/`enum` has no `req`/`ens` contract).
fn contract_of<'p>(program: &'p Program, item: &str) -> Option<&'p Contract> {
    program.items.iter().find_map(|i| match i {
        Item::Fn(f) if f.name == item => Some(&f.contract),
        _ => None,
    })
}

/// The shared open-hole refusal text for a holed exec fn (#193/#195,
/// goal-repl.md REQ-4/REQ-5). Returns `Some(detail)` iff `f` carries any open body
/// hole (`?N`), naming every `<fn>.?N` address + the first open goal, mirroring the
/// `check::check_file_with_options` `OpenHole` reject language verbatim so every
/// lowering path (`build::build_file`, `body_tv`, `exec_tv`) refuses/skips a holed
/// item with one message rather than three drifting copies (the #192 lesson).
/// `None` for a hole-free fn. A holed item is L0-equivalent (incomplete) and does
/// not lower; `check.rs`'s per-item loop, `build_file`, and the two TV phases all
/// gate on this. Pure function of `f.holes` (R-CODE-5).
pub(crate) fn open_hole_reason(f: &thermite_syntax::FnItem) -> Option<String> {
    let first = f.holes.first()?;
    let addrs: Vec<String> = f
        .holes
        .iter()
        .map(|h| format!("{}.?{}", f.name, h.number))
        .collect();
    Some(format!(
        "`{}` has {} open body hole(s) [{}] — an item with any `?N` hole is \
         L0-equivalent (incomplete) and does NOT certify until every hole is \
         filled (`forge fill {} <code>`). First open goal: hole `?{}` at byte \
         {} (`.design/forge/goal-repl.md` REQ-5).",
        f.name,
        f.holes.len(),
        addrs.join(", "),
        addrs[0],
        first.number,
        first.span.start,
    ))
}

/// The shared open-proof-hole refusal text for a forge-tier item carrying any open
/// `?pN` proof hole (`.design/stage1-forge-tier.md` REQ-3 / AC-7). Returns
/// `Some(detail)` iff the item's proof block(s) carry any open proof hole — a
/// `lemma`'s proof block, or any `proof for f` obligation's `by { … }` block (a
/// `prop fn`/`witness` carries no proof block). Mirrors [`open_hole_reason`]: an
/// item with an open proof hole is incomplete and does not certify and does not
/// build (the same never-ship-incomplete invariant), so `check.rs` and `build.rs`
/// gate on this with one message. The covenant/proof-view consumers (2b/2e) own
/// the fill loop; this gate only refuses an open one. Pure function of the item's
/// proof holes (R-CODE-5).
pub(crate) fn open_proof_hole_reason(forge: &thermite_syntax::ForgeItem) -> Option<String> {
    use thermite_syntax::ForgeItem;
    // Collect (address, hole) for every open proof hole the item carries.
    let mut entries: Vec<(String, &thermite_syntax::Hole)> = Vec::new();
    match forge {
        ForgeItem::Lemma(l) => {
            for h in &l.proof.holes {
                entries.push((format!("{}.proof.?p{}", l.name, h.number), h));
            }
        }
        ForgeItem::Proof(p) => {
            for ob in &p.obligations {
                let clause = match ob.clause.index {
                    Some(k) => format!("{}.proof.{}#{}", p.target, ob.clause.keyword, k),
                    None => format!("{}.proof.{}", p.target, ob.clause.keyword),
                };
                for h in &ob.proof.holes {
                    entries.push((format!("{}.?p{}", clause, h.number), h));
                }
            }
        }
        // A `prop fn` body and a `witness` block carry no proof block (REQ-3).
        ForgeItem::PropFn(_) | ForgeItem::Witness(_) => {}
    }
    let (_, first) = entries.first()?;
    let addrs: Vec<&str> = entries.iter().map(|(a, _)| a.as_str()).collect();
    Some(format!(
        "`{}` has {} open proof hole(s) [{}] — a forge-tier item with any `?pN` \
         proof hole is incomplete and does NOT certify or build until every proof \
         hole is filled. First open proof goal: hole `?p{}` at byte {} \
         (`.design/stage1-forge-tier.md` REQ-3 / AC-7).",
        forge.name(),
        entries.len(),
        addrs.join(", "),
        first.number,
        first.span.start,
    ))
}

/// The open body holes (`?N`) of the named `fn` item, in document order (#193,
/// goal-repl.md REQ-4). Empty for a hole-free fn / a `spec fn`/`struct`/`enum`.
/// The source of the §5.1 `holes:` render section.
fn holes_of<'p>(program: &'p Program, item: &str) -> &'p [thermite_syntax::Hole] {
    program
        .items
        .iter()
        .find_map(|i| match i {
            Item::Fn(f) if f.name == item => Some(f.holes.as_slice()),
            _ => None,
        })
        .unwrap_or(&[])
}

/// The byte span an editable address names (REQ-3): a `fn` root, a `loop#N`, an
/// `inv#M`, or a `dec`. Mirrors the `address::addresses_of` traversal (which
/// returns no span) so `edit` can splice. Returns `None` for an address that
/// resolves but names no v1-editable span.
///
/// The walk also returns the index of the top-level item whose source the span
/// indexes into. A package program's spans are module-local, so a splice needs
/// both the span and the declaring item to find the module file that owns it
/// (`.design/build/kernel-primitives.md` — source identity and diagnostic spans
/// are preserved per module).
fn located_span_of_address(program: &Program, addr: &str) -> Option<(usize, Span)> {
    let entry_kind = address::resolve(program, addr).ok()?.kind;

    // A `?pN` proof hole lives on a forge-tier item (a `lemma`'s proof block or a
    // `proof for f` obligation's block), not on a `fn`, so it dispatches before the
    // `fn` lookup below (stage1-forge-tier.md REQ-7, increment 2e).
    if entry_kind == AddrKind::ProofHole {
        return proof_hole_span(program, addr);
    }

    let mut segs = addr.split('.');
    let root = segs.next()?;

    let (item_index, fn_item) =
        program
            .items
            .iter()
            .enumerate()
            .find_map(|(index, i)| match i {
                Item::Fn(f) if f.name == root => Some((index, f)),
                _ => None,
            })?;

    let span = match entry_kind {
        AddrKind::Fn => Some(fn_item.span),
        AddrKind::Loop | AddrKind::Inv | AddrKind::Dec => {
            let body = fn_item.body.as_ref()?;
            // The address's inner segments name `loop#N` then optionally
            // `inv#M`/`dec`. Walk to the addressed loop, then to the clause.
            let loop_seg = segs.next()?; // loop#N
            let loop_index: usize = loop_seg.strip_prefix("loop#")?.parse().ok()?;
            let lp = nth_loop(body, loop_index)?;
            match segs.next() {
                None => Some(lp.span),
                Some("dec") => Some(lp.dec.span),
                Some(clause_seg) => {
                    let m: usize = clause_seg.strip_prefix("inv#")?.parse().ok()?;
                    lp.invs.get(m.checked_sub(1)?).map(|c| c.span)
                }
            }
        }
        AddrKind::Hole => {
            // A hole address `<fn>.?N` (#193, goal-repl.md REQ-4): the splice target
            // is the `?N` token's span, recorded on `FnItem.holes` by its verbatim
            // surface number. Find the hole whose number matches the `?N` segment.
            let hole_seg = segs.next()?; // ?N
            let number: u32 = hole_seg.strip_prefix('?')?.parse().ok()?;
            fn_item
                .holes
                .iter()
                .find(|h| h.number == number)
                .map(|h| h.span)
        }
        AddrKind::SpecFn => None,
        // A `ProofHole` is handled by the early dispatch above; a `Forge` root (a
        // `prop fn`/`lemma`/`proof`/`witness` node) has no `edit`-able span (its
        // consumers are the proof-view 2e / library 3) — resolve-but-no-span, mirroring
        // the inert `SpecFn => None` arm.
        AddrKind::Forge | AddrKind::ProofHole => None,
    };
    Some((item_index, span?))
}

/// Resolve a `?pN` proof-hole address to the hole token's byte span (stage1-forge-tier.md
/// REQ-7, increment 2e — the `forge fill` splice target). A proof hole lives on a
/// forge-tier item: a `lemma`'s proof block (`<lemma>.proof.?pN`) or a `proof for f`
/// obligation's block (`<f>.proof.<clause>.?pN`). Walks the same forge items
/// `address::collect_forge_addresses` enumerates, matching the clause label + hole
/// number, and returns the matched hole's span. `None` for an address that resolves
/// but names no recorded hole (a clean miss, never a panic — R-CODE-2).
fn proof_hole_span(program: &Program, addr: &str) -> Option<(usize, Span)> {
    let segs: Vec<&str> = addr.split('.').collect();
    // `<root> . proof . ?pN`            (lemma — 3 segments)
    // `<root> . proof . <clause> . ?pN` (proof-for — 4 segments)
    let root = *segs.first()?;
    if *segs.get(1)? != "proof" {
        return None;
    }
    let hole_seg = *segs.last()?;
    let number: u32 = hole_seg.strip_prefix("?p")?.parse().ok()?;

    for (index, it) in program.items.iter().enumerate() {
        let Item::Forge(forge) = it else { continue };
        match forge {
            // A lemma: `<lemma>.proof.?pN` — exactly 3 segments, no clause.
            ForgeItem::Lemma(l) if l.name == root && segs.len() == 3 => {
                return l
                    .proof
                    .holes
                    .iter()
                    .find(|h| h.number == number)
                    .map(|h| (index, h.span));
            }
            // A proof-for: `<f>.proof.<clause>.?pN` — 4 segments; the clause segment
            // selects the obligation by its label (`ens#k`/`req`).
            ForgeItem::Proof(p) if p.target == root && segs.len() == 4 => {
                let clause_seg = segs[2];
                for ob in &p.obligations {
                    if clause_label(&ob.clause) == clause_seg {
                        return ob
                            .proof
                            .holes
                            .iter()
                            .find(|h| h.number == number)
                            .map(|h| (index, h.span));
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Find the `loop_index`-th (1-based) loop in `body`, in the same source-order /
/// flat-numbering scheme `address::collect_in_block` uses (descend into `if`
/// branches; nested loops continue the flat function-level count).
fn nth_loop(body: &Block, loop_index: usize) -> Option<&thermite_syntax::LoopNode> {
    let mut counter = 0usize;
    find_loop(body, loop_index, &mut counter)
}

fn find_loop<'b>(
    block: &'b Block,
    target: usize,
    counter: &mut usize,
) -> Option<&'b thermite_syntax::LoopNode> {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(lp) => {
                *counter += 1;
                if *counter == target {
                    return Some(lp);
                }
                if let Some(found) = find_loop(&lp.body, target, counter) {
                    return Some(found);
                }
            }
            Stmt::If { then, else_, .. } => {
                if let Some(found) = find_loop(then, target, counter) {
                    return Some(found);
                }
                if let Some(eb) = else_ {
                    if let Some(found) = find_loop(eb, target, counter) {
                        return Some(found);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Splice `replacement` over the byte range named by `span` in `src` (the pure
/// edit, R-CODE-5). `span` is a byte offset + length into `src` (lexer::Span), so
/// slicing is on UTF-8 boundaries the lexer already aligned to.
fn splice(src: &str, span: Span, replacement: &str) -> String {
    let mut out = String::with_capacity(src.len() + replacement.len());
    out.push_str(&src[..span.start]);
    out.push_str(replacement);
    out.push_str(&src[span.end()..]);
    out
}

/// The root (fn-name) segment of an address — the item `edit` re-checks.
fn address_root(addr: &str) -> &str {
    addr.split('.').next().unwrap_or(addr)
}

/// Map a structured `AddressError` (REQ-7) into a `ForgeError::Usage` (not a
/// panic; the error path for a bad `edit`/`goal` address).
fn address_usage(e: AddressError) -> ForgeError {
    ForgeError::Usage(format!("address resolution failed: {e}"))
}

/// Write the source file back (IO error → `ForgeError::Io`).
fn write_file(file: &Path, contents: &str) -> Result<(), ForgeError> {
    std::fs::write(file, contents).map_err(|e| ForgeError::Io {
        path: file.display().to_string(),
        source: e,
    })
}

/// Parse `file` into a clean `Program` (a parse failure is a `ForgeError::Parse`,
/// surfaced, not swallowed). `file` is a single `.th` source or a canonical
/// `.thpkg.json` manifest, whose declared module closure resolves through the
/// shared front door. Re-parse of a known-good corpus file is deterministic
/// (R-CODE-5).
fn parse_program(file: &Path) -> Result<Program, ForgeError> {
    Ok(crate::thermite_package::load_source(file)?
        .program()
        .clone())
}

/// The string form of a [`Level`] for the goal render.
fn level_str(level: Level) -> &'static str {
    match level {
        Level::L0 => "L0",
        Level::L1 => "L1",
        Level::L2 => "L2",
        Level::L3 => "L3",
        Level::L4 => "L4",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ContractQuality, ObligationResult};
    use thermite_syntax::parse;

    fn parse_ok(src: &str) -> Program {
        let p = parse(src);
        assert!(p.is_clean(), "fixture must parse clean: {:?}", p.errors);
        p.program
    }

    /// A discharged-L3 cert with the corpus battery verdict (anchored to
    /// `conformance/sum.cert.json`, not copied from the verb, R-CHAR-3).
    fn sum_cert_l3() -> Certificate {
        let mut c = Certificate::new(
            "sum",
            Level::L3,
            vec!["pure".to_string()],
            0,
            vec![ObligationResult::discharged("sum_ensures")],
        );
        c.contract_quality = ContractQuality {
            tautology: false,
            vacuous_precondition: false,
            mutants_killed: "17/18".to_string(),
            survivor: Some(
                "mutant#11: `i = i + 1` → `i = i + 2` survives ens but killed by inv#2".to_string(),
            ),
        };
        c
    }

    // REQ-1 / AC-1: the battery view reads the cert's contract_quality verbatim:
    // the §7 verdict the gate computed, not recomputed. Anchored to the golden
    // sum.cert.json values (`17/18`, non-vacuous).
    #[test]
    fn battery_view_reads_contract_quality() {
        let cert = sum_cert_l3();
        let rendered = render_battery_item(&cert);
        assert!(rendered.contains("mutants killed: 17/18"), "{rendered}");
        assert!(rendered.contains("non-vacuous"), "{rendered}");
        assert!(
            rendered.contains("survives ens but killed by inv#2"),
            "{rendered}"
        );
    }

    // REQ-2 / AC-2: a clean L3 cert renders all goals discharged + the level + the
    // §7 battery line.
    #[test]
    fn goal_render_discharged() {
        let program = parse_ok("fn f(n: u32) -> u32 req n < 10 ens result == n fx pure { n }");
        let cert = {
            let mut c = sum_cert_l3();
            c.item = "f".to_string();
            c
        };
        let rendered = render_goal_item(&cert, &program);
        assert!(rendered.contains("ALL GOALS DISCHARGED"), "{rendered}");
        assert!(rendered.contains("certified L3"), "{rendered}");
        assert!(rendered.contains("given: n < 10"), "{rendered}");
        assert!(rendered.contains("result == n"), "{rendered}");
        assert!(rendered.contains("mutants killed 17/18"), "{rendered}");
    }

    // REQ-2 / AC-3: a failed obligation renders the concrete witness from the
    // ObligationResult diagnostic + location, not a bare adjective (§5.1
    // property 2).
    #[test]
    fn goal_render_counterexample() {
        let program = parse_ok("fn f(n: u32) -> u32 req n < 10 ens result == n fx pure { n }");
        let cert = Certificate::new(
            "f",
            Level::L0,
            vec!["pure".to_string()],
            0,
            vec![ObligationResult::failed(
                "lo <= hi preserved across `lo = mid + 1`",
                Some("binary_search.th:20:5".to_string()),
                Some("lo=3, hi=3, mid=3 -> lo=4 > hi=3".to_string()),
            )],
        );
        let rendered = render_goal_item(&cert, &program);
        assert!(rendered.contains("open — obligation:"), "{rendered}");
        assert!(
            rendered.contains("counterexample: lo=3, hi=3, mid=3 -> lo=4 > hi=3"),
            "{rendered}"
        );
        assert!(rendered.contains("binary_search.th:20:5"), "{rendered}");
        assert!(
            !rendered.contains("ALL GOALS DISCHARGED"),
            "a failed obligation must not claim discharge: {rendered}"
        );
    }

    // REQ-3: the span-of-address walk finds the inv#2 clause span, and the splice
    // replaces that clause's source text (round-trips to the same address
    // set with the new text). Drives the binary_search corpus shape.
    #[test]
    fn edit_splice_replaces_clause_span() {
        let src = std::fs::read_to_string("../conformance/binary_search.th")
            .expect("read binary_search.th");
        let program = parse_ok(&src);
        let (_, span) = located_span_of_address(&program, "binary_search.loop#1.inv#2")
            .expect("inv#2 has a span");
        // The addressed span must cover the verbatim inv#2 clause text.
        let original = &src[span.start..span.end()];
        assert_eq!(original, "forall_below(haystack, lo, |x| x < needle)");

        let replacement = "forall_below(haystack, lo, |x| x <= needle)";
        let spliced = splice(&src, span, replacement);
        // The spliced file re-parses, and inv#2 now resolves to the new text.
        let reparsed = parse_ok(&spliced);
        let entry = address::resolve(&reparsed, "binary_search.loop#1.inv#2")
            .expect("inv#2 still resolves");
        assert_eq!(entry.text.as_deref(), Some(replacement));
        // The address set is unchanged (stability under the edit, REQ-3).
        let before: Vec<String> = address::addresses_of(&program)
            .into_iter()
            .map(|e| e.addr)
            .collect();
        let after: Vec<String> = address::addresses_of(&reparsed)
            .into_iter()
            .map(|e| e.addr)
            .collect();
        assert_eq!(before, after);
    }

    // REQ-3 / REQ-7: a bad address resolves to a structured error, never a panic.
    #[test]
    fn edit_bad_address_is_structured_error() {
        let program = parse_ok("fn f(n: u32) -> u32 req n < 10 ens result == n fx pure { n }");
        // A well-formed but absent address → NotFound; a malformed one → Malformed.
        assert!(matches!(
            address::resolve(&program, "f.loop#9"),
            Err(AddressError::NotFound(_))
        ));
        // The Usage mapping never panics and carries the cause.
        let err = address_usage(AddressError::NotFound("f.loop#9".to_string()));
        match err {
            ForgeError::Usage(msg) => assert!(msg.contains("no such address `f.loop#9`"), "{msg}"),
            other => panic!("expected Usage, got {other:?}"),
        }
        // An address that resolves but is not v1-editable (a spec-fn root) → None
        // from located_span_of_address (a clean miss, not a panic).
        let spec_program = parse_ok("spec fn m(n: u32) -> u32 dec n { n }");
        assert!(located_span_of_address(&spec_program, "m").is_none());
    }

    /// Extract the first `ForgeItem` from a parsed program (test helper).
    fn first_forge(program: &Program) -> &ForgeItem {
        program
            .items
            .iter()
            .find_map(|i| match i {
                Item::Forge(f) => Some(f),
                _ => None,
            })
            .expect("a forge item")
    }

    // REQ-7 / AC-11: the proof view renders a lemma's hypotheses in scope (its typed
    // params), the `⊢ goal` (the `ens`), and an open `?pN` hole as the `forge fill`
    // operand. A non-trivial `req` is bound as `h_req`.
    #[test]
    fn proof_view_renders_lemma_hypotheses_goal_and_holes() {
        let program = parse_ok("lemma le_id(a: u64, b: u64) req a <= b ens a <= b proof { ?p0 }");
        let ForgeItem::Lemma(l) = first_forge(&program) else {
            panic!("expected a lemma");
        };
        let r = render_lemma_proof(l);
        assert!(
            r.contains("PROOF VIEW — le_id (lemma, forge-routed \u{2192} L3)"),
            "{r}"
        );
        assert!(r.contains("a : u64"), "typed param binder: {r}");
        assert!(r.contains("b : u64"), "typed param binder: {r}");
        assert!(
            r.contains("h_req : a <= b"),
            "the `req` precondition is in scope as a hypothesis: {r}"
        );
        assert!(
            r.contains("\u{22a2} goal: a <= b"),
            "the `ens` is the goal: {r}"
        );
        assert!(
            r.contains("?p0 : open — fill with `forge fill le_id.proof.?p0 \"<tactics>\"`"),
            "the open proof hole is the fill operand: {r}"
        );
    }

    // REQ-7: a `req true` lemma assumes nothing — no `h_req` line; a hole-free authored
    // proof shows the committed line.
    #[test]
    fn proof_view_omits_trivial_req_and_marks_authored() {
        let program = parse_ok("lemma add_id(a: u64) req true ens a == a proof { omega }");
        let ForgeItem::Lemma(l) = first_forge(&program) else {
            panic!("expected a lemma");
        };
        let r = render_lemma_proof(l);
        assert!(!r.contains("h_req"), "`req true` assumes nothing: {r}");
        assert!(r.contains("\u{22a2} goal: a == a"), "{r}");
        assert!(r.contains("proof: authored (no open holes)"), "{r}");
    }

    // REQ-7 / AC-11: a `proof for f` obligation resolves its `ens#k` goal against `f`'s
    // contract (0-based: `ens#0` is the first `ens`) and binds `f`'s params + `req` as
    // the hypotheses in scope; the `?pN` hole names the `f.proof.ens#k.?pN` fill operand.
    #[test]
    fn proof_view_proof_for_resolves_clause_against_target_contract() {
        let src = "fn maxv(x: u64, y: u64) -> u64 req true ens result >= x ens result >= y \
                   fx pure { if x > y { x } else { y } }\n\
                   proof for maxv { ens#1 by { ?p0 } }";
        let program = parse_ok(src);
        let ForgeItem::Proof(p) = program
            .items
            .iter()
            .find_map(|i| match i {
                Item::Forge(f @ ForgeItem::Proof(_)) => Some(f),
                _ => None,
            })
            .expect("a proof-for item")
        else {
            panic!("expected a proof-for");
        };
        let r = render_proof_for(p, &program);
        assert!(
            r.contains("PROOF VIEW — proof for maxv.ens#1 (forge-routed \u{2192} L3)"),
            "{r}"
        );
        assert!(
            r.contains("x : u64") && r.contains("y : u64"),
            "f's params bound: {r}"
        );
        // `ens#1` is the second ens clause (0-based), `result >= y`.
        assert!(
            r.contains("\u{22a2} goal: result >= y"),
            "ens#1 resolves to the second ens clause: {r}"
        );
        assert!(
            r.contains("forge fill maxv.proof.ens#1.?p0"),
            "the proof-hole fill operand: {r}"
        );
    }

    // REQ-7 / AC-11: a `?pN` proof-hole address resolves to the hole token's span —
    // the `forge fill` splice target — for both a lemma and a proof-for obligation.
    #[test]
    fn proof_hole_span_resolves_lemma_and_proof_for() {
        // Lemma: `l.proof.?p0` spans the `?p0` token in the proof block.
        let lemma_src = "lemma l(a: u64) req true ens a == a proof { ?p0 }";
        let lp = parse_ok(lemma_src);
        let (_, span) = located_span_of_address(&lp, "l.proof.?p0").expect("lemma proof-hole span");
        assert_eq!(&lemma_src[span.start..span.end()], "?p0");

        // Proof-for: `f.proof.ens#0.?p1` spans the `?p1` token in that obligation.
        let pf_src = "fn f(n: u32) -> u32 req true ens result == n fx pure { n }\n\
                      proof for f { ens#0 by { ?p1 } }";
        let pf = parse_ok(pf_src);
        let (_, span2) =
            located_span_of_address(&pf, "f.proof.ens#0.?p1").expect("proof-for hole span");
        assert_eq!(&pf_src[span2.start..span2.end()], "?p1");

        // A resolvable forge root that is not a hole has no fill span (clean miss).
        assert!(located_span_of_address(&lp, "l").is_none());
    }

    // REQ-7 / R-CODE-2: a `proof for` whose clause selector is out of range renders an
    // explicit "unresolved" goal, never a fabricated one or a panic.
    #[test]
    fn proof_view_proof_for_out_of_range_clause_is_unresolved() {
        let src = "fn f(n: u32) -> u32 req true ens result == n fx pure { n }\n\
                   proof for f { ens#9 by { omega } }";
        let program = parse_ok(src);
        let ForgeItem::Proof(p) = first_forge(&program) else {
            panic!("expected a proof-for");
        };
        let r = render_proof_for(p, &program);
        assert!(
            r.contains("unresolved"),
            "out-of-range clause is unresolved: {r}"
        );
    }
}
