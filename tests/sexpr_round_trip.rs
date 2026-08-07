//! Property-based round-trip proofs for RubyType / RbsType sexpr impls.
//!
//! Same laws as iac-forge's proptest: direct round-trip, text round-trip
//! (emit → parse → from_sexpr), deterministic emission, printable-ASCII
//! output, balanced parens.

#![cfg(feature = "iac-bridge")]
// Every proof in this file reaches iac-forge — either `iac_forge::*`
// directly or the `iac_bridge` / sexpr impls, both of which live behind
// this feature. Without the gate the target cannot COMPILE under a plain
// `cargo test`, which is not a skipped test: it is a hard build error
// that takes the whole run down and hides every other target's result.
// Gated rather than made default because `iac-bridge` pulls the
// iac-forge dependency in, and src/sexpr.rs says that is deliberate.

use proptest::prelude::*;

use iac_forge::sexpr::{FromSExpr, SExpr, ToSExpr};
use ruby_synthesizer::{RbsType, RubyType};

// ── Strategies ──────────────────────────────────────────────────────

fn arb_ruby_type() -> impl Strategy<Value = RubyType> {
    let leaf = prop_oneof![
        "[A-Z][A-Za-z:_]{0,10}".prop_map(|n| RubyType::simple(&n)),
        Just(RubyType::Hash),
        Just(RubyType::Any),
    ];
    leaf.prop_recursive(3, 16, 4, |inner| {
        prop_oneof![
            inner.clone().prop_map(|t| RubyType::Array(Box::new(t))),
            inner.clone().prop_map(|t| RubyType::Optional(Box::new(t))),
            prop::collection::vec(inner.clone(), 2..4).prop_map(RubyType::union),
            (inner.clone(), "[a-z_]+: [^\\)]{1,20}")
                .prop_map(|(base, constraint)| RubyType::constrained(base, &constraint)),
        ]
    })
}

fn arb_rbs_type() -> impl Strategy<Value = RbsType> {
    let leaf = prop_oneof![
        "[A-Z][A-Za-z]{0,10}".prop_map(|n| RbsType::named(&n)),
        Just(RbsType::Untyped),
        "[a-z][a-z0-9-]{0,10}".prop_map(|s| RbsType::string_literal(&s)),
    ];
    leaf.prop_recursive(3, 16, 4, |inner| {
        prop_oneof![
            inner.clone().prop_map(|t| RbsType::Array(Box::new(t))),
            inner.clone().prop_map(|t| RbsType::Nilable(Box::new(t))),
            (inner.clone(), inner.clone())
                .prop_map(|(k, v)| RbsType::Hash(Box::new(k), Box::new(v))),
            prop::collection::vec(inner.clone(), 2..4).prop_map(RbsType::union),
        ]
    })
}

// ── Properties ──────────────────────────────────────────────────────

proptest! {
    #[test]
    fn rubytype_direct_roundtrip(ty in arb_ruby_type()) {
        let parsed = RubyType::from_sexpr(&ty.to_sexpr())
            .unwrap_or_else(|e| panic!("from_sexpr: {e:?}"));
        prop_assert_eq!(parsed, ty);
    }

    #[test]
    fn rubytype_text_roundtrip(ty in arb_ruby_type()) {
        let emitted = ty.to_sexpr().emit();
        let sexpr = SExpr::parse(&emitted)
            .unwrap_or_else(|e| panic!("parse: {e:?} from {emitted}"));
        let parsed = RubyType::from_sexpr(&sexpr)
            .unwrap_or_else(|e| panic!("from_sexpr: {e:?}"));
        prop_assert_eq!(parsed, ty);
    }

    #[test]
    fn rubytype_emit_is_deterministic(ty in arb_ruby_type()) {
        prop_assert_eq!(ty.to_sexpr().emit(), ty.to_sexpr().emit());
    }

    #[test]
    fn rbstype_direct_roundtrip(ty in arb_rbs_type()) {
        let parsed = RbsType::from_sexpr(&ty.to_sexpr())
            .unwrap_or_else(|e| panic!("from_sexpr: {e:?}"));
        prop_assert_eq!(parsed, ty);
    }

    #[test]
    fn rbstype_text_roundtrip(ty in arb_rbs_type()) {
        let emitted = ty.to_sexpr().emit();
        let sexpr = SExpr::parse(&emitted)
            .unwrap_or_else(|e| panic!("parse: {e:?} from {emitted}"));
        let parsed = RbsType::from_sexpr(&sexpr)
            .unwrap_or_else(|e| panic!("from_sexpr: {e:?}"));
        prop_assert_eq!(parsed, ty);
    }

    #[test]
    fn rbstype_emit_is_deterministic(ty in arb_rbs_type()) {
        prop_assert_eq!(ty.to_sexpr().emit(), ty.to_sexpr().emit());
    }

    /// Parens balance outside string literals in any RubyType emission.
    #[test]
    fn rubytype_parens_balanced(ty in arb_ruby_type()) {
        let e = ty.to_sexpr().emit();
        let mut depth = 0i64;
        let mut in_str = false;
        let mut esc = false;
        for ch in e.chars() {
            if in_str {
                if esc { esc = false; }
                else if ch == '\\' { esc = true; }
                else if ch == '"' { in_str = false; }
            } else if ch == '"' { in_str = true; }
            else if ch == '(' { depth += 1; }
            else if ch == ')' { depth -= 1; prop_assert!(depth >= 0); }
        }
        prop_assert_eq!(depth, 0);
    }

    /// Parens balance outside string literals in any RbsType emission.
    #[test]
    fn rbstype_parens_balanced(ty in arb_rbs_type()) {
        let e = ty.to_sexpr().emit();
        let mut depth = 0i64;
        let mut in_str = false;
        let mut esc = false;
        for ch in e.chars() {
            if in_str {
                if esc { esc = false; }
                else if ch == '\\' { esc = true; }
                else if ch == '"' { in_str = false; }
            } else if ch == '"' { in_str = true; }
            else if ch == '(' { depth += 1; }
            else if ch == ')' { depth -= 1; prop_assert!(depth >= 0); }
        }
        prop_assert_eq!(depth, 0);
    }
}
