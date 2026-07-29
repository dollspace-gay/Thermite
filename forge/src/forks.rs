//! `forge/src/forks.rs` — the "semantic forks and definition towers" review/audit
//! section (`.design/stage3-bv-reconstruction.md` REQ-6 / AC-7, issue #348). The
//! human-audit surface for the two legibility risks the forge tier and the `@bv` machine-
//! semantics tag add to the program:
//!
//! 1. **semantic forks** — `@bv`-tagged clauses are interpreted over a fixed-width
//!    machine-semantics fork, not the default unbounded integers. Lock 1 (REQ-3) already
//!    makes each tagged clause loud + greppable (its `bv_shadow` block, surfaced per
//!    clause in `forge audit`/`forge review`). This section is the aggregate over those
//!    clauses: **bv-shadow density per module** — how much of each contract-bearing
//!    item's postcondition surface has committed to a machine-semantics fork.
//! 2. **definition towers** — a forge-tier `lemma`'s `req ∪ ens` can unfold a tower of
//!    `spec fn` definitions an auditor must read through (the REQ-6c anti-Goodhart risk
//!    `meaning.rs` bounds for `fn`s). This section reports the **tower depth for every
//!    burned lemma**, so the project's proven-lemma library is legible at a glance.
//!
//! ## Why this section is the POST-SHIP F-F tripwire (D-BVSCOPE)
//!
//! Q-BVSCOPE asked whether to ship the `@bv` tag full, `nowrap`-only, or lemma-only. The
//! "measure bv-shadow density first" input was circular — no bv clause exists to measure
//! until the tag ships. So the resolution (`.design/stage3-bv-reconstruction.md` Decision
//! Record) ships the full tag guarded by its three locks and makes this density report the
//! *post-ship* retreat trigger: rising shadow-flag density in contract-bearing code is the
//! named **F-F tripwire** down the ladder (full → `nowrap`-only → lemma-only → drop). The
//! tripwire fires when the project's bv-shadow density crosses
//! [`FF_DENSITY_THRESHOLD_PERMILLE`] — a loud, informational warning that the program is
//! becoming dominated by machine-semantics forks and the retreat ladder should be weighed.
//!
//! ## A pure projection that gates nothing
//!
//! Like `forge audit` itself (#274 "audit gates nothing") and the `--meaning` companion,
//! this section is a deterministic projection of the settled cert collection + the parsed
//! program (R-CODE-5): it re-runs no prover, changes no verdict, and alters no exit code.
//! The F-F warning is a human signal, not a gate. Density is a parse-level fact (which
//! clauses carry the `@bv` tag), the tower depth reuses `meaning::tower_metrics` (the same
//! spec-fn meaning closure `forge audit --meaning` reports), and which lemmas are "burned"
//! is read from the certificates (mirroring `review::burned_lemma_projection`).

use serde::{Deserialize, Serialize};
use thermite_syntax::{Expr, Item, Program};

use crate::manifest::{cert_certifies, Certificate};

/// The F-F retreat-trigger threshold (REQ-6 / AC-7): the project-wide bv-shadow density,
/// in per-MILLE of the contract-bearing postcondition surface, at or above which the named
/// F-F tripwire fires. `500‰` (half) is the Schelling point — when a MAJORITY of the
/// project's `ens` clauses have committed to a fixed-width machine-semantics fork, the
/// program has drifted far enough toward the fork that the retreat ladder (full →
/// `nowrap`-only → lemma-only → drop) should be weighed. Per-mille (not percent) so a
/// modest density still resolves to a stable integer (`Eq`-safe — no `f64`), keeping the
/// section a byte-deterministic projection.
pub const FF_DENSITY_THRESHOLD_PERMILLE: u32 = 500;

/// The "semantic forks and definition towers" section (REQ-6 / AC-7) — the additive,
/// read-only audit/review surface for the forge + `@bv` legibility risks. A pure
/// projection ([`SemanticForks::build`]); present only when the project has a semantic
/// fork or a burned lemma to report (so the v1 / non-bv corpus omits it and its goldens
/// stay byte-identical — the `bv_shadows`/`burned_lemmas` additive discipline).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticForks {
    /// The bv-shadow density per module (one row per contract-bearing item with ≥1 `ens`
    /// clause, in source order) — the semantic-fork legibility surface.
    pub bv_density: Vec<ModuleDensity>,
    /// The definition-tower depth of every burned lemma (in cert order) — the
    /// definition-tower legibility surface.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub burned_lemma_towers: Vec<LemmaTower>,
    /// The project-wide F-F density tripwire (the post-ship retreat trigger).
    pub tripwire: FfTripwire,
}

