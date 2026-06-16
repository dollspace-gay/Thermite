//! `forge/src/covenant_engine.rs` — the covenant PRODUCER + the `falsify` driver +
//! the covenant-before-burn gate (REQ-4; `.design/stage1-forge-tier.md`, increment 2b).
//!
//! This is the LOGIC the foundation (#20) and the 2a surface (#29) set up for. The
//! foundation threaded a non-optional [`CovenantRecord`] through
//! [`crate::engine::Engine::discharge`] and wired [`crate::verdict::CertVerdict::
//! CovenantRefuted`] as a `Counterexample`-class hard fail in the degrade ladder; 2a
//! parsed the `witness { inhabit (…); falsify N; }` surface into
//! [`thermite_syntax::ast::WitnessBlock`]. Here we CONSUME them:
//!
//! 1. [`analyze_covenant`] binds a witness block to the `fn` it covenants, type-checks
//!    and EXECUTES each `inhabit` witness against the item's `req` (a witness that does
//!    not satisfy `req` is a loud [`CovenantError::WitnessRefutesReq`], never silently
//!    dropped), then drives the `falsify` run.
//! 2. The `falsify` run rides the SplitMix64 generator ([`thermite_tv::Rng`]) — the
//!    same deterministic, clock-free generator the TV streams ride — aimed at the
//!    item's executable semantics ([`crate::covenant_eval`]). An input satisfying `req`
//!    whose executable body violates `ens` is a [`CovenantCounterexample`] (the
//!    [`CertVerdict::CovenantRefuted`](crate::verdict::CertVerdict::CovenantRefuted)
//!    hard-fail material). Q3 default: a fixed-seed `falsify 50_000` when the budget is
//!    unstated.
//! 3. [`covenant_gate`] enforces covenant-before-burn STRUCTURALLY (R-COV-1): the burn
//!    closure (the L3 proof search) is invoked ONLY when the covenant validated; a
//!    refuted or malformed covenant returns WITHOUT invoking burn. This is the
//!    closure-instrumented invariant (the `degrade.rs` style), not a convention.
//!
//! ## Binding: a witness covenants the preceding `fn`
//!
//! The 2a surface parses `witness { … }` as a freestanding anonymous
//! [`thermite_syntax::Item::Forge`] item (numbered `witness#N`). A witness block
//! covenants the `fn` it immediately follows in source order, and its `inhabit` tuple
//! binds positionally to that `fn`'s parameters (the 2a `forge_items` round-trip pairs
//! `fn id(x: u64) … ` with the following `witness { inhabit (1); … }`).
//! [`witness_bindings`] computes that binding over a parsed [`Program`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thermite_syntax::ast::{FnItem, Item, Param, PrimType, Type, WitnessBlock};
use thermite_syntax::Program;
use thermite_tv::Rng;

use crate::covenant::{CovenantRecord, DEFAULT_FALSIFY_BUDGET, DEFAULT_FALSIFY_SEED};
use crate::covenant_eval::{eval_block, eval_expr, CovenantEvalError, Env, IntWidth, Value};
use crate::verdict::CovenantCounterexample;

/// The deterministic covenant evidence block recorded in the certificate (REQ-4 /
/// Q-ORACLE): the author witness count, the `falsify` generated/refuted counts, and
/// the fixed seed. All deterministic (a fixed seed), so it joins the forge-tier cert
/// oracle and cannot drift silently — weakening a budget or dropping a witness changes
/// these numbers (`.design/stage1-forge-tier.md` REQ-4, the covenant evidence block).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CovenantEvidence {
    /// The number of author-stated `inhabit` witnesses (REQ-4: ≥1 required).
    pub witness_count: usize,
    /// The number of `falsify` inputs actually drawn from the generator and run
    /// through the executable body → `ens` check (≤ the budget; a refutation stops
    /// the run early, so this is the budget on a clean covenant and the count-until-hit
    /// on a refuted one — deterministic under the fixed seed).
    pub falsify_generated: u64,
    /// The number of `falsify` inputs that refuted the covenant (`req` held, the body
    /// violated `ens`). The run stops at the first hit, so this is `0` (clean) or `1`
    /// (refuted).
    pub falsify_refuted: u64,
    /// The deterministic SplitMix64 seed of the `falsify` run (Q3 fixed seed).
    pub seed: u64,
}

