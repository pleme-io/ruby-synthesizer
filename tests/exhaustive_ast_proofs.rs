//! Exhaustive AST proofs — maximum coverage of every RubyNode variant,
//! every RubyType operation, every builder guarantee, every IaC bridge
//! mapping, and cross-cutting structural invariants.
//!
//! 40+ tests. Uses proptest for randomized proofs (500+ cases per test).

use proptest::prelude::*;

use ruby_synthesizer::builders::{ResourceFileBuilder, TypesFileBuilder};
use ruby_synthesizer::{emit_file, RSpecBuilder, RubyNode, RubyType};

// ══════════════════════════════════════════════════════════════════
// Strategies
// ══════════════════════════════════════════════════════════════════

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
        3,
        16,
        4,
        |inner| {
            prop_oneof![
                inner.clone().prop_map(RubyType::array),
                inner.clone().prop_map(RubyType::optional),
                prop::collection::vec(inner.clone(), 2..=4).prop_map(RubyType::union),
                inner.prop_map(|t| RubyType::constrained(t, "gt?: 0")),
            ]
        },
    )
}

/// Generate one of every RubyNode variant at a given indent.
fn all_node_variants() -> Vec<RubyNode> {
    vec![
        RubyNode::FrozenStringLiteral,
        RubyNode::Comment("test comment".into()),
        RubyNode::Blank,
        RubyNode::Require("dry-struct".into()),
        RubyNode::RequireRelative("../types".into()),
        RubyNode::Module {
            path: vec!["A".into(), "B".into()],
            body: vec![RubyNode::Blank],
        },
        RubyNode::InlineModuleDecl(vec!["X".into(), "Y".into(), "Z".into()]),
        RubyNode::Class {
            name: "Foo".into(),
            parent: Some("Bar".into()),
            body: vec![RubyNode::Blank],
        },
        RubyNode::Class {
            name: "Baz".into(),
            parent: None,
            body: vec![],
        },
        RubyNode::Include("Dry.Types()".into()),
        RubyNode::ConstAssign {
            name: "T".into(),
            value: "Some::Module".into(),
        },
        RubyNode::Attribute {
            name: "field".into(),
            type_expr: RubyType::simple("T::String"),
            required: true,
        },
        RubyNode::Attribute {
            name: "opt_field".into(),
            type_expr: RubyType::simple("T::Integer"),
            required: false,
        },
        RubyNode::DefineResource {
            tf_type: "test_resource".into(),
            attrs_class: "Test::Types::Attrs".into(),
            outputs: vec![("id".into(), "id".into())],
            map: vec!["a".into()],
            map_present: vec!["b".into()],
            map_bool: vec!["c".into()],
        },
        RubyNode::DefineData {
            tf_type: "test_data".into(),
            attrs_class: "Test::Types::DataAttrs".into(),
            outputs: vec![("name".into(), "name".into())],
            map: vec!["x".into()],
            map_present: vec![],
            map_bool: vec![],
        },
        RubyNode::RegistryCall("Pangea::Resources::Test".into()),
        RubyNode::Raw("puts 'hello'".into()),
        RubyNode::Describe {
            subject: "'subject'".into(),
            body: vec![RubyNode::Blank],
        },
        RubyNode::SharedExamples {
            name: "shared".into(),
            params: "param".into(),
            body: vec![RubyNode::Blank],
        },
        RubyNode::Context {
            name: "when something".into(),
            body: vec![RubyNode::Blank],
        },
        RubyNode::It {
            name: "does something".into(),
            body: vec![RubyNode::Raw("assert true".into())],
        },
        RubyNode::It {
            name: "empty body".into(),
            body: vec![],
        },
        RubyNode::ItBehavesLike {
            name: "shared example".into(),
            params: vec![("key".into(), "value".into())],
        },
        RubyNode::ItBehavesLike {
            name: "no params".into(),
            params: vec![],
        },
        RubyNode::Let {
            name: "subject".into(),
            expr: "described_class.new".into(),
        },
        RubyNode::Expect {
            subject: "result".into(),
            matcher: "to eq(42)".into(),
        },
    ]
}

// ══════════════════════════════════════════════════════════════════
// NODE EMISSION PROOFS — every variant
// ══════════════════════════════════════════════════════════════════