/// One module's bv-shadow density (REQ-6 / AC-7) — a contract-bearing item (`fn` /
/// `lemma`) and how much of its `ens` postcondition surface is a machine-semantics fork.
/// Denominated by the `ens` clauses because the `@bv` tag attaches only to `ens` (and a
/// lemma's `ens`) — the taggable surface (`check::fn_has_bv_tag` keys on `contract.ens`).
/// A pure parse-level fact (which clauses carry the `@bv` tag); never recomputed from a
/// verdict (R-CODE-5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleDensity {
    /// The module — the `fn` / `lemma` item name.
    pub module: String,
    /// The number of `@bv`-tagged `ens` clauses (the machine-semantics forks).
    pub shadow_clauses: usize,
    /// The total `ens` clauses (the taggable postcondition surface; the denominator).
    pub contract_clauses: usize,
    /// The density in per-mille: `shadow_clauses * 1000 / contract_clauses` (an item with
    /// at least one `ens` always has a non-zero denominator, so this never divides by
    /// zero — items with no `ens` clause are not listed). Integer (`Eq`-safe).
    pub density_permille: u32,
}

/// One burned lemma's definition-tower depth (REQ-6 / AC-7) — a certified forge-tier
/// `lemma` (the `review::BurnedLemma` set) and the depth + size of the `spec fn` tower its
/// `req ∪ ens` unfolds (`meaning::tower_metrics`, the same meaning closure
/// `forge audit --meaning` reports for `fn`s). A scalar lemma (no `spec fn` reference) has
/// depth `0`. Pure (R-CODE-5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LemmaTower {
    /// The burned lemma's name.
    pub lemma: String,
    /// The tower depth — the longest distinct-definition `spec fn` unfolding chain rooted
    /// at the lemma's `req ∪ ens` (`0` for a scalar contract).
    pub depth: usize,
    /// The number of distinct `spec fn` definitions the tower reaches.
    pub definitions: usize,
}

/// The project-wide F-F density tripwire (REQ-6 / AC-7) — the post-ship retreat trigger.
/// Aggregates the bv-shadow density across all contract-bearing `ens` clauses and compares
/// it to [`FF_DENSITY_THRESHOLD_PERMILLE`]. Informational: a tripped tripwire gates
/// nothing (it changes no verdict and no exit code), it raises a named human warning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FfTripwire {
    /// The project total of `@bv`-tagged `ens` clauses (machine-semantics forks).
    pub shadow_clauses: usize,
    /// The project total of `ens` clauses (the contract-bearing postcondition surface).
    pub contract_clauses: usize,
    /// The project-wide density in per-mille (`0` when there is no `ens` surface).
    pub density_permille: u32,
    /// The threshold the density is compared against ([`FF_DENSITY_THRESHOLD_PERMILLE`]).
    pub threshold_permille: u32,
    /// `true` iff the density is at or above the threshold — the F-F tripwire fired.
    pub tripped: bool,
    /// The named F-F warning, present iff `tripped` (`#[serde(skip_serializing_if)]`). The
    /// auditor's one-line "the program is fork-dominated; weigh the retreat ladder".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

impl SemanticForks {
    /// Build the section from the settled cert collection + the parsed program (REQ-6 /
    /// AC-7), or `None` when there is nothing to report — no `@bv`-tagged clause and no
    /// burned lemma (the v1 / non-bv corpus, whose goldens stay byte-identical). A pure
    /// projection: density is a parse-level fact, the burned-lemma set is read from the
    /// certificates (the `review::burned_lemma_projection` predicate), and the tower depth
    /// reuses `meaning::tower_metrics`. No prover, no wall-clock (R-CODE-5).
    #[must_use]
    pub fn build(certs: &[Certificate], program: &Program) -> Option<Self> {
        let bv_density = module_densities(program);
        let burned_lemma_towers = burned_lemma_towers(certs, program);
        let tripwire = FfTripwire::from_densities(&bv_density);

        // Present only when there is a semantic fork (≥1 tagged clause) or a burned lemma
        // to report — otherwise omitted, so the v1 corpus serializes byte-identically.
        if tripwire.shadow_clauses == 0 && burned_lemma_towers.is_empty() {
            return None;
        }
        Some(SemanticForks {
            bv_density,
            burned_lemma_towers,
            tripwire,
        })
    }

