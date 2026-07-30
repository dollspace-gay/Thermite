//! Production reconstruction for admitted S₂.0 relation/sequence clauses.
//!
//! Rust renders the canonical classifier IR into Lean, asks Lean to recompute
//! structural Skolemization, the finite ground universe, theory clauses, and
//! Tseitin CNF, then uses the pinned CaDiCaL + drat-trim pair only to find an
//! answer and (for UNSAT) an LRAT certificate. A clause is certified only after
//! a second Lean run parses and kernel-checks that LRAT against the recomputed
//! problem and proves the actual `req → clause` formula.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thermite_spec::classifier::{self, Atom, Frm, Mach, Rel, ScalarValue, Sort2, Tm, Verdict};
use thermite_spec::{QFreeAtom, QFreeFragment, S2Recon};
use thermite_syntax::{BinOp, BvWidth, Clause, Expr, FnItem, UnaryOp};

use crate::lean_smt_export::{
    ReconstructionEvidence, ReconstructionOutcome, SmtFragment, SmtValidityObligation,
};

const SOLVER_SECONDS: u64 = 30;
const EPR_FRAGMENT: &str = "s2_recon_v2";
const CADICAL_VERSION: &str = "2.1.3";
const CADICAL_REVISION: &str = "f13d74439a5b5c963ac5b02d05ce93a8098018b8";
const DRAT_TRIM_REVISION: &str = "effa1dcce85c878236f8313133dff1a2b766cd7c";
const EPR_CHECKER: &str = "Lean kernel + structural EPR + CaDiCaL 2.1.3 + drat-trim effa1dc + \
     term-producing LRAT replay";
const EPR_CACHE_SCHEMA: &str = "thermite.epr.artifacts.v1";
const AXIOM_ALLOWLIST: &[&str] = &["propext", "Classical.choice", "Quot.sound"];
const COUNTERMODEL_SEEDS: usize = 1 << 16;
const COUNTERMODEL_QFREE_MASKS: usize = 1 << 8;
static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);
static RECONSTRUCTION_LOCK: Mutex<()> = Mutex::new(());
static LEAN_MODULE_BUILD: OnceLock<Result<(), String>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundAtomValue {
    pub atom: String,
    pub value: bool,
}

/// A checked SAT assignment presented as a finite Herbrand model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiniteCountermodel {
    pub model: String,
    pub universe_count: usize,
    pub universe_sha256: String,
    pub atoms: Vec<GroundAtomValue>,
    pub cnf_sha256: String,
    pub qfree_checks: Vec<String>,
    pub axioms: Vec<String>,
}

impl FiniteCountermodel {
    #[must_use]
    pub fn diagnostic(&self) -> String {
        const DISPLAYED_ATOMS: usize = 8;
        let mut assignments = self
            .atoms
            .iter()
            .enumerate()
            .take(DISPLAYED_ATOMS)
            .map(|(index, entry)| format!("a{index}={}", entry.value))
            .collect::<Vec<_>>();
        if self.atoms.len() > DISPLAYED_ATOMS {
            assignments.push(format!("… {} more", self.atoms.len() - DISPLAYED_ATOMS));
        }
        format!(
            "Lean-checked finite S₂.0 countermodel; {}; ground terms={} \
             (sha256={}); evaluated atoms={} [{}]; satisfying CNF sha256={}; \
             checked QF groups=[{}]; axioms=[{}]",
            self.model,
            self.universe_count,
            self.universe_sha256,
            self.atoms.len(),
            assignments.join(", "),
            self.cnf_sha256,
            self.qfree_checks.join(", "),
            self.axioms.join(", ")
        )
    }
}

struct QfreeRealization {
    values: Vec<bool>,
    checks: Vec<ReconstructionEvidence>,
    witnesses: Vec<String>,
}

struct QfreeGroupRealization {
    evidence: ReconstructionEvidence,
    witness: String,
    values: BTreeMap<String, u128>,
}

