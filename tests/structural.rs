//! Structural builder proofs — 21 proptest invariants proving TypesFileBuilder
//! and ResourceFileBuilder always produce structurally valid Ruby.

use proptest::prelude::*;
use regex::Regex;
use std::collections::HashSet;

use ruby_synthesizer::builders::{ResourceFileBuilder, TypesFileBuilder};
use ruby_synthesizer::RubyType;

// ── Strategies ──────────────────────────────────────────────────────

fn arb_provider() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("aws".to_string()),
        Just("porkbun".to_string()),
        Just("hcloud".to_string()),
        Just("cloudflare".to_string()),
        Just("datadog".to_string()),
    ]
}

fn arb_attr_name() -> impl Strategy<Value = String> {
    "[a-z][a-z_]{1,15}".prop_filter("no ruby keywords", |s| {
        !["end", "def", "class", "module", "do", "if", "else", "begin", "rescue"]
            .contains(&s.as_str())
    })
}

fn arb_ruby_type() -> impl Strategy<Value = RubyType> {
    prop_oneof![
        Just(RubyType::simple("T::String")),
        Just(RubyType::simple("T::Integer")),
        Just(RubyType::simple("T::Bool")),
        Just(RubyType::Hash),
        Just(RubyType::Any),
        Just(RubyType::array(RubyType::simple("T::String"))),
    ]
}

/// Generate a vector of unique attribute names by deduplicating via HashSet.
fn arb_unique_attr_names(min: usize, max: usize) -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(arb_attr_name(), max..=max * 2).prop_map(move |names| {
        let mut seen = HashSet::new();
        let unique: Vec<String> = names
            .into_iter()
            .filter(|n| seen.insert(n.clone()))
            .take(max)
            .collect();
        // Guarantee at least min unique names by padding if needed
        if unique.len() < min {
            let mut padded = unique;
            for i in 0..(min - padded.len()) {
                padded.push(format!("attr_{i}"));
            }
            padded
        } else {
            unique
        }
    })
}

/// Convert provider name to expected PascalCase (mirrors the builder's internal logic).
fn expected_pascal(provider: &str) -> String {
    match provider {
        "aws" => "AWS".to_string(),
        "porkbun" => "Porkbun".to_string(),
        "hcloud" => "Hcloud".to_string(),
        "cloudflare" => "Cloudflare".to_string(),
        "datadog" => "Datadog".to_string(),
        _ => {
            let mut result = String::new();
            let mut cap = true;
            for ch in provider.chars() {
                if ch == '_' || ch == '-' {
                    cap = true;
                } else if cap {
                    result.extend(ch.to_uppercase());
                    cap = false;
                } else {
                    result.push(ch);
                }
            }
            result
        }
    }
}

