//! `forge/src/covenant_eval.rs` — the executable-semantics evaluator for the
//! covenant engine (REQ-4; `.design/stage1-forge-tier.md`, increment 2b).
//!
//! The covenant (RFC-1 §5) checks an item against its *own declared meaning* by
//! EXECUTING it: an `inhabit` witness must satisfy `req`, and a `falsify` input that
//! satisfies `req` but whose body violates `ens` is a [`crate::verdict::CertVerdict::
//! CovenantRefuted`] hard fail. Both moves need to run the source on concrete inputs,
//! and the v1 pipeline has no in-Rust interpreter — every other check (contract-TV,
//! exec-TV, the L3 proof) discharges through Verus/Z3. This module is that
//! interpreter: a small, deterministic evaluator over the scalar fragment of the
//! [`thermite_syntax`] AST, the dual of the exec-TV reference encoder
//! ([`thermite_tv::exec_ref_value`]) — where that emits a Verus *value expression
//! string*, this computes the concrete [`Value`] so 50_000 `falsify` inputs run
//! in-process (a Verus run per input would be infeasible).
//!
//! ## The fragment (and why it is bounded)
//!
//! The evaluator admits the pure scalar fragment: integer (`u32`/`u64`/`usize`) and
//! `bool` values; the arithmetic/comparison/logical/bitwise operators; `!`; `if`
//! expressions; `as` casts; and a fn body of `let`/`if`/`return`/tail statements over
//! that fragment. Anything outside it — a sequence/`Seq` value, a combinator call, a
//! method call, a `match`, a struct/enum, a loop — is an
//! [`CovenantEvalError::Unsupported`] carrying the offending shape (it never silently
//! evaluates a wrong value, mirroring [`thermite_tv::exec_encode`]'s
//! `RefEncodeError::Unsupported`, R-CODE-2 / R-APG-1). A covenant declared on an item
//! outside the fragment surfaces that error rather than dropping the witness.
//!
//! ## The integer-value model (and what the covenant does not discriminate)
//!
//! Integers are evaluated as mathematical `i128`, wide enough to hold every `u64`,
//! with `as` casts modelling the truncating bit-width semantics (`x as u32` reduces
//! mod 2³²). Arithmetic, shifts, and bitwise ops (including the type-directed integer
//! `!`) do not truncate to the operand width: the covenant checks the AGREEMENT between
//! the body's computed value and what `ens` asserts, and it evaluates both sides under
//! the same `i128` semantics, so a width-truncation difference (an overflowing `*`, a
//! `<<` past the width, a `!` complement) on one side that the other side also computes
//! identically can never manufacture a spurious refutation. The absolute width-truncated
//! value (the exec-TV / L3 surface, `forge/src/exec_tv.rs`) is not the covenant's
//! discrimination target — only the body-vs-`ens` agreement is. A runtime trap with no `ens` bearing —
//! a divide-by-zero / shift-out-of-range — is a [`CovenantEvalError::Trap`]: the
//! `falsify` driver treats a trapped input as not-evaluated (skipped), not a hit
//! (REQ-4: a hit is an `ens` violation on a `req`-satisfying input, `req` is expected
//! to guard the partial operator).
//!
//! ## Determinism (R-CODE-5)
//!
//! Evaluation is a pure function of the AST + the environment — no clock, no global
//! state — so a covenant's witness/falsify evidence is reproducible (Q-ORACLE: the
//! covenant evidence joins the deterministic forge-tier cert oracle).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use thermite_syntax::ast::{BinOp, Block, Expr, PrimType, Stmt, Type, UnaryOp};

