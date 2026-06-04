// L1 runtime-check lowering of `conformance/sum.th` — the L1 rung
// (.design/lower/l1-runtime-checks.md). Reference oracle for
// `thermite-lower::l1`: hand-authored from the design (R-CHAR-3), compiles and
// runs under `rustc`, and its always-active contract checks fire on violation.
//
// This is the EXECUTABLE counterpart of `tests/golden/lower/sum.verus.rs`: where
// the L3 file carries Verus annotations for an SMT proof, this file EXECUTES the
// same contract — every `req`/`ens`/`inv` becomes a runtime check, and the
// `spec fn` is a real recursive Rust fn (§4.2 "spec functions are executable").

/// The defined contract-violation behavior of the GENERATED program (not a
/// toolchain panic — this is the L1 program's intended abort with a structured,
/// legible diagnostic; §2.4 / §6). Always active in every build profile.
fn thermite_contract_violation(kind: &str, text: &str) -> ! {
    panic!("thermite L1 contract violation [{kind}]: {text}");
}

/// Always-active check — a plain `if !(cond)`, NOT a debug-only assertion macro
/// (those are stripped in release; §6 demands the check in every build profile).
macro_rules! thermite_check {
    ($kind:literal, $text:literal, $cond:expr) => {
        if !($cond) {
            thermite_contract_violation($kind, $text);
        }
    };
}

// Executable `spec fn` (REQ-4): the same `spec_sum` text the contract names is
// runnable. Slice-match `[] => 0, [head, ..t] => …` → length branch over `&[u32]`.
fn spec_sum(xs: &[u32]) -> u64 {
    if xs.is_empty() {
        0
    } else {
        xs[0] as u64 + spec_sum(&xs[1..])
    }
}

fn sum(xs: &[u32]) -> u64 {
    thermite_check!("req", "xs.len() <= 1_000_000", xs.len() <= 1_000_000);
    let result = {
        let mut acc: u64 = 0;
        let mut i: usize = 0;
        while i < xs.len() {
            thermite_check!("inv", "i <= xs.len()", i <= xs.len());
            thermite_check!("inv", "acc == spec_sum(&xs[..i])", acc == spec_sum(&xs[..i]));
            thermite_check!(
                "inv",
                "acc <= i as u64 * u32::MAX as u64",
                acc <= i as u64 * u32::MAX as u64
            );
            acc = acc + xs[i] as u64;
            i = i + 1;
        }
        acc
    };
    thermite_check!("ens", "result == spec_sum(xs)", result == spec_sum(xs));
    thermite_check!(
        "ens",
        "result <= xs.len() as u64 * u32::MAX as u64",
        result <= xs.len() as u64 * u32::MAX as u64
    );
    result
}

fn main() {
    // Positive: valid inputs satisfy every contract check (no violation fires).
    assert_eq!(sum(&[1, 2, 3]), 6);
    assert_eq!(sum(&[]), 0);
    assert_eq!(sum(&[7]), 7);
    // The executable spec fn agrees with the L3 `Seq` denotation (AC-4).
    assert_eq!(spec_sum(&[1, 2, 3]), 6);
    println!("ok");
}
