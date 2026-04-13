//! Algebraic law proofs for RubyType.
//!
//! Verifies injectivity, constants, structural patterns, and output invariants
//! of the RubyType constructors and emit() function.

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

// ── Algebraic law proofs ────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    // ── 1. Injectivity of array: array(a).emit() == array(b).emit() => a.emit() == b.emit()
    #[test]
    fn array_injective(
        a in arb_ruby_type(),
        b in arb_ruby_type(),
    ) {
        let arr_a = RubyType::array(a.clone()).emit();
        let arr_b = RubyType::array(b.clone()).emit();
        if arr_a == arr_b {
            prop_assert_eq!(
                a.emit(),
                b.emit(),
                "array is not injective: array(a) == array(b) but a != b"
            );
        }
    }

    // ── 2. Injectivity of optional (modulo idempotency):
    //       optional(a).emit() == optional(b).emit() => optional(a) == optional(b)
    //       We compare normalized forms since optional is idempotent.
    #[test]
    fn optional_injective(
        a in arb_ruby_type(),
        b in arb_ruby_type(),
    ) {
        let opt_a = RubyType::optional(a);
        let opt_b = RubyType::optional(b);
        if opt_a.emit() == opt_b.emit() {
            prop_assert_eq!(
                opt_a,
                opt_b,
                "optional is not injective: optional(a).emit() == optional(b).emit() but optional(a) != optional(b)"
            );
        }
    }

    // ── 3. Injectivity of constrained (same constraint):
    //       constrained(a,c).emit() == constrained(b,c).emit() => a.emit() == b.emit()
    #[test]
    fn constrained_injective(
        a in arb_ruby_type(),
        b in arb_ruby_type(),
    ) {
        let c = "min_size?: 1";
        let ca = RubyType::constrained(a.clone(), c).emit();
        let cb = RubyType::constrained(b.clone(), c).emit();
        if ca == cb {
            prop_assert_eq!(
                a.emit(),
                b.emit(),
                "constrained is not injective: constrained(a,c) == constrained(b,c) but a != b"
            );
        }
    }

    // ── 4. Injectivity of simple: simple(a) == simple(b) => a == b
    #[test]
    fn simple_injective(
        a in "[A-Z][a-z]{2,8}",
        b in "[A-Z][a-z]{2,8}",
    ) {
        let sa = RubyType::simple(&a).emit();
        let sb = RubyType::simple(&b).emit();
        if sa == sb {
            prop_assert_eq!(a, b, "simple is not injective");
        }
    }

    // ── 5. Hash constant: Hash.emit() is always "T::Hash"
    #[test]
    fn hash_constant(_dummy in 0..100u32) {
        prop_assert_eq!(RubyType::Hash.emit(), "T::Hash");
    }

    // ── 6. Any constant: Any.emit() is always "T::Any"
    #[test]
    fn any_constant(_dummy in 0..100u32) {
        prop_assert_eq!(RubyType::Any.emit(), "T::Any");
    }

    // ── 7. Array structural pattern: array(x).emit() starts with "T::Array.of("
    #[test]
    fn array_starts_with_prefix(ty in arb_ruby_type()) {
        let emit = RubyType::array(ty).emit();
        prop_assert!(
            emit.starts_with("T::Array.of("),
            "array emit should start with 'T::Array.of(', got: {}",
            emit
        );
    }

    // ── 8. Array structural pattern: array(x).emit() ends with ")"
    #[test]
    fn array_ends_with_close_paren(ty in arb_ruby_type()) {
        let emit = RubyType::array(ty).emit();
        prop_assert!(
            emit.ends_with(')'),
            "array emit should end with ')', got: {}",
            emit
        );
    }

    // ── 9. Optional structural pattern: optional(x).emit() ends with ".optional"
    #[test]
    fn optional_ends_with_suffix(ty in arb_ruby_type()) {
        let emit = RubyType::optional(ty).emit();
        prop_assert!(
            emit.ends_with(".optional"),
            "optional emit should end with '.optional', got: {}",
            emit
        );
    }

    // ── 10. Constrained structural pattern: constrained(x,c).emit() contains ".constrained("
    #[test]
    fn constrained_contains_marker(ty in arb_ruby_type()) {
        let emit = RubyType::constrained(ty, "gt?: 0").emit();
        prop_assert!(
            emit.contains(".constrained("),
            "constrained emit should contain '.constrained(', got: {}",
            emit
        );
    }

    // ── 11. No newlines in any type emit
    #[test]
    fn emit_no_newlines(ty in arb_ruby_type()) {
        let emit = ty.emit();
        prop_assert!(
            !emit.contains('\n') && !emit.contains('\r'),
            "emit should not contain newlines, got: {:?}",
            emit
        );
    }

    // ── 12. All emits are printable ASCII (bytes 0x20..=0x7E)
    #[test]
    fn emit_printable_ascii(ty in arb_ruby_type()) {
        let emit = ty.emit();
        for (i, byte) in emit.bytes().enumerate() {
            prop_assert!(
                (0x20..=0x7E).contains(&byte),
                "non-printable ASCII byte 0x{:02x} at position {} in: {}",
                byte,
                i,
                emit
            );
        }
    }

    // ── 13. Union emit structure: union of N variants has exactly N-1 top-level " | " separators.
    //        We count only pipes at paren depth 1 (the outermost union wrapper).
    #[test]
    fn union_separator_count(variants in prop::collection::vec(arb_ruby_type(), 2..=4)) {
        let n = variants.len();
        let union = RubyType::union(variants);
        let emit = union.emit();
        // Count " | " occurrences at depth 1 (inside the outermost parens only).
        let mut depth: i32 = 0;
        let chars: Vec<char> = emit.chars().collect();
        let mut top_level_pipes = 0usize;
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '(' => depth += 1,
                ')' => depth -= 1,
                ' ' if depth == 1 => {
                    // Check for " | " at this position.
                    if i + 2 < chars.len() && chars[i + 1] == '|' && chars[i + 2] == ' ' {
                        top_level_pipes += 1;
                        i += 3;
                        continue;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        prop_assert_eq!(
            top_level_pipes,
            n - 1,
            "union of {} variants should have {} top-level pipes, got {} in: {}",
            n,
            n - 1,
            top_level_pipes,
            emit
        );
    }

    // ── 14. Emit is non-empty for all types
    #[test]
    fn emit_nonempty(ty in arb_ruby_type()) {
        let emit = ty.emit();
        prop_assert!(!emit.is_empty(), "emit should never be empty");
    }
}
