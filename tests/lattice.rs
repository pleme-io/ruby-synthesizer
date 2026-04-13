//! Lattice property proofs for RubyType.
//!
//! The partial order is: type A <= type B iff every value A accepts, B also accepts.
//! We approximate this structurally through emit() string relationships:
//!   - optional(x) >= x        (optional wraps, accepting more values)
//!   - union(a,b) >= a, >= b   (union contains both variants)
//!   - constrained(x,c) <= x   (constrained restricts the base)

use proptest::prelude::*;
use ruby_synthesizer::RubyType;

// ── Arbitrary RubyType strategy (copied from tests/properties.rs) ───

fn arb_ruby_type() -> impl Strategy<Value = RubyType> {
    let leaf = prop_oneof![
        Just(RubyType::simple("T::String")),
        Just(RubyType::simple("T::Integer")),
        Just(RubyType::simple("T::Bool")),
        Just(RubyType::simple("T::Coercible::Float")),
        Just(RubyType::Hash),
        Just(RubyType::Any),
    ];

    leaf.prop_recursive(
        3,  // depth
        16, // max nodes
        4,  // items per collection
        |inner| {
            prop_oneof![
                inner.clone().prop_map(RubyType::array),
                inner.clone().prop_map(RubyType::optional),
                prop::collection::vec(inner.clone(), 2..=4).prop_map(RubyType::union),
                inner.prop_map(|t| RubyType::constrained(t, "included_in: ['a', 'b']")),
            ]
        },
    )
}

