//! Native code generation backend using Cranelift.
//!
//! This module provides compilation of Gleam to native machine code via the
//! Cranelift code generator. It produces object files that can be linked into
//! standalone executables.
//!
//! ## Current Status
//!
//! This is an experimental backend. Currently supported:
//! - Integer literals
//! - Float literals
//! - Binary operations (arithmetic, comparison)
//! - Let bindings
//! - Function definitions and calls
//!
//! Not yet supported:
//! - Custom types (tagged unions)
//! - Lists
//! - Strings
//! - Pattern matching
//! - Closures
//! - Standard library

mod expression;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use cranelift_codegen::ir::{types, AbiParam, Function, InstBuilder, Signature, UserFuncName};
use cranelift_codegen::isa::TargetIsa;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::ast::{TypedFunction, TypedModule, TypedStatement};
use crate::line_numbers::LineNumbers;
use ecow::EcoString;

/// Errors that can occur during native code generation.
#[derive(Debug, Clone)]
pub enum Error {
    /// The target architecture is not supported.
    UnsupportedTarget(String),
    /// A Cranelift compilation error occurred.
    Cranelift(String),
    /// A feature is not yet implemented.
    NotImplemented(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnsupportedTarget(msg) => write!(f, "Unsupported target: {msg}"),
            Error::Cranelift(msg) => write!(f, "Cranelift error: {msg}"),
            Error::NotImplemented(msg) => write!(f, "Not implemented: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

/// Configuration for native code generation.
#[derive(Debug, Clone)]
pub struct Config {
    /// Whether to enable optimizations.
    pub optimize: bool,
    /// Target triple (e.g., "x86_64-unknown-linux-gnu").
    /// If None, uses the host target.
    pub target: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            optimize: false,
            target: None,
        }
    }
}

/// The main code generator for native compilation.
#[derive(Debug)]
pub struct Generator<'a> {
    module: &'a TypedModule,
    #[allow(dead_code)]
    line_numbers: &'a LineNumbers,
    #[allow(dead_code)]
    config: Config,
    pub object_module: ObjectModule,
    ctx: Context,
    func_ctx: FunctionBuilderContext,
    /// Maps function names to their Cranelift function IDs.
    pub function_ids: HashMap<EcoString, cranelift_module::FuncId>,
    /// Maps variable names to their Cranelift values within current function.
    pub variables: HashMap<EcoString, cranelift_codegen::ir::Value>,
    /// Counter for generating unique variable indices.
    #[allow(dead_code)]
    next_var: usize,
}

impl<'a> Generator<'a> {
    /// Create a new native code generator.
    pub fn new(
        module: &'a TypedModule,
        line_numbers: &'a LineNumbers,
        config: Config,
    ) -> Result<Self, Error> {
        let isa = Self::create_target_isa(&config)?;

        let object_builder = ObjectBuilder::new(
            isa,
            module.name.as_str(),
            cranelift_module::default_libcall_names(),
        )
        .map_err(|e| Error::Cranelift(e.to_string()))?;

        let object_module = ObjectModule::new(object_builder);

        Ok(Self {
            module,
            line_numbers,
            config,
            object_module,
            ctx: Context::new(),
            func_ctx: FunctionBuilderContext::new(),
            function_ids: HashMap::new(),
            variables: HashMap::new(),
            next_var: 0,
        })
    }

    /// Create the target ISA (Instruction Set Architecture) for code generation.
    fn create_target_isa(config: &Config) -> Result<std::sync::Arc<dyn TargetIsa>, Error> {
        let mut flag_builder = settings::builder();

        // Enable optimizations if requested
        if config.optimize {
            flag_builder
                .set("opt_level", "speed")
                .map_err(|e| Error::Cranelift(e.to_string()))?;
        }

        let flags = settings::Flags::new(flag_builder);

        // Use native target or specified target
        match &config.target {
            Some(triple) => {
                let triple = triple
                    .parse()
                    .map_err(|e: target_lexicon::ParseError| Error::UnsupportedTarget(e.to_string()))?;
                cranelift_codegen::isa::lookup(triple)
                    .map_err(|e| Error::UnsupportedTarget(e.to_string()))?
                    .finish(flags)
                    .map_err(|e| Error::Cranelift(e.to_string()))
            }
            None => cranelift_native::builder()
                .map_err(|e| Error::UnsupportedTarget(e.to_string()))?
                .finish(flags)
                .map_err(|e| Error::Cranelift(e.to_string())),
        }
    }

    /// Compile the module and return the object code as bytes.
    pub fn compile(mut self) -> Result<Vec<u8>, Error> {
        // First pass: declare all functions
        self.declare_functions()?;

        // Second pass: define all functions
        self.define_functions()?;

        // Finish and emit the object file
        let object = self.object_module.finish();
        Ok(object.emit().map_err(|e| Error::Cranelift(e.to_string()))?)
    }

    /// Declare all functions in the module (first pass).
    fn declare_functions(&mut self) -> Result<(), Error> {
        for func in &self.module.definitions.functions {
            self.declare_function(func)?;
        }
        Ok(())
    }

