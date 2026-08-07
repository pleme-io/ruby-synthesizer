//! RSpec builder proofs — prove test generation structure.

use ruby_synthesizer::{RSpecBuilder, RubyNode};

// ── describe structure ───────────────────────────────────────────

#[test]
fn describe_produces_rspec_describe() {
    let node = RSpecBuilder::describe("'my test'").build();
    let output = node.emit(0);
    assert!(output.starts_with("RSpec.describe 'my test' do"));
    assert!(output.ends_with("end"));
}

// ── it blocks ────────────────────────────────────────────────────

#[test]
fn it_block_structure() {
    let node = RSpecBuilder::describe("'test'")
        .it("does something", |b| b.expect("x", "to eq(1)"))
        .build();
    let output = node.emit(0);
    assert!(output.contains("it 'does something' do"));
    assert!(output.contains("expect(x).to eq(1)"));
}

// ── let bindings ─────────────────────────────────────────────────

#[test]
fn let_binding_structure() {
    let node = RSpecBuilder::describe("'test'")
        .let_bind("subject", "described_class.new")
        .build();
    let output = node.emit(0);
    assert!(output.contains("let(:subject) { described_class.new }"));
}

// ── expect assertions ────────────────────────────────────────────

#[test]
fn expect_assertion_structure() {
    let node = RubyNode::Expect {
        subject: "result".into(),
        matcher: "to be_nil".into(),
    };
    assert_eq!(node.emit(2), "    expect(result).to be_nil");
}

// ── it_behaves_like with no params ───────────────────────────────

#[test]
fn it_behaves_like_no_trailing_comma() {
    let node = RubyNode::ItBehavesLike {
        name: "a test".into(),
        params: vec![],
    };
    let output = node.emit(0);
    assert_eq!(output, "it_behaves_like 'a test'");
    assert!(!output.contains(','));
}

// ── Indentation correctness ──────────────────────────────────────

#[test]
fn nested_describe_indentation() {
    let node = RSpecBuilder::describe("'outer'")
        .context("when nested", |b| {
            b.it("indents correctly", |b| b.expect("true", "to be true"))
        })
        .build();
    let output = node.emit(0);

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }
        let spaces = line.len() - line.trim_start().len();
        assert!(spaces % 2 == 0, "line has odd indentation: '{line}'");
    }
}
