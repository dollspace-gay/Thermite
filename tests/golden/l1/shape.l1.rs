// L1 runtime-check lowering (.design/lower/l1-runtime-checks.md). Self-contained,
// compiles and runs under `rustc`; the always-active contract checks fire on
// violation in every build profile (NOT debug-only).

/// The defined contract-violation behavior of the GENERATED program (not a
/// toolchain panic): the L1 program's intended abort with a structured, legible
/// diagnostic (§2.4 / §6). Always active in every build profile.
fn thermite_contract_violation(kind: &str, text: &str) -> ! {
    panic!("thermite L1 contract violation [{kind}]: {text}");
}

/// Always-active check: a plain `if !(cond)` so the contract is enforced in
/// EVERY build profile (a release-stripped assertion would not be; §6 demands
/// every profile).
macro_rules! thermite_check {
    ($kind:literal, $text:literal, $cond:expr) => {
        if !($cond) {
            thermite_contract_violation($kind, $text);
        }
    };
}

enum Shape {
    Circle(u64),
    Rect { w: u64, h: u64 },
}

fn is_circle(s: Shape) -> bool {
    let result = {
        match s {
            Shape::Circle(r) => true,
            Shape::Rect { w, h } => false,
        }
    };
    thermite_check!("ens", "result == (s is Circle)", result == matches!(s, Shape::Circle { .. }));
    result
}
