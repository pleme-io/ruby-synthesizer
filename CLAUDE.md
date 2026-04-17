# ruby-synthesizer

Typed AST for structurally correct Ruby code generation from Rust.
Ruby is a build artifact -- authored in Rust, materialized as Ruby,
proven by 360 tests. Syntax errors are impossible at the Rust compiler level.

## How It Is Consumed

`pangea-forge` (the Pangea backend in the iac-forge pipeline) depends on
ruby-synthesizer to generate BOTH Ruby and RBS (Ruby Signature) code for
Pangea providers (pangea-aws, pangea-akeyless, pangea-cloudflare, etc.). The flow is:

```
TOML resource spec -> iac-forge IR (IacType, IacResource)
    -> pangea-forge (Backend impl)
        -> ruby-synthesizer (TypesFileBuilder, ResourceFileBuilder,
                             TypesRbsFileBuilder)
            -> emitted Ruby (types.rb, resource.rb, spec.rb)
            -> emitted RBS (types.rbs)  [steep-checkable signatures]
```

pangea-forge calls:
- `TypesFileBuilder` for types.rb (Dry::Struct attribute classes)
- `ResourceFileBuilder` for resource.rb (ResourceBuilder DSL)
- `TypesRbsFileBuilder` for types.rbs (RBS type signatures — paired with
  the Dry::Types version, closes the type gap at the render edge)
- `RSpecBuilder` for test files

Two IR bridges:
- `iac_type_to_ruby(&IacType) -> RubyType` — Dry::Types mapping
- `iac_type_to_rbs(&IacType) -> RbsType` — RBS mapping

Both are first-class `ProvenMorphism` values via `IacTypeToRuby`,
`IacTypeToRbs`, `RubyTypeToString`, `RbsTypeToString`. Compose with
`iac_forge::morphism::Composed` to carry proofs end-to-end.

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

## What's Proven (360 tests)

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
| Exhaustive AST proofs | 48 | `tests/exhaustive_ast_proofs.rs` | Every node variant emission, type algebra proptest, builder edge cases, IaC bridge exhaustive coverage, cross-cutting structural invariants |
| Unit (emitter) | 1 | `src/emitter.rs` | Complete file emission |

## IaC Bridge (feature: iac-bridge)

Two bridges, same invariants (injective, total, deterministic):

### `iac_type_to_ruby(&IacType) -> RubyType`

Mapping to Dry::Types: String→T::String, Integer→T::Integer,
Float→T::Coercible::Float, Numeric→union(Integer|Float), Boolean→T::Bool,
List/Set→Array, Map/Object→Hash, Enum→Constrained, Any→T::Any.

### `iac_type_to_rbs(&IacType) -> RbsType`

Mapping to RBS: String→String, Integer→Integer, Float→Float,
Numeric→union(Integer|Float), Boolean→bool, List/Set→Array[T],
Map/Object→Hash[Symbol, untyped], Enum→string-literal union of values
(or underlying if empty), Any→untyped.

### Bridges as ProvenMorphisms

`IacTypeToRuby`, `IacTypeToRbs`, `RubyTypeToString`, `RbsTypeToString`
are all named, first-class `ProvenMorphism` values. Compose them to
chain proofs:

```rust
use iac_forge::morphism::{Composed, Morphism, ProvenMorphism};
use ruby_synthesizer::iac_bridge::{IacTypeToRuby, RubyTypeToString};

let chain = Composed::new(IacTypeToRuby, RubyTypeToString);
let emitted = chain.apply(&IacType::List(Box::new(IacType::String)));
assert_eq!(emitted, "T::Array.of(T::String)");
// check_invariants returns violation list prefixed by source morphism name
assert!(chain.check_invariants(&src, &emitted).is_empty());
```

## RbsType

Parallels `RubyType` for RBS signature output. 7 variants:
`Named(String)`, `Array(Box<RbsType>)`, `Hash(key, value)`,
`Union(Vec<RbsType>)`, `StringLiteral(String)`, `Nilable(Box<RbsType>)`,
`Untyped`.

