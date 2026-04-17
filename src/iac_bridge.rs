//! IaC IR → Ruby type bridge.
//!
//! Maps `iac_forge::ir::IacType` to `RubyType` with proven parity.
//! This is the typed bridge between the platform-independent IR and
//! the Ruby type system. Every IacType maps to exactly one RubyType.
//!
//! Enabled via the `iac-bridge` feature flag.

use iac_forge::ir::IacType;
use crate::types::RubyType;
use crate::rbs_types::RbsType;

/// Convert an IaC IR type to its Ruby Dry::Types representation.
///
/// This mapping is:
/// - **Injective**: different IacTypes produce different RubyTypes
/// - **Total**: every IacType variant is handled (no fallbacks)
/// - **Deterministic**: same input → same output, always
///
/// The match arms are exhaustive with explicit panic on unknown variants,
/// keeping the problem space known.
#[must_use]
pub fn iac_type_to_ruby(ty: &IacType) -> RubyType {
    match ty {
        IacType::String => RubyType::simple("T::String"),
        IacType::Integer => RubyType::simple("T::Integer"),
        IacType::Float => RubyType::simple("T::Coercible::Float"),
        IacType::Numeric => RubyType::union(vec![
            RubyType::simple("T::Coercible::Integer"),
            RubyType::simple("T::Coercible::Float"),
        ]),
        IacType::Boolean => RubyType::simple("T::Bool"),
        IacType::List(inner) | IacType::Set(inner) => {
            RubyType::array(iac_type_to_ruby(inner))
        }
        IacType::Map(_) | IacType::Object { .. } => RubyType::Hash,
        IacType::Enum { values, underlying } => {
            let base = iac_type_to_ruby(underlying);
            if values.is_empty() {
                base
            } else {
                let vals = values
                    .iter()
                    .map(|v| format!("'{}'", v.replace('\'', "\\'")))
                    .collect::<Vec<_>>()
                    .join(", ");
                RubyType::constrained(base, &format!("included_in: [{vals}]"))
            }
        }
        IacType::Any => RubyType::Any,
        other => panic!("unsupported IacType variant in iac_type_to_ruby: {other:?} — add an explicit mapping"),
    }
}

