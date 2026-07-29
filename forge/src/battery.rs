//! `forge/src/battery.rs` — the frozen forge-tier proof battery (REQ-5 / AC-9;
//! `.design/stage1-forge-tier.md`, increment 2c).
//!
//! This is the logic the foundation (#20) + the 2a surface (#29) set up for. 2a parsed
//! `proof { … }` / `lemma … proof { … }` blocks verbatim (their tactic content captured
//! as a raw [`thermite_syntax::ast::ProofBlock::text`], tactic parsing explicitly
//! deferred to this increment). Here we CONSUME that text:
//!
//! 1. The frozen [`REGISTRY`](static@FROZEN_TACTICS) — a single static, auditable,
//!    byte-deterministic source of truth modeled on the combinator registry
//!    (`thermite-spec/src/combinators.rs`'s static `REGISTRY`): a closed tactic
//!    allowlist + a closed simp-lemma set, pinned against the hand-derived oracle at
//!    `conformance/battery/registry.json` (the combinator-oracle precedent, R-CHAR-3).
//! 2. [`scan_citations`] parses a verbatim proof block into the tactic heads + the
//!    `simp [ … ]` lemma citations it makes.
//! 3. [`enforce`] is the elaboration-time gate: a proof citing an unlisted tactic OR an
//!    unlisted simp lemma is REFUSED — a hard [`BatteryViolation`] naming the offending
//!    tactic/lemma (R-BAT-1, AC-9), never warned. `check.rs` runs it over every
//!    forge-tier proof block before the item is admitted (the proof-tier mirror of the
//!    `thermite_spec::validate` cage gate).
//! 4. [`stuck_from_lake_output`] is the battery's companion on the Lean discharge path
//!    ([`crate::verdict::cert_verdict_for_lean`]): a proof that ELABORATES but leaves a
//!    residual goal is [`CertVerdict::Stuck`](crate::verdict::CertVerdict::Stuck) — the
//!    residual goal(s) + the "missing simp bridge" heuristic (RFC-1 §8) — never silently
//!    `Proved` and never mis-classed as a solver `Timeout`.
//!
//! ## Why the battery is frozen to these
//!
//! The tactic allowlist is the REQ-5 list verbatim. The simp set is the exact
//! `simp only [ … ]` list the generated auto battery emits
//! (`lean_export.rs::auto_tactic_battery`) — the single auditable record of what the
//! frozen battery knows how to rewrite. A citation outside it is an unlisted simp lemma
//! (refused); a residual goal mentioning a symbol the set does not normalize is the
//! missing-bridge Stuck hint. The merge example's `melems_cons` (the RFC-1 §8 transcript)
//! is exactly such an out-of-set bridge.

use serde::{Deserialize, Serialize};

/// One frozen battery entry (REQ-5): a tactic name or a simp-lemma name plus its
/// provenance. Plain `Copy` static-string struct, the combinator-registry shape, so the
/// table is `const`-derived and byte-deterministic (R-CODE-5). The conformance test pins
/// the table field-for-field against `conformance/battery/registry.json` (AC-9 — "the
/// battery registry file exists").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryEntry {
    /// The canonical name (a tactic head, or a fully-qualified simp lemma).
    pub name: &'static str,
}

/// The frozen tactic allowlist (REQ-5) — the closed set an author `proof`/`lemma` block
/// may cite. Adding/removing/changing an entry is a design-doc amendment, not a
/// code-local choice (the combinator-registry discipline). Pinned against the oracle's
/// `tactics` array. Order mirrors the spec list; lookup is by name, so order is
/// immaterial.
static FROZEN_TACTICS: [BatteryEntry; 9] = [
    BatteryEntry { name: "omega" },
    BatteryEntry { name: "simp" },
    BatteryEntry { name: "nlinarith" },
    BatteryEntry { name: "induction" },
    BatteryEntry { name: "decide" },
    BatteryEntry { name: "calc" },
    BatteryEntry { name: "exact" },
    BatteryEntry { name: "from" },
    BatteryEntry { name: "push_neg" },
];

/// The frozen simp-lemma set (REQ-5) — the closed set a `simp [ … ]` citation inside a
/// frozen-battery proof may name. These are the exact `simp only [ … ]` lemmas the
/// generated auto battery emits (`lean_export.rs::auto_tactic_battery`): the single
/// auditable source of what the frozen battery knows how to rewrite. Pinned against the
/// oracle's `simp_lemmas` array.
static FROZEN_SIMP_LEMMAS: [BatteryEntry; 10] = [
    BatteryEntry {
        name: "Thermite.Env.bindInt",
    },
    BatteryEntry {
        name: "Thermite.intVal",
    },
    BatteryEntry {
        name: "Thermite.denote",
    },
    BatteryEntry {
        name: "Thermite.arithDenote",
    },
    BatteryEntry {
        name: "Thermite.castDenote",
    },
    BatteryEntry {
        name: "Thermite.seqIdx",
    },
    BatteryEntry {
        name: "Thermite.seqSub",
    },
    BatteryEntry {
        name: "Thermite.scrutVal",
    },
    BatteryEntry {
        name: "Thermite.OptResVal.isVariant",
    },
    BatteryEntry {
        name: "Thermite.OptResVal.variant",
    },
];

