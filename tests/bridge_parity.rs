//! Bridge parity proofs — 12 property-based tests proving iac_type_to_ruby correctness.
//!
//! These proofs verify that the IaC bridge mapping is:
//! - **Correct**: each scalar type maps to the expected Ruby type
//! - **Total**: no panics on any arbitrary IacType
//! - **Deterministic**: same input always produces the same output
//! - **Compositional**: wrapper types preserve inner type mappings

use proptest::prelude::*;

use iac_forge::ir::{IacAttribute, IacType};
use ruby_synthesizer::iac_bridge::iac_type_to_ruby;

// ── Arbitrary IacType strategy ──────────────────────────────────

fn arb_iac_type() -> impl Strategy<Value = IacType> {
    let leaf = prop_oneof![
        Just(IacType::String),
        Just(IacType::Integer),
        Just(IacType::Float),
        Just(IacType::Numeric),
        Just(IacType::Boolean),
        Just(IacType::Any),
    ];

    leaf.prop_recursive(
        3,  // depth
        32, // max nodes
        6,  // items per collection
        |inner| {
            prop_oneof![
                // List(inner)
                inner.clone().prop_map(|t| IacType::List(Box::new(t))),
                // Set(inner)
                inner.clone().prop_map(|t| IacType::Set(Box::new(t))),
                // Map(inner)
                inner.clone().prop_map(|t| IacType::Map(Box::new(t))),
                // Object { name, fields }
                (
                    "[a-z][a-z_]{1,8}",
                    prop::collection::vec(
                        (
                            "[a-z][a-z_]{1,8}",
                            inner.clone(),
                            any::<bool>(),
                        ).prop_map(|(name, ty, req)| IacAttribute {
                            api_name: name.clone(),
                            canonical_name: name,
                            description: String::new(),
                            iac_type: ty,
                            required: req,
                            optional: !req,
                            computed: false,
                            sensitive: false,
                            json_encoded: false,
                            immutable: false,
                            default_value: None,
                            enum_values: None,
                            read_path: None,
                            update_only: false,
                        }),
                        0..=4,
                    ),
                ).prop_map(|(name, fields)| IacType::Object { name, fields }),
                // Enum { values, underlying }
                (
                    prop::collection::vec("[a-z]{2,6}", 0..=5),
                    inner.clone(),
                ).prop_map(|(values, underlying)| IacType::Enum {
                    values,
                    underlying: Box::new(underlying),
                }),
            ]
        },
    )
}

// ── Proofs 1-6: Exhaustive scalar parity (deterministic, not proptest) ──

#[test]
fn proof_01_string_parity() {
    assert_eq!(iac_type_to_ruby(&IacType::String).emit(), "T::String");
}

#[test]
fn proof_02_integer_parity() {
    assert_eq!(iac_type_to_ruby(&IacType::Integer).emit(), "T::Integer");
}

#[test]
fn proof_03_float_parity() {
    assert_eq!(
        iac_type_to_ruby(&IacType::Float).emit(),
        "T::Coercible::Float"
    );
}

#[test]
fn proof_04_numeric_parity() {
    assert_eq!(
        iac_type_to_ruby(&IacType::Numeric).emit(),
        "(T::Coercible::Integer | T::Coercible::Float)"
    );
}

#[test]
fn proof_05_boolean_parity() {
    assert_eq!(iac_type_to_ruby(&IacType::Boolean).emit(), "T::Bool");
}

#[test]
fn proof_06_any_parity() {
    assert_eq!(iac_type_to_ruby(&IacType::Any).emit(), "T::Any");
}

// ── Proofs 7-12: Property-based proofs via proptest ─────────────

proptest! {
    /// Proof 7: List(arb) maps to T::Array.of(...)
    #[test]
    fn proof_07_list_parity(inner in arb_iac_type()) {
        let list_ty = IacType::List(Box::new(inner));
        let emitted = iac_type_to_ruby(&list_ty).emit();
        prop_assert!(
            emitted.starts_with("T::Array.of("),
            "List type did not emit T::Array.of(...): {emitted}"
        );
        prop_assert!(
            emitted.ends_with(')'),
            "List type emit not properly closed: {emitted}"
        );
    }

    /// Proof 8: Set(arb) maps to T::Array.of(...) — same as List in Ruby
    #[test]
    fn proof_08_set_parity(inner in arb_iac_type()) {
        let set_ty = IacType::Set(Box::new(inner.clone()));
        let list_ty = IacType::List(Box::new(inner));
        let set_emitted = iac_type_to_ruby(&set_ty).emit();
        let list_emitted = iac_type_to_ruby(&list_ty).emit();
        prop_assert_eq!(
            set_emitted, list_emitted,
            "Set and List with same inner should emit identically"
        );
    }

    /// Proof 9: Composition preserving — List(X) emit contains X emit
    #[test]
    fn proof_09_composition_preserving(inner in arb_iac_type()) {
        let inner_emitted = iac_type_to_ruby(&inner).emit();
        let list_emitted = iac_type_to_ruby(&IacType::List(Box::new(inner))).emit();
        prop_assert!(
            list_emitted.contains(&inner_emitted),
            "List emit '{}' does not contain inner emit '{}'",
            list_emitted,
            inner_emitted
        );
    }

    /// Proof 10: Totality — no panics on any arbitrary IacType
    #[test]
    fn proof_10_totality(ty in arb_iac_type()) {
        // If this completes without panic, the function is total
        // over the generated domain.
        let emitted = iac_type_to_ruby(&ty).emit();
        prop_assert!(!emitted.is_empty(), "emit produced empty string");
    }

    /// Proof 11: Determinism — same IacType produces same emit, always
    #[test]
    fn proof_11_determinism(ty in arb_iac_type()) {
        let a = iac_type_to_ruby(&ty).emit();
        let b = iac_type_to_ruby(&ty).emit();
        prop_assert_eq!(a, b, "non-deterministic emit for same IacType");
    }

    /// Proof 12: Nested depth — List(List(X)) has correct nesting
    #[test]
    fn proof_12_nested_depth(inner in arb_iac_type()) {
        let inner_emitted = iac_type_to_ruby(&inner).emit();
        let nested = IacType::List(Box::new(IacType::List(Box::new(inner))));
        let nested_emitted = iac_type_to_ruby(&nested).emit();

        let expected = format!("T::Array.of(T::Array.of({inner_emitted}))");
        prop_assert_eq!(
            nested_emitted,
            expected,
            "double-nested List did not produce correct nesting"
        );
    }
}
