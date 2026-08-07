//! `ToSExpr` / `FromSExpr` implementations for `RubyType` and `RbsType`.
//!
//! Delegates the value/trait layer to [`iac_forge::sexpr`] so every repo
//! in the pipeline uses one canonical interchange. The typed ASTs stay
//! the construction surface; the sexpr form is for interchange only.
//!
//! Enabled under the `iac-bridge` feature because it pulls in iac-forge.

use iac_forge::sexpr::{
    FromSExpr, SExpr, SExprError, ToSExpr, parse_struct, struct_expr, take_field,
};

use crate::rbs_types::RbsType;
use crate::types::RubyType;

// ── RubyType ────────────────────────────────────────────────────────

impl ToSExpr for RubyType {
    fn to_sexpr(&self) -> SExpr {
        match self {
            Self::Simple(name) => {
                SExpr::List(vec![SExpr::Symbol("simple".into()), name.to_sexpr()])
            }
            Self::Array(inner) => {
                SExpr::List(vec![SExpr::Symbol("array".into()), inner.to_sexpr()])
            }
            Self::Hash => SExpr::Symbol("hash".into()),
            Self::Union(variants) => {
                let mut items = Vec::with_capacity(variants.len() + 1);
                items.push(SExpr::Symbol("union".into()));
                for v in variants {
                    items.push(v.to_sexpr());
                }
                SExpr::List(items)
            }
            Self::Constrained { base, constraint } => struct_expr(
                "constrained",
                vec![
                    ("base", base.to_sexpr()),
                    ("constraint", constraint.to_sexpr()),
                ],
            ),
            Self::Optional(inner) => {
                SExpr::List(vec![SExpr::Symbol("optional".into()), inner.to_sexpr()])
            }
            Self::Any => SExpr::Symbol("any".into()),
        }
    }
}

impl FromSExpr for RubyType {
    fn from_sexpr(s: &SExpr) -> Result<Self, SExprError> {
        // Unit variants.
        if let SExpr::Symbol(tag) = s {
            return match tag.as_str() {
                "hash" => Ok(Self::Hash),
                "any" => Ok(Self::Any),
                other => Err(SExprError::UnknownVariant(format!("RubyType::{other}"))),
            };
        }

        let items = s.as_list()?;
        let (head, rest) = items
            .split_first()
            .ok_or_else(|| SExprError::Shape("empty RubyType list".into()))?;
        let tag = head.as_symbol()?;
        match tag {
            "simple" => {
                if rest.len() != 1 {
                    return Err(SExprError::Shape(format!(
                        "simple expects 1 arg, got {}",
                        rest.len()
                    )));
                }
                Ok(Self::Simple(String::from_sexpr(&rest[0])?))
            }
            "array" => {
                if rest.len() != 1 {
                    return Err(SExprError::Shape(format!(
                        "array expects 1 arg, got {}",
                        rest.len()
                    )));
                }
                Ok(Self::Array(Box::new(RubyType::from_sexpr(&rest[0])?)))
            }
            "optional" => {
                if rest.len() != 1 {
                    return Err(SExprError::Shape(format!(
                        "optional expects 1 arg, got {}",
                        rest.len()
                    )));
                }
                Ok(Self::Optional(Box::new(RubyType::from_sexpr(&rest[0])?)))
            }
            "union" => {
                if rest.is_empty() {
                    return Err(SExprError::Shape("union must have >= 1 variant".into()));
                }
                let variants = rest
                    .iter()
                    .map(RubyType::from_sexpr)
                    .collect::<Result<Vec<_>, _>>()?;
                if variants.len() == 1 {
                    // Preserve the RubyType::union constructor's degeneracy rule.
                    Ok(variants.into_iter().next().expect("checked"))
                } else {
                    Ok(Self::Union(variants))
                }
            }
            "constrained" => {
                let fields = parse_struct(s, "constrained")?;
                Ok(Self::Constrained {
                    base: Box::new(RubyType::from_sexpr(take_field(&fields, "base")?)?),
                    constraint: String::from_sexpr(take_field(&fields, "constraint")?)?,
                })
            }
            other => Err(SExprError::UnknownVariant(format!("RubyType::{other}"))),
        }
    }
}

// ── RbsType ─────────────────────────────────────────────────────────

