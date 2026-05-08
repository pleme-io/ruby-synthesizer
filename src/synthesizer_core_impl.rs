//! Conformance to [`synthesizer_core`] traits.
//!
//! Wave 2 of the compound-knowledge refactor: purely additive. No behavior
//! change to ruby-synthesizer's existing APIs — this module only adds trait
//! impls that downstream generic code can consume.

use crate::node::RubyNode;
use synthesizer_core::{NoRawAttestation, SynthesizerNode};

impl SynthesizerNode for RubyNode {
    fn emit(&self, indent: usize) -> String {
        // Delegate to the inherent `RubyNode::emit` — inherent takes
        // priority over trait methods in UFCS path lookup.
        RubyNode::emit(self, indent)
    }

    fn indent_unit() -> &'static str {
        "  "
    }

    fn variant_id(&self) -> u8 {
        match self {
            Self::FrozenStringLiteral => 0,
            Self::Comment(_) => 1,
            Self::Blank => 2,
            Self::Require(_) => 3,
            Self::RequireRelative(_) => 4,
            Self::Module { .. } => 5,
            Self::InlineModuleDecl(_) => 6,
            Self::Class { .. } => 7,
            Self::Include(_) => 8,
            Self::ConstAssign { .. } => 9,
            Self::ConstAssignNode { .. } => 9,
            Self::HashRocketLit(_) => 9,
            Self::Attribute { .. } => 10,
            Self::DefineResource { .. } => 11,
            Self::DefineData { .. } => 12,
            Self::RegistryCall(_) => 13,
            Self::PangeaTemplate { .. } => 14,
            Self::PangeaProvider { .. } => 15,
            Self::PangeaTerraform { .. } => 16,
            Self::PangeaResourceCall { .. } => 17,
            Self::PangeaOutput { .. } => 18,
            Self::PangeaSecretsConfig { .. } => 19,
            Self::PangeaRemoteStateConfig { .. } => 20,
            Self::PangeaRemoteStateOutput { .. } => 21,
            Self::ExtendModule { .. } => 22,
            Self::EnvFetch { .. } => 23,
            Self::EnvAssign { .. } => 24,
            Self::Assignment { .. } => 25,
            Self::Unless { .. } => 26,
            Self::IfBlock { .. } => 27,
            Self::BeginRescue { .. } => 28,
            Self::RequiredProviders { .. } => 29,
            Self::DoBlock { .. } => 30,
            Self::DslSetter { .. } => 31,
            Self::Ident(_) => 32,
            Self::SymbolLit(_) => 33,
            Self::StringLit(_) => 34,
            Self::ArrayLit(_) => 35,
            Self::HashLit(_) => 36,
            Self::Call { .. } => 37,
            Self::ConstPath(_) => 38,
            Self::MergeCall { .. } => 39,
            Self::MethodDef { .. } => 40,
            Self::FrozenConstHash { .. } => 41,
            Self::AttrAccessor { .. } => 42,
            Self::Yield(_) => 43,
            Self::Alias { .. } => 44,
            Self::RSpecCode(_) => 45,
            Self::BodyLines(_) => 46,
            Self::Describe { .. } => 47,
            Self::SharedExamples { .. } => 48,
            Self::Context { .. } => 49,
            Self::It { .. } => 50,
            Self::ItBehavesLike { .. } => 51,
            Self::Let { .. } => 52,
            Self::Expect { .. } => 53,
        }
    }
}

impl NoRawAttestation for RubyNode {
    fn attestation() -> &'static str {
        "RubyNode::Raw was REMOVED in Wave 3 of the compound-knowledge \
         refactor. The no-raw invariant is now STRUCTURAL: RubyNode cannot \
         represent arbitrary raw strings because no such variant exists — \
         invalid states are unrepresentable at the type level. \
         tests/synthesizer_core_conformance.rs::no_raw_constructor_in_production_source \
         remains as a defensive scanner guarding against accidental \
         reintroduction. RSpecCode and BodyLines are narrow typed bridges \
         for specific content categories (RSpec test code, and multi-line \
         body output from higher-level proven generators respectively); \
         they are not raw escape hatches, and they cannot stand in for \
         arbitrary text at arbitrary structural positions."
    }
}
