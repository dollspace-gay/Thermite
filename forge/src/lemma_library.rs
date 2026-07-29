//! `forge/src/lemma_library.rs` — the Stage-1 forge-tier **lemma library mechanics**
//! (`.design/stage1-forge-tier.md` REQ-9 / AC-13, increment 3; the last stage-1
//! feature increment). This is the logic the surface (2a's `lemma` item), the proof
//! discharge (2e's `discharge_forge_lemma`), and the frozen battery (2c's
//! `scan_citations`) set up for. REQ-9 trails usage — it is last in dependency order
//! because it reasons about ALREADY-discharged lemma certificates.
//!
//! Three mechanics, all per-project (Q1 — there is no cross-project sharing; that is
//! deferred, see the spec's Out of Scope):
//!
//! 1. **Per-project lemma namespace** (Q1). A project's top-level `lemma` items form a
//!    namespace ([`LemmaLibrary`]) keyed by name. Only a top-level [`LemmaItem`] enters
//!    the namespace — a proof-local `have`/`let` binding never does, so the namespace is
//!    the only cross-function citation surface (Q6: "no cross-function sharing except via
//!    a lemma"). A `proof for f` block's several `ens#k` obligations share that one
//!    function's local context; a name they establish locally is invisible to `proof for
//!    g` (it is not in the namespace) — enforced structurally by building the namespace
//!    from top-level lemmas only.
//!
//! 2. **Certified-only citation resolution** (AC-13). A proof citing a project lemma
//!    (`simp [melems_cons]`) resolves only if that lemma carries a certificate. A
//!    citation to a project lemma that did not certify is REFUSED — named — never a
//!    silent pass ([`enforce_citations`] → [`UncertifiedCitation`]). This is the
//!    proof-tier analogue of the frozen-battery refusal: the frozen battery refuses a
//!    citation outside the closed spine simp set; REQ-9 refuses a citation to an
//!    uncertified project lemma. The two compose — a simp citation resolves against the
//!    frozen battery OR a certified project lemma; anything else is refused.
//!
//! 3. **Dedup-on-burn by statement hash with citation rewrite** (Q1 / AC-13). Two
//!    lemmas with the same statement (same params + `req` + `ens`) under different names
//!    are one proven fact: only the first (the canonical) is stored, and a citation to
//!    the duplicate is rewritten to the canonical ([`LemmaLibrary::rewrite_citations`]).
//!    The statement hash ([`statement_hash`]) excludes the lemma NAME and the proof, so
//!    re-proving the same statement under a new name dedups to the existing lemma rather
//!    than storing a copy.
//!
//! Governing design: `.design/stage1-forge-tier.md` REQ-9 / AC-13.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use thermite_syntax::ast::{ForgeItem, Item, LemmaItem, Param, Program, Type};

use crate::manifest::{Certificate, Level};

/// Domain-separation tag for the lemma statement hash, so a forge lemma statement hash
/// never collides with an unrelated sha256 use of the same bytes (mirrors the proof
/// cache's `DOMAIN` discipline, `cache.rs`).
const DOMAIN: &[u8] = b"thermite.forge.lemma-library.v1";

/// The content hash of a lemma's statement (REQ-9 / Q1) — a lowercase-hex sha256 over
/// the lemma's params + `req` + every `ens`, domain-tagged and length-prefixed (the
/// `cache::field` discipline). The lemma NAME and the proof are excluded: two lemmas
/// stating the same proposition under different names (or proved by different tactics)
/// share a statement hash — that is the dedup key (AC-13: "two identical lemmas under
/// different names → one stored"). Pure and deterministic (R-CODE-5): the params are
/// rendered structurally (name + the span-free [`Type`] shape, never a byte offset) and
/// the clauses by their verbatim `text`, so the hash is a function of the statement, not
/// of where it sits in the source.
#[must_use]
pub fn statement_hash(l: &LemmaItem) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN);
    // Params, in order: each `name:type` rendered span-free (`Type` carries no span, so
    // its structural rendering is position-independent — two identical lemmas at
    // different source offsets render identically).
    field(
        &mut hasher,
        b"param-count",
        &(l.params.len() as u64).to_le_bytes(),
    );
    for p in &l.params {
        field(&mut hasher, b"param", render_param(p).as_bytes());
    }
    // `req` — the single hypothesis clause's verbatim text.
    field(&mut hasher, b"req", l.req.text.as_bytes());
    // `ens` — every conclusion clause's verbatim text, in order.
    field(
        &mut hasher,
        b"ens-count",
        &(l.ens.len() as u64).to_le_bytes(),
    );
    for e in &l.ens {
        field(&mut hasher, b"ens", e.text.as_bytes());
    }
    hex_lower(&hasher.finalize())
}