/// A malformed or absent covenant on a covenant-routed item (REQ-4): refused BEFORE
/// burn, named (R-COV-1, AC-8). Distinct from a [`CovenantCounterexample`] (a
/// `falsify` refutation of a well-formed covenant) — these are author errors in the
/// covenant declaration itself, surfaced loudly rather than silently dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CovenantError {
    /// A `witness` block with no author-stated `inhabit` witness (REQ-4: a covenant
    /// must carry ≥1 author witness; generator-synthesized inputs may augment but
    /// cannot be the only ones). Refused before burn.
    NoAuthorWitness {
        /// The covenanted item's name.
        item: String,
    },
    /// An `inhabit` tuple whose arity does not match the covenanted `fn`'s parameter
    /// count.
    ArityMismatch {
        /// The covenanted item's name.
        item: String,
        /// The rendered offending witness tuple.
        witness: String,
        /// The `fn`'s parameter count.
        expected: usize,
        /// The witness tuple's arity.
        got: usize,
    },
    /// An `inhabit` witness whose value kind does not match the parameter type (a
    /// `bool` for an integer parameter, or vice versa).
    WitnessTypeMismatch {
        /// The covenanted item's name.
        item: String,
        /// The rendered offending witness tuple + the mismatch detail.
        detail: String,
    },
    /// An `inhabit` witness that does NOT satisfy `req` (REQ-4: a covenant error,
    /// surfaced loudly — the author claims an inhabitant of the precondition that is
    /// not one). Refused before burn.
    WitnessRefutesReq {
        /// The covenanted item's name.
        item: String,
        /// The rendered offending witness tuple.
        witness: String,
    },
    /// The covenanted item is outside the covenant-checkable scalar fragment (a
    /// non-scalar parameter type, a boundary fn with no body, a body/contract using a
    /// construct [`crate::covenant_eval`] does not admit). Carries the offending shape.
    UnsupportedItem {
        /// The covenanted item's name.
        item: String,
        /// What put the item outside the fragment.
        detail: String,
    },
}

impl CovenantError {
    /// A stable cause tag for the rejection certificate (parallel to the
    /// `RejectReason` causes the v1 gates use).
    #[must_use]
    pub fn cause(&self) -> &'static str {
        match self {
            CovenantError::NoAuthorWitness { .. } => "CovenantNoAuthorWitness",
            CovenantError::ArityMismatch { .. } => "CovenantArityMismatch",
            CovenantError::WitnessTypeMismatch { .. } => "CovenantWitnessTypeMismatch",
            CovenantError::WitnessRefutesReq { .. } => "CovenantWitnessRefutesReq",
            CovenantError::UnsupportedItem { .. } => "CovenantUnsupportedItem",
        }
    }

    /// The covenanted item's name (every variant names its item — R-COV-1: refusals
    /// are named).
    #[must_use]
    pub fn item(&self) -> &str {
        match self {
            CovenantError::NoAuthorWitness { item }
            | CovenantError::ArityMismatch { item, .. }
            | CovenantError::WitnessTypeMismatch { item, .. }
            | CovenantError::WitnessRefutesReq { item, .. }
            | CovenantError::UnsupportedItem { item, .. } => item,
        }
    }

    /// A human detail for the rejection certificate.
    #[must_use]
    pub fn detail(&self) -> String {
        match self {
            CovenantError::NoAuthorWitness { item } => format!(
                "the covenant on `{item}` declares no author `inhabit` witness; a \
                 forge-routed item must carry at least one author-stated witness \
                 (REQ-4) — refused before burn"
            ),
            CovenantError::ArityMismatch {
                item,
                witness,
                expected,
                got,
            } => format!(
                "the `inhabit {witness}` witness on `{item}` has arity {got}, but `{item}` \
                 takes {expected} parameter(s)"
            ),
            CovenantError::WitnessTypeMismatch { item, detail } => {
                format!("an `inhabit` witness on `{item}` is ill-typed: {detail}")
            }
            CovenantError::WitnessRefutesReq { item, witness } => format!(
                "the `inhabit {witness}` witness on `{item}` does NOT satisfy `req` — a \
                 covenant error (REQ-4): the author claims a precondition inhabitant that \
                 is not one. Surfaced loudly, never dropped — refused before burn"
            ),
            CovenantError::UnsupportedItem { item, detail } => {
                format!("`{item}` is outside the covenant-checkable scalar fragment: {detail}")
            }
        }
    }
}