// ── Lattice property proofs ─────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    // ── 1. Optional widens: optional(x).emit() appends ".optional" to x.emit()
    #[test]
    fn optional_widens_by_appending_suffix(ty in arb_ruby_type()) {
        let base_emit = ty.emit();
        let opt = RubyType::optional(ty);
        let opt_emit = opt.emit();
        // If the type was already optional, idempotency collapses it.
        // Otherwise the optional emit must end with ".optional" appended to the base.
        prop_assert!(
            opt_emit.ends_with(".optional"),
            "optional({}) should end with .optional, got: {}",
            base_emit,
            opt_emit
        );
    }

    // ── 2. Optional is an upper bound: optional(x) >= x structurally
    //       Verified by: optional(x).emit() contains x.emit() as a prefix.
    #[test]
    fn optional_upper_bound_contains_base(ty in arb_ruby_type()) {
        let base_emit = ty.emit();
        let opt_emit = RubyType::optional(ty).emit();
        // The optional emit is base_emit + ".optional" (unless already optional).
        // Either way, the base string must appear as a prefix.
        prop_assert!(
            opt_emit.starts_with(&base_emit),
            "optional({}).emit() should start with base emit, got: {}",
            base_emit,
            opt_emit
        );
    }

    // ── 3. Constrained narrows: constrained(x,c).emit() starts with x.emit()
    #[test]
    fn constrained_narrows_starts_with_base(ty in arb_ruby_type()) {
        let base_emit = ty.emit();
        let constrained = RubyType::constrained(ty, "min_size?: 1");
        let c_emit = constrained.emit();
        prop_assert!(
            c_emit.starts_with(&base_emit),
            "constrained({}).emit() should start with base, got: {}",
            base_emit,
            c_emit
        );
    }

    // ── 4. Constrained is a lower bound: constrained(x,c) <= x
    //       Verified by: constrained adds ".constrained(...)" suffix to base emit.
    #[test]
    fn constrained_lower_bound_extends_base(ty in arb_ruby_type()) {
        let base_emit = ty.emit();
        let constrained = RubyType::constrained(ty, "format?: /^[a-z]+$/");
        let c_emit = constrained.emit();
        prop_assert!(
            c_emit.len() > base_emit.len(),
            "constrained emit ({}) should be strictly longer than base ({})",
            c_emit,
            base_emit
        );
    }

    // ── 5. Union contains first variant: union(a,b).emit() contains a.emit()
    #[test]
    fn union_contains_first_variant(
        a in arb_ruby_type(),
        b in arb_ruby_type(),
    ) {
        let a_emit = a.emit();
        let union = RubyType::union(vec![a, b]);
        let u_emit = union.emit();
        prop_assert!(
            u_emit.contains(&a_emit),
            "union emit should contain first variant '{}', got: {}",
            a_emit,
            u_emit
        );
    }

    // ── 6. Union contains second variant: union(a,b).emit() contains b.emit()
    #[test]
    fn union_contains_second_variant(
        a in arb_ruby_type(),
        b in arb_ruby_type(),
    ) {
        let b_emit = b.emit();
        let union = RubyType::union(vec![a, b]);
        let u_emit = union.emit();
        prop_assert!(
            u_emit.contains(&b_emit),
            "union emit should contain second variant '{}', got: {}",
            b_emit,
            u_emit
        );
    }

    // ── 7. Union is wrapped in parens (the join is parenthesized)
    #[test]
    fn union_wrapped_in_parens(
        a in arb_ruby_type(),
        b in arb_ruby_type(),
    ) {
        let union = RubyType::union(vec![a, b]);
        let u_emit = union.emit();
        prop_assert!(
            u_emit.starts_with('(') && u_emit.ends_with(')'),
            "union emit should be wrapped in parens, got: {}",
            u_emit
        );
    }

    // ── 8. Union pipe separator: union(a,b).emit() contains " | "
    #[test]
    fn union_uses_pipe_separator(
        a in arb_ruby_type(),
        b in arb_ruby_type(),
    ) {
        let union = RubyType::union(vec![a, b]);
        let u_emit = union.emit();
        prop_assert!(
            u_emit.contains(" | "),
            "union emit should contain ' | ', got: {}",
            u_emit
        );
    }

    // ── 9. Reflexivity: x.emit() == x.emit() (same element is <= itself)
    #[test]
    fn reflexivity(ty in arb_ruby_type()) {
        let a = ty.emit();
        let b = ty.emit();
        prop_assert_eq!(a, b, "emit must be reflexive (deterministic)");
    }

    // ── 10. Optional idempotence as lattice join: join(x, optional(x)) == optional(x)
    //        Since optional(x) >= x, joining again doesn't grow.
    #[test]
    fn optional_idempotent_join(ty in arb_ruby_type()) {
        let once = RubyType::optional(ty.clone());
        let twice = RubyType::optional(once.clone());
        prop_assert_eq!(
            once.emit(),
            twice.emit(),
            "optional must be idempotent (lattice join absorbed)"
        );
    }

    // ── 11. Constrained preserves structure: constrained does not alter base prefix
    //        The emit is exactly base.emit() + ".constrained(" + constraint + ")"
    #[test]
    fn constrained_preserves_base_structure(ty in arb_ruby_type()) {
        let base_emit = ty.emit();
        let constraint = "gt?: 0";
        let c = RubyType::constrained(ty, constraint);
        let c_emit = c.emit();
        let expected = format!("{}.constrained({})", base_emit, constraint);
        prop_assert_eq!(
            c_emit,
            expected,
            "constrained emit should be base + .constrained(constraint)"
        );
    }

    // ── 12. Optional of constrained: optional(constrained(x,c)) >= constrained(x,c) >= x
    //        Transitive chain: the final emit starts with x.emit().
    #[test]
    fn optional_constrained_transitive(ty in arb_ruby_type()) {
        let base_emit = ty.emit();
        let constrained = RubyType::constrained(ty, "lt?: 100");
        let opt_constrained = RubyType::optional(constrained);
        let emit = opt_constrained.emit();
        prop_assert!(
            emit.starts_with(&base_emit),
            "optional(constrained(x)).emit() should start with x.emit() '{}', got: {}",
            base_emit,
            emit
        );
    }

    // ── 13. Array preserves ordering: if x.emit() is a prefix of constrained(x).emit(),
    //        then array(x).emit() appears as a substring of array(constrained(x)).emit()
    //        (monotonicity of the array functor over the partial order).
    #[test]
    fn array_monotone_over_constrained(ty in arb_ruby_type()) {
        let base_emit = ty.emit();
        let constrained = RubyType::constrained(ty.clone(), "filled?: true");
        let c_emit = constrained.emit();
        // Verify the base relationship holds first.
        prop_assert!(c_emit.starts_with(&base_emit));
        // Now verify the array functor preserves it.
        let arr_base = RubyType::array(ty);
        let arr_constrained = RubyType::array(constrained);
        let arr_c_emit = arr_constrained.emit();
        let arr_b_emit = arr_base.emit();
        // array(constrained(x)).emit() contains the base type emit inside.
        prop_assert!(
            arr_c_emit.contains(&base_emit),
            "array(constrained(x)).emit() should contain x.emit() '{}', got: {}",
            base_emit,
            arr_c_emit
        );
        // The array of constrained is strictly longer than the array of base.
        prop_assert!(
            arr_c_emit.len() > arr_b_emit.len(),
            "array(constrained(x)).emit() should be longer than array(x).emit()"
        );
    }
}
