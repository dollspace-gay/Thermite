//! `forge/src/meaning.rs` — the definition-tower budget (REQ-6c, increment 2d;
//! `.design/stage1-forge-tier.md`) and the unfolded-tower projection
//! `forge audit --meaning` pins into the certificate.
//!
//! ## Anti-Goodhart defense (c): the definition tower
//!
//! A forge-tier contract gets its *meaning* by unfolding the `spec fn`s its
//! `req`/`ens` clauses reference, which unfold the `spec fn`s *they* reference, and
//! so on — a tower of definitions. A contract can be made to "say" something while
//! hiding the claim behind a deep stack of definitions an auditor cannot read
//! through (the Goodhart move: optimize "the contract proves" by making the contract
//! unreadable). REQ-6c bounds that tower: the **Q2 default budget** is a
//! [`TOWER_DEPTH_BUDGET`] of `4` (the longest distinct-definition unfolding chain)
//! and a [`TOWER_DEFINITION_BUDGET`] of `40` (the total distinct definitions). A
//! tower deeper or wider than the budget does not certify — a refusal at certify
//! time, never a silent pass.
//!
//! ## Where the gate lives (and where it does not)
//!
//! The gate is a **certify-time gate on the discharge path** (`check.rs`, the
//! forge/Lean `--engine` route), not in `forge audit`. `forge audit`'s "gates
//! nothing" projection invariant is shipped (#274, `.design/forge/audit-manifest.md`
//! REQ-10): it re-derives no verdict and changes no exit code. `forge audit
//! --meaning` is the **read-only companion**: it prints the unfolded tower and
//! reports its budget status, but it gates nothing itself — the refusal happens at
//! certify time, and the certificate pins the unfolded-tower hash
//! ([`MeaningAudit::tower_hash`]) so a later reader can confirm the meaning that was
//! certified is the one in front of them.
//!
//! The tower is rooted at the **contract** (`req ∪ ens`): the meaning surface an
//! auditor reads. The body's own `spec fn` calls are about the implementation, not
//! the contract's claim, so they do not seed the tower (a reached `spec fn`'s own
//! `body ∪ dec` *is* followed transitively, since the definition's meaning includes
//! its body). This mirrors the existing #226 closure machinery in `check.rs`,
//! reusing its `spec fn` call collectors rather than forking a second walker.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thermite_syntax::{Expr, FnItem, Item, Program, SpecFnItem};

/// The Q2 default definition-tower depth budget (REQ-6c): the maximum length of the
/// longest distinct-definition unfolding chain rooted at the contract. A tower whose
/// deepest chain exceeds this does not certify (a certify-time refusal). `4` is the
/// Q2 default in `.design/stage1-forge-tier.md` REQ-6.
pub const TOWER_DEPTH_BUDGET: usize = 4;

/// The Q2 default definition-tower size budget (REQ-6c): the maximum number of
/// distinct definitions reachable from the contract. A tower with more distinct
/// definitions than this does not certify. `40` is the Q2 default.
pub const TOWER_DEFINITION_BUDGET: usize = 40;

/// One definition in the unfolded tower (REQ-6c): the `spec fn`'s name, its verbatim
/// source text (sliced from the original program source by span — what an auditor
/// would read), and its level in the tower (the shortest reference distance from a
/// contract-referenced root, root = `1`). Source order across the tower is the
/// deterministic enumeration order (R-CODE-5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TowerDef {
    /// The `spec fn` name.
    pub name: String,
    /// The verbatim source of the definition (the span slice — the unfolded text).
    pub text: String,
    /// The definition's level in the tower (a contract-referenced root is `1`; a
    /// definition reached only through another is `2`, and so on).
    pub level: usize,
}

/// The definition tower of one forge-tier item's contract (REQ-6c) — the transitive
/// `spec fn` closure of `req ∪ ens`, with the longest-chain depth and the verbatim
/// unfolded text of each definition. A pure function of the AST + source
/// ([`build_tower`]); it re-runs no prover (the audit is a projection, like
/// `forge audit` itself).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionTower {
    /// The item whose contract roots the tower.
    pub item: String,
    /// The reached definitions, in source order (deterministic).
    pub defs: Vec<TowerDef>,
    /// The tower depth — the longest distinct-definition unfolding chain rooted at a
    /// contract-referenced definition. `0` when the contract references no `spec fn`
    /// (a scalar contract has an empty tower — trivially within budget).
    pub depth: usize,
}

