//! Thermite semantic addressing — stable, positional block addresses computed
//! over the AST (`binary_search.loop#1.inv#2`).
//!
//! Governing design: `.design/syntax/semantic-addressing.md`. Addresses are the
//! operands of `forge edit <addr>` and the keys of the per-item proof cache
//! (§5.3), so they must be STABLE under unrelated edits: the address of a block
//! is a function of its position WITHIN ITS ENCLOSING ITEM only (REQ-5).
//! `while` and `loop` share the `loop#N` namespace (REQ-2). Resolution is
//! bidirectional and never panics — a bad address yields a structured
//! `AddressError` (REQ-6). Blocker #26 is resolved by the oracle: 1-based source
//! order, all invariants counted (`inv#2` = `forall_below`, `inv#3` =
//! `forall_from`).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (address grammar) | SHIPPED | `Address` segments = function name + `loop#N`/`inv#M`/`dec`; `parse_address` + `Display`. |
//! | REQ-2 (loop numbering) | SHIPPED | `addresses_of` numbers loops 1-based in source order, `while`+`loop` shared. |
//! | REQ-3 (inv numbering) | SHIPPED | per-loop 1-based `inv#M` in source order; `tests/conformance.rs` asserts `inv#2`/`inv#3`. |
//! | REQ-4 (dec address) | SHIPPED | each loop's single `dec` is `<loop>.dec` (no ordinal). |
//! | REQ-5 (stability under unrelated edits) | SHIPPED | numbering reads only the enclosing item; `tests/conformance.rs` stability fixture. |
//! | REQ-6 (deterministic + bidirectional) | SHIPPED | `addresses_of` (node->addr) + `resolve` (addr->node); bad addr -> `AddressError`, no panic. |
//! | goal-repl REQ-4 (`<fn>.?N` hole address, #193) | SHIPPED | `addresses_of` emits a `AddrKind::Hole` entry `<fn>.?N` per `FnItem.holes` member (verbatim surface number, document order); `validate_segments` accepts a `?N` segment (a bare `?`/non-digit is `Malformed`). The operand of `forge fill <fn>.?N <code>`. Resolves through the same `resolve` path (a `<fn>.?9` for an absent hole → `NotFound`, never a panic). Consumer: `forge::goal_repl::fill_hole`. |

use crate::ast::{Block, Item, LoopNode, Program, Stmt};
use std::fmt;

/// A structured error from address resolution (semantic-addressing.md REQ-6).
/// Never a panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressError {
    /// The address string was not well-formed.
    Malformed(String),
    /// No item/block in the program matches the address.
    NotFound(String),
}

impl fmt::Display for AddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AddressError::Malformed(a) => write!(f, "malformed address `{a}`"),
            AddressError::NotFound(a) => write!(f, "no such address `{a}`"),
        }
    }
}

impl std::error::Error for AddressError {}

/// The kind of node an address points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddrKind {
    Fn,
    SpecFn,
    Loop,
    Inv,
    Dec,
    /// An open body HOLE `?N` (`.design/forge/goal-repl.md` REQ-4, #193). Addressed
    /// `<fn>.?N` where `N` is the hole's verbatim surface number. The operand of
    /// `forge fill <fn>.?N <code>`.
    Hole,
}

/// A computed address with the kind of node it names and, for `inv`/`dec`, the
/// verbatim source text the address resolves to (semantic-addressing.md AC-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressEntry {
    pub addr: String,
    pub kind: AddrKind,
    /// The surface keyword for a loop (`loop`/`while`), else `None`.
    pub surface_keyword: Option<&'static str>,
    /// The clause source text for `inv`/`dec`, else `None`.
    pub text: Option<String>,
}

/// Compute every valid address in `program`, in document order
/// (semantic-addressing.md REQ-1..REQ-4). Deterministic: same AST -> same list
/// (R-CODE-5).
pub fn addresses_of(program: &Program) -> Vec<AddressEntry> {
    let mut out = Vec::new();
    for item in &program.items {
        match item {
            Item::Fn(f) => {
                out.push(AddressEntry {
                    addr: f.name.clone(),
                    kind: AddrKind::Fn,
                    surface_keyword: None,
                    text: None,
                });
                // A boundary fn (ffi-boundary.md REQ-2) has `body: None` — no
                // Thermite body, so no addressable inner loops. An in-language fn
                // carries a body whose loops are numbered as before.
                if let Some(body) = &f.body {
                    collect_block_loops(&f.name, body, &mut out);
                }
                // Open body holes (`?N`) are addressed `<fn>.?N` by their verbatim
                // surface number (`.design/forge/goal-repl.md` REQ-4, #193), in
                // document order — the operand of `forge fill <fn>.?N <code>`. EMPTY
                // for every hole-free fn (the entire pre-#193 corpus), so this is a
                // purely additive set of addresses; a holed fn never certifies
                // (`forge check` short-circuits it), so these addresses exist only to
                // let `forge fill` name the hole to splice.
                for hole in &f.holes {
                    out.push(AddressEntry {
                        addr: format!("{}.?{}", f.name, hole.number),
                        kind: AddrKind::Hole,
                        surface_keyword: None,
                        text: None,
                    });
                }
            }
            Item::SpecFn(s) => {
                // A spec fn has no addressable inner blocks in v0.1 (its `dec`
                // is a spec-fn measure, not a loop dec — OQ-2).
                out.push(AddressEntry {
                    addr: s.name.clone(),
                    kind: AddrKind::SpecFn,
                    surface_keyword: None,
                    text: None,
                });
            }
            // A `struct`/`enum` type item (`.design/basis/01-adts.md` Stage 1a)
            // is NOT an addressable node: the addressing scheme
            // (`.design/syntax/semantic-addressing.md` REQ-1/REQ-2) roots only at
            // FUNCTION names and numbers their inner loops/`inv`/`dec` — a type
            // declaration has no loops, no contract clauses, hence no address.
            // This additive no-op arm keeps the same-crate exhaustive `match`
            // compiling; types gain no `forge edit` address.
            Item::Struct(_) | Item::Enum(_) => {}
        }
    }
    out
}