/// An failure to evaluate a construct, never a wrong value (REQ-4, the dual of
/// [`thermite_tv::exec_encode`]'s `RefEncodeError`). The covenant producer surfaces
/// every variant; the `falsify` driver maps a [`CovenantEvalError::Trap`] to a
/// skipped input (a partial-operator trap is not an `ens` violation) and any other
/// variant to a covenant error (the item is outside the covenant-checkable fragment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CovenantEvalError {
    /// A construct the pure scalar fragment does not admit (a sequence value, a
    /// combinator/method call, a `match`, a struct/enum, a loop, …). Carries a short
    /// description of the offending node so a human sees what bit.
    Unsupported(String),
    /// A reference to a name not bound in the environment (an undeclared variable, a
    /// multi-segment path). Carries the name.
    Unbound(String),
    /// A type mismatch the evaluator cannot reconcile (a `bool` where an integer is
    /// required, or vice versa). Carries the offending shape.
    Type(String),
    /// A runtime trap on a partial operator — divide/remainder by zero, a shift past
    /// the operand width. Under a `req`-satisfying input this is a skipped `falsify`
    /// input, not a refutation (REQ-4: `req` is expected to guard the partial op).
    Trap(String),
}

impl fmt::Display for CovenantEvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CovenantEvalError::Unsupported(what) => {
                write!(f, "covenant_eval: unsupported construct: {what}")
            }
            CovenantEvalError::Unbound(name) => {
                write!(f, "covenant_eval: unbound name: {name}")
            }
            CovenantEvalError::Type(what) => write!(f, "covenant_eval: type mismatch: {what}"),
            CovenantEvalError::Trap(what) => write!(f, "covenant_eval: runtime trap: {what}"),
        }
    }
}

impl std::error::Error for CovenantEvalError {}

/// A concrete value in the scalar fragment (REQ-4). Integers are mathematical `i128`
/// (the body and `ens` are evaluated under one model so an overflow never manufactures
/// a refutation — see the module docs); `bool` is the contract/predicate value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    /// An integer value (`u32`/`u64`/`usize` source type), held wide as `i128`.
    Int(i128),
    /// A boolean value (a comparison/connective result, or a `bool` param/literal).
    Bool(bool),
}

impl Value {
    /// The `i128` an integer value holds, or a [`CovenantEvalError::Type`] for a
    /// `bool` (an arithmetic/comparison operand must be an integer).
    fn as_int(self) -> Result<i128, CovenantEvalError> {
        match self {
            Value::Int(n) => Ok(n),
            Value::Bool(b) => Err(CovenantEvalError::Type(format!(
                "expected an integer, found bool {b}"
            ))),
        }
    }

    /// The `bool` a value holds, or a [`CovenantEvalError::Type`] for an integer (a
    /// connective/`if` condition / a `req`/`ens` predicate must be a `bool`). Public so
    /// the covenant engine reads `req`/`ens` predicate values through one accessor.
    pub fn as_bool(self) -> Result<bool, CovenantEvalError> {
        match self {
            Value::Bool(b) => Ok(b),
            Value::Int(n) => Err(CovenantEvalError::Type(format!(
                "expected a bool, found integer {n}"
            ))),
        }
    }
}

/// The unsigned bit-width of a scalar integer type, for `as`-cast truncation
/// (`x as u32` reduces mod 2³²) and for the `falsify` input generator's range bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntWidth {
    /// `u8` — 8-bit, max `u8::MAX`.
    U8,
    /// `u16` — 16-bit, max `u16::MAX`.
    U16,
    /// `u32` — 32-bit, max `u32::MAX`.
    U32,
    /// `u64` — 64-bit, max `u64::MAX`.
    U64,
    /// `usize` — modelled as 64-bit (the corpus target), max `u64::MAX`.
    Usize,
}

impl IntWidth {
    /// The inclusive maximum value of this width (the minimum is always `0` — the
    /// scalar fragment is the unsigned `u32`/`u64`/`usize` types).
    #[must_use]
    pub fn max_value(self) -> u128 {
        match self {
            IntWidth::U8 => u128::from(u8::MAX),
            IntWidth::U16 => u128::from(u16::MAX),
            IntWidth::U32 => u128::from(u32::MAX),
            IntWidth::U64 | IntWidth::Usize => u128::from(u64::MAX),
        }
    }