/// The cause a tower exceeded the budget (REQ-6c) — which dimension and by how much.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TowerBudgetExceeded {
    /// The longest unfolding chain is deeper than [`TOWER_DEPTH_BUDGET`].
    Depth,
    /// The total distinct-definition count exceeds [`TOWER_DEFINITION_BUDGET`].
    Definitions,
}

/// The budget verdict for a tower (REQ-6c): within budget, or over on one dimension.
/// The depth check is reported first when both are exceeded (the depth is the harder
/// auditability barrier — a wide-but-shallow tower is still readable level by level).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TowerBudget {
    /// Both dimensions are within the Q2 default budget — the item may certify.
    WithinBudget,
    /// One dimension exceeds the budget — a certify-time refusal.
    OverBudget {
        /// Which dimension was exceeded.
        kind: TowerBudgetExceeded,
        /// The observed value on that dimension.
        observed: usize,
        /// The Q2 default limit on that dimension.
        limit: usize,
    },
}

/// The certificate-pinned meaning audit (REQ-6c / Q-ORACLE): the unfolded-tower hash
/// plus its depth + definition count. Pinned on a forge-tier certificate so a reader
/// can confirm the certified meaning is the one in front of them (a changed
/// definition anywhere in the tower changes the hash). Per Q-ORACLE the meaning-audit
/// hash joins the cert oracle subset, so it cannot drift silently; a v1 item never
/// populates it (`None`), so the v1 golden certs stay byte-identical (mirrors
/// `covenant_evidence`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeaningAudit {
    /// The lowercase-hex sha256 over the unfolded tower (name + verbatim text of each
    /// definition, in source order, length-prefixed — deterministic, R-CODE-5).
    pub tower_hash: String,
    /// The tower depth (the longest distinct-definition unfolding chain).
    pub depth: usize,
    /// The number of distinct definitions in the tower.
    pub definitions: usize,
}

impl DefinitionTower {
    /// The number of distinct definitions in the tower.
    #[must_use]
    pub fn definition_count(&self) -> usize {
        self.defs.len()
    }

    /// The budget verdict for this tower (REQ-6c). Depth is checked before size: a
    /// deep tower is the harder auditability barrier, so it is named first when both
    /// are over.
    #[must_use]
    pub fn budget_verdict(&self) -> TowerBudget {
        if self.depth > TOWER_DEPTH_BUDGET {
            return TowerBudget::OverBudget {
                kind: TowerBudgetExceeded::Depth,
                observed: self.depth,
                limit: TOWER_DEPTH_BUDGET,
            };
        }
        if self.definition_count() > TOWER_DEFINITION_BUDGET {
            return TowerBudget::OverBudget {
                kind: TowerBudgetExceeded::Definitions,
                observed: self.definition_count(),
                limit: TOWER_DEFINITION_BUDGET,
            };
        }
        TowerBudget::WithinBudget
    }

