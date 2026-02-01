//! Expression compilation for the Cranelift backend.
//!
//! This module handles translating Gleam's TypedExpr AST nodes into
//! Cranelift IR instructions.

use std::collections::HashMap;

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{types, InstBuilder, Value};
use cranelift_frontend::FunctionBuilder;
use cranelift_module::{FuncId, Module};
use cranelift_object::ObjectModule;
use ecow::EcoString;
use num_traits::ToPrimitive;

use crate::ast::{BinOp, TypedExpr};

use super::Error;

/// Compile an expression to Cranelift IR.
pub fn compile_expr(
    expr: &TypedExpr,
    builder: &mut FunctionBuilder,
    variables: &mut HashMap<EcoString, Value>,
    function_ids: &HashMap<EcoString, FuncId>,
    object_module: &mut ObjectModule,
) -> Result<Value, Error> {
    match expr {
        TypedExpr::Int { int_value, .. } => {
            // Convert BigInt to i64
            let value = int_value
                .to_i64()
                .ok_or_else(|| Error::NotImplemented("integers larger than i64".to_string()))?;
            Ok(builder.ins().iconst(types::I64, value))
        }

        TypedExpr::Float { float_value, .. } => {
            let value = float_value.value();
            Ok(builder.ins().f64const(value))
        }

        TypedExpr::Var { name, .. } => variables
            .get(name)
            .copied()
            .ok_or_else(|| Error::Cranelift(format!("undefined variable: {name}"))),

        TypedExpr::BinOp {
            name, left, right, ..
        } => compile_binop(*name, left, right, builder, variables, function_ids, object_module),

        TypedExpr::Block { statements, .. } => {
            super::compile_statements(
                statements.as_slice(),
                builder,
                variables,
                function_ids,
                object_module,
            )
        }

        TypedExpr::Pipeline {
            first_value,
            assignments,
            finally,
            ..
        } => {
            // Compile the first value and bind it
            let first = compile_expr(&first_value.value, builder, variables, function_ids, object_module)?;
            let _ = variables.insert(first_value.name.clone(), first);

            // Compile each pipeline step
            for (assignment, _kind) in assignments {
                let value = compile_expr(&assignment.value, builder, variables, function_ids, object_module)?;
                let _ = variables.insert(assignment.name.clone(), value);
            }

            // Compile and return the final expression
            compile_expr(finally, builder, variables, function_ids, object_module)
        }

        TypedExpr::Call {
            fun, arguments, ..
        } => compile_call(fun, arguments, builder, variables, function_ids, object_module),

        TypedExpr::NegateInt { value, .. } => {
            let val = compile_expr(value, builder, variables, function_ids, object_module)?;
            Ok(builder.ins().ineg(val))
        }

        TypedExpr::NegateBool { value, .. } => {
            let val = compile_expr(value, builder, variables, function_ids, object_module)?;
            // Boolean negation: XOR with 1
            let one = builder.ins().iconst(types::I64, 1);
            Ok(builder.ins().bxor(val, one))
        }

        TypedExpr::Tuple { elements, .. } => {
            // TODO: Proper tuple representation with heap allocation
            // For now, only support empty tuples (Nil)
            if elements.is_empty() {
                Ok(builder.ins().iconst(types::I64, 0))
            } else {
                Err(Error::NotImplemented("tuples".to_string()))
            }
        }

        TypedExpr::Case { .. } => Err(Error::NotImplemented("case expressions".to_string())),

        TypedExpr::String { .. } => Err(Error::NotImplemented("strings".to_string())),

        TypedExpr::List { .. } => Err(Error::NotImplemented("lists".to_string())),

        TypedExpr::Fn { .. } => Err(Error::NotImplemented("anonymous functions".to_string())),

        TypedExpr::RecordAccess { .. } => Err(Error::NotImplemented("record access".to_string())),

        TypedExpr::PositionalAccess { .. } => {
            Err(Error::NotImplemented("positional access".to_string()))
        }

        TypedExpr::ModuleSelect { .. } => Err(Error::NotImplemented("module select".to_string())),

        TypedExpr::TupleIndex { .. } => Err(Error::NotImplemented("tuple index".to_string())),

        TypedExpr::Todo { .. } => {
            // TODO: Generate a panic/trap
            Err(Error::NotImplemented("todo".to_string()))
        }

        TypedExpr::Panic { .. } => {
            // TODO: Generate a trap instruction
            Err(Error::NotImplemented("panic".to_string()))
        }

        TypedExpr::Echo { expression, .. } => {
            // TODO: Actually print something
            // For now, just evaluate and return the expression
            if let Some(expr) = expression {
                compile_expr(expr, builder, variables, function_ids, object_module)
            } else {
                Ok(builder.ins().iconst(types::I64, 0))
            }
        }

        TypedExpr::BitArray { .. } => Err(Error::NotImplemented("bit arrays".to_string())),

        TypedExpr::RecordUpdate { .. } => Err(Error::NotImplemented("record update".to_string())),

        TypedExpr::Invalid { .. } => Err(Error::Cranelift("invalid expression in AST".to_string())),
    }
}

