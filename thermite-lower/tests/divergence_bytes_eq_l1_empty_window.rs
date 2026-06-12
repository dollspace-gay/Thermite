//! Critic divergence pin (#276-arc audit of `af1e6f2c`): the `bytes_eq` L1 exec
//! twin DIVERGES from the spec body on the EMPTY window at an out-of-bounds
//! offset (`n == 0`, `ai > a.len()`).
//!
//! Authority — `.design/basis/07-strings.md` REQ-20 (the L1 exec twin): the twin
//! is "a bounds-checked byte-compare loop over the runtime `TString`s computing
//! the **SAME value as the spec body**"; the sanctioned exception is that "an
//! out-of-bounds runtime **index** is a check failure, not UB". The spec body
//! (REQ-18, emitted verbatim by `emit_bytes_eq_defs`) is
//!
//! ```text
//! if n <= 0 { true } else { a[ai] == b[bi] && bytes_eq(a, b, ai+1, bi+1, n-1) }
//! ```
//!
//! — for `n == 0` it is `true` UNCONDITIONALLY, and NO index is accessed, so the
//! out-of-bounds-index exception cannot apply: there is no index. The shipped
//! twin (`l1::emit_string_runtime_l1`) instead guards the whole window FIRST
//! (`if ai_u + n_u > a.data.len() ... return false`), so for `(n = 0, ai > len)`
//! the twin returns `false` where the spec is `true`.
//!
//! Consequence (demonstrated live during the audit): a program whose `ens`
//! carries `bytes_eq(result, result, 5, 5, 0)` CERTIFIES L3 under `forge check`
//! (verus proves the `n <= 0` arm — the spec value IS `true`), then the
//! `forge build` binary PANICS at runtime on that very same certified `ens`
//! ("thermite L1 contract violation [ens]"). A verus-PROVEN postcondition
//! failing its own always-active runtime check is the exact check/build
//! value-divergence REQ-20's "SAME value" clause exists to forbid (distinct
//! from #280, which is an honest COMPILE failure on the `&`-field spelling).
//!
//! Expected value derivation (R-CHAR-3): `bytes_eq(_, _, 5, 5, 0)` = `true` is
//! hand-derived from the REQ-18 definition's `n <= 0` arm — never copied from
//! toolchain output.
//!
//! This test FAILS against the current toolchain (the run aborts); it passes
//! once the twin mirrors the spec's `n <= 0 -> true` arm before (or instead of
//! failing on) the window guard for the no-index case.

use std::process::Command;

/// Compile `src` with `rustc` (debug), then RUN. Returns
/// `(compiled_ok, ran_ok, combined_output)` — the `divergence_l1.rs` harness
/// shape (`--crate-name` because the `.` in `*.l1.rs` breaks derivation).
fn compile_and_run(src: &str, crate_name: &str) -> (bool, bool, String) {
    let dir = std::env::temp_dir();
    let rs = dir.join(format!("{crate_name}.l1.rs"));
    let bin = dir.join(crate_name);
    std::fs::write(&rs, src).unwrap_or_else(|e| panic!("write temp {crate_name}: {e}"));
    let comp = Command::new("rustc")
        .arg("--crate-name")
        .arg(crate_name)
        .arg("--edition")
        .arg("2021")
        .arg(&rs)
        .arg("-o")
        .arg(&bin)
        .current_dir(&dir)
        .output()
        .unwrap_or_else(|e| panic!("rustc failed for {crate_name}: {e}"));
    let mut combined = String::from_utf8_lossy(&comp.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&comp.stderr));
    if !comp.status.success() {
        return (false, false, combined);
    }
    let run = Command::new(&bin)
        .current_dir(&dir)
        .output()
        .unwrap_or_else(|e| panic!("running {crate_name} failed: {e}"));
    combined.push_str(&String::from_utf8_lossy(&run.stdout));
    combined.push_str(&String::from_utf8_lossy(&run.stderr));
    (true, run.status.success(), combined)
}

fn lower_l1_str(src: &str) -> String {
    let parsed = thermite_syntax::parse(src);
    assert!(
        parsed.errors.is_empty(),
        "probe must parse clean: {:?}",
        parsed.errors
    );
    thermite_lower::lower_l1(&parsed.program).unwrap_or_else(|e| panic!("lower_l1 failed: {e}"))
}

// ---------------------------------------------------------------------------
// Divergence: the empty window (`n = 0`) at an offset past the end. Spec value
// (REQ-18 `n <= 0` arm): TRUE — verus certifies the `ens` L3 (verified live:
// `forge check` exits 0, item L3). The REQ-20 twin must compute the SAME value,
// so the L1 binary must run CLEAN. The shipped twin's window-first guard
// returns `false` -> the certified `ens` aborts at runtime -> this test FAILS.
// ---------------------------------------------------------------------------
#[test]
fn bytes_eq_l1_twin_empty_window_matches_certified_spec_value() {
    let src = r#"
fn empty_window() -> String
  req true
  ens result.len() == 2
  ens bytes_eq(result, result, 5, 5, 0)
  fx alloc
{
  "ab"
}
"#;
    let emitted = lower_l1_str(src);
    // The certified contract must be present as an always-active L1 check.
    assert!(
        emitted.contains("bytes_eq(result, result, 5, 5, 0)"),
        "the bytes_eq ens must lower to an always-active L1 check:\n{emitted}"
    );
    let program = format!(
        "{emitted}\nfn main() {{\n    let _ = empty_window();\n    println!(\"l1-clean\");\n}}\n"
    );
    let (compiled, ran, out) = compile_and_run(&program, "bytes_eq_empty_window_l1");
    assert!(compiled, "the L1 program must COMPILE:\n{out}");
    // THE PINNED EXPECTATION (REQ-20): bytes_eq(_, _, 5, 5, 0) is spec-TRUE (the
    // n <= 0 arm; no index is accessed), forge check certifies it L3, so the
    // SAME-value twin must let the certified program run CLEAN.
    assert!(
        ran && out.contains("l1-clean"),
        "REQ-20 divergence: the L3-certified `ens bytes_eq(result, result, 5, 5, 0)` \
         (spec value TRUE — the `n <= 0` arm, no index accessed) must NOT abort at \
         the L1 runtime twin; the twin returned a DIFFERENT value than the spec \
         body:\n{out}"
    );
}