enum CountermodelAttemptFailure {
    Retry(String),
    Fatal(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EprOutcome {
    Proved(ReconstructionEvidence),
    Counterexample(FiniteCountermodel),
    Timeout(String),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct GroundMetadata {
    dimacs: String,
    order: String,
    ground: String,
    formula: String,
    theory: String,
    problem: String,
    bool_problem: String,
    atoms: Vec<(usize, String)>,
    ground_count: usize,
    instantiation_count: usize,
    theory_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedUnsat {
    schema: String,
    input_key_sha256: String,
    verdict_key_sha256: String,
    canonical: String,
    source_clause: String,
    theorem: String,
    final_source: String,
    lrat: String,
    ground: GroundMetadata,
}

struct Scratch {
    path: PathBuf,
}

#[derive(Debug)]
struct SolverToolchain {
    cadical: PathBuf,
    drat_trim: PathBuf,
}

impl Scratch {
    fn new(key: &str) -> Result<Self, String> {
        let serial = SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "thermite-epr-{}-{}-{}",
            std::process::id(),
            serial,
            &sha256_hex(key.as_bytes())[..12]
        ));
        fs::create_dir(&path)
            .map_err(|error| format!("could not create EPR scratch directory: {error}"))?;
        Ok(Self { path })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if std::env::var_os("THERMITE_KEEP_EPR_SCRATCH").is_none() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[must_use]
pub fn needs_reconstruction(formula: &Frm) -> bool {
    fn term_is_epr(term: &Tm) -> bool {
        match term {
            Tm::Read(_, _, _) | Tm::Len(_) | Tm::App1(_, _, _, _) => true,
            Tm::Var(Sort2::Opaque(_), _)
            | Tm::Const(Sort2::Opaque(_), _)
            | Tm::Lit(Sort2::Opaque(_), _) => true,
            Tm::Var(_, _) | Tm::Const(_, _) | Tm::Lit(_, _) => false,
            Tm::Cast(_, inner) | Tm::IdxOp(inner, _) => term_is_epr(inner),
            Tm::Mul(left, right) => term_is_epr(left) || term_is_epr(right),
        }
    }
    fn atom_is_epr(atom: &Atom) -> bool {
        match atom {
            Atom::QFree(_) => false,
            Atom::Rel(_, left, right) => term_is_epr(left) || term_is_epr(right),
        }
    }
    match formula {
        Frm::All(_, _) | Frm::Ex(_, _) => true,
        Frm::Atom(atom) => atom_is_epr(atom),
        Frm::Neg(inner) => needs_reconstruction(inner),
        Frm::Conj(left, right) | Frm::Disj(left, right) | Frm::Imp(left, right) => {
            needs_reconstruction(left) || needs_reconstruction(right)
        }
    }
}

/// Reconstruct one already-bridged `req → clause` obligation.
#[must_use]
pub fn reconstruct(
    recon: &S2Recon,
    item: &FnItem,
    premise_clause: &Clause,
    conclusion_clause: &Clause,
) -> EprOutcome {
    let _reconstruction_guard = RECONSTRUCTION_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let started = Instant::now();
    if !matches!(classifier::classify(&recon.formula), Verdict::Admitted) {
        return EprOutcome::Failed(
            "EprClassifierRejected: production reconstruction was called for a \
             non-admitted S₂.0 formula"
                .to_string(),
        );
    }
    let (premise, conclusion) = match obligation_parts(&recon.formula) {
        Some(parts) => parts,
        None => {
            return EprOutcome::Failed(
                "EprBridgePolarity: canonical obligation is not `req ∧ ¬clause`".to_string(),
            )
        }
    };
    let toolchain = match verify_solver_toolchain() {
        Ok(toolchain) => toolchain,
        Err(reason) => return EprOutcome::Failed(reason),
    };
    if let Err(reason) = ensure_lean_reconstruction_modules() {
        return EprOutcome::Failed(format!("EprLeanBuild: {reason}"));
    }
    let premise = match render_frm(premise, recon, item) {
        Ok(rendered) => rendered,
        Err(reason) => return EprOutcome::Failed(format!("EprLeanExport: {reason}")),
    };
    let conclusion = match render_frm(conclusion, recon, item) {
        Ok(rendered) => rendered,
        Err(reason) => return EprOutcome::Failed(format!("EprLeanExport: {reason}")),
    };

    let canonical = recon.canonical_wire();
    let theorem = format!(
        "thermite_epr_{}_{}",
        lean_ident(&recon.address.item),
        lean_ident(&recon.address.clause)
    );
    let source_clause = format!(
        "{}\n{}",
        thermite_spec::canonical_source_expr(&premise_clause.expr),
        thermite_spec::canonical_source_expr(&conclusion_clause.expr)
    );
    let cache_input = cache_input_key(&canonical, &source_clause, &premise, &conclusion).ok();
    let scratch = match Scratch::new(&canonical) {
        Ok(scratch) => scratch,
        Err(reason) => return EprOutcome::Failed(format!("EprScratch: {reason}")),
    };
    if let Some(input_key) = cache_input.as_deref() {
        match try_cached_unsat(
            input_key,
            &canonical,
            &source_clause,
            &theorem,
            &premise,
            &conclusion,
            &scratch.path,
            started,
        ) {
            Ok(Some(evidence)) => return EprOutcome::Proved(evidence),
            Ok(None) => {}
            Err(reason) => return EprOutcome::Failed(reason),
        }
    }
    let driver_source = ground_driver_source(&premise, &conclusion);
    let driver_path = scratch.path.join("ground.lean");
    if let Err(error) = fs::write(&driver_path, driver_source) {
        return EprOutcome::Failed(format!("EprScratch: could not write Lean driver: {error}"));
    }
    let ground_output = match run_lean(&driver_path, true) {
        Ok(output) => output,
        Err(reason) => return EprOutcome::Failed(format!("EprGrounding: {reason}")),
    };
    let ground = match parse_ground_output(&ground_output) {
        Ok(metadata) => metadata,
        Err(reason) => return EprOutcome::Failed(format!("EprGroundingOutput: {reason}")),
    };

    let cnf_path = scratch.path.join("problem.cnf");
    let proof_path = scratch.path.join("problem.drat");
    let model_path = scratch.path.join("model.txt");
    let mut blocked_qfree_masks = Vec::new();
    let mut last_rejected_mask = None;
    loop {
        let active_dimacs = match dimacs_with_blocking_clauses(&ground.dimacs, &blocked_qfree_masks)
        {
            Ok(dimacs) => dimacs,
            Err(reason) => return EprOutcome::Failed(format!("EprCountermodelBlocking: {reason}")),
        };
        if let Err(error) = fs::write(&cnf_path, active_dimacs.as_bytes()) {
            return EprOutcome::Failed(format!("EprScratch: could not write DIMACS: {error}"));
        }
        let solver = match run_cadical(&toolchain.cadical, &cnf_path, &proof_path, &model_path) {
            Ok(output) => output,
            Err(reason) => return EprOutcome::Failed(reason),
        };
        match solver.status.code() {
            Some(10) => {
                let model_text = match fs::read_to_string(&model_path) {
                    Ok(text) => text,
                    Err(error) => {
                        return EprOutcome::Failed(format!(
                            "EprCountermodelMissing: CaDiCaL reported SAT but its model \
                         could not be read: {error}"
                        ))
                    }
                };
                let assignment = match parse_sat_assignment(&model_text) {
                    Ok(assignment) => assignment,
                    Err(reason) => {
                        return EprOutcome::Failed(format!("EprCountermodelMalformed: {reason}"))
                    }
                };
                if let Err(reason) = validate_dimacs_assignment(&active_dimacs, &assignment) {
                    return EprOutcome::Failed(format!("EprCountermodelInvalid: {reason}"));
                }
                let qfree_values = match qfree_values_from_assignment(recon, &ground, &assignment) {
                    Ok(values) => values,
                    Err(reason) => {
                        return EprOutcome::Failed(format!(
                            "EprCountermodelQFreeAssignment: {reason}"
                        ))
                    }
                };
                match checked_counterexample(
                    recon,
                    item,
                    &scratch.path,
                    &premise,
                    &conclusion,
                    &ground,
                    qfree_values,
                ) {
                    Ok(mut counterexample) => {
                        if !blocked_qfree_masks.is_empty() {
                            counterexample.model.push_str(&format!(
                                ", after rejecting {} unrealized QFree mask(s)",
                                blocked_qfree_masks.len()
                            ));
                        }
                        return EprOutcome::Counterexample(counterexample);
                    }
                    Err(CountermodelAttemptFailure::Fatal(reason)) => {
                        return EprOutcome::Failed(reason)
                    }
                    Err(CountermodelAttemptFailure::Retry(reason)) => {
                        if blocked_qfree_masks.len() + 1 >= COUNTERMODEL_QFREE_MASKS {
                            return EprOutcome::Timeout(format!(
                                "EprCountermodelMaskBudget: checked {} distinct QFree masks; \
                             last rejection: {reason}",
                                COUNTERMODEL_QFREE_MASKS
                            ));
                        }
                        let blocking = match qfree_blocking_clause(
                            recon.qfree_atoms.len(),
                            &ground,
                            &assignment,
                        ) {
                            Ok(clause) => clause,
                            Err(blocking_reason) => {
                                return EprOutcome::Failed(format!(
                                    "EprCountermodelBlocking: {blocking_reason}; \
                                     rejected mask: {reason}"
                                ))
                            }
                        };
                        if blocked_qfree_masks.contains(&blocking) {
                            return EprOutcome::Failed(format!(
                                "EprCountermodelBlocking: CaDiCaL repeated an already-blocked \
                             QFree mask; rejected mask: {reason}"
                            ));
                        }
                        blocked_qfree_masks.push(blocking);
                        last_rejected_mask = Some(reason);
                    }
                }
            }
            Some(20) if blocked_qfree_masks.is_empty() => break,
            Some(20) => {
                return EprOutcome::Failed(format!(
                    "EprCountermodelMasksExhausted: the Boolean problem has no untried \
                     QFree assignment after {} checked rejection(s); last rejection: {}",
                    blocked_qfree_masks.len(),
                    last_rejected_mask.as_deref().unwrap_or("none")
                ))
            }
            _ => {
                let detail = output_head(&solver);
                if detail.contains("time limit") || detail.contains("UNKNOWN") {
                    return EprOutcome::Timeout(format!(
                        "EprSolverTimeout: CaDiCaL did not decide the finite problem within \
                         {SOLVER_SECONDS}s: {detail}"
                    ));
                }
                return EprOutcome::Failed(format!(
                    "EprSolverFailure: CaDiCaL exited {:?}: {detail}",
                    solver.status.code()
                ));
            }
        }
    }

    let lrat_path = scratch.path.join("problem.lrat");
    let trim = match run_drat_trim(&toolchain.drat_trim, &cnf_path, &proof_path, &lrat_path) {
        Ok(output) => output,
        Err(reason) => return EprOutcome::Failed(reason),
    };
    if !trim.status.success() {
        return EprOutcome::Failed(format!(
            "EprLratConversion: drat-trim rejected the proof: {}",
            output_head(&trim)
        ));
    }
    let lrat = match fs::read_to_string(&lrat_path) {
        Ok(text) if !text.trim().is_empty() => strip_lrat_deletions(&text),
        Ok(_) => {
            return EprOutcome::Failed(
                "EprLratMissing: drat-trim produced an empty certificate".to_string(),
            )
        }
        Err(error) => {
            return EprOutcome::Failed(format!(
                "EprLratMissing: could not read drat-trim output: {error}"
            ))
        }
    };
    let final_source = replay_source(&theorem, &premise, &conclusion, &lrat, &ground);
    let replay_path = scratch.path.join("replay.lean");
    if let Err(error) = fs::write(&replay_path, final_source.as_bytes()) {
        return EprOutcome::Failed(format!(
            "EprScratch: could not write replay theorem: {error}"
        ));
    }
    let replay_output = match run_lean(&replay_path, false) {
        Ok(output) => output,
        Err(reason) if is_kernel_budget(&reason) => {
            return EprOutcome::Timeout(format!("EprKernelBudget: {reason}"))
        }
        Err(reason) => return EprOutcome::Failed(format!("EprKernelReplay: {reason}")),
    };
    let axioms = match parse_axioms(&replay_output, &theorem) {
        Ok(axioms) => axioms,
        Err(reason) => {
            return EprOutcome::Failed(format!(
                "EprAxiomReport: {reason}; replay output: {}",
                replay_output.chars().take(1200).collect::<String>()
            ))
        }
    };
    let evidence = build_evidence(
        &theorem,
        &final_source,
        &canonical,
        &source_clause,
        &ground,
        &lrat,
        axioms,
        started,
        false,
    );
    if let Some(input_key) = cache_input.as_deref() {
        if let Some(verdict_key_sha256) = evidence.verdict_key_sha256.clone() {
            let entry = CachedUnsat {
                schema: EPR_CACHE_SCHEMA.to_string(),
                input_key_sha256: input_key.to_string(),
                verdict_key_sha256,
                canonical,
                source_clause,
                theorem,
                final_source,
                lrat,
                ground,
            };
            let _ = store_cached_unsat(&entry);
        }
    }
    EprOutcome::Proved(evidence)
}

fn obligation_parts(formula: &Frm) -> Option<(&Frm, &Frm)> {
    match formula {
        Frm::Conj(premise, negated) => match negated.as_ref() {
            Frm::Neg(conclusion) => Some((premise, conclusion)),
            _ => None,
        },
        _ => None,
    }
}

fn render_sort(sort: &Sort2) -> String {
    match sort {
        Sort2::Mach(machine) => {
            let name = match machine {
                Mach::U8 => "u8",
                Mach::U16 => "u16",
                Mach::U32 => "u32",
                Mach::U64 => "u64",
                Mach::Usize => "usize",
                Mach::Bool => "bool",
            };
            format!("(.mach .{name})")
        }
        Sort2::Seq(inner) => format!("(.seq {})", render_sort(inner)),
        Sort2::Opaque(id) => format!("(.opaque {id})"),
    }
}

fn render_tm(term: &Tm) -> String {
    match term {
        Tm::Var(sort, index) => format!("(.var {} {index})", render_sort(sort)),
        Tm::Const(sort, id) => format!("(.const {} {id})", render_sort(sort)),
        Tm::Lit(sort, ScalarValue::Int(value)) => {
            format!("(.lit {} (.int {value}))", render_sort(sort))
        }
        Tm::Lit(sort, ScalarValue::Bool(value)) => {
            format!("(.lit {} (.bool {value}))", render_sort(sort))
        }
        Tm::Read(elem, sequence, index) => format!(
            "(.read {} {} {})",
            render_sort(elem),
            render_tm(sequence),
            render_tm(index)
        ),
        Tm::Len(sequence) => format!("(.len {})", render_tm(sequence)),
        Tm::Cast(target, inner) => {
            format!("(.cast {} {})", render_sort(target), render_tm(inner))
        }
        Tm::IdxOp(inner, offset) => format!("(.idxOp {} {offset})", render_tm(inner)),
        Tm::Mul(left, right) => format!("(.mul {} {})", render_tm(left), render_tm(right)),
        Tm::App1(argument, result, id, inner) => format!(
            "(.app1 {} {} {id} {})",
            render_sort(argument),
            render_sort(result),
            render_tm(inner)
        ),
    }
}

fn render_atom(atom: &Atom, recon: &S2Recon, item: &FnItem) -> Result<String, String> {
    match atom {
        Atom::Rel(relation, left, right) => {
            let relation = match relation {
                Rel::Eq => "eq",
                Rel::Ne => "ne",
                Rel::Lt => "lt",
                Rel::Le => "le",
                Rel::Gt => "gt",
                Rel::Ge => "ge",
            };
            Ok(format!(
                "(.rel .{relation} {} {})",
                render_tm(left),
                render_tm(right)
            ))
        }
        Atom::QFree(id) => {
            let source = recon
                .qfree_atoms
                .iter()
                .find(|atom| atom.id == *id)
                .ok_or_else(|| format!("qfree id {id} has no canonical source expression"))?;
            let expression = crate::lean_export::encode_strat_qfree_expr(&source.expression, item)?;
            Ok(format!("(.qfree {id} {expression})"))
        }
    }
}

fn render_frm(formula: &Frm, recon: &S2Recon, item: &FnItem) -> Result<String, String> {
    match formula {
        Frm::Atom(atom) => Ok(format!("(.atom {})", render_atom(atom, recon, item)?)),
        Frm::Neg(inner) => Ok(format!("(.neg {})", render_frm(inner, recon, item)?)),
        Frm::Conj(left, right) => Ok(format!(
            "(.conj {} {})",
            render_frm(left, recon, item)?,
            render_frm(right, recon, item)?
        )),
        Frm::Disj(left, right) => Ok(format!(
            "(.disj {} {})",
            render_frm(left, recon, item)?,
            render_frm(right, recon, item)?
        )),
        Frm::Imp(left, right) => Ok(format!(
            "(.imp {} {})",
            render_frm(left, recon, item)?,
            render_frm(right, recon, item)?
        )),
        Frm::All(sort, body) => Ok(format!(
            "(.all {} {})",
            render_sort(sort),
            render_frm(body, recon, item)?
        )),
        Frm::Ex(sort, body) => Ok(format!(
            "(.ex {} {})",
            render_sort(sort),
            render_frm(body, recon, item)?
        )),
    }
}

fn common_source(premise: &str, conclusion: &str) -> String {
    format!(
        r#"import Thermite.Strat.EprReplay

open Thermite.Strat.Cls
open Std.Tactic.BVDecide

set_option maxHeartbeats 8000000
set_option maxRecDepth 100000

private def premise : Frm := {premise}
private def conclusion : Frm := {conclusion}
private def source : Frm := .conj premise (.neg conclusion)
private def skeleton : EprReplayCertificate := buildEprSkeleton source
private def problem := eprCnf skeleton
"#
    )
}

fn ground_driver_source(premise: &str, conclusion: &str) -> String {
    format!(
        r#"{}

def main : IO Unit := do
  IO.println "THERMITE-DIMACS-BEGIN"
  IO.print problem.dimacs
  IO.println "THERMITE-DIMACS-END"
  IO.println s!"THERMITE-GROUND-COUNT={{skeleton.instantiation.grounding.ground.length}}"
  IO.println s!"THERMITE-INSTANTIATION-COUNT={{skeleton.instantiation.formula.atoms.length}}"
  IO.println s!"THERMITE-THEORY-COUNT={{skeleton.theory.length}}"
  IO.println s!"THERMITE-ORDER={{
    (repr skeleton.instantiation.grounding.order).pretty 1000000}}"
  IO.println s!"THERMITE-GROUND={{
    (repr skeleton.instantiation.grounding.ground).pretty 1000000}}"
  IO.println s!"THERMITE-FORMULA={{
    (repr skeleton.instantiation.formula).pretty 1000000}}"
  IO.println s!"THERMITE-THEORY={{(repr skeleton.theory).pretty 1000000}}"
  IO.println "THERMITE-PROBLEM=direct-horn-tseitin"
  IO.println s!"THERMITE-BOOL-PROBLEM={{
    (repr (eprFormula skeleton)).pretty 1000000}}"
  let atoms := eprAtoms skeleton
  for index in List.range atoms.length do
    match atoms[index]? with
    | none => pure ()
    | some atom =>
      let dimacsVariable :=
        (Thermite.PropReconstruct.tseitinVariablesWith
          (eprFormula skeleton) (eprTheoryClauses skeleton)).idxOf
          (.source index) + 1
      IO.println s!"THERMITE-ATOM={{dimacsVariable}}|{{
        (repr atom).pretty 1000000}}"
  IO.println s!"THERMITE-INSTANTIATION-VERIFIED={{
    verifyStructuralInstantiation source skeleton.instantiation}}"
  IO.println s!"THERMITE-THEORY-VERIFIED={{
    verifyTheory (eprGround skeleton) skeleton.theory}}"
