//! Ruby AST nodes — structural representation of Ruby source code.

use crate::types::RubyType;

/// A Ruby source code node. Compose these to build structurally correct Ruby.
///
/// Each variant maps to exactly one Ruby construct. The emitter produces
/// correctly indented, syntactically valid Ruby from any tree of nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RubyNode {
    // ── Pragmas & comments ─────────────────────────────────────────

    /// `# frozen_string_literal: true`
    FrozenStringLiteral,

    /// `# comment text`
    Comment(String),

    /// Blank line separator
    Blank,

    // ── Imports ────────────────────────────────────────────────────

    /// `require 'path'`
    Require(String),

    /// `require_relative 'path'`
    RequireRelative(String),

    // ── Declarations ──────────────────────────────────────────────

    /// `module Path::To::Mod ... end`
    Module {
        path: Vec<String>,
        body: Vec<RubyNode>,
    },

    /// `module A; module B; module C; end; end; end`
    InlineModuleDecl(Vec<String>),

    /// `class Name < Parent ... end`
    Class {
        name: String,
        parent: Option<String>,
        body: Vec<RubyNode>,
    },

    // ── Statements ────────────────────────────────────────────────

    /// `include ModuleName`
    Include(String),

    /// `T = Pangea::Resources::Provider::Types` (constant assignment)
    ConstAssign {
        name: String,
        value: String,
    },

    /// `attribute :name, Type` or `attribute? :name, Type.optional`
    Attribute {
        name: String,
        type_expr: RubyType,
        required: bool,
    },

    /// `define_resource :type, attrs_class: ..., outputs: { ... }, map: [...], ...`
    DefineResource {
        tf_type: String,
        attrs_class: String,
        outputs: Vec<(String, String)>,
        map: Vec<String>,
        map_present: Vec<String>,
        map_bool: Vec<String>,
    },

    /// `define_data :type, attrs_class: ..., outputs: { ... }, map: [...], ...`
    DefineData {
        tf_type: String,
        attrs_class: String,
        outputs: Vec<(String, String)>,
        map: Vec<String>,
        map_present: Vec<String>,
        map_bool: Vec<String>,
    },

    /// `Pangea::ResourceRegistry.register_module(Module)`
    RegistryCall(String),

    /// Raw Ruby expression (escape hatch — use sparingly)
    Raw(String),

    // ── RSpec ──────────────────────────────────────────────────────

    /// `RSpec.describe 'subject' do ... end`
    Describe {
        subject: String,
        body: Vec<RubyNode>,
    },

    /// `RSpec.shared_examples 'name' do |params| ... end`
    SharedExamples {
        name: String,
        params: String,
        body: Vec<RubyNode>,
    },

    /// `context 'name' do ... end`
    Context {
        name: String,
        body: Vec<RubyNode>,
    },

    /// `it 'name' do ... end`
    It {
        name: String,
        body: Vec<RubyNode>,
    },

    /// `it_behaves_like 'name', key: value, ...`
    ItBehavesLike {
        name: String,
        params: Vec<(String, String)>,
    },

    /// `let(:name) { expr }`
    Let {
        name: String,
        expr: String,
    },

    /// `expect(subject).matcher`
    Expect {
        subject: String,
        matcher: String,
    },
}