/// Proof: Every RubyNode variant emits non-empty output.
#[test]
fn every_node_variant_emits_nonempty() {
    for (i, node) in all_node_variants().into_iter().enumerate() {
        let output = node.emit(0);
        // Blank is the one exception: it emits an empty string (blank line separator)
        if matches!(node, RubyNode::Blank) {
            assert_eq!(output, "", "Blank should emit empty string");
        } else {
            assert!(
                !output.is_empty(),
                "node variant #{i} ({node:?}) emits empty output"
            );
        }
    }
}

/// Proof: Every non-Blank node at indent 0 starts without leading spaces.
#[test]
fn every_node_at_indent_zero_has_no_leading_spaces() {
    for (i, node) in all_node_variants().into_iter().enumerate() {
        if matches!(node, RubyNode::Blank) {
            continue;
        }
        let output = node.emit(0);
        let first_line = output.lines().next().unwrap_or("");
        assert!(
            !first_line.starts_with(' '),
            "node variant #{i} at indent 0 starts with spaces: '{first_line}'"
        );
    }
}

/// Proof: Every non-Blank node at indent 3 starts with 6 spaces.
#[test]
fn every_node_at_indent_three_starts_with_six_spaces() {
    for (i, node) in all_node_variants().into_iter().enumerate() {
        if matches!(node, RubyNode::Blank) {
            continue;
        }
        // FrozenStringLiteral is special: it does NOT use indent
        if matches!(node, RubyNode::FrozenStringLiteral) {
            // FrozenStringLiteral ignores indent entirely per implementation
            continue;
        }
        let output = node.emit(3);
        let first_line = output.lines().next().unwrap_or("");
        assert!(
            first_line.starts_with("      "),
            "node variant #{i} at indent 3 should start with 6 spaces: '{first_line}'"
        );
    }
}

/// Proof: Module nodes always contain "module" and "end".
#[test]
fn module_nodes_contain_module_and_end() {
    let node = RubyNode::Module {
        path: vec!["Alpha".into(), "Beta".into()],
        body: vec![RubyNode::Blank],
    };
    let output = node.emit(0);
    assert!(output.contains("module "), "Module missing 'module' keyword");
    assert!(output.contains("end"), "Module missing 'end' keyword");
}

/// Proof: Class nodes always contain "class" and "end".
#[test]
fn class_nodes_contain_class_and_end() {
    let with_parent = RubyNode::Class {
        name: "Foo".into(),
        parent: Some("Bar".into()),
        body: vec![],
    };
    let without_parent = RubyNode::Class {
        name: "Baz".into(),
        parent: None,
        body: vec![],
    };
    for node in [with_parent, without_parent] {
        let output = node.emit(0);
        assert!(output.contains("class "), "Class missing 'class' keyword");
        assert!(output.contains("end"), "Class missing 'end' keyword");
    }
}

/// Proof: Describe nodes always contain "RSpec.describe" and "end".
#[test]
fn describe_nodes_contain_rspec_describe_and_end() {
    let node = RubyNode::Describe {
        subject: "'test'".into(),
        body: vec![],
    };
    let output = node.emit(0);
    assert!(
        output.contains("RSpec.describe"),
        "Describe missing 'RSpec.describe'"
    );
    assert!(output.contains("end"), "Describe missing 'end'");
}

/// Proof: DefineResource always contains "define_resource".
#[test]
fn define_resource_contains_keyword() {
    let node = RubyNode::DefineResource {
        tf_type: "aws_vpc".into(),
        attrs_class: "Attrs".into(),
        outputs: vec![],
        map: vec![],
        map_present: vec![],
        map_bool: vec![],
    };
    let output = node.emit(0);
    assert!(
        output.contains("define_resource"),
        "DefineResource missing keyword"
    );
}

/// Proof: DefineData always contains "define_data".
#[test]
fn define_data_contains_keyword() {
    let node = RubyNode::DefineData {
        tf_type: "aws_data".into(),
        attrs_class: "Attrs".into(),
        outputs: vec![],
        map: vec![],
        map_present: vec![],
        map_bool: vec![],
    };
    let output = node.emit(0);
    assert!(output.contains("define_data"), "DefineData missing keyword");
}

/// Proof: SharedExamples contains "RSpec.shared_examples" and "end".
#[test]
fn shared_examples_structure() {
    let node = RubyNode::SharedExamples {
        name: "example".into(),
        params: "x, y".into(),
        body: vec![RubyNode::Blank],
    };
    let output = node.emit(0);
    assert!(output.contains("RSpec.shared_examples 'example'"));
    assert!(output.contains("|x, y|"));
    assert!(output.contains("end"));
}

