//! Seam test for `thermite_lower::lower_equivalence_obligation`
//! (`.design/forge/equivalent-mutants.md` REQ-1, crosslink #101).
//!
//! Grounds the SEAM the equivalent-mutant exclusion lowers through: an exec body
//! rendered into a Verus EQUIVALENCE OBLIGATION that VERIFIES for the equivalent
//! case and FAILS (counterexample) for the distinguishing case. The decision is
//! the real `verus` verdict (R-DEFER-9 — exclude ONLY on a proof), so this test
//! shells the real binary. It SKIPS LOUDLY when verus is absent (mirroring
//! `lower_conformance.rs`), never panics.
//!
//! Expected verdicts are hand-derived from the design's *Ground the path*
//! (R-CHAR-3): `clamp_zero`'s `req x == 0` makes the early-`return 0` and the
//! `x - 0` flip observably equal to the real `x + 0`, but the `x + 1` off-by-one
//! and the `loose` (`req x <= 100`) early-`return 0` are distinguishing.

use std::path::{Path, PathBuf};
use std::process::Command;

use thermite_syntax::ast::{Block, Expr, Stmt};

fn verus_present() -> bool {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        if Path::new(&p).exists() {
            return true;
        }
    }
    if let Ok(out) = Command::new("which").arg("verus").output() {
        if out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty() {
            return true;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if PathBuf::from(home).join(".local/bin/verus").exists() {
            return true;
        }
    }
    false
}

fn verus_bin() -> String {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        if Path::new(&p).exists() {
            return p;
        }
    }
    "verus".to_string()
}

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

/// Run verus on `source`; return `true` iff it VERIFIES (`0 errors`).
fn verus_verifies(source: &str, label: &str) -> bool {
    let dir = std::env::temp_dir().join(format!(
        "thermite_equiv_{}_{}_{label}",
        std::process::id(),
        unique()
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let stem = format!("{label}_check");
    let path = dir.join(format!("{stem}.rs"));
    std::fs::write(&path, source).expect("write source");
    let out = Command::new(verus_bin())
        .arg(&path)
        .current_dir(&dir)
        .output()
        .expect("spawn verus");
    let _ = std::fs::remove_dir_all(&dir);
    // The grounded summary line is `verification results:: N verified, 0 errors`
    // (emitted to stderr without `--output-json`). A VERIFIED run exits 0 AND
    // reports `, 0 errors`; a counterexample exits non-zero with `, 1 errors`.
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out.status.success() && combined.contains(", 0 errors")
}

/// Parse a single-fn program and return its `FnItem`.
fn parse_fn(src: &str) -> thermite_syntax::ast::FnItem {
    let prog = thermite_syntax::parse(src).program;
    prog.items
        .into_iter()
        .find_map(|i| match i {
            thermite_syntax::ast::Item::Fn(f) => Some(f),
            _ => None,
        })
        .expect("a fn item")
}

/// An early-`return <lit>` mutant body for a scalar fn (the `mutation::generate`
/// early-return family): a leading `return lit;` with the real tail kept after.
fn early_return_body(real: &Block, lit: u128) -> Block {
    let mut body = real.clone();
    body.stmts.insert(
        0,
        Stmt::Return(Some(Expr::IntLit {
            value: lit,
            raw: lit.to_string(),
        })),
    );
    body
}

const CLAMP_ZERO: &str = "fn clamp_zero(x: u64) -> u64\n    req x == 0\n    ens result == 0\n    fx pure\n{\n    let y: u64 = x + 0;\n    y\n}\n";

const LOOSE: &str = "fn loose(x: u64) -> u64\n    req x <= 100\n    ens result <= 1000\n    fx pure\n{\n    let y: u64 = x + 0;\n    y\n}\n";

#[test]
fn equivalent_early_return_verifies() {
    // `clamp_zero`'s early-`return 0` IS observably equal to `x + 0` under
    // `req x == 0` (design: `2 verified, 0 errors`). The obligation VERIFIES.
    if !verus_present() {
        eprintln!("SKIP equivalent_early_return_verifies: verus absent");
        return;
    }
    let f = parse_fn(CLAMP_ZERO);
    let mutant = early_return_body(f.body.as_ref().unwrap(), 0);
    let obligation = thermite_lower::lower_equivalence_obligation(&f, &mutant)
        .expect("scalar obligation lowers");
    assert!(
        verus_verifies(&obligation, "clamp_equiv"),
        "the early-return-0 mutant is PROVED equivalent to `x + 0` under x == 0; \
         obligation must VERIFY (REQ-2).\n--- obligation ---\n{obligation}"
    );
}

#[test]
fn distinguishing_offbyone_fails() {
    // The off-by-one `return 1` (the killed-class witness) is NOT equivalent to
    // `x + 0` under `req x == 0` (design: `0 verified, 1 errors`). The obligation
    // FAILS — so the survivor would STAY counted (the soundness line, REQ-3).
    if !verus_present() {
        eprintln!("SKIP distinguishing_offbyone_fails: verus absent");
        return;
    }
    let f = parse_fn(CLAMP_ZERO);
    let mutant = early_return_body(f.body.as_ref().unwrap(), 1);
    let obligation = thermite_lower::lower_equivalence_obligation(&f, &mutant)
        .expect("scalar obligation lowers");
    assert!(
        !verus_verifies(&obligation, "clamp_distinguish"),
        "the early-return-1 mutant DIFFERS from `x + 0` under x == 0; the \
         equivalence obligation must FAIL — never launder a distinguishing \
         mutant (REQ-3).\n--- obligation ---\n{obligation}"
    );
}

#[test]
fn loose_early_return_stays_distinguishing() {
    // Under the LOOSER `req x <= 100` the SAME early-`return 0` mutant is NOT
    // equivalent (x = 5 distinguishes), so the obligation FAILS — the decision is
    // the verus verdict, NOT a syntactic shape match (AC-3 / AC-2 soundness).
    if !verus_present() {
        eprintln!("SKIP loose_early_return_stays_distinguishing: verus absent");
        return;
    }
    let f = parse_fn(LOOSE);
    let mutant = early_return_body(f.body.as_ref().unwrap(), 0);
    let obligation = thermite_lower::lower_equivalence_obligation(&f, &mutant)
        .expect("scalar obligation lowers");
    assert!(
        !verus_verifies(&obligation, "loose_distinguish"),
        "under req x <= 100 the early-return-0 mutant is distinguishing (x = 5); \
         the obligation must FAIL (the verdict is verus's, not syntactic).\n\
         --- obligation ---\n{obligation}"
    );
}

#[test]
fn non_scalar_return_is_unsupported() {
    // A non-scalar (slice) return is out of the OQ-1 scalar scope: the seam
    // returns `Unsupported` so the caller leaves the survivor COUNTED (the
    // sound-but-incomplete fallback) — never a panic, never a spurious exclusion.
    let src = "fn head(xs: &[u32]) -> &[u32]\n    req true\n    ens true\n    fx pure\n{\n    &xs[..0]\n}\n";
    let f = parse_fn(src);
    let body = f.body.clone().unwrap();
    let res = thermite_lower::lower_equivalence_obligation(&f, &body);
    assert!(
        matches!(res, Err(thermite_lower::LowerError::Unsupported { .. })),
        "a non-scalar return must be Unsupported (OQ-1), got {res:?}"
    );
}
