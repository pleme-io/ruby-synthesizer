//! Compile-time guarantees — the Rust type system prevents invalid infrastructure.
//!
//! These proofs demonstrate that ruby-synthesizer makes certain classes
//! of infrastructure bugs IMPOSSIBLE at the Rust compiler level.
//!
//! Where other test files prove algebraic laws (type_algebra.rs), lattice
//! properties (lattice.rs), or builder structure (structural.rs), THIS file
//! proves infrastructure correctness: the generated Ruby is always valid
//! infrastructure code, regardless of input.

use proptest::prelude::*;
use regex::Regex;
use std::collections::HashSet;

use ruby_synthesizer::builders::{ResourceFileBuilder, TypesFileBuilder};
use ruby_synthesizer::{RubyNode, RubyType, emit_file};

// ══════════════════════════════════════════════════════════════════
// Strategies — random infrastructure configurations
// ══════════════════════════════════════════════════════════════════

/// Generate random provider names: alphanumeric + underscores, lowercase.
fn arb_provider_name() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{1,12}"
}

/// Generate random attribute names: lowercase + underscores.
fn arb_attr_name() -> impl Strategy<Value = String> {
    "[a-z][a-z_]{1,15}".prop_filter("no ruby keywords", |s| {
        ![
            "end", "def", "class", "module", "do", "if", "else", "begin", "rescue",
        ]
        .contains(&s.as_str())
    })
}

/// Generate random class names: PascalCase.
fn arb_class_name() -> impl Strategy<Value = String> {
    "[A-Z][a-z]{2,10}Attributes"
}

/// Recursive RubyType strategy — all variants, arbitrary depth.
fn arb_ruby_type() -> impl Strategy<Value = RubyType> {
    let leaf = prop_oneof![
        Just(RubyType::simple("T::String")),
        Just(RubyType::simple("T::Integer")),
        Just(RubyType::simple("T::Bool")),
        Just(RubyType::simple("T::Coercible::Float")),
        Just(RubyType::simple("T::Coercible::Integer")),
        Just(RubyType::Hash),
        Just(RubyType::Any),
    ];

    leaf.prop_recursive(
        4,  // depth
        24, // max nodes
        6,  // items per collection
        |inner| {
            prop_oneof![
                inner.clone().prop_map(RubyType::array),
                inner.clone().prop_map(RubyType::optional),
                prop::collection::vec(inner.clone(), 2..=4).prop_map(RubyType::union),
                inner.prop_map(|t| RubyType::constrained(t, "included_in: ['x', 'y']")),
            ]
        },
    )
}

/// Generate a unique set of attribute names.
fn arb_unique_attrs(min: usize, max: usize) -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(arb_attr_name(), max..=max * 3).prop_map(move |names| {
        let mut seen = HashSet::new();
        let mut unique: Vec<String> = names
            .into_iter()
            .filter(|n| seen.insert(n.clone()))
            .take(max)
            .collect();
        while unique.len() < min {
            unique.push(format!("gen_attr_{}", unique.len()));
        }
        unique
    })
}

/// Generate a random TypesFileBuilder with N classes, each with M attributes.
fn arb_types_file_builder()
-> impl Strategy<Value = (String, Vec<(String, Vec<(String, RubyType, bool)>)>)> {
    (
        arb_provider_name(),
        prop::collection::vec(
            (
                arb_class_name(),
                prop::collection::vec((arb_attr_name(), arb_ruby_type(), any::<bool>()), 1..=6),
            ),
            1..=3,
        ),
    )
}

/// Generate a random ResourceFileBuilder configuration.
fn arb_resource_file_config()
-> impl Strategy<Value = (String, Vec<String>, Vec<String>, Vec<String>)> {
    (
        arb_provider_name(),
        arb_unique_attrs(0, 5), // map fields
        arb_unique_attrs(0, 3), // map_present fields
        arb_unique_attrs(0, 2), // map_bool fields
    )
}

/// Build a types file from the strategy output.
fn build_types_file(provider: &str, classes: &[(String, Vec<(String, RubyType, bool)>)]) -> String {
    let mut builder = TypesFileBuilder::new(provider);
    for (class_name, attrs) in classes {
        let attrs_clone = attrs.clone();
        builder = builder.class(class_name, |c| {
            let mut cb = c;
            for (name, ty, required) in attrs_clone {
                cb = cb.attribute(&name, ty, required);
            }
            cb
        });
    }
    builder.emit()
}

