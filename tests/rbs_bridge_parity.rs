//! RBS bridge parity proofs — property-based tests over `iac_type_to_rbs`.
//!
//! Parallels `tests/bridge_parity.rs` for the Ruby bridge. Same invariants:
//! - **Total**: no panics on any arbitrary IacType
//! - **Deterministic**: same input always produces the same output
//! - **Non-empty emission**: every valid IacType renders to non-empty RBS
//! - **Balanced brackets**: Array[...] and Hash[...] are always balanced
//! - **Compositional**: wrapper types preserve inner type mappings

use proptest::prelude::*;

use iac_forge::ir::{IacAttribute, IacType};
use ruby_synthesizer::iac_bridge::iac_type_to_rbs;

// ── Arbitrary IacType strategy (mirrors bridge_parity.rs) ───────────

fn arb_iac_type() -> impl Strategy<Value = IacType> {
    let leaf = prop_oneof![
        Just(IacType::String),
        Just(IacType::Integer),
        Just(IacType::Float),
        Just(IacType::Numeric),
        Just(IacType::Boolean),
        Just(IacType::Any),
    ];

    leaf.prop_recursive(3, 32, 6, |inner| {
        prop_oneof![
            inner.clone().prop_map(|t| IacType::List(Box::new(t))),
            inner.clone().prop_map(|t| IacType::Set(Box::new(t))),
            inner.clone().prop_map(|t| IacType::Map(Box::new(t))),
            (
                "[a-z][a-z_]{1,8}",
                prop::collection::vec(
                    ("[a-z][a-z_]{1,8}", inner.clone(), any::<bool>()).prop_map(
                        |(name, ty, req)| IacAttribute {
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
                        }
                    ),
                    0..4,
                )
            )
                .prop_map(|(name, fields)| IacType::Object { name, fields }),
            (
                prop::collection::vec("[a-z][a-z_]{1,8}", 1..6),
                inner.clone(),
            )
                .prop_map(|(values, underlying)| IacType::Enum {
                    values,
                    underlying: Box::new(underlying),
                }),
        ]
    })
}

// ── Properties ──────────────────────────────────────────────────────

