//! SPIKE-2 — the normalizer-probe fixture generator + hit-rate target
//! (`.design/m0-spikes.md` REQ-5 / REQ-7).
//!
//! This is the spike's bin target (REQ-7: "computed by a test/bin target").
//! Running it
//!
//! ```text
//! cargo run -p thermite-tv --example strat_probe
//! ```
//!
//! (re)materializes the committed fixture set under
//! `thermite-tv/tests/fixtures/strat_probe/` — one file per production/reference
//! instance pair — writes the fixtures `README.md`, and prints the hit-rate
//! report (corpus-only n=4 + corpus+generated + per-shape breakdown + the
//! decision-rule branch). It lives in `examples/` (not `src/`) so the AC-6 grep
//! over `thermite-tv/src/` finds no `normalize` consumer; an example is spike
//! scaffolding, not a TV pipeline code path.
//!
//! ## Why two hand-written spellings per shape
//!
//! Neither emitter produces stratified raw-quantifier forms yet, so the two
//! columns are hand-written approximations of the conventions each emitter uses
//! today (`.design/m0-spikes.md` Architecture):
//!
//! - production-style mimics `thermite-lower/src/lower.rs` inlining the
//!   `thermite_spec::CombinatorSig.verus_l3` body — chained comparisons
//!   (`0 <= i <= j < len(s)`), `i`/`j` binders, the spec-context `idx(s,i)` /
//!   `len(s)` accessors;
//! - reference-style mimics `lean/Thermite/RefEncode.lean`'s independent
//!   encoding — alpha-renamed binders, explicit split-and-reordered antecedent
//!   conjuncts, flipped atom orientation, nested quantifiers.
//!
//! The differences are the surface noise the four layer-1 passes are
//! designed to canonicalize, and the mimicry biases the measured rate downward
//! (real stage-2 emitters can be nudged toward convergence), so a high measured
//! rate is trustworthy evidence (fallback F-C, the safe asymmetry).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use thermite_syntax::ast::{BinOp, Expr, PrimType, Type};
use thermite_tv::gen::generate_clauses;
use thermite_tv::normalize;

/// The bounded-quantifier combinator shapes the probe covers. These are the six
/// of the eight frozen registry combinators that have a layer-1 raw-quantifier
/// expansion (RefEncode.lean's "6 bounded-quantifier combinators"). The other two
/// — `count_where` (a recursive `nat` fold) and `permutation_of` (a multiset
/// equality) — have no raw-quantifier form, so they are out of layer-1 scope and
/// excluded (documented in the README), not counted as misses.
const SHAPES: &[&str] = &[
    "sorted",
    "forall_in",
    "forall_below",
    "forall_from",
    "exists_in",
    "disjoint",
];

/// A predicate-closure body `|x| <lhs> <op> <rhs>`, the `Pred` argument of a
/// combinator. `cast` is the optional `as <ty>` on the bound element.
#[derive(Debug, Clone)]
struct Pred {
    cast: Option<String>,
    op: &'static str,
    rhs: String,
}

impl Pred {
    /// Render the predicate applied to an element TERM (`x` ↦ `elem`).
    fn apply(&self, elem: &str) -> String {
        // No parens around the cast: `idx(s,i) as u32 < 9` — `parse_term` binds the
        // `as` tighter than the comparison, so an outer paren would be misread as a
        // formula group (the leading `(` is formula grouping in `parse_unary`).
        let lhs = match &self.cast {
            Some(ty) => format!("{elem} as {ty}"),
            None => elem.to_string(),
        };
        format!("{lhs} {} {}", self.op, self.rhs)
    }
}

/// One combinator instance — the data both expansion templates are applied to.
#[derive(Debug, Clone)]
struct Instance {
    shape: String,
    /// The fixture source designation (REQ-5): a scheme-validated address for the
    /// `inv` corpus clauses, an informal designation for `req`/`ens`, and a
    /// `gen#<seed>.<k>` tag for generator-drawn instances.
    source: String,
    /// `corpus` or `generated`.
    origin: &'static str,
    slice_a: String,
    slice_b: Option<String>,
    index: Option<String>,
    pred: Option<Pred>,
}

