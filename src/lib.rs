//! ruby-synthesizer — Typed AST for structurally correct Ruby code generation.
//!
//! Replaces string concatenation (`push_str`) with typed AST node construction.
//! Syntax errors become impossible at the Rust compiler level.
//!
//! # Usage
//!
//! ```rust
//! use ruby_synthesizer::{RubyNode, RubyType, emit_file};
//!
//! let file = vec![
//!     RubyNode::FrozenStringLiteral,
//!     RubyNode::Comment("Generated code".into()),
//!     RubyNode::Require("dry-struct".into()),
//!     RubyNode::Module {
//!         path: vec!["MyModule".into()],
//!         body: vec![
//!             RubyNode::Class {
//!                 name: "MyClass".into(),
//!                 parent: Some("BaseClass".into()),
//!                 body: vec![
//!                     RubyNode::Attribute {
//!                         name: "name".into(),
//!                         type_expr: RubyType::simple("T::String"),
//!                         required: true,
//!                     },
//!                 ],
//!             },
//!         ],
//!     },
//! ];
//!
//! let ruby_source = emit_file(&file);
//! ```

mod node;
mod types;
mod emitter;
mod rspec;
pub mod builders;

#[cfg(feature = "iac-bridge")]
pub mod iac_bridge;

pub use node::RubyNode;
pub use types::RubyType;
pub use emitter::emit_file;
pub use rspec::RSpecBuilder;
