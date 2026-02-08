use std::collections::HashMap;

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{types, InstBuilder, Value};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::Module;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::typeck::env::TypeEnv;
use crate::typeck::types::PlutoType;

/// Lower a function body into Cranelift IR.
pub fn lower_function(
    func: &Function,
    mut builder: FunctionBuilder<'_>,
    env: &TypeEnv,
    module: &mut dyn Module,
    func_ids: &HashMap<String, cranelift_module::FuncId>,
) -> Result<(), CompileError> {
    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);
    builder.seal_block(entry_block);

    let mut variables = HashMap::new();
    let mut var_types = HashMap::new();
    let mut next_var = 0u32;

    // Declare parameters as variables
    for (i, param) in func.params.iter().enumerate() {
        let ty = pluto_to_cranelift(&resolve_param_type(param));
        let var = Variable::from_u32(next_var);
        next_var += 1;
        builder.declare_var(var, ty);
        let val = builder.block_params(entry_block)[i];
        builder.def_var(var, val);
        variables.insert(param.name.node.clone(), var);
        var_types.insert(param.name.node.clone(), resolve_param_type(param));
    }

    // Lower body statements
    let is_main = func.name.node == "main";
    let mut terminated = false;

    for stmt in &func.body.node.stmts {
        if terminated {
            break;
        }
        let stmt_terminates = matches!(stmt.node, Stmt::Return(_));
        lower_stmt(
            &stmt.node,
            &mut builder,
            env,
            module,
            &mut variables,
            &mut var_types,
            &mut next_var,
            func_ids,
            &mut terminated,
        )?;
        if stmt_terminates {
            terminated = true;
        }
    }

    // If main and no explicit return, return 0
    if is_main && !terminated {
        let zero = builder.ins().iconst(types::I64, 0);
        builder.ins().return_(&[zero]);
    } else if !terminated {
        // Void function with no return
        let ret_type = env.functions.get(&func.name.node).map(|s| &s.return_type);
        if ret_type == Some(&PlutoType::Void) {
            builder.ins().return_(&[]);
        }
    }

    builder.finalize();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn lower_stmt(
    stmt: &Stmt,
    builder: &mut FunctionBuilder<'_>,
    env: &TypeEnv,
    module: &mut dyn Module,
    variables: &mut HashMap<String, Variable>,
    var_types: &mut HashMap<String, PlutoType>,
    next_var: &mut u32,
    func_ids: &HashMap<String, cranelift_module::FuncId>,
    terminated: &mut bool,
) -> Result<(), CompileError> {
    if *terminated {
        return Ok(());
    }
    match stmt {
        Stmt::Let { name, value, .. } => {
            let val = lower_expr(&value.node, builder, env, module, variables, var_types, func_ids)?;
            let val_type = infer_type_for_expr(&value.node, env, var_types);
            let cl_type = pluto_to_cranelift(&val_type);

            let var = Variable::from_u32(*next_var);
            *next_var += 1;
            builder.declare_var(var, cl_type);
            builder.def_var(var, val);
            variables.insert(name.node.clone(), var);
            var_types.insert(name.node.clone(), val_type);
        }
        Stmt::Return(value) => {
            match value {
                Some(expr) => {
                    let val = lower_expr(&expr.node, builder, env, module, variables, var_types, func_ids)?;
                    builder.ins().return_(&[val]);
                }
                None => {
                    builder.ins().return_(&[]);
                }
            }
            *terminated = true;
        }
        Stmt::Assign { target, value } => {
            let val = lower_expr(&value.node, builder, env, module, variables, var_types, func_ids)?;
            let var = variables.get(&target.node).ok_or_else(|| {
                CompileError::codegen(format!("undefined variable '{}'", target.node))
            })?;
            builder.def_var(*var, val);
        }
        Stmt::If { condition, then_block, else_block } => {
            let cond_val = lower_expr(&condition.node, builder, env, module, variables, var_types, func_ids)?;

            let then_bb = builder.create_block();
            let merge_bb = builder.create_block();

            if let Some(else_blk) = else_block {
                let else_bb = builder.create_block();
                builder.ins().brif(cond_val, then_bb, &[], else_bb, &[]);

                // Then branch
                builder.switch_to_block(then_bb);
                builder.seal_block(then_bb);
                let mut then_terminated = false;
                for s in &then_block.node.stmts {
                    lower_stmt(&s.node, builder, env, module, variables, var_types, next_var, func_ids, &mut then_terminated)?;
                }
                if !then_terminated {
                    builder.ins().jump(merge_bb, &[]);
                }

                // Else branch
                builder.switch_to_block(else_bb);
                builder.seal_block(else_bb);
                let mut else_terminated = false;
                for s in &else_blk.node.stmts {
                    lower_stmt(&s.node, builder, env, module, variables, var_types, next_var, func_ids, &mut else_terminated)?;
                }
                if !else_terminated {
                    builder.ins().jump(merge_bb, &[]);
                }

                if then_terminated && else_terminated {
                    *terminated = true;
                }
            } else {
                builder.ins().brif(cond_val, then_bb, &[], merge_bb, &[]);

                builder.switch_to_block(then_bb);
                builder.seal_block(then_bb);
                let mut then_terminated = false;
                for s in &then_block.node.stmts {
                    lower_stmt(&s.node, builder, env, module, variables, var_types, next_var, func_ids, &mut then_terminated)?;
                }
                if !then_terminated {
                    builder.ins().jump(merge_bb, &[]);
                }
            }

            if !*terminated {
                builder.switch_to_block(merge_bb);
                builder.seal_block(merge_bb);
            }
        }
        Stmt::While { condition, body } => {
            let header_bb = builder.create_block();
            let body_bb = builder.create_block();
            let exit_bb = builder.create_block();

            builder.ins().jump(header_bb, &[]);

            builder.switch_to_block(header_bb);
            let cond_val = lower_expr(&condition.node, builder, env, module, variables, var_types, func_ids)?;
            builder.ins().brif(cond_val, body_bb, &[], exit_bb, &[]);

            builder.switch_to_block(body_bb);
            builder.seal_block(body_bb);
            let mut body_terminated = false;
            for s in &body.node.stmts {
                lower_stmt(&s.node, builder, env, module, variables, var_types, next_var, func_ids, &mut body_terminated)?;
            }
            if !body_terminated {
                builder.ins().jump(header_bb, &[]);
            }

            builder.seal_block(header_bb);
            builder.switch_to_block(exit_bb);
            builder.seal_block(exit_bb);
        }
        Stmt::Expr(expr) => {
            lower_expr(&expr.node, builder, env, module, variables, var_types, func_ids)?;
        }
    }
    Ok(())
}

