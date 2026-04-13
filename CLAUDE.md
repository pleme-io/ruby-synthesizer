# ruby-synthesizer

Typed AST for structurally correct Ruby code generation from Rust.
Ruby is a build artifact -- authored in Rust, materialized as Ruby,
proven by 173 tests. Syntax errors are impossible at the Rust compiler level.

## How It Is Consumed

`pangea-forge` (the Pangea backend in the iac-forge pipeline) depends on
ruby-synthesizer to generate all Ruby DSL code for Pangea providers
(pangea-aws, pangea-akeyless, pangea-cloudflare, etc.). The flow is:

```
TOML resource spec -> iac-forge IR (IacType, IacResource)
    -> pangea-forge (Backend impl)
        -> ruby-synthesizer (TypesFileBuilder, ResourceFileBuilder)
            -> emitted Ruby (types.rb, resource.rb, spec.rb)
```

pangea-forge calls `TypesFileBuilder` for types files, `ResourceFileBuilder`
for resource files, and `RSpecBuilder` for test files. The IaC bridge
(`iac_type_to_ruby`) converts `IacType` to `RubyType` at the boundary.

## The Compiler Prevents Invalid Ruby

The following classes of bugs are **impossible** at the Rust compiler level:

| Guarantee | How | Enforced by |
|-----------|-----|-------------|
| No unbalanced `module`/`class`/`end` | `Module` and `Class` nodes always emit matching `end` | `RubyNode::emit()` |
| No attribute outside a class | `ClassBuilder` only exposes `.attribute()` | Type system |
| No class outside a module | `TypesFileBuilder.class()` places inside module | Builder structure |
| No double `.optional.optional` | `RubyType::optional()` is idempotent | Checked at construction |
| No empty union | `RubyType::union(vec![])` panics | Runtime invariant |
| Single-variant union degenerates | `RubyType::union(vec![x])` returns `x` | Checked at construction |
| Frozen pragma always first | Both builders emit it unconditionally first | Builder structure |
| Valid indentation (2-space) | `emit()` takes indent level, always `"  ".repeat(n)` | Emitter |
| No trailing whitespace | Emit produces exact content, no trailing spaces | Emitter |
| Clean ASCII output | All output bytes are 0x20-0x7E or newline | Emitter |
| Deterministic output | Same AST always produces byte-identical Ruby | `emit_file()` is pure |

## Builder API Reference

### TypesFileBuilder

Generates a Pangea types.rb file. Structure is enforced:
`frozen_string_literal` -> `require` -> `module Provider::Types` -> `include Dry.Types()` -> `class < BaseAttributes` -> `T = ...` -> attributes.

```rust
TypesFileBuilder::new("aws")                          // provider name
    .class("VpcAttributes", |c| {                     // adds a class
        c.attribute("cidr_block", RubyType::simple("T::String"), true)
         .attribute("tags", RubyType::Hash, false)     // false = optional
    })
    .emit()                                            // -> String
```

### ResourceFileBuilder

Generates a Pangea resource.rb file. Structure:
`frozen_string_literal` -> requires -> `module Pangea::Resources` -> `module ProviderResource` -> `include ResourceBuilder` -> `define_resource` -> provider module include -> registry call.

```rust
ResourceFileBuilder::new("porkbun", "porkbun_nameservers", "nameservers")
    .outputs(vec![("id", "id")])                       // output mappings
    .map(vec!["domain", "nameservers"])                 // direct map fields
    .map_present(vec!["description"])                   // map_present fields
    .map_bool(vec!["enable_dns"])                       // boolean map fields
    .emit()                                            // -> String
```

### RSpecBuilder

Fluent API for RSpec test construction:

```rust
RSpecBuilder::describe("'my test'")                    // RSpec.describe
    .let_bind("subject", "described_class.new")        // let(:subject) { ... }
    .it("works", |b| b.expect("result", "to eq(42)")) // it + expect
    .context("when empty", |b| {                       // context block
        b.it("returns nil", |b| b.expect("subject", "to be_nil"))
    })
    .it_behaves_like("a typed provider", vec![         // shared examples
        ("provider_module", "Pangea::Resources::AWS"),
    ])
    .build()                                           // -> RubyNode
```