proptest! {
    /// Totality: no panics for any arbitrary IacType.
    #[test]
    fn rbs_bridge_is_total(ty in arb_iac_type()) {
        let _ = iac_type_to_rbs(&ty);
    }

    /// Determinism: same input → same output.
    #[test]
    fn rbs_bridge_is_deterministic(ty in arb_iac_type()) {
        prop_assert_eq!(iac_type_to_rbs(&ty).emit(), iac_type_to_rbs(&ty).emit());
    }

    /// Non-empty emission: every valid IacType renders to non-empty RBS.
    #[test]
    fn rbs_emission_is_nonempty(ty in arb_iac_type()) {
        let rbs = iac_type_to_rbs(&ty).emit();
        prop_assert!(!rbs.is_empty(), "empty RBS emission for {:?}", ty);
    }

    /// No internal newlines — every RBS type expression fits on one line.
    #[test]
    fn rbs_emission_has_no_newlines(ty in arb_iac_type()) {
        let rbs = iac_type_to_rbs(&ty).emit();
        prop_assert!(!rbs.contains('\n'), "newline in RBS emission: {:?}", rbs);
    }

    /// Printable ASCII: generated RBS must be clean source.
    #[test]
    fn rbs_emission_is_printable_ascii(ty in arb_iac_type()) {
        let rbs = iac_type_to_rbs(&ty).emit();
        for ch in rbs.chars() {
            prop_assert!(
                ch == ' ' || (ch as u32 >= 0x21 && ch as u32 <= 0x7e),
                "non-printable-ASCII char {:?} in RBS: {:?}", ch, rbs,
            );
        }
    }

    /// Bracket balance: every `[` has a matching `]`.
    #[test]
    fn rbs_brackets_are_balanced(ty in arb_iac_type()) {
        let rbs = iac_type_to_rbs(&ty).emit();
        let open = rbs.matches('[').count();
        let close = rbs.matches(']').count();
        prop_assert_eq!(open, close, "unbalanced brackets in {:?}", rbs);
    }

    /// Quote balance: every `"` is matched by another `"` (string literals).
    #[test]
    fn rbs_string_literals_are_balanced(ty in arb_iac_type()) {
        let rbs = iac_type_to_rbs(&ty).emit();
        let unescaped_quotes = rbs.chars().enumerate().filter(|(i, c)| {
            *c == '"' && (*i == 0 || rbs.as_bytes()[*i - 1] != b'\\')
        }).count();
        prop_assert_eq!(
            unescaped_quotes % 2, 0,
            "odd number of unescaped quotes in {:?}", rbs,
        );
    }

    /// Compositional: List(inner) always contains inner's emission.
    #[test]
    fn list_contains_inner_emission(inner in arb_iac_type()) {
        let list = IacType::List(Box::new(inner.clone()));
        let outer = iac_type_to_rbs(&list).emit();
        let inner_emit = iac_type_to_rbs(&inner).emit();
        prop_assert!(
            outer.contains(&inner_emit),
            "List[{}] should contain inner {}: got {}",
            inner_emit, inner_emit, outer,
        );
    }

    /// Compositional: Set and List produce identical RBS for identical inner.
    #[test]
    fn set_and_list_rbs_are_identical(inner in arb_iac_type()) {
        let list = IacType::List(Box::new(inner.clone()));
        let set = IacType::Set(Box::new(inner));
        prop_assert_eq!(
            iac_type_to_rbs(&list).emit(),
            iac_type_to_rbs(&set).emit(),
        );
    }

    /// Map and Object emit the same RBS shape (Hash[Symbol, untyped]).
    #[test]
    fn map_and_object_emit_same_hash(inner in arb_iac_type()) {
        let map = IacType::Map(Box::new(inner.clone()));
        let obj = IacType::Object { name: "x".into(), fields: vec![] };
        prop_assert_eq!(
            iac_type_to_rbs(&map).emit(),
            iac_type_to_rbs(&obj).emit(),
        );
        prop_assert_eq!(iac_type_to_rbs(&map).emit(), "Hash[Symbol, untyped]");
        // Silence the unused inner warning.
        let _ = inner;
    }

    /// Enum with at least one value produces a union of string literals.
    #[test]
    fn enum_with_values_is_string_literal_union(values in prop::collection::vec("[a-z][a-z_]{1,5}", 1..5)) {
        let ty = IacType::Enum {
            values: values.clone(),
            underlying: Box::new(IacType::String),
        };
        let rbs = iac_type_to_rbs(&ty).emit();
        // Every value must appear as a quoted literal in the output.
        for v in &values {
            prop_assert!(
                rbs.contains(&format!("\"{v}\"")),
                "value {} missing from enum emission {}", v, rbs,
            );
        }
        // Number of pipes = values.len() - 1 (the union separator count).
        let expected_pipes = values.len().saturating_sub(1);
        let actual_pipes = rbs.matches(" | ").count();
        prop_assert_eq!(expected_pipes, actual_pipes);
    }

    /// Injectivity on scalars: distinct scalar IacTypes produce distinct RBS.
    #[test]
    fn scalar_injectivity(
        (a, b) in (
            prop_oneof![
                Just(IacType::String),
                Just(IacType::Integer),
                Just(IacType::Float),
                Just(IacType::Boolean),
                Just(IacType::Any),
            ],
            prop_oneof![
                Just(IacType::String),
                Just(IacType::Integer),
                Just(IacType::Float),
                Just(IacType::Boolean),
                Just(IacType::Any),
            ],
        )
    ) {
        if a != b {
            prop_assert_ne!(
                iac_type_to_rbs(&a).emit(),
                iac_type_to_rbs(&b).emit(),
            );
        }
    }
}
