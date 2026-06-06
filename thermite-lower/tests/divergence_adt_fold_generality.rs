//! acto-critic divergence test: `is_adt_fold_sum` (`thermite-lower/src/lower.rs`)
//! is oracle-fitted to the EXACT `sum_list` shape, not the general structural
//! recursion `.design/basis/01-adts.md` REQ-10 grounds (commit `322d479`, #67).
//!
//! Authority: `.design/basis/01-adts.md` "RECORDED FINDING (the structural-
//! recursion stack is end-to-end feasible)" — "A `Tree` (`Node(Box<Tree>,
//! Box<Tree>)`) with a `tree_sum` fold was also confirmed to verify." + REQ-10
//! ("a `spec fn` over it carries `decreases <value>` … recurses through the
//! `Box` with `*`") + the GROUNDED `tree_sum` of the design's Verus seed. The
//! design REQUIRES a `Box`-recursive `Tree` fold to lower + verify, not just the
//! single corpus `sum_list` shape. `goal.md` R-DEFER-9 (no corpus-specific
//! gaming — the lowering must be GENERAL, not a "lower this exact program"
//! special case).
//!
//! The defect: `is_adt_fold_sum` requires the base arm to be a UNIT variant
//! whose body is the LITERAL `0` (`Nil => 0`). The grounded `Tree` fold's base
//! is a value-carrying tuple variant (`Leaf(v) => v as u64`), so the shape
//! predicate returns `false`, the spec fn is NOT coerced to `-> nat`, and the
//! recursive arm `tree_sum(*l) + tree_sum(*r)` lowers to an `int`-typed body
//! that conflicts with the `u64` return — verus rejects with `match arms have
//! incompatible types`. The lowering thus only works for the literal-`0`-base
//! corpus shape, not the general fold.
//!
//! Verus-backed; SKIPs LOUDLY if verus is absent.

use std::path::{Path, PathBuf};
use std::process::Command;

fn verus_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    if let Ok(out) = Command::new("which").arg("verus").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let cand = PathBuf::from(home).join(".local/bin/verus");
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

fn run_verus(file: &Path) -> Option<(bool, String)> {
    let bin = verus_bin()?;
    let out = Command::new(bin)
        .arg(file)
        .current_dir(std::env::temp_dir())
        .output()
        .ok()?;
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), combined))
}

/// DIVERGENCE — the GROUNDED `Tree` fold (`.design/basis/01-adts.md` recorded
/// finding) does NOT lower to verifiable Verus. The lowering must be GENERAL
/// (any `Box`-recursive structural fold with a `dec` measure), not fitted to the
/// literal-`0`-unit-base `sum_list` corpus shape (R-DEFER-9).
#[test]
fn divergence_tree_fold_does_not_lower_and_verify() {
    // The design's GROUNDED Tree fold: a value-carrying base case `Leaf(v) => v`
    // and a binary-recursive `Node(l, r) => tree_sum(*l) + tree_sum(*r)`.
    // (`.design/basis/01-adts.md` REQ-10 + the recorded structural-recursion
    // finding — "a `tree_sum` fold was also confirmed to verify".)
    let src = "enum Tree {\n  Leaf(u64),\n  Node(Box<Tree>, Box<Tree>),\n}\n\n\
               spec fn tree_sum(t: Tree) -> u64\n  dec t\n{\n  match t {\n    \
               Leaf(v)    => v as u64,\n    Node(l, r) => tree_sum(*l) + tree_sum(*r),\n  }\n}\n";

    let parsed = thermite_syntax::parse(src);
    assert!(
        parsed.errors.is_empty(),
        "the Tree fold must parse clean (Stage 1a/1b shipped): {:?}",
        parsed.errors
    );

    let emitted = thermite_lower::lower(&parsed.program)
        .unwrap_or_else(|e| panic!("lowering the grounded Tree fold failed: {e:?}"));

    if verus_bin().is_none() {
        eprintln!("SKIP: verus absent — Tree-fold generality not verified end-to-end.");
        return;
    }

    let tmp = std::env::temp_dir().join("tree_fold_generality.rs");
    std::fs::write(&tmp, &emitted).expect("write temp");
    let (ok, output) = run_verus(&tmp).expect("verus present (checked above)");

    // AUTHORITY: the design's recorded finding requires this fold to VERIFY
    // (`N verified, 0 errors`). A GENERAL structural-recursion lowering produces
    // verifiable Verus for it. The current `is_adt_fold_sum` shape-fit does not.
    assert!(
        ok && output.contains("0 errors") && output.contains("verified, 0 errors"),
        "the GROUNDED `Tree` fold (.design/basis/01-adts.md REQ-10 + recorded \
         structural-recursion finding) must lower to verifiable Verus, proving \
         the ADT-fold lowering is GENERAL (R-DEFER-9, not fitted to the \
         literal-`0`-base `sum_list` corpus shape). exit_success={ok}\n\
         --- verus output ---\n{output}\n--- emitted ---\n{emitted}"
    );
}