/// The frozen tactic allowlist as a slice (REQ-5). Exposed so the conformance test can
/// assert the full table against the oracle (AC-9) and so a later consumer (the skill
/// generator, §10) can regenerate the battery section from this single source of truth —
/// the combinator-registry `all()` precedent.
#[allow(
    dead_code,
    reason = "REQ-5 registry accessor: the conformance test (battery::tests) asserts it \
              against the golden oracle (AC-9), and it is the single-source-of-truth API \
              the skill regenerator (§10) consumes — the combinators::all parallel. forge \
              is a binary crate, so the pub fn is not reachable externally."
)]
#[must_use]
pub fn all_tactics() -> &'static [BatteryEntry] {
    &FROZEN_TACTICS
}

/// The frozen simp-lemma set as a slice (REQ-5). Exposed for the conformance test (AC-9).
#[allow(
    dead_code,
    reason = "REQ-5 registry accessor: the conformance test asserts it against the golden \
              oracle (AC-9). forge is a binary crate, so the pub fn is not reachable \
              externally (the combinators::all parallel)."
)]
#[must_use]
pub fn all_simp_lemmas() -> &'static [BatteryEntry] {
    &FROZEN_SIMP_LEMMAS
}

/// Is `tactic` an allowlisted frozen-battery tactic head? (REQ-5.) Exact-match by name
/// — `simp_all`/`simp?` are not `simp` (the frozen set is closed to the listed heads).
#[must_use]
pub fn is_allowed_tactic(tactic: &str) -> bool {
    FROZEN_TACTICS.iter().any(|e| e.name == tactic)
}

/// The leaf segment of a (possibly qualified) name (`Thermite.intVal` → `intVal`).
fn leaf(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

/// Is `lemma` a frozen simp lemma? (REQ-5.) Matches the full qualified name OR the leaf
/// segment, so a spine-local citation `intVal` resolves to the frozen `Thermite.intVal`
/// (the spine opens the `Thermite` namespace) without the author having to qualify.
#[must_use]
pub fn is_allowed_simp_lemma(lemma: &str) -> bool {
    FROZEN_SIMP_LEMMAS
        .iter()
        .any(|e| e.name == lemma || leaf(e.name) == leaf(lemma))
}

/// A citation extracted from a verbatim proof block (REQ-5): a tactic head, or a
/// simp-lemma name named inside a `simp [ … ]`. The scanner records every citation in
/// document order; [`enforce`] checks each against the frozen battery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Citation {
    /// A tactic invocation head (the leading identifier of a tactic-sequencing unit).
    Tactic(String),
    /// A simp lemma named inside a `simp [ … ]` / `simp only [ … ]` argument list.
    SimpLemma(String),
}

/// A frozen-battery refusal (REQ-5 / AC-9): a proof cites a tactic or a simp lemma the
/// frozen battery does not list. A hard error, named — never a warning (R-BAT-1). The
/// proof-tier analogue of `thermite_spec::SpecError` (the contract-cage refusal) and
/// `covenant_engine::CovenantError` (the covenant refusal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatteryViolation {
    /// The proof cites a tactic outside the frozen allowlist.
    UnlistedTactic {
        /// The covenanted/proved item the proof block belongs to (named, R-BAT-1).
        item: String,
        /// The offending tactic head.
        tactic: String,
    },
    /// The proof cites a simp lemma outside the frozen simp set.
    UnlistedSimpLemma {
        /// The item the proof block belongs to (named, R-BAT-1).
        item: String,
        /// The offending simp lemma name (as cited).
        lemma: String,
    },
}