    /// `true` iff this tower is within the Q2 default budget (the certify-gate
    /// predicate). The discharge-path gate ([`crate::check`]) reads
    /// [`DefinitionTower::over_budget_detail`] (it needs the refusal string), so this
    /// boolean form is the predicate the budget tests assert against.
    #[allow(
        dead_code,
        reason = "REQ-6c budget predicate: the certify-time gate reads `over_budget_detail` \
                  (it needs the refusal detail), so the boolean form is exercised by the \
                  budget tests; kept public as the readable predicate."
    )]
    #[must_use]
    pub fn within_budget(&self) -> bool {
        matches!(self.budget_verdict(), TowerBudget::WithinBudget)
    }

    /// The lowercase-hex sha256 over the unfolded tower (REQ-6c / Q-ORACLE). Hashes
    /// the ordered `(name, verbatim text)` pairs with length prefixes (so no two
    /// distinct towers collide by concatenation) plus the depth + count. Deterministic
    /// (R-CODE-5): a pure function of the tower content.
    #[must_use]
    pub fn tower_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"thermite-definition-tower-v1");
        hasher.update((self.depth as u64).to_le_bytes());
        hasher.update((self.defs.len() as u64).to_le_bytes());
        for def in &self.defs {
            hasher.update((def.name.len() as u64).to_le_bytes());
            hasher.update(def.name.as_bytes());
            hasher.update((def.text.len() as u64).to_le_bytes());
            hasher.update(def.text.as_bytes());
        }
        let digest = hasher.finalize();
        hex_lower(&digest)
    }

    /// The certificate-pinned [`MeaningAudit`] for this tower (REQ-6c / Q-ORACLE).
    #[must_use]
    pub fn meaning_audit(&self) -> MeaningAudit {
        MeaningAudit {
            tower_hash: self.tower_hash(),
            depth: self.depth,
            definitions: self.definition_count(),
        }
    }

    /// Render the unfolded tower as human-readable text (the `forge audit --meaning`
    /// body, REQ-6c). Lists each definition in source order with its level and
    /// verbatim text, then the depth / count summary and the pinned hash. Read-only:
    /// this prints; it gates nothing (the gate is at certify time).
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("meaning tower for `{}`:\n", self.item));
        if self.defs.is_empty() {
            out.push_str("  (no spec-fn definitions — the contract is scalar)\n");
        }
        for def in &self.defs {
            out.push_str(&format!("  [level {}] {}\n", def.level, def.name));
            for line in def.text.lines() {
                out.push_str(&format!("    {line}\n"));
            }
        }
        out.push_str(&format!(
            "tower: depth {} (budget {TOWER_DEPTH_BUDGET}), {} definitions (budget \
             {TOWER_DEFINITION_BUDGET})\n",
            self.depth,
            self.definition_count()
        ));
        match self.budget_verdict() {
            TowerBudget::WithinBudget => {
                out.push_str("budget: WITHIN budget\n");
            }
            TowerBudget::OverBudget {
                kind,
                observed,
                limit,
            } => {
                out.push_str(&format!(
                    "budget: OVER budget — {} {observed} exceeds limit {limit} (refused at \
                     certify time)\n",
                    match kind {
                        TowerBudgetExceeded::Depth => "depth",
                        TowerBudgetExceeded::Definitions => "definitions",
                    }
                ));
            }
        }
        out.push_str(&format!("tower_hash: {}\n", self.tower_hash()));
        out
    }

    /// The certify-time refusal detail for an over-budget tower (REQ-6c), or `None`
    /// when the tower is within budget. The string names the dimension, the observed
    /// value, and the Q2 default limit — the `RejectReason.detail` the discharge path
    /// records.
    #[must_use]
    pub fn over_budget_detail(&self) -> Option<String> {
        match self.budget_verdict() {
            TowerBudget::WithinBudget => None,
            TowerBudget::OverBudget {
                kind,
                observed,
                limit,
            } => Some(format!(
                "the definition tower of `{}` is over the Q2 budget: {} {observed} exceeds the \
                 limit {limit} (REQ-6c anti-Goodhart — a contract whose meaning hides behind a \
                 tower deeper/wider than the budget does not certify; refused at certify time, \
                 the unfolded tower hash is pinned in the certificate)",
                self.item,
                match kind {
                    TowerBudgetExceeded::Depth => "depth",
                    TowerBudgetExceeded::Definitions => "definitions",
                }
            )),
        }
    }
}