impl RubyNode {
    /// Emit this node as Ruby source code with the given indentation level.
    #[must_use]
    pub fn emit(&self, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        match self {
            // Pragmas & comments
            Self::FrozenStringLiteral => "# frozen_string_literal: true".to_string(),
            Self::Comment(text) => format!("{pad}# {text}"),
            Self::Blank => String::new(),

            // Imports
            Self::Require(path) => format!("{pad}require '{path}'"),
            Self::RequireRelative(path) => format!("{pad}require_relative '{path}'"),

            // Module
            Self::Module { path, body } => {
                let module_path = path.join("::");
                let mut out = format!("{pad}module {module_path}\n");
                for node in body {
                    out.push_str(&node.emit(indent + 1));
                    out.push('\n');
                }
                out.push_str(&format!("{pad}end"));
                out
            }

            Self::InlineModuleDecl(parts) => {
                let opens: Vec<String> = parts.iter().map(|p| format!("module {p}")).collect();
                let closes = "end; ".repeat(parts.len());
                format!("{pad}{}; {}", opens.join("; "), closes.trim_end_matches("; "))
            }

            // Class
            Self::Class { name, parent, body } => {
                let parent_str = parent.as_ref().map_or(String::new(), |p| format!(" < {p}"));
                let mut out = format!("{pad}class {name}{parent_str}\n");
                for node in body {
                    out.push_str(&node.emit(indent + 1));
                    out.push('\n');
                }
                out.push_str(&format!("{pad}end"));
                out
            }

            // Statements
            Self::Include(module_name) => format!("{pad}include {module_name}"),

            Self::ConstAssign { name, value } => format!("{pad}{name} = {value}"),

            Self::Attribute { name, type_expr, required } => {
                let keyword = if *required { "attribute" } else { "attribute?" };
                let ty = if *required {
                    type_expr.emit()
                } else {
                    RubyType::optional(type_expr.clone()).emit()
                };
                format!("{pad}{keyword} :{name}, {ty}")
            }

            Self::DefineResource { tf_type, attrs_class, outputs, map, map_present, map_bool } |
            Self::DefineData { tf_type, attrs_class, outputs, map, map_present, map_bool } => {
                let kind = if matches!(self, Self::DefineResource { .. }) {
                    "define_resource"
                } else {
                    "define_data"
                };
                let mut out = format!("{pad}{kind} :{tf_type},\n");
                out.push_str(&format!("{pad}  attributes_class: {attrs_class},\n"));

                // outputs
                let output_pairs: Vec<String> = outputs
                    .iter()
                    .map(|(k, v)| format!("{k}: :{v}"))
                    .collect();
                out.push_str(&format!("{pad}  outputs: {{ {} }}", output_pairs.join(", ")));

                // map categories
                if !map.is_empty() {
                    let syms = format_symbol_list(map);
                    out.push_str(&format!(",\n{pad}  map: [{syms}]"));
                }
                if !map_present.is_empty() {
                    let syms = format_symbol_list(map_present);
                    out.push_str(&format!(",\n{pad}  map_present: [{syms}]"));
                }
                if !map_bool.is_empty() {
                    let syms = format_symbol_list(map_bool);
                    out.push_str(&format!(",\n{pad}  map_bool: [{syms}]"));
                }

                out
            }

            Self::RegistryCall(module_path) => {
                format!("{pad}Pangea::ResourceRegistry.register_module({module_path})")
            }

            Self::Raw(code) => format!("{pad}{code}"),

            // RSpec
            Self::Describe { subject, body } => {
                let mut out = format!("{pad}RSpec.describe {subject} do\n");
                for node in body {
                    out.push_str(&node.emit(indent + 1));
                    out.push('\n');
                }
                out.push_str(&format!("{pad}end"));
                out
            }

            Self::SharedExamples { name, params, body } => {
                let mut out = format!("{pad}RSpec.shared_examples '{name}' do |{params}|\n");
                for node in body {
                    out.push_str(&node.emit(indent + 1));
                    out.push('\n');
                }
                out.push_str(&format!("{pad}end"));
                out
            }

            Self::Context { name, body } => {
                let mut out = format!("{pad}context '{name}' do\n");
                for node in body {
                    out.push_str(&node.emit(indent + 1));
                    out.push('\n');
                }
                out.push_str(&format!("{pad}end"));
                out
            }

            Self::It { name, body } => {
                if body.is_empty() {
                    format!("{pad}it '{name}' do\n{pad}end")
                } else {
                    let mut out = format!("{pad}it '{name}' do\n");
                    for node in body {
                        out.push_str(&node.emit(indent + 1));
                        out.push('\n');
                    }
                    out.push_str(&format!("{pad}end"));
                    out
                }
            }

            Self::ItBehavesLike { name, params } => {
                if params.is_empty() {
                    format!("{pad}it_behaves_like '{name}'")
                } else {
                    let mut out = format!("{pad}it_behaves_like '{name}',\n");
                    for (i, (k, v)) in params.iter().enumerate() {
                        let comma = if i < params.len() - 1 { "," } else { "" };
                        out.push_str(&format!("{pad}  {k}: {v}{comma}\n"));
                    }
                    out.trim_end_matches('\n').to_string()
                }
            }

            Self::Let { name, expr } => {
                format!("{pad}let(:{name}) {{ {expr} }}")
            }

            Self::Expect { subject, matcher } => {
                format!("{pad}expect({subject}).{matcher}")
            }
        }
    }
}