// ── TypesFileBuilder proofs (1-11) ──────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Proof 1: Output always starts with frozen_string_literal pragma.
    #[test]
    fn types_always_starts_with_frozen_literal(
        provider in arb_provider(),
        name in arb_attr_name(),
        ty in arb_ruby_type(),
    ) {
        let source = TypesFileBuilder::new(&provider)
            .class("TestAttributes", |c| c.attribute(&name, ty, true))
            .emit();
        prop_assert!(
            source.starts_with("# frozen_string_literal: true\n"),
            "missing frozen pragma in:\n{source}"
        );
    }

    /// Proof 2: Exactly one `module` declaration per builder output.
    #[test]
    fn types_has_exactly_one_module_declaration(
        provider in arb_provider(),
        name in arb_attr_name(),
        ty in arb_ruby_type(),
    ) {
        let source = TypesFileBuilder::new(&provider)
            .class("TestAttributes", |c| c.attribute(&name, ty, true))
            .emit();
        let module_re = Regex::new(r"(?m)^module ").unwrap();
        let count = module_re.find_iter(&source).count();
        prop_assert!(count == 1, "expected 1 module declaration, got {count} in:\n{source}");
    }

    /// Proof 3: Module path contains provider in PascalCase.
    #[test]
    fn types_module_path_contains_pascal_provider(
        provider in arb_provider(),
        name in arb_attr_name(),
        ty in arb_ruby_type(),
    ) {
        let source = TypesFileBuilder::new(&provider)
            .class("TestAttributes", |c| c.attribute(&name, ty, true))
            .emit();
        let pascal = expected_pascal(&provider);
        let expected_module = format!("module Pangea::Resources::{pascal}::Types");
        prop_assert!(
            source.contains(&expected_module),
            "missing module path '{expected_module}' in:\n{source}"
        );
    }

    /// Proof 4: Output contains `include Dry.Types()`.
    #[test]
    fn types_has_include_dry_types(
        provider in arb_provider(),
        name in arb_attr_name(),
        ty in arb_ruby_type(),
    ) {
        let source = TypesFileBuilder::new(&provider)
            .class("TestAttributes", |c| c.attribute(&name, ty, true))
            .emit();
        prop_assert!(
            source.contains("include Dry.Types()"),
            "missing include Dry.Types() in:\n{source}"
        );
    }

    /// Proof 5: Class inherits from BaseAttributes.
    #[test]
    fn types_class_inherits_base_attributes(
        provider in arb_provider(),
        name in arb_attr_name(),
        ty in arb_ruby_type(),
    ) {
        let source = TypesFileBuilder::new(&provider)
            .class("TestAttributes", |c| c.attribute(&name, ty, true))
            .emit();
        prop_assert!(
            source.contains("< Pangea::Resources::BaseAttributes"),
            "missing BaseAttributes inheritance in:\n{source}"
        );
    }

    /// Proof 6: Every attribute name matches [a-z_][a-z0-9_]*.
    #[test]
    fn types_attribute_names_valid(
        provider in arb_provider(),
        names in arb_unique_attr_names(1, 5),
    ) {
        let mut builder = TypesFileBuilder::new(&provider);
        let names_clone = names.clone();
        builder = builder.class("TestAttributes", |c| {
            let mut cb = c;
            for n in &names_clone {
                cb = cb.attribute(n, RubyType::simple("T::String"), true);
            }
            cb
        });
        let source = builder.emit();
        let attr_name_re = Regex::new(r"attribute\??\s+:([a-z_][a-z0-9_]*)").unwrap();
        for cap in attr_name_re.captures_iter(&source) {
            let attr_name = &cap[1];
            prop_assert!(
                attr_name.chars().all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()),
                "invalid attribute name '{attr_name}' in:\n{source}"
            );
        }
    }

    /// Proof 7: No duplicate attribute names when given unique inputs.
    #[test]
    fn types_no_duplicate_attribute_names(
        provider in arb_provider(),
        names in arb_unique_attr_names(2, 6),
    ) {
        let names_clone = names.clone();
        let source = TypesFileBuilder::new(&provider)
            .class("TestAttributes", |c| {
                let mut cb = c;
                for n in &names_clone {
                    cb = cb.attribute(n, RubyType::simple("T::String"), true);
                }
                cb
            })
            .emit();
        let attr_name_re = Regex::new(r"attribute\??\s+:(\w+)").unwrap();
        let mut seen = HashSet::new();
        for cap in attr_name_re.captures_iter(&source) {
            let name = cap[1].to_string();
            prop_assert!(
                seen.insert(name.clone()),
                "duplicate attribute name '{name}' in:\n{source}"
            );
        }
    }

    /// Proof 8: T constant references the correct types module.
    #[test]
    fn types_t_constant_references_correct_module(
        provider in arb_provider(),
        name in arb_attr_name(),
        ty in arb_ruby_type(),
    ) {
        let source = TypesFileBuilder::new(&provider)
            .class("TestAttributes", |c| c.attribute(&name, ty, true))
            .emit();
        let pascal = expected_pascal(&provider);
        let expected_t = format!("T = Pangea::Resources::{pascal}::Types");
        prop_assert!(
            source.contains(&expected_t),
            "missing T constant '{expected_t}' in:\n{source}"
        );
    }

    /// Proof 9: Output contains the base_attributes require.
    #[test]
    fn types_has_base_attributes_require(
        provider in arb_provider(),
        name in arb_attr_name(),
        ty in arb_ruby_type(),
    ) {
        let source = TypesFileBuilder::new(&provider)
            .class("TestAttributes", |c| c.attribute(&name, ty, true))
            .emit();
        prop_assert!(
            source.contains("require 'pangea/resources/base_attributes'"),
            "missing base_attributes require in:\n{source}"
        );
    }

    /// Proof 10: File ends with a trailing newline.
    #[test]
    fn types_file_ends_with_newline(
        provider in arb_provider(),
        name in arb_attr_name(),
        ty in arb_ruby_type(),
    ) {
        let source = TypesFileBuilder::new(&provider)
            .class("TestAttributes", |c| c.attribute(&name, ty, true))
            .emit();
        prop_assert!(
            source.ends_with('\n'),
            "file does not end with newline"
        );
    }

    /// Proof 11: Every class has a T constant assignment.
    #[test]
    fn types_every_class_has_t_constant(
        provider in arb_provider(),
        names in arb_unique_attr_names(1, 3),
    ) {
        let names_clone = names.clone();
        let source = TypesFileBuilder::new(&provider)
            .class("AlphaAttributes", |c| {
                let mut cb = c;
                for n in &names_clone {
                    cb = cb.attribute(n, RubyType::simple("T::String"), true);
                }
                cb
            })
            .emit();
        let class_re = Regex::new(r"(?m)^\s+class ").unwrap();
        let t_re = Regex::new(r"(?m)^\s+T = ").unwrap();
        let class_count = class_re.find_iter(&source).count();
        let t_count = t_re.find_iter(&source).count();
        prop_assert!(
            class_count == t_count,
            "class count ({}) != T constant count ({}) in:\n{}",
            class_count, t_count, source
        );
    }
}

