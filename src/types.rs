//! Ruby type expressions — structural representation of Dry::Types.

/// A Ruby type expression (e.g., `T::String`, `T::Array.of(T::String)`).
///
/// Constructed in Rust, emitted as valid Ruby. No string manipulation
/// at the call site — the type system guarantees well-formedness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RubyType {
    /// Simple named type: `T::String`, `T::Integer`, `T::Bool`
    Simple(String),
    /// Array type: `T::Array.of(inner)`
    Array(Box<RubyType>),
    /// Hash type: `T::Hash`
    Hash,
    /// Union type: `(T::X | T::Y)`
    Union(Vec<RubyType>),
    /// Constrained type: `T::String.constrained(included_in: [...])`
    Constrained {
        base: Box<RubyType>,
        constraint: String,
    },
    /// Optional wrapper: `inner.optional`
    Optional(Box<RubyType>),
    /// Any type: `T::Any`
    Any,
}

impl RubyType {
    /// Convenience: simple named type.
    #[must_use]
    pub fn simple(name: &str) -> Self {
        Self::Simple(name.to_string())
    }

    /// Convenience: array of inner type.
    #[must_use]
    pub fn array(inner: Self) -> Self {
        Self::Array(Box::new(inner))
    }

    /// Convenience: union of types.
    #[must_use]
    pub fn union(variants: Vec<Self>) -> Self {
        Self::Union(variants)
    }

    /// Convenience: constrained type.
    #[must_use]
    pub fn constrained(base: Self, constraint: &str) -> Self {
        Self::Constrained {
            base: Box::new(base),
            constraint: constraint.to_string(),
        }
    }

    /// Convenience: optional wrapper.
    #[must_use]
    pub fn optional(inner: Self) -> Self {
        Self::Optional(Box::new(inner))
    }

    /// Emit this type expression as a Ruby string.
    #[must_use]
    pub fn emit(&self) -> String {
        match self {
            Self::Simple(name) => name.clone(),
            Self::Array(inner) => format!("T::Array.of({})", inner.emit()),
            Self::Hash => "T::Hash".to_string(),
            Self::Union(variants) => {
                let parts: Vec<String> = variants.iter().map(Self::emit).collect();
                format!("({})", parts.join(" | "))
            }
            Self::Constrained { base, constraint } => {
                format!("{}.constrained({})", base.emit(), constraint)
            }
            Self::Optional(inner) => format!("{}.optional", inner.emit()),
            Self::Any => "T::Any".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_type() {
        assert_eq!(RubyType::simple("T::String").emit(), "T::String");
    }

    #[test]
    fn array_type() {
        assert_eq!(
            RubyType::array(RubyType::simple("T::String")).emit(),
            "T::Array.of(T::String)"
        );
    }

    #[test]
    fn nested_array() {
        assert_eq!(
            RubyType::array(RubyType::array(RubyType::simple("T::String"))).emit(),
            "T::Array.of(T::Array.of(T::String))"
        );
    }

    #[test]
    fn union_type() {
        assert_eq!(
            RubyType::union(vec![
                RubyType::simple("T::Coercible::Integer"),
                RubyType::simple("T::Coercible::Float"),
            ]).emit(),
            "(T::Coercible::Integer | T::Coercible::Float)"
        );
    }

    #[test]
    fn constrained_type() {
        assert_eq!(
            RubyType::constrained(
                RubyType::simple("T::String"),
                "included_in: ['tcp', 'udp']",
            ).emit(),
            "T::String.constrained(included_in: ['tcp', 'udp'])"
        );
    }

    #[test]
    fn optional_type() {
        assert_eq!(
            RubyType::optional(RubyType::simple("T::String")).emit(),
            "T::String.optional"
        );
    }

    #[test]
    fn optional_array() {
        assert_eq!(
            RubyType::optional(RubyType::array(RubyType::simple("T::String"))).emit(),
            "T::Array.of(T::String).optional"
        );
    }

    #[test]
    fn any_type() {
        assert_eq!(RubyType::Any.emit(), "T::Any");
    }

    #[test]
    fn hash_type() {
        assert_eq!(RubyType::Hash.emit(), "T::Hash");
    }
}
