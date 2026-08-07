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
        Self {
            name: name.into(),
            default: None,
        }
    }

    pub fn with_default(name: impl Into<String>, default: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            default: Some(default.into()),
        }
    }
}

/// A Ruby source code node. Compose these to build structurally correct Ruby.
///
/// Each variant maps to exactly one Ruby construct. The emitter produces
/// correctly indented, syntactically valid Ruby from any tree of nodes.
/// Accessor mode: reader, writer, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessorMode {
    Reader,
    Writer,
    Accessor,
}

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
    ConstAssign { name: String, value: String },

    /// `NAME = <typed RubyNode expression>` — typed peer of
    /// [`ConstAssign`]. Use when the right-hand side is a structured
    /// value (ArrayLit, HashLit, Call chain) that would otherwise
    /// require a stringly-typed escape. Emits the value via
    /// `RubyNode::emit_expr` at the same indent as `ConstAssign`.
    ///
    /// Example:
    /// ```ignore
    /// use ruby_synthesizer::{RubyNode};
    /// let node = RubyNode::ConstAssignNode {
    ///     name: "DEFAULT_NODE_GROUPS".into(),
    ///     value: Box::new(RubyNode::Call {
    ///         receiver: Some(Box::new(RubyNode::ArrayLit(vec![]))),
    ///         method: "freeze".into(),
    ///         args: vec![],
    ///     }),
    /// };
    /// assert!(node.emit(0).contains("DEFAULT_NODE_GROUPS"));
    /// assert!(node.emit(0).contains(".freeze"));
    /// ```
    ConstAssignNode { name: String, value: Box<RubyNode> },

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
    PangeaTemplate { name: String, body: Vec<RubyNode> },

    /// `provider :name do ... end` or `provider :name, key: value`
    PangeaProvider {
        name: String,
        args: Vec<(String, String)>,
        body: Vec<RubyNode>,
    },

    /// `terraform do ... end`
    PangeaTerraform { body: Vec<RubyNode> },

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
    PangeaSecretsConfig { sops_file: String },

    /// `Pangea::RemoteState.configure(bucket: ..., region: ...)`
    PangeaRemoteStateConfig { bucket: String, region: String },

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
    EnvAssign { key: String, value: String },

    /// `variable = expr`
    Assignment { variable: String, value: String },

    /// `variable = <typed RubyNode expression>` — typed peer of
    /// [`Assignment`], exactly as [`ConstAssignNode`] is the typed peer of
    /// [`ConstAssign`]. Use whenever the right-hand side is a Ruby *value*
    /// (a string literal, an array, an index call) rather than an opaque
    /// fragment, so the emitter — not the caller — owns quoting and escaping.
    ///
    /// The left-hand side stays a `String` because an assignment target is
    /// not an expression: `s.metadata['k'] = v` has no value-node reading.
    ///
    /// ```ignore
    /// use ruby_synthesizer::RubyNode;
    /// let node = RubyNode::AssignmentNode {
    ///     variable: "s.name".into(),
    ///     value: Box::new(RubyNode::StringLit("pangea-platform".into())),
    /// };
    /// assert_eq!(node.emit(0), "s.name = 'pangea-platform'");
    /// ```
    AssignmentNode {
        variable: String,
        value: Box<RubyNode>,
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
    RequiredProviders { providers: Vec<(String, String)> },

    /// Generic Ruby block: `header do [|params|] ... end`
    /// Used for any `name do ... end` pattern (group, namespace, gemspec, etc.)
    DoBlock {
        header: String,
        params: Option<String>,
        body: Vec<RubyNode>,
    },

    /// DSL method call without parens: `method_name value`
    /// Used for Ruby DSL setters in block context (provider blocks, etc.)
    DslSetter { method: String, value: String },

    /// `method_name arg, arg, key: arg` — typed peer of [`DslSetter`],
    /// paren-less like it, but taking an argument *list* of typed nodes
    /// instead of one pre-rendered string. Empty args emit the bare method
    /// name (`gemspec`), matching `DslSetter`'s empty-value behaviour.
    ///
    /// This is what a gemspec or Gemfile line actually is — a call with
    /// arguments — so `s.add_dependency 'pangea-core', '~> 0.2'` and
    /// `gem 'pangea-core', path: '../pangea-core'` become node trees rather
    /// than comma-joined strings. Pair with [`RubyNode::KeywordArg`] for the
    /// trailing `key: value` form.
    ///
    /// ```ignore
    /// use ruby_synthesizer::RubyNode;
    /// let node = RubyNode::DslCall {
    ///     method: "s.add_dependency".into(),
    ///     args: vec![
    ///         RubyNode::StringLit("pangea-core".into()),
    ///         RubyNode::StringLit("~> 0.2".into()),
    ///     ],
    /// };
    /// assert_eq!(node.emit(0), "s.add_dependency 'pangea-core', '~> 0.2'");
    /// ```
    DslCall { method: String, args: Vec<RubyNode> },

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

    /// Hash-rocket literal: `{ 'key' => value, ... }` — emits the keys
    /// as single-quoted string literals with `=>` separators. Use when
    /// keys contain characters not legal in a Ruby symbol (dots,
    /// slashes, hyphens): `'pleme.io/pool' => 'system'`.
    HashRocketLit(Vec<(String, RubyNode)>),

    /// Method call: `receiver.method(args)` or `method(args)`
    /// Receiver is optional (bare function call if None).
    Call {
        receiver: Option<Box<RubyNode>>,
        method: String,
        args: Vec<RubyNode>,
    },

    /// Index / element reference: `receiver[index]` — e.g. `Dir['lib/**/*.rb']`,
    /// `ENV['HOME']`, `h[:key]`.
    ///
    /// [`Call`] cannot express this: it always emits dot-and-parens, so the
    /// nearest it gets is `Dir.[]('lib/**/*.rb')`. Bracket indexing is a
    /// distinct Ruby surface form with no prior representation here, which is
    /// why it is a variant rather than a builder over `Call`.
    IndexCall {
        receiver: Box<RubyNode>,
        index: Box<RubyNode>,
    },

    /// A single bare keyword argument: `name: value`.
    ///
    /// Distinct from [`HashLit`], which always brackets its pairs — `gem 'x',
    /// path: '../x'` is a keyword argument, not the hash `{ path: '../x' }`.
    /// Only meaningful inside an argument list ([`DslCall`], [`Call`]).
    KeywordArg { name: String, value: Box<RubyNode> },

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

    /// `attr_reader :name, :email` / `attr_writer` / `attr_accessor`
    AttrAccessor {
        mode: AccessorMode,
        names: Vec<String>,
    },

    /// `yield` or `yield value` — yields to a block
    Yield(Option<Box<RubyNode>>),

    /// `alias new_name old_name`
    Alias { new_name: String, old_name: String },

    /// Arbitrary Ruby expression in RSpec context — typed bridge for test code.
    RSpecCode(String),

    /// Multi-line verbatim Ruby body content from a higher-level generator.
    ///
    /// A narrow typed bridge (peer of [`RSpecCode`]) for content categories
    /// that are produced as already-rendered Ruby by a proven upstream
    /// generator — e.g., Pangea architecture method bodies composed from
    /// simulation output, or Pangea template DSL bodies whose shape is
    /// derived from proven WorkspaceSpec types.
    ///
    /// Emission: each line is prefixed with the current indent pad. Empty
    /// input emits nothing. Internal indentation on each line is preserved
    /// relative to the pad so that content generated at a known structural
    /// depth composes correctly when nested inside typed containers.
    ///
    /// Do NOT use as a general escape hatch — it is a typed bridge for
    /// specific generator pipelines, not a catchall.
    BodyLines(Vec<String>),

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
    Context { name: String, body: Vec<RubyNode> },

    /// `it 'name' do ... end`
    It { name: String, body: Vec<RubyNode> },

    /// `it_behaves_like 'name', key: value, ...`
    ItBehavesLike {
        name: String,
        params: Vec<(String, String)>,
    },

    /// `let(:name) { expr }`
    Let { name: String, expr: String },

    /// `expect(subject).matcher`
    Expect { subject: String, matcher: String },
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
                format!(
                    "{pad}{}; {}",
                    opens.join("; "),
                    closes.trim_end_matches("; ")
                )
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

            Self::ConstAssignNode { name, value } => {
                // Emit the value at the same indent level, then splice
                // the `<NAME> = ` prefix onto its first line so
                // multi-line literals (ArrayLit with nested HashLit)
                // align under the constant without an extra blank line.
                let emitted = value.emit(indent);
                let stripped = emitted.strip_prefix(&pad).unwrap_or(&emitted);
                format!("{pad}{name} = {stripped}")
            }

            Self::Attribute {
                name,
                type_expr,
                required,
            } => {
                let keyword = if *required { "attribute" } else { "attribute?" };
                let ty = if *required {
                    type_expr.emit()
                } else {
                    RubyType::optional(type_expr.clone()).emit()
                };
                format!("{pad}{keyword} :{name}, {ty}")
            }

            Self::DefineResource {
                tf_type,
                attrs_class,
                outputs,
                map,
                map_present,
                map_bool,
            }
            | Self::DefineData {
                tf_type,
                attrs_class,
                outputs,
                map,
                map_present,
                map_bool,
            } => {
                let kind = if matches!(self, Self::DefineResource { .. }) {
                    "define_resource"
                } else {
                    "define_data"
                };
                let mut out = format!("{pad}{kind} :{tf_type},\n");
                out.push_str(&format!("{pad}  attributes_class: {attrs_class},\n"));

                // outputs
                let output_pairs: Vec<String> =
                    outputs.iter().map(|(k, v)| format!("{k}: :{v}")).collect();
                out.push_str(&format!(
                    "{pad}  outputs: {{ {} }}",
                    output_pairs.join(", ")
                ));

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
                    let args_str = args
                        .iter()
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

            Self::PangeaResourceCall {
                receiver,
                resource_type,
                symbol,
                args,
            } => {
                let rcv = match receiver {
                    Some(r) => format!("{r}."),
                    None => String::new(),
                };
                if args.is_empty() {
                    format!("{pad}{rcv}{resource_type}(:\"{symbol}\", {{}})")
                } else {
                    let args_str = args
                        .iter()
                        .map(|(k, v)| format!("{pad}  {k}: {v}"))
                        .collect::<Vec<_>>()
                        .join(",\n");
                    format!("{pad}{rcv}{resource_type}(:\"{symbol}\", {{\n{args_str},\n{pad}}})")
                }
            }

            Self::PangeaOutput {
                output_type,
                name,
                value,
                description,
            } => {
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
                format!("{pad}Pangea::RemoteState.configure(bucket: {bucket}, region: {region})")
            }

            Self::PangeaRemoteStateOutput {
                template,
                output,
                state_key,
            } => {
                format!(
                    "{pad}Pangea::RemoteState.output(template: '{template}', output: :{output}, state_key: '{state_key}')"
                )
            }

            Self::ExtendModule {
                receiver,
                module_path,
                guard_method,
            } => {
                let rcv = receiver.as_deref().unwrap_or("self");
                match guard_method {
                    Some(method) => format!(
                        "{pad}{rcv}.extend({module_path}) unless {rcv}.respond_to?(:{method})"
                    ),
                    None => format!("{pad}{rcv}.extend({module_path})"),
                }
            }

            Self::EnvFetch { key, default } => match default {
                Some(d) => format!("{pad}ENV.fetch('{key}', '{d}')"),
                None => format!("{pad}ENV.fetch('{key}')"),
            },

            Self::EnvAssign { key, value } => {
                format!("{pad}ENV['{key}'] ||= {value}")
            }

            Self::Assignment { variable, value } => {
                format!("{pad}{variable} = {value}")
            }

            Self::AssignmentNode { variable, value } => {
                // Same splice as `ConstAssignNode`: emit the value at this
                // indent, then replace its pad with the `<var> = ` prefix so a
                // multi-line right-hand side stays aligned under the target.
                let emitted = value.emit(indent);
                let stripped = emitted.strip_prefix(&pad).unwrap_or(&emitted);
                format!("{pad}{variable} = {stripped}")
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

            Self::BeginRescue {
                body,
                rescue_class,
                rescue_body,
            } => {
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
                let providers_str = providers
                    .iter()
                    .map(|(name, source)| format!("{pad}  {name}: {{ source: '{source}' }}"))
                    .collect::<Vec<_>>()
                    .join(",\n");
                format!("{pad}required_providers({{\n{providers_str},\n{pad}}})")
            }

            Self::DoBlock {
                header,
                params,
                body,
            } => {
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
            Self::DslCall { method, args } => {
                if args.is_empty() {
                    format!("{pad}{method}")
                } else {
                    let arg_strs: Vec<String> = args.iter().map(|a| a.emit(0)).collect();
                    format!("{pad}{method} {}", arg_strs.join(", "))
                }
            }
            // General-purpose typed nodes
            Self::Ident(name) => format!("{pad}{name}"),
            Self::SymbolLit(name) => format!("{pad}:{name}"),
            Self::StringLit(val) => format!("{pad}'{}'", escape_single_quoted(val)),
            Self::ArrayLit(elements) => {
                let inner: Vec<String> = elements.iter().map(|e| e.emit(0)).collect();
                format!("{pad}[{}]", inner.join(", "))
            }
            Self::HashLit(pairs) => {
                if pairs.is_empty() {
                    format!("{pad}{{}}")
                } else if pairs.len() <= 3 {
                    let inner: Vec<String> = pairs
                        .iter()
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
            Self::HashRocketLit(pairs) => {
                if pairs.is_empty() {
                    format!("{pad}{{}}")
                } else if pairs.len() <= 3 {
                    let inner: Vec<String> = pairs
                        .iter()
                        .map(|(k, v)| format!("'{k}' => {}", v.emit(0)))
                        .collect();
                    format!("{pad}{{ {} }}", inner.join(", "))
                } else {
                    let inner_pad = "  ".repeat(indent + 1);
                    let mut out = format!("{pad}{{\n");
                    for (k, v) in pairs {
                        out.push_str(&format!("{inner_pad}'{k}' => {},\n", v.emit(0)));
                    }
                    out.push_str(&format!("{pad}}}"));
                    out
                }
            }
            Self::Call {
                receiver,
                method,
                args,
            } => {
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
            Self::IndexCall { receiver, index } => {
                format!("{pad}{}[{}]", receiver.emit(0), index.emit(0))
            }
            Self::KeywordArg { name, value } => {
                format!("{pad}{name}: {}", value.emit(0))
            }
            Self::ConstPath(parts) => format!("{pad}{}", parts.join("::")),
            Self::MergeCall { receiver, hash } => {
                let inner: Vec<String> = hash
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", v.emit(0)))
                    .collect();
                format!("{pad}{}.merge({})", receiver.emit(0), inner.join(", "))
            }

            // Architecture generation nodes
            Self::MethodDef {
                receiver,
                name,
                params,
                body,
            } => {
                let rcv = match receiver {
                    Some(r) => format!("{r}."),
                    None => String::new(),
                };
                let params_str = params
                    .iter()
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
                    let pairs: Vec<String> = entries
                        .iter()
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

            Self::AttrAccessor { mode, names } => {
                let directive = match mode {
                    AccessorMode::Reader => "attr_reader",
                    AccessorMode::Writer => "attr_writer",
                    AccessorMode::Accessor => "attr_accessor",
                };
                let symbols = names
                    .iter()
                    .map(|n| format!(":{n}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{pad}{directive} {symbols}")
            }

            Self::Yield(value) => match value {
                Some(expr) => format!("{pad}yield {}", expr.emit(0)),
                None => format!("{pad}yield"),
            },

            Self::Alias { new_name, old_name } => format!("{pad}alias {new_name} {old_name}"),

            Self::RSpecCode(code) => format!("{pad}{code}"),

            Self::BodyLines(lines) => {
                if lines.is_empty() {
                    return String::new();
                }
                lines
                    .iter()
                    .map(|line| {
                        if line.is_empty() {
                            String::new()
                        } else {
                            format!("{pad}{line}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }

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
                let name = escape_single_quoted(name);
                let mut out = format!("{pad}RSpec.shared_examples '{name}' do |{params}|\n");
                for node in body {
                    out.push_str(&node.emit(indent + 1));
                    out.push('\n');
                }
                out.push_str(&format!("{pad}end"));
                out
            }

            Self::Context { name, body } => {
                let name = escape_single_quoted(name);
                let mut out = format!("{pad}context '{name}' do\n");
                for node in body {
                    out.push_str(&node.emit(indent + 1));
                    out.push('\n');
                }
                out.push_str(&format!("{pad}end"));
                out
            }

            Self::It { name, body } => {
                let name = escape_single_quoted(name);
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
                let name = escape_single_quoted(name);
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

/// Escape a value for the inside of a Ruby **single-quoted** literal.
///
/// Ruby single-quoting recognises exactly two escapes, `\\` and `\'`; every
/// other backslash is literal. So the whole job is: double the backslashes
/// first, then escape the quotes. Doing it in that order matters — quote-first
/// would then double the backslash it had just introduced.
///
/// This lives in the emitter because that is the only place that knows the
/// value is about to be single-quoted. A caller that escapes its own strings
/// is a caller that can forget to, and callers did: the fleet's own
/// `pangea-platform` gemspec carries a description reading
/// "…arch-synthesizer's pangea_render…", which without this produces a
/// gemspec Ruby cannot parse.
///
/// Escaping the backslash is a strict widening, not a behaviour change, for
/// every value that has no trailing backslash: `'a\nb'` and `'a\\nb'` denote
/// the same four characters in Ruby. The values it rescues are the ones that
/// previously could not round-trip at all.
fn escape_single_quoted(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

/// Format a list of strings as Ruby symbols: `[:a, :b, :c]`
fn format_symbol_list(names: &[String]) -> String {
    names
        .iter()
        .map(|n| format!(":{n}"))
        .collect::<Vec<_>>()
        .join(", ")
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
            path: vec![
                "Pangea".into(),
                "Resources".into(),
                "Porkbun".into(),
                "Types".into(),
            ],
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
        assert_eq!(
            node.emit(2),
            "    attribute? :description, T::String.optional"
        );
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
            "Pangea".into(),
            "Resources".into(),
            "AWS".into(),
            "Types".into(),
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
            body: vec![RubyNode::It {
                name: "does something".into(),
                body: vec![RubyNode::Expect {
                    subject: "result".into(),
                    matcher: "to eq(42)".into(),
                }],
            }],
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
                (
                    "provider_module".into(),
                    "Pangea::Resources::Porkbun".into(),
                ),
                (
                    "types_module".into(),
                    "Pangea::Resources::Porkbun::Types".into(),
                ),
                (
                    "lib_path".into(),
                    "File.expand_path('../../lib', __dir__)".into(),
                ),
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
            body: vec![RubyNode::Assignment {
                variable: "config".into(),
                value: "Types::Config.new(config).to_h".into(),
            }],
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
            entries: vec![("SC-7".into(), "NIST 800-53".into())],
        };
        assert_eq!(
            node.emit(0),
            "CONTROLS = { 'SC-7' => 'NIST 800-53' }.freeze"
        );
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
        let a = RubyNode::FrozenConstHash {
            name: "X".into(),
            entries: entries.clone(),
        }
        .emit(0);
        let b = RubyNode::FrozenConstHash {
            name: "X".into(),
            entries,
        }
        .emit(0);
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
        assert!(
            output
                .contains("self.extend(Pangea::Resources::AWS) unless self.respond_to?(:aws_vpc)")
        );
        assert!(output.contains("aws_ebs_encryption_by_default"));
        assert!(output.contains("end"));
    }

    // ── New variant tests ────────────────────────────────────

    #[test]
    fn attr_accessor() {
        let node = RubyNode::AttrAccessor {
            mode: AccessorMode::Accessor,
            names: vec!["name".into(), "email".into()],
        };
        assert_eq!(node.emit(0), "attr_accessor :name, :email");
    }

    #[test]
    fn attr_reader() {
        let node = RubyNode::AttrAccessor {
            mode: AccessorMode::Reader,
            names: vec!["id".into()],
        };
        assert_eq!(node.emit(0), "attr_reader :id");
    }

    #[test]
    fn yield_no_value() {
        assert_eq!(RubyNode::Yield(None).emit(0), "yield");
    }

    #[test]
    fn yield_with_value() {
        let node = RubyNode::Yield(Some(Box::new(RubyNode::Ident("result".into()))));
        assert_eq!(node.emit(0), "yield result");
    }

    #[test]
    fn alias_directive() {
        let node = RubyNode::Alias {
            new_name: "new_method".into(),
            old_name: "old_method".into(),
        };
        assert_eq!(node.emit(0), "alias new_method old_method");
    }

    #[test]
    fn body_lines_empty_emits_nothing() {
        assert_eq!(RubyNode::BodyLines(vec![]).emit(2), "");
    }

    #[test]
    fn body_lines_prefixes_each_line_with_pad() {
        let node = RubyNode::BodyLines(vec!["a = 1".into(), "b = 2".into()]);
        assert_eq!(node.emit(1), "  a = 1\n  b = 2");
    }

    #[test]
    fn body_lines_preserves_blank_lines_without_trailing_space() {
        let node = RubyNode::BodyLines(vec!["a = 1".into(), String::new(), "b = 2".into()]);
        assert_eq!(node.emit(1), "  a = 1\n\n  b = 2");
    }

    #[test]
    fn body_lines_deterministic() {
        let lines = vec!["x".into(), "y".into(), "z".into()];
        let a = RubyNode::BodyLines(lines.clone()).emit(3);
        let b = RubyNode::BodyLines(lines).emit(3);
        assert_eq!(a, b);
    }

    // ── Typed value peers: AssignmentNode / DslCall / IndexCall / KeywordArg ──

    #[test]
    fn assignment_node_emits_the_same_shape_as_stringly_assignment() {
        let typed = RubyNode::AssignmentNode {
            variable: "s.version".into(),
            value: Box::new(RubyNode::StringLit("0.1.0".into())),
        };
        let stringly = RubyNode::Assignment {
            variable: "s.version".into(),
            value: "'0.1.0'".into(),
        };
        assert_eq!(typed.emit(0), "s.version = '0.1.0'");
        assert_eq!(typed.emit(1), stringly.emit(1));
    }

    #[test]
    fn assignment_node_takes_a_structured_right_hand_side() {
        let node = RubyNode::AssignmentNode {
            variable: "s.authors".into(),
            value: Box::new(RubyNode::ArrayLit(vec![RubyNode::StringLit(
                "Pleme Team".into(),
            )])),
        };
        assert_eq!(node.emit(1), "  s.authors = ['Pleme Team']");
    }

    #[test]
    fn dsl_call_joins_args_without_parens_and_degenerates_when_empty() {
        let with_args = RubyNode::DslCall {
            method: "s.add_dependency".into(),
            args: vec![
                RubyNode::StringLit("pangea-core".into()),
                RubyNode::StringLit("~> 0.2".into()),
            ],
        };
        assert_eq!(
            with_args.emit(1),
            "  s.add_dependency 'pangea-core', '~> 0.2'"
        );

        let bare = RubyNode::DslCall {
            method: "gemspec".into(),
            args: vec![],
        };
        assert_eq!(bare.emit(0), "gemspec");
        // Same degeneracy as its stringly peer with an empty value.
        assert_eq!(
            bare.emit(0),
            RubyNode::DslSetter {
                method: "gemspec".into(),
                value: String::new()
            }
            .emit(0)
        );
    }

    #[test]
    fn keyword_arg_is_bare_and_not_a_braced_hash() {
        let node = RubyNode::DslCall {
            method: "gem".into(),
            args: vec![
                RubyNode::StringLit("pangea-core".into()),
                RubyNode::KeywordArg {
                    name: "path".into(),
                    value: Box::new(RubyNode::StringLit("../pangea-core".into())),
                },
            ],
        };
        let out = node.emit(0);
        assert_eq!(out, "gem 'pangea-core', path: '../pangea-core'");
        assert!(
            !out.contains('{'),
            "keyword arg must not brace like HashLit: {out}"
        );
    }

    #[test]
    fn index_call_emits_brackets_not_a_dot_method() {
        let node = RubyNode::IndexCall {
            receiver: Box::new(RubyNode::Ident("Dir".into())),
            index: Box::new(RubyNode::StringLit("lib/**/*.rb".into())),
        };
        assert_eq!(node.emit(0), "Dir['lib/**/*.rb']");
    }

    // ── Single-quote escaping ────────────────────────────────────────────
    //
    // These pin the reason the escape moved into the emitter: a caller that
    // has to remember to escape is a caller that can forget, and the fleet's
    // own `pangea-platform` gemspec description contains an apostrophe.

    #[test]
    fn string_lit_escapes_an_embedded_single_quote() {
        let node = RubyNode::StringLit("arch-synthesizer's pangea_render".into());
        assert_eq!(node.emit(0), "'arch-synthesizer\\'s pangea_render'");
    }

    #[test]
    fn string_lit_escapes_backslash_before_quote_not_after() {
        // Quote-first ordering would double the backslash it just introduced,
        // yielding `'a\\\'b'` — a different string.
        let node = RubyNode::StringLit(r"a\'b".into());
        assert_eq!(node.emit(0), r"'a\\\'b'");
    }

    #[test]
    fn string_lit_leaves_ordinary_values_byte_identical() {
        for plain in ["pangea-platform", "MIT", ">= 3.3.0", "lib/**/*.rb"] {
            assert_eq!(
                RubyNode::StringLit(plain.into()).emit(0),
                format!("'{plain}'"),
                "escaping must be a no-op for values with no quote or backslash"
            );
        }
    }

    // ── RSpec description escaping ──────────────────────────────────
    //
    // `Context`/`It`/`SharedExamples`/`ItBehavesLike` carry the most
    // free-form prose of any node in the enum — an invariant's English
    // description, a scenario sentence — and single-quoted it without
    // escaping, so one apostrophe ended the literal early and CRuby
    // rejected the whole file. Same class as `StringLit`, one layer up:
    // proven live against ruby 2.6, which reports `syntax error` and then
    // `unterminated string meets end of file`.

    #[test]
    fn it_escapes_an_apostrophe_in_its_description() {
        let node = RubyNode::It {
            name: "the gateway's key is required".into(),
            body: vec![],
        };
        assert_eq!(node.emit(0), "it 'the gateway\\'s key is required' do\nend");
    }

    #[test]
    fn context_escapes_backslash_before_quote_not_after() {
        // Same ordering argument as `StringLit`: backslashes double first.
        let node = RubyNode::Context {
            name: r"a\'b".into(),
            body: vec![],
        };
        assert_eq!(node.emit(0), "context 'a\\\\\\'b' do\nend");
    }

    #[test]
    fn rspec_description_nodes_leave_ordinary_values_byte_identical() {
        // The fleet's existing descriptions contain neither character, so this
        // widening moves no bytes anywhere it already emitted valid Ruby —
        // confirmed by a 169-artifact byte-diff of arch-synthesizer's
        // `render_constellation` across this change.
        for plain in ["has path attribute", "when empty", "enforces bounds"] {
            assert_eq!(
                RubyNode::It {
                    name: plain.into(),
                    body: vec![]
                }
                .emit(0),
                format!("it '{plain}' do\nend")
            );
            assert_eq!(
                RubyNode::Context {
                    name: plain.into(),
                    body: vec![]
                }
                .emit(0),
                format!("context '{plain}' do\nend")
            );
            assert_eq!(
                RubyNode::ItBehavesLike {
                    name: plain.into(),
                    params: vec![]
                }
                .emit(0),
                format!("it_behaves_like '{plain}'")
            );
        }
    }
}
