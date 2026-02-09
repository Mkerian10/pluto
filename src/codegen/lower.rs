use std::collections::HashMap;

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::immediates::Offset32;
use cranelift_codegen::ir::{types, AbiParam, InstBuilder, MemFlags, Value};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::{DataDescription, DataId, FuncId, Module};

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::typeck::env::TypeEnv;
use crate::typeck::types::PlutoType;

/// Wrap a class pointer as a trait handle by calling __pluto_trait_wrap.
fn wrap_class_as_trait(
    class_val: Value,
    class_name: &str,
    trait_name: &str,
    builder: &mut FunctionBuilder<'_>,
    module: &mut dyn Module,
    vtable_ids: &HashMap<(String, String), DataId>,
    trait_wrap_id: FuncId,
) -> Result<Value, CompileError> {
    let vtable_data_id = vtable_ids
        .get(&(class_name.to_string(), trait_name.to_string()))
        .ok_or_else(|| {
            CompileError::codegen(format!("no vtable for ({class_name}, {trait_name})"))
        })?;
    let gv = module.declare_data_in_func(*vtable_data_id, builder.func);
    let vtable_ptr = builder.ins().global_value(types::I64, gv);
    let func_ref = module.declare_func_in_func(trait_wrap_id, builder.func);
    let call = builder.ins().call(func_ref, &[class_val, vtable_ptr]);
    Ok(builder.inst_results(call)[0])
}