/// The result of analyzing a covenant-routed item (REQ-4), BEFORE the burn gate. The
/// pure analysis: the `inhabit` witnesses are validated against `req` and the
/// `falsify` run has executed against the body → `ens`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CovenantAnalysis {
    /// The covenant validated: every author witness satisfies `req`, and no `falsify`
    /// input refuted it. Carries the record (threaded into burn) + the evidence.
    Validated {
        /// The covenant record threaded into the burn (`Engine::discharge`).
        record: CovenantRecord,
        /// The deterministic covenant evidence for the certificate.
        evidence: CovenantEvidence,
    },
    /// A `falsify` input satisfied `req` but the body violated `ens` —
    /// [`CertVerdict::CovenantRefuted`](crate::verdict::CertVerdict::CovenantRefuted).
    /// The burn is NOT entered (R-COV-1 / the never-degrades treatment).
    Refuted {
        /// The concrete falsifying input + the seed.
        counterexample: CovenantCounterexample,
        /// The covenant evidence (`falsify_refuted == 1`).
        evidence: CovenantEvidence,
    },
    /// The covenant is malformed/absent — refused before burn, named (R-COV-1).
    Error(CovenantError),
}

/// The outcome of [`covenant_gate`]: the covenant analysis composed with the burn
/// closure (R-COV-1). On a validated covenant the burn ran (its result `T` is carried
/// with the evidence); on a refutation/refusal the burn did NOT run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CovenantGate<T> {
    /// The covenant validated and the burn closure ran. Carries the burn result + the
    /// covenant evidence to attach to its certificate.
    Burned {
        /// The burn closure's result (the L3 discharge certificate, in production).
        result: T,
        /// The covenant evidence for the certificate.
        evidence: CovenantEvidence,
    },
    /// A `falsify` refutation — the burn did NOT run (covenant-before-burn / the
    /// never-degrades treatment, R-COV-1).
    Refuted {
        /// The concrete falsifying input + the seed.
        counterexample: CovenantCounterexample,
        /// The covenant evidence (`falsify_refuted == 1`).
        evidence: CovenantEvidence,
    },
    /// A malformed/absent covenant — the burn did NOT run; the refusal is named.
    Refused {
        /// The named covenant error.
        error: CovenantError,
    },
}

/// Enforce covenant-before-burn STRUCTURALLY (R-COV-1, AC-8): invoke the `burn` closure
/// (the L3 proof search) ONLY when the covenant validated; on a refutation or a
/// malformed/absent covenant return WITHOUT invoking it. This is the closure-instrumented
/// invariant in the [`crate::degrade`] style — the proof-search path cannot start
/// without a valid covenant record, proven by the closure never being called on the
/// non-`Validated` arms (the `covenant_gate_never_burns_without_covenant` test).
///
/// `burn` receives the validated [`CovenantRecord`] — the same record the foundation
/// threads into [`crate::engine::Engine::discharge`], so the L3 path is entered with the
/// covenant in hand, never without it.
pub fn covenant_gate<T, F>(analysis: CovenantAnalysis, burn: F) -> CovenantGate<T>
where
    F: FnOnce(&CovenantRecord) -> T,
{
    match analysis {
        CovenantAnalysis::Validated { record, evidence } => {
            // The ONLY arm that invokes burn — the covenant is in hand (R-COV-1).
            let result = burn(&record);
            CovenantGate::Burned { result, evidence }
        }
        CovenantAnalysis::Refuted {
            counterexample,
            evidence,
        } => CovenantGate::Refuted {
            counterexample,
            evidence,
        },
        CovenantAnalysis::Error(error) => CovenantGate::Refused { error },
    }
}

/// Map each `fn` name to the `witness` block that covenants it (REQ-4): a witness block
/// covenants the `fn` it immediately follows in source order. Returns the binding over
/// the whole [`Program`]; a `fn` with no following witness is absent from the map (a
/// plain v1 item, not covenant-routed). A `witness` block not preceded by a `fn` (e.g.
/// the first item) is unbindable and ignored here (the parser already addressed it;
/// covenant routing simply has no `fn` to attach it to).
#[must_use]
pub fn witness_bindings(program: &Program) -> BTreeMap<String, WitnessBlock> {
    let mut out = BTreeMap::new();
    let mut last_fn: Option<String> = None;
    for item in &program.items {
        match item {
            Item::Fn(f) => last_fn = Some(f.name.clone()),
            Item::Forge(thermite_syntax::ast::ForgeItem::Witness(w)) => {
                if let Some(name) = &last_fn {
                    // A `fn` carries at most one covenant; a second witness block after
                    // the same fn replaces the first (last-wins, deterministic). The
                    // corpus never writes two, so this is a defensive total rule.
                    out.insert(name.clone(), w.clone());
                }
            }
            _ => {}
        }
    }
    out
}