"#,
        common_source(premise, conclusion)
    )
}

fn replay_source(
    theorem: &str,
    premise: &str,
    conclusion: &str,
    lrat: &str,
    ground: &GroundMetadata,
) -> String {
    let lrat_literal = serde_json::to_string(lrat).expect("serializing a string cannot fail");
    format!(
        r#"{}

private def checkedOrder : List Sort₂ := {order}
private def checkedGround : GroundUniverse := {ground}
private def checkedFormula : GroundFrm := {formula}
private def checkedTheory : List GroundTheoryStep := {theory}

kernel_lrat_text_decl thermiteEprLratCertificate from {lrat_literal}
private def certificate : EprReplayCertificate :=
  {{ instantiation :=
      {{ grounding := {{ order := checkedOrder, ground := checkedGround }}
        formula := checkedFormula }}
    theory := checkedTheory
    lrat := thermiteEprLratCertificate }}
def thermiteEprCnf : Std.Sat.CNF Nat := eprCnf certificate

private theorem instantiationChecked :
    verifyStructuralBinding source certificate.instantiation = true := by
  kernel_bool_check

private theorem theoryChecked :
    verifyTheory (eprGround certificate) certificate.theory = true := by
  kernel_bool_check

private theorem actionsChecked :
    LRAT.check thermiteEprLratCertificate thermiteEprCnf = true := by
  kernel_lrat_cnf_check "thermiteEprCnf"
    with "thermiteEprLratCertificate"

theorem {theorem} : EprClaim premise conclusion := by
  exact checked_structural_binding_claim_of_epr_actions
    instantiationChecked theoryChecked actionsChecked

#print axioms {theorem}
"#,
        common_source(premise, conclusion),
        order = ground.order,
        ground = ground.ground,
        formula = ground.formula,
        theory = ground.theory,
    )
}

fn conjunction(mut formulas: Vec<Expr>) -> Expr {
    if formulas.is_empty() {
        return Expr::BoolLit(true);
    }
    let first = formulas.remove(0);
    formulas
        .into_iter()
        .fold(first, |left, right| Expr::Binary {
            op: BinOp::And,
            lhs: Box::new(left),
            rhs: Box::new(right),
        })
}

fn expected_qfree_formula(atoms: &[(&QFreeAtom, bool)]) -> Expr {
    conjunction(
        atoms
            .iter()
            .map(|(atom, value)| {
                if *value {
                    atom.expression.clone()
                } else {
                    Expr::Unary {
                        op: UnaryOp::Not,
                        expr: Box::new(atom.expression.clone()),
                    }
                }
            })
            .collect(),
    )
}

fn numeric_qfree_vars(recon: &S2Recon) -> Vec<String> {
    recon
        .constants
        .iter()
        .filter(|constant| {
            matches!(
                constant.sort,
                Sort2::Mach(Mach::U8 | Mach::U16 | Mach::U32 | Mach::U64 | Mach::Usize)
            )
        })
        .map(|constant| constant.name.clone())
        .collect()
}

fn literal(value: u128) -> Expr {
    Expr::IntLit {
        value,
        raw: value.to_string(),
    }
}

fn binding_formula(values: &BTreeMap<String, u128>) -> Expr {
    conjunction(
        values
            .iter()
            .map(|(name, value)| Expr::Binary {
                op: BinOp::Eq,
                lhs: Box::new(Expr::Path(vec![name.clone()])),
                rhs: Box::new(literal(*value)),
            })
            .collect(),
    )
}

fn render_lia_term_smt(expression: &Expr) -> Result<String, String> {
    match expression {
        Expr::IntLit { value, .. } => Ok(value.to_string()),
        Expr::Path(path) if path.len() == 1 => Ok(path[0].clone()),
        Expr::Binary { op, lhs, rhs } => {
            let operator = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                _ => return Err(format!("`{op:?}` is not a QF_LIA term operator")),
            };
            Ok(format!(
                "({operator} {} {})",
                render_lia_term_smt(lhs)?,
                render_lia_term_smt(rhs)?
            ))
        }
        Expr::Cast { expr, .. } => render_lia_term_smt(expr),
        other => Err(format!("`{other:?}` is not a QF_LIA term")),
    }
}

fn render_lia_prop_smt(expression: &Expr) -> Result<String, String> {
    match expression {
        Expr::BoolLit(value) => Ok(value.to_string()),
        Expr::Binary { op, lhs, rhs } => match op {
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let operator = match op {
                    BinOp::Eq | BinOp::Ne => "=",
                    BinOp::Lt => "<",
                    BinOp::Le => "<=",
                    BinOp::Gt => ">",
                    BinOp::Ge => ">=",
                    _ => unreachable!("the outer match fixed the relation"),
                };
                let relation = format!(
                    "({operator} {} {})",
                    render_lia_term_smt(lhs)?,
                    render_lia_term_smt(rhs)?
                );
                if *op == BinOp::Ne {
                    Ok(format!("(not {relation})"))
                } else {
                    Ok(relation)
                }
            }
            BinOp::And | BinOp::Or => {
                let operator = if *op == BinOp::And { "and" } else { "or" };
                Ok(format!(
                    "({operator} {} {})",
                    render_lia_prop_smt(lhs)?,
                    render_lia_prop_smt(rhs)?
                ))
            }
            other => Err(format!("`{other:?}` is not a QF_LIA proposition operator")),
        },
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => Ok(format!("(not {})", render_lia_prop_smt(expr)?)),
        other => Err(format!("`{other:?}` is not a QF_LIA proposition")),
    }
}

fn lia_model_query(vars: &[String], formula: &Expr) -> Result<String, String> {
    let mut query = String::from("(set-logic QF_LIA)\n(set-option :timeout 30000)\n");
    for variable in vars {
        query.push_str(&format!("(declare-const {variable} Int)\n"));
        query.push_str(&format!("(assert (>= {variable} 0))\n"));
    }
    query.push_str(&format!("(assert {})\n", render_lia_prop_smt(formula)?));
    query.push_str("(check-sat)\n(get-model)\n");
    Ok(query)
}

fn run_z3(query: &str) -> Result<(String, String), String> {
    let z3 = solver_binary("THERMITE_EPR_Z3", "z3");
    run_z3_at(&z3, query)
}