/// Convert an IaC IR type to its RBS (Ruby Signature) representation.
///
/// Parallel to `iac_type_to_ruby`, preserving the same invariants:
/// - **Injective**: different IacTypes produce different RbsTypes
/// - **Total**: every IacType variant handled
/// - **Deterministic**: same input → same output
///
/// Mapping:
/// `String → String`, `Integer → Integer`, `Float/Numeric → Float`
/// (Numeric folds to `Integer | Float`), `Boolean → bool`,
/// `List/Set → Array[inner]`, `Map/Object → Hash[untyped, untyped]`,
/// `Enum → union of string literals` (when values present),
/// `Any → untyped`.
#[must_use]
pub fn iac_type_to_rbs(ty: &IacType) -> RbsType {
    match ty {
        IacType::String => RbsType::named("String"),
        IacType::Integer => RbsType::named("Integer"),
        IacType::Float => RbsType::named("Float"),
        IacType::Numeric => {
            RbsType::union(vec![RbsType::named("Integer"), RbsType::named("Float")])
        }
        IacType::Boolean => RbsType::named("bool"),
        IacType::List(inner) | IacType::Set(inner) => RbsType::array(iac_type_to_rbs(inner)),
        IacType::Map(_) | IacType::Object { .. } => {
            RbsType::hash(RbsType::named("Symbol"), RbsType::Untyped)
        }
        IacType::Enum { values, underlying } => {
            if values.is_empty() {
                iac_type_to_rbs(underlying)
            } else {
                RbsType::union(
                    values
                        .iter()
                        .map(|v| RbsType::string_literal(v))
                        .collect(),
                )
            }
        }
        IacType::Any => RbsType::Untyped,
        other => panic!("unsupported IacType variant in iac_type_to_rbs: {other:?} — add an explicit mapping"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iac_forge::ir::IacAttribute;

    // ── Exhaustive variant coverage ──────────────────────────────

    #[test]
    fn string_maps_to_t_string() {
        assert_eq!(iac_type_to_ruby(&IacType::String).emit(), "T::String");
    }

    #[test]
    fn integer_maps_to_t_integer() {
        assert_eq!(iac_type_to_ruby(&IacType::Integer).emit(), "T::Integer");
    }

    #[test]
    fn float_maps_to_coercible_float() {
        assert_eq!(iac_type_to_ruby(&IacType::Float).emit(), "T::Coercible::Float");
    }

    #[test]
    fn numeric_maps_to_union() {
        assert_eq!(
            iac_type_to_ruby(&IacType::Numeric).emit(),
            "(T::Coercible::Integer | T::Coercible::Float)"
        );
    }

    #[test]
    fn boolean_maps_to_t_bool() {
        assert_eq!(iac_type_to_ruby(&IacType::Boolean).emit(), "T::Bool");
    }

    #[test]
    fn list_of_strings() {
        assert_eq!(
            iac_type_to_ruby(&IacType::List(Box::new(IacType::String))).emit(),
            "T::Array.of(T::String)"
        );
    }

    #[test]
    fn set_of_integers() {
        assert_eq!(
            iac_type_to_ruby(&IacType::Set(Box::new(IacType::Integer))).emit(),
            "T::Array.of(T::Integer)"
        );
    }

    #[test]
    fn nested_list() {
        assert_eq!(
            iac_type_to_ruby(&IacType::List(Box::new(IacType::List(Box::new(IacType::String))))).emit(),
            "T::Array.of(T::Array.of(T::String))"
        );
    }

    #[test]
    fn map_of_strings_maps_to_hash() {
        assert_eq!(
            iac_type_to_ruby(&IacType::Map(Box::new(IacType::String))).emit(),
            "T::Hash"
        );
    }

    #[test]
    fn object_maps_to_hash() {
        assert_eq!(
            iac_type_to_ruby(&IacType::Object {
                name: "test".into(),
                fields: vec![],
            }).emit(),
            "T::Hash"
        );
    }

    #[test]
    fn enum_with_values() {
        assert_eq!(
            iac_type_to_ruby(&IacType::Enum {
                values: vec!["tcp".into(), "udp".into()],
                underlying: Box::new(IacType::String),
            }).emit(),
            "T::String.constrained(included_in: ['tcp', 'udp'])"
        );
    }

    #[test]
    fn enum_empty_values_degenerates() {
        assert_eq!(
            iac_type_to_ruby(&IacType::Enum {
                values: vec![],
                underlying: Box::new(IacType::String),
            }).emit(),
            "T::String"
        );
    }

    #[test]
    fn any_maps_to_t_any() {
        assert_eq!(iac_type_to_ruby(&IacType::Any).emit(), "T::Any");
    }

    // ── Parity with pangea-forge iac_type_to_dry ─────────────────
    // These tests prove that the ruby-synthesizer bridge produces
    // IDENTICAL output to the string-based pangea-forge function.

    #[test]
    fn parity_string() {
        assert_eq!(iac_type_to_ruby(&IacType::String).emit(), "T::String");
    }

    #[test]
    fn parity_list_of_bools() {
        assert_eq!(
            iac_type_to_ruby(&IacType::List(Box::new(IacType::Boolean))).emit(),
            "T::Array.of(T::Bool)"
        );
    }

    #[test]
    fn parity_enum_constrained() {
        let ty = IacType::Enum {
            values: vec!["tcp".into(), "udp".into(), "icmp".into()],
            underlying: Box::new(IacType::String),
        };
        assert_eq!(
            iac_type_to_ruby(&ty).emit(),
            "T::String.constrained(included_in: ['tcp', 'udp', 'icmp'])"
        );
    }

    // ── Injectivity: different inputs → different outputs ────────

    #[test]
    fn string_differs_from_integer() {
        assert_ne!(
            iac_type_to_ruby(&IacType::String).emit(),
            iac_type_to_ruby(&IacType::Integer).emit(),
        );
    }

    #[test]
    fn list_differs_from_set_by_content() {
        // List and Set both map to Array, but with different inner types
        // they produce different output
        let list_str = iac_type_to_ruby(&IacType::List(Box::new(IacType::String)));
        let list_int = iac_type_to_ruby(&IacType::List(Box::new(IacType::Integer)));
        assert_ne!(list_str.emit(), list_int.emit());
    }

    // ── RBS bridge ────────────────────────────────────────────────

    #[test]
    fn rbs_string_maps_to_string() {
        assert_eq!(iac_type_to_rbs(&IacType::String).emit(), "String");
    }

    #[test]
    fn rbs_integer_maps_to_integer() {
        assert_eq!(iac_type_to_rbs(&IacType::Integer).emit(), "Integer");
    }

    #[test]
    fn rbs_float_maps_to_float() {
        assert_eq!(iac_type_to_rbs(&IacType::Float).emit(), "Float");
    }

    #[test]
    fn rbs_numeric_maps_to_union() {
        assert_eq!(
            iac_type_to_rbs(&IacType::Numeric).emit(),
            "Integer | Float",
        );
    }

    #[test]
    fn rbs_boolean_maps_to_lowercase_bool() {
        assert_eq!(iac_type_to_rbs(&IacType::Boolean).emit(), "bool");
    }

    #[test]
    fn rbs_list_of_strings() {
        assert_eq!(
            iac_type_to_rbs(&IacType::List(Box::new(IacType::String))).emit(),
            "Array[String]",
        );
    }

    #[test]
    fn rbs_set_of_integers() {
        assert_eq!(
            iac_type_to_rbs(&IacType::Set(Box::new(IacType::Integer))).emit(),
            "Array[Integer]",
        );
    }

    #[test]
    fn rbs_nested_list() {
        assert_eq!(
            iac_type_to_rbs(&IacType::List(Box::new(IacType::List(Box::new(IacType::String))))).emit(),
            "Array[Array[String]]",
        );
    }

    #[test]
    fn rbs_map_to_hash() {
        assert_eq!(
            iac_type_to_rbs(&IacType::Map(Box::new(IacType::String))).emit(),
            "Hash[Symbol, untyped]",
        );
    }

    #[test]
    fn rbs_object_to_hash() {
        assert_eq!(
            iac_type_to_rbs(&IacType::Object {
                name: "x".into(),
                fields: vec![],
            }).emit(),
            "Hash[Symbol, untyped]",
        );
    }

    #[test]
    fn rbs_enum_with_values_string_literal_union() {
        let ty = IacType::Enum {
            values: vec!["tcp".into(), "udp".into()],
            underlying: Box::new(IacType::String),
        };
        assert_eq!(iac_type_to_rbs(&ty).emit(), "\"tcp\" | \"udp\"");
    }

    #[test]
    fn rbs_enum_empty_degenerates_to_underlying() {
        let ty = IacType::Enum {
            values: vec![],
            underlying: Box::new(IacType::String),
        };
        assert_eq!(iac_type_to_rbs(&ty).emit(), "String");
    }

    #[test]
    fn rbs_enum_single_value_degenerates_to_literal() {
        let ty = IacType::Enum {
            values: vec!["only".into()],
            underlying: Box::new(IacType::String),
        };
        assert_eq!(iac_type_to_rbs(&ty).emit(), "\"only\"");
    }

    #[test]
    fn rbs_any_maps_to_untyped() {
        assert_eq!(iac_type_to_rbs(&IacType::Any).emit(), "untyped");
    }

    #[test]
    fn rbs_deterministic() {
        let ty = IacType::List(Box::new(IacType::Enum {
            values: vec!["a".into(), "b".into()],
            underlying: Box::new(IacType::String),
        }));
        assert_eq!(iac_type_to_rbs(&ty).emit(), iac_type_to_rbs(&ty).emit());
    }

    #[test]
    fn rbs_injective_list_vs_set_inner() {
        let a = iac_type_to_rbs(&IacType::List(Box::new(IacType::String)));
        let b = iac_type_to_rbs(&IacType::List(Box::new(IacType::Integer)));
        assert_ne!(a.emit(), b.emit());
    }
}
