//! Production reconstruction for admitted S₂.0 relation/sequence clauses.
//!
//! Rust renders the canonical classifier IR into Lean, asks Lean to recompute
//! structural Skolemization, the finite ground universe, theory clauses, and
//! Tseitin CNF, then uses the pinned CaDiCaL + drat-trim pair only to find an
//! answer and (for UNSAT) an LRAT certificate. A clause is certified only after
//! a second Lean run parses and kernel-checks that LRAT against the recomputed
//! problem and proves the actual `req → clause` formula.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thermite_spec::classifier::{self, Atom, Frm, Mach, Rel, ScalarValue, Sort2, Tm, Verdict};
use thermite_spec::S2Recon;
use thermite_syntax::{Clause, FnItem};

use crate::lean_smt_export::ReconstructionEvidence;

const SOLVER_SECONDS: u64 = 30;
const EPR_FRAGMENT: &str = "s2_recon_v1";
const CADICAL_VERSION: &str = "2.1.3";
const CADICAL_REVISION: &str = "f13d74439a5b5c963ac5b02d05ce93a8098018b8";
const DRAT_TRIM_REVISION: &str = "effa1dcce85c878236f8313133dff1a2b766cd7c";
const EPR_CHECKER: &str =
    "Lean kernel + structural EPR + CaDiCaL 2.1.3 + drat-trim effa1dc + LRAT replay";
const EPR_CACHE_SCHEMA: &str = "thermite.epr.artifacts.v1";
const AXIOM_ALLOWLIST: &[&str] = &["propext", "Classical.choice", "Quot.sound"];
const COUNTERMODEL_SEEDS: usize = 1 << 16;
static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

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
             axioms=[{}]",
            self.model,
            self.universe_count,
            self.universe_sha256,
            self.atoms.len(),
            assignments.join(", "),
            self.cnf_sha256,
            self.axioms.join(", ")
        )
    }
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
    if let Err(error) = fs::write(&cnf_path, ground.dimacs.as_bytes()) {
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
            if let Err(reason) = validate_dimacs_assignment(&ground.dimacs, &assignment) {
                return EprOutcome::Failed(format!("EprCountermodelInvalid: {reason}"));
            }
            if !recon.qfree_atoms.is_empty() {
                return EprOutcome::Failed(
                    "EprCountermodelQFreeRealization: SAT was checked, but a genuine source \
                     countermodel for embedded QF_LIA/QF_BV atoms must be decoded through \
                     their existing checked model path"
                        .to_string(),
                );
            }
            let (seed, atoms, axioms) =
                match check_bool_countermodel(&scratch.path, &premise, &conclusion, &ground) {
                    Ok(model) => model,
                    Err(reason) => {
                        return EprOutcome::Failed(format!(
                            "EprCountermodelRealization: the propositional problem is SAT, \
                         but no checked typed model was produced: {reason}"
                        ))
                    }
                };
            EprOutcome::Counterexample(FiniteCountermodel {
                model: format!(
                    "two-element typed model seed {seed} (Lean searched constants, \
                     unary functions, order relations, and injective sequence views)"
                ),
                universe_count: ground.ground_count,
                universe_sha256: sha256_hex(ground.ground.as_bytes()),
                atoms,
                cnf_sha256: sha256_hex(ground.dimacs.as_bytes()),
                axioms,
            })
        }
        Some(20) => {
            let lrat_path = scratch.path.join("problem.lrat");
            let trim = match run_drat_trim(&toolchain.drat_trim, &cnf_path, &proof_path, &lrat_path)
            {
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
                        canonical: canonical.clone(),
                        source_clause: source_clause.clone(),
                        theorem: theorem.clone(),
                        final_source: final_source.clone(),
                        lrat: lrat.clone(),
                        ground: ground.clone(),
                    };
                    let _ = store_cached_unsat(&entry);
                }
            }
            EprOutcome::Proved(evidence)
        }
        _ => {
            let detail = output_head(&solver);
            if detail.contains("time limit") || detail.contains("UNKNOWN") {
                EprOutcome::Timeout(format!(
                    "EprSolverTimeout: CaDiCaL did not decide the finite problem within \
                     {SOLVER_SECONDS}s: {detail}"
                ))
            } else {
                EprOutcome::Failed(format!(
                    "EprSolverFailure: CaDiCaL exited {:?}: {detail}",
                    solver.status.code()
                ))
            }
        }
    }
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

fn countermodel_search_source(premise: &str, conclusion: &str) -> String {
    format!(
        r#"import Thermite.Strat.TestModel

{}

def main : IO Unit := do
  let found := (List.range {COUNTERMODEL_SEEDS}).find? fun seed =>
    evalFrm (searchedBoolModel seed) premise
        (emptySearchedBoolValuation seed) &&
      !evalFrm (searchedBoolModel seed) conclusion
        (emptySearchedBoolValuation seed)
  match found with
  | some seed => IO.println s!"THERMITE-COUNTERMODEL-SEED={{seed}}"
  | none => IO.println "THERMITE-COUNTERMODEL-SEED=none"
"#,
        common_source(premise, conclusion)
    )
}

fn countermodel_replay_source(premise: &str, conclusion: &str, seed: usize) -> String {
    format!(
        r#"import Thermite.Strat.TestModel

{}

private def counterSeed : Nat := {seed}
private def counterModel : Model := searchedBoolModel counterSeed
private def counterValuation : Valuation counterModel :=
  emptySearchedBoolValuation counterSeed
private def counterInterpretation : GroundInterpretation counterModel where
  qfree := fun _ => false
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
        common_source(premise, conclusion)
    )
}

fn check_bool_countermodel(
    scratch: &Path,
    premise: &str,
    conclusion: &str,
    ground: &GroundMetadata,
) -> Result<(usize, Vec<GroundAtomValue>, Vec<String>), String> {
    let search_source = countermodel_search_source(premise, conclusion);
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

    let replay_source = countermodel_replay_source(premise, conclusion, seed);
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

fn run_lean(source: &Path, run_main: bool) -> Result<String, String> {
    let lake = lake_binary();
    let mut command = Command::new(&lake);
    command.arg("env").arg("lean").arg("--tstack=65536");
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
