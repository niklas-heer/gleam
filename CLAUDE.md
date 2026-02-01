# Gleam Native Backend Development Guide

This fork adds a native code generation backend to Gleam using Cranelift.

## Development Workflow

### Test-Driven Development (Red-Green-Refactor)

We follow TDD for this project:

1. **Red**: Write a failing test first that describes the desired behavior
2. **Green**: Write the minimum code needed to make the test pass
3. **Refactor**: Clean up the code while keeping tests passing

### Running Tests

```bash
# Run all tests
cargo test -p gleam-core

# Run only cranelift backend tests
cargo test -p gleam-core cranelift

# Run a specific test
cargo test -p gleam-core cranelift::tests::test_name
```

### Building

```bash
# Check compilation (fast)
cargo check -p gleam-core

# Build debug
cargo build

# Build release
cargo build --release
```

## Project Structure

```
compiler-core/src/
├── cranelift.rs          # Main native backend module
├── cranelift/
│   ├── expression.rs     # Expression compilation
│   └── tests.rs          # Backend tests
├── javascript.rs         # Reference: JS backend
└── erlang.rs             # Reference: Erlang backend
```

## Key Files

- `ROADMAP.md` - Implementation progress and milestones
- `compiler-core/src/cranelift.rs` - Entry point for native compilation
- `compiler-core/src/cranelift/expression.rs` - TypedExpr → Cranelift IR

## Adding New Features

1. Add a test in `cranelift/tests.rs` that exercises the feature
2. Run `cargo test` - watch it fail
3. Implement the feature in `expression.rs` or `cranelift.rs`
4. Run `cargo test` - watch it pass
5. Refactor if needed
6. Update `ROADMAP.md` to mark the feature complete

## Code Style

- Follow existing Gleam compiler patterns
- Use `#[allow(unused)]` sparingly - prefer removing dead code
- Handle all TypedExpr variants (use `Error::NotImplemented` for unimplemented)
- Add doc comments for public functions

## Common Tasks

### Adding support for a new expression type

1. Find the TypedExpr variant in `ast/typed.rs`
2. Add a test case in `cranelift/tests.rs`
3. Add a match arm in `expression.rs::compile_expr()`
4. Implement the Cranelift IR generation

### Adding a new binary operation

1. Check BinOp enum in `ast.rs`
2. Add test case
3. Add match arm in `compile_binop()`

### Debugging Cranelift IR

```rust
// Print the generated IR for debugging
println!("{}", builder.func.display());
```

## Architecture Notes

### Type Representation

Currently all values are represented as `i64`. This is a simplification.

Future work:
- Proper tagged pointers for heap values
- Separate float representation (f64)
- Reference counting or GC for heap objects

### Memory Model

Currently: Stack only (no heap allocation)

Future work:
- Heap allocation for lists, tuples, strings
- Reference counting or tracing GC
- Arena allocator for short-lived values

## References

- [Cranelift Documentation](https://cranelift.dev/)
- [Gleam Language Reference](https://gleam.run/documentation/)
- [Similar: JavaScript backend](../javascript.rs)