/// Render a param as a span-free `name:type` string for the statement hash. [`Type`] is
/// a pure structural enum (no span field), so its [`std::fmt::Debug`] rendering is
/// position-independent and deterministic — exactly what the dedup key needs.
fn render_param(p: &Param) -> String {
    format!("{}:{}", p.name, render_type(&p.ty))
}

/// A span-free structural rendering of a [`Type`] for the statement hash. Recurses the
/// type so two structurally-equal types render identically regardless of source offset.
/// Shared with the `dec wf` accessibility carrier rendering (`accessibility.rs`).
pub(crate) fn render_type(ty: &Type) -> String {
    match ty {
        Type::Ref { mutable, inner } => {
            format!(
                "&{}{}",
                if *mutable { "mut " } else { "" },
                render_type(inner)
            )
        }
        Type::Slice(inner) => format!("[{}]", render_type(inner)),
        Type::Generic { name, arg } => format!("{name}<{}>", render_type(arg)),
        Type::Box(inner) => format!("Box<{}>", render_type(inner)),
        Type::Vec(inner) => format!("Vec<{}>", render_type(inner)),
        Type::Option(inner) => format!("Option<{}>", render_type(inner)),
        Type::Result(ok, err) => format!("Result<{},{}>", render_type(ok), render_type(err)),
        Type::Map(k, v) => format!("Map<{},{}>", render_type(k), render_type(v)),
        Type::Tuple(elems) => {
            let parts: Vec<String> = elems.iter().map(render_type).collect();
            format!("({})", parts.join(","))
        }
        // The leaf shapes (`Prim`, `Unit`, `Named`, `String`) carry no inner type, so a
        // span-free `Debug` is already position-independent.
        Type::Prim(_) | Type::Unit | Type::Named(_) | Type::String => format!("{ty:?}"),
    }
}

/// Feed one domain-tagged, length-prefixed field into the hasher (the `cache::field`
/// layout: `len(tag):u32-le || tag || len(value):u64-le || value`), so neither the tag
/// nor the value can be re-split into a different (tag, value) pair — the hash is
/// injective on the structured tuple, not on a flat concatenation.
fn field(hasher: &mut Sha256, tag: &[u8], value: &[u8]) {
    hasher.update((tag.len() as u32).to_le_bytes());
    hasher.update(tag);
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

/// Render a byte digest as lowercase hex (the `cache::hex_lower` form). Pure.
fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// One project lemma's namespace entry (REQ-9): its statement hash + whether it carries
/// a certificate. The name is the [`LemmaLibrary::by_name`] key.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LemmaEntry {
    /// The lemma's [`statement_hash`] — the dedup key.
    statement_hash: String,
    /// Whether the lemma discharged to a certificate (an L3, non-rejected cert). Drives
    /// the certified-only citation resolution (AC-13).
    certified: bool,
}

/// The resolution of a `simp [ … ]` citation against the per-project lemma library + the
/// frozen battery (REQ-9 / AC-13). A citation resolves iff it is [`Frozen`] or
/// [`Certified`]; [`Uncertified`] is a REFUSAL (named), and [`Unknown`] falls through to
/// the frozen-battery's unlisted-simp-lemma refusal.
///
/// [`Frozen`]: CitationResolution::Frozen
/// [`Certified`]: CitationResolution::Certified
/// [`Uncertified`]: CitationResolution::Uncertified
/// [`Unknown`]: CitationResolution::Unknown
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CitationResolution {
    /// A frozen-battery spine simp lemma (`Thermite.intVal`, …) — resolves, no project
    /// lemma involved.
    Frozen,
    /// A certified project lemma. `canonical` is the dedup target — the first certified
    /// lemma sharing this statement (the name a citation is rewritten to).
    Certified {
        /// The canonical stored lemma for this statement hash (the dedup target).
        canonical: String,
    },
    /// A known project lemma that did not certify — the citation is REFUSED, named
    /// (AC-13: "citing an uncertified lemma fails with the lemma named").
    Uncertified,
    /// Neither a frozen lemma nor a project lemma — left to the frozen battery's
    /// unlisted-simp-lemma refusal (REQ-9 does not widen the frozen set).
    Unknown,
}