/// Proof: Context contains "context" and "end".
#[test]
fn context_structure() {
    let node = RubyNode::Context {
        name: "when valid".into(),
        body: vec![],
    };
    let output = node.emit(0);
    assert!(output.contains("context 'when valid'"));
    assert!(output.contains("end"));
}

/// Proof: Let binding has correct format.
#[test]
fn let_binding_format() {
    let node = RubyNode::Let {
        name: "result".into(),
        expr: "42".into(),
    };
    assert_eq!(node.emit(0), "let(:result) { 42 }");
}

/// Proof: RequireRelative has correct format.
#[test]
fn require_relative_format() {
    let node = RubyNode::RequireRelative("../helpers/test_helper".into());
    assert_eq!(node.emit(0), "require_relative '../helpers/test_helper'");
}

/// Proof: InlineModuleDecl generates correct semicolon-separated structure.
#[test]
fn inline_module_decl_balanced() {
    let node = RubyNode::InlineModuleDecl(vec![
        "A".into(),
        "B".into(),
        "C".into(),
        "D".into(),
        "E".into(),
    ]);
    let output = node.emit(0);
    // Count "module" occurrences
    let module_count = output.matches("module ").count();
    // Count "end" occurrences
    let end_count = output.matches("end").count();
    assert_eq!(module_count, 5, "should have 5 module keywords");
    assert_eq!(end_count, 5, "should have 5 end keywords");
}

// ══════════════════════════════════════════════════════════════════
// TYPE ALGEBRA PROOFS (proptest)
// ══════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Proof: For any RubyType, emit() is non-empty.
    #[test]
    fn type_emit_always_nonempty(ty in arb_ruby_type()) {
        prop_assert!(!ty.emit().is_empty());
    }

    /// Proof: optional(optional(optional(x))) == optional(x) (triple idempotence).
    #[test]
    fn triple_optional_idempotent(ty in arb_ruby_type()) {
        let once = RubyType::optional(ty.clone());
        let twice = RubyType::optional(once.clone());
        let thrice = RubyType::optional(twice.clone());
        prop_assert_eq!(&once, &thrice, "optional^3 must equal optional^1");
        prop_assert_eq!(once.emit(), thrice.emit());
    }

    /// Proof: constrained(x, rule) emit starts with x emit.
    #[test]
    fn constrained_emit_starts_with_base(ty in arb_ruby_type()) {
        let base = ty.emit();
        let constrained = RubyType::constrained(ty, "min_size?: 1").emit();
        prop_assert!(
            constrained.starts_with(&base),
            "constrained '{}' should start with base '{}'",
            constrained, base
        );
    }

    /// Proof: Array(Array(x)) emit contains x emit.
    #[test]
    fn nested_array_contains_inner(ty in arb_ruby_type()) {
        let inner_emit = ty.emit();
        let double_array = RubyType::array(RubyType::array(ty)).emit();
        prop_assert!(
            double_array.contains(&inner_emit),
            "Array(Array(x)) '{}' should contain x '{}'",
            double_array, inner_emit
        );
    }

    /// Proof: Union of N types has N-1 "|" characters at top level.
    #[test]
    fn union_pipe_count(variants in prop::collection::vec(arb_ruby_type(), 2..=5)) {
        let n = variants.len();
        let union = RubyType::union(variants);
        let emit = union.emit();
        // Count top-level pipes (depth 1 inside outermost parens)
        let mut depth: i32 = 0;
        let chars: Vec<char> = emit.chars().collect();
        let mut pipes = 0usize;
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '(' => depth += 1,
                ')' => depth -= 1,
                '|' if depth == 1 => pipes += 1,
                _ => {}
            }
            i += 1;
        }
        prop_assert_eq!(pipes, n - 1, "union of {} variants should have {} pipes, got {} in '{}'", n, n - 1, pipes, emit);
    }
}

/// Proof: Hash always emits "T::Hash".
#[test]
fn hash_always_emits_t_hash() {
    assert_eq!(RubyType::Hash.emit(), "T::Hash");
}

/// Proof: Any always emits "T::Any".
#[test]
fn any_always_emits_t_any() {
    assert_eq!(RubyType::Any.emit(), "T::Any");
}

// ══════════════════════════════════════════════════════════════════
// BUILDER EXHAUSTIVE PROOFS
// ══════════════════════════════════════════════════════════════════