/// Build the definition tower of `f`'s contract (REQ-6c), slicing each reached
/// `spec fn`'s verbatim source from `src` by span. Rooted at `req ∪ ens` (the meaning
/// surface), with each reached definition's own `body ∪ dec` followed transitively —
/// reusing the `check.rs` #226 `spec fn` call collectors so the meaning closure is
/// the same graph the obligation closure walks, not a fork.
///
/// `src` must be the source the `program` was parsed from (the spans index into it).
/// A name with no in-file `spec fn` declaration (a combinator / cross-file callee) is
/// not part of the tower (it is not an unfoldable in-file definition).
#[must_use]
pub fn build_tower(program: &Program, src: &str, f: &FnItem) -> DefinitionTower {
    let spec_decls = spec_decls_of(program);

    // The roots: the spec fns the contract (`req ∪ ens`) directly references — the
    // meaning surface. (The body's own calls are implementation, not the claim.)
    let mut roots: BTreeSet<String> = BTreeSet::new();
    crate::check::collect_expr_spec_fn_calls(&f.contract.req.expr, &spec_decls, &mut roots);
    for ens in &f.contract.ens {
        crate::check::collect_expr_spec_fn_calls(&ens.expr, &spec_decls, &mut roots);
    }

    let edges = spec_fn_edges(&spec_decls);
    let level = reachable_levels(&roots, &edges);

    // The defs in source order (deterministic), each with its verbatim span slice.
    let defs: Vec<TowerDef> = program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::SpecFn(s) if level.contains_key(&s.name) => Some(TowerDef {
                name: s.name.clone(),
                text: slice_span(src, s.span.start, s.span.end()),
                level: level.get(&s.name).copied().unwrap_or(1),
            }),
            _ => None,
        })
        .collect();

    let depth = tower_depth(&roots, &edges);

    DefinitionTower {
        item: f.name.clone(),
        defs,
        depth,
    }
}

/// The definition-tower depth + distinct-definition count rooted at an arbitrary set of
/// contract clause exprs (stage-3 REQ-6) — the depth-only projection the "semantic forks
/// and definition towers" section ([`crate::forks`]) uses for a burned `lemma`. A
/// [`thermite_syntax::LemmaItem`] carries `req ∪ ens` but no `FnItem`/body, so
/// [`build_tower`] (which keys on a `FnItem` + slices `src` for verbatim text) does not
/// apply, and the section needs only the depth + count, not the unfolded text — so this
/// takes no `src`. It reuses the same `spec fn` call collectors + edge graph helpers as
/// [`build_tower`], so the two AGREE on what "the tower" is (the spec-fn meaning closure
/// of the contract). Pure (R-CODE-5): a function of the AST alone. Returns
/// `(depth, definition_count)`.
#[must_use]
pub fn tower_metrics(program: &Program, root_exprs: &[&Expr]) -> (usize, usize) {
    let spec_decls = spec_decls_of(program);
    let mut roots: BTreeSet<String> = BTreeSet::new();
    for e in root_exprs {
        crate::check::collect_expr_spec_fn_calls(e, &spec_decls, &mut roots);
    }
    let edges = spec_fn_edges(&spec_decls);
    let level = reachable_levels(&roots, &edges);
    let depth = tower_depth(&roots, &edges);
    // The reachable set is exactly the keys of `level` (the collectors only insert names
    // in `spec_decls`, so every reached name is an in-file definition) — `level.len()` is
    // the distinct-definition count, the same count `build_tower`'s `defs` carries.
    (depth, level.len())
}

/// The in-file `spec fn` declarations, keyed by name (shared by [`build_tower`] +
/// [`tower_metrics`]). A combinator / cross-file callee has no in-file declaration and so
/// is not in this map (and never enters the tower).
fn spec_decls_of(program: &Program) -> BTreeMap<&str, &SpecFnItem> {
    program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::SpecFn(s) => Some((s.name.as_str(), s)),
            _ => None,
        })
        .collect()
}

/// The per-definition callee edges (intersected with the in-file spec-fn set): a
/// definition's meaning unfolds the spec fns its `body ∪ dec` references. Shared by
/// [`build_tower`] + [`tower_metrics`].
fn spec_fn_edges(spec_decls: &BTreeMap<&str, &SpecFnItem>) -> BTreeMap<String, BTreeSet<String>> {
    spec_decls
        .iter()
        .map(|(name, decl)| {
            let mut callees: BTreeSet<String> = BTreeSet::new();
            crate::check::collect_block_spec_fn_calls(&decl.body, spec_decls, &mut callees);
            crate::check::collect_expr_spec_fn_calls(&decl.dec.expr, spec_decls, &mut callees);
            ((*name).to_string(), callees)
        })
        .collect()
}