    /// The modulus `2^bits` for truncating an `as`-cast to this width.
    fn modulus(self) -> i128 {
        match self {
            IntWidth::U8 => 1_i128 << 8,
            IntWidth::U16 => 1_i128 << 16,
            IntWidth::U32 => 1_i128 << 32,
            IntWidth::U64 | IntWidth::Usize => 1_i128 << 64,
        }
    }

    /// The scalar integer width of a primitive type, or `None` for `bool` (the only
    /// non-integer primitive).
    #[must_use]
    pub fn of_prim(p: PrimType) -> Option<IntWidth> {
        match p {
            PrimType::U8 => Some(IntWidth::U8),
            PrimType::U16 => Some(IntWidth::U16),
            PrimType::U32 => Some(IntWidth::U32),
            PrimType::U64 => Some(IntWidth::U64),
            PrimType::Usize => Some(IntWidth::Usize),
            PrimType::Bool => None,
        }
    }
}

/// The evaluation environment: a name → [`Value`] binding map (the params bound to a
/// witness/falsify input, plus `let`-bound locals, plus `result` while evaluating
/// `ens`). A [`BTreeMap`] for deterministic iteration (R-CODE-5).
pub type Env = BTreeMap<String, Value>;

/// Evaluate a scalar-fragment expression to a concrete [`Value`] in `env` (REQ-4).
/// Returns a [`CovenantEvalError`] for any construct outside the fragment, an unbound
/// name, a type mismatch, or a partial-operator trap — never a silent wrong value.
pub fn eval_expr(expr: &Expr, env: &Env) -> Result<Value, CovenantEvalError> {
    match expr {
        Expr::IntLit { value, .. } => Ok(Value::Int(*value as i128)),
        Expr::BoolLit(b) => Ok(Value::Bool(*b)),
        Expr::Path(segments) => match segments.as_slice() {
            [name] => env
                .get(name)
                .copied()
                .ok_or_else(|| CovenantEvalError::Unbound(name.clone())),
            other => Err(CovenantEvalError::Unsupported(format!(
                "multi-segment path {}",
                other.join("::")
            ))),
        },
        Expr::Unary { op, expr } => {
            let v = eval_expr(expr, env)?;
            match op {
                // `!` is type-directed (ast.md REQ-10 / OQ-4, the single `UnaryOp::Not`):
                // logical-not on a `bool`, bitwise-not on an integer. The integer case
                // uses the same `i128` agreement model as the other bitwise/arithmetic
                // ops (the body and `ens` evaluate `!` identically, so a covenant on
                // `ens result == !x` validates; absolute width-truncated bitwise values
                // are the agreement-model caveat documented for arithmetic, not a
                // covenant discrimination target).
                UnaryOp::Not => match v {
                    Value::Bool(b) => Ok(Value::Bool(!b)),
                    Value::Int(n) => Ok(Value::Int(!n)),
                },
            }
        }
        Expr::Binary { op, lhs, rhs } => eval_binary(*op, lhs, rhs, env),
        Expr::If { cond, then, else_ } => {
            if eval_expr(cond, env)?.as_bool()? {
                eval_block(then, env)
            } else {
                eval_block(else_, env)
            }
        }
        Expr::Cast { expr, ty } => {
            let v = eval_expr(expr, env)?.as_int()?;
            match cast_width(ty)? {
                // An `as`-cast to an integer width truncates (reduces mod 2^bits), the
                // bounded-int exec semantics (`exec_encode` models the same).
                Some(w) => Ok(Value::Int(v.rem_euclid(w.modulus()))),
                None => Err(CovenantEvalError::Type(
                    "cast to a non-integer type".to_string(),
                )),
            }
        }
        // Everything else is outside the covenant scalar fragment and returns Unsupported.
        other => Err(CovenantEvalError::Unsupported(expr_shape(other))),
    }
}