    /// Declare a single function.
    fn declare_function(&mut self, func: &TypedFunction) -> Result<(), Error> {
        // Get the function name (external functions may not have a name)
        let name = match &func.name {
            Some((_, name)) => name.clone(),
            None => return Ok(()), // Skip external-only functions
        };

        let signature = self.function_signature(func);

        // Determine linkage - public functions are exported
        let linkage = if func.publicity.is_public() {
            Linkage::Export
        } else {
            Linkage::Local
        };

        let func_id = self
            .object_module
            .declare_function(&name, linkage, &signature)
            .map_err(|e| Error::Cranelift(e.to_string()))?;

        let _ = self.function_ids.insert(name, func_id);
        Ok(())
    }

    /// Define all functions in the module (second pass).
    fn define_functions(&mut self) -> Result<(), Error> {
        // Collect functions to avoid borrow issues
        let functions: Vec<_> = self
            .module
            .definitions
            .functions
            .iter()
            .filter_map(|f| f.name.as_ref().map(|(_, n)| (n.clone(), f.clone())))
            .collect();

        for (name, func) in functions {
            self.define_function(&name, &func)?;
        }
        Ok(())
    }

    /// Define a single function (compile its body).
    fn define_function(&mut self, name: &EcoString, func: &TypedFunction) -> Result<(), Error> {
        let func_id = *self
            .function_ids
            .get(name)
            .expect("function should be declared");

        // Create fresh function
        self.ctx.func = Function::with_name_signature(
            UserFuncName::user(0, func_id.as_u32()),
            self.function_signature(func),
        );

        // Reset variable state for this function
        self.variables.clear();
        self.next_var = 0;

        // Build the function body
        {
            let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.func_ctx);
            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            // Bind function parameters to variables
            let params = builder.block_params(entry_block).to_vec();
            for (i, arg) in func.arguments.iter().enumerate() {
                if let Some(arg_name) = arg.names.get_variable_name() {
                    let _ = self.variables.insert(arg_name.clone(), params[i]);
                }
            }

            // Compile function body - use standalone function to avoid borrow issues
            let return_value = compile_statements(
                &func.body,
                &mut builder,
                &mut self.variables,
                &self.function_ids,
                &mut self.object_module,
            )?;

            // Return the result
            let _ = builder.ins().return_(&[return_value]);
            builder.finalize();
        }

        // Define the function in the module
        self.object_module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| Error::Cranelift(e.to_string()))?;

        // Clear the context for the next function
        self.object_module.clear_context(&mut self.ctx);

        Ok(())
    }

    /// Create a Cranelift function signature from a Gleam function.
    fn function_signature(&self, func: &TypedFunction) -> Signature {
        let call_conv = self.object_module.isa().default_call_conv();
        let mut sig = Signature::new(call_conv);

        // For now, all values are represented as i64
        // TODO: Proper type representation
        for _arg in &func.arguments {
            sig.params.push(AbiParam::new(types::I64));
        }

        // Return type - for now always i64
        // TODO: Handle Nil return type, tuples, etc.
        sig.returns.push(AbiParam::new(types::I64));

        sig
    }

    /// Compile a list of statements, returning the value of the last expression.
    pub fn compile_statements(
        &mut self,
        statements: &[TypedStatement],
        builder: &mut FunctionBuilder<'_>,
    ) -> Result<cranelift_codegen::ir::Value, Error> {
        compile_statements(
            statements,
            builder,
            &mut self.variables,
            &self.function_ids,
            &mut self.object_module,
        )
    }
}

/// Compile a list of statements (standalone function to avoid borrow issues).
fn compile_statements(
    statements: &[TypedStatement],
    builder: &mut FunctionBuilder<'_>,
    variables: &mut HashMap<EcoString, cranelift_codegen::ir::Value>,
    function_ids: &HashMap<EcoString, cranelift_module::FuncId>,
    object_module: &mut ObjectModule,
) -> Result<cranelift_codegen::ir::Value, Error> {
    let mut result = builder.ins().iconst(types::I64, 0); // Default to 0 (Nil)

    for statement in statements.iter() {
        result = compile_statement(statement, builder, variables, function_ids, object_module)?;
    }

    Ok(result)
}

/// Compile a single statement.
fn compile_statement(
    statement: &TypedStatement,
    builder: &mut FunctionBuilder<'_>,
    variables: &mut HashMap<EcoString, cranelift_codegen::ir::Value>,
    function_ids: &HashMap<EcoString, cranelift_module::FuncId>,
    object_module: &mut ObjectModule,
) -> Result<cranelift_codegen::ir::Value, Error> {
    match statement {
        TypedStatement::Expression(expr) => {
            expression::compile_expr(expr, builder, variables, function_ids, object_module)
        }
        TypedStatement::Assignment(assignment) => {
            let value = expression::compile_expr(
                &assignment.value,
                builder,
                variables,
                function_ids,
                object_module,
            )?;
            // For simple variable patterns, bind the name
            if let crate::ast::Pattern::Variable { name, .. } = &assignment.pattern {
                let _ = variables.insert(name.clone(), value);
            }
            // TODO: Handle other patterns (tuples, constructors, etc.)
            Ok(value)
        }
        TypedStatement::Use(_) => Err(Error::NotImplemented("use statements".to_string())),
        TypedStatement::Assert(_) => Err(Error::NotImplemented("assert statements".to_string())),
    }
}

/// Compile a Gleam module to native object code.
///
/// This is the main entry point for native compilation.
pub fn compile(
    module: &TypedModule,
    line_numbers: &LineNumbers,
    config: Config,
) -> Result<Vec<u8>, Error> {
    Generator::new(module, line_numbers, config)?.compile()
}
