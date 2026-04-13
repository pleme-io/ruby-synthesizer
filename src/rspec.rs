//! RSpec builder — fluent API for constructing test nodes.
//!
//! Instead of manually constructing RSpec node variants, use the builder:
//!
//! ```rust
//! use ruby_synthesizer::RSpecBuilder;
//!
//! let spec = RSpecBuilder::describe("'my class'")
//!     .it("works", |b| b.expect("result", "to eq(42)"))
//!     .context("when empty", |b| {
//!         b.it("returns nil", |b| b.expect("subject", "to be_nil"))
//!     })
//!     .build();
//! ```

use crate::node::RubyNode;

/// Fluent builder for RSpec test structures.
pub struct RSpecBuilder {
    nodes: Vec<RubyNode>,
}

impl RSpecBuilder {
    /// Start a `RSpec.describe` block.
    #[must_use]
    pub fn describe(subject: &str) -> DescribeBuilder {
        DescribeBuilder {
            subject: subject.to_string(),
            body: Vec::new(),
        }
    }

    /// Start a `RSpec.shared_examples` block.
    #[must_use]
    pub fn shared_examples(name: &str, params: &str) -> SharedExamplesBuilder {
        SharedExamplesBuilder {
            name: name.to_string(),
            params: params.to_string(),
            body: Vec::new(),
        }
    }
}

/// Builder for RSpec.describe blocks.
pub struct DescribeBuilder {
    subject: String,
    body: Vec<RubyNode>,
}

impl DescribeBuilder {
    /// Add an `it` block.
    #[must_use]
    pub fn it(mut self, name: &str, f: impl FnOnce(ItBuilder) -> ItBuilder) -> Self {
        let builder = f(ItBuilder { body: Vec::new() });
        self.body.push(RubyNode::It {
            name: name.to_string(),
            body: builder.body,
        });
        self
    }

    /// Add a `context` block.
    #[must_use]
    pub fn context(mut self, name: &str, f: impl FnOnce(DescribeBuilder) -> DescribeBuilder) -> Self {
        let inner = f(DescribeBuilder {
            subject: String::new(),
            body: Vec::new(),
        });
        self.body.push(RubyNode::Context {
            name: name.to_string(),
            body: inner.body,
        });
        self
    }

    /// Add a `let` binding.
    #[must_use]
    pub fn let_bind(mut self, name: &str, expr: &str) -> Self {
        self.body.push(RubyNode::Let {
            name: name.to_string(),
            expr: expr.to_string(),
        });
        self
    }

    /// Add an `it_behaves_like` call.
    #[must_use]
    pub fn it_behaves_like(mut self, name: &str, params: Vec<(&str, &str)>) -> Self {
        self.body.push(RubyNode::ItBehavesLike {
            name: name.to_string(),
            params: params.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        });
        self
    }

    /// Add a raw node.
    #[must_use]
    pub fn node(mut self, node: RubyNode) -> Self {
        self.body.push(node);
        self
    }

    /// Build the `RSpec.describe` node.
    #[must_use]
    pub fn build(self) -> RubyNode {
        RubyNode::Describe {
            subject: self.subject,
            body: self.body,
        }
    }
}

/// Builder for shared_examples blocks.
pub struct SharedExamplesBuilder {
    name: String,
    params: String,
    body: Vec<RubyNode>,
}

impl SharedExamplesBuilder {
    /// Add an `it` block.
    #[must_use]
    pub fn it(mut self, name: &str, f: impl FnOnce(ItBuilder) -> ItBuilder) -> Self {
        let builder = f(ItBuilder { body: Vec::new() });
        self.body.push(RubyNode::It {
            name: name.to_string(),
            body: builder.body,
        });
        self
    }

    /// Add a `let` binding.
    #[must_use]
    pub fn let_bind(mut self, name: &str, expr: &str) -> Self {
        self.body.push(RubyNode::Let {
            name: name.to_string(),
            expr: expr.to_string(),
        });
        self
    }

    /// Add a raw node.
    #[must_use]
    pub fn node(mut self, node: RubyNode) -> Self {
        self.body.push(node);
        self
    }

    /// Build the `RSpec.shared_examples` node.
    #[must_use]
    pub fn build(self) -> RubyNode {
        RubyNode::SharedExamples {
            name: self.name,
            params: self.params,
            body: self.body,
        }
    }
}

/// Builder for `it` block body.
pub struct ItBuilder {
    body: Vec<RubyNode>,
}

impl ItBuilder {
    /// Add an expect assertion.
    #[must_use]
    pub fn expect(mut self, subject: &str, matcher: &str) -> Self {
        self.body.push(RubyNode::Expect {
            subject: subject.to_string(),
            matcher: matcher.to_string(),
        });
        self
    }

    /// Add a raw expression.
    #[must_use]
    pub fn raw(mut self, code: &str) -> Self {
        self.body.push(RubyNode::Raw(code.to_string()));
        self
    }
}