/// Evaluate a binary operation (REQ-4): arithmetic (`i128`, no overflow trap — see the
/// module docs), the partial `/`/`%`/`<<`/`>>` (a zero divisor / out-of-range shift is
/// a [`CovenantEvalError::Trap`]), the bitwise ops, the comparisons (→ `bool`), and the
/// short-circuiting `&&`/`||`.
fn eval_binary(op: BinOp, lhs: &Expr, rhs: &Expr, env: &Env) -> Result<Value, CovenantEvalError> {
    // Short-circuit the logical connectives before evaluating the rhs (the exec
    // semantics: `a && b` does not evaluate `b` when `a` is false).
    match op {
        BinOp::And => {
            return Ok(Value::Bool(
                eval_expr(lhs, env)?.as_bool()? && eval_expr(rhs, env)?.as_bool()?,
            ));
        }
        BinOp::Or => {
            return Ok(Value::Bool(
                eval_expr(lhs, env)?.as_bool()? || eval_expr(rhs, env)?.as_bool()?,
            ));
        }
        _ => {}
    }

    let l = eval_expr(lhs, env)?;
    let r = eval_expr(rhs, env)?;

    // The comparison operators are defined on both integers and bools (equality);
    // the ordering operators on integers.
    match op {
        BinOp::Eq => return Ok(Value::Bool(value_eq(l, r)?)),
        BinOp::Ne => return Ok(Value::Bool(!value_eq(l, r)?)),
        _ => {}
    }

    let (a, b) = (l.as_int()?, r.as_int()?);
    let v = match op {
        BinOp::Add => Value::Int(a + b),
        BinOp::Sub => Value::Int(a - b),
        BinOp::Mul => Value::Int(a * b),
        BinOp::Div => {
            if b == 0 {
                return Err(CovenantEvalError::Trap("divide by zero".to_string()));
            }
            // Truncating-toward-zero division (the unsigned operands keep this
            // identical to Rust's `/` over the non-negative scalar fragment).
            Value::Int(a / b)
        }
        BinOp::Rem => {
            if b == 0 {
                return Err(CovenantEvalError::Trap("remainder by zero".to_string()));
            }
            Value::Int(a % b)
        }
        BinOp::Shl | BinOp::Shr => {
            if !(0..128).contains(&b) {
                return Err(CovenantEvalError::Trap(format!(
                    "shift amount {b} out of range"
                )));
            }
            if op == BinOp::Shl {
                Value::Int(a << b)
            } else {
                Value::Int(a >> b)
            }
        }
        BinOp::BitAnd => Value::Int(a & b),
        BinOp::BitOr => Value::Int(a | b),
        BinOp::BitXor => Value::Int(a ^ b),
        BinOp::Lt => Value::Bool(a < b),
        BinOp::Le => Value::Bool(a <= b),
        BinOp::Gt => Value::Bool(a > b),
        BinOp::Ge => Value::Bool(a >= b),
        // Add/Sub/Mul/Div/Rem/Shl/Shr/BitAnd/BitOr/BitXor/Lt/Le/Gt/Ge handled above;
        // Eq/Ne/And/Or returned earlier. This arm is unreachable for the closed BinOp
        // set, but we map it to an error rather than panic (R-APG-1).
        BinOp::Eq | BinOp::Ne | BinOp::And | BinOp::Or => {
            return Err(CovenantEvalError::Unsupported(
                "binary operator dispatch".to_string(),
            ));
        }
    };
    Ok(v)
}

/// Structural equality on two values (`==`/`!=`): integers compare numerically, bools
/// logically; a cross-type comparison is a [`CovenantEvalError::Type`] (the source is
/// type-checked, so this never fires on well-typed input; it returns an error rather
/// than `false`).
fn value_eq(l: Value, r: Value) -> Result<bool, CovenantEvalError> {
    match (l, r) {
        (Value::Int(a), Value::Int(b)) => Ok(a == b),
        (Value::Bool(a), Value::Bool(b)) => Ok(a == b),
        _ => Err(CovenantEvalError::Type(
            "equality between an integer and a bool".to_string(),
        )),
    }
}