impl BatteryViolation {
    /// A stable cause tag for the rejection certificate (parallel to the covenant
    /// `RejectReason` causes the 2b gate uses).
    #[must_use]
    pub fn cause(&self) -> &'static str {
        match self {
            BatteryViolation::UnlistedTactic { .. } => "BatteryUnlistedTactic",
            BatteryViolation::UnlistedSimpLemma { .. } => "BatteryUnlistedSimpLemma",
        }
    }

    /// The item the offending proof belongs to (every variant names its item — R-BAT-1).
    #[must_use]
    pub fn item(&self) -> &str {
        match self {
            BatteryViolation::UnlistedTactic { item, .. }
            | BatteryViolation::UnlistedSimpLemma { item, .. } => item,
        }
    }

    /// A human detail for the rejection certificate — NAMING the offending tactic/lemma
    /// (AC-9: "refused with the name in the error").
    #[must_use]
    pub fn detail(&self) -> String {
        match self {
            BatteryViolation::UnlistedTactic { item, tactic } => format!(
                "the proof of `{item}` cites the tactic `{tactic}`, which is not in the frozen \
                 battery allowlist (REQ-5: omega, simp, nlinarith, induction, decide, calc, \
                 exact, from, push_neg). A proof citing an unlisted tactic is REFUSED at \
                 elaboration, never warned."
            ),
            BatteryViolation::UnlistedSimpLemma { item, lemma } => format!(
                "the proof of `{item}` cites the simp lemma `{lemma}` in a `simp [ … ]`, which \
                 is not in the frozen battery simp set (REQ-5). A proof citing an unlisted simp \
                 lemma is REFUSED at elaboration, never warned."
            ),
        }
    }
}

/// Strip Lean comments from proof text so a tactic name inside a comment is never cited
/// (REQ-5): `-- …` line comments to end-of-line, and `/- … -/` block comments (one level
/// — the corpus does not nest battery-proof comments). Deterministic.
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Block comment `/- … -/`.
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'-' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'-' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            out.push(' ');
            continue;
        }
        // Line comment `-- …`.
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        out.push(text[i..].chars().next().unwrap());
        i += text[i..].chars().next().unwrap().len_utf8();
    }
    out
}

/// Is `c` a tactic-identifier character? Lean identifiers are alphanumeric + `_` + `'`;
/// a leading char must be a letter or `_` (a bare `_` placeholder is filtered later).
fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\''
}

/// The leading identifier of `s` (skipping leading whitespace + a curated set of
/// non-tactic punctuation), or `None` if `s` opens with a non-identifier (an expression,
/// a `_` placeholder, an operator). Used to find a tactic head. The returned slice is the
/// run of identifier chars; a qualified head's `.`-suffix (rare for tactics) is dropped.
fn leading_ident(s: &str) -> Option<&str> {
    let s = s.trim_start_matches([' ', '\t', '·', '•', '-', '+', '(', '{', ')', '}']);
    let s = s.trim_start();
    let mut chars = s.char_indices();
    let (_, first) = chars.next()?;
    if !(first.is_alphabetic() || first == '_') {
        return None;
    }
    let end = s
        .char_indices()
        .find(|(_, c)| !is_ident_char(*c))
        .map_or(s.len(), |(i, _)| i);
    let ident = &s[..end];
    // A bare `_` (a calc-step hole / discard pattern) is not a tactic head.
    if ident == "_" {
        return None;
    }
    Some(ident)
}

/// Extract the tactic head(s) of one tactic-sequencing fragment (REQ-5). Handles the
/// common proof shapes without a full Lean parser, returning 0..2 heads in order:
/// - a `calc` block first line `calc a = b := by tac` → both `calc` and the inline step's
///   `by` tactic (`calc` opens a step-structured block);
/// - a match/`induction … with` arm `| label args => tac` → the head is after the last
///   `=>` (the arm label is not a tactic);
/// - a relation-bearing step `a = b := by tac` / `_ = c := by tac` → the head is after
///   the last `by` (the LHS is a calc expression, not a tactic); a term-mode step
///   `… := term` (no `by`) cites no tactic;
/// - otherwise the leading identifier is the head (`exact h`, `apply f`, `omega`, and a
///   binder `have h := by tac` whose leading `have` is itself the unlisted citation).
fn fragment_heads(fragment: &str) -> Vec<&str> {
    let fragment = fragment.trim_start_matches([' ', '\t', '·', '•', '(', '{', ')', '}']);
    let fragment = fragment.trim();
    // `calc` opens a step-structured block: it is a head, and its inline first step's
    // `by` tactic (if present) is a second head.
    if let Some(rest) = strip_word_prefix(fragment, "calc") {
        let mut heads = vec!["calc"];
        heads.extend(step_head(rest));
        return heads;
    }
    // Match-arm RHS: take after the last `=>`.
    let after_arrow = fragment
        .rfind("=>")
        .map_or(fragment, |i| &fragment[i + 2..]);
    step_head(after_arrow)
}

