use thermite_lower::{lower, lower_l1, lower_l2};
use thermite_syntax::parse;

const SOURCE: &str = "const SLOTS: usize = 4;\n\
fn store(at: usize, value: u64) -> u64\n\
req at < SLOTS\n\
ens result == value\n\
fx pure\n\
{\n\
  let mut slots: [u64; SLOTS] = [0; SLOTS];\n\
  slots[at] = value;\n\
  slots[at]\n\
}\n\
fn read(slots: [u64; SLOTS], at: usize) -> u64\n\
req at < SLOTS\n\
ens result == slots[at]\n\
fx pure\n\
{ slots[at] }\n\
fn equal(left: [u64; SLOTS], right: [u64; SLOTS]) -> bool\n\
req true\n\
ens result == left.array_eq(right)\n\
fx pure\n\
{ left.array_eq(right) }\n";

fn program() -> thermite_syntax::Program {
    let parsed = parse(SOURCE);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    thermite_spec::validate(&parsed.program).expect("fixed-array program must validate");
    parsed.program
}

#[test]
fn l3_uses_native_fixed_arrays_and_preserves_mutation() {
    let emitted = lower(&program()).expect("L3 fixed-array lowering must succeed");
    assert!(emitted.contains("pub const SLOTS: usize = 4;"), "{emitted}");
    assert!(
        emitted.contains("let mut slots: [u64; SLOTS] = [0; SLOTS];"),
        "{emitted}"
    );
    assert!(emitted.contains("slots[at] = value;"), "{emitted}");
    assert!(emitted.contains("slots@[at as int]"), "{emitted}");
    assert!(
        emitted.contains("pub trait __thermite_FixedArrayEq"),
        "{emitted}"
    );
    assert!(
        emitted.contains("(left).__thermite_fixed_array_eq(&(right))"),
        "{emitted}"
    );
    assert!(emitted.contains("((left)@ =~= (right)@)"), "{emitted}");
}

#[test]
fn runtime_and_bounded_backends_keep_exact_capacity() {
    let program = program();
    let l1 = lower_l1(&program).expect("L1 fixed-array lowering must succeed");
    assert!(l1.contains("pub const SLOTS: usize = 4;"), "{l1}");
    assert!(l1.contains("[u64; SLOTS]"), "{l1}");
    assert!(l1.contains("[0; SLOTS]"), "{l1}");
    assert!(l1.contains("slots[at] = value;"), "{l1}");
    assert!(l1.contains("(left) == (right)"), "{l1}");

    let l2 = lower_l2(&program).expect("L2 fixed-array lowering must succeed");
    assert!(l2.contains("const SLOTS: usize = 4;"), "{l2}");
    assert!(l2.contains("[u64; SLOTS]"), "{l2}");
    assert!(l2.contains("[0; SLOTS]"), "{l2}");
    assert!(l2.contains("(left) == (right)"), "{l2}");
}
