use thermite_syntax::{parse_package, PackageModuleSource, PackageParseError, Span};

fn module(name: &str, path: &str, source: &str) -> PackageModuleSource {
    PackageModuleSource {
        name: name.to_string(),
        path: path.to_string(),
        source: source.to_string(),
    }
}

#[test]
fn parses_modules_independently_and_preserves_item_origins() {
    let sources = vec![
        module(
            "base",
            "src/base.th",
            "fn base(x: u64) -> u64 req true ens result == x fx pure { x }\n",
        ),
        module(
            "api",
            "src/api.th",
            "fn api(x: u64) -> u64 req true ens result == x fx pure { base(x) }\n",
        ),
    ];
    let parsed = parse_package(&sources);
    assert!(parsed.is_clean(), "{:?}", parsed.errors);
    assert_eq!(parsed.program.items.len(), 2);
    assert_eq!(parsed.modules[0].first_item, 0);
    assert_eq!(parsed.modules[1].first_item, 1);
    assert_eq!(parsed.origin(0).unwrap().path, "src/base.th");
    assert_eq!(parsed.origin(1).unwrap().path, "src/api.th");
    // Both items begin at byte zero in different sources. Their identity is the
    // module/path pair, not an offset into a concatenated buffer.
    assert_eq!(parsed.origin(0).unwrap().span, Span::new(0, 61));
    assert_eq!(parsed.origin(1).unwrap().span.start, 0);
}

#[test]
fn syntax_diagnostics_name_the_exact_module_and_local_span() {
    let parsed = parse_package(&[
        module(
            "good",
            "good.th",
            "fn good(x: u64) -> u64 req true ens result == x fx pure { x }\n",
        ),
        module("bad", "nested/bad.th", "fn bad(\n"),
    ]);
    let error = parsed
        .errors
        .iter()
        .find_map(|error| match error {
            PackageParseError::Syntax {
                module,
                path,
                error,
            } => Some((module, path, error.span())),
            PackageParseError::DuplicateItem { .. } => None,
        })
        .expect("module-local syntax error");
    assert_eq!(error.0, "bad");
    assert_eq!(error.1, "nested/bad.th");
    assert!(error.2.start <= 8);
}

#[test]
fn duplicate_package_names_report_both_source_locations() {
    let parsed = parse_package(&[
        module(
            "a",
            "a.th",
            "fn same(x: u64) -> u64 req true ens result == x fx pure { x }\n",
        ),
        module(
            "b",
            "b.th",
            "fn same(x: u64) -> u64 req true ens result == x fx pure { x }\n",
        ),
    ]);
    let duplicate = parsed
        .errors
        .iter()
        .find_map(|error| match error {
            PackageParseError::DuplicateItem {
                name,
                first,
                duplicate,
            } => Some((name, first, duplicate)),
            PackageParseError::Syntax { .. } => None,
        })
        .expect("duplicate diagnostic");
    assert_eq!(duplicate.0, "same");
    assert_eq!(duplicate.1.module, "a");
    assert_eq!(duplicate.2.module, "b");
    assert_eq!(duplicate.1.span.start, 0);
    assert_eq!(duplicate.2.span.start, 0);
}

#[test]
fn proof_and_witness_address_roots_are_not_declaration_collisions() {
    let parsed = parse_package(&[
        module(
            "code",
            "code.th",
            "fn same(x: u64) -> u64 req true ens result == x fx pure { x }\n",
        ),
        module(
            "proofs",
            "proofs.th",
            "proof for same { ens#0 by { omega } }\n\
             witness { inhabit (1); falsify 10; }\n\
             witness { inhabit (2); falsify 10; }\n",
        ),
    ]);
    assert!(parsed.is_clean(), "{:?}", parsed.errors);
    assert_eq!(parsed.program.items.len(), 4);
}
