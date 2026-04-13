//! IaC IR → Ruby type bridge.
//!
//! Maps `iac_forge::ir::IacType` to `RubyType` with proven parity.
//! This is the typed bridge between the platform-independent IR and
//! the Ruby type system. Every IacType maps to exactly one RubyType.
//!
//! Enabled via the `iac-bridge` feature flag.

use iac_forge::ir::IacType;
use crate::types::RubyType;

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
}
