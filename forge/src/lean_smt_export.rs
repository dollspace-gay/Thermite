//! Rust→Lean export and checked replay for clause-validity obligations
//! (`.design/stage3-bv-reconstruction.md` REQ-7 / AC-8).
//!
//! Certification replays the solver route's actual `req → clause` validity theorem.
//! The route supplies any implicit domain guards and result grounding. Lean must accept it,
//! and its axiom report must pass the standard allowlist. QF_LIA uses `omega`.
//! QF_BV uses an axiom-clean portfolio and records the successful checker.
//!
//! The older translation-equivalence exporter remains below as a separate audit tool.
//! It is not reconstruction evidence and is not consulted by certification.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thermite_syntax::{BinOp, BvWidth, Expr, Item, Program, UnaryOp};

/// The SMT fragment a per-clause obligation is exported into
/// (`.design/stage3-bv-reconstruction.md` REQ-7). The fragment fixes both the Lean
/// sort the free variables are rendered at and the operator semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtFragment {
    /// QF_LIA scalar: comparisons + boolean connectives + linear arithmetic over
    /// `Int` (the `SmtDemo.lean` PoC shape).
    Lia,
    /// QF_BV at a fixed width, rendered directly over `BitVec N`.
    Bv(BvWidth),
}

/// An out-of-fragment refusal (`.design/stage3-bv-reconstruction.md` REQ-7). Mirrors
/// the skip discipline of [`crate::bitvector::render_bv_prop`]: a construct
/// outside the renderable fragment is named, never silently mis-encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmtExportError {
    /// An `Expr` construct outside the selected fragment, such as a multi-segment
    /// path, method call, or arithmetic operator at proposition position. Carries a
    /// description of the offending construct.
    OutOfFragment(String),
}

impl std::fmt::Display for SmtExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SmtExportError::OutOfFragment(desc) => {
                write!(f, "out of the smt-export fragment: {desc}")
            }
        }
    }
}

/// One per-clause translation-validation equivalence obligation to export
/// (`.design/stage3-bv-reconstruction.md` REQ-7). The exported theorem is the TV
/// shape `(P_production) ⟺ (P_reference)` over the obligation's free variables — the
/// same logical content `thermite-tv`'s `equivalence_obligation` discharges through
/// Verus/Z3, here discharged by a kernel-checked Lean proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmtEquivObligation {
    /// The item / clause name the theorem is named after (sanitized to a Lean ident).
    pub item: String,
    /// The free variables, in binder order. QF_LIA uses `Int`; QF_BV uses `BitVec N`.
    pub vars: Vec<String>,
    /// The production-lowered predicate (the artifact under test).
    pub prod: Expr,
    /// The reference-lowered predicate (the independent encoding).
    pub reference: Expr,
    /// The fragment (QF_LIA or QF_BV at a width).
    pub fragment: SmtFragment,
}

/// The solver route's `req → clause` theorem used for trust migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmtValidityObligation {
    /// Stable clause identity, used in the generated theorem name.
    pub item: String,
    /// Variables quantified by the solver query.
    pub vars: Vec<String>,
    /// The query precondition.
    pub req: Expr,
    /// The clause used by the solver route.
    pub clause: Expr,
    /// Integer or fixed-width semantics.
    pub fragment: SmtFragment,
}

/// Evidence from a successful Lean replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconstructionEvidence {
    /// Generated Lean theorem name.
    pub theorem: String,
    /// SHA-256 of the complete generated Lean source.
    pub source_sha256: String,
    /// Stable fragment label (`qf_lia` or `qf_bvN`).
    pub fragment: String,
    /// Kernel-checked tactic/certificate path.
    pub checker: String,
    /// Axioms reported by Lean, after validation against the standard allowlist.
    pub axioms: Vec<String>,
    /// SHA-256 of the exact SMT-LIB query, when the solver route exposes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solver_query_sha256: Option<String>,
    /// SHA-256 of the canonical source-clause IR supplied to reconstruction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_ir_sha256: Option<String>,
    /// SHA-256 of the exact `req` and conclusion source-clause serialization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_clause_sha256: Option<String>,
    /// SHA-256 of the exact recomputed finite ground universe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_sha256: Option<String>,
    /// SHA-256 of the exact recomputed ground formula/instantiation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instantiation_sha256: Option<String>,
    /// SHA-256 of the exact checked ground-theory clause list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theory_sha256: Option<String>,
    /// SHA-256 of the Boolean problem before DIMACS serialization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub propositional_problem_sha256: Option<String>,
    /// SHA-256 of the DIMACS bytes consumed by the SAT solver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cnf_sha256: Option<String>,
    /// SHA-256 of the LRAT bytes parsed and replayed by Lean.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lrat_sha256: Option<String>,
    /// Number of terms in the checked finite ground universe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_universe_count: Option<usize>,
    /// Number of distinct atoms in the finite instantiation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instantiation_count: Option<usize>,
    /// Number of checked equality/congruence theory clauses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theory_clause_count: Option<usize>,
    /// End-to-end reconstruction elapsed time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
    /// Stable budget result (`within-budget`, `solver-timeout`, or
    /// `kernel-budget`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_outcome: Option<String>,
    /// Domain-separated digest over every verdict-bearing reconstruction
    /// artifact. EPR cache entries must bind to this complete collection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict_key_sha256: Option<String>,
    /// Whether the checked replay artifacts came from the content-addressed
    /// EPR cache. Cached artifacts are still replayed by Lean before use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_hit: Option<bool>,
}

/// The result of attempting a validity replay.
///
/// Only [`ReconstructionOutcome::Checked`] authorizes kernel trust. Other
/// variants preserve the reason replay did not succeed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconstructionOutcome {
    /// Lean accepted the actual validity theorem and its axiom report passed.
    Checked(ReconstructionEvidence),
    /// The actual precondition or clause cannot be represented in this fragment.
    Unsupported(String),
    /// Lean/Lake or the pinned Lean package could not be invoked.
    Unavailable(String),
    /// Lean ran but did not accept the theorem or its axiom report.
    Failed(String),
}

/// `2^width` as a `u128`. `width ≤ 64`, so `1u128 << 64` stays well inside `u128`.
#[must_use]
fn modulus(width: u32) -> u128 {
    1u128 << width
}

/// Sanitize an item name into a Lean identifier tail (`.design/stage3-bv-reconstruction.md`
/// REQ-7). Non-`[A-Za-z0-9_]` characters become `_`; a leading digit is prefixed with
/// `_` so the result is a legal Lean ident. Deterministic (R-CODE-5).
#[must_use]
fn lean_ident(item: &str) -> String {
    let mut out = String::with_capacity(item.len());
    for ch in item.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    } else if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// Render an arithmetic or bitwise term in the selected Lean fragment.