fn lower_expr(
    expr: &Expr,
    builder: &mut FunctionBuilder<'_>,
    env: &TypeEnv,
    module: &mut dyn Module,
    variables: &HashMap<String, Variable>,
    var_types: &HashMap<String, PlutoType>,
    func_ids: &HashMap<String, cranelift_module::FuncId>,
) -> Result<Value, CompileError> {
    match expr {
        Expr::IntLit(n) => Ok(builder.ins().iconst(types::I64, *n)),
        Expr::FloatLit(n) => Ok(builder.ins().f64const(*n)),
        Expr::BoolLit(b) => Ok(builder.ins().iconst(types::I8, if *b { 1 } else { 0 })),
        Expr::StringLit(_) => {
            // v0.1 placeholder — strings not needed yet
            Ok(builder.ins().iconst(types::I64, 0))
        }
        Expr::Ident(name) => {
            let var = variables.get(name).ok_or_else(|| {
                CompileError::codegen(format!("undefined variable '{name}'"))
            })?;
            Ok(builder.use_var(*var))
        }
        Expr::BinOp { op, lhs, rhs } => {
            let l = lower_expr(&lhs.node, builder, env, module, variables, var_types, func_ids)?;
            let r = lower_expr(&rhs.node, builder, env, module, variables, var_types, func_ids)?;

            let lhs_type = infer_type_for_expr(&lhs.node, env, var_types);
            let is_float = lhs_type == PlutoType::Float;

            let result = match op {
                BinOp::Add if is_float => builder.ins().fadd(l, r),
                BinOp::Add => builder.ins().iadd(l, r),
                BinOp::Sub if is_float => builder.ins().fsub(l, r),
                BinOp::Sub => builder.ins().isub(l, r),
                BinOp::Mul if is_float => builder.ins().fmul(l, r),
                BinOp::Mul => builder.ins().imul(l, r),
                BinOp::Div if is_float => builder.ins().fdiv(l, r),
                BinOp::Div => builder.ins().sdiv(l, r),
                BinOp::Mod => builder.ins().srem(l, r),
                BinOp::Eq if is_float => builder.ins().fcmp(FloatCC::Equal, l, r),
                BinOp::Eq => builder.ins().icmp(IntCC::Equal, l, r),
                BinOp::Neq if is_float => builder.ins().fcmp(FloatCC::NotEqual, l, r),
                BinOp::Neq => builder.ins().icmp(IntCC::NotEqual, l, r),
                BinOp::Lt if is_float => builder.ins().fcmp(FloatCC::LessThan, l, r),
                BinOp::Lt => builder.ins().icmp(IntCC::SignedLessThan, l, r),
                BinOp::Gt if is_float => builder.ins().fcmp(FloatCC::GreaterThan, l, r),
                BinOp::Gt => builder.ins().icmp(IntCC::SignedGreaterThan, l, r),
                BinOp::LtEq if is_float => builder.ins().fcmp(FloatCC::LessThanOrEqual, l, r),
                BinOp::LtEq => builder.ins().icmp(IntCC::SignedLessThanOrEqual, l, r),
                BinOp::GtEq if is_float => builder.ins().fcmp(FloatCC::GreaterThanOrEqual, l, r),
                BinOp::GtEq => builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, l, r),
                BinOp::And => builder.ins().band(l, r),
                BinOp::Or => builder.ins().bor(l, r),
            };
            Ok(result)
        }
        Expr::UnaryOp { op, operand } => {
            let val = lower_expr(&operand.node, builder, env, module, variables, var_types, func_ids)?;
            let operand_type = infer_type_for_expr(&operand.node, env, var_types);
            match op {
                UnaryOp::Neg if operand_type == PlutoType::Float => Ok(builder.ins().fneg(val)),
                UnaryOp::Neg => Ok(builder.ins().ineg(val)),
                UnaryOp::Not => {
                    let one = builder.ins().iconst(types::I8, 1);
                    Ok(builder.ins().bxor(val, one))
                }
            }
        }
        Expr::Call { name, args } => {
            let func_id = func_ids.get(&name.node).ok_or_else(|| {
                CompileError::codegen(format!("undefined function '{}'", name.node))
            })?;

            let func_ref = module.declare_func_in_func(*func_id, builder.func);
            let mut arg_values = Vec::new();
            for arg in args {
                arg_values.push(lower_expr(&arg.node, builder, env, module, variables, var_types, func_ids)?);
            }

            let call = builder.ins().call(func_ref, &arg_values);
            let results = builder.inst_results(call);
            if results.is_empty() {
                // Void function — return a dummy value
                Ok(builder.ins().iconst(types::I64, 0))
            } else {
                Ok(results[0])
            }
        }
    }
}