fn run_z3_at(z3: &Path, query: &str) -> Result<(String, String), String> {
    let mut child = Command::new(z3)
        .arg("-in")
        .arg("-smt2")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not invoke `{}`: {error}", z3.display()))?;
    let Some(mut stdin) = child.stdin.take() else {
        return Err("z3 did not expose stdin".to_string());
    };
    stdin
        .write_all(query.as_bytes())
        .map_err(|error| format!("could not send the QF realization query to z3: {error}"))?;
    drop(stdin);
    let output = child
        .wait_with_output()
        .map_err(|error| format!("could not collect the z3 model: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let result = stdout
        .lines()
        .find(|line| matches!(*line, "sat" | "unsat" | "unknown"))
        .map(str::to_string);
    if !output.status.success() && !matches!(result.as_deref(), Some("unsat" | "unknown")) {
        return Err(format!(
            "z3 exited {:?}: {}{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(800)
                .collect::<String>(),
            stdout.chars().take(800).collect::<String>()
        ));
    }
    let result = result.ok_or_else(|| format!("z3 returned no satisfiability result: {stdout}"))?;
    Ok((result, stdout))
}

fn matching_parenthesis(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, byte) in text.as_bytes()[open..].iter().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_z3_int_value(text: &str) -> Option<i128> {
    let text = text.trim();
    if let Some(negative) = text
        .strip_prefix("(-")
        .and_then(|value| value.strip_suffix(')'))
    {
        return negative.trim().parse::<i128>().ok()?.checked_neg();
    }
    text.parse().ok()
}

fn parse_z3_int_model(model: &str, vars: &[String]) -> Result<BTreeMap<String, u128>, String> {
    let mut values = BTreeMap::new();
    for variable in vars {
        let needle = format!("(define-fun {variable} ");
        let value = if let Some(open) = model.find(&needle) {
            let close = matching_parenthesis(model, open)
                .ok_or_else(|| format!("unterminated z3 binding for `{variable}`"))?;
            let definition = &model[open + needle.len()..close];
            let after_sort = definition
                .find("Int")
                .map(|position| &definition[position + 3..])
                .ok_or_else(|| format!("z3 binding for `{variable}` is not integer-valued"))?;
            parse_z3_int_value(after_sort)
                .ok_or_else(|| format!("could not decode z3 integer value `{after_sort}`"))?
        } else {
            0
        };
        values.insert(
            variable.clone(),
            u128::try_from(value)
                .map_err(|_| format!("z3 produced negative unsigned value {value}"))?,
        );
    }
    Ok(values)
}

fn checked_qfree_group(
    item: &FnItem,
    suffix: &str,
    vars: &[String],
    desired: Expr,
    values: BTreeMap<String, u128>,
    fragment: SmtFragment,
    solver_query: &str,
) -> Result<QfreeGroupRealization, String> {
    let outcome = crate::lean_smt_export::reconstruct_validity(
        &SmtValidityObligation {
            item: format!("{}_qfree_{suffix}", item.name),
            vars: vars.to_vec(),
            req: binding_formula(&values),
            clause: desired,
            fragment,
        },
        Some(solver_query),
    );
    let ReconstructionOutcome::Checked(evidence) = outcome else {
        return Err(format!(
            "Lean rejected the concrete {suffix} QFree realization: {outcome:?}"
        ));
    };
    let witness = values
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(",");
    Ok(QfreeGroupRealization {
        evidence,
        witness: format!("{suffix}{{{witness}}}"),
        values,
    })
}

fn realize_lia_group(
    item: &FnItem,
    vars: &[String],
    atoms: &[(&QFreeAtom, bool)],
) -> Result<QfreeGroupRealization, String> {
    let desired = expected_qfree_formula(atoms);
    let query = lia_model_query(vars, &desired)?;
    let (result, model) =
        run_z3(&query).map_err(|reason| format!("EprQFreeSolverUnavailable: {reason}"))?;
    if result != "sat" {
        return Err(format!(
            "EprQFreeUnrealizable: QF_LIA assignment is {result}"
        ));
    }
    let values = parse_z3_int_model(&model, vars)?;
    checked_qfree_group(
        item,
        "qf_lia",
        vars,
        desired,
        values,
        SmtFragment::Lia,
        &query,
    )
}

fn realize_bv_group_with_values(
    item: &FnItem,
    vars: &[String],
    width: BvWidth,
    atoms: &[(&QFreeAtom, bool)],
    fixed_values: &BTreeMap<String, u128>,
) -> Result<QfreeGroupRealization, String> {
    let desired = expected_qfree_formula(atoms);
    let query_formula = if fixed_values.is_empty() {
        desired.clone()
    } else {
        conjunction(vec![binding_formula(fixed_values), desired.clone()])
    };
    let false_clause = Expr::BoolLit(false);
    let (query, _) =
        crate::bitvector::validity_query(vars, Some(&query_formula), &false_clause, width)?;
    let outcome = crate::bitvector::BitVectorEngine::new().discharge_bv(
        vars,
        Some(&query_formula),
        &false_clause,
        width,
    );
    let crate::bitvector::BvOutcome::Counterexample { bits } = outcome else {
        return Err(format!(
            "EprQFreeUnrealizable: QF_BV{} assignment produced {outcome:?}",
            width.bits()
        ));
    };
    let values = if fixed_values.is_empty() {
        bits.into_iter()
            .map(|pattern| (pattern.var, pattern.value))
            .collect()
    } else {
        fixed_values.clone()
    };
    checked_qfree_group(
        item,
        &format!("qf_bv{}", width.bits()),
        vars,
        desired,
        values,
        SmtFragment::Bv(width),
        &query,
    )
}

fn check_qfree_realization(
    recon: &S2Recon,
    item: &FnItem,
    values: Vec<bool>,
) -> Result<QfreeRealization, String> {
    if values.len() != recon.qfree_atoms.len() {
        return Err("qfree assignment length does not match the canonical IR".to_string());
    }
    let vars = numeric_qfree_vars(recon);
    let atoms_with_values = recon
        .qfree_atoms
        .iter()
        .zip(values.iter().copied())
        .collect::<Vec<_>>();
    let mut checks = Vec::new();
    let mut witnesses = Vec::new();
    let mut joint_values = BTreeMap::new();

    let lia = atoms_with_values
        .iter()
        .copied()
        .filter(|(atom, _)| atom.fragment == QFreeFragment::Lia)
        .collect::<Vec<_>>();
    if !lia.is_empty() {
        let group = realize_lia_group(item, &vars, &lia)?;
        joint_values = group.values;
        checks.push(group.evidence);
        witnesses.push(group.witness);
    }
    for width in [BvWidth::W8, BvWidth::W16, BvWidth::W32, BvWidth::W64] {
        let group = atoms_with_values
            .iter()
            .copied()
            .filter(|(atom, _)| atom.fragment == QFreeFragment::Bv(width))
            .collect::<Vec<_>>();
        if group.is_empty() {
            continue;
        }
        let realized = realize_bv_group_with_values(item, &vars, width, &group, &joint_values)?;
        if joint_values.is_empty() {
            joint_values = realized.values.clone();
        }
        checks.push(realized.evidence);
        witnesses.push(realized.witness);
    }
    Ok(QfreeRealization {
        values,
        checks,
        witnesses,
    })
}

fn checked_counterexample(
    recon: &S2Recon,
    item: &FnItem,
    scratch: &Path,
    premise: &str,
    conclusion: &str,
    ground: &GroundMetadata,
    qfree_values: Vec<bool>,
) -> Result<FiniteCountermodel, CountermodelAttemptFailure> {
    let qfree = check_qfree_realization(recon, item, qfree_values).map_err(|reason| {
        if reason.starts_with("EprQFreeUnrealizable:") {
            CountermodelAttemptFailure::Retry(reason)
        } else {
            CountermodelAttemptFailure::Fatal(format!("EprCountermodelQFreeRealization: {reason}"))
        }
    })?;
    let (seed, atoms, mut axioms) =
        check_bool_countermodel(scratch, premise, conclusion, ground, &qfree.values).map_err(
            |reason| {
                if reason.starts_with("no source countermodel was found") {
                    CountermodelAttemptFailure::Retry(reason)
                } else {
                    CountermodelAttemptFailure::Fatal(format!(
                        "EprCountermodelRealization: the propositional problem is SAT, \
                         but no checked typed model was produced: {reason}"
                    ))
                }
            },
        )?;
    for axiom in qfree
        .checks
        .iter()
        .flat_map(|evidence| evidence.axioms.iter())
    {
        if !axioms.contains(axiom) {
            axioms.push(axiom.clone());
        }
    }
    let qfree_checks = qfree
        .checks
        .iter()
        .zip(&qfree.witnesses)
        .map(|(evidence, witness)| {
            format!("{} via {} ({witness})", evidence.fragment, evidence.checker)
        })
        .collect();
    Ok(FiniteCountermodel {
        model: format!(
            "two-element typed model seed {seed} (Lean searched constants, unary functions, \
             order relations, and injective sequence views)"
        ),
        universe_count: ground.ground_count,
        universe_sha256: sha256_hex(ground.ground.as_bytes()),
        atoms,
        cnf_sha256: sha256_hex(ground.dimacs.as_bytes()),
        qfree_checks,
        axioms,
    })
}

fn qfree_values_source(values: &[bool]) -> String {
    let mut source = String::from("private def counterQfree : Nat → Bool\n");
    for (id, value) in values.iter().enumerate() {
        source.push_str(&format!("  | {id} => {value}\n"));
    }
    source.push_str("  | _ => false\n");
    source
}

fn countermodel_search_source(premise: &str, conclusion: &str, qfree_values: &[bool]) -> String {
    format!(
        r#"import Thermite.Strat.TestModel

{}

{}

def main : IO Unit := do
  let found := (List.range {COUNTERMODEL_SEEDS}).find? fun seed =>
    evalFrm (searchedBoolModelWithQfree seed counterQfree) premise
        (emptySearchedBoolValuationWithQfree seed counterQfree) &&
      !evalFrm (searchedBoolModelWithQfree seed counterQfree) conclusion
        (emptySearchedBoolValuationWithQfree seed counterQfree)
  match found with
  | some seed => IO.println s!"THERMITE-COUNTERMODEL-SEED={{seed}}"
  | none => IO.println "THERMITE-COUNTERMODEL-SEED=none"
"#,
        common_source(premise, conclusion),
        qfree_values_source(qfree_values),
    )
}

fn countermodel_replay_source(
    premise: &str,
    conclusion: &str,
    seed: usize,
    qfree_values: &[bool],
) -> String {
    format!(
        r#"import Thermite.Strat.TestModel

{}

{}

private def counterSeed : Nat := {seed}
private def counterModel : Model :=
  searchedBoolModelWithQfree counterSeed counterQfree
private def counterValuation : Valuation counterModel :=
  emptySearchedBoolValuationWithQfree counterSeed counterQfree
private def counterInterpretation : GroundInterpretation counterModel where
  qfree := counterQfree
  skolem := fun _ _ result => counterModel.default result

theorem thermiteEprCountermodel :
    evalFrm counterModel premise counterValuation = true ∧
      evalFrm counterModel conclusion counterValuation = false := by
  decide

#print axioms thermiteEprCountermodel

def main : IO Unit := do
  for atom in eprAtoms skeleton do
    IO.println s!"THERMITE-MODEL-ATOM={{
      evalGroundAtom counterModel counterInterpretation atom}}|{{
      (repr atom).pretty 1000000}}"
"#,
        common_source(premise, conclusion),
        qfree_values_source(qfree_values),
    )
}

fn check_bool_countermodel(
    scratch: &Path,
    premise: &str,
    conclusion: &str,
    ground: &GroundMetadata,
    qfree_values: &[bool],
) -> Result<(usize, Vec<GroundAtomValue>, Vec<String>), String> {
    let search_source = countermodel_search_source(premise, conclusion, qfree_values);
    let search_path = scratch.join("countermodel-search.lean");
    fs::write(&search_path, search_source.as_bytes())
        .map_err(|error| format!("could not write countermodel search driver: {error}"))?;
    let search_output = run_lean(&search_path, true)?;
    let seed = search_output
        .lines()
        .find_map(|line| line.strip_prefix("THERMITE-COUNTERMODEL-SEED="))
        .ok_or("countermodel search did not report a result")?;
    if seed == "none" {
        return Err(format!(
            "no source countermodel was found in the {COUNTERMODEL_SEEDS}-member \
             checked finite-model family"
        ));
    }
    let seed = seed
        .parse::<usize>()
        .map_err(|error| format!("countermodel search returned invalid seed `{seed}`: {error}"))?;

    let replay_source = countermodel_replay_source(premise, conclusion, seed, qfree_values);
    let replay_path = scratch.join("countermodel-replay.lean");
    fs::write(&replay_path, replay_source.as_bytes())
        .map_err(|error| format!("could not write countermodel driver: {error}"))?;
    let output = run_lean(&replay_path, true)?;
    let axioms = parse_axioms(&output, "thermiteEprCountermodel")?;
    let mut atoms = Vec::new();
    for line in output.lines() {
        let Some(value) = line.strip_prefix("THERMITE-MODEL-ATOM=") else {
            continue;
        };
        let (value, atom) = value
            .split_once('|')
            .ok_or_else(|| format!("malformed model atom output `{line}`"))?;
        let value = match value {
            "true" => true,
            "false" => false,
            other => return Err(format!("invalid model truth value `{other}`")),
        };
        atoms.push(GroundAtomValue {
            atom: atom.to_string(),
            value,
        });
    }
    if atoms.len() != ground.atoms.len() {
        return Err(format!(
            "Lean evaluated {} atoms, but the recomputed problem contains {}",
            atoms.len(),
            ground.atoms.len()
        ));
    }
    Ok((seed, atoms, axioms))
}

