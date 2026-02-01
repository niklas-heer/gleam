//! Tests for the Cranelift native code generation backend.
//!
//! These tests verify that Gleam code compiles successfully to native code
//! via Cranelift. We follow TDD: write failing tests first, then implement.

use std::collections::HashMap;

use crate::{
    analyse::TargetSupport,
    build::{Origin, Target},
    config::PackageConfig,
    line_numbers::LineNumbers,
    type_::PRELUDE_MODULE_NAME,
    uid::UniqueIdGenerator,
    warning::{TypeWarningEmitter, WarningEmitter},
};
use camino::Utf8PathBuf;

use super::{compile, Config};

/// Compile Gleam source code to a typed module.
fn compile_module(src: &str) -> crate::ast::TypedModule {
    let mut modules = im::HashMap::new();
    let ids = UniqueIdGenerator::new();

    // Insert prelude
    let _ = modules.insert(
        PRELUDE_MODULE_NAME.into(),
        crate::type_::build_prelude(&ids),
    );

    let parsed = crate::parse::parse_module(
        Utf8PathBuf::from("test/path"),
        src,
        &WarningEmitter::null(),
    )
    .expect("syntax error");

    let mut ast = parsed.module;
    ast.name = "my/mod".into();
    let line_numbers = LineNumbers::new(src);
    let mut config = PackageConfig::default();
    config.name = "testpackage".into();

    crate::analyse::ModuleAnalyzerConstructor::<()> {
        target: Target::JavaScript, // Use JS target for now (no BEAM-specific features)
        ids: &ids,
        origin: Origin::Src,
        importable_modules: &modules,
        warnings: &TypeWarningEmitter::null(),
        direct_dependencies: &HashMap::new(),
        dev_dependencies: &std::collections::HashSet::new(),
        target_support: TargetSupport::NotEnforced,
        package_config: &config,
    }
    .infer_module(ast, line_numbers, "src/module.gleam".into())
    .expect("type error")
}

/// Compile Gleam source to native object code bytes.
/// Returns Ok(bytes) on success, Err(message) on failure.
fn compile_native(src: &str) -> Result<Vec<u8>, String> {
    let module = compile_module(src);
    let line_numbers = LineNumbers::new(src);
    let config = Config::default();

    compile(&module, &line_numbers, config).map_err(|e| e.to_string())
}

/// Assert that the given Gleam source compiles successfully to native code.
macro_rules! assert_native_ok {
    ($src:expr) => {{
        let result = compile_native($src);
        match result {
            Ok(bytes) => {
                assert!(!bytes.is_empty(), "compiled code should not be empty");
            }
            Err(e) => {
                panic!("compilation failed: {}\n\nSource:\n{}", e, $src);
            }
        }
    }};
}

/// Assert that compilation fails with NotImplemented for the given source.
macro_rules! assert_native_not_implemented {
    ($src:expr, $feature:expr) => {{
        let result = compile_native($src);
        match result {
            Ok(_) => {
                panic!(
                    "expected NotImplemented error for '{}', but compilation succeeded",
                    $feature
                );
            }
            Err(e) => {
                assert!(
                    e.contains("Not implemented"),
                    "expected NotImplemented error, got: {}",
                    e
                );
            }
        }
    }};
}

// =============================================================================
// Phase 1: Literals and Basic Expressions
// =============================================================================

#[test]
fn int_literal() {
    assert_native_ok!(
        r#"
pub fn main() {
    42
}
"#
    );
}

#[test]
fn int_literal_negative() {
    assert_native_ok!(
        r#"
pub fn main() {
    -42
}
"#
    );
}

#[test]
fn int_literals_various() {
    assert_native_ok!(
        r#"
pub fn go() {
    1
    2
    -3
    4001
    0b00001111
    0o17
    0xF
    1_000
}
"#
    );
}

#[test]
fn float_literal() {
    assert_native_ok!(
        r#"
pub fn main() {
    3.14
}
"#
    );
}

#[test]
fn float_literals_various() {
    assert_native_ok!(
        r#"
pub fn go() {
    1.5
    2.0
    -0.1
    1.
}
"#
    );
}

// =============================================================================
// Phase 1: Arithmetic Operations
// =============================================================================

#[test]
fn int_addition() {
    assert_native_ok!(
        r#"
pub fn main() {
    1 + 2
}
"#
    );
}

#[test]
fn int_subtraction() {
    assert_native_ok!(
        r#"
pub fn main() {
    5 - 3
}
"#
    );
}

#[test]
fn int_multiplication() {
    assert_native_ok!(
        r#"
pub fn main() {
    4 * 5
}
"#
    );
}

#[test]
fn int_division() {
    assert_native_ok!(
        r#"
pub fn main() {
    10 / 3
}
"#
    );
}

#[test]
fn int_remainder() {
    assert_native_ok!(
        r#"
pub fn main() {
    10 % 3
}
"#
    );
}

#[test]
fn int_operators_combined() {
    assert_native_ok!(
        r#"
pub fn go() {
    1 + 1
    5 - 1
    5 / 2
    3 * 3
    5 % 2
}
"#
    );
}

#[test]
fn float_addition() {
    assert_native_ok!(
        r#"
pub fn main() {
    1.0 +. 2.5
}
"#
    );
}

