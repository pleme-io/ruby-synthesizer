//! RBS type expressions — structural representation of Ruby type signatures.
//!
//! Parallels `RubyType` but emits RBS (Ruby Signature) syntax used by
//! `steep` / `rbs` for static type checking. Constructed in Rust, emitted
//! as valid RBS. The type system guarantees well-formedness.

/// An RBS type expression (e.g., `String`, `Array[String]`, `String?`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RbsType {
    /// Named type: `String`, `Integer`, `bool`, `untyped`, `Float`
    Named(String),
    /// Array: `Array[inner]`
    Array(Box<RbsType>),
    /// Hash: `Hash[key, value]`
    Hash(Box<RbsType>, Box<RbsType>),
    /// Union: `A | B | ...`
    Union(Vec<RbsType>),
    /// String literal (for enum discrimination): `"tcp"`
    StringLiteral(String),
    /// Nilable wrapper: `inner?`
    Nilable(Box<RbsType>),
    /// Untyped — RBS's top type.
    Untyped,
}

impl RbsType {
    /// Convenience: named type.
    #[must_use]
    pub fn named(name: &str) -> Self {
        Self::Named(name.to_string())
    }

    /// Convenience: `Array[inner]`.
    #[must_use]
    pub fn array(inner: Self) -> Self {
        Self::Array(Box::new(inner))
    }

    /// Convenience: `Hash[K, V]`.
    #[must_use]
    pub fn hash(key: Self, value: Self) -> Self {
        Self::Hash(Box::new(key), Box::new(value))
    }

    /// Union of types. Enforces invariants:
    /// - 0 variants: panics (invalid — use `Untyped`)
    /// - 1 variant: degenerates to that variant (no wrapping)
    /// - 2+ variants: produces Union
    #[must_use]
    pub fn union(variants: Vec<Self>) -> Self {
        match variants.len() {
            0 => panic!("RbsType::union with 0 variants is invalid — use Untyped"),
            1 => variants.into_iter().next().expect("checked len"),
            _ => Self::Union(variants),
        }
    }

    /// Convenience: string literal.
    #[must_use]
    pub fn string_literal(value: &str) -> Self {
        Self::StringLiteral(value.to_string())
    }

    /// Nilable wrapper. Idempotent: `nilable(nilable(x)) == nilable(x)`.
    #[must_use]
    pub fn nilable(inner: Self) -> Self {
        if matches!(inner, Self::Nilable(_)) {
            return inner;
        }
        Self::Nilable(Box::new(inner))
    }

    /// Emit as RBS source string.
    #[must_use]
    pub fn emit(&self) -> String {
        match self {
            Self::Named(name) => name.clone(),
            Self::Array(inner) => format!("Array[{}]", inner.emit()),
            Self::Hash(k, v) => format!("Hash[{}, {}]", k.emit(), v.emit()),
            Self::Union(variants) => variants
                .iter()
                .map(Self::emit)
                .collect::<Vec<_>>()
                .join(" | "),
            Self::StringLiteral(value) => format!("\"{}\"", value.replace('"', "\\\"")),
            Self::Nilable(inner) => format!("{}?", inner.emit()),
            Self::Untyped => "untyped".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_type() {
        assert_eq!(RbsType::named("String").emit(), "String");
    }

    #[test]
    fn array_type() {
        assert_eq!(
            RbsType::array(RbsType::named("String")).emit(),
            "Array[String]",
        );
    }

    #[test]
    fn nested_array() {
        assert_eq!(
            RbsType::array(RbsType::array(RbsType::named("Integer"))).emit(),
            "Array[Array[Integer]]",
        );
    }

    #[test]
    fn hash_type() {
        assert_eq!(
            RbsType::hash(RbsType::named("Symbol"), RbsType::Untyped).emit(),
            "Hash[Symbol, untyped]",
        );
    }

    #[test]
    fn union_two() {
        assert_eq!(
            RbsType::union(vec![RbsType::named("Integer"), RbsType::named("Float")]).emit(),
            "Integer | Float",
        );
    }

    #[test]
    fn union_one_degenerates() {
        assert_eq!(
            RbsType::union(vec![RbsType::named("String")]).emit(),
            "String",
        );
    }

    #[test]
    #[should_panic(expected = "0 variants")]
    fn union_zero_panics() {
        let _ = RbsType::union(vec![]);
    }

    #[test]
    fn string_literal() {
        assert_eq!(RbsType::string_literal("tcp").emit(), "\"tcp\"");
    }

    #[test]
    fn string_literal_escapes_quotes() {
        assert_eq!(RbsType::string_literal("a\"b").emit(), "\"a\\\"b\"");
    }

    #[test]
    fn nilable() {
        assert_eq!(RbsType::nilable(RbsType::named("String")).emit(), "String?");
    }

    #[test]
    fn nilable_array() {
        assert_eq!(
            RbsType::nilable(RbsType::array(RbsType::named("String"))).emit(),
            "Array[String]?",
        );
    }

    #[test]
    fn nilable_idempotent() {
        let once = RbsType::nilable(RbsType::named("String"));
        let twice = RbsType::nilable(once.clone());
        assert_eq!(once, twice);
    }

    #[test]
    fn untyped() {
        assert_eq!(RbsType::Untyped.emit(), "untyped");
    }

    #[test]
    fn union_of_string_literals() {
        let ty = RbsType::union(vec![
            RbsType::string_literal("tcp"),
            RbsType::string_literal("udp"),
            RbsType::string_literal("icmp"),
        ]);
        assert_eq!(ty.emit(), "\"tcp\" | \"udp\" | \"icmp\"");
    }
}