    /// Render the section as human-readable text (the `forge audit` / `forge review` body,
    /// REQ-6). Read-only: this prints; it gates nothing.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("semantic forks and definition towers (the F-F tripwire):\n");

        out.push_str("  bv-shadow density per module:\n");
        if self.bv_density.is_empty() {
            out.push_str("    (no contract-bearing modules)\n");
        }
        for row in &self.bv_density {
            out.push_str(&format!(
                "    {}: {}/{} ens clauses machine-semantics ({}‰)\n",
                row.module, row.shadow_clauses, row.contract_clauses, row.density_permille
            ));
        }

        out.push_str("  burned-lemma definition-tower depths:\n");
        if self.burned_lemma_towers.is_empty() {
            out.push_str("    (no burned lemmas)\n");
        }
        for row in &self.burned_lemma_towers {
            out.push_str(&format!(
                "    {}: depth {}, {} definitions\n",
                row.lemma, row.depth, row.definitions
            ));
        }

        match &self.tripwire.warning {
            Some(warning) => out.push_str(&format!("  {warning}\n")),
            None => out.push_str(&format!(
                "  F-F tripwire: {}‰ bv-shadow density in contract-bearing code is WITHIN the \
                 {}‰ retreat threshold.\n",
                self.tripwire.density_permille, self.tripwire.threshold_permille
            )),
        }
        out
    }
}

impl FfTripwire {
    /// Aggregate the per-module densities into the project-wide F-F tripwire (REQ-6 /
    /// AC-7). Sums the tagged + total `ens` clauses across every module, computes the
    /// per-mille density, and fires the named warning iff it reaches the threshold.
    fn from_densities(densities: &[ModuleDensity]) -> Self {
        let shadow_clauses: usize = densities.iter().map(|d| d.shadow_clauses).sum();
        let contract_clauses: usize = densities.iter().map(|d| d.contract_clauses).sum();
        let density_permille = permille(shadow_clauses, contract_clauses);
        let tripped = density_permille >= FF_DENSITY_THRESHOLD_PERMILLE;
        let warning = tripped.then(|| {
            format!(
                "F-F tripwire TRIPPED: {density_permille}‰ bv-shadow density in contract-bearing \
                 code ({shadow_clauses}/{contract_clauses} ens clauses) reaches the \
                 {FF_DENSITY_THRESHOLD_PERMILLE}‰ retreat threshold — the program is becoming \
                 dominated by fixed-width machine-semantics forks; weigh the @bv retreat ladder \
                 (full → nowrap-only → lemma-only → drop). Informational (gates nothing)."
            )
        });
        FfTripwire {
            shadow_clauses,
            contract_clauses,
            density_permille,
            threshold_permille: FF_DENSITY_THRESHOLD_PERMILLE,
            tripped,
            warning,
        }
    }
}

/// The bv-shadow density of every contract-bearing module (REQ-6) — a `fn` / `lemma` with
/// ≥1 `ens` clause, in source order. A pure parse-level projection: the numerator counts
/// the `@bv`-tagged `ens` clauses (`Clause.bv.is_some()`), the denominator the total `ens`
/// clauses. An item with no `ens` clause (a `req`-only `fn`) has no taggable surface and is
/// not listed.
fn module_densities(program: &Program) -> Vec<ModuleDensity> {
    program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(f) => density_row(&f.name, &f.contract.ens),
            Item::Forge(thermite_syntax::ForgeItem::Lemma(l)) => density_row(&l.name, &l.ens),
            _ => None,
        })
        .collect()
}

/// One module's density row from its `ens` clauses (REQ-6), or `None` when the item has no
/// `ens` clause (no taggable postcondition surface).
fn density_row(name: &str, ens: &[thermite_syntax::Clause]) -> Option<ModuleDensity> {
    if ens.is_empty() {
        return None;
    }
    let shadow_clauses = ens.iter().filter(|c| c.bv.is_some()).count();
    let contract_clauses = ens.len();
    Some(ModuleDensity {
        module: name.to_string(),
        shadow_clauses,
        contract_clauses,
        density_permille: permille(shadow_clauses, contract_clauses),
    })
}

