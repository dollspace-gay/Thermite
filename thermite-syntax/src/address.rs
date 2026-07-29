//! Thermite semantic addressing — stable, positional block addresses computed
//! over the AST (`binary_search.loop#1.inv#2`).
//!
//! Governing design: `.design/syntax/semantic-addressing.md`. Addresses are the
//! operands of `forge edit <addr>` and the keys of the per-item proof cache
//! (§5.3), so they must be stable under unrelated edits: the address of a block
//! is a function of its position within its enclosing item only (REQ-5).
//! `while` and `loop` share the `loop#N` namespace (REQ-2). Resolution is
//! bidirectional and never panics: a bad address yields a structured
//! `AddressError` (REQ-6). Blocker #26 is resolved by the oracle: 1-based source
//! order, all invariants counted (`inv#2` = `forall_below`, `inv#3` =
//! `forall_from`).
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=thermite-syntax-address-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-ADDRESS-DEC | shipped | `thermite-syntax/src/address.rs` | Semantic address dec segment |  |
//! | REQ-SYNTAX-ADDRESS-DETERMINISTIC-RESOLVE | shipped | `thermite-syntax/src/address.rs` | Semantic address bidirectional resolution |  |
//! | REQ-SYNTAX-ADDRESS-GRAMMAR | shipped | `thermite-syntax/src/address.rs` | Semantic address grammar |  |
//! | REQ-SYNTAX-ADDRESS-HOLES | shipped | `thermite-syntax/src/address.rs` | Semantic address holes |  |
//! | REQ-SYNTAX-ADDRESS-INV-NUMBERING | shipped | `thermite-syntax/src/address.rs` | Semantic address invariant numbering |  |
//! | REQ-SYNTAX-ADDRESS-LOOP-NUMBERING | shipped | `thermite-syntax/src/address.rs` | Semantic address loop numbering |  |
//! | REQ-SYNTAX-ADDRESS-STABILITY | shipped | `thermite-syntax/src/address.rs` | Semantic address stability |  |
//! <!-- /generated:reqs -->

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
    /// An open body hole `?N` (`.design/forge/goal-repl.md` REQ-4, #193). Addressed
    /// `<fn>.?N` where `N` is the hole's verbatim surface number. The operand of
    /// `forge fill <fn>.?N <code>`.
    Hole,
    /// A Stage-1 forge-tier item root or proof obligation
    /// (`.design/stage1-forge-tier.md` REQ-3): a `prop fn`/`lemma` name, a numbered
    /// `witness#N`, or a proof obligation `f.proof.ens#k`. The consumers (proof view
    /// 2e, lemma library 3) resolve these; here they are addressable + round-trip.
    Forge,
    /// An open proof hole `?pN` (`.design/stage1-forge-tier.md` REQ-3) inside a
    /// proof block. Addressed `<lemma>.proof.?pN` / `f.proof.ens#k.?pN`. Distinct
    /// from a body [`AddrKind::Hole`]: `forge fill` targeting a proof hole is the
    /// proof view (increment 2e, REQ-7), so for now this is addressable + round-trip
    /// only (the body-hole `forge fill` path rejects it as "not a body hole").
    ProofHole,
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
    // Witness blocks have no name (`.design/stage1-forge-tier.md` REQ-3), so they
    // are numbered `witness#N` (1-based, source order) like `loop#N`.
    let mut witness_index = 0usize;
    for item in &program.items {
        match item {
            Item::Fn(f) => {
                out.push(AddressEntry {
                    addr: f.name.clone(),
                    kind: AddrKind::Fn,
                    surface_keyword: None,
                    text: None,
                });
                // A boundary fn (ffi-boundary.md REQ-2) has `body: None`: no
                // Thermite body, so no addressable inner loops. An in-language fn
                // carries a body whose loops are numbered as before.
                if let Some(body) = &f.body {
                    collect_block_loops(&f.name, body, &mut out);
                }
                // Open body holes (`?N`) are addressed `<fn>.?N` by their verbatim
                // surface number (`.design/forge/goal-repl.md` REQ-4, #193), in
                // document order: the operand of `forge fill <fn>.?N <code>`. Empty
                // for every hole-free fn (the entire pre-#193 corpus), so this is an
                // additive set of addresses; a holed fn never certifies
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
            // is not an addressable node: the addressing scheme
            // (`.design/syntax/semantic-addressing.md` REQ-1/REQ-2) roots only at
            // function names and numbers their inner loops/`inv`/`dec`; a type
            // declaration has no loops, no contract clauses, hence no address.
            // This additive no-op arm keeps the same-crate exhaustive `match`
            // compiling; types gain no `forge edit` address.
            Item::Struct(_) | Item::Enum(_) => {}
            // Stage-1 forge-tier items (`.design/stage1-forge-tier.md` REQ-3): the
            // prop/lemma/proof/witness addressing, including the proof-block
            // addresses (`f.proof.ens#k`) and the `?pN` proof-hole form (AC-7).
            Item::Forge(forge) => collect_forge_addresses(forge, &mut witness_index, &mut out),
        }
    }
    out
}