/// Convenience macro for building Ruby AST nodes with Ruby-like syntax.
///
/// ```rust
/// use ruby_synthesizer::{ruby_module, ruby_body, ruby_parent, RubyNode, RubyType};
///
/// let node = ruby_module!("Pangea::Resources::Test" => {
///     include "Dry.Types()";
///     class "MyAttrs" < "BaseAttributes" {
///         attribute "name", RubyType::simple("T::String");
///     }
/// });
/// ```
#[macro_export]
macro_rules! ruby_module {
    ($path:expr => { $($body:tt)* }) => {{
        let path: Vec<String> = $path.split("::").map(|s| s.to_string()).collect();
        $crate::RubyNode::Module {
            path,
            body: ruby_body!($($body)*),
        }
    }};
}

#[macro_export]
macro_rules! ruby_body {
    () => { vec![] };

    (include $module:expr; $($rest:tt)*) => {{
        let mut nodes = vec![$crate::RubyNode::Include($module.to_string())];
        nodes.extend(ruby_body!($($rest)*));
        nodes
    }};

    (class $name:literal < $parent:literal { $($body:tt)* } $($rest:tt)*) => {{
        let mut nodes = vec![$crate::RubyNode::Class {
            name: $name.to_string(),
            parent: Some($parent.to_string()),
            body: ruby_body!($($body)*),
        }];
        nodes.extend(ruby_body!($($rest)*));
        nodes
    }};

    (class $name:literal { $($body:tt)* } $($rest:tt)*) => {{
        let mut nodes = vec![$crate::RubyNode::Class {
            name: $name.to_string(),
            parent: None,
            body: ruby_body!($($body)*),
        }];
        nodes.extend(ruby_body!($($rest)*));
        nodes
    }};

    (attribute $name:literal, $type_expr:expr; $($rest:tt)*) => {{
        let mut nodes = vec![$crate::RubyNode::Attribute {
            name: $name.to_string(),
            type_expr: $type_expr,
            required: true,
        }];
        nodes.extend(ruby_body!($($rest)*));
        nodes
    }};

    (attribute? $name:literal, $type_expr:expr; $($rest:tt)*) => {{
        let mut nodes = vec![$crate::RubyNode::Attribute {
            name: $name.to_string(),
            type_expr: $type_expr,
            required: false,
        }];
        nodes.extend(ruby_body!($($rest)*));
        nodes
    }};

    (const $name:literal => $value:literal; $($rest:tt)*) => {{
        let mut nodes = vec![$crate::RubyNode::ConstAssign {
            name: $name.to_string(),
            value: $value.to_string(),
        }];
        nodes.extend(ruby_body!($($rest)*));
        nodes
    }};

    (blank; $($rest:tt)*) => {{
        let mut nodes = vec![$crate::RubyNode::Blank];
        nodes.extend(ruby_body!($($rest)*));
        nodes
    }};
}

#[macro_export]
macro_rules! ruby_parent {
    () => { None };
    ($parent:expr) => { Some($parent.to_string()) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RubyType;

    #[test]
    fn fluent_describe_builder() {
        let spec = RSpecBuilder::describe("'pangea-porkbun type purity'")
            .it_behaves_like("a pure typed provider", vec![
                ("provider_module", "Pangea::Resources::Porkbun"),
                ("types_module", "Pangea::Resources::Porkbun::Types"),
                ("lib_path", "File.expand_path('../../lib', __dir__)"),
            ])
            .build();

        let output = spec.emit(0);
        assert!(output.contains("RSpec.describe 'pangea-porkbun type purity' do"));
        assert!(output.contains("it_behaves_like 'a pure typed provider'"));
        assert!(output.contains("provider_module: Pangea::Resources::Porkbun,"));
    }

    #[test]
    fn fluent_it_with_expect() {
        let spec = RSpecBuilder::describe("'my test'")
            .it("returns 42", |b| {
                b.expect("result", "to eq(42)")
                 .expect("other", "to be_nil")
            })
            .build();

        let output = spec.emit(0);
        assert!(output.contains("it 'returns 42' do"));
        assert!(output.contains("expect(result).to eq(42)"));
        assert!(output.contains("expect(other).to be_nil"));
    }

    #[test]
    fn macro_module_with_class() {
        let node = ruby_module!("Pangea::Resources::Test::Types" => {
            include "Dry.Types()";
            blank;
            class "MyAttributes" < "Pangea::Resources::BaseAttributes" {
                const "T" => "Pangea::Resources::Test::Types";
                blank;
                attribute "name", RubyType::simple("T::String");
                attribute? "desc", RubyType::simple("T::String");
            }
        });

        let output = node.emit(0);
        assert!(output.contains("module Pangea::Resources::Test::Types"));
        assert!(output.contains("include Dry.Types()"));
        assert!(output.contains("class MyAttributes < Pangea::Resources::BaseAttributes"));
        assert!(output.contains("T = Pangea::Resources::Test::Types"));
        assert!(output.contains("attribute :name, T::String"));
        assert!(output.contains("attribute? :desc, T::String.optional"));
    }
}