### ClassBuilder

Restricted builder context inside `TypesFileBuilder.class()`. Only `.attribute()` is
available -- no module or file operations. This restriction is enforced by the type system.

```rust
.class("MyAttributes", |c| {
    c.attribute("name", RubyType::simple("T::String"), true)
     .attribute("desc", RubyType::simple("T::String"), false)
})
```

## Macro API

### `ruby_module!`

```rust
let node = ruby_module!("Pangea::Resources::AWS::Types" => {
    include "Dry.Types()";
    blank;
    class "VpcAttributes" < "Pangea::Resources::BaseAttributes" {
        const "T" => "Pangea::Resources::AWS::Types";
        blank;
        attribute "cidr_block", RubyType::simple("T::String");
        attribute? "description", RubyType::simple("T::String");
    }
});
```

### `ruby_body!`

Constructs a `Vec<RubyNode>` from Ruby-like syntax. Supports: `include`, `class` (with
or without parent), `attribute`/`attribute?`, `const`, `blank`.

### `ruby_parent!`

```rust
ruby_parent!()           // -> None
ruby_parent!("BaseClass") // -> Some("BaseClass".to_string())
```

## What's Proven (173 tests)

| Category | Tests | File | What |
|----------|-------|------|------|
| Compiler guarantees | 30 | `tests/compiler_guarantees.rs` | Non-empty output, frozen pragma, balanced blocks, determinism, optional idempotence, valid Dry::Types, constrained preservation, section ordering, indentation, clean ASCII, no trailing whitespace |
| Builder structure | 21 | `tests/structural.rs` | TypesFile + ResourceFile invariants via proptest |
| Unit (iac_bridge) | 18 | `src/iac_bridge.rs` | Exhaustive variant coverage + parity + injectivity |
| Property tests | 16 | `tests/properties.rs` | Balanced parens, deterministic, idempotent optional, macro parity |
| Type algebra | 14 | `tests/type_algebra.rs` | Injectivity, constants, structural patterns, union separator count |
| Lattice properties | 13 | `tests/lattice.rs` | Optional lifting, constrained narrowing, union bounds, monotonicity |
| Bridge parity | 12 | `tests/bridge_parity.rs` | IacType -> RubyType totality, determinism, composition |
| Convergence stages | 11 | `tests/convergence_stages.rs` | declared->resolved->converged, monotonicity |
| Unit (builders) | 10 | `src/builders.rs` | Builder output structure, pascal case |
| Unit (node) | 9 | `src/node.rs` | Individual node emission |
| Unit (types) | 9 | `src/types.rs` | Type emission for all variants |
| RSpec builder | 6 | `tests/rspec_builder.rs` | Structure, indentation, let bindings |
| Unit (rspec) | 3 | `src/rspec.rs` | Fluent builder + macro parity |
| Unit (emitter) | 1 | `src/emitter.rs` | Complete file emission |

## IaC Bridge (feature: iac-bridge)

Maps `iac_forge::ir::IacType` -> `RubyType` with proven parity:
- **Injective**: different inputs produce different outputs
- **Total**: every variant handled (unknown = explicit panic with message)
- **Deterministic**: same input always produces same output

Mapping: String->T::String, Integer->T::Integer, Float->T::Coercible::Float,
Numeric->union(Integer|Float), Boolean->T::Bool, List/Set->Array, Map/Object->Hash,
Enum->Constrained, Any->T::Any.

## Convergence Theory

The convergence pipeline at the language boundary:

```
declared          -> resolved            -> converged         -> verified
Rust enums           compile-time valid     emit_file()          173 tests
(RubyNode/RubyType)  (builder enforced)     (deterministic)      (proptest proofs)
```

- **declared** = Rust types (RubyNode 25 variants, RubyType 7 variants)
- **resolved** = AST construction (invalid nesting = compile error)
- **converged** = `emit_file()` produces Ruby source (deterministic, trailing newline)
- **verified** = 173 tests prove all invariants hold (lattice, algebra, bridge, structure)