/// The per-project lemma namespace (REQ-9 / Q1): the project's top-level `lemma` items,
/// each with its statement hash + certification status, plus the statement-hash → canonical
/// (first certified) name map that drives dedup-on-burn. Built once per checked file from
/// the parsed program + the settled cert collection ([`LemmaLibrary::build`]).
#[derive(Debug, Clone, Default)]
pub struct LemmaLibrary {
    /// Every top-level project lemma, by name (Q1 namespace). A proof-local `have`/`let`
    /// binding is not here — only a top-level `lemma` enters, so this is the sole
    /// cross-function citation surface (Q6).
    by_name: BTreeMap<String, LemmaEntry>,
    /// statement-hash → the canonical (first certified, source order) lemma name. The
    /// dedup target a citation to a same-statement duplicate is rewritten to (AC-13).
    canonical_by_hash: BTreeMap<String, String>,
}

impl LemmaLibrary {
    /// Build the per-project lemma namespace (REQ-9 / Q1) from the parsed `program` and
    /// the settled cert collection `certs`. A top-level `lemma` is `certified` iff `certs`
    /// carries an L3, non-rejected cert under its name (the discharge produced a proof).
    /// The canonical for a statement hash is the first certified lemma in source order
    /// (`program.items` is source-ordered), so dedup is deterministic.
    #[must_use]
    pub fn build(program: &Program, certs: &[Certificate]) -> Self {
        let certified_names: BTreeSet<&str> = certs
            .iter()
            .filter(|c| c.level == Level::L3 && c.reject.is_none())
            .map(|c| c.item.as_str())
            .collect();
        let mut by_name = BTreeMap::new();
        let mut canonical_by_hash: BTreeMap<String, String> = BTreeMap::new();
        for item in &program.items {
            if let Item::Forge(ForgeItem::Lemma(l)) = item {
                let h = statement_hash(l);
                let certified = certified_names.contains(l.name.as_str());
                if certified {
                    // First certified lemma (source order) for this statement is the
                    // canonical stored copy; a later same-statement lemma dedups to it.
                    canonical_by_hash
                        .entry(h.clone())
                        .or_insert_with(|| l.name.clone());
                }
                by_name.insert(
                    l.name.clone(),
                    LemmaEntry {
                        statement_hash: h,
                        certified,
                    },
                );
            }
        }
        LemmaLibrary {
            by_name,
            canonical_by_hash,
        }
    }