/// Apply the shape's two raw-quantifier templates to an instance, returning
/// `(production_text, reference_text)`. The two spellings are semantically
/// equivalent; their differences exercise the four layer-1 passes.
///
/// Binder discipline: the reference-style binders (`a`/`b`, `w`, `p`/`q`) are
/// chosen disjoint from the generator's free-variable vocabulary (`gen.rs` draws
/// slices `xs`/`ys` and indices from `n`/`m`/`k`), and the production-style
/// binders (`i`/`j`) likewise. A binder that coincided with a free index variable
/// (`forall k . … k < k …` when the index is `k`) would be a genuine capture (the
/// two spellings would then not be equivalent), so the probe avoids it.
fn templates(inst: &Instance) -> (String, String) {
    let s = &inst.slice_a;
    match inst.shape.as_str() {
        // forall|i,j| 0 <= i <= j < s.len() ==> s[i] <= s[j]
        "sorted" => (
            format!("forall i j . 0 <= i <= j < len({s}) => idx({s}, i) <= idx({s}, j)"),
            // alpha-rename (b,a) + nested, conjuncts reordered, consequent flipped.
            format!("forall b a . (b < len({s}) & a <= b & 0 <= a) => idx({s}, b) >= idx({s}, a)"),
        ),
        // forall|i| 0 <= i < s.len() ==> p(s[i])
        "forall_in" => {
            let p = inst.pred.as_ref().expect("forall_in needs a pred");
            (
                format!(
                    "forall i . 0 <= i < len({s}) => {}",
                    p.apply(&format!("idx({s}, i)"))
                ),
                format!(
                    "forall w . (w < len({s}) & 0 <= w) => {}",
                    p.apply(&format!("idx({s}, w)"))
                ),
            )
        }
        // forall|i| 0 <= i < n && i < s.len() ==> p(s[i])
        "forall_below" => {
            let p = inst.pred.as_ref().expect("forall_below needs a pred");
            let n = inst.index.as_ref().expect("forall_below needs an index");
            (
                format!(
                    "forall i . 0 <= i < {n} & i < len({s}) => {}",
                    p.apply(&format!("idx({s}, i)"))
                ),
                format!(
                    "forall w . (w < len({s}) & w < {n} & 0 <= w) => {}",
                    p.apply(&format!("idx({s}, w)"))
                ),
            )
        }
        // forall|i| n <= i < s.len() ==> p(s[i])
        "forall_from" => {
            let p = inst.pred.as_ref().expect("forall_from needs a pred");
            let n = inst.index.as_ref().expect("forall_from needs an index");
            (
                format!(
                    "forall i . {n} <= i < len({s}) => {}",
                    p.apply(&format!("idx({s}, i)"))
                ),
                format!(
                    "forall w . (w < len({s}) & {n} <= w) => {}",
                    p.apply(&format!("idx({s}, w)"))
                ),
            )
        }
        // exists|i| 0 <= i < s.len() && p(s[i])
        "exists_in" => {
            let p = inst.pred.as_ref().expect("exists_in needs a pred");
            (
                format!(
                    "exists i . 0 <= i & i < len({s}) & {}",
                    p.apply(&format!("idx({s}, i)"))
                ),
                format!(
                    "exists w . {} & w < len({s}) & 0 <= w",
                    p.apply(&format!("idx({s}, w)"))
                ),
            )
        }
        // forall|i,j| (0<=i<a.len() & 0<=j<b.len()) ==> a[i] != b[j]
        "disjoint" => {
            let a = s;
            let b = inst
                .slice_b
                .as_ref()
                .expect("disjoint needs a second slice");
            (
                format!(
                    "forall i j . (0 <= i & i < len({a}) & 0 <= j & j < len({b})) => idx({a}, i) != idx({b}, j)"
                ),
                // binders swapped/renamed, conjuncts reordered, != operands swapped.
                format!(
                    "forall q p . (q < len({b}) & 0 <= q & p < len({a}) & 0 <= p) => idx({b}, q) != idx({a}, p)"
                ),
            )
        }
        other => panic!("unknown shape {other}"),
    }
}