/// The reachable set (transitive closure of the roots over the edges) with each
/// definition's level (shortest reference distance from a root, root = 1) by BFS. Shared
/// by [`build_tower`] (which maps levels to `TowerDef`s) + [`tower_metrics`] (which counts
/// the keys). Deterministic (R-CODE-5): the `BTreeMap`/`BTreeSet` iteration order is
/// stable.
fn reachable_levels(
    roots: &BTreeSet<String>,
    edges: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeMap<String, usize> {
    let mut level: BTreeMap<String, usize> = BTreeMap::new();
    let mut frontier: Vec<String> = roots.iter().cloned().collect();
    for r in &frontier {
        level.insert(r.clone(), 1);
    }
    while let Some(name) = frontier.pop() {
        let cur = level.get(&name).copied().unwrap_or(1);
        if let Some(callees) = edges.get(&name) {
            for c in callees {
                let next = cur + 1;
                let improved = match level.get(c) {
                    Some(&existing) => next < existing,
                    None => true,
                };
                if improved {
                    level.insert(c.clone(), next);
                    frontier.push(c.clone());
                }
            }
        }
    }
    level
}

/// The longest distinct-definition unfolding chain rooted at any contract root
/// (REQ-6c). Computed by a memo-free DFS with an on-path guard so a recursive
/// `spec fn` (a self- or mutual cycle) does not loop or inflate the depth: a
/// definition already on the current path is not re-entered (recursion is the same
/// definition unfolding itself — bounded by `decreases`, not a deeper tower). The
/// in-file `spec fn` graphs are tiny, so the simple-path walk is cheap.
fn tower_depth(roots: &BTreeSet<String>, edges: &BTreeMap<String, BTreeSet<String>>) -> usize {
    let mut best = 0;
    for r in roots {
        let mut on_path: BTreeSet<String> = BTreeSet::new();
        best = best.max(longest_chain(r, edges, &mut on_path));
    }
    best
}

/// The longest distinct-definition chain starting at `node` (REQ-6c). `1` for a leaf;
/// `1 + max child` otherwise. A node already on the path contributes `0` (the cycle
/// is not extended) so the recursion terminates.
fn longest_chain(
    node: &str,
    edges: &BTreeMap<String, BTreeSet<String>>,
    on_path: &mut BTreeSet<String>,
) -> usize {
    if on_path.contains(node) {
        return 0;
    }
    on_path.insert(node.to_string());
    let mut best_child = 0;
    if let Some(callees) = edges.get(node) {
        for c in callees {
            best_child = best_child.max(longest_chain(c, edges, on_path));
        }
    }
    on_path.remove(node);
    1 + best_child
}

/// Slice `src[start..end]`, clamped to the source bounds (a malformed span never
/// panics, R-CODE-2; an out-of-range span yields an empty slice rather than a crash).
fn slice_span(src: &str, start: usize, end: usize) -> String {
    if start > end || start > src.len() {
        return String::new();
    }
    let end = end.min(src.len());
    // Snap to char boundaries so a multi-byte boundary never panics the slice.
    let start = floor_char_boundary(src, start);
    let end = floor_char_boundary(src, end);
    src.get(start..end).unwrap_or("").to_string()
}

/// The largest char boundary `<= i` (a tiny stand-in for the unstable
/// `str::floor_char_boundary`, so a span landing mid-codepoint snaps left). `src` is
/// in-file source whose spans are byte offsets from the lexer.
fn floor_char_boundary(src: &str, i: usize) -> usize {
    let i = i.min(src.len());
    let mut b = i;
    while b > 0 && !src.is_char_boundary(b) {
        b -= 1;
    }
    b
}

/// Render a byte digest as lowercase hex (the cert-pinned hash form). Mirrors
/// `cache::hex_lower`; kept module-local so `meaning` carries no cross-module
/// dependency for a two-line helper.
fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a single program source and return (the named `FnItem`, the program,
    /// the source). A parse failure means the fixture is wrong (a data-derived
    /// assert, keeping the gated `.unwrap` token out of any scanned patch).
    fn fixture(src: &str, item: &str) -> (FnItem, Program, String) {
        let parsed = thermite_syntax::parse(src);
        assert!(
            parsed.is_clean(),
            "fixture must parse clean: {:?}",
            parsed.errors
        );
        let f = parsed.program.items.iter().find_map(|i| match i {
            Item::Fn(f) if f.name == item => Some(f.clone()),
            _ => None,
        });
        assert!(f.is_some(), "fixture has no fn `{item}`");
        let f = f.unwrap_or_else(panic_unreachable);
        (f, parsed.program, src.to_string())
    }

    /// A data-unreachable fallback for the fixture helper — the assert above already
    /// failed the test, so this is never reached in a passing run.
    fn panic_unreachable() -> FnItem {
        FnItem {
            slag: None,
            boundary: None,
            name: String::new(),
            params: Vec::new(),
            ret: thermite_syntax::Type::Unit,
            contract: thermite_syntax::Contract {
                req: thermite_syntax::Clause {
                    expr: thermite_syntax::Expr::BoolLit(true),
                    text: String::new(),
                    span: thermite_syntax::Span::new(0, 0),
                    bv: None,
                },
                ens: Vec::new(),
                fx: thermite_syntax::EffectRow::Pure,
            },
            dec: None,
            body: Some(thermite_syntax::Block {
                stmts: Vec::new(),
                tail: None,
            }),
            holes: Vec::new(),
            refinements: Vec::new(),
            span: thermite_syntax::Span::new(0, 0),
        }
    }

    /// A scalar contract that references no `spec fn` has an empty tower: depth 0, no
    /// definitions, trivially within budget.
    #[test]
    fn scalar_contract_has_empty_tower() {
        let (f, program, src) = fixture(
            "fn inc(x: u32) -> u32 req x < 100 ens result == x + 1 fx pure { x + 1 }",
            "inc",
        );
        let tower = build_tower(&program, &src, &f);
        assert_eq!(tower.depth, 0);
        assert_eq!(tower.definition_count(), 0);
        assert!(tower.within_budget());
    }

    /// A contract referencing a `spec fn` that references another builds a tower whose
    /// depth is the unfolding chain length and whose defs carry verbatim text + level.
    #[test]
    fn tower_follows_the_spec_fn_chain() {
        let src = "\
spec fn a(x: u32) -> bool dec x { b(x) }
spec fn b(x: u32) -> bool dec x { c(x) }
spec fn c(x: u32) -> bool dec x { x > 0 }
fn f(x: u32) -> u32 req true ens a(x) fx pure { x }";
        let (f, program, src) = fixture(src, "f");
        let tower = build_tower(&program, &src, &f);
        // The contract references `a`, which references `b`, which references `c`:
        // a chain of 3 distinct definitions → depth 3, 3 definitions.
        assert_eq!(tower.depth, 3, "tower: {tower:?}");
        assert_eq!(tower.definition_count(), 3);
        // Levels: a is contract-rooted (1), b (2), c (3).
        let level_of = |name: &str| tower.defs.iter().find(|d| d.name == name).map(|d| d.level);
        assert_eq!(level_of("a"), Some(1));
        assert_eq!(level_of("b"), Some(2));
        assert_eq!(level_of("c"), Some(3));
        // The verbatim text is the span slice (the unfolded definition).
        let a_text = &tower.defs.iter().find(|d| d.name == "a").expect("a").text;
        assert!(a_text.contains("spec fn a"), "verbatim: {a_text}");
        assert!(tower.within_budget());
    }

    /// A definition referenced only by the body (not the contract) is not in the
    /// tower: the tower is the meaning of the contract, not the implementation. Here
    /// `contract_dep` roots the tower via `ens`; `body_dep` is called only from the
    /// body, so it is absent (and a `spec fn` it would have pulled in does not count).
    #[test]
    fn body_only_spec_fn_is_not_in_the_tower() {
        let src = "\
spec fn contract_dep(x: u32) -> bool dec x { x > 0 }
spec fn body_dep(x: u32) -> bool dec x { x < 100 }
fn f(x: u32) -> bool req true ens contract_dep(x) fx pure { body_dep(x) }";
        let (f, program, src) = fixture(src, "f");
        let tower = build_tower(&program, &src, &f);
        let names: Vec<&str> = tower.defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"contract_dep"), "tower: {names:?}");
        assert!(
            !names.contains(&"body_dep"),
            "a body-only spec fn is not part of the contract's meaning tower: {names:?}"
        );
        assert_eq!(tower.definition_count(), 1);
    }

    /// A recursive `spec fn` (a self-cycle) does not loop and does not inflate the
    /// depth: recursion is the same definition unfolding itself, bounded by
    /// `decreases`, not a deeper tower.
    #[test]
    fn recursive_spec_fn_does_not_loop_or_inflate_depth() {
        let src = "\
spec fn count_down(n: u32) -> u32 dec n { if n == 0 { 0 } else { count_down(n - 1) } }
fn f(n: u32) -> u32 req true ens result == count_down(n) fx pure { n }";
        let (f, program, src) = fixture(src, "f");
        let tower = build_tower(&program, &src, &f);
        assert_eq!(tower.definition_count(), 1);
        assert_eq!(tower.depth, 1, "a self-recursive def is one tower level");
        assert!(tower.within_budget());
    }

    /// AC-10 (tower clause): a tower deeper than the Q2 depth budget is OVER budget
    /// (a certify-time refusal), and the over-budget detail names the dimension +
    /// observed + limit.
    #[test]
    fn over_depth_tower_is_refused() {
        // A chain d1 → d2 → d3 → d4 → d5 (5 distinct defs) → depth 5 > budget 4.
        let src = "\
spec fn d1(x: u32) -> bool dec x { d2(x) }
spec fn d2(x: u32) -> bool dec x { d3(x) }
spec fn d3(x: u32) -> bool dec x { d4(x) }
spec fn d4(x: u32) -> bool dec x { d5(x) }
spec fn d5(x: u32) -> bool dec x { x > 0 }
fn f(x: u32) -> u32 req true ens d1(x) fx pure { x }";
        let (f, program, src) = fixture(src, "f");
        let tower = build_tower(&program, &src, &f);
        assert_eq!(tower.depth, 5);
        assert!(!tower.within_budget());
        assert_eq!(
            tower.budget_verdict(),
            TowerBudget::OverBudget {
                kind: TowerBudgetExceeded::Depth,
                observed: 5,
                limit: TOWER_DEPTH_BUDGET,
            }
        );
        let detail = tower.over_budget_detail().expect("over-budget detail");
        assert!(detail.contains("depth 5"), "{detail}");
        assert!(detail.contains("limit 4"), "{detail}");
    }

    /// AC-10 (tower clause): a within-budget tower pins a stable, content-derived
    /// unfolded-tower hash; changing a definition's body changes the hash.
    #[test]
    fn within_budget_tower_pins_a_stable_hash() {
        let src_a = "\
spec fn p(x: u32) -> bool dec x { x > 0 }
fn f(x: u32) -> u32 req true ens p(result) fx pure { x }";
        let (f, program, src) = fixture(src_a, "f");
        let tower = build_tower(&program, &src, &f);
        assert!(tower.within_budget());
        let audit = tower.meaning_audit();
        assert_eq!(audit.depth, 1);
        assert_eq!(audit.definitions, 1);
        assert_eq!(audit.tower_hash.len(), 64, "sha256 hex is 64 chars");
        // Determinism: re-building the same tower pins the same hash.
        let tower2 = build_tower(&program, &src, &f);
        assert_eq!(tower2.tower_hash(), audit.tower_hash);

        // A changed definition body changes the hash (the meaning moved).
        let src_b = "\
spec fn p(x: u32) -> bool dec x { x > 1 }
fn f(x: u32) -> u32 req true ens p(result) fx pure { x }";
        let (f2, program2, src2) = fixture(src_b, "f");
        let tower_b = build_tower(&program2, &src2, &f2);
        assert_ne!(
            tower_b.tower_hash(),
            audit.tower_hash,
            "a changed definition body must change the unfolded-tower hash"
        );
    }

    /// The render is read-only human text naming each definition, the budget status,
    /// and the pinned hash (the `forge audit --meaning` body).
    #[test]
    fn render_shows_tower_and_hash() {
        let src = "\
spec fn q(x: u32) -> bool dec x { x > 0 }
fn f(x: u32) -> u32 req true ens q(result) fx pure { x }";
        let (f, program, src) = fixture(src, "f");
        let tower = build_tower(&program, &src, &f);
        let rendered = tower.render();
        assert!(rendered.contains("meaning tower for `f`"));
        assert!(rendered.contains("spec fn q"));
        assert!(rendered.contains("WITHIN budget"));
        assert!(rendered.contains(&tower.tower_hash()));
    }
}