fn ensure_lean_reconstruction_modules() -> Result<(), String> {
    LEAN_MODULE_BUILD
        .get_or_init(|| {
            let lake = lake_binary();
            let output = Command::new(&lake)
                .arg("build")
                .arg("Thermite.Strat.EprReplay")
                .arg("Thermite.Strat.TestModel")
                .current_dir(lean_root())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(|error| {
                    format!(
                        "could not invoke `{}` to build reconstruction modules: {error}",
                        lake.display()
                    )
                })?;
            if output.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "`lake build` exited {:?}: {}",
                    output.status.code(),
                    output_head(&output)
                ))
            }
        })
        .clone()
}

fn run_lean(source: &Path, run_main: bool) -> Result<String, String> {
    let lake = lake_binary();
    let mut command = Command::new(&lake);
    command
        .arg("env")
        .arg("lean")
        // A single worker keeps the 64 MiB stack below the reconstruction gate's
        // memory ceiling, even on hosts with many logical CPUs.
        .arg("--threads=1")
        .arg("--tstack=65536");
    if run_main {
        command.arg("--run");
    }
    let output = command
        .arg(source)
        .current_dir(lean_root())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("could not invoke `{}`: {error}", lake.display()))?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if output.status.success() {
        Ok(combined)
    } else {
        Err(format!(
            "Lean exited {:?}: {}",
            output.status.code(),
            combined.chars().take(1200).collect::<String>()
        ))
    }
}

fn verify_solver_toolchain() -> Result<SolverToolchain, String> {
    let cadical = solver_binary("THERMITE_EPR_CADICAL", "cadical");
    let drat_trim = solver_binary("THERMITE_EPR_DRAT_TRIM", "drat-trim");
    verify_solver_toolchain_at(&cadical, &drat_trim)?;
    Ok(SolverToolchain { cadical, drat_trim })
}

fn verify_solver_toolchain_at(cadical: &Path, drat_trim: &Path) -> Result<(), String> {
    let cadical_version = Command::new(cadical)
        .arg("--version")
        .output()
        .map_err(|error| {
            format!(
                "EprSolverUnavailable: could not invoke pinned `{}`: {error}",
                cadical.display()
            )
        })?;
    if !cadical_version.status.success() {
        return Err(format!(
            "EprSolverVersion: `{}` could not report its pinned version: {}",
            cadical.display(),
            output_head(&cadical_version)
        ));
    }
    let actual_cadical = String::from_utf8_lossy(&cadical_version.stdout);
    if actual_cadical.trim() != CADICAL_VERSION {
        return Err(format!(
            "EprSolverVersion: expected CaDiCaL {CADICAL_VERSION} \
             ({CADICAL_REVISION}), found `{}`",
            actual_cadical.trim()
        ));
    }

    let drat_version = Command::new(drat_trim)
        .arg("--thermite-version")
        .output()
        .map_err(|error| {
            format!(
                "EprLratToolUnavailable: could not invoke pinned `{}`: {error}",
                drat_trim.display()
            )
        })?;
    let expected_drat = format!("drat-trim {DRAT_TRIM_REVISION}");
    if !drat_version.status.success()
        || String::from_utf8_lossy(&drat_version.stdout).trim() != expected_drat
    {
        return Err(format!(
            "EprLratToolVersion: expected `{expected_drat}`, found {}",
            output_head(&drat_version)
        ));
    }
    Ok(())
}

fn solver_binary(environment: &str, name: &str) -> PathBuf {
    if let Some(configured) = std::env::var_os(environment) {
        return PathBuf::from(configured);
    }
    let pinned = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("target")
        .join("g4-tools")
        .join("bin")
        .join(name);
    if pinned.is_file() {
        pinned
    } else {
        PathBuf::from(name)
    }
}

fn run_cadical(cadical: &Path, cnf: &Path, proof: &Path, model: &Path) -> Result<Output, String> {
    Command::new(cadical)
        .arg("-t")
        .arg(SOLVER_SECONDS.to_string())
        .arg("-w")
        .arg(model)
        .arg(cnf)
        .arg(proof)
        .output()
        .map_err(|error| {
            format!(
                "EprSolverUnavailable: could not invoke pinned `{}`: {error}",
                cadical.display()
            )
        })
}

fn run_drat_trim(
    drat_trim: &Path,
    cnf: &Path,
    proof: &Path,
    lrat: &Path,
) -> Result<Output, String> {
    Command::new(drat_trim)
        .arg(cnf)
        .arg(proof)
        .arg("-t")
        .arg(SOLVER_SECONDS.to_string())
        .arg("-L")
        .arg(lrat)
        .output()
        .map_err(|error| {
            format!(
                "EprLratToolUnavailable: could not invoke pinned `{}`: {error}",
                drat_trim.display()
            )
        })
}

fn parse_ground_output(output: &str) -> Result<GroundMetadata, String> {
    let begin = output
        .find("THERMITE-DIMACS-BEGIN\n")
        .ok_or("missing DIMACS begin marker")?
        + "THERMITE-DIMACS-BEGIN\n".len();
    let end = output[begin..]
        .find("THERMITE-DIMACS-END")
        .map(|offset| begin + offset)
        .ok_or("missing DIMACS end marker")?;
    let dimacs = output[begin..end].to_string();
    let value = |prefix: &str| -> Result<String, String> {
        output
            .lines()
            .find_map(|line| line.strip_prefix(prefix).map(ToOwned::to_owned))
            .ok_or_else(|| format!("missing `{prefix}` metadata"))
    };
    if value("THERMITE-INSTANTIATION-VERIFIED=")? != "true" {
        return Err("Lean rejected its recomputed structural instantiation".to_string());
    }
    if value("THERMITE-THEORY-VERIFIED=")? != "true" {
        return Err("Lean rejected its recomputed theory closure".to_string());
    }
    let parse_count = |prefix: &str| -> Result<usize, String> {
        value(prefix)?
            .parse()
            .map_err(|error| format!("invalid `{prefix}` count: {error}"))
    };
    let mut atoms = Vec::new();
    for line in output.lines() {
        let Some(rest) = line.strip_prefix("THERMITE-ATOM=") else {
            continue;
        };
        let (variable, atom) = rest
            .split_once('|')
            .ok_or_else(|| format!("malformed atom metadata `{line}`"))?;
        atoms.push((
            variable
                .parse()
                .map_err(|error| format!("invalid atom variable `{variable}`: {error}"))?,
            atom.to_string(),
        ));
    }
    Ok(GroundMetadata {
        dimacs,
        order: value("THERMITE-ORDER=")?,
        ground: value("THERMITE-GROUND=")?,
        formula: value("THERMITE-FORMULA=")?,
        theory: value("THERMITE-THEORY=")?,
        problem: value("THERMITE-PROBLEM=")?,
        bool_problem: value("THERMITE-BOOL-PROBLEM=")?,
        atoms,
        ground_count: parse_count("THERMITE-GROUND-COUNT=")?,
        instantiation_count: parse_count("THERMITE-INSTANTIATION-COUNT=")?,
        theory_count: parse_count("THERMITE-THEORY-COUNT=")?,
    })
}

fn parse_sat_assignment(model: &str) -> Result<Vec<bool>, String> {
    let mut max = 0usize;
    let mut literals = Vec::new();
    for token in model.split_whitespace() {
        let Ok(literal) = token.parse::<i64>() else {
            continue;
        };
        if literal == 0 {
            continue;
        }
        let variable = usize::try_from(literal.unsigned_abs())
            .map_err(|_| "model variable does not fit usize")?;
        max = max.max(variable);
        literals.push((variable, literal > 0));
    }
    if literals.is_empty() {
        return Err("model contains no signed DIMACS literals".to_string());
    }
    let mut assignment = vec![false; max + 1];
    for (variable, value) in literals {
        assignment[variable] = value;
    }
    Ok(assignment)
}

fn ground_qfree_id(atom: &str) -> Option<usize> {
    atom.strip_prefix("Thermite.Strat.Cls.GroundAtom.qfree ")?
        .trim()
        .parse()
        .ok()
}

fn qfree_values_from_assignment(
    recon: &S2Recon,
    ground: &GroundMetadata,
    assignment: &[bool],
) -> Result<Vec<bool>, String> {
    let mut values = vec![None; recon.qfree_atoms.len()];
    for (variable, atom) in &ground.atoms {
        let Some(id) = ground_qfree_id(atom) else {
            continue;
        };
        let slot = values
            .get_mut(id)
            .ok_or_else(|| format!("ground problem contains unknown qfree id {id}"))?;
        let value = assignment
            .get(*variable)
            .copied()
            .ok_or_else(|| format!("SAT model omits qfree DIMACS variable {variable}"))?;
        if slot.is_some_and(|existing| existing != value) {
            return Err(format!(
                "SAT model assigns conflicting values to qfree id {id}"
            ));
        }
        *slot = Some(value);
    }
    values
        .into_iter()
        .enumerate()
        .map(|(id, value)| value.ok_or_else(|| format!("ground problem omits qfree id {id}")))
        .collect()
}

fn qfree_blocking_clause(
    qfree_count: usize,
    ground: &GroundMetadata,
    assignment: &[bool],
) -> Result<Vec<i64>, String> {
    let mut clause = Vec::new();
    let mut seen_ids = vec![false; qfree_count];
    for (variable, atom) in &ground.atoms {
        let Some(id) = ground_qfree_id(atom) else {
            continue;
        };
        let seen = seen_ids
            .get_mut(id)
            .ok_or_else(|| format!("ground problem contains unknown qfree id {id}"))?;
        *seen = true;
        let variable_id = *variable;
        let value = assignment
            .get(variable_id)
            .copied()
            .ok_or_else(|| format!("SAT model omits qfree DIMACS variable {variable_id}"))?;
        let variable = i64::try_from(variable_id)
            .map_err(|_| format!("qfree DIMACS variable {variable_id} does not fit i64"))?;
        if variable == 0 {
            return Err("qfree DIMACS variable 0 is invalid".to_string());
        }
        clause.push(if value { -variable } else { variable });
    }
    if let Some(missing) = seen_ids.iter().position(|seen| !seen) {
        return Err(format!("ground problem omits qfree id {missing}"));
    }
    clause.sort_unstable_by_key(|literal| literal.unsigned_abs());
    clause.dedup();
    if clause.is_empty() {
        return Err("the rejected model has no QFree variables to block".to_string());
    }
    Ok(clause)
}