// ---- the 4 corpus instances (binary_search.th) ----------------------------

fn corpus_instances() -> Vec<Instance> {
    vec![
        // req sorted(haystack)
        Instance {
            shape: "sorted".into(),
            source: "binary_search.req".into(),
            origin: "corpus",
            slice_a: "haystack".into(),
            slice_b: None,
            index: None,
            pred: None,
        },
        // ens match None => forall_in(haystack, |x| x != needle)
        Instance {
            shape: "forall_in".into(),
            source: "binary_search.ens.None".into(),
            origin: "corpus",
            slice_a: "haystack".into(),
            slice_b: None,
            index: None,
            pred: Some(Pred {
                cast: None,
                op: "!=",
                rhs: "needle".into(),
            }),
        },
        // inv forall_below(haystack, lo, |x| x < needle)  — loop#1.inv#2
        Instance {
            shape: "forall_below".into(),
            source: "binary_search.loop#1.inv#2".into(),
            origin: "corpus",
            slice_a: "haystack".into(),
            slice_b: None,
            index: Some("lo".into()),
            pred: Some(Pred {
                cast: None,
                op: "<",
                rhs: "needle".into(),
            }),
        },
        // inv forall_from(haystack, hi, |x| x > needle)  — loop#1.inv#3
        Instance {
            shape: "forall_from".into(),
            source: "binary_search.loop#1.inv#3".into(),
            origin: "corpus",
            slice_a: "haystack".into(),
            slice_b: None,
            index: Some("hi".into()),
            pred: Some(Pred {
                cast: None,
                op: ">",
                rhs: "needle".into(),
            }),
        },
    ]
}

// ---- generator-drawn instances (via the public gen::generate_clauses) ------

/// The minimum number of distinct generator-drawn instances per shape (REQ-5:
/// "≥ 5 per shape"). Drawn to comfortably clear the n ≥ ~30 threshold.
const PER_SHAPE: usize = 6;

fn cmp_token(op: BinOp) -> Option<&'static str> {
    Some(match op {
        BinOp::Eq => "=",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        _ => return None,
    })
}

fn ty_token(t: &Type) -> Option<String> {
    Some(match t {
        Type::Prim(PrimType::U32) => "u32".into(),
        Type::Prim(PrimType::U64) => "u64".into(),
        Type::Prim(PrimType::Usize) => "usize".into(),
        Type::Named(n) => n.clone(),
        _ => return None,
    })
}

/// Render a generated index/scalar `Expr` into the probe's surface term syntax,
/// or `None` if it uses a construct outside the probe's term grammar.
fn render_term(e: &Expr) -> Option<String> {
    match e {
        Expr::Path(segs) if segs.len() == 1 => Some(segs[0].clone()),
        Expr::IntLit { value, .. } => Some(value.to_string()),
        Expr::Binary { op, lhs, rhs } => {
            let t = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                _ => return None,
            };
            Some(format!(
                "({} {} {})",
                render_term(lhs)?,
                t,
                render_term(rhs)?
            ))
        }
        Expr::Cast { expr, ty } => Some(format!("({} as {})", render_term(expr)?, ty_token(ty)?)),
        _ => None,
    }
}

/// Extract a [`Pred`] from a generated predicate closure body (a comparison
/// `x <op> <lit>`, the element possibly cast). `None` if the body is not a shape
/// the probe renders.
fn pred_from_closure(body: &Expr) -> Option<Pred> {
    let Expr::Binary { op, lhs, rhs } = body else {
        return None;
    };
    let op = cmp_token(*op)?;
    // The lhs is the bound element `x`, optionally cast `(x as u32)`.
    let cast = match lhs.as_ref() {
        Expr::Path(segs) if segs.len() == 1 && segs[0] == "x" => None,
        Expr::Cast { expr, ty } => match expr.as_ref() {
            Expr::Path(segs) if segs.len() == 1 && segs[0] == "x" => Some(ty_token(ty)?),
            _ => return None,
        },
        _ => return None,
    };
    let rhs = render_term(rhs)?;
    Some(Pred { cast, op, rhs })
}