// ── ResourceFileBuilder proofs (12-21) ──────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Proof 12: Output contains `define_resource`.
    #[test]
    fn resource_has_define_resource(
        provider in arb_provider(),
        field in arb_attr_name(),
    ) {
        let tf_type = format!("{provider}_test_resource");
        let source = ResourceFileBuilder::new(&provider, &tf_type, "test_resource")
            .map(vec![&*field])
            .emit();
        prop_assert!(
            source.contains("define_resource"),
            "missing define_resource in:\n{source}"
        );
    }

    /// Proof 13: Output contains `attributes_class:` reference.
    #[test]
    fn resource_has_attributes_class(
        provider in arb_provider(),
        field in arb_attr_name(),
    ) {
        let tf_type = format!("{provider}_widget");
        let source = ResourceFileBuilder::new(&provider, &tf_type, "widget")
            .map(vec![&*field])
            .emit();
        prop_assert!(
            source.contains("attributes_class:"),
            "missing attributes_class in:\n{source}"
        );
    }

    /// Proof 14: Output contains a registry call.
    #[test]
    fn resource_has_registry_call(
        provider in arb_provider(),
    ) {
        let tf_type = format!("{provider}_item");
        let source = ResourceFileBuilder::new(&provider, &tf_type, "item")
            .emit();
        let pascal = expected_pascal(&provider);
        let expected = format!("Pangea::ResourceRegistry.register_module(Pangea::Resources::{pascal})");
        prop_assert!(
            source.contains(&expected),
            "missing registry call '{expected}' in:\n{source}"
        );
    }

    /// Proof 15: Outer module is Pangea::Resources.
    #[test]
    fn resource_outer_module_is_pangea_resources(
        provider in arb_provider(),
    ) {
        let tf_type = format!("{provider}_thing");
        let source = ResourceFileBuilder::new(&provider, &tf_type, "thing")
            .emit();
        prop_assert!(
            source.contains("module Pangea::Resources"),
            "missing Pangea::Resources module in:\n{source}"
        );
    }

    /// Proof 16: Output contains ResourceBuilder include.
    #[test]
    fn resource_has_resource_builder_include(
        provider in arb_provider(),
    ) {
        let tf_type = format!("{provider}_gadget");
        let source = ResourceFileBuilder::new(&provider, &tf_type, "gadget")
            .emit();
        prop_assert!(
            source.contains("include Pangea::Resources::ResourceBuilder"),
            "missing ResourceBuilder include in:\n{source}"
        );
    }

    /// Proof 17: Output has base require.
    #[test]
    fn resource_has_base_require(
        provider in arb_provider(),
    ) {
        let tf_type = format!("{provider}_foo");
        let source = ResourceFileBuilder::new(&provider, &tf_type, "foo")
            .emit();
        prop_assert!(
            source.contains("require 'pangea/resources/base'"),
            "missing base require in:\n{source}"
        );
    }

    /// Proof 18: Output has reference require.
    #[test]
    fn resource_has_reference_require(
        provider in arb_provider(),
    ) {
        let tf_type = format!("{provider}_bar");
        let source = ResourceFileBuilder::new(&provider, &tf_type, "bar")
            .emit();
        prop_assert!(
            source.contains("require 'pangea/resources/reference'"),
            "missing reference require in:\n{source}"
        );
    }

    /// Proof 19: Map fields use symbol syntax (`:field_name`).
    #[test]
    fn resource_map_fields_use_symbol_syntax(
        provider in arb_provider(),
        fields in prop::collection::vec(arb_attr_name(), 1..=4),
    ) {
        let tf_type = format!("{provider}_mapped");
        let field_refs: Vec<&str> = fields.iter().map(String::as_str).collect();
        let source = ResourceFileBuilder::new(&provider, &tf_type, "mapped")
            .map(field_refs)
            .emit();
        for field in &fields {
            let sym = format!(":{field}");
            prop_assert!(
                source.contains(&sym),
                "missing symbol {sym} in:\n{source}"
            );
        }
    }

    /// Proof 20: Default outputs include id: :id.
    #[test]
    fn resource_default_outputs_has_id(
        provider in arb_provider(),
    ) {
        let tf_type = format!("{provider}_default");
        let source = ResourceFileBuilder::new(&provider, &tf_type, "default")
            .emit();
        prop_assert!(
            source.contains("id: :id"),
            "missing default id output in:\n{source}"
        );
    }

    /// Proof 21: File has balanced blocks (module/class/end).
    #[test]
    fn resource_file_has_balanced_blocks(
        provider in arb_provider(),
        fields in prop::collection::vec(arb_attr_name(), 0..=4),
    ) {
        let tf_type = format!("{provider}_balanced");
        let field_refs: Vec<&str> = fields.iter().map(String::as_str).collect();
        let source = ResourceFileBuilder::new(&provider, &tf_type, "balanced")
            .map(field_refs)
            .emit();
        let module_re = Regex::new(r"(?m)^\s*module ").unwrap();
        let class_re = Regex::new(r"(?m)^\s*class ").unwrap();
        let end_re = Regex::new(r"(?m)^\s*end\s*$").unwrap();

        let openers = module_re.find_iter(&source).count()
            + class_re.find_iter(&source).count();
        let closers = end_re.find_iter(&source).count();

        prop_assert!(
            openers == closers,
            "unbalanced blocks: {} openers vs {} ends in:\n{}",
            openers, closers, source
        );
    }
}