fn dimacs_with_blocking_clauses(dimacs: &str, blocks: &[Vec<i64>]) -> Result<String, String> {
    if blocks.is_empty() {
        return Ok(dimacs.to_string());
    }
    let mut output = String::new();
    let mut header_found = false;
    let mut variable_count = None;
    for line in dimacs.lines() {
        if line.trim_start().starts_with("p cnf ") {
            if header_found {
                return Err("DIMACS contains more than one problem header".to_string());
            }
            let fields = line.split_whitespace().collect::<Vec<_>>();
            if fields.len() != 4 || fields[0] != "p" || fields[1] != "cnf" {
                return Err(format!("malformed DIMACS problem header `{line}`"));
            }
            let variables = fields[2]
                .parse::<usize>()
                .map_err(|error| format!("invalid DIMACS variable count: {error}"))?;
            let clauses = fields[3]
                .parse::<usize>()
                .map_err(|error| format!("invalid DIMACS clause count: {error}"))?;
            let clauses = clauses
                .checked_add(blocks.len())
                .ok_or("DIMACS clause count overflow")?;
            output.push_str(&format!("p cnf {variables} {clauses}\n"));
            variable_count = Some(variables);
            header_found = true;
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    let variables = variable_count.ok_or("DIMACS has no problem header")?;
    for block in blocks {
        if block.is_empty() {
            return Err("a QFree blocking clause is empty".to_string());
        }
        for literal in block {
            let variable = usize::try_from(literal.unsigned_abs())
                .map_err(|_| format!("blocking literal {literal} does not fit usize"))?;
            if variable == 0 || variable > variables {
                return Err(format!(
                    "blocking literal {literal} exceeds DIMACS variable count {variables}"
                ));
            }
            output.push_str(&format!("{literal} "));
        }
        output.push_str("0\n");
    }
    Ok(output)
}

/// Deletion steps are an LRAT space optimization, not part of the
/// refutation. Retaining those clauses avoids exercising the checker's
/// partial array deletion primitive and leaves every later RUP hint valid.
fn strip_lrat_deletions(proof: &str) -> String {
    proof
        .lines()
        .filter(|line| line.split_whitespace().nth(1) != Some("d"))
        .map(|line| format!("{line}\n"))
        .collect()
}

fn validate_dimacs_assignment(dimacs: &str, assignment: &[bool]) -> Result<(), String> {
    for (line_number, line) in dimacs.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('c') || line.starts_with('p') {
            continue;
        }
        let mut satisfied = false;
        let mut terminated = false;
        for token in line.split_whitespace() {
            let literal: i64 = token
                .parse()
                .map_err(|error| format!("line {} is not DIMACS: {error}", line_number + 1))?;
            if literal == 0 {
                terminated = true;
                break;
            }
            let variable = usize::try_from(literal.unsigned_abs())
                .map_err(|_| format!("line {} variable is too large", line_number + 1))?;
            let value = assignment.get(variable).copied().unwrap_or(false);
            satisfied |= if literal > 0 { value } else { !value };
        }
        if !terminated {
            return Err(format!(
                "DIMACS clause {} has no terminator",
                line_number + 1
            ));
        }
        if !satisfied {
            return Err(format!(
                "CaDiCaL assignment falsifies DIMACS clause {}",
                line_number + 1
            ));
        }
    }
    Ok(())
}

fn parse_axioms(output: &str, theorem: &str) -> Result<Vec<String>, String> {
    let anchor = format!("'{theorem}' depends on axioms:");
    let start = output
        .find(&anchor)
        .ok_or_else(|| format!("missing anchored `#print axioms {theorem}` report"))?;
    let report = &output[start..];
    let list = report
        .split_once('[')
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(inside, _)| inside)
        .ok_or("malformed axiom report")?;
    let axioms = list
        .split(',')
        .map(str::trim)
        .filter(|axiom| !axiom.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if let Some(disallowed) = axioms
        .iter()
        .find(|axiom| !AXIOM_ALLOWLIST.contains(&axiom.as_str()))
    {
        return Err(format!("non-allowlisted axiom `{disallowed}`"));
    }
    Ok(axioms)
}

#[allow(clippy::too_many_arguments)]
fn build_evidence(
    theorem: &str,
    final_source: &str,
    canonical: &str,
    source_clause: &str,
    ground: &GroundMetadata,
    lrat: &str,
    axioms: Vec<String>,
    started: Instant,
    cache_hit: bool,
) -> ReconstructionEvidence {
    let source_sha256 = sha256_hex(final_source.as_bytes());
    let canonical_ir_sha256 = sha256_hex(canonical.as_bytes());
    let source_clause_sha256 = sha256_hex(source_clause.as_bytes());
    let ground_sha256 = sha256_hex(ground.ground.as_bytes());
    let instantiation_sha256 = sha256_hex(ground.formula.as_bytes());
    let theory_sha256 = sha256_hex(ground.theory.as_bytes());
    let propositional_problem = format!("{}\n{}", ground.problem, ground.bool_problem);
    let propositional_problem_sha256 = sha256_hex(propositional_problem.as_bytes());
    let cnf_sha256 = sha256_hex(ground.dimacs.as_bytes());
    let lrat_sha256 = sha256_hex(lrat.as_bytes());
    let axiom_report_sha256 = sha256_hex(axioms.join("\n").as_bytes());
    let ground_count = ground.ground_count.to_string();
    let instantiation_count = ground.instantiation_count.to_string();
    let theory_count = ground.theory_count.to_string();
    let verdict_key_sha256 = verdict_key(&[
        ("fragment", EPR_FRAGMENT),
        ("checker", EPR_CHECKER),
        ("theorem", theorem),
        ("source", &source_sha256),
        ("canonical-ir", &canonical_ir_sha256),
        ("source-clause", &source_clause_sha256),
        ("ground", &ground_sha256),
        ("instantiation", &instantiation_sha256),
        ("theory", &theory_sha256),
        ("propositional-problem", &propositional_problem_sha256),
        ("solver-query", &cnf_sha256),
        ("cnf", &cnf_sha256),
        ("lrat", &lrat_sha256),
        ("ground-count", &ground_count),
        ("instantiation-count", &instantiation_count),
        ("theory-count", &theory_count),
        ("axioms", &axiom_report_sha256),
        ("budget-outcome", "within-budget"),
    ]);
    let elapsed = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    ReconstructionEvidence {
        theorem: theorem.to_string(),
        source_sha256,
        fragment: EPR_FRAGMENT.to_string(),
        checker: EPR_CHECKER.to_string(),
        axioms,
        solver_query_sha256: Some(cnf_sha256.clone()),
        canonical_ir_sha256: Some(canonical_ir_sha256),
        source_clause_sha256: Some(source_clause_sha256),
        ground_sha256: Some(ground_sha256),
        instantiation_sha256: Some(instantiation_sha256),
        theory_sha256: Some(theory_sha256),
        propositional_problem_sha256: Some(propositional_problem_sha256),
        cnf_sha256: Some(cnf_sha256),
        lrat_sha256: Some(lrat_sha256),
        ground_universe_count: Some(ground.ground_count),
        instantiation_count: Some(ground.instantiation_count),
        theory_clause_count: Some(ground.theory_count),
        elapsed_ms: Some(elapsed),
        budget_outcome: Some("within-budget".to_string()),
        verdict_key_sha256: Some(verdict_key_sha256),
        cache_hit: Some(cache_hit),
    }
}

#[allow(clippy::too_many_arguments)]
fn try_cached_unsat(
    input_key: &str,
    canonical: &str,
    source_clause: &str,
    theorem: &str,
    premise: &str,
    conclusion: &str,
    scratch: &Path,
    started: Instant,
) -> Result<Option<ReconstructionEvidence>, String> {
    if std::env::var_os("THERMITE_EPR_CACHE_DISABLE").is_some() {
        return Ok(None);
    }
    let cache = epr_cache_dir();
    let strict = std::env::var_os("THERMITE_EPR_CACHE_STRICT").is_some();
    try_cached_unsat_at(
        &cache,
        strict,
        input_key,
        canonical,
        source_clause,
        theorem,
        premise,
        conclusion,
        scratch,
        started,
    )
}

#[allow(clippy::too_many_arguments)]
fn try_cached_unsat_at(
    cache: &Path,
    strict: bool,
    input_key: &str,
    canonical: &str,
    source_clause: &str,
    theorem: &str,
    premise: &str,
    conclusion: &str,
    scratch: &Path,
    started: Instant,
) -> Result<Option<ReconstructionEvidence>, String> {
    let index_path = cache.join("index").join(input_key);
    let verdict_key_sha256 = match fs::read_to_string(&index_path) {
        Ok(value) => value.trim().to_string(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return cache_failure(strict, format!("could not read cache index: {error}")),
    };
    if !is_sha256(&verdict_key_sha256) {
        return cache_failure(
            strict,
            "cache index does not contain a SHA-256 key".to_string(),
        );
    }
    let entry_path = cache
        .join("entries")
        .join(format!("{verdict_key_sha256}.json"));
    let entry_text = match fs::read_to_string(&entry_path) {
        Ok(value) => value,
        Err(error) => {
            return cache_failure(
                strict,
                format!("cache index points to an unreadable artifact entry: {error}"),
            )
        }
    };
    let entry: CachedUnsat = match serde_json::from_str(&entry_text) {
        Ok(entry) => entry,
        Err(error) => {
            return cache_failure(
                strict,
                format!("cache artifact entry is malformed: {error}"),
            )
        }
    };
    if entry.schema != EPR_CACHE_SCHEMA
        || entry.input_key_sha256 != input_key
        || entry.verdict_key_sha256 != verdict_key_sha256
        || entry.canonical != canonical
        || entry.source_clause != source_clause
        || entry.theorem != theorem
    {
        return cache_failure(
            strict,
            "cache artifact identity does not match the requested reconstruction".to_string(),
        );
    }

    // Recompute the finite grounding and DIMACS before trusting cached
    // evidence. The cache skips SAT search and LRAT conversion, never the Lean
    // bindings that make those artifacts meaningful.
    let driver_path = scratch.join("cached-ground.lean");
    fs::write(&driver_path, ground_driver_source(premise, conclusion))
        .map_err(|error| format!("EprCacheScratch: could not write ground driver: {error}"))?;
    let recomputed_ground =
        match run_lean(&driver_path, true).and_then(|output| parse_ground_output(&output)) {
            Ok(ground) => ground,
            Err(reason) => {
                return cache_failure(strict, format!("cached grounding replay failed: {reason}"))
            }
        };
    if recomputed_ground != entry.ground {
        return cache_failure(
            strict,
            "cached ground universe, theory, or CNF differs from Lean recomputation".to_string(),
        );
    }
    let expected_source = replay_source(theorem, premise, conclusion, &entry.lrat, &entry.ground);
    if expected_source != entry.final_source {
        return cache_failure(
            strict,
            "cached theorem source does not bind the requested formula and LRAT".to_string(),
        );
    }
    let replay_path = scratch.join("cached-replay.lean");
    fs::write(&replay_path, entry.final_source.as_bytes())
        .map_err(|error| format!("EprCacheScratch: could not write replay source: {error}"))?;
    let replay_output = match run_lean(&replay_path, false) {
        Ok(output) => output,
        Err(reason) => {
            return cache_failure(strict, format!("cached kernel replay failed: {reason}"))
        }
    };
    let axioms = match parse_axioms(&replay_output, theorem) {
        Ok(axioms) => axioms,
        Err(reason) => {
            return cache_failure(strict, format!("cached axiom report failed: {reason}"))
        }
    };
    let evidence = build_evidence(
        theorem,
        &entry.final_source,
        canonical,
        source_clause,
        &recomputed_ground,
        &entry.lrat,
        axioms,
        started,
        true,
    );
    if evidence.verdict_key_sha256.as_deref() != Some(entry.verdict_key_sha256.as_str()) {
        return cache_failure(
            strict,
            "cached verdict key does not cover the recomputed evidence".to_string(),
        );
    }
    Ok(Some(evidence))
}

fn cache_failure<T>(strict: bool, reason: String) -> Result<Option<T>, String> {
    if strict {
        Err(format!("EprCacheTampered: {reason}"))
    } else {
        Ok(None)
    }
}

fn store_cached_unsat(entry: &CachedUnsat) -> std::io::Result<()> {
    if std::env::var_os("THERMITE_EPR_CACHE_DISABLE").is_some() {
        return Ok(());
    }
    let cache = epr_cache_dir();
    store_cached_unsat_at(&cache, entry)
}

fn store_cached_unsat_at(cache: &Path, entry: &CachedUnsat) -> std::io::Result<()> {
    let entries = cache.join("entries");
    let indices = cache.join("index");
    fs::create_dir_all(&entries)?;
    fs::create_dir_all(&indices)?;
    let encoded = serde_json::to_vec_pretty(entry)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    atomic_cache_write(
        &entries.join(format!("{}.json", entry.verdict_key_sha256)),
        &encoded,
    )?;
    atomic_cache_write(
        &indices.join(&entry.input_key_sha256),
        entry.verdict_key_sha256.as_bytes(),
    )
}

fn atomic_cache_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    static CACHE_WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);
    let serial = CACHE_WRITE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_extension(format!("{}.{}.tmp", std::process::id(), serial));
    fs::write(&temporary, bytes)?;
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(error)
        }
    }
}

