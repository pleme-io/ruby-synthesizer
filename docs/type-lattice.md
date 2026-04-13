# Type Lattice Theory

`RubyType` forms a partial order that maps to Ruby's `Dry::Types` type system.
The partial order is: type A <= type B iff every value A accepts, B also accepts.

## The Seven Variants

```rust
pub enum RubyType {
    Simple(String),                        // T::String, T::Integer, T::Bool
    Array(Box<RubyType>),                  // T::Array.of(inner)
    Hash,                                  // T::Hash
    Union(Vec<RubyType>),                  // (T::X | T::Y)
    Constrained { base, constraint },      // T::String.constrained(...)
    Optional(Box<RubyType>),               // inner.optional
    Any,                                   // T::Any
}
```

## Lattice Operations

### Optional Lifts (Join / Upper Bound)

`optional(x)` accepts everything `x` accepts, plus `nil`. It is strictly wider:

```
optional(x) >= x
```

Verified structurally: `optional(x).emit()` is `x.emit() + ".optional"`.
The base emit always appears as a prefix of the optional emit.

**Idempotency** -- applying optional twice collapses to once:

```rust
RubyType::optional(RubyType::optional(x)) == RubyType::optional(x)
```

This is enforced at construction time in `RubyType::optional()`:
```rust
pub fn optional(inner: Self) -> Self {
    if matches!(inner, Self::Optional(_)) {
        return inner; // idempotent
    }
    Self::Optional(Box::new(inner))
}
```

No `.optional.optional` can ever appear in emitted output. This is proven by
proptest across all recursive type shapes (tests/properties.rs, tests/lattice.rs,
tests/compiler_guarantees.rs).

In lattice terms, `optional` is a closure operator: `join(x, optional(x)) == optional(x)`.
The join is absorbed because `optional(x)` is already the upper bound.

### Constrained Narrows (Meet / Lower Bound)

`constrained(x, c)` restricts `x` to values satisfying constraint `c`. It is strictly narrower:

```
constrained(x, c) <= x
```

Verified structurally: `constrained(x, c).emit()` starts with `x.emit()` and appends
`.constrained(c)`. The constrained emit is always strictly longer than the base emit.

Constraints map to Dry::Types predicates:
- `included_in: ['tcp', 'udp']` -- enum restriction
- `min_size?: 1` -- minimum length
- `format?: /^[a-z]+$/` -- regex format
- `gt?: 0`, `lt?: 1000` -- numeric bounds
- `filled?: true` -- non-empty

**Base preservation** -- constraining never alters the base type prefix:
```
constrained(x, c).emit() == x.emit() + ".constrained(" + c + ")"
```

This exact equality is proven in tests/lattice.rs `constrained_preserves_base_structure`.

### Union Contains Both (Upper Bound of Variants)

`union(a, b)` accepts any value that either `a` or `b` accepts:

```
union(a, b) >= a
union(a, b) >= b
```

Verified structurally: `union(a, b).emit()` contains both `a.emit()` and `b.emit()`
as substrings, separated by ` | ` and wrapped in parentheses.

**Invariants:**
- 0 variants: panics (invalid -- use a specific type)
- 1 variant: degenerates to that variant (no wrapping)
- 2+ variants: produces `Union` with parenthesized pipe syntax

**Separator count** -- a union of N variants has exactly N-1 top-level ` | ` separators.
This is verified by counting pipes at paren depth 1 in tests/type_algebra.rs.

### Transitive Chains

The ordering composes transitively:

```
optional(constrained(x, c)) >= constrained(x, c) >= x
```

Verified: `optional(constrained(x)).emit()` starts with `x.emit()`.
The full chain is proven in tests/lattice.rs `optional_constrained_transitive`.

### Array Monotonicity

The `Array` type constructor is a functor that preserves ordering:

```
if constrained(x, c) <= x, then array(constrained(x, c)) <= array(x)
```

Verified: `array(constrained(x)).emit()` is strictly longer than `array(x).emit()`,
and both contain `x.emit()`. The array functor does not collapse or reorder inner types.

## Mapping to Dry::Types

| Lattice operation | Dry::Types equivalent | Example |
|-------------------|----------------------|---------|
| `optional(x)` | `x.optional` | `T::String.optional` |
| `constrained(x, c)` | `x.constrained(c)` | `T::String.constrained(min_size?: 1)` |
| `union(a, b)` | `(a \| b)` | `(T::String \| T::Integer)` |
| `array(x)` | `T::Array.of(x)` | `T::Array.of(T::String)` |

The emit function is a homomorphism from the `RubyType` lattice to the string
representation of Dry::Types expressions. Every lattice property proven on the
AST side holds identically in the emitted Ruby string.

## Reflexivity and Determinism

- **Reflexivity**: `x.emit() == x.emit()` -- same element is `<=` itself
- **Determinism**: calling `emit()` multiple times produces identical output

Both are proven via proptest with 1000 random cases per property.

## Test Coverage

13 proptest properties in tests/lattice.rs:

1. Optional widens by appending `.optional` suffix
2. Optional upper bound contains base as prefix
3. Constrained narrows -- starts with base
4. Constrained lower bound -- strictly longer than base
5. Union contains first variant
6. Union contains second variant
7. Union wrapped in parens
8. Union uses pipe separator
9. Reflexivity (deterministic emit)
10. Optional idempotent join
11. Constrained preserves base structure (exact equality)
12. Optional-constrained transitive chain
13. Array monotone over constrained