fn render_term(e: &Expr, fragment: SmtFragment) -> Result<String, SmtExportError> {
    match e {
        Expr::IntLit { value, .. } => match fragment {
            SmtFragment::Lia => Ok(format!("({value} : Int)")),
            SmtFragment::Bv(w) => Ok(format!("({}#{})", value % modulus(w.bits()), w.bits())),
        },
        Expr::Path(segs) if segs.len() == 1 => Ok(segs[0].clone()),
        Expr::Binary { op, lhs, rhs } => {
            let sym = match (fragment, op) {
                (_, BinOp::Add) => "+",
                (_, BinOp::Sub) => "-",
                (_, BinOp::Mul) => "*",
                (SmtFragment::Bv(_), BinOp::Div) => "/",
                (SmtFragment::Bv(_), BinOp::Rem) => "%",
                (SmtFragment::Bv(_), BinOp::Shl) => "<<<",
                (SmtFragment::Bv(_), BinOp::Shr) => ">>>",
                (SmtFragment::Bv(_), BinOp::BitAnd) => "&&&",
                (SmtFragment::Bv(_), BinOp::BitOr) => "|||",
                (SmtFragment::Bv(_), BinOp::BitXor) => "^^^",
                (_, other) => {
                    let supported = match fragment {
                        SmtFragment::Lia => "`+`, `-`, or `*`",
                        SmtFragment::Bv(_) => {
                            "arithmetic, division, remainder, bitwise, or shift operators"
                        }
                    };
                    return Err(SmtExportError::OutOfFragment(format!(
                        "`{other:?}` is not a term operator in this fragment; expected {supported}"
                    )));
                }
            };
            let l = render_term(lhs, fragment)?;
            let r = render_term(rhs, fragment)?;
            if matches!((fragment, op), (SmtFragment::Bv(_), BinOp::Div)) {
                let SmtFragment::Bv(w) = fragment else {
                    unreachable!("the branch fixed the fragment")
                };
                // SMT-LIB defines bvudiv-by-zero as all ones, while Lean's BitVec
                // division returns zero. Spell out the SMT case so the two renderers
                // agree even before Thermite's nonzero-divisor obligation is applied.
                let zero = format!("(0#{})", w.bits());
                Ok(format!(
                    "(if {r} = {zero} then (~~~{zero}) else ({l} / {r}))"
                ))
            } else {
                Ok(format!("({l} {sym} {r})"))
            }
        }
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } if matches!(fragment, SmtFragment::Bv(_)) => {
            Ok(format!("(~~~{})", render_term(expr, fragment)?))
        }
        // Casts in this fragment preserve the selected fixed-width representation.
        Expr::Cast { expr, .. } => render_term(expr, fragment),
        other => Err(SmtExportError::OutOfFragment(format!(
            "`{other:?}` is outside the renderable term fragment (only integer \
             literals, single-segment variables, fragment operators, and casts)"
        ))),
    }
}

/// Render a proposition to a Lean `Prop` in the given fragment
/// (`.design/stage3-bv-reconstruction.md` REQ-7). `BitVec` comparisons are unsigned,
/// matching the scalar types and the SMT-LIB QF_BV renderer.
fn render_prop(e: &Expr, fragment: SmtFragment) -> Result<String, SmtExportError> {
    match e {
        Expr::BoolLit(b) => Ok(if *b { "True" } else { "False" }.to_string()),
        Expr::Binary { op, lhs, rhs } => match op {
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let l = render_term(lhs, fragment)?;
                let r = render_term(rhs, fragment)?;
                let rel = match op {
                    BinOp::Eq => "=",
                    BinOp::Ne => "≠",
                    BinOp::Lt => "<",
                    BinOp::Le => "≤",
                    BinOp::Gt => ">",
                    BinOp::Ge => "≥",
                    _ => unreachable!("the outer match fixed the comparison set"),
                };
                Ok(format!("({l} {rel} {r})"))
            }
            BinOp::And => Ok(format!(
                "({} ∧ {})",
                render_prop(lhs, fragment)?,
                render_prop(rhs, fragment)?
            )),
            BinOp::Or => Ok(format!(
                "({} ∨ {})",
                render_prop(lhs, fragment)?,
                render_prop(rhs, fragment)?
            )),
            other => Err(SmtExportError::OutOfFragment(format!(
                "`{other:?}` is an arithmetic/bitwise operator, not a proposition — a \
                 clause must be a comparison or a boolean connective at its root"
            ))),
        },
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => Ok(format!("(¬ {})", render_prop(expr, fragment)?)),
        other => Err(SmtExportError::OutOfFragment(format!(
            "`{other:?}` is outside the renderable proposition fragment (a comparison \
             / boolean connective over the term fragment)"
        ))),
    }
}

/// Render the equivalence GOAL `(P_prod) ↔ (P_ref)` for an obligation
/// (`.design/stage3-bv-reconstruction.md` REQ-7) — the body of the exported theorem,
/// without the binders or the tactic.
pub fn render_goal(o: &SmtEquivObligation) -> Result<String, SmtExportError> {
    // `render_prop` already fully parenthesizes each side, so the `↔` binds the whole
    // predicates with no precedence surprise — no extra wrapping needed.
    let prod = render_prop(&o.prod, o.fragment)?;
    let reference = render_prop(&o.reference, o.fragment)?;
    Ok(format!("{prod} ↔ {reference}"))
}

/// Render the actual clause-validity goal `req → clause`.
pub fn render_validity_goal(o: &SmtValidityObligation) -> Result<String, SmtExportError> {
    let req = render_prop(&o.req, o.fragment)?;
    let clause = render_prop(&o.clause, o.fragment)?;
    Ok(format!("{req} → {clause}"))
}

fn fragment_label(fragment: SmtFragment) -> String {
    match fragment {
        SmtFragment::Lia => "qf_lia".to_string(),
        SmtFragment::Bv(width) => format!("qf_bv{}", width.bits()),
    }
}

fn validity_theorem_name(o: &SmtValidityObligation, goal: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(fragment_label(o.fragment));
    hash.update([0]);
    hash.update(goal);
    let suffix: String = hash
        .finalize()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("thermite_valid_{}_{}", lean_ident(&o.item), suffix)
}

fn valid_lean_binder(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && !matches!(
            name,
            "by" | "do"
                | "else"
                | "end"
                | "false"
                | "for"
                | "fun"
                | "if"
                | "in"
                | "let"
                | "match"
                | "namespace"
                | "open"
                | "theorem"
                | "true"
                | "where"
                | "with"
        )
}