/// The single tactic head of a fragment with no leading `calc`/`=>` structure (REQ-5): a
/// relation-bearing calc step (`a = b := by tac`) heads at the `by` tactic; a term-mode
/// step (`… := term`, no `by`) cites nothing; otherwise the leading identifier is the
/// head (covering `omega`, `exact h`, and a non-frozen binder `have h := by …`).
fn step_head(fragment: &str) -> Vec<&str> {
    if let Some(i) = fragment.find(":=") {
        let lhs = &fragment[..i];
        let rhs = &fragment[i + 2..];
        // A calc step's LHS is an expression (a relation, or a `_` hole) — not a tactic.
        if lhs.trim_start().starts_with('_') || has_relation_op(lhs) {
            return match find_by(rhs) {
                Some(j) => leading_ident(&rhs[j..]).into_iter().collect(),
                // A term-mode step body cites no tactic.
                None => Vec::new(),
            };
        }
        // Else the leading ident IS the head (a binder like `have`/`let`, or a tactic).
    }
    // A leading `by ` (or a bare-by fragment) hands off to the tactic after it.
    let body = find_by(fragment).map_or(fragment, |j| &fragment[j..]);
    leading_ident(body).into_iter().collect()
}

/// `Some(rest)` if `s` starts with the whole word `word` (bounded by a non-identifier
/// char), with `rest` the remainder after it; else `None`.
fn strip_word_prefix<'a>(s: &'a str, word: &str) -> Option<&'a str> {
    let rest = s.strip_prefix(word)?;
    if rest.chars().next().is_none_or(|c| !is_ident_char(c)) {
        Some(rest)
    } else {
        None
    }
}

/// Does `s` contain a relation operator (the mark of a calc-step LHS, not a tactic)?
fn has_relation_op(s: &str) -> bool {
    s.contains('=')
        || s.contains('≤')
        || s.contains('<')
        || s.contains('>')
        || s.contains('≥')
        || s.contains('≠')
        || s.contains('↔')
        || s.contains('∣')
        || s.contains('∈')
        || s.contains('⊆')
}

/// The byte offset just past a `by` keyword token in `s` (the last one), or `None`. A
/// `by` must be a whole word (bounded by non-identifier chars) so `bypass` does not match.
fn find_by(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut found = None;
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'b' && bytes[i + 1] == b'y' {
            let before_ok = i == 0 || !is_ident_char(s[..i].chars().next_back().unwrap());
            let after = i + 2;
            let after_ok =
                after >= bytes.len() || !is_ident_char(s[after..].chars().next().unwrap_or(' '));
            if before_ok && after_ok {
                found = Some(after);
            }
        }
        i += 1;
    }
    found
}