/// Lower a function body into Cranelift IR.
#[allow(clippy::too_many_arguments)]
pub fn lower_function(
    func: &Function,
    mut builder: FunctionBuilder<'_>,
    env: &TypeEnv,
    module: &mut dyn Module,
    func_ids: &HashMap<String, FuncId>,
    print_ids: &HashMap<&str, FuncId>,
    alloc_id: FuncId,
    string_ids: &HashMap<&str, FuncId>,
    array_ids: &HashMap<&str, FuncId>,
    class_name: Option<&str>,
    vtable_ids: &HashMap<(String, String), DataId>,
    trait_wrap_id: FuncId,
) -> Result<(), CompileError> {
    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);
    builder.seal_block(entry_block);

    let mut variables = HashMap::new();
    let mut var_types = HashMap::new();
    let mut next_var = 0u32;

    // Declare parameters as variables — trait params are now a single I64 handle
    let mut cranelift_param_idx = 0usize;
    for param in func.params.iter() {
        let pty = if param.name.node == "self" {
            if let Some(cn) = class_name {
                PlutoType::Class(cn.to_string())
            } else {
                PlutoType::Void
            }
        } else {
            resolve_param_type(param, env)
        };

        let ty = pluto_to_cranelift(&pty);
        let var = Variable::from_u32(next_var);
        next_var += 1;
        builder.declare_var(var, ty);
        let val = builder.block_params(entry_block)[cranelift_param_idx];
        builder.def_var(var, val);
        variables.insert(param.name.node.clone(), var);
        var_types.insert(param.name.node.clone(), pty);
        cranelift_param_idx += 1;
    }

    // Compute expected return type for class→trait wrapping in return statements
    let expected_return_type = if func.name.node == "main" {
        Some(PlutoType::Int)
    } else {
        let lookup_name = if let Some(cn) = class_name {
            format!("{}_{}", cn, func.name.node)
        } else {
            func.name.node.clone()
        };
        env.functions.get(&lookup_name).map(|s| s.return_type.clone())
    };

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
            print_ids,
            alloc_id,
            string_ids,
            array_ids,
            vtable_ids,
            trait_wrap_id,
            &expected_return_type,
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
        let lookup_name = if let Some(cn) = class_name {
            format!("{}_{}", cn, func.name.node)
        } else {
            func.name.node.clone()
        };
        let ret_type = env.functions.get(&lookup_name).map(|s| &s.return_type);
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
    func_ids: &HashMap<String, FuncId>,
    terminated: &mut bool,
    print_ids: &HashMap<&str, FuncId>,
    alloc_id: FuncId,
    string_ids: &HashMap<&str, FuncId>,
    array_ids: &HashMap<&str, FuncId>,
    vtable_ids: &HashMap<(String, String), DataId>,
    trait_wrap_id: FuncId,
    expected_return_type: &Option<PlutoType>,
) -> Result<(), CompileError> {
    if *terminated {
        return Ok(());
    }
    match stmt {
        Stmt::Let { name, ty, value } => {
            let val = lower_expr(&value.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
            let val_type = infer_type_for_expr(&value.node, env, var_types);

            // Resolve declared type if present
            let declared_type = ty.as_ref().map(|t| resolve_type_expr_to_pluto(&t.node, env));

            // If assigning a class to a trait-typed variable, wrap it
            let (final_val, store_type) = match (&val_type, &declared_type) {
                (PlutoType::Class(cn), Some(PlutoType::Trait(tn))) => {
                    let wrapped = wrap_class_as_trait(val, cn, tn, builder, module, vtable_ids, trait_wrap_id)?;
                    (wrapped, PlutoType::Trait(tn.clone()))
                }
                (_, Some(dt)) => (val, dt.clone()),
                _ => (val, val_type),
            };

            let cl_type = pluto_to_cranelift(&store_type);
            let var = Variable::from_u32(*next_var);
            *next_var += 1;
            builder.declare_var(var, cl_type);
            builder.def_var(var, final_val);
            variables.insert(name.node.clone(), var);
            var_types.insert(name.node.clone(), store_type);
        }
        Stmt::Return(value) => {
            match value {
                Some(expr) => {
                    let val = lower_expr(&expr.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
                    let val_type = infer_type_for_expr(&expr.node, env, var_types);

                    // If returning a class where a trait is expected, wrap it
                    let final_val = match (&val_type, expected_return_type) {
                        (PlutoType::Class(cn), Some(PlutoType::Trait(tn))) => {
                            wrap_class_as_trait(val, cn, tn, builder, module, vtable_ids, trait_wrap_id)?
                        }
                        _ => val,
                    };
                    builder.ins().return_(&[final_val]);
                }
                None => {
                    builder.ins().return_(&[]);
                }
            }
            *terminated = true;
        }
        Stmt::Assign { target, value } => {
            let val = lower_expr(&value.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
            let val_type = infer_type_for_expr(&value.node, env, var_types);
            let target_type = var_types.get(&target.node);

            // If assigning a class to a trait-typed variable, wrap it
            let final_val = match (&val_type, target_type) {
                (PlutoType::Class(cn), Some(PlutoType::Trait(tn))) => {
                    wrap_class_as_trait(val, cn, &tn.clone(), builder, module, vtable_ids, trait_wrap_id)?
                }
                _ => val,
            };

            let var = variables.get(&target.node).ok_or_else(|| {
                CompileError::codegen(format!("undefined variable '{}'", target.node))
            })?;
            builder.def_var(*var, final_val);
        }
        Stmt::FieldAssign { object, field, value } => {
            let ptr = lower_expr(&object.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
            let val = lower_expr(&value.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
            let obj_type = infer_type_for_expr(&object.node, env, var_types);
            if let PlutoType::Class(class_name) = &obj_type {
                if let Some(class_info) = env.classes.get(class_name) {
                    let offset = class_info.fields.iter()
                        .position(|(n, _)| *n == field.node)
                        .ok_or_else(|| CompileError::codegen(format!("unknown field '{}' on class '{class_name}'", field.node)))? as i32 * 8;
                    builder.ins().store(MemFlags::new(), val, ptr, Offset32::new(offset));
                }
            }
        }
        Stmt::IndexAssign { object, index, value } => {
            let handle = lower_expr(&object.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
            let idx = lower_expr(&index.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
            let val = lower_expr(&value.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
            let obj_type = infer_type_for_expr(&object.node, env, var_types);
            if let PlutoType::Array(elem) = &obj_type {
                let slot = to_array_slot(val, elem, builder);
                let func_ref = module.declare_func_in_func(array_ids["set"], builder.func);
                builder.ins().call(func_ref, &[handle, idx, slot]);
            }
        }
        Stmt::If { condition, then_block, else_block } => {
            let cond_val = lower_expr(&condition.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;

            let then_bb = builder.create_block();
            let merge_bb = builder.create_block();

            if let Some(else_blk) = else_block {
                let else_bb = builder.create_block();
                builder.ins().brif(cond_val, then_bb, &[], else_bb, &[]);

                builder.switch_to_block(then_bb);
                builder.seal_block(then_bb);
                let mut then_terminated = false;
                for s in &then_block.node.stmts {
                    lower_stmt(&s.node, builder, env, module, variables, var_types, next_var, func_ids, &mut then_terminated, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id, expected_return_type)?;
                }
                if !then_terminated {
                    builder.ins().jump(merge_bb, &[]);
                }

                builder.switch_to_block(else_bb);
                builder.seal_block(else_bb);
                let mut else_terminated = false;
                for s in &else_blk.node.stmts {
                    lower_stmt(&s.node, builder, env, module, variables, var_types, next_var, func_ids, &mut else_terminated, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id, expected_return_type)?;
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
                    lower_stmt(&s.node, builder, env, module, variables, var_types, next_var, func_ids, &mut then_terminated, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id, expected_return_type)?;
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
            let cond_val = lower_expr(&condition.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
            builder.ins().brif(cond_val, body_bb, &[], exit_bb, &[]);

            builder.switch_to_block(body_bb);
            builder.seal_block(body_bb);
            let mut body_terminated = false;
            for s in &body.node.stmts {
                lower_stmt(&s.node, builder, env, module, variables, var_types, next_var, func_ids, &mut body_terminated, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id, expected_return_type)?;
            }
            if !body_terminated {
                builder.ins().jump(header_bb, &[]);
            }

            builder.seal_block(header_bb);
            builder.switch_to_block(exit_bb);
            builder.seal_block(exit_bb);
        }
        Stmt::For { var, iterable, body } => {
            // Lower iterable to get array handle
            let handle = lower_expr(&iterable.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;

            // Get element type from iterable
            let iter_type = infer_type_for_expr(&iterable.node, env, var_types);
            let elem_type = match &iter_type {
                PlutoType::Array(elem) => *elem.clone(),
                _ => return Err(CompileError::codegen("for loop requires array".to_string())),
            };

            // Call len() on the array
            let len_ref = module.declare_func_in_func(array_ids["len"], builder.func);
            let len_call = builder.ins().call(len_ref, &[handle]);
            let len_val = builder.inst_results(len_call)[0];

            // Create counter variable, init to 0
            let counter_var = Variable::from_u32(*next_var);
            *next_var += 1;
            builder.declare_var(counter_var, types::I64);
            let zero = builder.ins().iconst(types::I64, 0);
            builder.def_var(counter_var, zero);

            // Create blocks
            let header_bb = builder.create_block();
            let body_bb = builder.create_block();
            let exit_bb = builder.create_block();

            builder.ins().jump(header_bb, &[]);

            // Header: check counter < len
            builder.switch_to_block(header_bb);
            let counter = builder.use_var(counter_var);
            let cond = builder.ins().icmp(IntCC::SignedLessThan, counter, len_val);
            builder.ins().brif(cond, body_bb, &[], exit_bb, &[]);

            // Body
            builder.switch_to_block(body_bb);
            builder.seal_block(body_bb);

            // Get element: array_get(handle, counter)
            let counter_for_get = builder.use_var(counter_var);
            let get_ref = module.declare_func_in_func(array_ids["get"], builder.func);
            let get_call = builder.ins().call(get_ref, &[handle, counter_for_get]);
            let raw_slot = builder.inst_results(get_call)[0];
            let elem_val = from_array_slot(raw_slot, &elem_type, builder);

            // Create loop variable, saving any prior binding for restoration
            let prev_var = variables.get(&var.node).cloned();
            let prev_type = var_types.get(&var.node).cloned();

            let loop_var = Variable::from_u32(*next_var);
            *next_var += 1;
            let cl_elem_type = pluto_to_cranelift(&elem_type);
            builder.declare_var(loop_var, cl_elem_type);
            builder.def_var(loop_var, elem_val);
            variables.insert(var.node.clone(), loop_var);
            var_types.insert(var.node.clone(), elem_type);

            // Lower body statements
            let mut body_terminated = false;
            for s in &body.node.stmts {
                lower_stmt(&s.node, builder, env, module, variables, var_types, next_var, func_ids, &mut body_terminated, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id, expected_return_type)?;
            }

            // Restore prior variable binding if shadowed
            if let Some(pv) = prev_var {
                variables.insert(var.node.clone(), pv);
            } else {
                variables.remove(&var.node);
            }
            if let Some(pt) = prev_type {
                var_types.insert(var.node.clone(), pt);
            } else {
                var_types.remove(&var.node);
            }

            // Increment counter
            if !body_terminated {
                let counter_inc = builder.use_var(counter_var);
                let one = builder.ins().iconst(types::I64, 1);
                let new_counter = builder.ins().iadd(counter_inc, one);
                builder.def_var(counter_var, new_counter);
                builder.ins().jump(header_bb, &[]);
            }

            builder.seal_block(header_bb);
            builder.switch_to_block(exit_bb);
            builder.seal_block(exit_bb);
        }
        Stmt::Expr(expr) => {
            lower_expr(&expr.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
        }
    }
    Ok(())
}

/// Create a null-terminated string in the data section and return its pointer as a Value.
fn create_data_str(
    s: &str,
    builder: &mut FunctionBuilder<'_>,
    module: &mut dyn Module,
) -> Result<Value, CompileError> {
    let mut data_desc = DataDescription::new();
    let mut bytes = s.as_bytes().to_vec();
    bytes.push(0); // null terminator
    data_desc.define(bytes.into_boxed_slice());

    let data_id = module
        .declare_anonymous_data(false, false)
        .map_err(|e| CompileError::codegen(format!("declare data error: {e}")))?;
    module
        .define_data(data_id, &data_desc)
        .map_err(|e| CompileError::codegen(format!("define data error: {e}")))?;

    let gv = module.declare_data_in_func(data_id, builder.func);
    Ok(builder.ins().global_value(types::I64, gv))
}

#[allow(clippy::too_many_arguments)]
fn lower_expr(
    expr: &Expr,
    builder: &mut FunctionBuilder<'_>,
    env: &TypeEnv,
    module: &mut dyn Module,
    variables: &HashMap<String, Variable>,
    var_types: &HashMap<String, PlutoType>,
    func_ids: &HashMap<String, FuncId>,
    print_ids: &HashMap<&str, FuncId>,
    alloc_id: FuncId,
    string_ids: &HashMap<&str, FuncId>,
    array_ids: &HashMap<&str, FuncId>,
    vtable_ids: &HashMap<(String, String), DataId>,
    trait_wrap_id: FuncId,
) -> Result<Value, CompileError> {
    match expr {
        Expr::IntLit(n) => Ok(builder.ins().iconst(types::I64, *n)),
        Expr::FloatLit(n) => Ok(builder.ins().f64const(*n)),
        Expr::BoolLit(b) => Ok(builder.ins().iconst(types::I8, if *b { 1 } else { 0 })),
        Expr::StringLit(s) => {
            let raw_ptr = create_data_str(s, builder, module)?;
            let len_val = builder.ins().iconst(types::I64, s.len() as i64);
            let func_ref = module.declare_func_in_func(string_ids["new"], builder.func);
            let call = builder.ins().call(func_ref, &[raw_ptr, len_val]);
            Ok(builder.inst_results(call)[0])
        }
        Expr::Ident(name) => {
            let var = variables.get(name).ok_or_else(|| {
                CompileError::codegen(format!("undefined variable '{name}'"))
            })?;
            Ok(builder.use_var(*var))
        }
        Expr::BinOp { op, lhs, rhs } => {
            let l = lower_expr(&lhs.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
            let r = lower_expr(&rhs.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;

            let lhs_type = infer_type_for_expr(&lhs.node, env, var_types);
            let is_float = lhs_type == PlutoType::Float;
            let is_string = lhs_type == PlutoType::String;

            let result = match op {
                BinOp::Add if is_string => {
                    let func_ref = module.declare_func_in_func(string_ids["concat"], builder.func);
                    let call = builder.ins().call(func_ref, &[l, r]);
                    builder.inst_results(call)[0]
                }
                BinOp::Add if is_float => builder.ins().fadd(l, r),
                BinOp::Add => builder.ins().iadd(l, r),
                BinOp::Sub if is_float => builder.ins().fsub(l, r),
                BinOp::Sub => builder.ins().isub(l, r),
                BinOp::Mul if is_float => builder.ins().fmul(l, r),
                BinOp::Mul => builder.ins().imul(l, r),
                BinOp::Div if is_float => builder.ins().fdiv(l, r),
                BinOp::Div => builder.ins().sdiv(l, r),
                BinOp::Mod => builder.ins().srem(l, r),
                BinOp::Eq if is_string => {
                    let func_ref = module.declare_func_in_func(string_ids["eq"], builder.func);
                    let call = builder.ins().call(func_ref, &[l, r]);
                    let i32_result = builder.inst_results(call)[0];
                    builder.ins().ireduce(types::I8, i32_result)
                }
                BinOp::Eq if is_float => builder.ins().fcmp(FloatCC::Equal, l, r),
                BinOp::Eq => builder.ins().icmp(IntCC::Equal, l, r),
                BinOp::Neq if is_string => {
                    let func_ref = module.declare_func_in_func(string_ids["eq"], builder.func);
                    let call = builder.ins().call(func_ref, &[l, r]);
                    let i32_result = builder.inst_results(call)[0];
                    let i8_result = builder.ins().ireduce(types::I8, i32_result);
                    let one = builder.ins().iconst(types::I8, 1);
                    builder.ins().bxor(i8_result, one)
                }
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
            let val = lower_expr(&operand.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
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
            if name.node == "print" {
                return lower_print(builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id, args);
            }

            let func_id = func_ids.get(&name.node).ok_or_else(|| {
                CompileError::codegen(format!("undefined function '{}'", name.node))
            })?;

            let func_ref = module.declare_func_in_func(*func_id, builder.func);

            // Look up the function signature to check for trait params
            let sig = env.functions.get(&name.node);
            let mut arg_values = Vec::new();
            for (i, arg) in args.iter().enumerate() {
                let val = lower_expr(&arg.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
                let arg_actual_type = infer_type_for_expr(&arg.node, env, var_types);
                let param_expected = sig.and_then(|s| s.params.get(i));

                if let (PlutoType::Class(cn), Some(PlutoType::Trait(tn))) = (&arg_actual_type, param_expected) {
                    // Wrap class as trait handle (single pointer)
                    let wrapped = wrap_class_as_trait(val, cn, tn, builder, module, vtable_ids, trait_wrap_id)?;
                    arg_values.push(wrapped);
                } else {
                    arg_values.push(val);
                }
            }

            let call = builder.ins().call(func_ref, &arg_values);
            let results = builder.inst_results(call);
            if results.is_empty() {
                Ok(builder.ins().iconst(types::I64, 0))
            } else {
                Ok(results[0])
            }
        }
        Expr::StructLit { name, fields } => {
            let class_info = env.classes.get(&name.node).ok_or_else(|| {
                CompileError::codegen(format!("unknown class '{}'", name.node))
            })?;
            let num_fields = class_info.fields.len() as i64;
            let size = num_fields * 8;

            let alloc_ref = module.declare_func_in_func(alloc_id, builder.func);
            let size_val = builder.ins().iconst(types::I64, size);
            let call = builder.ins().call(alloc_ref, &[size_val]);
            let ptr = builder.inst_results(call)[0];

            for (lit_name, lit_val) in fields {
                let val = lower_expr(&lit_val.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
                let offset = class_info.fields.iter()
                    .position(|(n, _)| *n == lit_name.node)
                    .ok_or_else(|| CompileError::codegen(format!("unknown field '{}' on class '{}'", lit_name.node, name.node)))? as i32 * 8;
                builder.ins().store(MemFlags::new(), val, ptr, Offset32::new(offset));
            }

            Ok(ptr)
        }
        Expr::ArrayLit { elements } => {
            let n = elements.len() as i64;
            let func_ref_new = module.declare_func_in_func(array_ids["new"], builder.func);
            let cap_val = builder.ins().iconst(types::I64, n);
            let call = builder.ins().call(func_ref_new, &[cap_val]);
            let handle = builder.inst_results(call)[0];

            let elem_type = infer_type_for_expr(&elements[0].node, env, var_types);
            let func_ref_push = module.declare_func_in_func(array_ids["push"], builder.func);
            for elem in elements {
                let val = lower_expr(&elem.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
                let slot = to_array_slot(val, &elem_type, builder);
                builder.ins().call(func_ref_push, &[handle, slot]);
            }

            Ok(handle)
        }
        Expr::Index { object, index } => {
            let handle = lower_expr(&object.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
            let idx = lower_expr(&index.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
            let obj_type = infer_type_for_expr(&object.node, env, var_types);
            if let PlutoType::Array(elem) = &obj_type {
                let func_ref = module.declare_func_in_func(array_ids["get"], builder.func);
                let call = builder.ins().call(func_ref, &[handle, idx]);
                let raw = builder.inst_results(call)[0];
                Ok(from_array_slot(raw, elem, builder))
            } else {
                Err(CompileError::codegen(format!("index on non-array type {obj_type}")))
            }
        }
        Expr::FieldAccess { object, field } => {
            let ptr = lower_expr(&object.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
            let obj_type = infer_type_for_expr(&object.node, env, var_types);
            if let PlutoType::Class(class_name) = &obj_type {
                let class_info = env.classes.get(class_name).ok_or_else(|| {
                    CompileError::codegen(format!("unknown class '{class_name}'"))
                })?;
                let (field_idx, (_, field_type)) = class_info.fields.iter()
                    .enumerate()
                    .find(|(_, (n, _))| *n == field.node)
                    .ok_or_else(|| {
                        CompileError::codegen(format!("unknown field '{}'", field.node))
                    })?;
                let offset = (field_idx as i32) * 8;
                let cl_type = pluto_to_cranelift(field_type);
                Ok(builder.ins().load(cl_type, MemFlags::new(), ptr, Offset32::new(offset)))
            } else {
                Err(CompileError::codegen(format!("field access on non-class type {obj_type}")))
            }
        }
        Expr::MethodCall { object, method, args } => {
            let obj_ptr = lower_expr(&object.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
            let obj_type = infer_type_for_expr(&object.node, env, var_types);

            // Array methods
            if let PlutoType::Array(elem) = &obj_type {
                match method.node.as_str() {
                    "len" => {
                        let func_ref = module.declare_func_in_func(array_ids["len"], builder.func);
                        let call = builder.ins().call(func_ref, &[obj_ptr]);
                        return Ok(builder.inst_results(call)[0]);
                    }
                    "push" => {
                        let arg_val = lower_expr(&args[0].node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
                        let slot = to_array_slot(arg_val, elem, builder);
                        let func_ref = module.declare_func_in_func(array_ids["push"], builder.func);
                        builder.ins().call(func_ref, &[obj_ptr, slot]);
                        return Ok(builder.ins().iconst(types::I64, 0));
                    }
                    _ => {
                        return Err(CompileError::codegen(format!("array has no method '{}'", method.node)));
                    }
                }
            }

            // String methods
            if obj_type == PlutoType::String {
                if method.node == "len" {
                    let func_ref = module.declare_func_in_func(string_ids["len"], builder.func);
                    let call = builder.ins().call(func_ref, &[obj_ptr]);
                    return Ok(builder.inst_results(call)[0]);
                }
                return Err(CompileError::codegen(format!("string has no method '{}'", method.node)));
            }

            // Trait dynamic dispatch via handle
            if let PlutoType::Trait(trait_name) = &obj_type {
                let trait_info = env.traits.get(trait_name).ok_or_else(|| {
                    CompileError::codegen(format!("unknown trait '{trait_name}'"))
                })?;
                let method_idx = trait_info.methods.iter()
                    .position(|(n, _)| *n == method.node)
                    .ok_or_else(|| {
                        CompileError::codegen(format!("trait '{trait_name}' has no method '{}'", method.node))
                    })?;

                // obj_ptr is a trait handle: pointer to [data_ptr, vtable_ptr]
                let data_ptr = builder.ins().load(types::I64, MemFlags::new(), obj_ptr, Offset32::new(0));
                let vtable_ptr = builder.ins().load(types::I64, MemFlags::new(), obj_ptr, Offset32::new(8));

                // Load fn_ptr from vtable at offset method_idx * 8
                let offset = (method_idx as i32) * 8;
                let fn_ptr = builder.ins().load(types::I64, MemFlags::new(), vtable_ptr, Offset32::new(offset));

                // Build indirect call signature
                let method_sig = &trait_info.methods[method_idx].1;
                let mut sig = module.make_signature();
                sig.params.push(AbiParam::new(types::I64)); // self (data_ptr)
                for param_ty in &method_sig.params[1..] {
                    let cl_ty = pluto_to_cranelift(param_ty);
                    sig.params.push(AbiParam::new(cl_ty));
                }
                if method_sig.return_type != PlutoType::Void {
                    sig.returns.push(AbiParam::new(pluto_to_cranelift(&method_sig.return_type)));
                }
                let sig_ref = builder.func.import_signature(sig);

                let mut call_args = vec![data_ptr]; // data_ptr as self
                for (i, arg) in args.iter().enumerate() {
                    let val = lower_expr(&arg.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;
                    let arg_type = infer_type_for_expr(&arg.node, env, var_types);
                    let param_expected = method_sig.params.get(i + 1); // +1 to skip self
                    if let (PlutoType::Class(cn), Some(PlutoType::Trait(tn))) = (&arg_type, param_expected) {
                        let wrapped = wrap_class_as_trait(val, cn, tn, builder, module, vtable_ids, trait_wrap_id)?;
                        call_args.push(wrapped);
                    } else {
                        call_args.push(val);
                    }
                }

                let call = builder.ins().call_indirect(sig_ref, fn_ptr, &call_args);
                let results = builder.inst_results(call);
                if results.is_empty() {
                    Ok(builder.ins().iconst(types::I64, 0))
                } else {
                    Ok(results[0])
                }
            } else if let PlutoType::Class(class_name) = &obj_type {
                let mangled = format!("{}_{}", class_name, method.node);
                let func_id = func_ids.get(&mangled).ok_or_else(|| {
                    CompileError::codegen(format!("undefined method '{}'", method.node))
                })?;
                let func_ref = module.declare_func_in_func(*func_id, builder.func);

                let mut arg_values = vec![obj_ptr];
                for arg in args {
                    arg_values.push(lower_expr(&arg.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?);
                }

                let call = builder.ins().call(func_ref, &arg_values);
                let results = builder.inst_results(call);
                if results.is_empty() {
                    Ok(builder.ins().iconst(types::I64, 0))
                } else {
                    Ok(results[0])
                }
            } else {
                Err(CompileError::codegen(format!("method call on non-class type {obj_type}")))
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_print(
    builder: &mut FunctionBuilder<'_>,
    env: &TypeEnv,
    module: &mut dyn Module,
    variables: &HashMap<String, Variable>,
    var_types: &HashMap<String, PlutoType>,
    func_ids: &HashMap<String, FuncId>,
    print_ids: &HashMap<&str, FuncId>,
    alloc_id: FuncId,
    string_ids: &HashMap<&str, FuncId>,
    array_ids: &HashMap<&str, FuncId>,
    vtable_ids: &HashMap<(String, String), DataId>,
    trait_wrap_id: FuncId,
    args: &[crate::span::Spanned<Expr>],
) -> Result<Value, CompileError> {
    let arg = &args[0];
    let arg_type = infer_type_for_expr(&arg.node, env, var_types);
    let arg_val = lower_expr(&arg.node, builder, env, module, variables, var_types, func_ids, print_ids, alloc_id, string_ids, array_ids, vtable_ids, trait_wrap_id)?;

    match arg_type {
        PlutoType::Int => {
            let func_id = print_ids["int"];
            let func_ref = module.declare_func_in_func(func_id, builder.func);
            builder.ins().call(func_ref, &[arg_val]);
        }
        PlutoType::Float => {
            let func_id = print_ids["float"];
            let func_ref = module.declare_func_in_func(func_id, builder.func);
            builder.ins().call(func_ref, &[arg_val]);
        }
        PlutoType::String => {
            let func_id = print_ids["string"];
            let func_ref = module.declare_func_in_func(func_id, builder.func);
            builder.ins().call(func_ref, &[arg_val]);
        }
        PlutoType::Bool => {
            let func_id = print_ids["bool"];
            let func_ref = module.declare_func_in_func(func_id, builder.func);
            // Widen I8 bool to I32 for the C function
            let widened = builder.ins().uextend(types::I32, arg_val);
            builder.ins().call(func_ref, &[widened]);
        }
        PlutoType::Void | PlutoType::Class(_) | PlutoType::Array(_) | PlutoType::Trait(_) => {
            return Err(CompileError::codegen(format!("cannot print {arg_type}")));
        }
    }

    // print returns void, so return a dummy value
    Ok(builder.ins().iconst(types::I64, 0))
}

fn resolve_param_type(param: &Param, env: &TypeEnv) -> PlutoType {
    resolve_type_expr_to_pluto(&param.ty.node, env)
}

fn resolve_type_expr_to_pluto(ty: &TypeExpr, env: &TypeEnv) -> PlutoType {
    match ty {
        TypeExpr::Named(name) => match name.as_str() {
            "int" => PlutoType::Int,
            "float" => PlutoType::Float,
            "bool" => PlutoType::Bool,
            "string" => PlutoType::String,
            _ => {
                if env.classes.contains_key(name) {
                    PlutoType::Class(name.clone())
                } else if env.traits.contains_key(name) {
                    PlutoType::Trait(name.clone())
                } else {
                    PlutoType::Void
                }
            }
        },
        TypeExpr::Array(inner) => {
            let elem = resolve_type_expr_to_pluto(&inner.node, env);
            PlutoType::Array(Box::new(elem))
        }
        TypeExpr::Qualified { module, name } => {
            let prefixed = format!("{}.{}", module, name);
            if env.classes.contains_key(&prefixed) {
                PlutoType::Class(prefixed)
            } else if env.traits.contains_key(&prefixed) {
                PlutoType::Trait(prefixed)
            } else {
                PlutoType::Void
            }
        }
    }
}

/// Convert a Pluto value to an i64 slot for array storage.
fn to_array_slot(val: Value, ty: &PlutoType, builder: &mut FunctionBuilder<'_>) -> Value {
    match ty {
        PlutoType::Float => builder.ins().bitcast(types::I64, MemFlags::new(), val),
        PlutoType::Bool => builder.ins().uextend(types::I64, val),
        _ => val, // int, string, class, array are already I64
    }
}

/// Convert an i64 slot from array storage back to the Pluto type's representation.
fn from_array_slot(val: Value, ty: &PlutoType, builder: &mut FunctionBuilder<'_>) -> Value {
    match ty {
        PlutoType::Float => builder.ins().bitcast(types::F64, MemFlags::new(), val),
        PlutoType::Bool => builder.ins().ireduce(types::I8, val),
        _ => val,
    }
}

pub fn pluto_to_cranelift(ty: &PlutoType) -> types::Type {
    match ty {
        PlutoType::Int => types::I64,
        PlutoType::Float => types::F64,
        PlutoType::Bool => types::I8,
        PlutoType::String => types::I64,       // pointer
        PlutoType::Void => types::I64,         // shouldn't be used for values
        PlutoType::Class(_) => types::I64,     // pointer
        PlutoType::Array(_) => types::I64,     // pointer to handle
        PlutoType::Trait(_) => types::I64,     // pointer to trait handle
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
        Expr::StructLit { name, .. } => PlutoType::Class(name.node.clone()),
        Expr::FieldAccess { object, field } => {
            let obj_type = infer_type_for_expr(&object.node, env, var_types);
            if let PlutoType::Class(class_name) = &obj_type {
                if let Some(class_info) = env.classes.get(class_name) {
                    class_info.fields.iter()
                        .find(|(n, _)| *n == field.node)
                        .map(|(_, t)| t.clone())
                        .unwrap_or(PlutoType::Void)
                } else {
                    PlutoType::Void
                }
            } else {
                PlutoType::Void
            }
        }
        Expr::ArrayLit { elements } => {
            let first = infer_type_for_expr(&elements[0].node, env, var_types);
            PlutoType::Array(Box::new(first))
        }
        Expr::Index { object, .. } => {
            let obj_type = infer_type_for_expr(&object.node, env, var_types);
            if let PlutoType::Array(elem) = obj_type {
                *elem
            } else {
                PlutoType::Void
            }
        }
        Expr::MethodCall { object, method, .. } => {
            let obj_type = infer_type_for_expr(&object.node, env, var_types);
            if matches!(&obj_type, PlutoType::Array(_)) {
                return match method.node.as_str() {
                    "len" => PlutoType::Int,
                    _ => PlutoType::Void,
                };
            }
            if obj_type == PlutoType::String && method.node == "len" {
                return PlutoType::Int;
            }
            if let PlutoType::Trait(trait_name) = &obj_type {
                if let Some(trait_info) = env.traits.get(trait_name) {
                    return trait_info.methods.iter()
                        .find(|(n, _)| *n == method.node)
                        .map(|(_, sig)| sig.return_type.clone())
                        .unwrap_or(PlutoType::Void);
                }
                return PlutoType::Void;
            }
            if let PlutoType::Class(class_name) = &obj_type {
                let mangled = format!("{}_{}", class_name, method.node);
                env.functions.get(&mangled).map(|s| s.return_type.clone()).unwrap_or(PlutoType::Void)
            } else {
                PlutoType::Void
            }
        }
    }
}