fn epr_cache_dir() -> PathBuf {
    std::env::var_os("THERMITE_EPR_CACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("target")
                .join("thermite-epr-cache")
        })
}

fn cache_dependency_hash() -> Result<String, String> {
    let root = lean_root();
    let mut files = Vec::new();
    collect_lean_dependencies(&root.join("Thermite"), &mut files)
        .map_err(|error| format!("could not inventory Lean dependencies: {error}"))?;
    for name in ["lake-manifest.json", "lean-toolchain", "lakefile.toml"] {
        let path = root.join(name);
        if path.is_file() {
            files.push(path);
        }
    }
    files.sort();
    let mut digest = Sha256::new();
    digest.update(b"thermite.epr.dependencies.v1");
    for path in files {
        let relative = path.strip_prefix(&root).unwrap_or(&path);
        let name = relative.to_string_lossy();
        let contents = fs::read(&path)
            .map_err(|error| format!("could not read `{}`: {error}", path.display()))?;
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name.as_bytes());
        digest.update((contents.len() as u64).to_le_bytes());
        digest.update(contents);
    }
    let rust_source = include_bytes!("epr_reconstruct.rs");
    digest.update((rust_source.len() as u64).to_le_bytes());
    digest.update(rust_source);
    Ok(format!("{:x}", digest.finalize()))
}

fn collect_lean_dependencies(path: &Path, output: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_lean_dependencies(&path, output)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("lean") {
            output.push(path);
        }
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn output_head(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .chars()
    .take(800)
    .collect()
}

fn is_kernel_budget(detail: &str) -> bool {
    detail.contains("maximum number of heartbeats")
        || detail.contains("maximum recursion depth")
        || detail.contains("deterministic timeout")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn cache_input_key(
    canonical: &str,
    source_clause: &str,
    premise: &str,
    conclusion: &str,
) -> Result<String, String> {
    let dependencies = cache_dependency_hash()?;
    Ok(verdict_key(&[
        ("schema", EPR_CACHE_SCHEMA),
        ("fragment", EPR_FRAGMENT),
        ("canonical-ir", &sha256_hex(canonical.as_bytes())),
        ("source-clause", &sha256_hex(source_clause.as_bytes())),
        ("premise", &sha256_hex(premise.as_bytes())),
        ("conclusion", &sha256_hex(conclusion.as_bytes())),
        ("lean-dependencies", &dependencies),
        ("forge-version", env!("CARGO_PKG_VERSION")),
    ]))
}

fn verdict_key(fields: &[(&str, &str)]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"thermite.epr.verdict-key.v1");
    for (name, value) in fields {
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name.as_bytes());
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn lean_ident(value: &str) -> String {
    let mut output = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if output.is_empty() || output.starts_with(|character: char| character.is_ascii_digit()) {
        output.insert(0, '_');
    }
    output
}

fn lean_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("lean")
}

fn lake_binary() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        let elan = PathBuf::from(home).join(".elan/bin/lake");
        if elan.is_file() {
            return elan;
        }
    }
    PathBuf::from("lake")
}

#[cfg(test)]
mod tests {
    use super::*;
    use thermite_syntax::{parse, Item};