/// The definition-tower depth of every burned lemma (REQ-6) — a certified forge-tier
/// `lemma` carrying a burn receipt (the `review::burned_lemma_projection` predicate),
/// projected to its `req ∪ ens` `spec fn` tower depth via `meaning::tower_metrics`. In
/// cert order (deterministic, R-CODE-5).
fn burned_lemma_towers(certs: &[Certificate], program: &Program) -> Vec<LemmaTower> {
    let mut rows = Vec::new();
    for cert in certs {
        // Mirror `review::burned_lemma_projection`: the cert's item is a top-level
        // `lemma`, it certified (`cert_certifies`), and it carries a burn receipt.
        let Some(lemma) = program.items.iter().find_map(|i| match i {
            Item::Forge(thermite_syntax::ForgeItem::Lemma(l)) if l.name == cert.item => Some(l),
            _ => None,
        }) else {
            continue;
        };
        if !cert_certifies(cert) || cert.burn.is_none() {
            continue;
        }
        // The tower roots: the lemma's `req ∪ ens` (the meaning surface), as
        // `meaning::build_tower` roots a `fn`'s tower.
        let mut roots: Vec<&Expr> = Vec::with_capacity(1 + lemma.ens.len());
        roots.push(&lemma.req.expr);
        roots.extend(lemma.ens.iter().map(|c| &c.expr));
        let (depth, definitions) = crate::meaning::tower_metrics(program, &roots);
        rows.push(LemmaTower {
            lemma: cert.item.clone(),
            depth,
            definitions,
        });
    }
    rows
}

