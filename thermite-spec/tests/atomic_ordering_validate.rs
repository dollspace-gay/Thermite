use thermite_spec::{validate, SpecError};
use thermite_syntax::parse;

const PRELUDE: &str = r#"
enum AtomicOrdering { Relaxed, Acquire, Release, AcqRel, SeqCst }
#[sealed] struct AtomicU32 { identity: usize }
"#;

fn validate_src(src: &str) -> Result<(), Vec<SpecError>> {
    let parsed = parse(src);
    assert!(
        parsed.is_clean(),
        "fixture parse errors: {:?}\nsource:\n{src}",
        parsed.errors
    );
    validate(&parsed.program)
}

fn single_order_program(target: &str, order: &str, arity: usize) -> String {
    let (params, args) = match arity {
        1 => ("", format!("AtomicOrdering::{order}")),
        2 => (
            "cell: &AtomicU32, ",
            format!("cell, AtomicOrdering::{order}"),
        ),
        3 => (
            "cell: &AtomicU32, ",
            format!("cell, 1, AtomicOrdering::{order}"),
        ),
        _ => panic!("unsupported test arity"),
    };
    format!(
        r#"{PRELUDE}
#[boundary("{target}")]
fn primitive({params}order: AtomicOrdering) -> u32
  req true ens result == result fx platform(atomic);

fn caller({params}order: AtomicOrdering) -> u32
  req true ens result == result fx platform(atomic)
{{ primitive({args}) }}
"#
    )
}

fn cas_program(success: &str, failure: &str) -> String {
    format!(
        r#"{PRELUDE}
#[boundary("thermite::atomic::u32::compare_exchange")]
fn primitive(
  cell: &AtomicU32,
  current: u32,
  value: u32,
  success: AtomicOrdering,
  failure: AtomicOrdering,
) -> u32
  req true ens result == result fx platform(atomic);

fn caller(cell: &AtomicU32) -> u32
  req true ens result == result fx platform(atomic)
{{
  primitive(
    cell,
    0,
    1,
    AtomicOrdering::{success},
    AtomicOrdering::{failure},
  )
}}
"#
    )
}

fn has_atomic_error(errors: &[SpecError]) -> bool {
    errors
        .iter()
        .any(|error| matches!(error, SpecError::IllegalAtomicOrdering { .. }))
}

#[test]
fn accepts_every_legal_single_order_shape() {
    for order in ["Relaxed", "Acquire", "SeqCst"] {
        assert!(
            validate_src(&single_order_program(
                "thermite::atomic::u32::load",
                order,
                2
            ))
            .is_ok(),
            "legal load ordering {order} was rejected"
        );
    }
    for order in ["Relaxed", "Release", "SeqCst"] {
        assert!(
            validate_src(&single_order_program(
                "thermite::atomic::u32::store",
                order,
                3
            ))
            .is_ok(),
            "legal store ordering {order} was rejected"
        );
    }
    for order in ["Acquire", "Release", "AcqRel", "SeqCst"] {
        assert!(
            validate_src(&single_order_program(
                "thermite::atomic::hardware_fence",
                order,
                1
            ))
            .is_ok(),
            "legal fence ordering {order} was rejected"
        );
    }
    for order in ["Relaxed", "Acquire", "Release", "AcqRel", "SeqCst"] {
        assert!(
            validate_src(&single_order_program(
                "thermite::atomic::u32::fetch_add",
                order,
                3
            ))
            .is_ok(),
            "legal RMW ordering {order} was rejected"
        );
    }
}

#[test]
fn rejects_illegal_load_store_and_fence_orderings() {
    for (target, order, arity) in [
        ("thermite::atomic::u32::load", "Release", 2),
        ("thermite::atomic::u32::load", "AcqRel", 2),
        ("thermite::atomic::u32::store", "Acquire", 3),
        ("thermite::atomic::u32::store", "AcqRel", 3),
        ("thermite::atomic::compiler_fence", "Relaxed", 1),
        ("thermite::atomic::hardware_fence", "Relaxed", 1),
    ] {
        let errors = validate_src(&single_order_program(target, order, arity))
            .expect_err("illegal ordering must fail before lowering");
        assert!(
            has_atomic_error(&errors),
            "missing atomic error: {errors:?}"
        );
    }
}