/// Recursively collect every combinator-call subexpression of a covered shape
/// from a generated clause, turning each into an [`Instance`] (tagged with the
/// draw `source`). Returns nothing for the `count_where`/`permutation_of` calls
/// (not covered) and for any call whose args fall outside the term grammar.
fn collect_instances(e: &Expr, source: &str, out: &mut Vec<Instance>) {
    if let Expr::Call { callee, args } = e {
        if let Expr::Path(segs) = callee.as_ref() {
            if segs.len() == 1 {
                if let Some(inst) = instance_from_call(&segs[0], args, source) {
                    out.push(inst);
                }
            }
        }
    }
    // Recurse into every child so nested combinator calls are also collected.
    for child in children(e) {
        collect_instances(child, source, out);
    }
}

fn instance_from_call(name: &str, args: &[Expr], source: &str) -> Option<Instance> {
    if !SHAPES.contains(&name) {
        return None;
    }
    let slice = |e: &Expr| match e {
        Expr::Path(segs) if segs.len() == 1 => Some(segs[0].clone()),
        _ => None,
    };
    let pred = |e: &Expr| match e {
        Expr::Closure { params, body } if params == &["x".to_string()] => pred_from_closure(body),
        _ => None,
    };
    let mk = |slice_a, slice_b, index, p| {
        Some(Instance {
            shape: name.to_string(),
            source: source.to_string(),
            origin: "generated",
            slice_a,
            slice_b,
            index,
            pred: p,
        })
    };
    match name {
        "sorted" => mk(slice(args.first()?)?, None, None, None),
        "forall_in" | "exists_in" => {
            mk(slice(args.first()?)?, None, None, Some(pred(args.get(1)?)?))
        }
        "forall_below" | "forall_from" => mk(
            slice(args.first()?)?,
            None,
            Some(render_term(args.get(1)?)?),
            Some(pred(args.get(2)?)?),
        ),
        "disjoint" => mk(
            slice(args.first()?)?,
            Some(slice(args.get(1)?)?),
            None,
            None,
        ),
        _ => None,
    }
}

/// The direct sub-expressions of `e` (for the recursive collector).
fn children(e: &Expr) -> Vec<&Expr> {
    match e {
        Expr::Binary { lhs, rhs, .. } => vec![lhs, rhs],
        Expr::Unary { expr, .. } => vec![expr],
        Expr::Call { args, .. } => args.iter().collect(),
        Expr::Closure { body, .. } => vec![body],
        Expr::Cast { expr, .. } => vec![expr],
        _ => Vec::new(),
    }
}

