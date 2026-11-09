# Gleam Native Backend Roadmap

This document tracks the implementation progress of the Cranelift-based native backend for Gleam.

## Goal

Create Go-like standalone native binaries from Gleam source code:
- Single binary output (~2MB for hello world)
- No runtime dependencies
- Fast startup time
- Cross-compilation support

## Phase 1: Foundation (Current)

Core infrastructure and basic compilation.

### Completed
- [x] Project setup and Cranelift integration
- [x] Basic module compilation structure
- [x] Function declaration and definition
- [x] Integer literals
- [x] Float literals
- [x] Variable references
- [x] Integer arithmetic (+, -, *, /, %)
- [x] Float arithmetic (+, -, *, /)
- [x] Integer comparisons (<, <=, >, >=, ==, !=)
- [x] Float comparisons
- [x] Boolean operations (&&, ||)
- [x] Negation (- for ints, ! for bools)
- [x] Let bindings (simple variable patterns)
- [x] Function calls (same module)
- [x] Block expressions
- [x] Pipeline expressions
- [x] Empty tuples (Nil)

### In Progress
- [x] Integration tests with actual Gleam code (39 tests passing!)
- [ ] End-to-end test: compile and execute simple program

### Not Started
- [ ] Echo/print (requires runtime)
- [ ] Panic/todo (requires trap handling)

## Phase 2: Control Flow

Pattern matching and control flow constructs.

### Not Started
- [ ] Case expressions (simple patterns)
- [ ] Boolean case (if/else equivalent)
- [ ] Integer patterns
- [ ] Variable patterns in case
- [ ] Wildcard patterns (_)
- [ ] Guard clauses
- [ ] Nested patterns

## Phase 3: Data Structures

Heap-allocated data types.

### Not Started
- [ ] Memory allocator integration
- [ ] Reference counting setup
- [ ] Tuples (non-empty)
- [ ] Tuple access by index
- [ ] Lists (Cons/Nil)
- [ ] List literals
- [ ] Custom types (tagged unions)
- [ ] Record creation
- [ ] Record field access
- [ ] Record update syntax

## Phase 4: Strings

String support requires runtime functions.

### Not Started
- [ ] String representation (UTF-8)
- [ ] String literals
- [ ] String concatenation (<>)
- [ ] String comparison
- [ ] String to/from UTF-8 bytes

## Phase 5: Functions

Advanced function features.

### Not Started
- [ ] Anonymous functions (closures)
- [ ] Capturing variables
- [ ] Indirect function calls
- [ ] Tail call optimization
- [ ] Partial application

## Phase 6: Standard Library

Core library functions.

### Not Started
- [ ] gleam/io (print, println)
- [ ] gleam/int (basic operations)
- [ ] gleam/float (basic operations)
- [ ] gleam/string (basic operations)
- [ ] gleam/list (map, filter, fold)
- [ ] gleam/result (ok, error, map)
- [ ] gleam/option (some, none, map)

## Phase 7: Build Integration

Integration with Gleam build system.

### Not Started
- [ ] CLI flag: `gleam build --target native`
- [ ] Object file output (.o)
- [ ] Linking to executable
- [ ] Cross-compilation support
- [ ] Release/debug builds
- [ ] Optimization levels

## Phase 8: Advanced Features

Nice-to-have features.

### Not Started
- [ ] Bit arrays
- [ ] External functions (@external(native, ...))
- [ ] C FFI
- [ ] Multi-module compilation
- [ ] Incremental compilation
- [ ] Debug info (DWARF)
- [ ] Stack traces

## Architecture Decisions

### Type Representation
- All values currently represented as i64
- Future: tagged pointers for heap values
- Floats will need separate handling (f64)

### Memory Management
- Phase 1-2: Stack only
- Phase 3+: Reference counting (like Swift)
- Alternative: Arena allocation per request

### Calling Convention
- Using system default (SystemV on Unix, Windows on Windows)
- All args/returns as i64 for now
- Future: proper struct passing

## Non-Goals (for now)

- Processes/actors (BEAM feature)
- Message passing (BEAM feature)
- Hot code reloading (BEAM feature)
- OTP supervision (BEAM feature)
- Distribution (BEAM feature)

These are fundamental BEAM features. The native backend targets a different use case:
CLI tools, WASM compilation, embedded systems.

## Testing Strategy

1. **Unit tests**: Individual expression compilation
2. **Integration tests**: Full module compilation
3. **Execution tests**: Compile and run, check output
4. **Comparison tests**: Same code on BEAM and native should produce same results

## Success Metrics

- [ ] Compile and run "Hello, World!"
- [ ] Compile and run FizzBuzz
- [ ] Compile and run a recursive fibonacci
- [ ] Binary size under 5MB for simple programs
- [ ] Startup time under 10ms
- [ ] Pass Gleam's stdlib test suite (adapted for native)

## Resources

- [Cranelift IR Reference](https://github.com/bytecodealliance/wasmtime/blob/main/cranelift/docs/ir.md)
- [Cranelift Tutorial](https://github.com/bytecodealliance/simplejit-demo)
- [Go runtime internals](https://go.dev/src/runtime/) (for reference)
- [Swift ARC](https://docs.swift.org/swift-book/LanguageGuide/AutomaticReferenceCounting.html)