Emitted forms (proven by 14 unit tests + 12 proptest runs):
- `Named("String")` → `String`
- `Array(Named("Integer"))` → `Array[Integer]`
- `Hash(Named("Symbol"), Untyped)` → `Hash[Symbol, untyped]`
- `Union([StringLiteral("tcp"), StringLiteral("udp")])` → `"tcp" | "udp"`
- `Nilable(Named("String"))` → `String?`
- `Untyped` → `untyped`

Nilable is idempotent at construction (`nilable(nilable(x)) == nilable(x)`);
Union of 1 variant degenerates to that variant.

## TypesRbsFileBuilder

Parallels `TypesFileBuilder` for RBS files. Same structural discipline:
module chain enforced, class-only-in-module, attribute-only-in-class,
required attributes emit non-nilable, optional attributes emit nilable.

```rust
TypesRbsFileBuilder::new("datadog")
    .class("MonitorAttributes", |c| {
        c.attribute("name", RbsType::named("String"), true)         // required
         .attribute("tags", RbsType::array(RbsType::named("String")), false)  // optional
    })
    .emit()
```

Produces:
```rbs
# Code generated by pangea-forge. DO NOT EDIT.
module Pangea
  module Resources
    module Datadog
      module Types
        class MonitorAttributes < Pangea::Resources::BaseAttributes
          attr_reader name: String
          attr_reader tags: Array[String]?
        end
      end
    end
  end
end
```

## BodyLines (Typed Bridge for Multi-Line Generator Output)

`RubyNode::BodyLines(Vec<String>)` is a narrow typed bridge (peer of
`RSpecCode`) for multi-line content produced by higher-level generators —
architecture method bodies composed from simulation output, template DSL
bodies derived from WorkspaceSpec, etc. Each input line is prefixed with
the current indent pad; blank lines stay blank.

**Not a general escape hatch.** The `NoRawAttestation` contract still
holds: `RubyNode::Raw` is gone, BodyLines is a narrow bridge for specific
content categories. Downstream consumers (pangea-forge) use BodyLines
for method/template bodies; everything else lives in structured nodes.

## Canonical Sexpr Interchange

Under the `iac-bridge` feature, every `RubyType` and `RbsType` implements
`iac_forge::sexpr::{ToSExpr, FromSExpr}`. Round-trip is lossless and
deterministic:

```rust
use iac_forge::sexpr::{SExpr, ToSExpr, FromSExpr};

let ty = RubyType::array(RubyType::simple("T::String"));
let s = ty.to_sexpr();
assert_eq!(s.emit(), "(array (simple \"T::String\"))");

let parsed = RubyType::from_sexpr(&SExpr::parse(&s.emit()).unwrap()).unwrap();
assert_eq!(parsed, ty);
```

Single-variant union on parse degenerates to the wrapped variant,
matching the constructor invariant.

## Cross-Language Content-Hash Vectors

`tests/cross_lang_vectors.rs` pins canonical emissions + BLAKE3 hex for
every RubyType and RbsType variant (17 vectors). Hashes verified
independently via `b3sum 1.8.4`. Any reimplementation that correctly
emits canonical sexpr + BLAKE3-hashes will produce these same hashes.

## Convergence Theory

The convergence pipeline at the language boundary:

```
declared          -> resolved            -> converged         -> verified
Rust enums           compile-time valid     emit_file()          225 tests
(RubyNode/RubyType)  (builder enforced)     (deterministic)      (proptest proofs)
```

- **declared** = Rust types (RubyNode 54 variants — Raw removed in Wave 3,
  BodyLines added as narrow typed bridge; RubyType 7 variants; RbsType 7 variants)
- **resolved** = AST construction (invalid nesting = compile error)
- **converged** = `emit_file()` produces Ruby/RBS source (deterministic, trailing newline)
- **verified** = 360 tests prove all invariants hold (lattice, algebra, bridge,
  structure, sexpr round-trip, cross-language content-hash vectors)
