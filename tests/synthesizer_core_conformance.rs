//! Integration tests proving `RubyNode` conforms to `synthesizer_core` traits.
//!
//! Wave 2 of the compound-knowledge refactor. Every test calls one of
//! `synthesizer_core::node::laws::*` on a real `RubyNode` value, compounding
//! proof surface: the same laws prove properties of every synthesizer that
//! conforms.

use ruby_synthesizer::RubyNode;
use synthesizer_core::node::laws;
use synthesizer_core::{NoRawAttestation, SynthesizerNode};

// ─── Trait shape ────────────────────────────────────────────────────

#[test]
fn indent_unit_is_two_spaces() {
    assert_eq!(<RubyNode as SynthesizerNode>::indent_unit(), "  ");
}

#[test]
fn variant_ids_distinct_across_disjoint_variants() {
    let samples: Vec<RubyNode> = vec![
        RubyNode::FrozenStringLiteral,
        RubyNode::Blank,
        RubyNode::Comment("c".into()),
        RubyNode::Require("dry-struct".into()),
        RubyNode::RequireRelative("./x".into()),
        RubyNode::Include("M".into()),
        RubyNode::RegistryCall("Foo".into()),
        RubyNode::Ident("x".into()),
        RubyNode::SymbolLit("s".into()),
        RubyNode::StringLit("v".into()),
        RubyNode::ArrayLit(vec![]),
        RubyNode::HashLit(vec![]),
        RubyNode::ConstPath(vec!["A".into(), "B".into()]),
        RubyNode::Yield(None),
        RubyNode::RSpecCode("subject.foo".into()),
        RubyNode::InlineModuleDecl(vec!["A".into(), "B".into()]),
    ];
    let before = samples.len();
    let mut ids: Vec<u8> = samples.iter().map(SynthesizerNode::variant_id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(
        ids.len(),
        before,
        "variant_id must be distinct for disjoint variants"
    );
}

// ─── SynthesizerNode laws ───────────────────────────────────────────

#[test]
fn law_determinism_holds_on_simple_nodes() {
    for n in [
        RubyNode::FrozenStringLiteral,
        RubyNode::Blank,
        RubyNode::Comment("x".into()),
        RubyNode::Require("foo".into()),
        RubyNode::Ident("bar".into()),
    ] {
        assert!(laws::is_deterministic(&n, 0));
        assert!(laws::is_deterministic(&n, 3));
    }
}

#[test]
fn law_determinism_holds_on_array_lit() {
    let n = RubyNode::ArrayLit(vec![
        RubyNode::StringLit("a".into()),
        RubyNode::StringLit("b".into()),
    ]);
    assert!(laws::is_deterministic(&n, 2));
}

#[test]
fn law_determinism_holds_on_hash_lit() {
    let n = RubyNode::HashLit(vec![
        ("name".into(), RubyNode::StringLit("x".into())),
        ("count".into(), RubyNode::Ident("42".into())),
    ]);
    assert!(laws::is_deterministic(&n, 1));
}

#[test]
fn law_honors_indent_unit_on_comment() {
    assert!(laws::honors_indent_unit(
        &RubyNode::Comment("hello".into()),
        0
    ));
    assert!(laws::honors_indent_unit(
        &RubyNode::Comment("hello".into()),
        2
    ));
}

#[test]
fn law_indent_monotone_len_on_require() {
    assert!(laws::indent_monotone_len(&RubyNode::Require("x".into()), 0));
    assert!(laws::indent_monotone_len(&RubyNode::Require("x".into()), 3));
}

#[test]
fn law_variant_id_valid_on_all_sample_variants() {
    let samples = [
        RubyNode::FrozenStringLiteral,
        RubyNode::Blank,
        RubyNode::Comment("x".into()),
        RubyNode::Ident("y".into()),
        RubyNode::SymbolLit("z".into()),
        RubyNode::StringLit("s".into()),
        RubyNode::ArrayLit(vec![]),
        RubyNode::HashLit(vec![]),
    ];
    for n in &samples {
        assert!(laws::variant_id_is_valid(n));
    }
}

// ─── NoRawAttestation ───────────────────────────────────────────────

#[test]
fn attestation_is_nonempty() {
    assert!(!<RubyNode as NoRawAttestation>::attestation().is_empty());
}

#[test]
fn attestation_mentions_raw() {
    let s = <RubyNode as NoRawAttestation>::attestation();
    assert!(
        s.to_lowercase().contains("raw"),
        "attestation must explain how no-raw is enforced — got: {s}"
    );
}

// ─── No-raw source invariant ────────────────────────────────────────

#[test]
fn no_raw_constructor_in_production_source() {
    // Scan src/ for `RubyNode::Raw(...)` or `Self::Raw(...)` constructor
    // uses. Legitimate non-constructions (variant declaration, match arms,
    // #[allow(deprecated)]-pinned references, comments, attribute lines)
    // are exempted.
    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();
    for path in walk_rust_files(&src_dir) {
        let content = std::fs::read_to_string(&path).expect("read src file");
        let lines: Vec<&str> = content.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("*") {
                continue;
            }
            // Variant declaration line.
            if line.contains("Raw(String)") {
                continue;
            }
            // Match arms (patterns, not constructions).
            if line.contains("=>") {
                continue;
            }
            // Attribute lines.
            if trimmed.starts_with("#[") {
                continue;
            }
            // Preceding #[allow(deprecated)] → intentional reference.
            let prev_allows = i > 0 && lines[i - 1].contains("#[allow(deprecated)]");
            if prev_allows {
                continue;
            }
            if line.contains("RubyNode::Raw(") || line.contains("Self::Raw(") {
                violations.push(format!("{}:{}", path.display(), i + 1));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "RubyNode::Raw construction in production source is forbidden \
         (use a typed variant). Violations: {violations:?}"
    );
}

fn walk_rust_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root).expect("read src dir") {
        let entry = entry.expect("read dir entry");
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk_rust_files(&path));
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    out
}