/// Emit the self-contained Lean module used for validity replay.
fn export_validity_theorem_with_tactic(
    o: &SmtValidityObligation,
    bv_tactic: Option<&str>,
    include_bv_helpers: bool,
) -> Result<(String, String), SmtExportError> {
    if let Some(name) = o.vars.iter().find(|name| !valid_lean_binder(name)) {
        return Err(SmtExportError::OutOfFragment(format!(
            "`{name}` is not a safe Lean binder identifier"
        )));
    }
    let goal = render_validity_goal(o)?;
    let name = validity_theorem_name(o, &goal);
    let (binder, tactic) = match o.fragment {
        SmtFragment::Lia => {
            let binder = if o.vars.is_empty() {
                String::new()
            } else {
                format!(" ({} : Int)", o.vars.join(" "))
            };
            (binder, "omega")
        }
        SmtFragment::Bv(width) => {
            let binder = if o.vars.is_empty() {
                String::new()
            } else {
                format!(" ({} : BitVec {})", o.vars.join(" "), width.bits())
            };
            (
                binder,
                bv_tactic.unwrap_or("bv_reconstruct (timeout := 30)"),
            )
        }
    };
    let bv_helpers = if include_bv_helpers && matches!(o.fragment, SmtFragment::Bv(_)) {
        r#"
theorem thermite_rotateRight_rotateLeft {w : Nat} (x : BitVec w) (r : Nat) :
    (x.rotateLeft r).rotateRight r = x := by
  cases w with
  | zero => exact Subsingleton.elim _ _
  | succ w =>
    apply BitVec.eq_of_getLsbD_eq
    intro i hi
    have hr : r % (w + 1) < w + 1 := Nat.mod_lt _ (by omega)
    simp only [BitVec.getLsbD_rotateRight, BitVec.getLsbD_rotateLeft]
    by_cases h : i < w + 1 - r % (w + 1)
    · simp [h]
      omega
    · simp [h, hi]
      have heq : w + 1 - r % (w + 1) + (i - (w + 1 - r % (w + 1))) = i := by omega
      have hlt : i - (w + 1 - r % (w + 1)) < r % (w + 1) := by omega
      simp [hlt, heq, ← BitVec.getLsbD_eq_getElem]

theorem thermite_rotateLeft_injective {w : Nat} (r : Nat) :
    Function.Injective (fun x : BitVec w => x.rotateLeft r) := by
  intro x y h
  have h' := congrArg (fun z : BitVec w => z.rotateRight r) h
  simpa only [thermite_rotateRight_rotateLeft] using h'
"#
    } else {
        ""
    };
    let source = format!(
        "import Mathlib.Tactic\n\
         import Thermite.Reconstruct\n\n\
         namespace Thermite.Reconstruction\n\
         {bv_helpers}\n\
         set_option maxRecDepth 1000000\n\
         set_option maxHeartbeats 2000000 in\n\
         theorem {name}{binder} :\n    {goal} := by\n  {tactic}\n\
         #print axioms {name}\n\n\
         end Thermite.Reconstruction\n"
    );
    Ok((name, source))
}

/// Emit the first-choice source so rendering tests can inspect the generated goal.
#[cfg(test)]
fn export_validity_theorem(o: &SmtValidityObligation) -> Result<(String, String), SmtExportError> {
    export_validity_theorem_with_tactic(o, None, false)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn lake_binary() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        let elan = PathBuf::from(home).join(".elan/bin/lake");
        if elan.exists() {
            return elan;
        }
    }
    PathBuf::from("lake")
}

fn lean_package_root() -> PathBuf {
    std::env::var_os("THERMITE_LEAN_ROOT").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../lean"),
        PathBuf::from,
    )
}

const RECONSTRUCTION_AXIOM_ALLOWLIST: &[&str] = &["propext", "Classical.choice", "Quot.sound"];

fn parse_and_validate_axioms(output: &str, theorem: &str) -> Result<Vec<String>, String> {
    let theorem_at = output
        .find(theorem)
        .ok_or_else(|| format!("missing anchored `#print axioms {theorem}` report"))?;
    let report = &output[theorem_at..];
    let report_head = report
        .lines()
        .next()
        .ok_or_else(|| format!("empty `#print axioms {theorem}` report"))?;
    if report_head.contains("does not depend on any axioms") {
        return Ok(Vec::new());
    }
    if !report_head.contains("depends on axioms:") {
        return Err(format!("missing anchored `#print axioms {theorem}` report"));
    }
    // Lean may wrap the report, so parse through its closing bracket.
    let list = report
        .split_once('[')
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(inside, _)| inside)
        .ok_or_else(|| format!("malformed axiom report: {report_head}"))?;
    let axioms: Vec<String> = list
        .split(',')
        .map(str::trim)
        .filter(|axiom| !axiom.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    if let Some(axiom) = axioms
        .iter()
        .find(|axiom| !RECONSTRUCTION_AXIOM_ALLOWLIST.contains(&axiom.as_str()))
    {
        return Err(format!(
            "theorem `{theorem}` depends on non-allowlisted axiom `{axiom}`"
        ));
    }
    Ok(axioms)
}

fn replay_validity_source(lean_root: &Path, theorem: &str, source: &str) -> ReconstructionOutcome {
    let lake = lake_binary();
    let mut child = match Command::new(&lake)
        .arg("env")
        .arg("lean")
        .arg("--stdin")
        // Full-surface QF_BV goals need more interpreter stack than Lean's default.
        .arg("--tstack=65536")
        .current_dir(lean_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return ReconstructionOutcome::Unavailable(format!(
                "could not invoke `{}` for Lean reconstruction: {error}",
                lake.display()
            ))
        }
    };
    let Some(mut stdin) = child.stdin.take() else {
        return ReconstructionOutcome::Unavailable(
            "the Lean reconstruction process did not expose stdin".to_string(),
        );
    };
    if let Err(error) = stdin.write_all(source.as_bytes()) {
        return ReconstructionOutcome::Unavailable(format!(
            "could not send the validity theorem to Lean: {error}"
        ));
    }
    drop(stdin);
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(error) => {
            return ReconstructionOutcome::Unavailable(format!(
                "could not collect the Lean reconstruction result: {error}"
            ))
        }
    };
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() {
        let head: String = combined.chars().take(600).collect();
        return ReconstructionOutcome::Failed(format!(
            "Lean rejected the actual validity theorem `{theorem}`: {head}"
        ));
    }
    match parse_and_validate_axioms(&combined, theorem) {
        Ok(axioms) => ReconstructionOutcome::Checked(ReconstructionEvidence {
            theorem: theorem.to_string(),
            source_sha256: sha256_hex(source.as_bytes()),
            fragment: String::new(),
            checker: String::new(),
            axioms,
            solver_query_sha256: None,
            canonical_ir_sha256: None,
            source_clause_sha256: None,
            ground_sha256: None,
            instantiation_sha256: None,
            theory_sha256: None,
            propositional_problem_sha256: None,
            cnf_sha256: None,
            lrat_sha256: None,
            ground_universe_count: None,
            instantiation_count: None,
            theory_clause_count: None,
            elapsed_ms: None,
            budget_outcome: None,
            verdict_key_sha256: None,
            cache_hit: None,
        }),
        Err(reason) => ReconstructionOutcome::Failed(reason),
    }
}