    /// Is `name` a top-level project lemma (in the namespace)? (Q1.)
    #[allow(
        dead_code,
        reason = "REQ-9 namespace-query API: the production gate uses `resolve_citation` \
                  (which subsumes this membership test); this predicate is the auditable \
                  single-question accessor exercised by lemma_library::tests."
    )]
    #[must_use]
    pub fn is_project_lemma(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }

    /// Did the project lemma `name` certify? `false` for a non-lemma name (AC-13).
    #[allow(
        dead_code,
        reason = "REQ-9 namespace-query API: production resolution goes through \
                  `resolve_citation`; this is the single-question certification accessor \
                  exercised by lemma_library::tests."
    )]
    #[must_use]
    pub fn is_certified(&self, name: &str) -> bool {
        self.by_name.get(name).is_some_and(|e| e.certified)
    }

    /// The canonical (dedup-target) name for `name`'s statement hash (Q1) — the first
    /// certified lemma sharing its statement. `None` unless `name` is a certified project
    /// lemma (a non-certified / non-lemma name has no canonical stored copy to rewrite to).
    #[must_use]
    pub fn canonical_name(&self, name: &str) -> Option<&str> {
        let entry = self.by_name.get(name)?;
        if !entry.certified {
            return None;
        }
        self.canonical_by_hash
            .get(&entry.statement_hash)
            .map(String::as_str)
    }

    /// Resolve a `simp [ … ]` citation `name` against the frozen battery + the namespace
    /// (REQ-9 / AC-13). The frozen battery is consulted first (a spine lemma resolves
    /// `Frozen`); otherwise a project lemma resolves `Certified` (with its dedup canonical)
    /// or `Uncertified`; a name that is neither is `Unknown`.
    #[must_use]
    pub fn resolve_citation(&self, name: &str) -> CitationResolution {
        if crate::battery::is_allowed_simp_lemma(name) {
            return CitationResolution::Frozen;
        }
        match self.by_name.get(name) {
            Some(e) if e.certified => CitationResolution::Certified {
                canonical: self.canonical_name(name).unwrap_or(name).to_string(),
            },
            Some(_) => CitationResolution::Uncertified,
            None => CitationResolution::Unknown,
        }
    }

    /// Rewrite a proof's cited-lemma list to its canonical dedup targets (REQ-9 / AC-13:
    /// "burning a statement-hash duplicate rewrites the citation"). A citation to a
    /// certified project lemma is rewritten to the canonical (first certified) lemma for
    /// its statement; a frozen / uncertified / unknown citation is left verbatim. The
    /// result is deduplicated in document order (mirrors `burn::cited_lemmas`), so two
    /// citations that rewrite to the same canonical collapse to one.
    #[must_use]
    pub fn rewrite_citations(&self, cited: &[String]) -> Vec<String> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for name in cited {
            let resolved = self.canonical_name(name).unwrap_or(name).to_string();
            if seen.insert(resolved.clone()) {
                out.push(resolved);
            }
        }
        out
    }

    /// The deduped set of STORED project lemmas (REQ-9 / Q1): one canonical name per
    /// statement hash (AC-13: "one stored"). A non-certified lemma is not stored (it has
    /// no canonical). Sorted (the `BTreeMap` value set), deduped.
    #[allow(
        dead_code,
        reason = "REQ-9 dedup-result accessor: the rewrite is applied per-citation via \
                  `rewrite_citations`/`canonical_name` on the production path; this returns \
                  the deduped stored set as a whole (one per statement) and is exercised by \
                  the dedup-on-burn lemma_library::tests (AC-13 'one stored')."
    )]
    #[must_use]
    pub fn stored_lemmas(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .canonical_by_hash
            .values()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        out.sort();
        out
    }
}

/// A REQ-9 certified-only citation refusal (AC-13): a proof cites a project lemma that
/// lacks a certificate. A hard error, named — the proof-tier analogue of the frozen
/// battery's [`crate::battery::BatteryViolation`] and the covenant refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UncertifiedCitation {
    /// The citing item (the lemma/proof whose proof block made the citation), named so
    /// the refusal is actionable (AC-13).
    pub item: String,
    /// The cited project lemma that did not certify (named in the error, AC-13).
    pub lemma: String,
}

impl UncertifiedCitation {
    /// The stable cause tag for the rejection certificate (parallel to the battery /
    /// covenant cause tags).
    #[must_use]
    pub fn cause(&self) -> &'static str {
        "UncertifiedLemmaCitation"
    }

    /// The human detail — NAMING the offending lemma (AC-13: "fails with the lemma named").
    #[must_use]
    pub fn detail(&self) -> String {
        format!(
            "the proof of `{}` cites the project lemma `{}`, which has no certificate \
             (REQ-9: a citation resolves only against a CERTIFIED lemma). An uncertified-lemma \
             citation is REFUSED at elaboration, named, never warned — fix `{}`'s proof (or \
             remove the citation) before citing it.",
            self.item, self.lemma, self.lemma
        )
    }
}

