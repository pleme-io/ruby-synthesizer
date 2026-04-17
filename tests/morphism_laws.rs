//! Morphism composition laws — property tests for ProvenMorphism.
//!
//! Proves the categorical laws over the actual bridge implementations:
//! - Identity is a left unit: `Identity; f == f`
//! - Identity is a right unit: `f; Identity == f`
//! - Associativity: `(f; g); h == f; (g; h)` (holds because apply is pure)
//! - Proof preservation: composing two proven morphisms yields a morphism
//!   whose invariants hold on every (src, dst) pair.

use proptest::prelude::*;

use iac_forge::ir::{IacAttribute, IacType};
use iac_forge::morphism::{Composed, Identity, Morphism, ProvenMorphism};
use ruby_synthesizer::iac_bridge::{
    IacTypeToRbs, IacTypeToRuby, RbsTypeToString, RubyTypeToString,
};

// ── Arbitrary IacType strategy (copy for test isolation) ────────────

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
                prop::collection::vec("[a-z][a-z_]{1,8}", 1..5),
                inner.clone(),
            )
                .prop_map(|(values, underlying)| IacType::Enum {
                    values,
                    underlying: Box::new(underlying),
                }),
        ]
    })
}

proptest! {
    /// Identity is a right unit of Ruby composition.
    #[test]
    fn ruby_composition_right_identity(ty in arb_iac_type()) {
        let m = IacTypeToRuby;
        let composed = Composed::new(m, Identity::<_>::default());
        let direct = m.apply(&ty);
        let via_composed = composed.apply(&ty);
        prop_assert_eq!(direct.emit(), via_composed.emit());
    }

    /// Identity is a left unit of Ruby composition.
    #[test]
    fn ruby_composition_left_identity(ty in arb_iac_type()) {
        let m = IacTypeToRuby;
        let composed = Composed::new(Identity::<_>::default(), m);
        let direct = m.apply(&ty);
        let via_composed = composed.apply(&ty);
        prop_assert_eq!(direct.emit(), via_composed.emit());
    }

    /// Identity is a right unit of RBS composition.
    #[test]
    fn rbs_composition_right_identity(ty in arb_iac_type()) {
        let m = IacTypeToRbs;
        let composed = Composed::new(m, Identity::<_>::default());
        prop_assert_eq!(m.apply(&ty).emit(), composed.apply(&ty).emit());
    }

    /// Identity is a left unit of RBS composition.
    #[test]
    fn rbs_composition_left_identity(ty in arb_iac_type()) {
        let m = IacTypeToRbs;
        let composed = Composed::new(Identity::<_>::default(), m);
        prop_assert_eq!(m.apply(&ty).emit(), composed.apply(&ty).emit());
    }

    /// Associativity: (f; g); h ≡ f; (g; h) over the Ruby chain.
    /// (Because apply is pure, this is trivially true — but we prove it.)
    #[test]
    fn ruby_chain_is_associative(ty in arb_iac_type()) {
        // f: IacType → RubyType, g: RubyType → String, h: String → String (Identity)
        let left = Composed::new(
            Composed::new(IacTypeToRuby, RubyTypeToString),
            Identity::<String>::default(),
        );
        let right = Composed::new(
            IacTypeToRuby,
            Composed::new(RubyTypeToString, Identity::<String>::default()),
        );
        prop_assert_eq!(left.apply(&ty), right.apply(&ty));
    }

    /// Associativity for the RBS chain.
    #[test]
    fn rbs_chain_is_associative(ty in arb_iac_type()) {
        let left = Composed::new(
            Composed::new(IacTypeToRbs, RbsTypeToString),
            Identity::<String>::default(),
        );
        let right = Composed::new(
            IacTypeToRbs,
            Composed::new(RbsTypeToString, Identity::<String>::default()),
        );
        prop_assert_eq!(left.apply(&ty), right.apply(&ty));
    }

    /// Proof preservation: IacType → RubyType → String composed invariants
    /// always hold for any generated input.
    #[test]
    fn ruby_chain_proofs_hold(ty in arb_iac_type()) {
        let chain = Composed::new(IacTypeToRuby, RubyTypeToString);
        let out = chain.apply(&ty);
        let violations = chain.check_invariants(&ty, &out);
        prop_assert!(
            violations.is_empty(),
            "proof violation on valid input: {:?}", violations,
        );
    }

    /// Proof preservation: IacType → RbsType → String composed invariants
    /// always hold for any generated input.
    #[test]
    fn rbs_chain_proofs_hold(ty in arb_iac_type()) {
        let chain = Composed::new(IacTypeToRbs, RbsTypeToString);
        let out = chain.apply(&ty);
        let violations = chain.check_invariants(&ty, &out);
        prop_assert!(
            violations.is_empty(),
            "proof violation on valid input: {:?}", violations,
        );
    }

    /// Non-empty emission: the composed String output is always non-empty.
    #[test]
    fn ruby_chain_output_nonempty(ty in arb_iac_type()) {
        let chain = Composed::new(IacTypeToRuby, RubyTypeToString);
        prop_assert!(!chain.apply(&ty).is_empty());
    }

    #[test]
    fn rbs_chain_output_nonempty(ty in arb_iac_type()) {
        let chain = Composed::new(IacTypeToRbs, RbsTypeToString);
        prop_assert!(!chain.apply(&ty).is_empty());
    }

    /// Morphisms are deterministic: same input → same output over the chain.
    #[test]
    fn ruby_chain_is_deterministic(ty in arb_iac_type()) {
        let chain = Composed::new(IacTypeToRuby, RubyTypeToString);
        prop_assert_eq!(chain.apply(&ty), chain.apply(&ty));
    }

    #[test]
    fn rbs_chain_is_deterministic(ty in arb_iac_type()) {
        let chain = Composed::new(IacTypeToRbs, RbsTypeToString);
        prop_assert_eq!(chain.apply(&ty), chain.apply(&ty));
    }
}