/// Replay a clause theorem in Lean and validate its axiom report.
///
/// When available, `solver_query` is hashed into the evidence.
#[must_use]
pub fn reconstruct_validity(
    obligation: &SmtValidityObligation,
    solver_query: Option<&str>,
) -> ReconstructionOutcome {
    let fragment = fragment_label(obligation.fragment);
    let lrat = (
        "Lean kernel + proof-producing LRAT reconstruction",
        Some("bv_reconstruct (timeout := 30)"),
        false,
    );
    let simp = (
        "Lean kernel + BitVec simplification",
        Some(
            "simp [BitVec.sub_eq_add_neg, BitVec.neg_eq_not_add, \
             BitVec.add_comm, BitVec.mul_comm, BitVec.and_comm, \
             BitVec.or_comm, BitVec.xor_assoc]",
        ),
        false,
    );
    let grind = ("Lean kernel + grind", Some("grind"), false);
    let rotate = (
        "Lean kernel + rotate-left injectivity lemma",
        Some(
            "intro thermite_ne thermite_eq\n  \
             apply thermite_ne\n  \
             apply thermite_rotateLeft_injective 1\n  \
             simpa only [BitVec.rotateLeft_def] using thermite_eq",
        ),
        true,
    );
    let attempts: Vec<(&str, Option<&str>, bool)> = match obligation.fragment {
        SmtFragment::Lia => vec![(
            "Lean kernel + omega (verified Presburger procedure)",
            None,
            false,
        )],
        SmtFragment::Bv(_) => {
            // Library lemmas reduce broad operator-surface conjunctions cheaply.
            // Try them before asking the LRAT path to bitblast the whole bundle.
            let is_surface_bundle =
                render_validity_goal(obligation).is_ok_and(|goal| goal.matches('∧').count() >= 4);
            if is_surface_bundle {
                vec![simp, grind, lrat, rotate]
            } else {
                vec![lrat, simp, grind, rotate]
            }
        }
    };
    let mut failures = Vec::new();
    for (checker, tactic, helpers) in attempts {
        let (theorem, source) =
            match export_validity_theorem_with_tactic(obligation, tactic, helpers) {
                Ok(exported) => exported,
                Err(error) => return ReconstructionOutcome::Unsupported(error.to_string()),
            };
        let outcome = match replay_validity_source(&lean_package_root(), &theorem, &source) {
            ReconstructionOutcome::Checked(mut evidence) => {
                evidence.fragment = fragment.clone();
                evidence.checker = checker.to_string();
                evidence.solver_query_sha256 =
                    solver_query.map(|query| sha256_hex(query.as_bytes()));
                ReconstructionOutcome::Checked(evidence)
            }
            ReconstructionOutcome::Unavailable(reason) => {
                return ReconstructionOutcome::Unavailable(reason)
            }
            ReconstructionOutcome::Failed(reason) => {
                failures.push(format!("{checker}: {reason}"));
                continue;
            }
            ReconstructionOutcome::Unsupported(reason) => {
                return ReconstructionOutcome::Unsupported(reason)
            }
        };
        if std::env::var_os("THERMITE_TRACE_RECONSTRUCTION").is_some() {
            eprintln!(
                "reconstruction theorem `{theorem}` ({fragment}) via {checker}: \
                 {outcome:?}\n--- Lean source ---\n{source}"
            );
        }
        return outcome;
    }
    ReconstructionOutcome::Failed(failures.join("\n"))
}

/// Lemmas needed for the commutative rewrites performed by [`reference_normalize`].
/// Order and comparison rewrites are already handled by `simp`.
fn bv_normalization_lemmas(e: &Expr) -> Vec<&'static str> {
    fn walk(e: &Expr, add: &mut bool, mul: &mut bool) {
        match e {
            Expr::Binary { op, lhs, rhs } => {
                *add |= *op == BinOp::Add;
                *mul |= *op == BinOp::Mul;
                walk(lhs, add, mul);
                walk(rhs, add, mul);
            }
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => walk(expr, add, mul),
            _ => {}
        }
    }

    let mut add = false;
    let mut mul = false;
    walk(e, &mut add, &mut mul);
    let mut lemmas = Vec::new();
    if add {
        lemmas.push("BitVec.add_comm");
    }
    if mul {
        lemmas.push("BitVec.mul_comm");
    }
    lemmas
}

/// Export one obligation as a self-contained Lean theorem followed by its
/// `#print axioms` probe (`.design/stage3-bv-reconstruction.md`
/// REQ-7 / AC-8). The theorem is named `thermite_smt_<item>`.
///
/// - [`SmtFragment::Lia`]: `theorem T (a b … : Int) : (P_prod) ↔ (P_ref) := by smt`.
/// - [`SmtFragment::Bv`]: `theorem T (a b … : BitVec N) :
///   (P_prod) ↔ (P_ref) := by simp [BitVec.add_comm, BitVec.mul_comm]`.
pub fn export_theorem(o: &SmtEquivObligation) -> Result<String, SmtExportError> {
    let name = format!("thermite_smt_{}", lean_ident(&o.item));
    let goal = render_goal(o)?;

    match o.fragment {
        SmtFragment::Lia => {
            let binder = if o.vars.is_empty() {
                String::new()
            } else {
                format!(" ({} : Int)", o.vars.join(" "))
            };
            Ok(format!(
                "theorem {name}{binder} :\n    {goal} := by smt\n#print axioms {name}\n"
            ))
        }
        SmtFragment::Bv(w) => {
            let binder = if o.vars.is_empty() {
                String::new()
            } else {
                format!(" ({} : BitVec {})", o.vars.join(" "), w.bits())
            };
            let lemmas = bv_normalization_lemmas(&o.prod);
            let tactic = if lemmas.is_empty() {
                "by\n  simp".to_string()
            } else {
                format!("by\n  simp [{}]", lemmas.join(", "))
            };
            Ok(format!(
                "theorem {name}{binder} :\n    {goal} := {tactic}\n#print axioms {name}\n"
            ))
        }
    }
}

