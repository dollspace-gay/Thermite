//! Hand-derived surface tests for reusable kernel-authoring primitives.

use thermite_syntax::{parse, Effect, Item, PlatformDomain, PrimType, Type};

#[test]
fn parses_freestanding_scalar_widths_and_every_platform_domain() {
    let source = r#"
fn platform_probe(byte: u8, word: u16) -> u16
  req byte as u16 <= word
  ens result == word
  fx platform(boot), platform(memory), platform(mmio), platform(pio),
     platform(irq), platform(cpu), platform(atomic), platform(smp),
     platform(dma), platform(clock), platform(entropy), platform(power)
{
  word
}
"#;

    let parsed = parse(source);
    assert!(
        parsed.is_clean(),
        "freestanding scalar/effect surface must parse: {:?}",
        parsed.errors
    );
    let Item::Fn(function) = &parsed.program.items[0] else {
        panic!("expected a function item");
    };
    assert_eq!(function.params[0].ty, Type::Prim(PrimType::U8));
    assert_eq!(function.params[1].ty, Type::Prim(PrimType::U16));
    assert_eq!(function.ret, Type::Prim(PrimType::U16));

    let thermite_syntax::EffectRow::Set(effects) = &function.contract.fx else {
        panic!("expected the explicit platform effect row");
    };
    assert_eq!(
        effects,
        &vec![
            Effect::Platform(PlatformDomain::Boot),
            Effect::Platform(PlatformDomain::Memory),
            Effect::Platform(PlatformDomain::Mmio),
            Effect::Platform(PlatformDomain::Pio),
            Effect::Platform(PlatformDomain::Irq),
            Effect::Platform(PlatformDomain::Cpu),
            Effect::Platform(PlatformDomain::Atomic),
            Effect::Platform(PlatformDomain::Smp),
            Effect::Platform(PlatformDomain::Dma),
            Effect::Platform(PlatformDomain::Clock),
            Effect::Platform(PlatformDomain::Entropy),
            Effect::Platform(PlatformDomain::Power),
        ]
    );
}

#[test]
fn rejects_unregistered_platform_domain() {
    let source = r#"
fn bad() -> ()
  req true
  ens true
  fx platform(filesystem)
{
}
"#;
    let parsed = parse(source);
    assert_eq!(parsed.program.items.len(), 0);
    assert_eq!(parsed.errors.len(), 1);
    assert!(
        parsed.errors[0].to_string().contains("a platform domain"),
        "unexpected diagnostic: {}",
        parsed.errors[0]
    );
}

#[test]
fn parses_mutable_byte_slice_write_and_final_state_contract() {
    let parsed = parse(
        "fn write_byte(data: &mut [u8], at: usize, value: u8) -> u8\n\
         req at < data.len()\n\
         ens result == value\n\
         ens final(data)[at] == value\n\
         fx platform(memory)\n\
         { data[at] = value; value }\n",
    );
    assert!(
        parsed.is_clean(),
        "mutable slice surface: {:?}",
        parsed.errors
    );
    let Item::Fn(function) = &parsed.program.items[0] else {
        panic!("expected mutable-slice function");
    };
    assert_eq!(
        function.params[0].ty,
        Type::Ref {
            mutable: true,
            inner: Box::new(Type::Slice(Box::new(Type::Prim(PrimType::U8)))),
        }
    );
    assert_eq!(function.contract.ens.len(), 2);
}

#[test]
fn distinguishes_borrowed_fixed_arrays_from_slices_of_arrays() {
    let parsed = parse(
        "fn storage_refs(array: &mut [u64; 4], rows: &mut [[u64; 2]]) -> u64\n\
         req true ens result == 0 fx platform(memory) { 0 }\n",
    );
    assert!(parsed.is_clean(), "borrowed storage: {:?}", parsed.errors);
    let Item::Fn(function) = &parsed.program.items[0] else {
        panic!("expected storage_refs function");
    };
    assert_eq!(
        function.params[0].ty,
        Type::Ref {
            mutable: true,
            inner: Box::new(Type::Array {
                elem: Box::new(Type::Prim(PrimType::U64)),
                len: thermite_syntax::ArrayLen::Literal {
                    value: 4,
                    raw: "4".to_string(),
                },
            }),
        }
    );
    assert_eq!(
        function.params[1].ty,
        Type::Ref {
            mutable: true,
            inner: Box::new(Type::Slice(Box::new(Type::Array {
                elem: Box::new(Type::Prim(PrimType::U64)),
                len: thermite_syntax::ArrayLen::Literal {
                    value: 2,
                    raw: "2".to_string(),
                },
            }))),
        }
    );
}