/// The integer width of a cast target type, or `None` for a `bool` cast. A non-scalar
/// cast target (a slice, a generic) is an [`CovenantEvalError::Unsupported`].
fn cast_width(ty: &Type) -> Result<Option<IntWidth>, CovenantEvalError> {
    match ty {
        Type::Prim(p) => Ok(IntWidth::of_prim(*p)),
        other => Err(CovenantEvalError::Unsupported(format!(
            "cast to non-scalar type {other:?}"
        ))),
    }
}

/// Evaluate a fn body / `if`-expression block to its value (REQ-4): run the statement
/// stream (`let` binds a local, `if` branches, `return e` short-circuits the block to
/// `e`'s value), then evaluate the tail expression. A block with no tail and no
/// `return` (a unit-valued block) is a [`CovenantEvalError::Type`] — a covenant item
/// returns a scalar value. Loops / `break` / `continue` / mutation are outside the
/// fragment and return Unsupported.
pub fn eval_block(block: &Block, env: &Env) -> Result<Value, CovenantEvalError> {
    let mut local = env.clone();
    // The names a `let mut` introduced — the only names a `Stmt::Assign` may reassign
    // (Rust/Verus reject assignment to an immutable binding, E0384). Params + plain
    // `let` bindings are absent, so reassigning one is an error, not a silent
    // mutation (the covenant-eval faithfulness contract).
    let mut mutable = BTreeSet::new();
    if let Some(v) = eval_stmts(&block.stmts, &mut local, &mut mutable)? {
        // An early `return` inside the statement stream supplies the block's value.
        return Ok(v);
    }
    match &block.tail {
        Some(tail) => eval_expr(tail, &local),
        None => Err(CovenantEvalError::Type(
            "block has no tail expression and no return (unit-valued)".to_string(),
        )),
    }
}

/// Run a statement stream, threading the environment + the set of mutable (`let mut`)
/// binding names (REQ-4). Returns `Ok(Some(v))` if a `return e` (or a returning `if`)
/// short-circuits the block to value `v`, else `Ok(None)` (fall through to the block
/// tail).
fn eval_stmts(
    stmts: &[Stmt],
    env: &mut Env,
    mutable: &mut BTreeSet<String>,
) -> Result<Option<Value>, CovenantEvalError> {
    for stmt in stmts {
        match stmt {
            Stmt::Let {
                mutable: is_mut,
                name,
                init,
                ..
            } => {
                let v = eval_expr(init, env)?;
                env.insert(name.clone(), v);
                // Track mutability so a later `Stmt::Assign` can tell a legal `let mut`
                // reassignment from an illegal immutable reassignment. A `let` (no `mut`)
                // that re-binds an existing name is shadowing, not mutation — drop any
                // stale mutable mark so the new immutable binding is honoured.
                if *is_mut {
                    mutable.insert(name.clone());
                } else {
                    mutable.remove(name);
                }
            }
            Stmt::Assign { target, value } => match target {
                Expr::Path(segments) if segments.len() == 1 => {
                    let name = &segments[0];
                    if !mutable.contains(name) {
                        // Assignment to a param or a non-`mut` `let` binding — invalid
                        // executable code (Verus E0384). Surface it rather than
                        // silently threading the mutation (the faithfulness contract).
                        return Err(CovenantEvalError::Unsupported(format!(
                            "assignment to immutable binding `{name}` (a covenant body must \
                             declare `let mut {name}` to reassign; Verus rejects this as E0384)"
                        )));
                    }
                    let v = eval_expr(value, env)?;
                    env.insert(name.clone(), v);
                }
                other => {
                    return Err(CovenantEvalError::Unsupported(format!(
                        "assignment target {}",
                        expr_shape(other)
                    )));
                }
            },
            Stmt::Return(opt) => match opt {
                Some(e) => return Ok(Some(eval_expr(e, env)?)),
                None => {
                    return Err(CovenantEvalError::Type(
                        "bare `return;` (unit) in a scalar covenant body".to_string(),
                    ));
                }
            },
            Stmt::If { cond, then, else_ } => {
                // A statement-position `if/else` (the parser only emits `Stmt::If` here;
                // a tail-position `if/else` is an `Expr::If` handled by `eval_block`'s
                // tail). Its value is discarded — Rust/Verus semantics — so a branch's
                // tail expression does not short-circuit the enclosing block; only a
                // `return` statement inside the taken branch does (`eval_stmts` returns
                // `Some`). Evaluating the branch tail as a block return was a bug that
                // manufactured a false refutation on e.g. `{ if c { x } else { x } 0 }`
                // (which returns `0`, not `x`).
                let branch_returned = if eval_expr(cond, env)?.as_bool()? {
                    eval_stmts(&then.stmts, env, mutable)?
                } else if let Some(else_block) = else_ {
                    eval_stmts(&else_block.stmts, env, mutable)?
                } else {
                    None
                };
                // Only an actual `return` inside the taken branch short-circuits.
                if let Some(v) = branch_returned {
                    return Ok(Some(v));
                }
            }
            Stmt::Expr(_) => {
                // A bare expression statement in the scalar fragment is pure and
                // value-discarded (no side effects to model). Skip it.
            }
            Stmt::Loop(_) | Stmt::Break | Stmt::Continue => {
                return Err(CovenantEvalError::Unsupported(
                    "loop / break / continue is outside the covenant scalar fragment".to_string(),
                ));
            }
        }
    }
    Ok(None)
}