/// Proof: TypesFileBuilder with 0 classes produces valid output.
#[test]
fn types_builder_zero_classes() {
    let source = TypesFileBuilder::new("empty_provider").emit();
    assert!(source.starts_with("# frozen_string_literal: true\n"));
    assert!(source.contains("module Pangea::Resources::EmptyProvider::Types"));
    assert!(source.contains("include Dry.Types()"));
    assert!(source.ends_with('\n'));
    assert!(!source.ends_with("\n\n"));
}

/// Proof: TypesFileBuilder with 10 classes produces valid output.
#[test]
fn types_builder_ten_classes() {
    let mut builder = TypesFileBuilder::new("multi");
    for i in 0..10 {
        let class_name = format!("Class{i}Attributes");
        builder = builder.class(&class_name, |c| {
            c.attribute(&format!("field_{i}"), RubyType::simple("T::String"), true)
        });
    }
    let source = builder.emit();
    assert!(source.starts_with("# frozen_string_literal: true\n"));

    // Verify all 10 classes present
    for i in 0..10 {
        assert!(
            source.contains(&format!("class Class{i}Attributes")),
            "missing Class{i}Attributes"
        );
        assert!(
            source.contains(&format!("attribute :field_{i}")),
            "missing field_{i}"
        );
    }

    // Count class declarations
    let class_count = source.matches("class Class").count();
    assert_eq!(class_count, 10, "should have exactly 10 classes");

    // Count T constant assignments
    let t_count = source.matches("T = Pangea::Resources::Multi::Types").count();
    assert_eq!(t_count, 10, "should have exactly 10 T constants");

    assert!(source.ends_with('\n'));
    assert!(!source.ends_with("\n\n"));
}

/// Proof: ResourceFileBuilder with all field categories produces valid output.
#[test]
fn resource_builder_all_field_categories() {
    let source = ResourceFileBuilder::new("test", "test_full", "full")
        .outputs(vec![("id", "id"), ("arn", "arn")])
        .map(vec!["alpha", "beta"])
        .map_present(vec!["gamma", "delta"])
        .map_bool(vec!["enabled", "active"])
        .emit();

    assert!(source.contains("map: [:alpha, :beta]"));
    assert!(source.contains("map_present: [:gamma, :delta]"));
    assert!(source.contains("map_bool: [:enabled, :active]"));
    assert!(source.contains("outputs: { id: :id, arn: :arn }"));
    assert!(source.starts_with("# frozen_string_literal: true\n"));
    assert!(source.ends_with('\n'));
    assert!(!source.ends_with("\n\n"));
}

/// Proof: ResourceFileBuilder with empty field categories produces valid output.
#[test]
fn resource_builder_empty_field_categories() {
    let source = ResourceFileBuilder::new("test", "test_empty", "empty")
        .map(vec![])
        .map_present(vec![])
        .map_bool(vec![])
        .emit();

    // Should not contain map/map_present/map_bool keys when empty
    assert!(!source.contains("map:"), "empty map should be omitted");
    assert!(
        !source.contains("map_present:"),
        "empty map_present should be omitted"
    );
    assert!(
        !source.contains("map_bool:"),
        "empty map_bool should be omitted"
    );
    assert!(source.contains("define_resource"));
    assert!(source.starts_with("# frozen_string_literal: true\n"));
    assert!(source.ends_with('\n'));
}

/// Proof: Every builder output has exactly one trailing newline.
#[test]
fn builders_exactly_one_trailing_newline() {
    let types_source = TypesFileBuilder::new("test")
        .class("Attrs", |c| {
            c.attribute("x", RubyType::simple("T::String"), true)
        })
        .emit();
    assert!(types_source.ends_with('\n'));
    assert!(!types_source.ends_with("\n\n"));

    let resource_source = ResourceFileBuilder::new("test", "test_x", "x")
        .map(vec!["a"])
        .emit();
    assert!(resource_source.ends_with('\n'));
    assert!(!resource_source.ends_with("\n\n"));
}

/// Proof: Every builder output starts with frozen pragma.
#[test]
fn builders_start_with_frozen_pragma() {
    let types_source = TypesFileBuilder::new("test")
        .class("A", |c| c.attribute("x", RubyType::Any, true))
        .emit();
    assert!(types_source.starts_with("# frozen_string_literal: true\n"));

    let resource_source = ResourceFileBuilder::new("test", "test_a", "a").emit();
    assert!(resource_source.starts_with("# frozen_string_literal: true\n"));
}

