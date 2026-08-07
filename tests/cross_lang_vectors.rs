//! Frozen b3sum vectors for RubyType and RbsType canonical emissions.
//!
//! Sibling to `iac-forge/tests/cross_lang_vectors.rs`. Any value of
//! either of these two type enums must emit one of the canonical
//! forms below, and its BLAKE3 content hash must match the pinned
//! hex. Independent `b3sum 1.8.4` verification at commit time.
//!
//! If a vector fails, either canonical emission changed (schema bump
//! required) or the hash function changed. No silent drift.

#![cfg(feature = "iac-bridge")]
// Every proof in this file reaches iac-forge — either `iac_forge::*`
// directly or the `iac_bridge` / sexpr impls, both of which live behind
// this feature. Without the gate the target cannot COMPILE under a plain
// `cargo test`, which is not a skipped test: it is a hard build error
// that takes the whole run down and hides every other target's result.
// Gated rather than made default because `iac-bridge` pulls the
// iac-forge dependency in, and src/sexpr.rs says that is deliberate.

use iac_forge::sexpr::{SExpr, ToSExpr};
use ruby_synthesizer::{RbsType, RubyType};

/// (canonical emission, expected BLAKE3 hex, Rust value that emits it)
///
/// All hashes verified via:
/// ```sh
/// nix-shell -p b3sum --run "printf '%s' '<TEXT>' | b3sum --no-names"
/// ```
#[allow(clippy::type_complexity)]
fn ruby_type_vectors() -> Vec<(&'static str, &'static str, RubyType)> {
    vec![
        (
            "(simple \"T::String\")",
            "9cd5f904c8ed4748eed176b63fde28f7b11487adede8ab2ada8b5163e2cefd7f",
            RubyType::simple("T::String"),
        ),
        (
            "(simple \"T::Integer\")",
            "84783156859c7180a900394cd04aac0d0e9276c232032753015f0e39ae92751c",
            RubyType::simple("T::Integer"),
        ),
        (
            "(array (simple \"T::String\"))",
            "02db9750a4c8fb19c10eb70cdccaf0280c8db950b020e4a4e4359b84db267549",
            RubyType::array(RubyType::simple("T::String")),
        ),
        (
            "hash",
            "b6716efe6829269249e48a93798c6e40058255c268f21566431b8e7ea7da3b15",
            RubyType::Hash,
        ),
        (
            "(union (simple \"T::Coercible::Integer\") (simple \"T::Coercible::Float\"))",
            "f2b4773ee214468ffb3ff8afce9c868189aeb0c97b365d61321ef2321b76bbf6",
            RubyType::union(vec![
                RubyType::simple("T::Coercible::Integer"),
                RubyType::simple("T::Coercible::Float"),
            ]),
        ),
        (
            "(constrained (:base (simple \"T::String\")) (:constraint \"x\"))",
            "6ad4e3ed59b9b3ee13881499ab1e8733d116f93b1a47c5dfee7697ab1f4fe3a3",
            RubyType::constrained(RubyType::simple("T::String"), "x"),
        ),
        (
            "(optional (simple \"T::Integer\"))",
            "cba66661972132ba4bace88af27787463f632bc4f80996fb858163cdaad02f20",
            RubyType::optional(RubyType::simple("T::Integer")),
        ),
        (
            "any",
            "fd0b6c0bab658ae9e3e6bf09032b6aec599d2c78a2b3783afb8fc415078533d1",
            RubyType::Any,
        ),
    ]
}