impl ToSExpr for RbsType {
    fn to_sexpr(&self) -> SExpr {
        match self {
            Self::Named(name) => SExpr::List(vec![SExpr::Symbol("named".into()), name.to_sexpr()]),
            Self::Array(inner) => {
                SExpr::List(vec![SExpr::Symbol("array".into()), inner.to_sexpr()])
            }
            Self::Hash(key, value) => SExpr::List(vec![
                SExpr::Symbol("hash".into()),
                key.to_sexpr(),
                value.to_sexpr(),
            ]),
            Self::Union(variants) => {
                let mut items = Vec::with_capacity(variants.len() + 1);
                items.push(SExpr::Symbol("union".into()));
                for v in variants {
                    items.push(v.to_sexpr());
                }
                SExpr::List(items)
            }
            Self::StringLiteral(s) => {
                SExpr::List(vec![SExpr::Symbol("string-literal".into()), s.to_sexpr()])
            }
            Self::Nilable(inner) => {
                SExpr::List(vec![SExpr::Symbol("nilable".into()), inner.to_sexpr()])
            }
            Self::Untyped => SExpr::Symbol("untyped".into()),
        }
    }
}

impl FromSExpr for RbsType {
    fn from_sexpr(s: &SExpr) -> Result<Self, SExprError> {
        if let SExpr::Symbol(tag) = s {
            return match tag.as_str() {
                "untyped" => Ok(Self::Untyped),
                other => Err(SExprError::UnknownVariant(format!("RbsType::{other}"))),
            };
        }

        let items = s.as_list()?;
        let (head, rest) = items
            .split_first()
            .ok_or_else(|| SExprError::Shape("empty RbsType list".into()))?;
        let tag = head.as_symbol()?;
        match tag {
            "named" => {
                if rest.len() != 1 {
                    return Err(SExprError::Shape(format!(
                        "named expects 1 arg, got {}",
                        rest.len()
                    )));
                }
                Ok(Self::Named(String::from_sexpr(&rest[0])?))
            }
            "array" => {
                if rest.len() != 1 {
                    return Err(SExprError::Shape(format!(
                        "array expects 1 arg, got {}",
                        rest.len()
                    )));
                }
                Ok(Self::Array(Box::new(RbsType::from_sexpr(&rest[0])?)))
            }
            "hash" => {
                if rest.len() != 2 {
                    return Err(SExprError::Shape(format!(
                        "hash expects 2 args (key, value), got {}",
                        rest.len()
                    )));
                }
                Ok(Self::Hash(
                    Box::new(RbsType::from_sexpr(&rest[0])?),
                    Box::new(RbsType::from_sexpr(&rest[1])?),
                ))
            }
            "union" => {
                if rest.is_empty() {
                    return Err(SExprError::Shape("union must have >= 1 variant".into()));
                }
                let variants = rest
                    .iter()
                    .map(RbsType::from_sexpr)
                    .collect::<Result<Vec<_>, _>>()?;
                if variants.len() == 1 {
                    Ok(variants.into_iter().next().expect("checked"))
                } else {
                    Ok(Self::Union(variants))
                }
            }
            "string-literal" => {
                if rest.len() != 1 {
                    return Err(SExprError::Shape(format!(
                        "string-literal expects 1 arg, got {}",
                        rest.len()
                    )));
                }
                Ok(Self::StringLiteral(String::from_sexpr(&rest[0])?))
            }
            "nilable" => {
                if rest.len() != 1 {
                    return Err(SExprError::Shape(format!(
                        "nilable expects 1 arg, got {}",
                        rest.len()
                    )));
                }
                Ok(Self::Nilable(Box::new(RbsType::from_sexpr(&rest[0])?)))
            }
            other => Err(SExprError::UnknownVariant(format!("RbsType::{other}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── RubyType ──────────────────────────────────────────────

    fn rt_roundtrip(ty: &RubyType) {
        let s = ty.to_sexpr();
        let parsed = RubyType::from_sexpr(&s).expect("parse");
        assert_eq!(&parsed, ty);
        // Text round-trip
        let emitted = s.emit();
        let reparsed =
            RubyType::from_sexpr(&SExpr::parse(&emitted).expect("lex")).expect("from-emit");
        assert_eq!(&reparsed, ty);
    }

    #[test]
    fn rubytype_simple() {
        rt_roundtrip(&RubyType::simple("T::String"));
    }

    #[test]
    fn rubytype_hash() {
        rt_roundtrip(&RubyType::Hash);
    }

    #[test]
    fn rubytype_any() {
        rt_roundtrip(&RubyType::Any);
    }

    #[test]
    fn rubytype_array_of_string() {
        rt_roundtrip(&RubyType::array(RubyType::simple("T::String")));
    }

    #[test]
    fn rubytype_optional() {
        rt_roundtrip(&RubyType::optional(RubyType::simple("T::Integer")));
    }

    #[test]
    fn rubytype_union() {
        rt_roundtrip(&RubyType::union(vec![
            RubyType::simple("T::Coercible::Integer"),
            RubyType::simple("T::Coercible::Float"),
        ]));
    }

    #[test]
    fn rubytype_constrained() {
        rt_roundtrip(&RubyType::constrained(
            RubyType::simple("T::String"),
            "included_in: ['tcp','udp']",
        ));
    }

    #[test]
    fn rubytype_nested_array_of_optional() {
        rt_roundtrip(&RubyType::array(RubyType::optional(RubyType::simple(
            "T::Bool",
        ))));
    }

    #[test]
    fn rubytype_from_sexpr_rejects_unknown_variant() {
        let err = RubyType::from_sexpr(&SExpr::Symbol("nope".into())).unwrap_err();
        assert!(matches!(err, SExprError::UnknownVariant(_)));
    }

    #[test]
    fn rubytype_union_single_variant_degenerates() {
        // emit → parse of a single-variant list must NOT produce a Union
        // (matches the constructor's degeneracy rule).
        let s = SExpr::List(vec![
            SExpr::Symbol("union".into()),
            SExpr::List(vec![
                SExpr::Symbol("simple".into()),
                SExpr::String("T::String".into()),
            ]),
        ]);
        let parsed = RubyType::from_sexpr(&s).expect("parse");
        assert!(matches!(parsed, RubyType::Simple(_)));
    }

    // ── RbsType ───────────────────────────────────────────────

    fn rbs_roundtrip(ty: &RbsType) {
        let s = ty.to_sexpr();
        let parsed = RbsType::from_sexpr(&s).expect("parse");
        assert_eq!(&parsed, ty);
        let emitted = s.emit();
        let reparsed =
            RbsType::from_sexpr(&SExpr::parse(&emitted).expect("lex")).expect("from-emit");
        assert_eq!(&reparsed, ty);
    }

    #[test]
    fn rbstype_named() {
        rbs_roundtrip(&RbsType::named("String"));
    }

    #[test]
    fn rbstype_untyped() {
        rbs_roundtrip(&RbsType::Untyped);
    }

    #[test]
    fn rbstype_array() {
        rbs_roundtrip(&RbsType::array(RbsType::named("Integer")));
    }

    #[test]
    fn rbstype_hash() {
        rbs_roundtrip(&RbsType::hash(RbsType::named("Symbol"), RbsType::Untyped));
    }

    #[test]
    fn rbstype_nilable() {
        rbs_roundtrip(&RbsType::nilable(RbsType::named("String")));
    }

    #[test]
    fn rbstype_string_literal() {
        rbs_roundtrip(&RbsType::string_literal("tcp"));
    }

    #[test]
    fn rbstype_union_of_string_literals() {
        rbs_roundtrip(&RbsType::union(vec![
            RbsType::string_literal("tcp"),
            RbsType::string_literal("udp"),
        ]));
    }

    #[test]
    fn rbstype_nested_array_hash_nilable() {
        rbs_roundtrip(&RbsType::nilable(RbsType::array(RbsType::hash(
            RbsType::named("Symbol"),
            RbsType::named("String"),
        ))));
    }

    #[test]
    fn rbstype_rejects_unknown_variant() {
        let err = RbsType::from_sexpr(&SExpr::Symbol("nope".into())).unwrap_err();
        assert!(matches!(err, SExprError::UnknownVariant(_)));
    }

    #[test]
    fn rbstype_hash_rejects_arity() {
        let s = SExpr::List(vec![
            SExpr::Symbol("hash".into()),
            SExpr::Symbol("untyped".into()),
        ]);
        let err = RbsType::from_sexpr(&s).unwrap_err();
        assert!(matches!(err, SExprError::Shape(_)));
    }
}