/// A scalar parameter kind for `falsify` input generation (REQ-4): an unsigned integer
/// of a given width, or a `bool`. A parameter outside this set puts the item outside
/// the covenant-checkable fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParamKind {
    Int(IntWidth),
    Bool,
}

impl ParamKind {
    /// The scalar kind of a parameter type, or `None` for a non-scalar type (a slice,
    /// a `Vec`, a user type) — which puts the item outside the covenant fragment.
    fn of_type(ty: &Type) -> Option<ParamKind> {
        match ty {
            Type::Prim(PrimType::Bool) => Some(ParamKind::Bool),
            Type::Prim(p) => IntWidth::of_prim(*p).map(ParamKind::Int),
            _ => None,
        }
    }
}

/// Render a tuple of values as a source-like witness string (`(1, 2)`) for the record
/// and the diagnostic messages. Deterministic.
fn render_values(vals: &[Value]) -> String {
    let parts: Vec<String> = vals
        .iter()
        .map(|v| match v {
            Value::Int(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
        })
        .collect();
    format!("({})", parts.join(", "))
}

/// Bind a parameter list to a value tuple, checking arity + kind (REQ-4). Returns the
/// evaluation [`Env`], or a [`CovenantError`] naming the mismatch.
fn bind_params(
    item: &str,
    params: &[Param],
    kinds: &[ParamKind],
    vals: &[Value],
) -> Result<Env, CovenantError> {
    if vals.len() != params.len() {
        return Err(CovenantError::ArityMismatch {
            item: item.to_string(),
            witness: render_values(vals),
            expected: params.len(),
            got: vals.len(),
        });
    }
    let mut env = Env::new();
    for ((param, kind), val) in params.iter().zip(kinds).zip(vals) {
        match (kind, val) {
            (ParamKind::Int(width), Value::Int(n)) => {
                // Width-check the witness against the parameter's integer type: a value
                // outside `0..=width.max` is not an inhabitant of the parameter type, so
                // it is an ill-typed witness, NOT a `req`-satisfying input (without this,
                // an out-of-range author witness like `inhabit (4294967296)` for a `u32`
                // truncates and manufactures a false CovenantRefuted on a sound item).
                // Generated inputs are always in-range (`gen_value` caps at the max), so
                // this only ever rejects an author witness.
                let max = i128::try_from(width.max_value()).unwrap_or(i128::MAX);
                if *n < 0 || *n > max {
                    return Err(CovenantError::WitnessTypeMismatch {
                        item: item.to_string(),
                        detail: format!(
                            "parameter `{}` is {width:?} (range 0..={max}), witness supplies \
                             out-of-range {n}",
                            param.name
                        ),
                    });
                }
            }
            (ParamKind::Bool, Value::Bool(_)) => {}
            _ => {
                return Err(CovenantError::WitnessTypeMismatch {
                    item: item.to_string(),
                    detail: format!(
                        "parameter `{}` expects {kind:?}, witness supplies {val:?}",
                        param.name
                    ),
                });
            }
        }
        env.insert(param.name.clone(), *val);
    }
    Ok(env)
}

/// Evaluate the `inhabit` argument tuple to concrete values (REQ-4): the witness
/// expressions are evaluated in the EMPTY environment (an `inhabit` tuple is a closed
/// constant tuple). A non-constant or out-of-fragment witness expression is a
/// [`CovenantError::UnsupportedItem`].
fn eval_inhabit(
    item: &str,
    args: &[thermite_syntax::ast::Expr],
) -> Result<Vec<Value>, CovenantError> {
    let empty = Env::new();
    args.iter()
        .map(|e| {
            eval_expr(e, &empty).map_err(|err| CovenantError::UnsupportedItem {
                item: item.to_string(),
                detail: format!("an `inhabit` witness expression is not a closed constant: {err}"),
            })
        })
        .collect()
}

/// Evaluate `req` under a parameter binding (REQ-4): returns `Ok(true)` iff `req`
/// holds, `Ok(false)` iff it does not, or a [`CovenantEvalError`] on an out-of-fragment
/// construct / trap.
fn eval_req(f: &FnItem, env: &Env) -> Result<bool, CovenantEvalError> {
    eval_expr(&f.contract.req.expr, env)?.as_bool()
}

/// Evaluate the body and check every `ens` clause under a parameter binding (REQ-4).
/// Returns `Ok(true)` iff all `ens` clauses hold for the executed `result`, `Ok(false)`
/// iff some clause is violated (a refutation), or a [`CovenantEvalError`].
fn body_satisfies_ens(f: &FnItem, env: &Env) -> Result<bool, CovenantEvalError> {
    let body = f
        .body
        .as_ref()
        .ok_or_else(|| CovenantEvalError::Unsupported("covenant on a body-less fn".to_string()))?;
    let result = eval_block(body, env)?;
    let mut ens_env = env.clone();
    ens_env.insert("result".to_string(), result);
    for clause in &f.contract.ens {
        if !eval_expr(&clause.expr, &ens_env)?.as_bool()? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Generate one scalar input value for a parameter kind from the generator (REQ-4),
/// biased toward boundary values (`0`, `1`, the type max, small ints) that surface
/// off-by-one / boundary `ens` bugs, falling back to a full-range draw. Deterministic
/// in the generator's state.
fn gen_value(rng: &mut Rng, kind: ParamKind) -> Value {
    match kind {
        ParamKind::Bool => Value::Bool(rng.next_u64() & 1 == 0),
        ParamKind::Int(width) => {
            let max = width.max_value();
            // 1-in-4 draws is a boundary value (0 / 1 / max / a small int); the rest
            // are a uniform full-range draw. Boundary bias finds edge `ens` bugs fast.
            let n = if rng.below(4) == 0 {
                let boundaries = [0_u128, 1, max, u128::from(rng.below(16) as u64)];
                boundaries[rng.below(boundaries.len())].min(max)
            } else {
                u128::from(rng.next_u64()) % (max + 1)
            };
            Value::Int(n as i128)
        }
    }
}

/// Analyze a covenant-routed item (REQ-4): bind the `witness` block to `fn` `f`,
/// validate each `inhabit` witness against `req`, then drive the `falsify` run against
/// the executable body → `ens`. The pure analysis BEFORE the burn gate — see
/// [`covenant_gate`].
#[must_use]
pub fn analyze_covenant(f: &FnItem, witness: &WitnessBlock) -> CovenantAnalysis {
    let item = f.name.clone();

    // (0) The parameter kinds — a non-scalar parameter puts the item outside the
    //     covenant-checkable fragment (refused before burn, named).
    let kinds: Vec<ParamKind> = match f
        .params
        .iter()
        .map(|p| {
            ParamKind::of_type(&p.ty).ok_or_else(|| format!("parameter `{}` is non-scalar", p.name))
        })
        .collect::<Result<_, _>>()
    {
        Ok(k) => k,
        Err(detail) => {
            return CovenantAnalysis::Error(CovenantError::UnsupportedItem { item, detail });
        }
    };

    // (1) R-COV-1: a covenant must carry at least one author-stated `inhabit` witness.
    if witness.inhabits.is_empty() {
        return CovenantAnalysis::Error(CovenantError::NoAuthorWitness { item });
    }

    // (2) Type-check + EXECUTE each author witness against `req`. A witness that does
    //     not satisfy `req` is a loud covenant error (never silently dropped).
    let mut author_envs = Vec::with_capacity(witness.inhabits.len());
    let mut witness_strings = Vec::with_capacity(witness.inhabits.len());
    for inhabit in &witness.inhabits {
        let vals = match eval_inhabit(&item, &inhabit.args) {
            Ok(v) => v,
            Err(e) => return CovenantAnalysis::Error(e),
        };
        let env = match bind_params(&item, &f.params, &kinds, &vals) {
            Ok(e) => e,
            Err(e) => return CovenantAnalysis::Error(e),
        };
        let witness_str = render_values(&vals);
        match eval_req(f, &env) {
            Ok(true) => {}
            Ok(false) => {
                return CovenantAnalysis::Error(CovenantError::WitnessRefutesReq {
                    item,
                    witness: witness_str,
                });
            }
            Err(e) => {
                return CovenantAnalysis::Error(CovenantError::UnsupportedItem {
                    item,
                    detail: format!("evaluating `req` on witness {witness_str}: {e}"),
                });
            }
        }
        author_envs.push(env);
        witness_strings.push(witness_str);
    }

    // (3) The falsify budget + seed (Q3: a fixed-seed `falsify 50_000` when unstated).
    let budget = witness
        .falsifies
        .first()
        .map_or(DEFAULT_FALSIFY_BUDGET, |fal| fal.budget);
    let seed = DEFAULT_FALSIFY_SEED;
    let record = CovenantRecord {
        declared: true,
        witnesses: witness_strings,
        falsify_budget: budget,
        falsify_seed: seed,
    };
    let witness_count = record.witnesses.len();

    // (4) The falsify run. The author witnesses (which satisfy `req`) are checked first
    //     — they may augment the generator's hits (REQ-4: witnesses may augment but
    //     cannot be the only ones) — then `budget` generated inputs. A `req`-satisfying
    //     input whose body violates `ens` is the refutation; the run stops at the first
    //     hit. A `Trap` (a partial-operator trap `req` should have guarded) skips the
    //     input; any other eval error means the item is outside the fragment.
    let mut generated: u64 = 0;

    // (4a) The author witnesses, run through body → ens.
    for (env, witness_str) in author_envs.iter().zip(&record.witnesses) {
        generated += 1;
        match body_satisfies_ens(f, env) {
            Ok(true) => {}
            Ok(false) => {
                return CovenantAnalysis::Refuted {
                    counterexample: CovenantCounterexample {
                        input: witness_str.clone(),
                        seed,
                    },
                    evidence: CovenantEvidence {
                        witness_count,
                        falsify_generated: generated,
                        falsify_refuted: 1,
                        seed,
                    },
                };
            }
            Err(CovenantEvalError::Trap(_)) => {}
            Err(e) => {
                return CovenantAnalysis::Error(CovenantError::UnsupportedItem {
                    item,
                    detail: format!("evaluating the body/`ens` on witness {witness_str}: {e}"),
                });
            }
        }
    }

    // (4b) The generated inputs.
    let mut rng = Rng::new(seed);
    for _ in 0..budget {
        let vals: Vec<Value> = kinds.iter().map(|k| gen_value(&mut rng, *k)).collect();
        let env = match bind_params(&item, &f.params, &kinds, &vals) {
            Ok(e) => e,
            // Arity/kind cannot mismatch here (we built `vals` from `kinds`), but map it
            // honestly rather than unwrap.
            Err(e) => return CovenantAnalysis::Error(e),
        };
        // Only `req`-satisfying inputs are candidates (REQ-4).
        match eval_req(f, &env) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(CovenantEvalError::Trap(_)) => continue,
            Err(e) => {
                return CovenantAnalysis::Error(CovenantError::UnsupportedItem {
                    item,
                    detail: format!("evaluating `req` on a generated input: {e}"),
                });
            }
        }
        generated += 1;
        match body_satisfies_ens(f, &env) {
            Ok(true) => {}
            Ok(false) => {
                return CovenantAnalysis::Refuted {
                    counterexample: CovenantCounterexample {
                        input: render_values(&vals),
                        seed,
                    },
                    evidence: CovenantEvidence {
                        witness_count,
                        falsify_generated: generated,
                        falsify_refuted: 1,
                        seed,
                    },
                };
            }
            Err(CovenantEvalError::Trap(_)) => {}
            Err(e) => {
                return CovenantAnalysis::Error(CovenantError::UnsupportedItem {
                    item,
                    detail: format!("evaluating the body/`ens` on a generated input: {e}"),
                });
            }
        }
    }

    CovenantAnalysis::Validated {
        record,
        evidence: CovenantEvidence {
            witness_count,
            falsify_generated: generated,
            falsify_refuted: 0,
            seed,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use thermite_syntax::Item;

    /// Parse a program and return the single covenanted `fn` + its witness block.
    fn parse_covenant(src: &str) -> (FnItem, WitnessBlock) {
        let result = thermite_syntax::parse(src);
        assert!(
            result.errors.is_empty(),
            "fixture must parse cleanly: {:?}",
            result.errors
        );
        let f = result
            .program
            .items
            .iter()
            .find_map(|i| match i {
                Item::Fn(f) => Some(f.clone()),
                _ => None,
            })
            .expect("a fn");
        let w = witness_bindings(&result.program)
            .remove(&f.name)
            .expect("a witness bound to the fn");
        (f, w)
    }

    /// A correct `max`: `ens result >= x && result >= y` with a body that returns the
    /// larger. The covenant validates (no falsify hit).
    const CORRECT_MAX: &str = "fn maxv(x: u64, y: u64) -> u64 \
        req true ens result >= x && result >= y && (result == x || result == y) fx pure \
        { if x > y { x } else { y } } \
        witness { inhabit (3, 7); inhabit (10, 2); falsify 2000; }";

    /// A PLANTED-BUG `max`: the body always returns `x`, so `ens result >= y` is
    /// violated whenever `y > x`. The covenant is refuted with a concrete input.
    const BUGGY_MAX: &str = "fn maxv(x: u64, y: u64) -> u64 \
        req true ens result >= x && result >= y fx pure \
        { x } \
        witness { inhabit (3, 7); falsify 5000; }";

    #[test]
    fn correct_item_validates_with_evidence() {
        let (f, w) = parse_covenant(CORRECT_MAX);
        match analyze_covenant(&f, &w) {
            CovenantAnalysis::Validated { record, evidence } => {
                assert!(record.declared);
                assert_eq!(record.witnesses.len(), 2);
                assert_eq!(record.falsify_budget, 2000);
                assert_eq!(evidence.witness_count, 2);
                assert_eq!(evidence.falsify_refuted, 0);
                // 2 author witnesses + 2000 generated, all req-satisfying (req true).
                assert_eq!(evidence.falsify_generated, 2002);
                assert_eq!(evidence.seed, DEFAULT_FALSIFY_SEED);
            }
            other => panic!("a correct item must validate, got {other:?}"),
        }
    }

    #[test]
    fn planted_bug_is_refuted_with_concrete_counterexample() {
        let (f, w) = parse_covenant(BUGGY_MAX);
        match analyze_covenant(&f, &w) {
            CovenantAnalysis::Refuted {
                counterexample,
                evidence,
            } => {
                assert_eq!(evidence.falsify_refuted, 1);
                assert_eq!(counterexample.seed, DEFAULT_FALSIFY_SEED);
                // The concrete input is a rendered tuple `(x, y)`.
                assert!(
                    counterexample.input.starts_with('(') && counterexample.input.contains(','),
                    "the counterexample carries a concrete input tuple: {}",
                    counterexample.input
                );
            }
            other => panic!("a planted bug must be refuted, got {other:?}"),
        }
    }

    #[test]
    fn refutation_is_deterministic_under_fixed_seed() {
        let (f, w) = parse_covenant(BUGGY_MAX);
        let a = analyze_covenant(&f, &w);
        let b = analyze_covenant(&f, &w);
        assert_eq!(a, b, "the fixed-seed falsify run is deterministic");
    }

    #[test]
    fn unstated_budget_defaults_to_50_000() {
        // No `falsify N;` directive — the Q3 default 50_000 is recorded.
        let src = "fn idv(x: u64) -> u64 req true ens result == x fx pure { x } \
                   witness { inhabit (1); }";
        let (f, w) = parse_covenant(src);
        match analyze_covenant(&f, &w) {
            CovenantAnalysis::Validated { record, evidence } => {
                assert_eq!(record.falsify_budget, DEFAULT_FALSIFY_BUDGET);
                assert_eq!(record.falsify_budget, 50_000);
                // 1 author witness + 50_000 generated (req is `true`, all satisfy it).
                assert_eq!(evidence.falsify_generated, 50_001);
            }
            other => panic!("expected validation, got {other:?}"),
        }
    }

    #[test]
    fn witness_not_satisfying_req_is_a_loud_error() {
        // `inhabit (2)` does not satisfy `req x > 5` — a loud covenant error, never
        // silently dropped (REQ-4).
        let src = "fn pos(x: u64) -> u64 req x > 5 ens result == x fx pure { x } \
                   witness { inhabit (2); falsify 10; }";
        let (f, w) = parse_covenant(src);
        match analyze_covenant(&f, &w) {
            CovenantAnalysis::Error(CovenantError::WitnessRefutesReq { item, witness }) => {
                assert_eq!(item, "pos");
                assert_eq!(witness, "(2)");
            }
            other => panic!("expected a WitnessRefutesReq error, got {other:?}"),
        }
    }

    #[test]
    fn witness_block_without_author_inhabit_is_refused() {
        // A witness block with only `falsify` (no author `inhabit`) is refused, named
        // (R-COV-1: an author witness is mandatory). The parser allows a falsify-only
        // block, so the engine is the enforcer.
        let src = "fn idv(x: u64) -> u64 req true ens result == x fx pure { x } \
                   witness { falsify 10; }";
        let (f, w) = parse_covenant(src);
        assert!(w.inhabits.is_empty());
        match analyze_covenant(&f, &w) {
            CovenantAnalysis::Error(CovenantError::NoAuthorWitness { item }) => {
                assert_eq!(item, "idv");
            }
            other => panic!("expected NoAuthorWitness, got {other:?}"),
        }
    }

    #[test]
    fn non_scalar_param_is_outside_the_fragment() {
        let src = "fn sumv(xs: Vec<u64>) -> u64 req true ens result >= 0 fx pure { 0 } \
                   witness { inhabit (1); falsify 10; }";
        let (f, w) = parse_covenant(src);
        match analyze_covenant(&f, &w) {
            CovenantAnalysis::Error(CovenantError::UnsupportedItem { item, .. }) => {
                assert_eq!(item, "sumv");
            }
            other => panic!("expected UnsupportedItem, got {other:?}"),
        }
    }

    #[test]
    fn out_of_range_author_witness_is_a_type_mismatch_not_a_refutation() {
        // `inhabit (4294967296)` (= 2^32) for a `u32` param is not a `u32` inhabitant —
        // an ill-typed witness (WitnessTypeMismatch), NOT a `req`-satisfying input. Without
        // the width check the truncating model would compute result 0 and manufacture a
        // false CovenantRefuted on a sound item (#300).
        let src = "fn idu(x: u32) -> u32 req true ens result == x fx pure { x } \
                   witness { inhabit (4294967296); falsify 10; }";
        let (f, w) = parse_covenant(src);
        match analyze_covenant(&f, &w) {
            CovenantAnalysis::Error(CovenantError::WitnessTypeMismatch { item, .. }) => {
                assert_eq!(item, "idu");
            }
            other => panic!("expected WitnessTypeMismatch, got {other:?}"),
        }
        // An in-range witness validates (the item is sound for every real u32).
        let ok = "fn idu(x: u32) -> u32 req true ens result == x fx pure { x } \
                  witness { inhabit (5); falsify 10; }";
        let (f2, w2) = parse_covenant(ok);
        assert!(matches!(
            analyze_covenant(&f2, &w2),
            CovenantAnalysis::Validated { .. }
        ));
    }

    // R-COV-1, the closure-instrumented covenant-before-burn invariant (the degrade.rs
    // style): the burn closure is invoked ONLY on a validated covenant. On a refutation
    // and on a refusal the closure must NOT run (a Cell records invocation).
    #[test]
    fn covenant_gate_never_burns_without_covenant() {
        // (a) Validated → burn runs, gets the record.
        let validated = CovenantAnalysis::Validated {
            record: CovenantRecord {
                declared: true,
                witnesses: vec!["(1)".to_string()],
                falsify_budget: 10,
                falsify_seed: DEFAULT_FALSIFY_SEED,
            },
            evidence: CovenantEvidence {
                witness_count: 1,
                falsify_generated: 11,
                falsify_refuted: 0,
                seed: DEFAULT_FALSIFY_SEED,
            },
        };
        let ran = Cell::new(false);
        let gate = covenant_gate(validated, |rec| {
            ran.set(true);
            assert!(rec.declared, "burn receives the validated covenant record");
            "burned"
        });
        assert!(ran.get(), "burn MUST run on a validated covenant");
        assert!(matches!(
            gate,
            CovenantGate::Burned {
                result: "burned",
                ..
            }
        ));

        // (b) Refuted → burn must NOT run.
        let refuted = CovenantAnalysis::Refuted {
            counterexample: CovenantCounterexample {
                input: "(3, 7)".to_string(),
                seed: DEFAULT_FALSIFY_SEED,
            },
            evidence: CovenantEvidence {
                witness_count: 1,
                falsify_generated: 1,
                falsify_refuted: 1,
                seed: DEFAULT_FALSIFY_SEED,
            },
        };
        let ran_r = Cell::new(false);
        let gate_r = covenant_gate(refuted, |_rec| {
            ran_r.set(true);
            "burned"
        });
        assert!(
            !ran_r.get(),
            "R-COV-1: burn must NEVER run on a covenant refutation (covenant-before-burn)"
        );
        assert!(matches!(gate_r, CovenantGate::Refuted { .. }));

        // (c) Error (no author witness) → burn must NOT run; the refusal is named.
        let refused = CovenantAnalysis::Error(CovenantError::NoAuthorWitness {
            item: "f".to_string(),
        });
        let ran_e = Cell::new(false);
        let gate_e = covenant_gate(refused, |_rec| {
            ran_e.set(true);
            "burned"
        });
        assert!(
            !ran_e.get(),
            "R-COV-1: burn must NEVER run on a malformed/absent covenant"
        );
        assert!(matches!(
            gate_e,
            CovenantGate::Refused {
                error: CovenantError::NoAuthorWitness { .. }
            }
        ));
    }
}