/// A short human description of an expression node kind, for an [`CovenantEvalError`]
/// message (so the unsupported-construct error names what bit, not the whole tree).
fn expr_shape(expr: &Expr) -> String {
    match expr {
        Expr::IntLit { .. } => "integer literal".to_string(),
        Expr::BoolLit(_) => "bool literal".to_string(),
        Expr::Path(p) => format!("path {}", p.join("::")),
        Expr::Call { .. } => "call expression".to_string(),
        Expr::MethodCall { name, .. } => format!("method call .{name}(…)"),
        Expr::Field { name, .. } => format!("field access .{name}"),
        Expr::Closure { .. } => "closure".to_string(),
        Expr::Match { .. } => "match expression".to_string(),
        Expr::If { .. } => "if expression".to_string(),
        Expr::Binary { .. } => "binary expression".to_string(),
        Expr::Unary { .. } => "unary expression".to_string(),
        Expr::Index { .. } => "index expression".to_string(),
        Expr::Cast { .. } => "cast expression".to_string(),
        Expr::Ref { .. } => "reference expression".to_string(),
        Expr::StructLit { .. } => "struct literal".to_string(),
        Expr::Is { .. } => "is-variant test".to_string(),
        Expr::Deref(_) => "deref expression".to_string(),
        Expr::StrLit(_) => "string literal".to_string(),
        Expr::Tuple(_) => "tuple expression".to_string(),
        Expr::TupleProj { index, .. } => format!("tuple projection .{index}"),
        // A raw quantifier (`.design/stage2-stratified-cage.md` REQ-0): a shape label;
        // covenant evaluation of stratified binders is stage-2 (REQ-8).
        Expr::Quantifier { quant, .. } => match quant {
            thermite_syntax::Quant::Forall => "forall quantifier".to_string(),
            thermite_syntax::Quant::Exists => "exists quantifier".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use thermite_syntax::ast::FnItem;
    use thermite_syntax::Item;

    /// Parse a single `fn` item from source (the test fixtures are all single-fn).
    fn parse_fn(src: &str) -> FnItem {
        let result = thermite_syntax::parse(src);
        assert!(
            result.errors.is_empty(),
            "fixture must parse cleanly: {:?}",
            result.errors
        );
        result
            .program
            .items
            .into_iter()
            .find_map(|i| match i {
                Item::Fn(f) => Some(f),
                _ => None,
            })
            .expect("fixture has a fn")
    }

    fn env(pairs: &[(&str, Value)]) -> Env {
        pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
    }

    /// Evaluate the contract expression of source `req true ens <EXPR>` clause k.
    fn eval_src(expr_src: &str, e: &Env) -> Result<Value, CovenantEvalError> {
        let f = parse_fn(&format!(
            "fn probe(x: u64, y: u64) -> u64 req true ens {expr_src} fx pure {{ x }}"
        ));
        eval_expr(&f.contract.ens[0].expr, e)
    }

    #[test]
    fn arithmetic_and_comparison() {
        let e = env(&[("x", Value::Int(7)), ("y", Value::Int(3))]);
        assert_eq!(eval_src("x + y == 10", &e), Ok(Value::Bool(true)));
        assert_eq!(eval_src("x - y == 4", &e), Ok(Value::Bool(true)));
        assert_eq!(eval_src("x * y == 21", &e), Ok(Value::Bool(true)));
        assert_eq!(eval_src("x > y", &e), Ok(Value::Bool(true)));
        assert_eq!(eval_src("x < y", &e), Ok(Value::Bool(false)));
        assert_eq!(eval_src("x >= 7 && y <= 3", &e), Ok(Value::Bool(true)));
    }

    #[test]
    fn logical_short_circuit_does_not_touch_unbound_rhs() {
        // `false && <unbound>` must short-circuit to false without evaluating the rhs
        // (so an unbound name on the dead side is not an error — the exec semantics).
        let e = env(&[("x", Value::Int(0))]);
        assert_eq!(eval_src("x > 5 && z == 1", &e), Ok(Value::Bool(false)));
        // `true || <unbound>` short-circuits to true likewise.
        assert_eq!(eval_src("x >= 0 || z == 1", &e), Ok(Value::Bool(true)));
    }

    #[test]
    fn divide_by_zero_is_a_trap_not_a_wrong_value() {
        let e = env(&[("x", Value::Int(7)), ("y", Value::Int(0))]);
        assert_eq!(
            eval_src("x / y == 0", &e),
            Err(CovenantEvalError::Trap("divide by zero".to_string()))
        );
    }

    #[test]
    fn cast_truncates_to_width() {
        // (u64::MAX) as u32 == u32::MAX  (truncation mod 2^32).
        let e = env(&[("x", Value::Int(i128::from(u64::MAX)))]);
        assert_eq!(
            eval_src("x as u32 == 4294967295", &e),
            Ok(Value::Bool(true))
        );
    }

    #[test]
    fn unsupported_construct_is_loud_not_silent() {
        // A combinator call is outside the scalar fragment — an Unsupported.
        let e = env(&[("x", Value::Int(1))]);
        let r = eval_src("forall_in(x, |i| true)", &e);
        assert!(
            matches!(r, Err(CovenantEvalError::Unsupported(_))),
            "a call expression must be a loud Unsupported, not a silent value: {r:?}"
        );
    }

    #[test]
    fn unbound_name_is_an_error() {
        let e = env(&[("x", Value::Int(1))]);
        assert_eq!(
            eval_src("w == 1", &e),
            Err(CovenantEvalError::Unbound("w".to_string()))
        );
    }

    #[test]
    fn body_with_let_and_tail() {
        // A body of `let`-bindings + a tail expression evaluates to the tail value.
        let f = parse_fn(
            "fn add(x: u64, y: u64) -> u64 req true ens result == x + y fx pure \
             { let s = x + y; s }",
        );
        let e = env(&[("x", Value::Int(4)), ("y", Value::Int(5))]);
        assert_eq!(
            eval_block(f.body.as_ref().expect("body"), &e),
            Ok(Value::Int(9))
        );
    }

    #[test]
    fn body_with_early_return_in_if() {
        // An `if` whose then-branch `return`s short-circuits the body.
        let f = parse_fn(
            "fn clamp(x: u64) -> u64 req true ens result <= 10 fx pure \
             { if x > 10 { return 10; } x }",
        );
        let big = env(&[("x", Value::Int(99))]);
        assert_eq!(
            eval_block(f.body.as_ref().expect("body"), &big),
            Ok(Value::Int(10))
        );
        let small = env(&[("x", Value::Int(3))]);
        assert_eq!(
            eval_block(f.body.as_ref().expect("body"), &small),
            Ok(Value::Int(3))
        );
    }

    #[test]
    fn stmt_position_if_value_is_discarded_not_an_early_return() {
        // A statement-position `if/else` value is DISCARDED (Rust/Verus semantics): the
        // body returns the trailing tail `0`, not the taken branch's tail `x`. A bug here
        // (taking the branch tail as a block return) manufactures a false refutation on a
        // correct item — the divergence the critic pinned (#298).
        let f = parse_fn(
            "fn alwayszero(x: u64) -> u64 req true ens result == 0 fx pure \
             { if x > 0 { x } else { x } 0 }",
        );
        for x in [0_i128, 1, 99, i128::from(u64::MAX)] {
            assert_eq!(
                eval_block(
                    f.body.as_ref().expect("body"),
                    &env(&[("x", Value::Int(x))])
                ),
                Ok(Value::Int(0)),
                "the statement-position if value is discarded; the body returns 0 for x={x}"
            );
        }
    }

    #[test]
    fn assignment_to_immutable_binding_is_a_loud_error_let_mut_works() {
        // Assignment to a non-`mut` `let` binding is invalid executable code (Verus
        // E0384) — an error, never a silent mutation (#299, the faithfulness
        // contract). The only textual difference from the legal control is `mut`.
        let immutable = parse_fn(
            "fn setone(x: u64) -> u64 req true ens result == 1 fx pure \
             { let r = 0; if x > 0 { r = 1; } else { r = 1; } r }",
        );
        let r = eval_block(
            immutable.body.as_ref().expect("body"),
            &env(&[("x", Value::Int(0))]),
        );
        assert!(
            matches!(r, Err(CovenantEvalError::Unsupported(ref m)) if m.contains("immutable")),
            "assigning to a non-mut binding must be a loud Unsupported, not a silent value: {r:?}"
        );

        // The `let mut` control evaluates the mutation legally.
        let mutable = parse_fn(
            "fn setone(x: u64) -> u64 req true ens result == 1 fx pure \
             { let mut r = 0; if x > 0 { r = 1; } else { r = 1; } r }",
        );
        assert_eq!(
            eval_block(
                mutable.body.as_ref().expect("body"),
                &env(&[("x", Value::Int(0))])
            ),
            Ok(Value::Int(1)),
            "a legal `let mut` reassignment threads the mutation"
        );
    }

    #[test]
    fn integer_bitwise_not_uses_the_i128_agreement_model() {
        // `!` is type-directed (ast.md REQ-10): bitwise-not on an integer, logical-not on
        // a bool. The integer case uses the i128 agreement model the other bitwise ops
        // use — a body `!x` and `ens result == !x` evaluate `!` identically, so a covenant
        // on a `!`-using item validates rather than being wrongly refused (#302).
        let f = parse_fn("fn flip(x: u64) -> u64 req true ens result == !x fx pure { !x }");
        let e = env(&[("x", Value::Int(5))]);
        let result = eval_block(f.body.as_ref().expect("body"), &e).expect("body evals");
        assert_eq!(result, Value::Int(!5_i128));
        let mut ens_env = e.clone();
        ens_env.insert("result".to_string(), result);
        assert_eq!(
            eval_expr(&f.contract.ens[0].expr, &ens_env),
            Ok(Value::Bool(true)),
            "body `!x` agrees with `ens result == !x`"
        );
        // Logical `!` on a bool is unaffected.
        assert_eq!(
            eval_expr(
                &parse_fn("fn p(b: bool) -> bool req true ens result == !b fx pure { b }")
                    .contract
                    .ens[0]
                    .expr,
                &env(&[("b", Value::Bool(true)), ("result", Value::Bool(false))]),
            ),
            Ok(Value::Bool(true))
        );
    }

    #[test]
    fn evaluation_is_deterministic() {
        let e = env(&[("x", Value::Int(7)), ("y", Value::Int(3))]);
        let once = eval_src("x + y == 10", &e);
        let twice = eval_src("x + y == 10", &e);
        assert_eq!(once, twice);
    }
}