/// Parse the lemma names from a `simp [ … ]` / `simp only [ … ]` argument list starting
/// at the `[` (REQ-5). Returns the leading identifier of each comma-separated entry
/// (stripping a leading `←`/`<-` rewrite-direction marker and any applied arguments). The
/// `]` terminates the list; an unterminated list takes to end-of-fragment.
fn simp_lemmas_in(bracketed: &str) -> Vec<String> {
    let inner = match (bracketed.find('['), bracketed.find(']')) {
        (Some(o), Some(c)) if c > o => &bracketed[o + 1..c],
        (Some(o), None) => &bracketed[o + 1..],
        _ => return Vec::new(),
    };
    inner
        .split(',')
        .filter_map(|entry| {
            let entry = entry
                .trim()
                .trim_start_matches(['←', '<', '-', ' ', '\t'])
                .trim_start();
            // The lemma name is the leading qualified identifier; applied args
            // (`foo bar`) and config (`foo := x`) are dropped to the head.
            let end = entry
                .char_indices()
                .find(|(_, c)| !(is_ident_char(*c) || *c == '.'))
                .map_or(entry.len(), |(i, _)| i);
            let name = &entry[..end];
            if name.is_empty() || name == "_" {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

/// Scan a verbatim proof block ([`thermite_syntax::ast::ProofBlock::text`]) into its
/// tactic + simp-lemma citations, in document order (REQ-5). A deterministic citation
/// scanner — not a full Lean parser: it strips comments, splits into tactic-sequencing
/// units (newline / `;` / `<;>` / `|`-alternative), extracts each unit's tactic head
/// ([`fragment_head`]), and parses every `simp [ … ]` lemma list it finds. Pure
/// (R-CODE-5).
#[must_use]
pub fn scan_citations(proof_text: &str) -> Vec<Citation> {
    let stripped = strip_comments(proof_text);
    // Normalize the multi-char sequencer `<;>` to a single `;` so the split is uniform.
    let normalized = stripped.replace("<;>", ";");
    let mut citations = Vec::new();
    for unit in normalized.split(['\n', ';', '|']) {
        let unit = unit.trim();
        if unit.is_empty() {
            continue;
        }
        for head in fragment_heads(unit) {
            citations.push(Citation::Tactic(head.to_string()));
        }
        // A `simp [ … ]` anywhere in the unit cites lemmas, regardless of whether `simp`
        // was the head (e.g. a `have h := by simp [bad]` body); find each `simp` token's
        // following bracket list.
        for lemma in simp_lemma_citations(unit) {
            citations.push(Citation::SimpLemma(lemma));
        }
    }
    citations
}

/// Every simp-lemma name cited by a `simp [ … ]` / `simp only [ … ]` in one unit (REQ-5).
fn simp_lemma_citations(unit: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search = unit;
    while let Some(pos) = find_word(search, "simp") {
        let rest = &search[pos..];
        // The bracket list for this `simp` is the first `[` before the next `simp`/end.
        if let Some(br) = rest.find('[') {
            // Guard: the `[` must belong to this simp (no intervening `simp`).
            let next_simp = find_word(&rest[4..], "simp").map(|p| p + 4);
            if next_simp.is_none_or(|ns| br < ns) {
                out.extend(simp_lemmas_in(&rest[br..]));
            }
        }
        search = &search[pos + 4..];
    }
    out
}

/// The byte offset of a whole-word occurrence of `word` in `s` (bounded by
/// non-identifier chars), or `None`. Used to anchor `simp` tokens.
fn find_word(s: &str, word: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(rel) = s[from..].find(word) {
        let at = from + rel;
        let before_ok = at == 0 || !is_ident_char(s[..at].chars().next_back().unwrap());
        let after = at + word.len();
        let after_ok = after >= s.len() || !is_ident_char(s[after..].chars().next().unwrap_or(' '));
        if before_ok && after_ok {
            return Some(at);
        }
        from = at + word.len();
    }
    None
}

/// The elaboration-time frozen-battery gate (REQ-5 / AC-9): refuse a proof block that
/// cites an unlisted tactic OR an unlisted simp lemma, naming the first offender (scan is
/// in document order). A clean proof (every citation in the frozen battery) returns
/// `Ok(())`. `item` names the proved/lemma item for the refusal message (R-BAT-1).
///
/// This is the hard gate — a violation is a refusal, never a warning: `check.rs` runs it
/// before a forge-tier item is admitted, the proof-tier mirror of the
/// `thermite_spec::validate` contract cage.
#[allow(
    dead_code,
    reason = "REQ-5 single-proof gate: the production caller (`check.rs`) goes through the \
              project-lemma-aware `enforce_forge_item_with_lemmas`; this no-namespace \
              convenience entry is exercised by battery::tests (the unlisted-tactic / \
              unlisted-simp-lemma refusals) and is the natural single-block API."
)]
pub fn enforce(item: &str, proof_text: &str) -> Result<(), BatteryViolation> {
    enforce_with_project_lemmas(item, proof_text, &std::collections::BTreeSet::new())
}

/// The frozen-battery gate, made aware of the per-project lemma namespace (REQ-9 / AC-13,
/// increment 3). Identical to [`enforce`] EXCEPT a `simp [ … ]` citation that names a
/// project lemma (`project_lemmas`) is not refused as an unlisted simp lemma — it is
/// DEFERRED to the REQ-9 certified-only citation gate
/// ([`crate::lemma_library::enforce_citations`]), which refuses it only if the lemma did
/// not certify. The frozen battery still refuses a citation that is neither a frozen spine
/// lemma NOR a project lemma (the closed-set discipline is unchanged), and tactic-head
/// enforcement is untouched. With an empty `project_lemmas` set this is byte-identical to
/// the pre-REQ-9 [`enforce`] (so the v1 corpus, which has no project lemmas, is a no-op).
pub fn enforce_with_project_lemmas(
    item: &str,
    proof_text: &str,
    project_lemmas: &std::collections::BTreeSet<String>,
) -> Result<(), BatteryViolation> {
    for citation in scan_citations(proof_text) {
        match citation {
            Citation::Tactic(tactic) => {
                if !is_allowed_tactic(&tactic) {
                    return Err(BatteryViolation::UnlistedTactic {
                        item: item.to_string(),
                        tactic,
                    });
                }
            }
            Citation::SimpLemma(lemma) => {
                // A project-lemma citation is resolved by REQ-9 (certified-only), not
                // refused here; only a citation outside both the frozen set and the
                // project namespace is the unlisted-simp-lemma refusal.
                if !is_allowed_simp_lemma(&lemma) && !project_lemmas.contains(&lemma) {
                    return Err(BatteryViolation::UnlistedSimpLemma {
                        item: item.to_string(),
                        lemma,
                    });
                }
            }
        }
    }
    Ok(())
}

/// Enforce the frozen battery over every proof block a forge-tier item carries (REQ-5 /
/// AC-9), naming the first offender. A `lemma … proof { … }` has one proof block; a
/// `proof for f { ens#k by { … } … }` has one per obligation (checked in source order). A
/// `prop fn` / `witness` block carries no proof to elaborate → `Ok(())`. This is the
/// per-item elaboration gate `check.rs` runs before a forge-tier item is admitted.
/// Enforce the frozen battery over every proof block a forge-tier item carries, made aware
/// of the per-project lemma namespace (REQ-5 / REQ-9 / AC-9 / AC-13, increment 3): a `simp
/// [ … ]` citation naming a project lemma is DEFERRED to the certified-only gate instead of
/// being refused as an unlisted simp lemma (see [`enforce_with_project_lemmas`]). `check.rs`
/// passes the project's lemma names here so a lemma-citing forge item passes the
/// frozen-battery gate and the REQ-9 certified-only resolution decides it. An empty
/// `project_lemmas` set reproduces the pre-REQ-9 behavior exactly (the v1 corpus has no
/// project lemmas → a no-op on the v1 oracle). A `lemma … proof { … }` has one proof block;
/// a `proof for f { ens#k by { … } … }` has one per obligation (checked in source order); a
/// `prop fn` / `witness` carries no proof to elaborate → `Ok(())`.
pub fn enforce_forge_item_with_lemmas(
    item: &thermite_syntax::ast::ForgeItem,
    project_lemmas: &std::collections::BTreeSet<String>,
) -> Result<(), BatteryViolation> {
    use thermite_syntax::ast::ForgeItem;
    match item {
        ForgeItem::Lemma(l) => enforce_with_project_lemmas(&l.name, &l.proof.text, project_lemmas),
        ForgeItem::Proof(p) => {
            for ob in &p.obligations {
                enforce_with_project_lemmas(&p.target, &ob.proof.text, project_lemmas)?;
            }
            Ok(())
        }
        // No proof block to elaborate.
        ForgeItem::PropFn(_) | ForgeItem::Witness(_) => Ok(()),
    }
}

/// A frozen-battery `Stuck` payload (REQ-5): the residual goal(s) a proof left open + the
/// "missing simp bridge" hint (RFC-1 §8). Carried into
/// [`CertVerdict::Stuck`](crate::verdict::CertVerdict::Stuck). Serializable so it joins
/// the cert diagnostic surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StuckHints {
    /// The residual proof goal(s) left open (the `⊢ …` lines lake reported).
    pub goals: Vec<String>,
    /// The "missing simp bridge" hint, if a residual goal mentions a symbol the frozen
    /// battery simp set does not normalize.
    pub hint: Option<String>,
}

/// The residual goals in lake's "unsolved goals" output (REQ-5): every `⊢ …` line after
/// the marker, trimmed, in order. Returns empty if the output has no "unsolved goals"
/// marker (the proof did not merely leave a residual — it failed some other way).
#[must_use]
pub fn residual_goals(lake_output: &str) -> Vec<String> {
    let lower = lake_output.to_ascii_lowercase();
    if !lower.contains("unsolved goals") {
        return Vec::new();
    }
    // From the marker line onward, collect the turnstile goal lines.
    let marker = lower.find("unsolved goals").unwrap_or(0);
    let tail = &lake_output[marker..];
    tail.lines()
        .map(str::trim)
        .filter(|l| l.starts_with('⊢'))
        .map(ToString::to_string)
        .collect()
}

/// The "missing simp bridge" heuristic (REQ-5, the RFC-1 §8 transcript): if a residual
/// goal still mentions a function-application head symbol the frozen battery simp set
/// does not normalize, name it + the simp bridge lemma (`<head>_cons`) likely missing
/// from the frozen set. Returns `None` when the residual mentions only frozen-normalized
/// symbols / bound variables (no specific bridge to suggest). Deterministic: the first
/// such symbol in the first goal.
#[must_use]
pub fn missing_bridge_hint(goals: &[String]) -> Option<String> {
    let first = goals.first()?;
    let candidate = bridge_candidate(first)?;
    Some(format!(
        "missing simp bridge: the residual goal still mentions `{candidate}`, which the \
         frozen battery simp set does not normalize — a `simp` bridge lemma for it (e.g. \
         `{candidate}_cons`) is likely needed but is not in the frozen battery"
    ))
}

/// The leftmost bridge candidate in a residual goal: the first multi-letter identifier
/// (after the `⊢`) that is neither a frozen simp lemma leaf nor a known logical/arithmetic
/// keyword/type. A single-letter token (a bound variable) is skipped. Returns the head of
/// the unreduced application the simp battery got stuck on.
fn bridge_candidate(goal: &str) -> Option<String> {
    // Drop the turnstile prefix.
    let body = goal.trim_start_matches('⊢').trim_start();
    let mut idents = Vec::new();
    let mut cur = String::new();
    for c in body.chars() {
        if is_ident_char(c) || c == '.' {
            cur.push(c);
        } else {
            if !cur.is_empty() {
                idents.push(std::mem::take(&mut cur));
            }
        }
    }
    if !cur.is_empty() {
        idents.push(cur);
    }
    idents.into_iter().find(|id| {
        id.chars().next().is_some_and(char::is_alphabetic)
            && leaf(id).len() >= 2
            && !is_frozen_or_builtin(leaf(id))
    })
}

/// Is `leaf` a frozen simp lemma leaf or a known builtin (a logical/arith connective name
/// or a base type) the missing-bridge heuristic should not suggest a bridge for?
fn is_frozen_or_builtin(leaf: &str) -> bool {
    if is_allowed_simp_lemma(leaf) {
        return true;
    }
    matches!(
        leaf,
        "True"
            | "False"
            | "Nat"
            | "Int"
            | "Bool"
            | "Prop"
            | "Type"
            | "And"
            | "Or"
            | "Not"
            | "Iff"
            | "Eq"
            | "id"
    )
}

/// Build a [`StuckHints`] from a lake "unsolved goals" output (REQ-5): the residual goals
/// plus the missing-bridge hint. Returns `None` when the output carries no residual goal
/// (so the caller falls through to the ordinary verdict map — a non-`Stuck` failure).
#[must_use]
pub fn stuck_from_lake_output(lake_output: &str) -> Option<StuckHints> {
    let goals = residual_goals(lake_output);
    if goals.is_empty() {
        return None;
    }
    let hint = missing_bridge_hint(&goals);
    Some(StuckHints { goals, hint })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::path::PathBuf;

    // ---- the golden oracle shapes (AC-9: the registry file exists + matches) --------

    #[derive(Debug, Deserialize)]
    struct BatteryOracle {
        tactics: Vec<OracleEntry>,
        simp_lemmas: Vec<OracleEntry>,
    }

    #[derive(Debug, Deserialize)]
    struct OracleEntry {
        name: String,
    }

    fn read_oracle() -> BatteryOracle {
        // CARGO_MANIFEST_DIR is forge/; the oracle is at the workspace root.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("conformance")
            .join("battery")
            .join("registry.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read battery oracle {}: {e}", path.display()));
        serde_json::from_str(&text).expect("battery registry.json parses")
    }

    /// AC-9: the crate's frozen battery equals `conformance/battery/registry.json`
    /// name-for-name, both sets, no extras either way (the combinator-oracle discipline).
    #[test]
    fn registry_matches_oracle() {
        let oracle = read_oracle();

        let code_tactics: Vec<&str> = all_tactics().iter().map(|e| e.name).collect();
        let oracle_tactics: Vec<&str> = oracle.tactics.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            code_tactics, oracle_tactics,
            "frozen tactic allowlist differs from the oracle (order + contents pinned)"
        );

        let code_lemmas: Vec<&str> = all_simp_lemmas().iter().map(|e| e.name).collect();
        let oracle_lemmas: Vec<&str> = oracle.simp_lemmas.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            code_lemmas, oracle_lemmas,
            "frozen simp set differs from the oracle (order + contents pinned)"
        );
    }

    /// The frozen lists are exactly the REQ-5 sizes (a guard against a silent add/drop
    /// that also happens to edit the oracle).
    #[test]
    fn frozen_sets_have_the_pinned_sizes() {
        assert_eq!(all_tactics().len(), 9, "9 frozen tactics (REQ-5)");
        assert_eq!(all_simp_lemmas().len(), 10, "10 frozen simp lemmas");
    }

    // ---- the citation scanner --------------------------------------------------------

    #[test]
    fn scans_tactic_heads_and_simp_lemmas() {
        let proof = "intro h\n  simp only [Thermite.intVal, Thermite.denote] at h\n  omega";
        let cites = scan_citations(proof);
        assert_eq!(
            cites,
            vec![
                Citation::Tactic("intro".to_string()),
                Citation::Tactic("simp".to_string()),
                Citation::SimpLemma("Thermite.intVal".to_string()),
                Citation::SimpLemma("Thermite.denote".to_string()),
                Citation::Tactic("omega".to_string()),
            ]
        );
    }

    #[test]
    fn match_arm_and_calc_step_heads_are_after_arrow_and_by() {
        // `induction … with | zero => simp | succ k ih => omega` — the arm labels
        // (`zero`/`succ`) are not tactics; the heads are the arm RHS tactics.
        let cites = scan_citations("induction n with | zero => decide | succ k ih => omega");
        assert_eq!(
            cites,
            vec![
                Citation::Tactic("induction".to_string()),
                Citation::Tactic("decide".to_string()),
                Citation::Tactic("omega".to_string()),
            ]
        );
        // A calc/have step `… := by tac` heads at the `by` tactic; a term-mode `:= term`
        // step cites no tactic.
        let calc = scan_citations("calc a = b := by simp\n  _ = c := by omega\n  _ = d := h.symm");
        assert_eq!(
            calc,
            vec![
                Citation::Tactic("calc".to_string()),
                Citation::Tactic("simp".to_string()),
                Citation::Tactic("omega".to_string()),
            ]
        );
    }

    #[test]
    fn comments_are_stripped_before_scanning() {
        let proof = "-- apply foo\n  exact h /- nlinarith -/";
        let cites = scan_citations(proof);
        assert_eq!(cites, vec![Citation::Tactic("exact".to_string())]);
    }

    // ---- the enforce gate (AC-9: refused with the name in the error) -----------------

    #[test]
    fn clean_proof_passes() {
        // Only frozen tactics + frozen simp lemmas → admitted.
        let clean = "  simp only [Thermite.intVal, Thermite.denote]\n  omega";
        assert!(enforce("f", clean).is_ok());
        // A match-arm proof citing only frozen tactics also passes.
        let arms = "induction n with | zero => decide | succ k ih => omega";
        assert!(enforce("f", arms).is_ok());
    }

    #[test]
    fn unlisted_tactic_is_refused_with_its_name() {
        // `apply` is not in the frozen allowlist → refused, named (AC-9).
        let proof = "  apply Nat.le_trans\n  omega";
        match enforce("merge", proof) {
            Err(BatteryViolation::UnlistedTactic { item, tactic }) => {
                assert_eq!(item, "merge");
                assert_eq!(tactic, "apply");
                assert!(enforce("merge", proof)
                    .unwrap_err()
                    .detail()
                    .contains("apply"));
            }
            other => panic!("expected an UnlistedTactic refusal naming `apply`, got {other:?}"),
        }
    }

    #[test]
    fn unlisted_simp_lemma_is_refused_with_its_name() {
        // `melems_cons` (the RFC-1 §8 merge bridge) is not in the frozen simp set →
        // refused, named (AC-9).
        let proof = "  simp [melems_cons]\n  omega";
        match enforce("merge", proof) {
            Err(BatteryViolation::UnlistedSimpLemma { item, lemma }) => {
                assert_eq!(item, "merge");
                assert_eq!(lemma, "melems_cons");
                assert!(enforce("merge", proof)
                    .unwrap_err()
                    .detail()
                    .contains("melems_cons"));
            }
            other => {
                panic!("expected an UnlistedSimpLemma refusal naming `melems_cons`, got {other:?}")
            }
        }
    }

    #[test]
    fn frozen_simp_lemma_resolves_by_leaf_or_qualified() {
        // The spine cites the unqualified leaf (`intVal`); it resolves to the frozen
        // qualified `Thermite.intVal`.
        assert!(is_allowed_simp_lemma("intVal"));
        assert!(is_allowed_simp_lemma("Thermite.intVal"));
        assert!(!is_allowed_simp_lemma("melems_cons"));
    }

    // ---- the Stuck producer (AC-9: residual goal + missing-bridge hint) --------------

    #[test]
    fn merge_example_residual_goal_carries_missing_bridge_hint() {
        // The RFC-1 §8 merge transcript: the auto battery elaborates but leaves a
        // residual multiset-elements goal `simp` could not close — `melems` is the
        // unreduced head, `melems_cons` the missing bridge.
        let lake = "error: unsolved goals\n\
                    ⊢ melems (merge a b) = melems a + melems b\n";
        let stuck = stuck_from_lake_output(lake).expect("a residual goal is Stuck, not None");
        assert_eq!(
            stuck.goals,
            vec!["⊢ melems (merge a b) = melems a + melems b".to_string()]
        );
        let hint = stuck.hint.expect("a missing-bridge hint");
        assert!(
            hint.contains("missing simp bridge"),
            "the hint names the heuristic: {hint}"
        );
        assert!(
            hint.contains("melems"),
            "the hint names the unreduced head: {hint}"
        );
        assert!(
            hint.contains("melems_cons"),
            "the hint names the missing bridge: {hint}"
        );
    }

    #[test]
    fn non_residual_output_is_not_stuck() {
        // A lake failure that is not an unsolved-goals residual (e.g. an elaboration
        // error) yields no Stuck payload — the caller falls through to the verdict map.
        assert!(stuck_from_lake_output("error: type mismatch\n  expected Nat").is_none());
        assert!(stuck_from_lake_output("").is_none());
    }

    #[test]
    fn residual_with_only_bound_vars_has_no_bridge_hint() {
        // `⊢ a ≤ b` is a residual (Stuck) but mentions only single-letter bound vars —
        // no specific missing bridge to name.
        let stuck = stuck_from_lake_output("error: unsolved goals\n⊢ a ≤ b").expect("Stuck");
        assert_eq!(stuck.goals, vec!["⊢ a ≤ b".to_string()]);
        assert!(
            stuck.hint.is_none(),
            "no multi-letter head → no bridge hint"
        );
    }
}
