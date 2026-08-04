use thermite_syntax::{parse, ArrayLen, Expr, Item, PrimType, Type};

#[test]
fn parses_capacity_constants_array_types_and_initializers() {
    let parsed = parse(
        "const CAP: usize = 4;\n\
         fn make(value: u64) -> [u64; CAP]\n\
           req true ens result[0] == value fx pure\n\
         {\n\
           let slots: [u64; 4] = [value; CAP];\n\
           slots\n\
         }",
    );
    assert!(parsed.is_clean(), "{:?}", parsed.errors);
    let Item::Const(capacity) = &parsed.program.items[0] else {
        panic!("first item must be the capacity declaration")
    };
    assert_eq!(capacity.name, "CAP");
    assert_eq!(capacity.value, 4);

    let Item::Fn(function) = &parsed.program.items[1] else {
        panic!("second item must be the function")
    };
    assert_eq!(
        function.ret,
        Type::Array {
            elem: Box::new(Type::Prim(PrimType::U64)),
            len: ArrayLen::Const("CAP".to_string()),
        }
    );
    let body = function.body.as_ref().unwrap();
    let thermite_syntax::Stmt::Let {
        ty: Some(Type::Array { len, .. }),
        init: Expr::ArrayRepeat { len: init_len, .. },
        ..
    } = &body.stmts[0]
    else {
        panic!("array declaration and repeat initializer must retain their shapes")
    };
    assert_eq!(
        len,
        &ArrayLen::Literal {
            value: 4,
            raw: "4".to_string()
        }
    );
    assert_eq!(init_len, &ArrayLen::Const("CAP".to_string()));
}

#[test]
fn parses_exact_and_empty_array_literals() {
    let parsed = parse(
        "fn exact() -> [u16; 3] req true ens result[0] == 1 fx pure { [1, 2, 3] }\n\
         fn empty() -> [u8; 0] req true ens true fx pure { [] }",
    );
    assert!(parsed.is_clean(), "{:?}", parsed.errors);
    let Item::Fn(exact) = &parsed.program.items[0] else {
        panic!("exact must parse as a function")
    };
    assert!(
        matches!(exact.body.as_ref().unwrap().tail.as_deref(), Some(Expr::Array(values)) if values.len() == 3)
    );
    let Item::Fn(empty) = &parsed.program.items[1] else {
        panic!("empty must parse as a function")
    };
    assert!(
        matches!(empty.body.as_ref().unwrap().tail.as_deref(), Some(Expr::Array(values)) if values.is_empty())
    );
}

#[test]
fn rejects_runtime_array_lengths_and_non_usize_constants() {
    for source in [
        "const CAP: u64 = 4;",
        "fn bad(n: usize) -> [u64; n + 1] req true ens true fx pure { [] }",
        "fn bad(n: usize) -> [u64; 1] req true ens true fx pure { [0; n + 1] }",
    ] {
        let parsed = parse(source);
        assert!(!parsed.is_clean(), "unexpectedly accepted `{source}`");
    }
}