fn resolve_param_type(param: &Param) -> PlutoType {
    match &param.ty.node {
        TypeExpr::Named(name) => match name.as_str() {
            "int" => PlutoType::Int,
            "float" => PlutoType::Float,
            "bool" => PlutoType::Bool,
            "string" => PlutoType::String,
            _ => PlutoType::Void,
        },
    }
}

pub fn pluto_to_cranelift(ty: &PlutoType) -> types::Type {
    match ty {
        PlutoType::Int => types::I64,
        PlutoType::Float => types::F64,
        PlutoType::Bool => types::I8,
        PlutoType::String => types::I64, // pointer
        PlutoType::Void => types::I64,   // shouldn't be used for values
    }
}

/// Quick type inference at codegen time (type checker has already validated).
fn infer_type_for_expr(expr: &Expr, env: &TypeEnv, var_types: &HashMap<String, PlutoType>) -> PlutoType {
    match expr {
        Expr::IntLit(_) => PlutoType::Int,
        Expr::FloatLit(_) => PlutoType::Float,
        Expr::BoolLit(_) => PlutoType::Bool,
        Expr::StringLit(_) => PlutoType::String,
        Expr::Ident(name) => var_types.get(name).cloned().unwrap_or(PlutoType::Void),
        Expr::BinOp { op, lhs, .. } => {
            match op {
                BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq | BinOp::And | BinOp::Or => PlutoType::Bool,
                _ => infer_type_for_expr(&lhs.node, env, var_types),
            }
        }
        Expr::UnaryOp { op, operand } => {
            match op {
                UnaryOp::Not => PlutoType::Bool,
                UnaryOp::Neg => infer_type_for_expr(&operand.node, env, var_types),
            }
        }
        Expr::Call { name, .. } => {
            env.functions.get(&name.node).map(|s| s.return_type.clone()).unwrap_or(PlutoType::Void)
        }
    }
}