/// Compile a binary operation.
fn compile_binop(
    op: BinOp,
    left: &TypedExpr,
    right: &TypedExpr,
    builder: &mut FunctionBuilder,
    variables: &mut HashMap<EcoString, Value>,
    function_ids: &HashMap<EcoString, FuncId>,
    object_module: &mut ObjectModule,
) -> Result<Value, Error> {
    let l = compile_expr(left, builder, variables, function_ids, object_module)?;
    let r = compile_expr(right, builder, variables, function_ids, object_module)?;

    let result = match op {
        // Integer arithmetic
        BinOp::AddInt => builder.ins().iadd(l, r),
        BinOp::SubInt => builder.ins().isub(l, r),
        BinOp::MultInt => builder.ins().imul(l, r),
        BinOp::DivInt => builder.ins().sdiv(l, r),
        BinOp::RemainderInt => builder.ins().srem(l, r),

        // Float arithmetic
        BinOp::AddFloat => builder.ins().fadd(l, r),
        BinOp::SubFloat => builder.ins().fsub(l, r),
        BinOp::MultFloat => builder.ins().fmul(l, r),
        BinOp::DivFloat => builder.ins().fdiv(l, r),

        // Integer comparisons - these return i8 (bool), extend to i64
        BinOp::LtInt => {
            let cmp = builder.ins().icmp(IntCC::SignedLessThan, l, r);
            builder.ins().uextend(types::I64, cmp)
        }
        BinOp::LtEqInt => {
            let cmp = builder.ins().icmp(IntCC::SignedLessThanOrEqual, l, r);
            builder.ins().uextend(types::I64, cmp)
        }
        BinOp::GtInt => {
            let cmp = builder.ins().icmp(IntCC::SignedGreaterThan, l, r);
            builder.ins().uextend(types::I64, cmp)
        }
        BinOp::GtEqInt => {
            let cmp = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, l, r);
            builder.ins().uextend(types::I64, cmp)
        }

        // Float comparisons
        BinOp::LtFloat => {
            let cmp = builder.ins().fcmp(FloatCC::LessThan, l, r);
            builder.ins().uextend(types::I64, cmp)
        }
        BinOp::LtEqFloat => {
            let cmp = builder.ins().fcmp(FloatCC::LessThanOrEqual, l, r);
            builder.ins().uextend(types::I64, cmp)
        }
        BinOp::GtFloat => {
            let cmp = builder.ins().fcmp(FloatCC::GreaterThan, l, r);
            builder.ins().uextend(types::I64, cmp)
        }
        BinOp::GtEqFloat => {
            let cmp = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, l, r);
            builder.ins().uextend(types::I64, cmp)
        }

        // Equality - for now only works with integers
        // TODO: Structural equality for complex types
        BinOp::Eq => {
            let cmp = builder.ins().icmp(IntCC::Equal, l, r);
            builder.ins().uextend(types::I64, cmp)
        }
        BinOp::NotEq => {
            let cmp = builder.ins().icmp(IntCC::NotEqual, l, r);
            builder.ins().uextend(types::I64, cmp)
        }

        // Boolean operations
        BinOp::And => builder.ins().band(l, r),
        BinOp::Or => builder.ins().bor(l, r),

        // String concatenation - not yet supported
        BinOp::Concatenate => {
            return Err(Error::NotImplemented("string concatenation".to_string()));
        }
    };

    Ok(result)
}

/// Compile a function call.
fn compile_call(
    fun: &TypedExpr,
    arguments: &[crate::ast::CallArg<TypedExpr>],
    builder: &mut FunctionBuilder,
    variables: &mut HashMap<EcoString, Value>,
    function_ids: &HashMap<EcoString, FuncId>,
    object_module: &mut ObjectModule,
) -> Result<Value, Error> {
    // Compile arguments first
    let mut arg_values = Vec::with_capacity(arguments.len());
    for arg in arguments {
        arg_values.push(compile_expr(&arg.value, builder, variables, function_ids, object_module)?);
    }

    // Get the function reference
    match fun {
        TypedExpr::Var { constructor, .. } => {
            use crate::type_::ValueConstructorVariant;
            match &constructor.variant {
                ValueConstructorVariant::ModuleFn { name, .. } => {
                    // Look up the function ID
                    let func_id = function_ids.get(name).ok_or_else(|| {
                        Error::Cranelift(format!("undefined function: {name}"))
                    })?;

                    // Get the function reference
                    let func_ref = object_module.declare_func_in_func(*func_id, builder.func);

                    // Make the call
                    let call = builder.ins().call(func_ref, &arg_values);
                    let results = builder.inst_results(call);
                    if results.is_empty() {
                        Ok(builder.ins().iconst(types::I64, 0))
                    } else {
                        Ok(results[0])
                    }
                }
                ValueConstructorVariant::LocalVariable { .. } => {
                    // Calling a function stored in a variable
                    // This would require indirect calls - not yet supported
                    Err(Error::NotImplemented("indirect function calls".to_string()))
                }
                ValueConstructorVariant::Record { .. } => {
                    // Constructor call
                    Err(Error::NotImplemented("constructors".to_string()))
                }
                _ => Err(Error::NotImplemented(format!(
                    "call to {:?}",
                    constructor.variant
                ))),
            }
        }
        TypedExpr::ModuleSelect { label, .. } => {
            // Cross-module function call
            Err(Error::NotImplemented(format!(
                "cross-module calls ({})",
                label
            )))
        }
        _ => Err(Error::NotImplemented("complex function expressions".to_string())),
    }
}