/// Draw [`PER_SHAPE`] generator instances per covered shape from
/// `gen::generate_clauses`, walking increasing seeds deterministically (no new
/// generator productions — REQ-5 / Out of Scope). Each draw is a real distinct
/// generator occurrence (unique `gen#<seed>.<k>#<occ>` provenance). The sample
/// takes distinct rendered instances first (ordered by first appearance),
/// then — for the low-vocabulary shapes whose distinct-instance count the
/// generator caps below `PER_SHAPE` (`sorted` takes only a slice ∈ {xs, ys} → 2;
/// `disjoint` a slice pair → 4) — pads to `PER_SHAPE` with further real draws, so
/// "≥ 5 generator-drawn instances per shape" holds literally even where the
/// rendered pair necessarily repeats (noted in the README).
fn generated_instances() -> Vec<Instance> {
    // All generator occurrences per shape, in draw order (with unique sources).
    let mut all: BTreeMap<String, Vec<Instance>> = BTreeMap::new();
    let mut occ: usize = 0;
    let mut seed: u64 = 1;
    loop {
        let clauses = generate_clauses(seed, 64);
        for (k, clause) in clauses.iter().enumerate() {
            let mut found = Vec::new();
            collect_instances(clause, &format!("gen#{seed}.{k}"), &mut found);
            for mut inst in found {
                inst.source = format!("{}#{occ}", inst.source);
                occ += 1;
                all.entry(inst.shape.clone()).or_default().push(inst);
            }
        }
        let enough = SHAPES
            .iter()
            .all(|s| all.get(*s).map(|v| v.len()).unwrap_or(0) >= PER_SHAPE);
        if enough || seed > 4000 {
            break;
        }
        seed += 1;
    }

    let mut out = Vec::new();
    for s in SHAPES {
        let bucket = all
            .get(*s)
            .unwrap_or_else(|| panic!("shape {s} drew no generator instances"));
        assert!(
            bucket.len() >= PER_SHAPE,
            "shape {s}: only {} generator draws (< {PER_SHAPE})",
            bucket.len()
        );
        // Distinct rendered pairs first (ordered by first appearance), then pad
        // with further real draws to reach PER_SHAPE.
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut distinct = Vec::new();
        let mut rest = Vec::new();
        for inst in bucket {
            let (prod, _) = templates(inst);
            if seen.insert(prod) {
                distinct.push(inst.clone());
            } else {
                rest.push(inst.clone());
            }
        }
        let mut sample = distinct;
        sample.extend(rest);
        out.extend(sample.into_iter().take(PER_SHAPE));
    }
    out
}

// ---- fixture files + README + report ---------------------------------------

/// A normalized fixture pair, ready to write + tally.
struct Pair {
    inst: Instance,
    production: String,
    reference: String,
    result: normalize::PairResult,
}

fn file_stem(inst: &Instance, n: usize) -> String {
    // A filesystem-safe stem from the source (the `#`/`.` are kept readable).
    let safe: String = inst
        .source
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{n:02}_{}_{safe}", inst.shape)
}

fn fixture_text(inst: &Instance, p: &Pair) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# strat_probe fixture — {}", inst.source);
    let _ = writeln!(s, "shape: {}", inst.shape);
    let _ = writeln!(s, "source: {}", inst.source);
    let _ = writeln!(s, "origin: {}", inst.origin);
    let _ = writeln!(s, "hit: {}", p.result.hit);
    let _ = writeln!(s, "--- production ---");
    let _ = writeln!(s, "{}", p.production);
    let _ = writeln!(s, "--- reference ---");
    let _ = writeln!(s, "{}", p.reference);
    s
}

fn main() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let fixtures_dir = PathBuf::from(manifest).join("tests/fixtures/strat_probe");

    let mut instances = corpus_instances();
    instances.extend(generated_instances());

    // Normalize each pair.
    let mut pairs: Vec<Pair> = Vec::new();
    for inst in instances {
        let (production, reference) = templates(&inst);
        let result = normalize::pair_hits(&production, &reference).unwrap_or_else(|e| {
            panic!(
                "fixture {} failed to parse: {e}\n  production: {production}\n  reference:  {reference}",
                inst.source
            )
        });
        pairs.push(Pair {
            inst,
            production,
            reference,
            result,
        });
    }

    // (Re)write the fixture files.
    reset_dir(&fixtures_dir);
    for (n, p) in pairs.iter().enumerate() {
        let path = fixtures_dir.join(format!("{}.fixture", file_stem(&p.inst, n)));
        fs::write(&path, fixture_text(&p.inst, p)).expect("write fixture");
    }

    // Tally + report.
    let report = Report::compute(&pairs);
    let readme = render_readme(&pairs, &report);
    fs::write(fixtures_dir.join("README.md"), &readme).expect("write README");

    println!("{}", report.render_console());
    println!(
        "\nWrote {} fixture pairs + README to {}",
        pairs.len(),
        fixtures_dir.display()
    );
}