/// Walk a function body and address every loop (and its `inv`/`dec`) in source
/// order. The loop counter is scoped to the enclosing function (REQ-2/REQ-5).
fn collect_block_loops(fn_name: &str, body: &Block, out: &mut Vec<AddressEntry>) {
    let mut loop_index = 0usize;
    collect_in_block(fn_name, body, &mut loop_index, out);
}

fn collect_in_block(
    fn_name: &str,
    block: &Block,
    loop_index: &mut usize,
    out: &mut Vec<AddressEntry>,
) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(lp) => {
                *loop_index += 1;
                let loop_addr = format!("{fn_name}.loop#{loop_index}");
                emit_loop(&loop_addr, lp, out);
                // Nested loops (none in the corpus) continue the flat
                // function-level numbering (OQ-3).
                collect_in_block(fn_name, &lp.body, loop_index, out);
            }
            Stmt::If { then, else_, .. } => {
                collect_in_block(fn_name, then, loop_index, out);
                if let Some(eb) = else_ {
                    collect_in_block(fn_name, eb, loop_index, out);
                }
            }
            _ => {}
        }
    }
}

/// Emit the loop's own address plus its `inv#M` (1-based, source order) and
/// `dec` addresses (semantic-addressing.md REQ-3/REQ-4).
fn emit_loop(loop_addr: &str, lp: &LoopNode, out: &mut Vec<AddressEntry>) {
    out.push(AddressEntry {
        addr: loop_addr.to_string(),
        kind: AddrKind::Loop,
        surface_keyword: Some(lp.kind.surface_keyword()),
        text: None,
    });
    for (m, inv) in lp.invs.iter().enumerate() {
        out.push(AddressEntry {
            addr: format!("{loop_addr}.inv#{}", m + 1),
            kind: AddrKind::Inv,
            surface_keyword: None,
            text: Some(inv.text.clone()),
        });
    }
    out.push(AddressEntry {
        addr: format!("{loop_addr}.dec"),
        kind: AddrKind::Dec,
        surface_keyword: None,
        text: Some(lp.dec.text.clone()),
    });
}

/// Resolve an address string against `program`, returning the matching entry or
/// a structured error (semantic-addressing.md REQ-6). Never panics.
pub fn resolve(program: &Program, addr: &str) -> Result<AddressEntry, AddressError> {
    if addr.is_empty() {
        return Err(AddressError::Malformed(addr.to_string()));
    }
    // Validate segment shape before searching, so a malformed address is
    // distinguished from a well-formed but absent one.
    validate_segments(addr)?;
    addresses_of(program)
        .into_iter()
        .find(|e| e.addr == addr)
        .ok_or_else(|| AddressError::NotFound(addr.to_string()))
}

/// Check that every segment after the root is a well-formed `loop#N`/`inv#M`/
/// `dec` (REQ-1). The root segment (function name) is unconstrained here; an
/// unknown name surfaces as `NotFound` from `resolve`.
fn validate_segments(addr: &str) -> Result<(), AddressError> {
    let mut segs = addr.split('.');
    // Root segment must be a non-empty identifier.
    match segs.next() {
        Some(root) if !root.is_empty() && !root.contains('#') => {}
        _ => return Err(AddressError::Malformed(addr.to_string())),
    }
    for seg in segs {
        if seg == "dec" {
            continue;
        }
        // A hole segment `?N` (`.design/forge/goal-repl.md` REQ-4, #193): the `?`
        // prefix + a non-empty ASCII-digit run. `?` with no digits / non-digits is
        // Malformed (mirroring `loop#`/`inv#`).
        if let Some(n) = seg.strip_prefix('?') {
            if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
                return Err(AddressError::Malformed(addr.to_string()));
            }
            continue;
        }
        if let Some(n) = seg
            .strip_prefix("loop#")
            .or_else(|| seg.strip_prefix("inv#"))
        {
            if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
                return Err(AddressError::Malformed(addr.to_string()));
            }
            continue;
        }
        return Err(AddressError::Malformed(addr.to_string()));
    }
    Ok(())
}