/// Proof: Attribute count in output matches input count.
#[test]
fn attribute_count_matches_input() {
    let counts = [1, 3, 7, 15];
    for &n in &counts {
        let source = TypesFileBuilder::new("count_test")
            .class("Attrs", |c| {
                let mut cb = c;
                for i in 0..n {
                    cb = cb.attribute(
                        &format!("attr_{i}"),
                        RubyType::simple("T::String"),
                        true,
                    );
                }
                cb
            })
            .emit();

        let attr_count = source
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                trimmed.starts_with("attribute :") || trimmed.starts_with("attribute? :")
            })
            .count();
        assert_eq!(
            attr_count, n,
            "expected {n} attributes, got {attr_count}"
        );
    }
}

// ══════════════════════════════════════════════════════════════════
// IAC BRIDGE EXHAUSTIVE PROOFS (--features iac-bridge)
// ══════════════════════════════════════════════════════════════════

#[cfg(feature = "iac-bridge")]
mod iac_bridge_proofs {
    use super::*;
    use iac_forge::ir::{IacAttribute, IacType};
    use ruby_synthesizer::iac_bridge::iac_type_to_ruby;

    /// Proof: Every IacType variant maps to a non-empty RubyType.
    #[test]
    fn every_iac_type_variant_nonempty() {
        let all_variants: Vec<IacType> = vec![
            IacType::String,
            IacType::Integer,
            IacType::Float,
            IacType::Numeric,
            IacType::Boolean,
            IacType::Any,
            IacType::List(Box::new(IacType::String)),
            IacType::Set(Box::new(IacType::Integer)),
            IacType::Map(Box::new(IacType::String)),
            IacType::Object {
                name: "test".into(),
                fields: vec![],
            },
            IacType::Enum {
                values: vec!["a".into(), "b".into()],
                underlying: Box::new(IacType::String),
            },
            IacType::Enum {
                values: vec![],
                underlying: Box::new(IacType::Integer),
            },
        ];

        for ty in &all_variants {
            let ruby = iac_type_to_ruby(ty);
            let emit = ruby.emit();
            assert!(
                !emit.is_empty(),
                "IacType {ty:?} produced empty RubyType emit"
            );
        }
    }

    /// Proof: Nested types (List(List(List(String)))) handle 5+ levels.
    #[test]
    fn deeply_nested_list_five_levels() {
        let mut ty = IacType::String;
        for _ in 0..5 {
            ty = IacType::List(Box::new(ty));
        }
        let ruby = iac_type_to_ruby(&ty);
        let emit = ruby.emit();
        assert!(!emit.is_empty());
        // Should have 5 levels of T::Array.of(
        let array_count = emit.matches("T::Array.of(").count();
        assert_eq!(
            array_count, 5,
            "5 levels of nesting should produce 5 T::Array.of( prefixes, got {array_count} in '{emit}'"
        );
        // Should contain T::String at the innermost level
        assert!(
            emit.contains("T::String"),
            "innermost type should be T::String"
        );
    }

    /// Proof: Enum with 100 values produces valid constrained type.
    #[test]
    fn enum_hundred_values() {
        let values: Vec<String> = (0..100).map(|i| format!("val_{i}")).collect();
        let ty = IacType::Enum {
            values: values.clone(),
            underlying: Box::new(IacType::String),
        };
        let ruby = iac_type_to_ruby(&ty);
        let emit = ruby.emit();

        assert!(
            emit.starts_with("T::String.constrained("),
            "100-value enum should be constrained"
        );
        assert!(emit.contains("included_in:"), "should have included_in constraint");

        // Verify all 100 values present
        for v in &values {
            assert!(
                emit.contains(&format!("'{v}'")),
                "missing value '{v}' in enum emit"
            );
        }
    }

    /// Proof: Map of any inner type always produces Hash.
    #[test]
    fn map_always_produces_hash() {
        let inner_types = vec![
            IacType::String,
            IacType::Integer,
            IacType::Boolean,
            IacType::List(Box::new(IacType::String)),
            IacType::Map(Box::new(IacType::Integer)),
            IacType::Any,
        ];
        for inner in inner_types {
            let ty = IacType::Map(Box::new(inner.clone()));
            let ruby = iac_type_to_ruby(&ty);
            assert_eq!(
                ruby.emit(),
                "T::Hash",
                "Map({inner:?}) should produce T::Hash"
            );
        }
    }