fn reset_dir(dir: &Path) {
    if dir.exists() {
        for entry in fs::read_dir(dir).expect("read fixtures dir").flatten() {
            let p = entry.path();
            if p.extension().map(|e| e == "fixture").unwrap_or(false) {
                fs::remove_file(&p).expect("remove old fixture");
            }
        }
    } else {
        fs::create_dir_all(dir).expect("create fixtures dir");
    }
}

/// The computed hit-rate report (REQ-7).
struct Report {
    corpus_hits: usize,
    corpus_total: usize,
    all_hits: usize,
    all_total: usize,
    /// shape -> (hits, total)
    by_shape: BTreeMap<String, (usize, usize)>,
}

impl Report {
    fn compute(pairs: &[Pair]) -> Report {
        let mut by_shape: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        let (mut ch, mut ct, mut ah, mut at) = (0, 0, 0, 0);
        for p in pairs {
            let hit = p.result.hit as usize;
            at += 1;
            ah += hit;
            if p.inst.origin == "corpus" {
                ct += 1;
                ch += hit;
            }
            let e = by_shape.entry(p.inst.shape.clone()).or_insert((0, 0));
            e.0 += hit;
            e.1 += 1;
        }
        Report {
            corpus_hits: ch,
            corpus_total: ct,
            all_hits: ah,
            all_total: at,
            by_shape,
        }
    }

    fn corpus_pct(&self) -> f64 {
        pct(self.corpus_hits, self.corpus_total)
    }

    fn all_pct(&self) -> f64 {
        pct(self.all_hits, self.all_total)
    }

    /// The program-plan decision rule applied to the threshold-bearing
    /// corpus+generated rate.
    fn decision(&self) -> &'static str {
        if self.all_pct() >= 90.0 {
            "hit rate ≥ 90% → stage-2 semantic TV phase ships as a thin fallback (F-C step 1)"
        } else {
            "hit rate < 90% → open a dedicated quantified-equivalence-query design issue before stage 2 commits"
        }
    }

    fn render_console(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "=== SPIKE-2 normalizer-probe hit rate ===");
        let _ = writeln!(
            s,
            "corpus-only (n={}, SMALL-N, not threshold-bearing): {}/{} = {:.1}%",
            self.corpus_total,
            self.corpus_hits,
            self.corpus_total,
            self.corpus_pct()
        );
        let _ = writeln!(
            s,
            "corpus+generated (n={}, THRESHOLD-bearing): {}/{} = {:.1}%",
            self.all_total,
            self.all_hits,
            self.all_total,
            self.all_pct()
        );
        let _ = writeln!(s, "per-shape breakdown:");
        for (shape, (h, t)) in &self.by_shape {
            let _ = writeln!(s, "  {shape:<14} {h}/{t} = {:.1}%", pct(*h, *t));
        }
        let _ = writeln!(s, "decision: {}", self.decision());
        s
    }
}

fn pct(h: usize, t: usize) -> f64 {
    if t == 0 {
        0.0
    } else {
        100.0 * h as f64 / t as f64
    }
}