/// `numerator * 1000 / denominator`, in per-mille; `0` when the denominator is `0` (no
/// contract surface). Integer arithmetic — `Eq`-safe and byte-deterministic (R-CODE-5).
fn permille(numerator: usize, denominator: usize) -> u32 {
    if denominator == 0 {
        return 0;
    }
    u32::try_from(numerator * 1000 / denominator).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::burn::BurnReceipt;
    use crate::manifest::{Level, ObligationResult};

    fn parse(src: &str) -> Program {
        let parsed = thermite_syntax::parse(src);
        assert!(
            parsed.is_clean(),
            "fixture must parse clean: {:?}",
            parsed.errors
        );
        parsed.program
    }

    // AC-7: bv-shadow density per module — known counts. `mix64` carries two `@bv64` ens
    // clauses + one unbounded ens clause (2/3); the injectivity lemma one `@bv64` clause
    // (1/1). The numbers match the fixture's known counts exactly (a pure parse-level
    // projection; no verus needed). The `bv` feature gates the tag's parse (REQ-1).
    #[cfg(feature = "bv")]
    #[test]
    fn bv_density_per_module_matches_known_counts() {
        let program = parse(include_str!("../../conformance/forge/mix64.th"));
        let densities = module_densities(&program);
        let row = |name: &str| densities.iter().find(|d| d.module == name).cloned();

        let mix = row("mix64").expect("mix64 is a contract-bearing module");
        assert_eq!(mix.shadow_clauses, 2, "two @bv64 ens clauses");
        assert_eq!(mix.contract_clauses, 3, "three ens clauses total");
        assert_eq!(mix.density_permille, 666, "2/3 → 666‰");

        let lemma = row("rotl1_injective").expect("the lemma is a contract-bearing module");
        assert_eq!(lemma.shadow_clauses, 1);
        assert_eq!(lemma.contract_clauses, 1);
        assert_eq!(lemma.density_permille, 1000, "1/1 → 1000‰");
    }

    // AC-7: a synthetic density spike trips the named F-F warning, and a normal density
    // does not. Built from raw `ModuleDensity` rows (no parse needed — the tripwire is a
    // pure aggregate). The named warning names the F-F retreat ladder.
    #[test]
    fn density_spike_trips_the_named_ff_warning() {
        // Normal: one tagged clause among five contract clauses (200‰ < 500‰) — no trip.
        let normal = vec![
            ModuleDensity {
                module: "wrap_one".to_string(),
                shadow_clauses: 1,
                contract_clauses: 1,
                density_permille: 1000,
            },
            ModuleDensity {
                module: "plain".to_string(),
                shadow_clauses: 0,
                contract_clauses: 4,
                density_permille: 0,
            },
        ];
        let tw = FfTripwire::from_densities(&normal);
        assert_eq!(tw.shadow_clauses, 1);
        assert_eq!(tw.contract_clauses, 5);
        assert_eq!(tw.density_permille, 200);
        assert!(!tw.tripped, "200‰ is within the 500‰ threshold");
        assert!(tw.warning.is_none(), "no warning when within threshold");

        // Spike: the machine-semantics forks become the majority (4/5 = 800‰ ≥ 500‰).
        let spike = vec![
            ModuleDensity {
                module: "fork_heavy".to_string(),
                shadow_clauses: 4,
                contract_clauses: 4,
                density_permille: 1000,
            },
            ModuleDensity {
                module: "plain".to_string(),
                shadow_clauses: 0,
                contract_clauses: 1,
                density_permille: 0,
            },
        ];
        let tw = FfTripwire::from_densities(&spike);
        assert_eq!(tw.density_permille, 800);
        assert!(tw.tripped, "800‰ reaches the 500‰ threshold");
        let warning = tw
            .warning
            .expect("a tripped tripwire carries the named warning");
        assert!(warning.contains("F-F tripwire TRIPPED"), "{warning}");
        assert!(
            warning.contains("full → nowrap-only → lemma-only → drop"),
            "the warning names the retreat ladder: {warning}"
        );
    }

    // AC-7: the tripwire fires exactly at the threshold (500‰ is "reaches", not "exceeds").
    #[test]
    fn tripwire_fires_at_the_threshold_boundary() {
        let at = vec![ModuleDensity {
            module: "half".to_string(),
            shadow_clauses: 1,
            contract_clauses: 2,
            density_permille: 500,
        }];
        let tw = FfTripwire::from_densities(&at);
        assert_eq!(tw.density_permille, 500);
        assert!(tw.tripped, "exactly 500‰ trips (>= threshold)");
    }

    // AC-7: burned-lemma tower depth. A burned lemma whose `ens` references a `spec fn`
    // chain reports that chain's depth; the section reads which lemmas burned from the
    // certs (a certified lemma with a burn receipt) — an uncertified lemma is not a tower.
    #[test]
    fn burned_lemma_tower_depth_follows_the_spec_fn_chain() {
        let program = parse(
            "spec fn a(x: u32) -> bool dec x { b(x) }\n\
             spec fn b(x: u32) -> bool dec x { x > 0 }\n\
             lemma deep(x: u32) req true ens a(x) proof { }",
        );
        let burned = Certificate::new(
            "deep",
            Level::L3,
            vec!["pure".to_string()],
            0,
            vec![ObligationResult::discharged("deep")],
        )
        .graduate_triage_clean()
        .with_burn(BurnReceipt::for_proof_text("trivial"));
        let towers = burned_lemma_towers(&[burned], &program);
        assert_eq!(towers.len(), 1, "the certified burned lemma is a tower");
        assert_eq!(towers[0].lemma, "deep");
        assert_eq!(towers[0].depth, 2, "a → b is a 2-deep spec-fn chain");
        assert_eq!(towers[0].definitions, 2);
    }

    // AC-7: an uncertified lemma (no burn) is not surfaced as a tower — only a burned
    // (certified, receipted) lemma is, mirroring `review::burned_lemma_projection`.
    #[test]
    fn uncertified_lemma_is_not_a_tower() {
        let program = parse("lemma bad(x: u32) req true ens x >= 0 proof { }");
        let rejected = Certificate::rejected(
            "bad".to_string(),
            vec!["pure".to_string()],
            false,
            crate::manifest::RejectReason {
                cause: "LeanUnknown".to_string(),
                detail: "did not discharge".to_string(),
            },
        );
        let towers = burned_lemma_towers(&[rejected], &program);
        assert!(
            towers.is_empty(),
            "an uncertified lemma is not a burned tower"
        );
    }

    // The section is omitted (None) for a tag-free, lemma-free program (the v1 corpus):
    // the additive discipline that keeps v1 goldens byte-identical.
    #[test]
    fn section_omitted_for_v1_corpus() {
        let program = parse("fn f(x: u32) -> u32 req x < 100 ens result == x fx pure { x }");
        let cert = Certificate::new("f", Level::L3, vec!["pure".to_string()], 0, vec![]);
        assert!(
            SemanticForks::build(&[cert], &program).is_none(),
            "a tag-free, lemma-free program carries no semantic-forks section"
        );
    }
}