#[test]
fn classifies_all_compare_exchange_order_pairs_exactly() {
    let orders = ["Relaxed", "Acquire", "Release", "AcqRel", "SeqCst"];
    for success in orders {
        for failure in orders {
            let expected_legal = matches!(
                (success, failure),
                ("Relaxed", "Relaxed")
                    | ("Acquire", "Relaxed")
                    | ("Acquire", "Acquire")
                    | ("Release", "Relaxed")
                    | ("AcqRel", "Relaxed")
                    | ("AcqRel", "Acquire")
                    | ("SeqCst", "Relaxed")
                    | ("SeqCst", "Acquire")
                    | ("SeqCst", "SeqCst")
            );
            let result = validate_src(&cas_program(success, failure));
            assert_eq!(
                result.is_ok(),
                expected_legal,
                "compare-exchange pair {success}/{failure} classified incorrectly: {result:?}"
            );
        }
    }
}

#[test]
fn rejects_dynamic_bogus_and_wrong_arity_ordering_calls() {
    let dynamic = format!(
        r#"{PRELUDE}
#[boundary("thermite::atomic::u32::load")]
fn primitive(cell: &AtomicU32, order: AtomicOrdering) -> u32
  req true ens result == result fx platform(atomic);
fn caller(cell: &AtomicU32, order: AtomicOrdering) -> u32
  req true ens result == result fx platform(atomic)
{{ primitive(cell, order) }}
"#
    );
    let errors = validate_src(&dynamic).expect_err("dynamic ordering must fail closed");
    assert!(has_atomic_error(&errors));

    let bogus = format!(
        r#"{PRELUDE}
#[boundary("thermite::atomic::u32::load")]
fn primitive(cell: &AtomicU32, order: AtomicOrdering) -> u32
  req true ens result == result fx platform(atomic);
fn caller(cell: &AtomicU32) -> u32
  req true ens result == result fx platform(atomic)
{{ primitive(cell, OtherOrdering::Acquire) }}
"#
    );
    let errors = validate_src(&bogus).expect_err("bogus ordering path must fail closed");
    assert!(has_atomic_error(&errors));

    let wrong_arity = format!(
        r#"{PRELUDE}
#[boundary("thermite::atomic::u32::load")]
fn primitive(cell: &AtomicU32, order: AtomicOrdering) -> u32
  req true ens result == result fx platform(atomic);
fn caller(cell: &AtomicU32) -> u32
  req true ens result == result fx platform(atomic)
{{ primitive(cell) }}
"#
    );
    let errors = validate_src(&wrong_arity).expect_err("unchecked arity must fail closed");
    assert!(has_atomic_error(&errors));
}

#[test]
fn similarly_named_non_atomic_functions_retain_ordinary_semantics() {
    let source = format!(
        r#"{PRELUDE}
fn atomic_u32_load(cell: &AtomicU32, order: AtomicOrdering) -> u32
  req true ens result == 0 fx pure
{{ 0 }}

fn caller(cell: &AtomicU32, order: AtomicOrdering) -> u32
  req true ens result == 0 fx pure
{{ atomic_u32_load(cell, order) }}
"#
    );
    assert!(validate_src(&source).is_ok());
}

#[test]
fn rejects_atomic_boundary_aliases_that_bypass_direct_ordering_inspection() {
    let source = format!(
        r#"{PRELUDE}
#[boundary("thermite::atomic::u32::load")]
fn primitive(cell: &AtomicU32, order: AtomicOrdering) -> u32
  req true ens result == result fx platform(atomic);

fn caller(cell: &AtomicU32, order: AtomicOrdering) -> u32
  req true ens result == result fx platform(atomic)
{{
  let alias = primitive;
  alias(cell, order)
}}
"#
    );
    let errors = validate_src(&source).expect_err("atomic function aliases must fail closed");
    assert!(has_atomic_error(&errors));
    assert!(errors.iter().any(|error| {
        matches!(
            error,
            SpecError::IllegalAtomicOrdering { detail, .. }
                if detail.contains("must be called directly")
        )
    }));
}
