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
use thermite_syntax::{Block, Contract, Item, Program, Span, Stmt};

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

/// Resolve `addr` against `file`, splice the replacement source text at the
/// addressed node's byte span, write the file back, re-parse, re-check the
/// affected item, and return the new goal state render (REQ-3). The splice is a
/// pure function of the span + replacement text (R-CODE-5); a bad/unresolvable
/// address is a structured `ForgeError::Usage` carrying the `AddressError`
/// (R-CODE-2, not a panic).
pub fn edit_file(file: &Path, addr: &str, replacement: &str) -> Result<String, ForgeError> {
    let src = read_file(file)?;
    let program = parse_program(file)?;

    // Resolve the address through the resolver (a bad address →
    // structured AddressError, surfaced as a Usage error, not a panic).
    address::resolve(&program, addr).map_err(address_usage)?;

    // The resolver confirms the address exists; the byte span is found by walking
    // the AST (the addressing namespace v1 `edit` operates on: a `fn` root, a
    // `loop#N`, an `inv#M`, or a `dec` — semantic-addressing.md REQ-1..REQ-4).
    let span = span_of_address(&program, addr).ok_or_else(|| {
        ForgeError::Usage(format!(
            "address `{addr}` resolves but names no `edit`-able span in v1 (editable forms: \
             a loop `inv`/`dec` clause); a `spec fn` measure / a `struct`/`enum` is not yet \
             splice-addressable"
        ))
    })?;

    // Splice the replacement text at the addressed span (the pure splice: prefix
    // + replacement + suffix over byte offsets). The spans are byte offsets into
    // the original source (lexer::Span), so this is UTF-8-boundary safe.
    let spliced = splice(&src, span, replacement);

    // Re-emit the file in place, then re-parse + re-check the affected item (the
    // v0.1 whole-item check; the per-item proof cache §5.3 keeps unaffected items
    // cheap). A re-parse failure after the splice is a real, reported error.
    write_file(file, &spliced)?;

    let root = address_root(addr);
    render_goal(file, Some(root))
}

/// Fill the hole named by `addr` (a `<fn>.?N` address) with `code`, re-check the
/// affected item, and return the new goal state render (REQ-6; the §5.1 fill loop).
/// `forge fill` is a specialization of `edit` whose address names a hole: it splices
/// the replacement source at the `?N` token's span (reusing the increment-(ii)
/// splice machinery), re-parses, and re-checks. The filled `code` may itself
/// contain new holes (`?1 ?2`), which the re-parse records and the new goal state
/// surfaces (the §5.1 dialogue's "fill ?0 … introducing ?1 ?2"). A non-hole address
/// (a `loop`/`inv`/`dec`/`fn`, an `edit` target rather than a `fill` target) is a
/// `ForgeError::Usage` (use `forge edit` for those); a bad/unresolvable hole address
/// is a structured error, not a panic (R-CODE-2).
pub fn fill_hole(file: &Path, addr: &str, code: &str) -> Result<String, ForgeError> {
    let src = read_file(file)?;
    let program = parse_program(file)?;

    // Resolve the address (bad address → structured AddressError, not a panic).
    let entry = address::resolve(&program, addr).map_err(address_usage)?;

    // `fill` targets a hole only; a non-hole address is the `edit` surface. Reject
    // it with an actionable message rather than silently splicing (the two verbs
    // have distinct contracts, REQ-3 vs REQ-6).
    if entry.kind != AddrKind::Hole {
        return Err(ForgeError::Usage(format!(
            "address `{addr}` is not a hole (it names a {:?} node); `forge fill` targets a `?N` \
             body hole — use `forge edit {addr} --replace <code>` to splice a non-hole node",
            entry.kind
        )));
    }

    // The hole's `?N` token span is the splice target (mirroring `edit`'s span walk).
    let span = span_of_address(&program, addr).ok_or_else(|| {
        ForgeError::Usage(format!(
            "hole address `{addr}` resolves but names no `?N` span (internal: the hole is recorded \
             on its fn but its span was not found)"
        ))
    })?;

    // Splice the fill code at the hole's `?N` position (the pure splice, R-CODE-5),
    // re-emit in place, then re-check the affected item. The new goal state may
    // surface new holes the filled code introduced (§5.1). A re-parse failure after
    // the splice (malformed fill code) is a reported error, not swallowed.
    let spliced = splice(&src, span, code);
    write_file(file, &spliced)?;

    let root = address_root(addr);
    render_goal(file, Some(root))
}

/// Render one item's goal state (REQ-2; §5.1). The `given` is the `req` clause
/// text; the `want` is the `ens` clause texts; then each obligation as discharged
/// or failed-with-witness; a clean cert renders `ALL GOALS DISCHARGED` + the level
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
fn span_of_address(program: &Program, addr: &str) -> Option<Span> {
    let entry_kind = address::resolve(program, addr).ok()?.kind;
    let mut segs = addr.split('.');
    let root = segs.next()?;

    let fn_item = program.items.iter().find_map(|i| match i {
        Item::Fn(f) if f.name == root => Some(f),
        _ => None,
    })?;

    match entry_kind {
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
    }
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

/// Read the source file (IO error → `ForgeError::Io`).
fn read_file(file: &Path) -> Result<String, ForgeError> {
    std::fs::read_to_string(file).map_err(|e| ForgeError::Io {
        path: file.display().to_string(),
        source: e,
    })
}

/// Write the source file back (IO error → `ForgeError::Io`).
fn write_file(file: &Path, contents: &str) -> Result<(), ForgeError> {
    std::fs::write(file, contents).map_err(|e| ForgeError::Io {
        path: file.display().to_string(),
        source: e,
    })
}

/// Parse `file` into a clean `Program` (a parse failure is a `ForgeError::Parse`,
/// surfaced, not swallowed). Re-parse of a known-good corpus file is
/// deterministic (R-CODE-5).
fn parse_program(file: &Path) -> Result<Program, ForgeError> {
    let src = read_file(file)?;
    let parsed = thermite_syntax::parse(&src);
    if !parsed.is_clean() {
        return Err(ForgeError::Parse(parsed.errors));
    }
    Ok(parsed.program)
}

/// The string form of a [`Level`] for the goal render.
fn level_str(level: Level) -> &'static str {
    match level {
        Level::L0 => "L0",
        Level::L1 => "L1",
        Level::L2 => "L2",
        Level::L3 => "L3",
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

    // REQ-2 / AC-2: a clean L3 cert renders ALL GOALS DISCHARGED + the level + the
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
        let span =
            span_of_address(&program, "binary_search.loop#1.inv#2").expect("inv#2 has a span");
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
        // from span_of_address (a clean miss, not a panic).
        let spec_program = parse_ok("spec fn m(n: u32) -> u32 dec n { n }");
        assert!(span_of_address(&spec_program, "m").is_none());
    }
}