/// Collect the addresses of a Stage-1 forge-tier item
/// (`.design/stage1-forge-tier.md` REQ-3): the item root, the proof-block
/// obligation addresses (`f.proof.ens#k`), and the `?pN` proof-hole addresses.
/// `witness_index` is the running 1-based witness counter (witnesses are anonymous,
/// so numbered `witness#N`).
fn collect_forge_addresses(
    forge: &crate::ast::ForgeItem,
    witness_index: &mut usize,
    out: &mut Vec<AddressEntry>,
) {
    use crate::ast::ForgeItem;
    match forge {
        ForgeItem::PropFn(p) => {
            // A prop fn is addressed by name, like a `spec fn` (its body is a
            // proposition with no addressable inner blocks / holes in v1).
            out.push(AddressEntry {
                addr: p.name.clone(),
                kind: AddrKind::Forge,
                surface_keyword: None,
                text: None,
            });
        }
        ForgeItem::Lemma(l) => {
            out.push(AddressEntry {
                addr: l.name.clone(),
                kind: AddrKind::Forge,
                surface_keyword: None,
                text: None,
            });
            // The lemma's proof block is a single block; its open proof holes are
            // addressed `<lemma>.proof.?pN`.
            let block_addr = format!("{}.proof", l.name);
            for hole in &l.proof.holes {
                out.push(AddressEntry {
                    addr: format!("{}.?p{}", block_addr, hole.number),
                    kind: AddrKind::ProofHole,
                    surface_keyword: None,
                    text: None,
                });
            }
        }
        ForgeItem::Proof(p) => {
            // Each obligation is addressed `f.proof.<clause>` (e.g. `f.proof.ens#k`);
            // its proof block's open holes are `f.proof.<clause>.?pN`.
            for ob in &p.obligations {
                let clause_addr = match ob.clause.index {
                    Some(k) => format!("{}.proof.{}#{}", p.target, ob.clause.keyword, k),
                    None => format!("{}.proof.{}", p.target, ob.clause.keyword),
                };
                out.push(AddressEntry {
                    addr: clause_addr.clone(),
                    kind: AddrKind::Forge,
                    surface_keyword: None,
                    text: None,
                });
                for hole in &ob.proof.holes {
                    out.push(AddressEntry {
                        addr: format!("{}.?p{}", clause_addr, hole.number),
                        kind: AddrKind::ProofHole,
                        surface_keyword: None,
                        text: None,
                    });
                }
            }
        }
        ForgeItem::Witness(_) => {
            *witness_index += 1;
            out.push(AddressEntry {
                addr: format!("witness#{witness_index}"),
                kind: AddrKind::Forge,
                surface_keyword: None,
                text: None,
            });
        }
    }
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
/// `dec` (REQ-1) or a forge-tier segment — `proof`, a clause family `ens`/`req`/
/// `inv` (optionally `#k`), or a proof hole `?pN` (`.design/stage1-forge-tier.md`
/// REQ-3). The root is a non-empty identifier (a fn/prop/lemma name) or the
/// anonymous-witness form `witness#N`; an unknown name surfaces as `NotFound` from
/// `resolve`.
fn validate_segments(addr: &str) -> Result<(), AddressError> {
    let all_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let mut segs = addr.split('.');
    // Root segment: a non-empty identifier, or the anonymous-witness `witness#N`
    // (the one `#`-bearing root, since witness blocks have no name).
    let root = match segs.next() {
        Some(r) if !r.is_empty() => r,
        _ => return Err(AddressError::Malformed(addr.to_string())),
    };
    if let Some((word, num)) = root.split_once('#') {
        if word != "witness" || !all_digits(num) {
            return Err(AddressError::Malformed(addr.to_string()));
        }
    }
    for seg in segs {
        // Bare keyword segments: the loop `dec`, the proof-block `proof`, and an
        // unindexed clause family (`f.proof.req`).
        if matches!(seg, "dec" | "proof" | "ens" | "req" | "inv") {
            continue;
        }
        // A hole segment — a body hole `?N` (#193) or a proof hole `?pN`
        // (forge-tier REQ-3): the `?` prefix, an optional `p`, then a non-empty
        // ASCII-digit run. `?`/`?p` with no digits / non-digits is Malformed.
        if let Some(rest) = seg.strip_prefix('?') {
            let digits = rest.strip_prefix('p').unwrap_or(rest);
            if !all_digits(digits) {
                return Err(AddressError::Malformed(addr.to_string()));
            }
            continue;
        }
        // A numbered segment `<word>#<digits>`: a loop/inv ordinal (#193 addressing)
        // or a forge-tier clause ordinal `ens#k`/`req#k`/`inv#k` (REQ-3).
        if let Some((word, num)) = seg.split_once('#') {
            if !matches!(word, "loop" | "inv" | "ens" | "req") || !all_digits(num) {
                return Err(AddressError::Malformed(addr.to_string()));
            }
            continue;
        }
        return Err(AddressError::Malformed(addr.to_string()));
    }
    Ok(())
}
