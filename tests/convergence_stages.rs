//! Convergence stage proofs — prove the pipeline stages hold.
//!
//! declared → resolved → converged → verified
//! Builder construction → AST → emit → valid Ruby

use ruby_synthesizer::{RubyNode, RubyType, emit_file};
use ruby_synthesizer::builders::{TypesFileBuilder, ResourceFileBuilder};

// ── declared → resolved: construction never panics ───────────────

#[test]
fn types_builder_construction_succeeds() {
    let source = TypesFileBuilder::new("aws")
        .class("VpcAttributes", |c| {
            c.attribute("cidr_block", RubyType::simple("T::String"), true)
             .attribute("enable_dns", RubyType::simple("T::Bool"), false)
        })
        .emit();
    assert!(!source.is_empty());
}

#[test]
fn resource_builder_construction_succeeds() {
    let source = ResourceFileBuilder::new("aws", "aws_vpc", "vpc")
        .map(vec!["cidr_block"])
        .map_present(vec!["description"])
        .map_bool(vec!["enable_dns_support"])
        .emit();
    assert!(!source.is_empty());
}

#[test]
fn empty_class_construction_succeeds() {
    let source = TypesFileBuilder::new("test")
        .class("EmptyAttributes", |c| c)
        .emit();
    assert!(source.contains("class EmptyAttributes"));
}

// ── resolved → converged: emit always ends with newline ──────────

#[test]
fn emit_file_ends_with_newline() {
    let nodes = vec![
        RubyNode::FrozenStringLiteral,
        RubyNode::Comment("test".into()),
    ];
    let output = emit_file(&nodes);
    assert!(output.ends_with('\n'));
}

#[test]
fn types_builder_emit_ends_with_newline() {
    let source = TypesFileBuilder::new("porkbun")
        .class("Test", |c| c.attribute("x", RubyType::simple("T::String"), true))
        .emit();
    assert!(source.ends_with('\n'));
}

#[test]
fn resource_builder_emit_ends_with_newline() {
    let source = ResourceFileBuilder::new("porkbun", "porkbun_dns_record", "dns_record")
        .map(vec!["domain"])
        .emit();
    assert!(source.ends_with('\n'));
}

// ── End-to-end determinism ───────────────────────────────────────

#[test]
fn types_builder_deterministic() {
    let build = || TypesFileBuilder::new("aws")
        .class("VpcAttributes", |c| {
            c.attribute("cidr", RubyType::simple("T::String"), true)
             .attribute("dns", RubyType::simple("T::Bool"), false)
        })
        .emit();

    assert_eq!(build(), build());
}

#[test]
fn resource_builder_deterministic() {
    let build = || ResourceFileBuilder::new("aws", "aws_vpc", "vpc")
        .map(vec!["cidr_block"])
        .map_present(vec!["tags"])
        .emit();

    assert_eq!(build(), build());
}

// ── Stage monotonicity: adding nodes only adds lines ─────────────

#[test]
fn adding_attribute_increases_output() {
    let small = TypesFileBuilder::new("test")
        .class("SmallAttributes", |c| {
            c.attribute("a", RubyType::simple("T::String"), true)
        })
        .emit();

    let large = TypesFileBuilder::new("test")
        .class("LargeAttributes", |c| {
            c.attribute("a", RubyType::simple("T::String"), true)
             .attribute("b", RubyType::simple("T::Integer"), true)
             .attribute("c", RubyType::simple("T::Bool"), false)
        })
        .emit();

    assert!(large.lines().count() > small.lines().count());
}

#[test]
fn adding_map_fields_increases_output() {
    let small = ResourceFileBuilder::new("test", "test_r", "r")
        .map(vec!["a"])
        .emit();

    let large = ResourceFileBuilder::new("test", "test_r", "r")
        .map(vec!["a", "b", "c"])
        .map_present(vec!["d", "e"])
        .map_bool(vec!["f"])
        .emit();

    assert!(large.len() > small.len());
}

// ── Macro parity ─────────────────────────────────────────────────

#[test]
fn macro_module_emit_matches_manual() {
    use ruby_synthesizer::{ruby_module, ruby_body, ruby_parent};

    let via_macro = ruby_module!("Test::Types" => {
        include "Dry.Types()";
    });

    let manual = RubyNode::Module {
        path: vec!["Test".into(), "Types".into()],
        body: vec![RubyNode::Include("Dry.Types()".into())],
    };

    assert_eq!(via_macro.emit(0), manual.emit(0));
}