#[test]
fn float_subtraction() {
    assert_native_ok!(
        r#"
pub fn main() {
    5.0 -. 3.0
}
"#
    );
}

#[test]
fn float_multiplication() {
    assert_native_ok!(
        r#"
pub fn main() {
    4.0 *. 2.5
}
"#
    );
}

#[test]
fn float_division() {
    assert_native_ok!(
        r#"
pub fn main() {
    10.0 /. 4.0
}
"#
    );
}

#[test]
fn float_operators_combined() {
    assert_native_ok!(
        r#"
pub fn go() {
    1.0 +. 1.4
    5.0 -. 1.5
    5.0 /. 2.0
    3.0 *. 3.1
}
"#
    );
}

// =============================================================================
// Phase 1: Comparison Operations
// =============================================================================

#[test]
fn int_comparisons() {
    assert_native_ok!(
        r#"
pub fn go() {
    2 > 1
    2 < 1
    2 >= 1
    2 <= 1
}
"#
    );
}

#[test]
fn int_equality() {
    assert_native_ok!(
        r#"
pub fn go() {
    1 != 2
    1 == 2
}
"#
    );
}

#[test]
fn float_comparisons() {
    assert_native_ok!(
        r#"
pub fn go() {
    2.0 >. 1.0
    2.0 <. 1.0
    2.0 >=. 1.0
    2.0 <=. 1.0
}
"#
    );
}

#[test]
fn float_equality() {
    assert_native_ok!(
        r#"
pub fn go() {
    1.0 != 2.0
    1.0 == 2.0
}
"#
    );
}

// =============================================================================
// Phase 1: Boolean Operations
// =============================================================================

#[test]
fn bool_and() {
    assert_native_ok!(
        r#"
pub fn main() {
    True && False
}
"#
    );
}

#[test]
fn bool_or() {
    assert_native_ok!(
        r#"
pub fn main() {
    True || False
}
"#
    );
}

#[test]
fn bool_negation() {
    assert_native_ok!(
        r#"
pub fn main() {
    !True
}
"#
    );
}

// =============================================================================
// Phase 1: Let Bindings
// =============================================================================

#[test]
fn let_binding_simple() {
    assert_native_ok!(
        r#"
pub fn main() {
    let x = 42
    x
}
"#
    );
}

#[test]
fn let_binding_multiple() {
    assert_native_ok!(
        r#"
pub fn main() {
    let x = 1
    let y = 2
    x + y
}
"#
    );
}

#[test]
fn let_binding_shadowing() {
    assert_native_ok!(
        r#"
pub fn main() {
    let x = 1
    let x = x + 1
    x
}
"#
    );
}

// =============================================================================
// Phase 1: Functions
// =============================================================================

#[test]
fn function_no_args() {
    assert_native_ok!(
        r#"
pub fn main() {
    42
}
"#
    );
}

#[test]
fn function_with_args() {
    assert_native_ok!(
        r#"
pub fn add(a, b) {
    a + b
}
"#
    );
}

#[test]
fn function_call() {
    assert_native_ok!(
        r#"
fn helper() {
    42
}

pub fn main() {
    helper()
}
"#
    );
}

#[test]
fn function_call_with_args() {
    assert_native_ok!(
        r#"
fn add(a, b) {
    a + b
}

pub fn main() {
    add(1, 2)
}
"#
    );
}

#[test]
fn recursive_function() {
    assert_native_ok!(
        r#"
pub fn factorial(n) {
    factorial(n - 1)
}
"#
    );
}

#[test]
fn multiple_functions() {
    assert_native_ok!(
        r#"
fn double(x) {
    x * 2
}

fn triple(x) {
    x * 3
}

pub fn main() {
    double(triple(5))
}
"#
    );
}

// =============================================================================
// Phase 1: Blocks
// =============================================================================

#[test]
fn block_expression() {
    assert_native_ok!(
        r#"
pub fn main() {
    {
        let x = 1
        let y = 2
        x + y
    }
}
"#
    );
}

// =============================================================================
// Phase 1: Empty Tuple (Nil)
// =============================================================================

#[test]
fn nil_literal() {
    assert_native_ok!(
        r#"
pub fn main() {
    Nil
}
"#
    );
}

// =============================================================================
// Phase 2: Not Yet Implemented Features
// These tests document what we expect to fail until implemented.
// =============================================================================

#[test]
fn strings_not_implemented() {
    assert_native_not_implemented!(
        r#"
pub fn main() {
    "hello"
}
"#,
        "strings"
    );
}

#[test]
fn lists_not_implemented() {
    assert_native_not_implemented!(
        r#"
pub fn main() {
    [1, 2, 3]
}
"#,
        "lists"
    );
}

#[test]
fn tuples_not_implemented() {
    assert_native_not_implemented!(
        r#"
pub fn main() {
    #(1, 2)
}
"#,
        "tuples"
    );
}

#[test]
fn case_not_implemented() {
    assert_native_not_implemented!(
        r#"
pub fn main() {
    case 1 {
        1 -> 2
        _ -> 3
    }
}
"#,
        "case expressions"
    );
}

#[test]
fn anonymous_function_not_implemented() {
    assert_native_not_implemented!(
        r#"
pub fn main() {
    fn() { 42 }
}
"#,
        "anonymous functions"
    );
}
