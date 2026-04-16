//! Ruby AST nodes — structural representation of Ruby source code.

use crate::types::RubyType;

/// Pangea output type (display for humans, data for downstream).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PangeaOutputType {
    Display,
    Data,
}

/// A method parameter with optional default value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodParam {
    pub name: String,
    pub default: Option<String>,
}

impl MethodParam {
    pub fn required(name: impl Into<String>) -> Self {
        Self { name: name.into(), default: None }
    }

    pub fn with_default(name: impl Into<String>, default: impl Into<String>) -> Self {
        Self { name: name.into(), default: Some(default.into()) }
    }
}

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

    // ── Pangea DSL ────────────────────────────────────────────────

    /// `template :name do ... end`
    PangeaTemplate {
        name: String,
        body: Vec<RubyNode>,
    },

    /// `provider :name do ... end` or `provider :name, key: value`
    PangeaProvider {
        name: String,
        args: Vec<(String, String)>,
        body: Vec<RubyNode>,
    },

    /// `terraform do ... end`
    PangeaTerraform {
        body: Vec<RubyNode>,
    },

    /// `[receiver.]resource_type(:symbol_name, { key: value, ... })`
    /// e.g., `synth.aws_route53_zone(:name, { name: domain, ... })`
    /// receiver=None → bare call (template context), Some("synth") → synth.call
    PangeaResourceCall {
        receiver: Option<String>,
        resource_type: String,
        symbol: String,
        args: Vec<(String, String)>,
    },

    /// `display_output :name do ... end` or `data_output :name do ... end`
    PangeaOutput {
        output_type: PangeaOutputType,
        name: String,
        value: String,
        description: String,
    },

    /// `Pangea::Secrets.configure(sops_file: ...)`
    PangeaSecretsConfig {
        sops_file: String,
    },

    /// `Pangea::RemoteState.configure(bucket: ..., region: ...)`
    PangeaRemoteStateConfig {
        bucket: String,
        region: String,
    },

    /// `Pangea::RemoteState.output(template: ..., output: ..., state_key: ...)`
    PangeaRemoteStateOutput {
        template: String,
        output: String,
        state_key: String,
    },

    /// `[receiver.]extend(ModuleName) unless [receiver.]respond_to?(:method_name)`
    /// receiver=None → self, Some("synth") → synth
    ExtendModule {
        receiver: Option<String>,
        module_path: String,
        guard_method: Option<String>,
    },

    /// `ENV.fetch('KEY', 'default')` or `ENV['KEY'] ||= value`
    EnvFetch {
        key: String,
        default: Option<String>,
    },

    /// `ENV['KEY'] ||= expr`
    EnvAssign {
        key: String,
        value: String,
    },

    /// `variable = expr`
    Assignment {
        variable: String,
        value: String,
    },

    /// `unless condition ... end`
    Unless {
        condition: String,
        body: Vec<RubyNode>,
    },

    /// `if condition ... end`
    IfBlock {
        condition: String,
        body: Vec<RubyNode>,
    },

    /// `begin ... rescue ExceptionClass ... end`
    BeginRescue {
        body: Vec<RubyNode>,
        rescue_class: String,
        rescue_body: Vec<RubyNode>,
    },

    /// `required_providers({ name: { source: 'source' } })`
    RequiredProviders {
        providers: Vec<(String, String)>,
    },

    /// Generic Ruby block: `header do [|params|] ... end`
    /// Used for any `name do ... end` pattern (group, namespace, gemspec, etc.)
    DoBlock {
        header: String,
        params: Option<String>,
        body: Vec<RubyNode>,
    },

    /// DSL method call without parens: `method_name value`
    /// Used for Ruby DSL setters in block context (provider blocks, etc.)
    DslSetter {
        method: String,
        value: String,
    },

    // ── General-purpose typed nodes ─────────────────────────────

    /// Bare identifier: `some_var`, `self`, `true`, `region`
    Ident(String),

    /// Symbol literal: `:name`
    SymbolLit(String),

    /// Single-quoted string: `'value'`
    StringLit(String),

    /// Array literal: `["a", "b"]`
    ArrayLit(Vec<RubyNode>),

    /// Hash literal: `{ key: value, ... }` (symbol keys)
    HashLit(Vec<(String, RubyNode)>),

    /// Method call: `receiver.method(args)` or `method(args)`
    /// Receiver is optional (bare function call if None).
    Call {
        receiver: Option<Box<RubyNode>>,
        method: String,
        args: Vec<RubyNode>,
    },

    /// Constant / module path: `Pangea::Architectures::SecureVpc`
    ConstPath(Vec<String>),

    /// `.merge(hash)` chain — `expr.merge(key: val)`
    MergeCall {
        receiver: Box<RubyNode>,
        hash: Vec<(String, RubyNode)>,
    },

    // ── Architecture generation nodes ───────────────────────────────

    /// `def self.method_name(param1, param2 = default) ... end`
    MethodDef {
        receiver: Option<String>,
        name: String,
        params: Vec<MethodParam>,
        body: Vec<RubyNode>,
    },

    /// `NAME = { 'key' => 'value', ... }.freeze`
    FrozenConstHash {
        name: String,
        entries: Vec<(String, String)>,
    },

    /// Arbitrary Ruby expression in RSpec context — typed bridge for test code.
    RSpecCode(String),

    /// Raw Ruby expression — DEPRECATED: use a typed variant instead.
    #[deprecated(note = "use a typed variant instead of Raw — Raw defeats provability")]
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
            Self::Comment(text) => {
                if text.contains('\n') {
                    text.lines()
                        .map(|line| format!("{pad}# {line}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    format!("{pad}# {text}")
                }
            }
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

            // ── Pangea DSL ────────────────────────────────────────────

            Self::PangeaTemplate { name, body } => {
                let mut out = format!("{pad}template :{name} do\n");
                for node in body {
                    out.push_str(&node.emit(indent + 1));
                    out.push('\n');
                }
                out.push_str(&format!("{pad}end"));
                out
            }

            Self::PangeaProvider { name, args, body } => {
                if body.is_empty() && !args.is_empty() {
                    let args_str = args.iter()
                        .map(|(k, v)| format!("{k}: {v}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{pad}provider :{name}, {args_str}")
                } else {
                    let mut out = format!("{pad}provider :{name} do\n");
                    for node in body {
                        out.push_str(&node.emit(indent + 1));
                        out.push('\n');
                    }
                    out.push_str(&format!("{pad}end"));
                    out
                }
            }

            Self::PangeaTerraform { body } => {
                let mut out = format!("{pad}terraform do\n");
                for node in body {
                    out.push_str(&node.emit(indent + 1));
                    out.push('\n');
                }
                out.push_str(&format!("{pad}end"));
                out
            }

            Self::PangeaResourceCall { receiver, resource_type, symbol, args } => {
                let rcv = match receiver {
                    Some(r) => format!("{r}."),
                    None => String::new(),
                };
                if args.is_empty() {
                    format!("{pad}{rcv}{resource_type}(:\"{symbol}\", {{}})")
                } else {
                    let args_str = args.iter()
                        .map(|(k, v)| format!("{pad}  {k}: {v}"))
                        .collect::<Vec<_>>()
                        .join(",\n");
                    format!("{pad}{rcv}{resource_type}(:\"{symbol}\", {{\n{args_str},\n{pad}}})")
                }
            }

            Self::PangeaOutput { output_type, name, value, description } => {
                let method = match output_type {
                    PangeaOutputType::Display => "display_output",
                    PangeaOutputType::Data => "data_output",
                };
                format!(
                    "{pad}{method} :{name} do\n{pad}  value {value}\n{pad}  description \"{description}\"\n{pad}end"
                )
            }

            Self::PangeaSecretsConfig { sops_file } => {
                format!(
                    "{pad}Pangea::Secrets.configure(\n{pad}  sops_file: File.expand_path('{sops_file}', __dir__),\n{pad})"
                )
            }

            Self::PangeaRemoteStateConfig { bucket, region } => {
                format!(
                    "{pad}Pangea::RemoteState.configure(bucket: {bucket}, region: {region})"
                )
            }

            Self::PangeaRemoteStateOutput { template, output, state_key } => {
                format!(
                    "{pad}Pangea::RemoteState.output(template: '{template}', output: :{output}, state_key: '{state_key}')"
                )
            }

            Self::ExtendModule { receiver, module_path, guard_method } => {
                let rcv = receiver.as_deref().unwrap_or("self");
                match guard_method {
                    Some(method) => format!("{pad}{rcv}.extend({module_path}) unless {rcv}.respond_to?(:{method})"),
                    None => format!("{pad}{rcv}.extend({module_path})"),
                }
            }

            Self::EnvFetch { key, default } => {
                match default {
                    Some(d) => format!("{pad}ENV.fetch('{key}', '{d}')"),
                    None => format!("{pad}ENV.fetch('{key}')"),
                }
            }

            Self::EnvAssign { key, value } => {
                format!("{pad}ENV['{key}'] ||= {value}")
            }

            Self::Assignment { variable, value } => {
                format!("{pad}{variable} = {value}")
            }

            Self::Unless { condition, body } => {
                let mut out = format!("{pad}unless {condition}\n");
                for node in body {
                    out.push_str(&node.emit(indent + 1));
                    out.push('\n');
                }
                out.push_str(&format!("{pad}end"));
                out
            }

            Self::IfBlock { condition, body } => {
                let mut out = format!("{pad}if {condition}\n");
                for node in body {
                    out.push_str(&node.emit(indent + 1));
                    out.push('\n');
                }
                out.push_str(&format!("{pad}end"));
                out
            }

            Self::BeginRescue { body, rescue_class, rescue_body } => {
                let mut out = format!("{pad}begin\n");
                for node in body {
                    out.push_str(&node.emit(indent + 1));
                    out.push('\n');
                }
                out.push_str(&format!("{pad}rescue {rescue_class}\n"));
                for node in rescue_body {
                    out.push_str(&node.emit(indent + 1));
                    out.push('\n');
                }
                out.push_str(&format!("{pad}end"));
                out
            }

            Self::RequiredProviders { providers } => {
                if providers.is_empty() {
                    return format!("{pad}required_providers({{}})");
                }
                let providers_str = providers.iter()
                    .map(|(name, source)| format!("{pad}  {name}: {{ source: '{source}' }}"))
                    .collect::<Vec<_>>()
                    .join(",\n");
                format!("{pad}required_providers({{\n{providers_str},\n{pad}}})")
            }

            Self::DoBlock { header, params, body } => {
                let params_str = match params {
                    Some(p) => format!(" |{p}|"),
                    None => String::new(),
                };
                let mut out = format!("{pad}{header} do{params_str}\n");
                for node in body {
                    out.push_str(&node.emit(indent + 1));
                    out.push('\n');
                }
                out.push_str(&format!("{pad}end"));
                out
            }
            Self::DslSetter { method, value } => {
                if value.is_empty() {
                    format!("{pad}{method}")
                } else {
                    format!("{pad}{method} {value}")
                }
            }
            // General-purpose typed nodes
            Self::Ident(name) => format!("{pad}{name}"),
            Self::SymbolLit(name) => format!("{pad}:{name}"),
            Self::StringLit(val) => format!("{pad}'{val}'"),
            Self::ArrayLit(elements) => {
                let inner: Vec<String> = elements.iter().map(|e| e.emit(0)).collect();
                format!("{pad}[{}]", inner.join(", "))
            }
            Self::HashLit(pairs) => {
                if pairs.is_empty() {
                    format!("{pad}{{}}")
                } else if pairs.len() <= 3 {
                    let inner: Vec<String> = pairs.iter()
                        .map(|(k, v)| format!("{k}: {}", v.emit(0)))
                        .collect();
                    format!("{pad}{{ {} }}", inner.join(", "))
                } else {
                    let inner_pad = "  ".repeat(indent + 1);
                    let mut out = format!("{pad}{{\n");
                    for (k, v) in pairs {
                        out.push_str(&format!("{inner_pad}{k}: {},\n", v.emit(0)));
                    }
                    out.push_str(&format!("{pad}}}"));
                    out
                }
            }
            Self::Call { receiver, method, args } => {
                let rcv = match receiver {
                    Some(r) => format!("{}.", r.emit(0)),
                    None => String::new(),
                };
                if args.is_empty() {
                    format!("{pad}{rcv}{method}")
                } else {
                    let arg_strs: Vec<String> = args.iter().map(|a| a.emit(0)).collect();
                    format!("{pad}{rcv}{method}({})", arg_strs.join(", "))
                }
            }
            Self::ConstPath(parts) => format!("{pad}{}", parts.join("::")),
            Self::MergeCall { receiver, hash } => {
                let inner: Vec<String> = hash.iter()
                    .map(|(k, v)| format!("{k}: {}", v.emit(0)))
                    .collect();
                format!("{pad}{}.merge({})", receiver.emit(0), inner.join(", "))
            }

            // Architecture generation nodes
            Self::MethodDef { receiver, name, params, body } => {
                let rcv = match receiver {
                    Some(r) => format!("{r}."),
                    None => String::new(),
                };
                let params_str = params.iter()
                    .map(|p| match &p.default {
                        Some(d) => format!("{} = {d}", p.name),
                        None => p.name.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut out = format!("{pad}def {rcv}{name}({params_str})\n");
                for node in body {
                    out.push_str(&node.emit(indent + 1));
                    out.push('\n');
                }
                out.push_str(&format!("{pad}end"));
                out
            }

            Self::FrozenConstHash { name, entries } => {
                if entries.is_empty() {
                    format!("{pad}{name} = {{}}.freeze")
                } else if entries.len() <= 2 {
                    let pairs: Vec<String> = entries.iter()
                        .map(|(k, v)| format!("'{k}' => '{v}'"))
                        .collect();
                    format!("{pad}{name} = {{ {} }}.freeze", pairs.join(", "))
                } else {
                    let inner_pad = "  ".repeat(indent + 1);
                    let mut out = format!("{pad}{name} = {{\n");
                    for (k, v) in entries {
                        out.push_str(&format!("{inner_pad}'{k}' => '{v}',\n"));
                    }
                    out.push_str(&format!("{pad}}}.freeze"));
                    out
                }
            }

            Self::RSpecCode(code) => format!("{pad}{code}"),
            #[allow(deprecated)]
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

    // ── Architecture generation nodes ──────────────────────

    #[test]
    fn method_def_with_defaults() {
        let node = RubyNode::MethodDef {
            receiver: Some("self".into()),
            name: "build".into(),
            params: vec![
                MethodParam::required("synth"),
                MethodParam::with_default("config", "{}"),
            ],
            body: vec![
                RubyNode::Assignment {
                    variable: "config".into(),
                    value: "Types::Config.new(config).to_h".into(),
                },
            ],
        };
        let output = node.emit(2);
        assert!(output.contains("def self.build(synth, config = {})"));
        assert!(output.contains("    config = Types::Config.new(config).to_h"));
        assert!(output.ends_with("  end"));
    }

    #[test]
    fn method_def_without_receiver() {
        let node = RubyNode::MethodDef {
            receiver: None,
            name: "initialize".into(),
            params: vec![MethodParam::required("name")],
            body: vec![],
        };
        let output = node.emit(0);
        assert!(output.starts_with("def initialize(name)"));
    }

    #[test]
    fn frozen_const_hash_empty() {
        let node = RubyNode::FrozenConstHash {
            name: "CONTROLS".into(),
            entries: vec![],
        };
        assert_eq!(node.emit(0), "CONTROLS = {}.freeze");
    }

    #[test]
    fn frozen_const_hash_small() {
        let node = RubyNode::FrozenConstHash {
            name: "CONTROLS".into(),
            entries: vec![
                ("SC-7".into(), "NIST 800-53".into()),
            ],
        };
        assert_eq!(node.emit(0), "CONTROLS = { 'SC-7' => 'NIST 800-53' }.freeze");
    }

    #[test]
    fn frozen_const_hash_multiline() {
        let node = RubyNode::FrozenConstHash {
            name: "COMPLIANCE_CONTROLS".into(),
            entries: vec![
                ("SC-7".into(), "NIST 800-53".into()),
                ("CIS-5.3".into(), "CIS AWS v3".into()),
                ("PCI-1.2.1".into(), "PCI DSS 4.0".into()),
            ],
        };
        let output = node.emit(2);
        assert!(output.contains("COMPLIANCE_CONTROLS = {"));
        assert!(output.contains("      'SC-7' => 'NIST 800-53',"));
        assert!(output.contains("      'PCI-1.2.1' => 'PCI DSS 4.0',"));
        assert!(output.contains("    }.freeze"));
    }

    #[test]
    fn frozen_const_hash_deterministic() {
        let entries = vec![
            ("A".into(), "1".into()),
            ("B".into(), "2".into()),
            ("C".into(), "3".into()),
        ];
        let a = RubyNode::FrozenConstHash { name: "X".into(), entries: entries.clone() }.emit(0);
        let b = RubyNode::FrozenConstHash { name: "X".into(), entries }.emit(0);
        assert_eq!(a, b);
    }

    #[test]
    fn method_def_with_body() {
        let node = RubyNode::MethodDef {
            receiver: Some("self".into()),
            name: "build".into(),
            params: vec![
                MethodParam::required("synth"),
                MethodParam::with_default("config", "{}"),
            ],
            body: vec![
                RubyNode::ExtendModule {
                    receiver: None,
                    module_path: "Pangea::Resources::AWS".into(),
                    guard_method: Some("aws_vpc".into()),
                },
                RubyNode::Blank,
                RubyNode::PangeaResourceCall {
                    receiver: None,
                    resource_type: "aws_ebs_encryption_by_default".into(),
                    symbol: "ebs-encryption".into(),
                    args: vec![("enabled".into(), "true".into())],
                },
            ],
        };
        let output = node.emit(0);
        assert!(output.contains("def self.build(synth, config = {})"));
        assert!(output.contains("self.extend(Pangea::Resources::AWS) unless self.respond_to?(:aws_vpc)"));
        assert!(output.contains("aws_ebs_encryption_by_default"));
        assert!(output.contains("end"));
    }
}