/// The header of an exported Lean file (`.design/stage3-bv-reconstruction.md`
/// REQ-7). A standing banner naming the generator, so the committed artifact is
/// self-describing as automated output (not hand-translation), plus the `import Smt`
/// the `smt` tactic needs.
const FILE_HEADER: &str = "\
/-
  Thermite/SmtExport.lean — AUTO-GENERATED by `forge/src/lean_smt_export.rs`
  (`.design/stage3-bv-reconstruction.md` REQ-7 / AC-8). DO NOT EDIT BY HAND.

  Each theorem is a per-clause translation-validation obligation `(P_prod) ⟺ (P_ref)`
  emitted by the automated Rust→Lean exporter. QF_LIA uses lean-smt/cvc5. QF_BV is
  rendered directly as `BitVec N` and proved from kernel-checked normalization lemmas.
  The `#print axioms` after each theorem must report a subset of
  {propext, Classical.choice, Quot.sound}.

  The literal QF_BV renderer covers wrapping arithmetic, unsigned comparisons,
  bitwise operations, shifts, unsigned division, and remainder. Regenerate via the
  `golden_file_matches_exporter` test with THERMITE_REGEN_SMT_EXPORT=1.
-/
import Smt

namespace Thermite.SmtExport
";

/// Export a batch of obligations into one self-contained Lean file
/// (`.design/stage3-bv-reconstruction.md` REQ-7 / AC-8). The file imports `Smt`,
/// opens the `Thermite.SmtExport` namespace, and emits each obligation's theorem +
/// `#print axioms`. Deterministic in the input order (R-CODE-5).
pub fn export_file(obligations: &[SmtEquivObligation]) -> Result<String, SmtExportError> {
    let mut out = String::from(FILE_HEADER);
    for o in obligations {
        out.push('\n');
        out.push_str(&export_theorem(o)?);
    }
    out.push_str("\nend Thermite.SmtExport\n");
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// The reference encoding + obligation minting (the consumers of the renderers).
// ─────────────────────────────────────────────────────────────────────────────

/// A single-segment variable `Expr`.
fn var(name: &str) -> Expr {
    Expr::Path(vec![name.to_string()])
}

/// A binary `Expr`.
fn bin(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// A logical-negation `Expr`.
fn not_expr(e: Expr) -> Expr {
    Expr::Unary {
        op: UnaryOp::Not,
        expr: Box::new(e),
    }
}

/// Produce an independent REFERENCE encoding of a production predicate — a
/// syntactically-different but logically-equivalent rewrite
/// (`.design/stage3-bv-reconstruction.md` REQ-7). This plays the role
/// `thermite-tv/src/ref_encode.rs` plays for the Verus obligation: the second,
/// independent rendering whose agreement with production the translation-validation
/// obligation `(P_prod) ⟺ (P_ref)` checks. Each rewrite is equivalence-preserving
/// over QF_LIA `Int` and QF_BV `BitVec N`: the comparison flips hold for their total
/// orders, while addition and multiplication commute in both representations.
///
/// - `a ≤ b` → `¬ (b < a)`, `a < b` → `¬ (b ≤ a)` (the comparison-faithfulness flip);
/// - `a ≥ b` → `b ≤ a`, `a > b` → `b < a`;
/// - `a ≠ b` → `¬ (a = b)`;
/// - `a + b` → `b + a`, `a * b` → `b * a` (commutation);
/// - `=`/`∧`/`∨`/`¬`/`-`/… recurse into normalized children, operator kept.
///
/// Deterministic, total (R-CODE-5): a leaf or an unhandled construct is returned
/// unchanged, so a clause the renderer later refuses is refused downstream,
/// not mangled here.
#[must_use]
pub fn reference_normalize(e: &Expr) -> Expr {
    match e {
        Expr::Binary { op, lhs, rhs } => {
            let l = reference_normalize(lhs);
            let r = reference_normalize(rhs);
            match op {
                BinOp::Le => not_expr(bin(BinOp::Lt, r, l)),
                BinOp::Lt => not_expr(bin(BinOp::Le, r, l)),
                BinOp::Ge => bin(BinOp::Le, r, l),
                BinOp::Gt => bin(BinOp::Lt, r, l),
                BinOp::Ne => not_expr(bin(BinOp::Eq, l, r)),
                BinOp::Add | BinOp::Mul => bin(*op, r, l),
                other => bin(*other, l, r),
            }
        }
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => not_expr(reference_normalize(expr)),
        Expr::Cast { expr, ty } => Expr::Cast {
            expr: Box::new(reference_normalize(expr)),
            ty: ty.clone(),
        },
        other => other.clone(),
    }
}

/// Collect the single-segment free variables of a predicate, sorted and de-duplicated
/// (`.design/stage3-bv-reconstruction.md` REQ-7 — the obligation's binder set).
/// `BTreeSet` gives a deterministic order (R-CODE-5).
#[must_use]
pub fn free_vars(e: &Expr) -> Vec<String> {
    fn walk(e: &Expr, acc: &mut BTreeSet<String>) {
        match e {
            Expr::Path(segs) if segs.len() == 1 => {
                acc.insert(segs[0].clone());
            }
            Expr::Binary { lhs, rhs, .. } => {
                walk(lhs, acc);
                walk(rhs, acc);
            }
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => walk(expr, acc),
            _ => {}
        }
    }
    let mut acc = BTreeSet::new();
    walk(e, &mut acc);
    acc.into_iter().collect()
}

/// Mint an equivalence obligation for a production predicate by pairing it with its
/// [`reference_normalize`] encoding (`.design/stage3-bv-reconstruction.md` REQ-7).
/// The binder set is the predicate's [`free_vars`]; the fragment is the caller's.
#[must_use]
pub fn obligation_for_predicate(
    item: &str,
    prod: &Expr,
    fragment: SmtFragment,
) -> SmtEquivObligation {
    SmtEquivObligation {
        item: item.to_string(),
        vars: free_vars(prod),
        prod: prod.clone(),
        reference: reference_normalize(prod),
        fragment,
    }
}

/// The canonical reconstruction-supported obligation set
/// (`.design/stage3-bv-reconstruction.md` REQ-7 / AC-8) — the batch the committed
/// `lean/Thermite/SmtExport.lean` is generated from. One QF_LIA scalar clause and two
/// QF_BV `@bv` clauses covering comparison, wrapping arithmetic, and the complete
/// bitwise/shift/division term surface, each paired with its [`reference_normalize`]
/// reference. The `@bv`
/// fragments are assigned explicitly (not parsed from a `@bvN` tag) so this set is
/// available in the default build, where the `bv` parse feature is off (REQ-1's
/// structural lock).
#[must_use]
pub fn reconstruction_demo_obligations() -> Vec<SmtEquivObligation> {
    // QF_LIA: `(a - b) <= c` — the SmtDemo Tier-3 contract-clause shape.
    let lia = bin(BinOp::Le, bin(BinOp::Sub, var("a"), var("b")), var("c"));
    // QF_BV comparison subfragment (bv64): `a <= b`.
    let bv_cmp = bin(BinOp::Le, var("a"), var("b"));
    // QF_BV modular arithmetic (bv8): `a + b == c`.
    let bv_arith = bin(BinOp::Eq, bin(BinOp::Add, var("a"), var("b")), var("c"));
    // QF_BV full term surface (bv8). The nested multiply is commuted in the
    // independent reference encoding, so this is not a reflexivity-only fixture.
    let bv_full = bin(
        BinOp::Ne,
        bin(
            BinOp::Rem,
            bin(
                BinOp::Div,
                bin(
                    BinOp::Shr,
                    bin(
                        BinOp::Shl,
                        bin(
                            BinOp::BitOr,
                            bin(BinOp::BitAnd, not_expr(var("a")), var("b")),
                            bin(BinOp::BitXor, var("a"), var("b")),
                        ),
                        var("c"),
                    ),
                    var("b"),
                ),
                var("c"),
            ),
            var("b"),
        ),
        bin(BinOp::Add, bin(BinOp::Mul, var("a"), var("b")), var("c")),
    );

    vec![
        obligation_for_predicate("lia_arith_cmp", &lia, SmtFragment::Lia),
        obligation_for_predicate("bv64_le_not_lt", &bv_cmp, SmtFragment::Bv(BvWidth::W64)),
        obligation_for_predicate("bv8_add_comm", &bv_arith, SmtFragment::Bv(BvWidth::W8)),
        obligation_for_predicate("bv8_full_terms", &bv_full, SmtFragment::Bv(BvWidth::W8)),
    ]
}

/// Build the export obligations for every renderable contract `ens` clause of a
/// parsed program (`.design/stage3-bv-reconstruction.md` REQ-7 — the file-driven
/// exporter). A clause carrying a `@bvN` tag (only present in a `bv`-feature build)
/// exports in [`SmtFragment::Bv`]; an untagged clause exports in [`SmtFragment::Lia`].
/// A clause outside the renderable fragment is a skip, named in the returned
/// skip list (never a silent drop). Deterministic in source order (R-CODE-5).
#[must_use]
pub fn obligations_for_program(program: &Program) -> (Vec<SmtEquivObligation>, Vec<String>) {
    let mut obligations = Vec::new();
    let mut skipped = Vec::new();
    for item in &program.items {
        let Item::Fn(f) = item else { continue };
        for (idx, clause) in f.contract.ens.iter().enumerate() {
            let fragment = bv_fragment(clause).unwrap_or(SmtFragment::Lia);
            let name = format!("{}_ens{idx}", f.name);
            let obligation = obligation_for_predicate(&name, &clause.expr, fragment);
            match render_goal(&obligation) {
                Ok(_) => obligations.push(obligation),
                Err(e) => skipped.push(format!("{name}: {e}")),
            }
        }
    }
    (obligations, skipped)
}

/// The QF_BV fragment of a clause carrying a `@bvN` tag, when the `bv` parse feature
/// is compiled in (`.design/stage3-bv-reconstruction.md` REQ-1/REQ-7). Without the
/// feature a `Clause` carries no `bv` field, so this is always `None` and every
/// clause exports as QF_LIA — the structural lock is honored at the exporter too.
#[cfg(feature = "bv")]
fn bv_fragment(clause: &thermite_syntax::Clause) -> Option<SmtFragment> {
    clause.bv.map(|tag| SmtFragment::Bv(tag.width))
}

/// Without the `bv` feature there is no clause-level tag, so every clause is QF_LIA.
#[cfg(not(feature = "bv"))]
fn bv_fragment(_clause: &thermite_syntax::Clause) -> Option<SmtFragment> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
    use thermite_syntax::{Item, Program};

    fn parse_one(src: &str) -> Program {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        parsed.program
    }

    /// Extract the parsed `ens` predicate `Expr` of a fn `name` from `src`. The
    /// demo obligations are built from real parsed Thermite predicates (not
    /// hand-built ASTs), so the exporter is exercised on the same `thermite-syntax`
    /// nodes the production obligation carries.
    fn ens_expr(src: &str, name: &str) -> Expr {
        let p = parse_one(src);
        p.items
            .iter()
            .find_map(|i| match i {
                Item::Fn(f) if f.name == name => Some(f.contract.ens[0].expr.clone()),
                _ => None,
            })
            .expect("fn with an ens clause present")
    }

    // REQ-7: `reference_normalize` produces an equivalence-preserving but
    // syntactically-different reference encoding (the comparison-faithfulness flip +
    // arithmetic commutation) — the second rendering the TV obligation checks.
    #[test]
    fn reference_normalize_flips_and_commutes() {
        // `a <= b`  →  `¬ (b < a)`.
        let le = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens a <= b fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&reference_normalize(&le), SmtFragment::Lia).unwrap(),
            "(¬ (b < a))"
        );
        // `a + b == c`  →  `(b + a) == c` (the Add commutes; the `=` is kept).
        let add = ens_expr(
            "fn p(a: u64, b: u64, c: u64) -> u64 req true ens a + b == c fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&reference_normalize(&add), SmtFragment::Lia).unwrap(),
            "((b + a) = c)"
        );
        // A pure variable / literal leaf is returned unchanged (totality).
        assert_eq!(reference_normalize(&var("x")), var("x"));
    }

    // REQ-8 / AC-9: validity export covers the complete QF_BV term surface and the
    // QF_LIA scalar surface. This is a rendering test only; kernel trust is tested
    // separately from a `ReconstructionOutcome::Checked` replay.
    #[test]
    fn validity_export_covers_lia_and_bv_surfaces() {
        let req = Expr::BoolLit(true);
        let render = |clause: Expr, fragment| {
            export_validity_theorem(&SmtValidityObligation {
                item: "clause".to_string(),
                vars: free_vars(&clause),
                req: req.clone(),
                clause,
                fragment,
            })
        };
        let add = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens a + b == b + a fx pure { a }",
            "p",
        );
        assert!(
            render(add, SmtFragment::Bv(BvWidth::W64)).is_ok(),
            "wrapping arithmetic has a validity theorem"
        );
        let xor = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens a ^ b ^ b == a fx pure { a }",
            "p",
        );
        assert!(
            render(xor, SmtFragment::Bv(BvWidth::W64)).is_ok(),
            "bitwise terms have a literal BitVec validity theorem"
        );
        let lia = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens a <= b fx pure { a }",
            "p",
        );
        assert!(render(lia, SmtFragment::Lia).is_ok());
        let shift = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens (a << b) == a fx pure { a }",
            "p",
        );
        assert!(render(shift, SmtFragment::Bv(BvWidth::W64)).is_ok());
    }

    #[test]
    fn axiom_reports_are_anchored_allowlisted_and_multiline() {
        let theorem = "thermite_valid_example";
        let wrapped = "\
info: 'thermite_valid_example' depends on axioms: [propext,\n\
  Classical.choice,\n\
  Quot.sound]\n";
        assert_eq!(
            parse_and_validate_axioms(wrapped, theorem).unwrap(),
            vec!["propext", "Classical.choice", "Quot.sound"]
        );

        let injected = "info: 'thermite_valid_example' depends on axioms: [propext, sorryAx]\n";
        let error = parse_and_validate_axioms(injected, theorem)
            .expect_err("a non-standard axiom must block reconstruction");
        assert!(error.contains("sorryAx"));

        let wrong_theorem = "info: 'some_other_theorem' depends on axioms: [propext]\n";
        assert!(
            parse_and_validate_axioms(wrong_theorem, theorem).is_err(),
            "an unrelated axiom report is not evidence for this theorem"
        );
    }

    #[cfg(feature = "bv")]
    #[test]
    fn live_validity_replay_checks_lia_and_rejects_false_goals() {
        let req = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens a <= b fx pure { a }",
            "p",
        );
        let clause = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens a + 1 <= b + 1 fx pure { a }",
            "p",
        );
        let valid = reconstruct_validity(
            &SmtValidityObligation {
                item: "lia_monotone".to_string(),
                vars: vec!["a".to_string(), "b".to_string()],
                req,
                clause,
                fragment: SmtFragment::Lia,
            },
            None,
        );
        let ReconstructionOutcome::Checked(evidence) = valid else {
            panic!("the actual QF_LIA implication must reconstruct: {valid:?}");
        };
        assert_eq!(evidence.fragment, "qf_lia");
        assert!(evidence.checker.contains("omega"));
        assert!(evidence.solver_query_sha256.is_none());

        let false_clause = ens_expr("fn p(a: u64) -> u64 req true ens a < a fx pure { a }", "p");
        let invalid = reconstruct_validity(
            &SmtValidityObligation {
                item: "lia_false".to_string(),
                vars: vec!["a".to_string()],
                req: Expr::BoolLit(true),
                clause: false_clause,
                fragment: SmtFragment::Lia,
            },
            None,
        );
        assert!(
            !matches!(invalid, ReconstructionOutcome::Checked(_)),
            "renderability cannot turn a false theorem into checked evidence: {invalid:?}"
        );
    }

    #[cfg(feature = "bv")]
    #[test]
    fn live_bv_replay_checks_the_complete_literal_surface_and_hashes_the_query() {
        let clause = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true \
             ens a + b == b + a \
                 && a - 0 == a \
                 && a * b == b * a \
                 && (a & b) == (b & a) \
                 && (a | b) == (b | a) \
                 && (a ^ b ^ b) == a \
                 && (a << 0) == a \
                 && (a >> 0) == a \
                 && (a / 0) == !0 \
                 && (a % 0) == a \
                 && !(a != a) \
                 && !(a < a) \
                 && a <= a \
                 && !(a > a) \
                 && a >= a \
                 && (a == a || a == b) \
             fx pure { a }",
            "p",
        );
        let query = "(set-logic QF_BV)\n; exact query bytes\n";
        let outcome = reconstruct_validity(
            &SmtValidityObligation {
                item: "bv8_surface".to_string(),
                vars: vec!["a".to_string(), "b".to_string()],
                req: Expr::BoolLit(true),
                clause,
                fragment: SmtFragment::Bv(BvWidth::W8),
            },
            Some(query),
        );
        let ReconstructionOutcome::Checked(evidence) = outcome else {
            panic!("the complete literal QF_BV surface must reconstruct: {outcome:?}");
        };
        assert_eq!(evidence.fragment, "qf_bv8");
        assert!(
            evidence.checker.contains("LRAT")
                || evidence.checker.contains("simplification")
                || evidence.checker.contains("grind"),
            "the evidence names the strategy that actually succeeded: {}",
            evidence.checker
        );
        assert_eq!(
            evidence.solver_query_sha256,
            Some(sha256_hex(query.as_bytes()))
        );
        assert!(evidence
            .axioms
            .iter()
            .all(|axiom| RECONSTRUCTION_AXIOM_ALLOWLIST.contains(&axiom.as_str())));
    }

    #[test]
    fn unsafe_binder_is_unsupported_before_replay() {
        let outcome = reconstruct_validity(
            &SmtValidityObligation {
                item: "unsafe_binder".to_string(),
                vars: vec!["bad-name".to_string()],
                req: Expr::BoolLit(true),
                clause: Expr::BoolLit(true),
                fragment: SmtFragment::Lia,
            },
            None,
        );
        assert!(matches!(outcome, ReconstructionOutcome::Unsupported(_)));
    }

    // REQ-7: the file-driven exporter mints a QF_LIA obligation per renderable `ens`
    // clause and names a non-renderable (bitwise) clause in the skip list.
    #[test]
    fn program_export_skips_out_of_fragment_clauses() {
        let p = parse_one(
            "fn ok(a: u64, b: u64) -> u64 req true ens a <= b fx pure { a }\n\
             fn bad(a: u64, b: u64) -> u64 req true ens (a & b) == a fx pure { a }",
        );
        let (obligations, skipped) = obligations_for_program(&p);
        assert_eq!(
            obligations.len(),
            1,
            "only the renderable clause is exported"
        );
        assert_eq!(obligations[0].item, "ok_ens0");
        assert_eq!(obligations[0].fragment, SmtFragment::Lia);
        assert_eq!(skipped.len(), 1, "the bitwise clause is a named skip");
        assert!(skipped[0].starts_with("bad_ens0:"));
    }

    // REQ-7: the QF_LIA term/prop renderer maps the contract sublanguage to Lean
    // `Int` syntax (the SmtDemo Tier-3 shape).
    #[test]
    fn lia_renders_arith_comparison() {
        let prod = ens_expr(
            "fn p(a: u64, b: u64, c: u64) -> u64 req true ens a - b <= c fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&prod, SmtFragment::Lia).unwrap(),
            "((a - b) ≤ c)"
        );
    }

    // REQ-7: the QF_LIA disjunction + comparison shape (tv_obligation_or_le surface).
    #[test]
    fn lia_renders_or_of_comparisons() {
        let p = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens a == b || a < b fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&p, SmtFragment::Lia).unwrap(),
            "((a = b) ∨ (a < b))"
        );
    }

    // REQ-7: QF_BV terms render directly as Lean `BitVec N` expressions.
    #[test]
    fn bv_renders_literal_bitvec_arithmetic() {
        let p = ens_expr(
            "fn p(a: u64, b: u64, c: u64) -> u64 req true ens a + b == c fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&p, SmtFragment::Bv(BvWidth::W8)).unwrap(),
            "((a + b) = c)"
        );
        // Numeric syntax carries the width and reduces the value modulo 2^N.
        let lit = ens_expr(
            "fn p(a: u64) -> u64 req true ens a == 300 fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&lit, SmtFragment::Bv(BvWidth::W8)).unwrap(),
            "(a = (44#8))"
        );
    }

    // REQ-7: the literal renderer covers every QF_BV term operator used by the
    // production SMT-LIB renderer.
    #[test]
    fn bv_renders_bitwise_shift_division_and_remainder() {
        for (src, lean_op) in [
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a & b) == a fx pure { a }",
                "&&&",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a | b) == a fx pure { a }",
                "|||",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a ^ b) == a fx pure { a }",
                "^^^",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a / b) == a fx pure { a }",
                "/",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a % b) == a fx pure { a }",
                "%",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a << b) == a fx pure { a }",
                "<<<",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a >> b) == a fx pure { a }",
                ">>>",
            ),
        ] {
            let p = ens_expr(src, "p");
            let rendered = render_prop(&p, SmtFragment::Bv(BvWidth::W64))
                .expect("the complete QF_BV term surface renders");
            assert!(
                rendered.contains(lean_op),
                "expected Lean operator `{lean_op}` in {rendered}"
            );
        }

        let not = ens_expr(
            "fn p(a: u64) -> u64 req true ens !a == a fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&not, SmtFragment::Bv(BvWidth::W8)).unwrap(),
            "((~~~a) = a)"
        );

        let div = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens (a / b) == a fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&div, SmtFragment::Bv(BvWidth::W8)).unwrap(),
            "((if b = (0#8) then (~~~(0#8)) else (a / b)) = a)",
            "Lean and SMT-LIB use different bvudiv-by-zero defaults, so the zero case \
             must be explicit"
        );
    }

    // REQ-7 / AC-8: QF_LIA uses `smt`; QF_BV uses literal `BitVec N` binders and
    // kernel-checked normalization lemmas.
    #[test]
    fn theorem_shapes_are_well_formed() {
        let obs = reconstruction_demo_obligations();
        let lia = export_theorem(&obs[0]).unwrap();
        assert!(lia.contains("theorem thermite_smt_lia_arith_cmp (a b c : Int) :"));
        assert!(lia.contains("((a - b) ≤ c) ↔ (¬ (c < (a - b)))"));
        assert!(lia.contains(":= by smt\n"));
        assert!(lia.contains("#print axioms thermite_smt_lia_arith_cmp"));

        let bv = export_theorem(&obs[1]).unwrap();
        assert!(bv.contains("(a b : BitVec 64)"));
        assert!(bv.contains(":= by\n  simp\n"));
        assert!(bv.contains("#print axioms thermite_smt_bv64_le_not_lt"));

        let full = export_theorem(&obs[3]).unwrap();
        assert!(full.contains("simp [BitVec.add_comm, BitVec.mul_comm]"));
    }

    // REQ-7: the committed `lean/Thermite/SmtExport.lean` IS the exporter's automated
    // output for the AC-8 batch — the proof the hand-translation gap is closed (the
    // file is generated, not authored). Set THERMITE_REGEN_SMT_EXPORT=1 to regenerate.
    #[test]
    fn golden_file_matches_exporter() {
        let generated =
            export_file(&reconstruction_demo_obligations()).expect("the demo batch exports");
        let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("lean")
            .join("Thermite")
            .join("SmtExport.lean");
        if std::env::var_os("THERMITE_REGEN_SMT_EXPORT").is_some() {
            std::fs::write(&golden_path, &generated).expect("regenerate the golden file");
            return;
        }
        let committed = std::fs::read_to_string(&golden_path).unwrap_or_else(|e| {
            panic!(
                "read committed {}: {e} (regenerate with THERMITE_REGEN_SMT_EXPORT=1)",
                golden_path.display()
            )
        });
        assert_eq!(
            generated, committed,
            "the committed SmtExport.lean must be the exporter's verbatim output \
             (regenerate with THERMITE_REGEN_SMT_EXPORT=1)"
        );
    }

    fn lake_binary() -> Option<PathBuf> {
        if let Ok(home) = std::env::var("HOME") {
            let elan = PathBuf::from(home).join(".elan/bin/lake");
            if elan.exists() {
                return Some(elan);
            }
        }
        if Command::new("lake")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(PathBuf::from("lake"));
        }
        None
    }

    /// AC-8 (live): `lake build` the exporter-generated module and assert every
    /// theorem's `#print axioms` report is a subset of `{propext, Classical.choice,
    /// Quot.sound}` — no `sorryAx`, no `Smt` oracle, no `Lean.ofReduceBool`. Gated on
    /// `lake` (the cvc5-FFI build): a shard without it SKIPs rather than fails (the
    /// `engine.rs` live-test precedent). Run requires the SmtDemo toolchain
    /// (toolchain v4.29.0 + Mathlib + vendored cvc5) already materialized.
    #[test]
    fn ac8_exported_obligations_discharge_axiom_clean() {
        let Some(lake) = lake_binary() else {
            eprintln!("SKIP: lake not available — the AC-8 axiom-clean check is not run");
            return;
        };
        let lean_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("lean");
        // Build the committed exporter output module (kept in sync by
        // `golden_file_matches_exporter`). `#print axioms` reports surface as `info:`
        // lines in the build output.
        let out = Command::new(&lake)
            .arg("build")
            .arg("Thermite.SmtExport")
            .current_dir(&lean_root)
            .output()
            .expect("spawn lake build");
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            out.status.success(),
            "lake build of Thermite.SmtExport must succeed (the `smt` tactic discharged \
             every obligation):\n{combined}"
        );
        // Every exported theorem's axiom report must be clean. The names are the
        // `thermite_smt_<item>` theorems of the demo batch.
        let allow = ["propext", "Classical.choice", "Quot.sound"];
        let mut checked = 0usize;
        for o in reconstruction_demo_obligations() {
            let thm = format!("thermite_smt_{}", lean_ident(&o.item));
            let anchor = format!("'Thermite.SmtExport.{thm}'");
            let line = combined
                .lines()
                .find(|l| l.contains(&anchor) && l.contains("depends on axioms:"))
                .unwrap_or_else(|| {
                    panic!("no `#print axioms` report for {thm} in lake output:\n{combined}")
                });
            assert!(
                !line.to_ascii_lowercase().contains("sorry"),
                "{thm} pulled a sorryAx (NOT kernel-clean): {line}"
            );
            let list = line
                .split_once('[')
                .and_then(|(_, rest)| rest.split_once(']'))
                .map(|(inside, _)| inside)
                .unwrap_or("");
            for ax in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                assert!(
                    allow.contains(&ax),
                    "{thm} depends on a non-standard axiom `{ax}` (outside {allow:?}): {line}"
                );
            }
            checked += 1;
        }
        assert_eq!(
            checked, 4,
            "all four demo obligations must be axiom-checked"
        );
    }
}
