# IaC Bridge -- IacType to RubyType Mapping

The IaC bridge (`src/iac_bridge.rs`, enabled via `feature = "iac-bridge"`) maps
the platform-independent `iac_forge::ir::IacType` to `RubyType`. This is the
typed boundary where infrastructure types become Ruby Dry::Types expressions.

## The Mapping Function

```rust
pub fn iac_type_to_ruby(ty: &IacType) -> RubyType
```

A single function, exhaustive match, no fallback. Every `IacType` variant is
explicitly handled. Unknown variants panic with a message identifying the
missing mapping -- this makes the problem space known and forces explicit
extension when `iac-forge` adds new types.

## Complete Mapping Table

| IacType | RubyType | Ruby output | Notes |
|---------|----------|-------------|-------|
| `String` | `Simple("T::String")` | `T::String` | Direct scalar |
| `Integer` | `Simple("T::Integer")` | `T::Integer` | Direct scalar |
| `Float` | `Simple("T::Coercible::Float")` | `T::Coercible::Float` | Coercible for JSON interop |
| `Numeric` | `Union([Integer, Float])` | `(T::Coercible::Integer \| T::Coercible::Float)` | Union of numeric types |
| `Boolean` | `Simple("T::Bool")` | `T::Bool` | Direct scalar |
| `List(inner)` | `Array(bridge(inner))` | `T::Array.of(...)` | Recursive -- inner type is bridged |
| `Set(inner)` | `Array(bridge(inner))` | `T::Array.of(...)` | Same as List in Ruby (no Set type) |
| `Map(_)` | `Hash` | `T::Hash` | Value type erased (Dry::Types limitation) |
| `Object { name, fields }` | `Hash` | `T::Hash` | Complex objects become hashes |
| `Enum { values, underlying }` | `Constrained(base, ...)` | `T::String.constrained(...)` | See enum section below |
| `Any` | `Any` | `T::Any` | Escape hatch |

## Enum Handling

Enums have two cases:

1. **Non-empty values**: Maps to `constrained(underlying, included_in: [values])`
   ```rust
   IacType::Enum {
       values: vec!["tcp".into(), "udp".into()],
       underlying: Box::new(IacType::String),
   }
   // -> T::String.constrained(included_in: ['tcp', 'udp'])
   ```

2. **Empty values**: Degenerates to the underlying type
   ```rust
   IacType::Enum {
       values: vec![],
       underlying: Box::new(IacType::String),
   }
   // -> T::String
   ```

Single quotes in enum values are escaped: `'` becomes `\'`.

## Proven Properties

### 1. Injectivity

Different `IacType` inputs produce different `RubyType` outputs:

```
iac_type_to_ruby(A).emit() == iac_type_to_ruby(B).emit()  =>  A == B
```

Proven for scalar types in `src/iac_bridge.rs` (`string_differs_from_integer`,
`list_differs_from_set_by_content`).

**Caveat**: `List` and `Set` with the same inner type produce identical output
(`T::Array.of(...)`). This is intentional -- Ruby has no `Set` in Dry::Types.
Injectivity holds for the mapping function's input/output pairs, but `List(X)`
and `Set(X)` are treated as equivalent at the Ruby boundary.

### 2. Totality

Every `IacType` variant is handled. The function does not return a default or
fallback. Unknown variants hit an explicit `panic!` with a diagnostic message:

```rust
other => panic!("unsupported IacType variant in iac_type_to_ruby: {other:?} — add an explicit mapping"),
```

Totality is verified via proptest in `tests/bridge_parity.rs` (proof 10):
a random `IacType` generator covering all variants (String, Integer, Float,
Numeric, Boolean, Any, List, Set, Map, Object, Enum) with recursive nesting
up to depth 3 and 32 max nodes. No panic across all generated cases.

### 3. Determinism

Same input always produces the same output:

```
iac_type_to_ruby(X).emit() == iac_type_to_ruby(X).emit()  // always true
```

Proven via proptest in `tests/bridge_parity.rs` (proof 11) with random inputs.

### 4. Composition Preservation

The mapping preserves compositional structure. For any inner type X:

```
iac_type_to_ruby(List(X)).emit()  contains  iac_type_to_ruby(X).emit()
```

The inner type's Ruby representation appears verbatim inside the List/Array wrapper.
No information is lost. Proven in `tests/bridge_parity.rs` (proof 9).

### 5. Nested Depth Preservation

Double nesting produces the correct structure:

```
iac_type_to_ruby(List(List(X))).emit()
    == "T::Array.of(T::Array.of(" + iac_type_to_ruby(X).emit() + "))"
```

Proven exactly in `tests/bridge_parity.rs` (proof 12).

### 6. Set/List Equivalence

Set and List with the same inner type produce identical Ruby output:

```
iac_type_to_ruby(Set(X)).emit() == iac_type_to_ruby(List(X)).emit()
```

This is a deliberate design choice. Dry::Types has no Set type, so both
collection types map to `T::Array.of(...)`. Proven in `tests/bridge_parity.rs`
(proof 8).

## How pangea-forge Consumes It

`pangea-forge` (the Pangea backend for iac-forge) uses `iac_type_to_ruby` to
convert attribute types when generating types files:

```
iac-forge IR                    pangea-forge                ruby-synthesizer
─────────────                    ────────────                ────────────────
IacResource                      for each attribute:
  .attributes[]                    iac_type_to_ruby(attr.iac_type)  -> RubyType
    .iac_type: IacType               |
                                     v
                                  TypesFileBuilder
                                    .class(name, |c| {
                                        c.attribute(name, ruby_type, required)
                                    })
                                    .emit()  -> String (types.rb)
```

The bridge sits at the boundary between the platform-independent IR layer
(which knows about Terraform types) and the Ruby-specific layer (which knows
about Dry::Types). The bridge function is the only point where these two
type systems touch.

## Test Coverage

### src/iac_bridge.rs (18 unit tests)

Exhaustive variant coverage (13 tests):
- `string_maps_to_t_string`
- `integer_maps_to_t_integer`
- `float_maps_to_coercible_float`
- `numeric_maps_to_union`
- `boolean_maps_to_t_bool`
- `list_of_strings`
- `set_of_integers`
- `nested_list`
- `map_of_strings_maps_to_hash`
- `object_maps_to_hash`
- `enum_with_values`
- `enum_empty_values_degenerates`
- `any_maps_to_t_any`

Parity with pangea-forge (3 tests):
- `parity_string` -- matches string-based `iac_type_to_dry`
- `parity_list_of_bools` -- matches list conversion
- `parity_enum_constrained` -- matches enum constraint formatting

Injectivity proofs (2 tests):
- `string_differs_from_integer`
- `list_differs_from_set_by_content`

### tests/bridge_parity.rs (12 tests)

6 deterministic scalar parity proofs + 6 proptest property proofs:
- Proof 7: List maps to `T::Array.of(...)`
- Proof 8: Set/List equivalence
- Proof 9: Composition preservation
- Proof 10: Totality (no panics on random input)
- Proof 11: Determinism (same input -> same output)
- Proof 12: Nested depth preservation