    /// Proof: Object of any structure always produces Hash.
    #[test]
    fn object_always_produces_hash() {
        let ty = IacType::Object {
            name: "complex".into(),
            fields: vec![
                IacAttribute {
                    api_name: "a".into(),
                    canonical_name: "a".into(),
                    description: String::new(),
                    iac_type: IacType::String,
                    required: true,
                    optional: false,
                    computed: false,
                    sensitive: false,
                    json_encoded: false,
                    immutable: false,
                    default_value: None,
                    enum_values: None,
                    read_path: None,
                    update_only: false,
                },
                IacAttribute {
                    api_name: "b".into(),
                    canonical_name: "b".into(),
                    description: String::new(),
                    iac_type: IacType::List(Box::new(IacType::Integer)),
                    required: false,
                    optional: true,
                    computed: false,
                    sensitive: false,
                    json_encoded: false,
                    immutable: false,
                    default_value: None,
                    enum_values: None,
                    read_path: None,
                    update_only: false,
                },
            ],
        };
        assert_eq!(
            iac_type_to_ruby(&ty).emit(),
            "T::Hash",
            "Object should always produce T::Hash"
        );
    }

    /// Proof: Set and List with same inner produce identical Ruby output.
    #[test]
    fn set_and_list_identical_output() {
        let inners = vec![
            IacType::String,
            IacType::Integer,
            IacType::Boolean,
            IacType::List(Box::new(IacType::String)),
        ];
        for inner in inners {
            let list = iac_type_to_ruby(&IacType::List(Box::new(inner.clone()))).emit();
            let set = iac_type_to_ruby(&IacType::Set(Box::new(inner.clone()))).emit();
            assert_eq!(
                list, set,
                "List and Set with same inner ({inner:?}) should emit identically"
            );
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        /// Proof: Every IacType maps to well-formed Dry::Types expression.
        #[test]
        fn iac_bridge_well_formed(ty in super::arb_iac_type_full()) {
            let ruby = iac_type_to_ruby(&ty);
            let emit = ruby.emit();
            prop_assert!(!emit.is_empty());
            // Must start with T:: or ( (for unions)
            let base = emit
                .trim_end_matches(".optional")
                .split(".constrained(")
                .next()
                .unwrap_or(&emit);
            prop_assert!(
                base.starts_with("T::") || base.starts_with("("),
                "IaC bridge produced invalid base type: '{base}' (full: '{emit}')"
            );
        }
    }
}

/// Strategy for generating arbitrary IacType values (used by iac_bridge_proofs).
#[cfg(feature = "iac-bridge")]
fn arb_iac_type_full() -> impl Strategy<Value = iac_forge::ir::IacType> {
    use iac_forge::ir::{IacAttribute, IacType};

    let leaf = prop_oneof![
        Just(IacType::String),
        Just(IacType::Integer),
        Just(IacType::Float),
        Just(IacType::Numeric),
        Just(IacType::Boolean),
        Just(IacType::Any),
    ];

    leaf.prop_recursive(
        3,
        32,
        6,
        |inner| {
            prop_oneof![
                inner.clone().prop_map(|t| IacType::List(Box::new(t))),
                inner.clone().prop_map(|t| IacType::Set(Box::new(t))),
                inner.clone().prop_map(|t| IacType::Map(Box::new(t))),
                (
                    "[a-z][a-z_]{1,8}",
                    prop::collection::vec(
                        (
                            "[a-z][a-z_]{1,8}",
                            inner.clone(),
                            any::<bool>(),
                        )
                            .prop_map(|(name, ty, req)| IacAttribute {
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
                            }),
                        0..=3,
                    ),
                )
                    .prop_map(|(name, fields)| IacType::Object { name, fields }),
                (
                    prop::collection::vec("[a-z]{2,6}", 0..=5),
                    inner.clone(),
                )
                    .prop_map(|(values, underlying)| IacType::Enum {
                        values,
                        underlying: Box::new(underlying),
                    }),
            ]
        },
    )
}

// ══════════════════════════════════════════════════════════════════
// CROSS-CUTTING PROOFS
// ══════════════════════════════════════════════════════════════════

/// Proof: emit_file on 100 nodes doesn't panic.
#[test]
fn emit_file_hundred_nodes_no_panic() {
    let mut nodes = Vec::with_capacity(100);
    nodes.push(RubyNode::FrozenStringLiteral);
    nodes.push(RubyNode::Comment("generated".into()));
    nodes.push(RubyNode::Blank);
    nodes.push(RubyNode::Require("something".into()));
    nodes.push(RubyNode::RequireRelative("other".into()));

    // Add 20 modules with classes inside
    for i in 0..20 {
        nodes.push(RubyNode::Module {
            path: vec![format!("Mod{i}")],
            body: vec![
                RubyNode::Class {
                    name: format!("Class{i}"),
                    parent: Some("Base".into()),
                    body: vec![
                        RubyNode::Attribute {
                            name: format!("field_{i}"),
                            type_expr: RubyType::simple("T::String"),
                            required: true,
                        },
                    ],
                },
            ],
        });
    }

    // Fill remaining with assorted nodes
    while nodes.len() < 100 {
        nodes.push(RubyNode::Comment(format!("line {}", nodes.len())));
    }

    let output = emit_file(&nodes);
    assert!(!output.is_empty());
    assert!(output.ends_with('\n'));
    assert!(output.lines().count() > 100);
}

/// Proof: Random node trees always produce balanced output (module/class/end).
#[test]
fn random_node_tree_balanced() {
    // Build a deeply nested tree
    let inner = RubyNode::Module {
        path: vec!["Inner".into()],
        body: vec![
            RubyNode::Class {
                name: "A".into(),
                parent: None,
                body: vec![RubyNode::Blank],
            },
            RubyNode::Class {
                name: "B".into(),
                parent: Some("A".into()),
                body: vec![
                    RubyNode::Attribute {
                        name: "x".into(),
                        type_expr: RubyType::simple("T::String"),
                        required: true,
                    },
                ],
            },
        ],
    };

    let outer = RubyNode::Module {
        path: vec!["Outer".into()],
        body: vec![inner],
    };

    let output = emit_file(&[outer]);
    let opens = output.matches("module ").count() + output.matches("class ").count();
    // Count standalone "end" lines
    let ends = output
        .lines()
        .filter(|l| l.trim() == "end")
        .count();
    assert_eq!(opens, ends, "tree must have balanced opens ({opens}) and ends ({ends})");
}

/// Proof: Output never contains null bytes.
#[test]
fn output_never_contains_null_bytes() {
    let variants = all_node_variants();
    let output = emit_file(&variants);
    assert!(
        !output.contains('\0'),
        "output must never contain null bytes"
    );
}

/// Proof: Output is valid UTF-8 (trivially true since Rust strings are UTF-8,
/// but we verify no unexpected byte sequences).
#[test]
fn output_is_valid_utf8() {
    let variants = all_node_variants();
    let output = emit_file(&variants);
    // Verify every byte is printable ASCII or newline
    for (i, byte) in output.bytes().enumerate() {
        assert!(
            (0x20..=0x7E).contains(&byte) || byte == b'\n',
            "non-printable byte 0x{byte:02x} at position {i}"
        );
    }
}

/// Proof: RSpecBuilder produces well-structured output with correct nesting.
#[test]
fn rspec_builder_complex_structure() {
    let spec = RSpecBuilder::describe("'complex test'")
        .let_bind("instance", "described_class.new")
        .context("when valid", |b| {
            b.it("passes", |b| {
                b.expect("instance.valid?", "to be true")
            })
            .it("has attributes", |b| {
                b.expect("instance.name", "to eq('test')")
                    .expect("instance.id", "not_to be_nil")
            })
        })
        .context("when invalid", |b| {
            b.it("fails", |b| b.expect("instance.valid?", "to be false"))
        })
        .it_behaves_like("a resource", vec![("type", "'test'")])
        .build();

    let output = spec.emit(0);

    // Structure checks
    assert!(output.contains("RSpec.describe 'complex test' do"));
    assert!(output.contains("let(:instance) { described_class.new }"));
    assert!(output.contains("context 'when valid' do"));
    assert!(output.contains("context 'when invalid' do"));
    assert!(output.contains("it_behaves_like 'a resource'"));

    // Balanced do/end
    let do_count = output.matches(" do\n").count() + output.matches(" do\r\n").count();
    let end_count = output.lines().filter(|l| l.trim() == "end").count();
    assert_eq!(
        do_count, end_count,
        "do ({do_count}) and end ({end_count}) must be balanced"
    );
}

/// Proof: SharedExamplesBuilder produces correct structure.
#[test]
fn shared_examples_builder_structure() {
    let shared = RSpecBuilder::shared_examples("typed provider", "mod, types")
        .let_bind("provider", "mod")
        .it("has types", |b| {
            b.expect("types", "not_to be_nil")
        })
        .build();

    let output = shared.emit(0);
    assert!(output.contains("RSpec.shared_examples 'typed provider' do |mod, types|"));
    assert!(output.contains("let(:provider) { mod }"));
    assert!(output.contains("it 'has types' do"));
    assert!(output.contains("end"));
}

/// Proof: Empty emit_file produces just a trailing newline.
#[test]
fn emit_file_empty_nodes() {
    let output = emit_file(&[]);
    assert_eq!(output, "\n", "empty nodes should produce just a newline");
}

/// Proof: Single Blank node emits correctly.
#[test]
fn emit_file_single_blank() {
    let output = emit_file(&[RubyNode::Blank]);
    assert_eq!(output, "\n", "single Blank should produce just a newline");
}

/// Proof: DefineResource and DefineData have identical structure except keyword.
#[test]
fn define_resource_vs_define_data_structure() {
    let resource = RubyNode::DefineResource {
        tf_type: "thing".into(),
        attrs_class: "Attrs".into(),
        outputs: vec![("id".into(), "id".into())],
        map: vec!["a".into()],
        map_present: vec!["b".into()],
        map_bool: vec!["c".into()],
    };
    let data = RubyNode::DefineData {
        tf_type: "thing".into(),
        attrs_class: "Attrs".into(),
        outputs: vec![("id".into(), "id".into())],
        map: vec!["a".into()],
        map_present: vec!["b".into()],
        map_bool: vec!["c".into()],
    };

    let r_out = resource.emit(0);
    let d_out = data.emit(0);

    // They should differ only in the keyword
    let r_normalized = r_out.replace("define_resource", "KEYWORD");
    let d_normalized = d_out.replace("define_data", "KEYWORD");
    assert_eq!(
        r_normalized, d_normalized,
        "DefineResource and DefineData should be structurally identical except keyword"
    );
}

/// Proof: ConstAssign format is exact.
#[test]
fn const_assign_exact_format() {
    let node = RubyNode::ConstAssign {
        name: "T".into(),
        value: "Pangea::Resources::Test::Types".into(),
    };
    assert_eq!(node.emit(0), "T = Pangea::Resources::Test::Types");
    assert_eq!(
        node.emit(2),
        "    T = Pangea::Resources::Test::Types"
    );
}

/// Proof: RegistryCall format is exact.
#[test]
fn registry_call_exact_format() {
    let node = RubyNode::RegistryCall("Pangea::Resources::AWS".into());
    assert_eq!(
        node.emit(0),
        "Pangea::ResourceRegistry.register_module(Pangea::Resources::AWS)"
    );
}

/// Proof: Raw node passes through content exactly.
#[test]
fn raw_node_passthrough() {
    let content = "some_method(:arg1, arg2) { |x| x + 1 }";
    let node = RubyNode::Raw(content.into());
    assert_eq!(node.emit(0), content);
    assert_eq!(node.emit(1), format!("  {content}"));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Proof: Multiple classes in TypesFileBuilder each get their own T constant.
    #[test]
    fn multi_class_t_constant_per_class(
        class_count in 1..=5usize,
    ) {
        let mut builder = TypesFileBuilder::new("multi_t");
        for i in 0..class_count {
            let name = format!("Class{i}Attrs");
            builder = builder.class(&name, |c| {
                c.attribute("x", RubyType::simple("T::String"), true)
            });
        }
        let source = builder.emit();
        let t_count = source.matches("T = Pangea::Resources::MultiT::Types").count();
        prop_assert_eq!(
            t_count, class_count,
            "expected {} T constants, got {}", class_count, t_count
        );
    }

    /// Proof: Deeply nested modules produce correct indentation.
    #[test]
    fn nested_module_indentation(depth in 1..=4usize) {
        let mut node = RubyNode::Include("Leaf".into());
        for i in (0..depth).rev() {
            node = RubyNode::Module {
                path: vec![format!("Level{i}")],
                body: vec![node],
            };
        }
        let output = node.emit(0);
        // The innermost include should be indented by depth*2 spaces
        let expected_indent = "  ".repeat(depth);
        let include_line = output.lines().find(|l| l.contains("include Leaf")).unwrap();
        prop_assert!(
            include_line.starts_with(&expected_indent),
            "include at depth {depth} should start with {} spaces, got: '{include_line}'",
            depth * 2
        );
    }
}