    #[cfg(unix)]
    fn write_test_tool(path: &Path, output: &str, status: i32) {
        use std::os::unix::fs::PermissionsExt;

        fs::write(
            path,
            format!("#!/bin/sh\nprintf '%s\\n' '{output}'\nexit {status}\n"),
        )
        .expect("write test tool");
        let mut permissions = fs::metadata(path)
            .expect("test tool metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("make test tool executable");
    }

    #[test]
    fn source_toolchain_pins_match_the_runtime_checks() {
        let pins = include_str!("../../scripts/g4-toolchain.env");
        assert!(pins.contains(&format!("CADICAL_VERSION={CADICAL_VERSION}\n")));
        assert!(pins.contains(&format!("CADICAL_REV={CADICAL_REVISION}\n")));
        assert!(pins.contains(&format!("DRAT_TRIM_REV={DRAT_TRIM_REVISION}\n")));
    }

    #[cfg(unix)]
    #[test]
    fn solver_toolchain_rejects_missing_and_mismatched_executables() {
        let scratch = Scratch::new("toolchain-negative-tests").expect("test scratch");
        let missing = scratch.path.join("missing");
        let error =
            verify_solver_toolchain_at(&missing, &missing).expect_err("a missing solver must fail");
        assert!(error.starts_with("EprSolverUnavailable:"), "{error}");

        let cadical = scratch.path.join("cadical");
        let drat_trim = scratch.path.join("drat-trim");
        write_test_tool(&cadical, "0.0.0", 0);
        write_test_tool(&drat_trim, &format!("drat-trim {DRAT_TRIM_REVISION}"), 0);
        let error = verify_solver_toolchain_at(&cadical, &drat_trim)
            .expect_err("a mismatched SAT solver must fail");
        assert!(error.starts_with("EprSolverVersion:"), "{error}");

        write_test_tool(&cadical, CADICAL_VERSION, 0);
        write_test_tool(&drat_trim, "drat-trim wrong-revision", 0);
        let error = verify_solver_toolchain_at(&cadical, &drat_trim)
            .expect_err("a mismatched LRAT converter must fail");
        assert!(error.starts_with("EprLratToolVersion:"), "{error}");

        write_test_tool(&drat_trim, &format!("drat-trim {DRAT_TRIM_REVISION}"), 0);
        verify_solver_toolchain_at(&cadical, &drat_trim)
            .expect("the exact pinned identities must pass");
    }

    #[test]
    fn dimacs_model_validation_rejects_a_falsified_clause() {
        let cnf = "p cnf 2 2\n1 2 0\n-1 0\n";
        assert!(validate_dimacs_assignment(cnf, &[false, false, true]).is_ok());
        assert!(validate_dimacs_assignment(cnf, &[false, true, true]).is_err());
    }

    #[test]
    fn rejected_qfree_mask_is_blocked_without_changing_the_original_cnf() {
        let original = "p cnf 3 1\n1 2 3 0\n";
        let ground = GroundMetadata {
            dimacs: original.to_string(),
            order: String::new(),
            ground: String::new(),
            formula: String::new(),
            theory: String::new(),
            problem: String::new(),
            bool_problem: String::new(),
            atoms: vec![
                (1, "Thermite.Strat.Cls.GroundAtom.qfree 0".to_string()),
                (3, "Thermite.Strat.Cls.GroundAtom.qfree 1".to_string()),
            ],
            ground_count: 0,
            instantiation_count: 0,
            theory_count: 0,
        };
        let rejected = [false, true, false, false];
        let block =
            qfree_blocking_clause(2, &ground, &rejected).expect("complete QFree assignment");
        assert_eq!(block, vec![-1, 3]);

        let augmented =
            dimacs_with_blocking_clauses(original, &[block]).expect("well-formed blocking CNF");
        assert!(augmented.starts_with("p cnf 3 2\n"));
        assert!(augmented.ends_with("-1 3 0\n"));
        assert!(validate_dimacs_assignment(original, &rejected).is_ok());
        assert!(validate_dimacs_assignment(&augmented, &rejected).is_err());
        assert!(validate_dimacs_assignment(&augmented, &[false, true, false, true]).is_ok());
        assert_eq!(original, ground.dimacs);
    }

    #[cfg(unix)]
    #[test]
    fn qfree_lia_solver_dependency_is_not_optional() {
        let scratch = Scratch::new("missing-z3-negative-test").expect("test scratch");
        let missing = scratch.path.join("missing-z3");
        let error = run_z3_at(&missing, "(check-sat)\n")
            .expect_err("a missing QF_LIA witness solver must fail");
        assert!(error.contains("could not invoke"), "{error}");
        assert!(error.contains("missing-z3"), "{error}");
    }

    #[test]
    fn qfree_model_parsers_keep_ids_and_signed_integer_values_exact() {
        assert_eq!(
            ground_qfree_id("Thermite.Strat.Cls.GroundAtom.qfree 17"),
            Some(17)
        );
        assert_eq!(ground_qfree_id("GroundAtom.rel eq"), None);

        let model = "(model\n\
          (define-fun x () Int\n\
            3)\n\
          (define-fun y () Int\n\
            (- 2))\n\
        )";
        let parsed =
            parse_z3_int_model(model, &["x".to_string()]).expect("non-negative integer model");
        assert_eq!(parsed["x"], 3);
        let negative = parse_z3_int_model(model, &["y".to_string()])
            .expect_err("unsigned QF models reject negative values");
        assert!(
            negative.contains("negative unsigned value -2"),
            "{negative}"
        );
    }

    #[test]
    fn epr_surface_requires_a_binder_or_relation_array_term() {
        let scalar = Frm::Atom(Atom::Rel(
            Rel::Eq,
            Tm::Const(Sort2::Mach(Mach::U64), 0),
            Tm::Const(Sort2::Mach(Mach::U64), 1),
        ));
        assert!(!needs_reconstruction(&scalar));
        assert!(needs_reconstruction(&Frm::All(
            Sort2::Mach(Mach::U64),
            Box::new(scalar)
        )));
    }

    #[test]
    fn production_reconstructs_an_admitted_array_clause() {
        let parsed = parse(
            "fn epr(xs: Vec<u64>) -> u64\n\
             req true\n\
             ens forall (i : usize) in xs. xs[i] == xs[i]\n\
             fx pure { 0 }",
        );
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        let Item::Fn(item) = &parsed.program.items[0] else {
            panic!("expected function");
        };
        let recon = thermite_spec::s2_recon_from_obligation(
            &parsed.program,
            item,
            &item.contract.req,
            &item.contract.ens[0],
            thermite_spec::SourceAddress {
                item: item.name.clone(),
                clause: "ens#0".to_string(),
            },
        )
        .expect("canonical S₂.0 bridge");
        assert!(needs_reconstruction(&recon.formula));
        match reconstruct(&recon, item, &item.contract.req, &item.contract.ens[0]) {
            EprOutcome::Proved(evidence) => {
                assert_eq!(evidence.fragment, EPR_FRAGMENT);
                assert_eq!(evidence.budget_outcome.as_deref(), Some("within-budget"));
            }
            other => panic!("expected checked reconstruction, found {other:?}"),
        }
    }

    #[test]
    fn production_reconstructs_sequence_extensionality() {
        let parsed = parse(
            "fn epr_ext(xs: Vec<u64>, ys: Vec<u64>) -> u64\n\
             req xs.len() == ys.len() && \
               forall (i : usize) in xs. xs[i] == ys[i]\n\
             ens xs == ys\n\
             fx pure { 0 }",
        );
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        let Item::Fn(item) = &parsed.program.items[0] else {
            panic!("expected function");
        };
        let recon = thermite_spec::s2_recon_from_obligation(
            &parsed.program,
            item,
            &item.contract.req,
            &item.contract.ens[0],
            thermite_spec::SourceAddress {
                item: item.name.clone(),
                clause: "ens#0".to_string(),
            },
        )
        .expect("canonical S₂.0 bridge");
        assert!(needs_reconstruction(&recon.formula));
        match reconstruct(&recon, item, &item.contract.req, &item.contract.ens[0]) {
            EprOutcome::Proved(evidence) => {
                assert_eq!(evidence.fragment, EPR_FRAGMENT);
                assert!(
                    evidence.theory_clause_count.unwrap_or_default() > 0,
                    "sequence extensionality must contribute checked theory clauses"
                );
            }
            other => panic!("expected checked extensionality reconstruction, found {other:?}"),
        }
    }

    #[test]
    fn production_returns_a_countermodel_with_checked_lia_leaves() {
        let parsed = parse(
            "fn epr_qfree(xs: Vec<u64>, x: u64) -> u64\n\
             req x + x == 2\n\
             ens (x + x == 4) && \
               forall (i : usize) in xs. xs[i] == xs[i]\n\
             fx pure { 0 }",
        );
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        let Item::Fn(item) = &parsed.program.items[0] else {
            panic!("expected function");
        };
        let recon = thermite_spec::s2_recon_from_obligation(
            &parsed.program,
            item,
            &item.contract.req,
            &item.contract.ens[0],
            thermite_spec::SourceAddress {
                item: item.name.clone(),
                clause: "ens#0".to_string(),
            },
        )
        .expect("canonical S₂.0 bridge");
        assert_eq!(recon.qfree_atoms.len(), 2);
        match reconstruct(&recon, item, &item.contract.req, &item.contract.ens[0]) {
            EprOutcome::Counterexample(model) => {
                assert!(
                    model
                        .qfree_checks
                        .iter()
                        .any(|check| check.contains("qf_lia") && check.contains("omega")),
                    "countermodel must carry checked QF_LIA evidence: {:?}",
                    model.qfree_checks
                );
                assert!(model
                    .axioms
                    .iter()
                    .all(|axiom| { AXIOM_ALLOWLIST.contains(&axiom.as_str()) }));
            }
            other => panic!("expected checked QF countermodel, found {other:?}"),
        }
    }

    #[test]
    fn production_checks_mixed_lia_and_bv_countermodel_leaves() {
        let parsed = parse(
            "fn epr_mixed_qfree(xs: Vec<u64>, x: u64) -> u64\n\
             req x + x == 2\n\
             ens@bv8 x + x == 4 && \
               forall (i : usize) in xs. xs[i] == xs[i]\n\
             fx pure { 0 }",
        );
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        let Item::Fn(item) = &parsed.program.items[0] else {
            panic!("expected function");
        };
        let recon = thermite_spec::s2_recon_from_obligation(
            &parsed.program,
            item,
            &item.contract.req,
            &item.contract.ens[0],
            thermite_spec::SourceAddress {
                item: item.name.clone(),
                clause: "ens#0".to_string(),
            },
        )
        .expect("canonical mixed-semantics S₂.0 bridge");
        assert_eq!(
            recon
                .qfree_atoms
                .iter()
                .map(|atom| atom.fragment)
                .collect::<Vec<_>>(),
            vec![QFreeFragment::Lia, QFreeFragment::Bv(BvWidth::W8)]
        );
        match reconstruct(&recon, item, &item.contract.req, &item.contract.ens[0]) {
            EprOutcome::Counterexample(model) => {
                assert!(model
                    .qfree_checks
                    .iter()
                    .any(|check| check.contains("qf_lia") && check.contains("omega")));
                assert!(model
                    .qfree_checks
                    .iter()
                    .any(|check| check.contains("qf_bv8")));
            }
            other => panic!("expected checked mixed-QF countermodel, found {other:?}"),
        }
    }

    #[test]
    fn production_retries_a_boolean_qfree_mask_that_lia_cannot_realize() {
        let parsed = parse(
            "fn epr_qfree_retry(xs: Vec<u64>, ys: Vec<u64>, x: u64) -> u64\n\
             req x + x == x + x || xs.len() == xs.len()\n\
             ens xs == ys\n\
             fx pure { 0 }",
        );
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        let Item::Fn(item) = &parsed.program.items[0] else {
            panic!("expected function");
        };
        let recon = thermite_spec::s2_recon_from_obligation(
            &parsed.program,
            item,
            &item.contract.req,
            &item.contract.ens[0],
            thermite_spec::SourceAddress {
                item: item.name.clone(),
                clause: "ens#0".to_string(),
            },
        )
        .expect("canonical S₂.0 bridge");
        assert_eq!(recon.qfree_atoms.len(), 1);
        match reconstruct(&recon, item, &item.contract.req, &item.contract.ens[0]) {
            EprOutcome::Counterexample(model) => {
                assert!(
                    model
                        .model
                        .contains("after rejecting 1 unrealized QFree mask"),
                    "the first Boolean mask should be blocked and retried: {}",
                    model.model
                );
                assert!(model
                    .qfree_checks
                    .iter()
                    .any(|check| check.contains("qf_lia") && check.contains("omega")));
            }
            other => panic!("expected retried checked countermodel, found {other:?}"),
        }
    }

    #[test]
    fn cache_replays_warm_entries_and_rejects_every_tampered_boundary() {
        let parsed = parse(
            "fn epr_cache(xs: Vec<u64>) -> u64\n\
             req true\n\
             ens forall (i : usize) in xs. xs[i] == xs[i]\n\
             fx pure { 0 }",
        );
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        let Item::Fn(item) = &parsed.program.items[0] else {
            panic!("expected function");
        };
        let recon = thermite_spec::s2_recon_from_obligation(
            &parsed.program,
            item,
            &item.contract.req,
            &item.contract.ens[0],
            thermite_spec::SourceAddress {
                item: item.name.clone(),
                clause: "ens#0".to_string(),
            },
        )
        .expect("canonical S₂.0 bridge");
        let EprOutcome::Proved(_) =
            reconstruct(&recon, item, &item.contract.req, &item.contract.ens[0])
        else {
            panic!("fixture must first produce a checked proof");
        };

        let (premise_formula, conclusion_formula) =
            obligation_parts(&recon.formula).expect("obligation polarity");
        let premise = render_frm(premise_formula, &recon, item).expect("render premise");
        let conclusion = render_frm(conclusion_formula, &recon, item).expect("render conclusion");
        let canonical = recon.canonical_wire();
        let source_clause = format!(
            "{}\n{}",
            thermite_spec::canonical_source_expr(&item.contract.req.expr),
            thermite_spec::canonical_source_expr(&item.contract.ens[0].expr)
        );
        let theorem = "thermite_epr_epr_cache_ens_0";
        let input_key = cache_input_key(&canonical, &source_clause, &premise, &conclusion)
            .expect("cache input key");
        let verdict_key_sha256 = fs::read_to_string(epr_cache_dir().join("index").join(&input_key))
            .expect("production reconstruction stores a cache index");
        let verdict_key_sha256 = verdict_key_sha256.trim();
        let entry_text = fs::read_to_string(
            epr_cache_dir()
                .join("entries")
                .join(format!("{verdict_key_sha256}.json")),
        )
        .expect("production reconstruction stores a cache entry");
        let entry: CachedUnsat = serde_json::from_str(&entry_text).expect("valid cached artifact");

        let cache_scratch = Scratch::new("cache-boundary-test").expect("cache scratch");
        let cache = cache_scratch.path.join("cache");
        let replay_scratch = Scratch::new("cache-replay-test").expect("replay scratch");
        let cold = try_cached_unsat_at(
            &cache,
            true,
            &input_key,
            &canonical,
            &source_clause,
            theorem,
            &premise,
            &conclusion,
            &replay_scratch.path,
            Instant::now(),
        )
        .expect("a missing cache is a cold miss");
        assert!(cold.is_none());

        store_cached_unsat_at(&cache, &entry).expect("seed isolated cache");
        let warm = try_cached_unsat_at(
            &cache,
            true,
            &input_key,
            &canonical,
            &source_clause,
            theorem,
            &premise,
            &conclusion,
            &replay_scratch.path,
            Instant::now(),
        )
        .expect("untampered cache must replay")
        .expect("warm cache hit");
        assert_eq!(warm.cache_hit, Some(true));

        let assert_tampered = |tampered: &CachedUnsat, boundary: &str| {
            store_cached_unsat_at(&cache, tampered).expect("write tampered cache");
            let scratch = Scratch::new(boundary).expect("tamper scratch");
            let error = try_cached_unsat_at(
                &cache,
                true,
                &input_key,
                &canonical,
                &source_clause,
                theorem,
                &premise,
                &conclusion,
                &scratch.path,
                Instant::now(),
            )
            .expect_err("strict cache replay must reject tampering");
            assert!(
                error.starts_with("EprCacheTampered:"),
                "{boundary} produced the wrong failure: {error}"
            );
        };

        let mut tampered = entry.clone();
        tampered.canonical.push('x');
        assert_tampered(&tampered, "canonical-ir");

        let mut tampered = entry.clone();
        tampered.ground.ground.push('x');
        assert_tampered(&tampered, "ground-universe");

        let mut tampered = entry.clone();
        tampered.ground.theory.push('x');
        assert_tampered(&tampered, "ground-theory");

        let mut tampered = entry.clone();
        tampered.ground.dimacs.push_str("c tampered\n");
        assert_tampered(&tampered, "cnf");

        let mut tampered = entry.clone();
        tampered.lrat.push_str("1 0 0\n");
        assert_tampered(&tampered, "lrat");

        let mut tampered = entry;
        tampered.final_source.push_str("\n-- tampered\n");
        assert_tampered(&tampered, "theorem-source");
    }
}