/// The REQ-9 certified-only citation gate (AC-13): scan a proof block's `simp [ … ]`
/// citations and refuse the first one that names an UNCERTIFIED project lemma, naming it.
/// A citation to a frozen battery lemma or a certified project lemma resolves; an
/// `Unknown` citation is not this gate's concern (the frozen-battery gate refuses it as
/// an unlisted simp lemma). `item` names the citing item for the refusal message.
///
/// This is the forge/Lean-path gate `check_file_with_engine` runs after the lemma
/// discharge pass (so certification status is settled), beside the frozen battery's
/// elaboration gate.
pub fn enforce_citations(
    library: &LemmaLibrary,
    item: &str,
    proof_text: &str,
) -> Result<(), UncertifiedCitation> {
    for citation in crate::battery::scan_citations(proof_text) {
        if let crate::battery::Citation::SimpLemma(name) = citation {
            if matches!(
                library.resolve_citation(&name),
                CitationResolution::Uncertified
            ) {
                return Err(UncertifiedCitation {
                    item: item.to_string(),
                    lemma: name,
                });
            }
        }
    }
    Ok(())
}

/// The set of top-level project lemma names in `program` (REQ-9 / Q1). Collected from
/// the parsed program (no certs needed), so the frozen-battery elaboration gate
/// ([`crate::battery::enforce_forge_item_with_lemmas`]) can DEFER a project-lemma
/// citation (not refuse it as an unlisted simp lemma) — the certified-only resolution
/// then happens on the forge/Lean path once discharge settles certification.
#[must_use]
pub fn project_lemma_names(program: &Program) -> BTreeSet<String> {
    program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Forge(ForgeItem::Lemma(l)) => Some(l.name.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a single-program source, asserting it is clean.
    fn parse_ok(src: &str) -> Program {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        parsed.program
    }

    /// A certified L3 cert for `item` (the discharge-produced shape).
    fn certified(item: &str) -> Certificate {
        Certificate::new(
            item,
            Level::L3,
            vec!["pure".to_string()],
            0,
            vec![crate::manifest::ObligationResult::discharged(item)],
        )
        .graduate_triage_clean()
    }

    // Two lemmas with the same statement (params + req + ens) under different names hash
    // EQUAL; a different statement hashes differently (REQ-9 / Q1: the dedup key excludes
    // the name + the proof).
    #[test]
    fn statement_hash_excludes_name_and_proof() {
        let prog = parse_ok(
            "lemma foo(n: u32) req n > 0 ens n >= 1 proof { omega }\n\
             lemma bar(n: u32) req n > 0 ens n >= 1 proof { simp; omega }\n\
             lemma baz(n: u32) req n > 1 ens n >= 1 proof { omega }",
        );
        let lemmas: Vec<&LemmaItem> = prog
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Forge(ForgeItem::Lemma(l)) => Some(l),
                _ => None,
            })
            .collect();
        let (foo, bar, baz) = (lemmas[0], lemmas[1], lemmas[2]);
        assert_eq!(
            statement_hash(foo),
            statement_hash(bar),
            "same statement, different name + proof → same hash"
        );
        assert_ne!(
            statement_hash(foo),
            statement_hash(baz),
            "a different `req` → a different statement hash"
        );
    }

    // AC-13: citing an UNCERTIFIED project lemma is refused, naming the lemma; a citation
    // to a certified project lemma (or a frozen spine lemma) resolves.
    #[test]
    fn uncertified_citation_is_refused_with_the_lemma_named() {
        let prog = parse_ok(
            "lemma melems_cons(n: u32) req n > 0 ens n >= 1 proof { omega }\n\
             lemma user(n: u32) req n > 0 ens n >= 1 proof { simp [melems_cons]; omega }",
        );
        // `melems_cons` did not certify (absent from the cert collection).
        let lib = LemmaLibrary::build(&prog, &[]);
        assert!(lib.is_project_lemma("melems_cons"));
        assert!(!lib.is_certified("melems_cons"));
        match enforce_citations(&lib, "user", "simp [melems_cons]; omega") {
            Err(e) => {
                assert_eq!(e.lemma, "melems_cons");
                assert_eq!(e.item, "user");
                assert!(e.detail().contains("melems_cons"), "named: {}", e.detail());
                assert_eq!(e.cause(), "UncertifiedLemmaCitation");
            }
            Ok(()) => panic!("expected an uncertified-citation refusal naming `melems_cons`"),
        }
        // Once `melems_cons` certifies, the same citation resolves (no refusal).
        let lib = LemmaLibrary::build(&prog, &[certified("melems_cons")]);
        assert!(lib.is_certified("melems_cons"));
        assert!(enforce_citations(&lib, "user", "simp [melems_cons]; omega").is_ok());
        assert_eq!(
            lib.resolve_citation("melems_cons"),
            CitationResolution::Certified {
                canonical: "melems_cons".to_string()
            }
        );
    }

    // A frozen spine simp lemma resolves `Frozen`; a name that is neither frozen nor a
    // project lemma is `Unknown` (left to the frozen-battery refusal).
    #[test]
    fn frozen_and_unknown_resolutions() {
        let lib = LemmaLibrary::default();
        assert_eq!(
            lib.resolve_citation("Thermite.intVal"),
            CitationResolution::Frozen
        );
        assert_eq!(
            lib.resolve_citation("not_a_lemma"),
            CitationResolution::Unknown
        );
        // `enforce_citations` only refuses `Uncertified` — a frozen / unknown citation
        // passes this gate (the battery handles `Unknown`).
        assert!(enforce_citations(&lib, "x", "simp [Thermite.intVal]; omega").is_ok());
        assert!(enforce_citations(&lib, "x", "simp [not_a_lemma]; omega").is_ok());
    }

    // AC-13: burning a statement-hash duplicate rewrites the citation to the canonical
    // lemma instead of storing a copy — TWO identical lemmas under different names → one
    // stored, the citation to the duplicate rewritten to the first (canonical).
    #[test]
    fn dedup_on_burn_rewrites_citation_to_canonical() {
        let prog = parse_ok(
            "lemma melems_cons(n: u32) req n > 0 ens n >= 1 proof { omega }\n\
             lemma melems_cons_dup(n: u32) req n > 0 ens n >= 1 proof { omega }\n\
             lemma user(n: u32) req n > 0 ens n >= 1 proof { simp [melems_cons_dup]; omega }",
        );
        // both duplicates certify; the first in source order (`melems_cons`) is canonical.
        let lib = LemmaLibrary::build(
            &prog,
            &[certified("melems_cons"), certified("melems_cons_dup")],
        );
        // Only one is stored (the canonical) — the duplicate dedups away.
        assert_eq!(
            lib.stored_lemmas(),
            vec!["melems_cons".to_string()],
            "two identical-statement lemmas store ONE canonical copy"
        );
        // A citation to the duplicate is rewritten to the canonical.
        assert_eq!(
            lib.canonical_name("melems_cons_dup"),
            Some("melems_cons"),
            "the duplicate's canonical dedup target is the first lemma"
        );
        assert_eq!(
            lib.rewrite_citations(&["melems_cons_dup".to_string()]),
            vec!["melems_cons".to_string()],
            "the burned citation is rewritten to the canonical, not stored as a copy"
        );
        // Citing both the canonical and the duplicate collapses to one (dedup in order).
        assert_eq!(
            lib.rewrite_citations(&["melems_cons".to_string(), "melems_cons_dup".to_string()]),
            vec!["melems_cons".to_string()]
        );
    }

    // Q6: only a top-level lemma enters the namespace — a name a `proof for f` establishes
    // locally is not a project lemma, so `proof for g` cannot cite it (no cross-function
    // sharing except via a lemma). The namespace is built from `Item::Forge(Lemma)` only.
    #[test]
    fn only_top_level_lemmas_are_in_the_namespace() {
        let prog = parse_ok(
            "fn f(n: u32) -> u32 req n > 0 ens result >= 1 fx pure { n }\n\
             lemma shared(n: u32) req n > 0 ens n >= 1 proof { omega }\n\
             proof for f { ens#0 by { exact local_fact } }",
        );
        let lib = LemmaLibrary::build(&prog, &[certified("shared")]);
        // A top-level lemma IS in the namespace (the cross-function shareable surface).
        assert!(lib.is_project_lemma("shared"));
        // A `proof for f`-local name is not — it never entered the namespace, so a citation
        // to it from anywhere resolves `Unknown` (refused by the frozen battery), never as a
        // shareable project lemma. This is Q6 enforced structurally.
        assert!(!lib.is_project_lemma("local_fact"));
        assert_eq!(
            lib.resolve_citation("local_fact"),
            CitationResolution::Unknown
        );
        assert_eq!(
            project_lemma_names(&prog),
            BTreeSet::from(["shared".to_string()]),
            "only the top-level lemma is a project-lemma name"
        );
    }
}