fn render_readme(pairs: &[Pair], report: &Report) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# SPIKE-2 — normalizer-probe fixtures (`strat_probe`)");
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "Per `.design/m0-spikes.md` REQ-5..REQ-7. Each `*.fixture` file is one"
    );
    let _ = writeln!(
        s,
        "production-style/reference-style raw-quantifier S₂ expansion pair for a"
    );
    let _ = writeln!(
        s,
        "bounded-quantifier combinator instance. The prototype normalizer"
    );
    let _ = writeln!(
        s,
        "(`thermite-tv/src/normalize.rs`, REQ-6) applies the four metatheory §8.2"
    );
    let _ = writeln!(
        s,
        "layer-1 passes (NNF, prenex, canonical de-Bruijn, atom ordering) to both"
    );
    let _ = writeln!(
        s,
        "spellings; a *hit* is two byte-identical normalized forms."
    );
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "Regenerate with `cargo run -p thermite-tv --example strat_probe`."
    );
    let _ = writeln!(s);

    // The hit-rate numbers (REQ-7 / AC-7).
    let _ = writeln!(s, "## Hit rate (REQ-7)");
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "- **corpus-only** (n={}, **small-n**, NOT threshold-bearing on its own): **{}/{} = {:.1}%**",
        report.corpus_total, report.corpus_hits, report.corpus_total, report.corpus_pct()
    );
    let _ = writeln!(
        s,
        "- **corpus+generated** (n={}, **threshold-bearing**): **{}/{} = {:.1}%**",
        report.all_total,
        report.all_hits,
        report.all_total,
        report.all_pct()
    );
    let _ = writeln!(s);
    let _ = writeln!(s, "### Per-shape breakdown");
    let _ = writeln!(s);
    let _ = writeln!(s, "| shape | hits / total | rate |");
    let _ = writeln!(s, "|---|---|---|");
    for (shape, (h, t)) in &report.by_shape {
        let _ = writeln!(s, "| `{shape}` | {h}/{t} | {:.1}% |", pct(*h, *t));
    }
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "Generator-draw note: `sorted` (a lone slice ∈ {{xs, ys}}) and `disjoint` (a"
    );
    let _ = writeln!(
        s,
        "slice pair) have only 2 and ≤4 DISTINCT rendered instances the existing"
    );
    let _ = writeln!(
        s,
        "`gen_combinator` vocabulary can produce — no new generator productions were"
    );
    let _ = writeln!(
        s,
        "added (REQ-5 / Out of Scope), so their ≥5-per-shape sample is padded with"
    );
    let _ = writeln!(
        s,
        "further REAL distinct draws (distinct `gen#…` provenance, repeated pair"
    );
    let _ = writeln!(
        s,
        "text). The predicate-bearing shapes have ample distinct instances."
    );
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "### Decision rule (applied to the corpus+generated rate)"
    );
    let _ = writeln!(s);
    let _ = writeln!(s, "> {}", report.decision());
    let _ = writeln!(s);

    // Out-of-scope shapes.
    let _ = writeln!(s, "## Shapes excluded from the probe");
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "Two of the eight frozen registry combinators have NO layer-1"
    );
    let _ = writeln!(
        s,
        "raw-quantifier expansion and are out of scope (NOT counted as misses):"
    );
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "- `count_where` — a recursive `nat` fold (`decreases s.len()`), not a quantifier."
    );
    let _ = writeln!(
        s,
        "- `permutation_of` — a multiset equality (`a.to_multiset() == b.to_multiset()`)."
    );
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "Their stratified handling is a stage-2 quantified-equivalence concern."
    );
    let _ = writeln!(s);

    // Source listing (REQ-5: scheme-validated address for inv, informal for req/ens).
    let _ = writeln!(s, "## Fixture sources (REQ-5)");
    let _ = writeln!(s);
    let _ = writeln!(s, "| file | shape | source | origin | hit |");
    let _ = writeln!(s, "|---|---|---|---|---|");
    for (n, p) in pairs.iter().enumerate() {
        let _ = writeln!(
            s,
            "| `{}.fixture` | `{}` | `{}` | {} | {} |",
            file_stem(&p.inst, n),
            p.inst.shape,
            p.inst.source,
            p.inst.origin,
            if p.result.hit { "✓" } else { "✗" }
        );
    }
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "Corpus sources use the `thermite-syntax` address scheme where it exists"
    );
    let _ = writeln!(
        s,
        "(`binary_search.loop#1.inv#2` = `forall_below`, `inv#3` = `forall_from`)"
    );
    let _ = writeln!(
        s,
        "and an informal designation for `req`/`ens` (which `address.rs` does not"
    );
    let _ = writeln!(
        s,
        "address): `binary_search.req`, `binary_search.ens.None`. Generated"
    );
    let _ = writeln!(
        s,
        "instances are tagged `gen#<seed>.<k>` (the `gen::generate_clauses` draw)."
    );
    s
}