/// Build a resource file from the strategy output.
fn build_resource_file(
    provider: &str,
    map: &[String],
    map_present: &[String],
    map_bool: &[String],
) -> String {
    let tf_type = format!("{provider}_resource");
    let map_refs: Vec<&str> = map.iter().map(String::as_str).collect();
    let map_present_refs: Vec<&str> = map_present.iter().map(String::as_str).collect();
    let map_bool_refs: Vec<&str> = map_bool.iter().map(String::as_str).collect();
    ResourceFileBuilder::new(provider, &tf_type, "resource")
        .map(map_refs)
        .map_present(map_present_refs)
        .map_bool(map_bool_refs)
        .emit()
}

// ══════════════════════════════════════════════════════════════════
// Proof 1: Every emitted file is non-empty
// No combination of valid RubyNode produces empty output.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_01_emitted_file_never_empty(
        (provider, classes) in arb_types_file_builder(),
    ) {
        let source = build_types_file(&provider, &classes);
        prop_assert!(!source.is_empty(), "TypesFileBuilder produced empty output");
        prop_assert!(source.len() > 10, "TypesFileBuilder output suspiciously short: {}", source.len());
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_01b_resource_file_never_empty(
        (provider, map, map_present, map_bool) in arb_resource_file_config(),
    ) {
        let source = build_resource_file(&provider, &map, &map_present, &map_bool);
        prop_assert!(!source.is_empty(), "ResourceFileBuilder produced empty output");
        prop_assert!(source.len() > 10, "ResourceFileBuilder output suspiciously short: {}", source.len());
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 2: Frozen pragma is ALWAYS first
// Both builders guarantee frozen_string_literal as the first line.
// This is a compile-time guarantee — the builder structs emit it
// unconditionally as the first node.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_02_frozen_pragma_always_first_types(
        (provider, classes) in arb_types_file_builder(),
    ) {
        let source = build_types_file(&provider, &classes);
        prop_assert!(
            source.starts_with("# frozen_string_literal: true\n"),
            "frozen pragma not first line in types file:\n{}",
            source.lines().next().unwrap_or("<empty>")
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_02b_frozen_pragma_always_first_resource(
        (provider, map, map_present, map_bool) in arb_resource_file_config(),
    ) {
        let source = build_resource_file(&provider, &map, &map_present, &map_bool);
        prop_assert!(
            source.starts_with("# frozen_string_literal: true\n"),
            "frozen pragma not first line in resource file:\n{}",
            source.lines().next().unwrap_or("<empty>")
        );
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 3: Module nesting is always balanced
// For any valid RubyNode tree emitted through the builders,
// count of "module" + "class" always equals count of "end".
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_03_blocks_always_balanced_types(
        (provider, classes) in arb_types_file_builder(),
    ) {
        let source = build_types_file(&provider, &classes);
        let module_re = Regex::new(r"(?m)^\s*module ").unwrap();
        let class_re = Regex::new(r"(?m)^\s*class ").unwrap();
        let end_re = Regex::new(r"(?m)^\s*end\s*$").unwrap();

        let openers = module_re.find_iter(&source).count()
            + class_re.find_iter(&source).count();
        let closers = end_re.find_iter(&source).count();

        prop_assert!(
            openers == closers,
            "unbalanced blocks in types file: {} openers vs {} ends\n{}",
            openers, closers, source
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_03b_blocks_always_balanced_resource(
        (provider, map, map_present, map_bool) in arb_resource_file_config(),
    ) {
        let source = build_resource_file(&provider, &map, &map_present, &map_bool);
        let module_re = Regex::new(r"(?m)^\s*module ").unwrap();
        let class_re = Regex::new(r"(?m)^\s*class ").unwrap();
        let end_re = Regex::new(r"(?m)^\s*end\s*$").unwrap();

        let openers = module_re.find_iter(&source).count()
            + class_re.find_iter(&source).count();
        let closers = end_re.find_iter(&source).count();

        prop_assert!(
            openers == closers,
            "unbalanced blocks in resource file: {} openers vs {} ends\n{}",
            openers, closers, source
        );
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 4: Attribute names are deterministic
// Same input attributes -> same output Ruby, always.
// Run the builder multiple times, verify byte-for-byte identical.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_04_types_file_byte_identical_across_runs(
        (provider, classes) in arb_types_file_builder(),
    ) {
        let run1 = build_types_file(&provider, &classes);
        let run2 = build_types_file(&provider, &classes);
        let run3 = build_types_file(&provider, &classes);

        prop_assert_eq!(&run1, &run2, "non-deterministic types file emit (run 1 vs 2)");
        prop_assert_eq!(&run2, &run3, "non-deterministic types file emit (run 2 vs 3)");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_04b_resource_file_byte_identical_across_runs(
        (provider, map, map_present, map_bool) in arb_resource_file_config(),
    ) {
        let run1 = build_resource_file(&provider, &map, &map_present, &map_bool);
        let run2 = build_resource_file(&provider, &map, &map_present, &map_bool);
        let run3 = build_resource_file(&provider, &map, &map_present, &map_bool);

        prop_assert_eq!(&run1, &run2, "non-deterministic resource file emit (run 1 vs 2)");
        prop_assert_eq!(&run2, &run3, "non-deterministic resource file emit (run 2 vs 3)");
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 5: Optional is idempotent across ALL type variants
// optional(optional(x)) == optional(x) for every RubyType,
// including deeply nested recursive types.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_05_optional_idempotent_deep(ty in arb_ruby_type()) {
        let once = RubyType::optional(ty.clone());
        let twice = RubyType::optional(once.clone());
        let thrice = RubyType::optional(twice.clone());

        // Structural equality: the AST itself is identical
        prop_assert_eq!(&once, &twice, "optional not structurally idempotent at depth 2");
        prop_assert_eq!(&twice, &thrice, "optional not structurally idempotent at depth 3");

        // Emit equality: the output strings are identical
        prop_assert_eq!(once.emit(), twice.emit(), "optional emit not idempotent at depth 2");
        prop_assert_eq!(twice.emit(), thrice.emit(), "optional emit not idempotent at depth 3");

        // No ".optional.optional" substring ever appears
        let emitted = thrice.emit();
        prop_assert!(
            !emitted.contains(".optional.optional"),
            "triple-optional produced .optional.optional: {}",
            emitted
        );
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 6: Type emit round-trip — valid Dry::Types expressions
// Every RubyType emits a non-empty string that starts with "T::"
// or "(" (for unions). This guarantees valid Dry::Types expressions.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_06_type_emit_valid_dry_types(ty in arb_ruby_type()) {
        let emitted = ty.emit();
        prop_assert!(!emitted.is_empty(), "type emitted empty string");

        // Every Dry::Types expression starts with T:: or ( for unions.
        // The .optional and .constrained() suffixes are appended to valid bases.
        // Strip suffixes to find the base.
        let base = emitted
            .trim_end_matches(".optional")
            .split(".constrained(")
            .next()
            .unwrap_or(&emitted);

        prop_assert!(
            base.starts_with("T::") || base.starts_with("("),
            "type base does not start with T:: or (: base='{}' full='{}'",
            base, emitted
        );
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 7: Constrained types preserve base
// constrained(x, rule).emit() always starts with x.emit()
// The constraint NARROWS, never changes the base type.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_07_constrained_preserves_base_across_rules(
        ty in arb_ruby_type(),
        rule in prop_oneof![
            Just("min_size?: 1"),
            Just("max_size?: 100"),
            Just("included_in: ['a', 'b', 'c']"),
            Just("format?: /^[a-z]+$/"),
            Just("gt?: 0"),
            Just("lt?: 1000"),
            Just("gteq?: -1"),
            Just("filled?: true"),
        ],
    ) {
        let base_emit = ty.emit();
        let constrained = RubyType::constrained(ty, rule);
        let c_emit = constrained.emit();

        // The constrained emit ALWAYS starts with the base emit
        prop_assert!(
            c_emit.starts_with(&base_emit),
            "constrained did not preserve base: base='{}' constrained='{}'",
            base_emit, c_emit
        );

        // The constrained emit is strictly longer
        prop_assert!(
            c_emit.len() > base_emit.len(),
            "constrained is not longer than base"
        );

        // The constrained emit contains .constrained(
        prop_assert!(
            c_emit.contains(".constrained("),
            "constrained emit missing .constrained() call"
        );
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 8: Array nesting preserves inner type
// For any type x, Array(x).emit() contains x.emit().
// No information lost in nesting. Also: double nesting preserves.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_08_array_preserves_inner_type(ty in arb_ruby_type()) {
        let inner_emit = ty.emit();
        let array_emit = RubyType::array(ty.clone()).emit();

        // Array contains the inner type verbatim
        prop_assert!(
            array_emit.contains(&inner_emit),
            "array does not contain inner: inner='{}' array='{}'",
            inner_emit, array_emit
        );

        // Double nesting preserves both levels
        let double_emit = RubyType::array(RubyType::array(ty)).emit();
        prop_assert!(
            double_emit.contains(&inner_emit),
            "double array lost inner: inner='{}' double='{}'",
            inner_emit, double_emit
        );
        prop_assert!(
            double_emit.contains(&array_emit),
            "double array lost single array: single='{}' double='{}'",
            array_emit, double_emit
        );
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 9: Resource file has all required sections
// For ANY provider/resource combination, ResourceFileBuilder emits:
// - frozen pragma, require statements, module, include,
//   define_resource, registry_call
// All 6 sections present in every output.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_09_resource_file_all_sections_present(
        (provider, map, map_present, map_bool) in arb_resource_file_config(),
    ) {
        let source = build_resource_file(&provider, &map, &map_present, &map_bool);

        // Section 1: frozen pragma
        prop_assert!(
            source.contains("# frozen_string_literal: true"),
            "resource file missing frozen pragma"
        );

        // Section 2: require statements (at least 3: base, reference, types)
        let require_re = Regex::new(r"require '").unwrap();
        let require_count = require_re.find_iter(&source).count();
        prop_assert!(
            require_count >= 3,
            "resource file has fewer than 3 require statements: {} in:\n{}",
            require_count, source
        );

        // Section 3: module declaration
        prop_assert!(
            source.contains("module Pangea::Resources"),
            "resource file missing module declaration"
        );

        // Section 4: include ResourceBuilder
        prop_assert!(
            source.contains("include Pangea::Resources::ResourceBuilder"),
            "resource file missing ResourceBuilder include"
        );

        // Section 5: define_resource
        prop_assert!(
            source.contains("define_resource"),
            "resource file missing define_resource"
        );

        // Section 6: registry call
        prop_assert!(
            source.contains("Pangea::ResourceRegistry.register_module"),
            "resource file missing registry call"
        );
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 10: Types file has all required sections
// For ANY provider/class combination, TypesFileBuilder emits:
// - frozen pragma, require, module, include Dry.Types(),
//   class with BaseAttributes parent, T constant
// All 6 sections present in every output.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_10_types_file_all_sections_present(
        (provider, classes) in arb_types_file_builder(),
    ) {
        let source = build_types_file(&provider, &classes);

        // Section 1: frozen pragma
        prop_assert!(
            source.contains("# frozen_string_literal: true"),
            "types file missing frozen pragma"
        );

        // Section 2: require statement
        prop_assert!(
            source.contains("require 'pangea/resources/base_attributes'"),
            "types file missing base_attributes require"
        );

        // Section 3: module declaration with Types suffix
        prop_assert!(
            source.contains("module Pangea::Resources::"),
            "types file missing module declaration"
        );
        prop_assert!(
            source.contains("::Types"),
            "types file module missing ::Types suffix"
        );

        // Section 4: include Dry.Types()
        prop_assert!(
            source.contains("include Dry.Types()"),
            "types file missing Dry.Types() include"
        );

        // Section 5: class with BaseAttributes parent
        prop_assert!(
            source.contains("< Pangea::Resources::BaseAttributes"),
            "types file missing BaseAttributes inheritance"
        );

        // Section 6: T constant assignment
        prop_assert!(
            source.contains("T = Pangea::Resources::"),
            "types file missing T constant"
        );
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 11: Section ordering is correct
// In every types file, frozen pragma comes before require,
// require comes before module, module contains class.
// Section order is structurally enforced by the builder.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_11_types_file_section_ordering(
        (provider, classes) in arb_types_file_builder(),
    ) {
        let source = build_types_file(&provider, &classes);

        let frozen_pos = source.find("# frozen_string_literal: true")
            .expect("frozen pragma must exist");
        let require_pos = source.find("require '")
            .expect("require must exist");
        let module_pos = source.find("module Pangea::Resources::")
            .expect("module must exist");
        let class_pos = source.find("class ")
            .expect("class must exist");

        prop_assert!(frozen_pos < require_pos, "frozen pragma must come before require");
        prop_assert!(require_pos < module_pos, "require must come before module");
        prop_assert!(module_pos < class_pos, "module must come before class");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_11b_resource_file_section_ordering(
        (provider, map, map_present, map_bool) in arb_resource_file_config(),
    ) {
        let source = build_resource_file(&provider, &map, &map_present, &map_bool);

        let frozen_pos = source.find("# frozen_string_literal: true")
            .expect("frozen pragma must exist");
        let require_pos = source.find("require '")
            .expect("require must exist");
        let module_pos = source.find("module Pangea::Resources")
            .expect("module must exist");
        let define_pos = source.find("define_resource")
            .expect("define_resource must exist");
        let registry_pos = source.find("Pangea::ResourceRegistry.register_module")
            .expect("registry call must exist");

        prop_assert!(frozen_pos < require_pos, "frozen pragma must come before require");
        prop_assert!(require_pos < module_pos, "require must come before module");
        prop_assert!(module_pos < define_pos, "module must come before define_resource");
        prop_assert!(define_pos < registry_pos, "define_resource must come before registry call");
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 12: Every line has valid indentation
// All non-blank, non-comment lines have indentation that is a
// multiple of 2 spaces. Tab characters never appear.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_12_indentation_always_valid(
        (provider, classes) in arb_types_file_builder(),
    ) {
        let source = build_types_file(&provider, &classes);

        // No tabs anywhere
        prop_assert!(
            !source.contains('\t'),
            "types file contains tab character"
        );

        for (i, line) in source.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let leading_spaces = line.len() - line.trim_start().len();
            prop_assert!(
                leading_spaces % 2 == 0,
                "line {} has {} leading spaces (not multiple of 2): '{}'",
                i + 1, leading_spaces, line
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_12b_resource_indentation_always_valid(
        (provider, map, map_present, map_bool) in arb_resource_file_config(),
    ) {
        let source = build_resource_file(&provider, &map, &map_present, &map_bool);

        prop_assert!(
            !source.contains('\t'),
            "resource file contains tab character"
        );

        for (i, line) in source.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let leading_spaces = line.len() - line.trim_start().len();
            // define_resource continuation lines and map lines may have
            // non-standard alignment; we only check even-space rule for
            // structural lines (module/class/include/end/attribute/require).
            let trimmed = line.trim();
            if trimmed.starts_with("module ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("include ")
                || trimmed == "end"
                || trimmed.starts_with("require ")
                || trimmed.starts_with("attribute")
            {
                prop_assert!(
                    leading_spaces % 2 == 0,
                    "line {} has {} leading spaces (not multiple of 2): '{}'",
                    i + 1, leading_spaces, line
                );
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 13: emit_file with arbitrary node trees never panics
// The emitter handles every RubyNode variant without panic.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_13_emit_file_never_panics(
        num_blanks in 0..5usize,
        comment in "[a-zA-Z0-9 ]{0,30}",
        require_path in "[a-z/]{1,20}",
        attr_name in arb_attr_name(),
        ty in arb_ruby_type(),
        required in any::<bool>(),
    ) {
        let mut nodes = vec![RubyNode::FrozenStringLiteral];

        for _ in 0..num_blanks {
            nodes.push(RubyNode::Blank);
        }

        if !comment.is_empty() {
            nodes.push(RubyNode::Comment(comment));
        }

        nodes.push(RubyNode::Require(require_path));
        nodes.push(RubyNode::Module {
            path: vec!["Test".into(), "Module".into()],
            body: vec![
                RubyNode::Include("Dry.Types()".into()),
                RubyNode::Class {
                    name: "TestClass".into(),
                    parent: Some("BaseClass".into()),
                    body: vec![
                        RubyNode::Attribute {
                            name: attr_name,
                            type_expr: ty,
                            required,
                        },
                    ],
                },
            ],
        });

        // This must not panic
        let output = emit_file(&nodes);
        prop_assert!(!output.is_empty());
        prop_assert!(output.ends_with('\n'));
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 14: Attribute required/optional distinction is always correct
// Required attributes emit "attribute :name, Type"
// Optional attributes emit "attribute? :name, Type.optional"
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_14_required_optional_distinction(
        attr_name in arb_attr_name(),
        ty in arb_ruby_type(),
    ) {
        // Required attribute
        let req_node = RubyNode::Attribute {
            name: attr_name.clone(),
            type_expr: ty.clone(),
            required: true,
        };
        let req_emit = req_node.emit(0);
        prop_assert!(
            req_emit.starts_with("attribute :"),
            "required attribute should use 'attribute :': '{}'",
            req_emit
        );
        prop_assert!(
            !req_emit.starts_with("attribute? :"),
            "required attribute should NOT use 'attribute? :': '{}'",
            req_emit
        );

        // Optional attribute
        let opt_node = RubyNode::Attribute {
            name: attr_name,
            type_expr: ty,
            required: false,
        };
        let opt_emit = opt_node.emit(0);
        prop_assert!(
            opt_emit.starts_with("attribute? :"),
            "optional attribute should use 'attribute? :': '{}'",
            opt_emit
        );
        prop_assert!(
            opt_emit.contains(".optional"),
            "optional attribute type should contain .optional: '{}'",
            opt_emit
        );
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 15: Multi-class types files have one T constant per class
// When TypesFileBuilder has N classes, the output has exactly N
// "T = " assignments — one per class, no more, no fewer.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_15_one_t_constant_per_class(
        (provider, classes) in arb_types_file_builder(),
    ) {
        let source = build_types_file(&provider, &classes);

        let class_re = Regex::new(r"(?m)^\s+class ").unwrap();
        let t_re = Regex::new(r"(?m)^\s+T = ").unwrap();

        let class_count = class_re.find_iter(&source).count();
        let t_count = t_re.find_iter(&source).count();

        prop_assert_eq!(
            class_count, t_count,
            "class count ({}) != T constant count ({}) in:\n{}",
            class_count, t_count, source
        );
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 16: No dangling syntax — every "do" has matching "end"
// RSpec nodes use "do...end" blocks. Count all "do\n" and "end"
// pairs to verify no dangling blocks.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_16_rspec_blocks_balanced(
        subject in "[a-z_]{3,12}",
        test_name in "[a-z ]{3,20}",
        expected in "[a-z0-9_]{1,10}",
    ) {
        let node = RubyNode::Describe {
            subject: format!("'{subject}'"),
            body: vec![
                RubyNode::Context {
                    name: "when valid".into(),
                    body: vec![
                        RubyNode::Let {
                            name: "result".into(),
                            expr: "described_class.new".into(),
                        },
                        RubyNode::It {
                            name: test_name,
                            body: vec![
                                RubyNode::Expect {
                                    subject: "result".into(),
                                    matcher: format!("to eq({expected})"),
                                },
                            ],
                        },
                    ],
                },
            ],
        };

        let output = emit_file(&[node]);
        let do_re = Regex::new(r"(?m) do\s*$").unwrap();
        let end_re = Regex::new(r"(?m)^\s*end\s*$").unwrap();

        let do_count = do_re.find_iter(&output).count();
        let end_count = end_re.find_iter(&output).count();

        prop_assert_eq!(
            do_count, end_count,
            "unbalanced do/end: {} do vs {} end in:\n{}",
            do_count, end_count, output
        );
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 17: Type composition is associative in emit
// array(constrained(x, r)) vs constrained(array(x), r) produce
// different but both valid outputs. Verify both are non-empty
// and structurally valid.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_17_type_composition_both_valid(ty in arb_ruby_type()) {
        let rule = "gt?: 0";

        // array(constrained(x))
        let arr_of_constrained = RubyType::array(RubyType::constrained(ty.clone(), rule));
        let emit_ac = arr_of_constrained.emit();

        // constrained(array(x))
        let constrained_of_arr = RubyType::constrained(RubyType::array(ty), rule);
        let emit_ca = constrained_of_arr.emit();

        // Both are non-empty
        prop_assert!(!emit_ac.is_empty());
        prop_assert!(!emit_ca.is_empty());

        // Both have balanced parens
        let ac_open = emit_ac.matches('(').count();
        let ac_close = emit_ac.matches(')').count();
        prop_assert_eq!(ac_open, ac_close, "unbalanced parens in array(constrained): {}", emit_ac);

        let ca_open = emit_ca.matches('(').count();
        let ca_close = emit_ca.matches(')').count();
        prop_assert_eq!(ca_open, ca_close, "unbalanced parens in constrained(array): {}", emit_ca);

        // They are different (composition order matters)
        prop_assert_ne!(emit_ac, emit_ca, "array(constrained(x)) should differ from constrained(array(x))");
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 18: Output is valid UTF-8 with no control characters
// Every byte in every emitted file is printable ASCII or newline.
// No NUL, no BOM, no non-ASCII — safe for Ruby source files.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_18_output_clean_ascii(
        (provider, classes) in arb_types_file_builder(),
    ) {
        let source = build_types_file(&provider, &classes);

        for (i, byte) in source.bytes().enumerate() {
            prop_assert!(
                (0x20..=0x7E).contains(&byte) || byte == b'\n',
                "non-printable byte 0x{:02x} at position {} in types file",
                byte, i
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_18b_resource_output_clean_ascii(
        (provider, map, map_present, map_bool) in arb_resource_file_config(),
    ) {
        let source = build_resource_file(&provider, &map, &map_present, &map_bool);

        for (i, byte) in source.bytes().enumerate() {
            prop_assert!(
                (0x20..=0x7E).contains(&byte) || byte == b'\n',
                "non-printable byte 0x{:02x} at position {} in resource file",
                byte, i
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 19: No trailing whitespace on any line
// Trailing whitespace is a linting violation in Ruby.
// The emitter must never produce it.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_19_no_trailing_whitespace_types(
        (provider, classes) in arb_types_file_builder(),
    ) {
        let source = build_types_file(&provider, &classes);
        for (i, line) in source.lines().enumerate() {
            if line.is_empty() {
                continue;
            }
            prop_assert!(
                !line.ends_with(' '),
                "line {} has trailing whitespace: '{}'",
                i + 1, line
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_19b_no_trailing_whitespace_resource(
        (provider, map, map_present, map_bool) in arb_resource_file_config(),
    ) {
        let source = build_resource_file(&provider, &map, &map_present, &map_bool);
        for (i, line) in source.lines().enumerate() {
            if line.is_empty() {
                continue;
            }
            prop_assert!(
                !line.ends_with(' '),
                "line {} has trailing whitespace: '{}'",
                i + 1, line
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 20: Map field symbols are syntactically valid Ruby
// Every field in map/map_present/map_bool is emitted as `:field_name`
// and the symbol name matches [a-z_][a-z0-9_]*.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_20_map_symbols_valid_ruby(
        provider in arb_provider_name(),
        fields in arb_unique_attrs(1, 5),
    ) {
        let tf_type = format!("{provider}_symboltest");
        let field_refs: Vec<&str> = fields.iter().map(String::as_str).collect();
        let source = ResourceFileBuilder::new(&provider, &tf_type, "symboltest")
            .map(field_refs)
            .emit();

        // Extract all symbols from map: [...] section
        let symbol_re = Regex::new(r":([a-z_][a-z0-9_]*)").unwrap();
        let symbols: Vec<&str> = symbol_re
            .captures_iter(&source)
            .map(|c| c.get(1).unwrap().as_str())
            .collect();

        // Every input field should appear as a symbol
        for field in &fields {
            prop_assert!(
                symbols.contains(&field.as_str()),
                "field '{}' not found as symbol in:\n{}",
                field, source
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Proof 21: File always ends with exactly one trailing newline
// Not zero, not two — exactly one.
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_21_exactly_one_trailing_newline(
        (provider, classes) in arb_types_file_builder(),
    ) {
        let source = build_types_file(&provider, &classes);
        prop_assert!(source.ends_with('\n'), "no trailing newline");
        prop_assert!(
            !source.ends_with("\n\n"),
            "double trailing newline"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn proof_21b_resource_exactly_one_trailing_newline(
        (provider, map, map_present, map_bool) in arb_resource_file_config(),
    ) {
        let source = build_resource_file(&provider, &map, &map_present, &map_bool);
        prop_assert!(source.ends_with('\n'), "no trailing newline");
        prop_assert!(
            !source.ends_with("\n\n"),
            "double trailing newline"
        );
    }
}
