# ruby-synthesizer

Typed AST for structurally correct Ruby code generation from Rust.
Ruby is a build artifact — authored in Rust, materialized as Ruby,
proven by 147 tests.

## The Concept

This is not a code generator. It's a **typed computation bridge**:

- Rust types declare intent (RubyNode, RubyType)
- The AST is resolved at compile time (invalid nesting = compile error)
- emit_file() converges to Ruby source (deterministic)
- Property tests verify every structural invariant (proptest)

Syntax errors are impossible at the Rust compiler level.

## Usage

```rust
use ruby_synthesizer::builders::TypesFileBuilder;
use ruby_synthesizer::RubyType;

// Declare in Rust → materialize as Ruby
let ruby_source = TypesFileBuilder::new("aws")
    .class("VpcAttributes", |c| {
        c.attribute("cidr_block", RubyType::simple("T::String"), true)
         .attribute("tags", RubyType::Hash, false)
    })
    .emit();
```

Or with macros:
```rust
use ruby_synthesizer::{ruby_module, ruby_body, ruby_parent, RubyType};

let node = ruby_module!("Pangea::Resources::AWS::Types" => {
    include "Dry.Types()";
    class "VpcAttributes" < "Pangea::Resources::BaseAttributes" {
        attribute "cidr_block", RubyType::simple("T::String");
    }
});
```

## What's Proven (147 tests)

| Category | Tests | What |
|----------|-------|------|
| Lattice properties | 13 | optional lifting, union bounds, monotonicity, closure |
| Type algebra | 14 | injectivity, constants, structural patterns |
| Bridge parity | 12 | IacType → RubyType totality, determinism |
| Builder structure | 21 | TypesFile + ResourceFile invariants |
| Convergence stages | 11 | declared→resolved→converged, monotonicity |
| RSpec builder | 6 | structure, indentation |
| Property tests | 16 | balanced parens, idempotent optional (proptest) |
| Unit + doc | 54 | node emission, type emission, macros |

## IaC Bridge (feature: iac-bridge)

Maps `iac_forge::ir::IacType` → `RubyType` with proven parity:
- Injective (different inputs → different outputs)
- Total (every variant handled, unknown = panic)
- Deterministic (same input → same output)

## Convergence Theory

The ruby-synthesizer implements the convergence pipeline at the language boundary:
- **declared** = Rust types (RubyNode, RubyType enums)
- **resolved** = AST construction (compile-time validated)
- **converged** = emit_file() produces Ruby source (deterministic)
- **verified** = 147 property tests prove invariants hold