#[allow(clippy::type_complexity)]
fn rbs_type_vectors() -> Vec<(&'static str, &'static str, RbsType)> {
    vec![
        (
            "(named \"String\")",
            "43201850e62bc40327a13ee4a3df9d3f9e12eaf5ece5d09c13de0463126af312",
            RbsType::named("String"),
        ),
        (
            "(named \"Integer\")",
            "7d3985bbd92921a69e84a0f085e55811e25a205a032d0cd776296a55f9aa3f44",
            RbsType::named("Integer"),
        ),
        (
            "(array (named \"String\"))",
            "5627bb484858b9d1a1760102e3c3dce889ed1857c158ef8870fb2861de93d6e9",
            RbsType::array(RbsType::named("String")),
        ),
        (
            "(hash (named \"Symbol\") untyped)",
            "3193ebd7efc11b842771fa7657fafa815224708f55119d7f4f438ba16d938cc5",
            RbsType::hash(RbsType::named("Symbol"), RbsType::Untyped),
        ),
        (
            "(string-literal \"tcp\")",
            "69e10108139a806ba815e8c95f504f4d8024a9a2ad15fd143c5cab89efb86ac5",
            RbsType::string_literal("tcp"),
        ),
        (
            "(nilable (named \"String\"))",
            "2a9fca7ee885d0fd011006dbe84eabdce8a1034441e9b787f361775a2baf72aa",
            RbsType::nilable(RbsType::named("String")),
        ),
        (
            "untyped",
            "384f22513927f65f8d891c22eadfafc5178043edca0649c5620d07acd3e1ee1f",
            RbsType::Untyped,
        ),
        (
            "(union (string-literal \"tcp\") (string-literal \"udp\"))",
            "e357f7b14b16cfaf8ffbda303debd361cb417e5d7cc230b36ebac5058f72c0ce",
            RbsType::union(vec![
                RbsType::string_literal("tcp"),
                RbsType::string_literal("udp"),
            ]),
        ),
        (
            "(union (named \"A\") (named \"B\"))",
            "4b86eabd9817d06ebd46302e3236e86c36845e336288284837bef957bf71d19d",
            RbsType::union(vec![RbsType::named("A"), RbsType::named("B")]),
        ),
    ]
}

// ── RubyType proofs ─────────────────────────────────────────────────

#[test]
fn ruby_type_emits_canonical_text() {
    for (expected_text, _, value) in ruby_type_vectors() {
        assert_eq!(
            value.to_sexpr().emit(),
            expected_text,
            "emission drift for {value:?}",
        );
    }
}

#[test]
fn ruby_type_hash_matches_frozen_vector() {
    for (_, expected_hex, value) in ruby_type_vectors() {
        assert_eq!(
            value.to_sexpr().content_hash().to_hex(),
            expected_hex,
            "hash drift for {value:?}",
        );
    }
}

#[test]
fn ruby_type_round_trip_preserves_hash() {
    for (text, _, _) in ruby_type_vectors() {
        let parsed = SExpr::parse(text).expect("parse");
        assert_eq!(parsed.emit(), text);
    }
}

// ── RbsType proofs ──────────────────────────────────────────────────

#[test]
fn rbs_type_emits_canonical_text() {
    for (expected_text, _, value) in rbs_type_vectors() {
        assert_eq!(
            value.to_sexpr().emit(),
            expected_text,
            "emission drift for {value:?}",
        );
    }
}

#[test]
fn rbs_type_hash_matches_frozen_vector() {
    for (_, expected_hex, value) in rbs_type_vectors() {
        assert_eq!(
            value.to_sexpr().content_hash().to_hex(),
            expected_hex,
            "hash drift for {value:?}",
        );
    }
}

#[test]
fn rbs_type_round_trip_preserves_hash() {
    for (text, _, _) in rbs_type_vectors() {
        let parsed = SExpr::parse(text).expect("parse");
        assert_eq!(parsed.emit(), text);
    }
}

// ── Cross-enum consistency ──────────────────────────────────────────

#[test]
fn union_of_one_variant_degenerates_in_rust_constructor() {
    // The constructor's single-variant degeneracy is documented; make
    // sure the observable hash agrees with the non-wrapped variant's
    // hash. This is a structural invariant that cross-language
    // implementations must also respect.
    let single_wrapped = RubyType::union(vec![RubyType::simple("T::String")]);
    let direct = RubyType::simple("T::String");
    assert_eq!(
        single_wrapped.to_sexpr().content_hash(),
        direct.to_sexpr().content_hash()
    );
}

#[test]
fn vector_set_not_empty() {
    assert!(ruby_type_vectors().len() >= 8);
    assert!(rbs_type_vectors().len() >= 9);
}