/// Format a list of strings as Ruby symbols: `[:a, :b, :c]`
fn format_symbol_list(names: &[String]) -> String {
    names.iter().map(|n| format!(":{n}")).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_string_literal() {
        assert_eq!(
            RubyNode::FrozenStringLiteral.emit(0),
            "# frozen_string_literal: true"
        );
    }

    #[test]
    fn require_statement() {
        assert_eq!(
            RubyNode::Require("dry-struct".into()).emit(0),
            "require 'dry-struct'"
        );
    }

    #[test]
    fn module_with_class() {
        let node = RubyNode::Module {
            path: vec!["Pangea".into(), "Resources".into(), "Porkbun".into(), "Types".into()],
            body: vec![
                RubyNode::Include("Dry.Types()".into()),
                RubyNode::Blank,
                RubyNode::Class {
                    name: "NameserversAttributes".into(),
                    parent: Some("Pangea::Resources::BaseAttributes".into()),
                    body: vec![
                        RubyNode::ConstAssign {
                            name: "T".into(),
                            value: "Pangea::Resources::Porkbun::Types".into(),
                        },
                        RubyNode::Blank,
                        RubyNode::Attribute {
                            name: "domain".into(),
                            type_expr: RubyType::simple("T::String"),
                            required: true,
                        },
                        RubyNode::Attribute {
                            name: "nameservers".into(),
                            type_expr: RubyType::array(RubyType::simple("T::String")),
                            required: true,
                        },
                    ],
                },
            ],
        };

        let output = node.emit(0);
        assert!(output.contains("module Pangea::Resources::Porkbun::Types"));
        assert!(output.contains("include Dry.Types()"));
        assert!(output.contains("class NameserversAttributes < Pangea::Resources::BaseAttributes"));
        assert!(output.contains("attribute :domain, T::String"));
        assert!(output.contains("attribute :nameservers, T::Array.of(T::String)"));
        assert!(output.contains("end")); // closes class
    }

    #[test]
    fn optional_attribute() {
        let node = RubyNode::Attribute {
            name: "description".into(),
            type_expr: RubyType::simple("T::String"),
            required: false,
        };
        assert_eq!(node.emit(2), "    attribute? :description, T::String.optional");
    }

    #[test]
    fn define_resource() {
        let node = RubyNode::DefineResource {
            tf_type: "porkbun_nameservers".into(),
            attrs_class: "Porkbun::Types::NameserversAttributes".into(),
            outputs: vec![("id".into(), "id".into())],
            map: vec!["domain".into(), "nameservers".into()],
            map_present: vec![],
            map_bool: vec![],
        };
        let output = node.emit(2);
        assert!(output.contains("define_resource :porkbun_nameservers"));
        assert!(output.contains("attributes_class: Porkbun::Types::NameserversAttributes"));
        assert!(output.contains("outputs: { id: :id }"));
        assert!(output.contains("map: [:domain, :nameservers]"));
    }

    #[test]
    fn inline_module_decl() {
        let node = RubyNode::InlineModuleDecl(vec![
            "Pangea".into(), "Resources".into(), "AWS".into(), "Types".into(),
        ]);
        assert_eq!(
            node.emit(0),
            "module Pangea; module Resources; module AWS; module Types; end; end; end; end"
        );
    }

    #[test]
    fn rspec_describe_with_it() {
        let node = RubyNode::Describe {
            subject: "'my test'".into(),
            body: vec![
                RubyNode::It {
                    name: "does something".into(),
                    body: vec![
                        RubyNode::Expect {
                            subject: "result".into(),
                            matcher: "to eq(42)".into(),
                        },
                    ],
                },
            ],
        };
        let output = node.emit(0);
        assert!(output.contains("RSpec.describe 'my test' do"));
        assert!(output.contains("  it 'does something' do"));
        assert!(output.contains("    expect(result).to eq(42)"));
    }

    #[test]
    fn it_behaves_like_with_params() {
        let node = RubyNode::ItBehavesLike {
            name: "a pure typed provider".into(),
            params: vec![
                ("provider_module".into(), "Pangea::Resources::Porkbun".into()),
                ("types_module".into(), "Pangea::Resources::Porkbun::Types".into()),
                ("lib_path".into(), "File.expand_path('../../lib', __dir__)".into()),
            ],
        };
        let output = node.emit(0);
        assert!(output.contains("it_behaves_like 'a pure typed provider'"));
        assert!(output.contains("  provider_module: Pangea::Resources::Porkbun,"));
        assert!(output.contains("  lib_path: File.expand_path('../../lib', __dir__)"));
    }

    #[test]
    fn registry_call() {
        let node = RubyNode::RegistryCall("Pangea::Resources::Porkbun".into());
        assert_eq!(
            node.emit(0),
            "Pangea::ResourceRegistry.register_module(Pangea::Resources::Porkbun)"
        );
    }
}
