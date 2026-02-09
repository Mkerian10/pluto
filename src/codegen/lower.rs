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

use super::runtime::RuntimeRegistry;

/// Size of a pointer in bytes. All heap-allocated objects use pointer-sized slots.
pub const POINTER_SIZE: i32 = 8;

struct LowerContext<'a> {
    builder: FunctionBuilder<'a>,
    module: &'a mut dyn Module,
    env: &'a TypeEnv,
    func_ids: &'a HashMap<String, FuncId>,
    runtime: &'a RuntimeRegistry,
    vtable_ids: &'a HashMap<(String, String), DataId>,
    // Per-function mutable state
    variables: HashMap<String, Variable>,
    var_types: HashMap<String, PlutoType>,
    next_var: u32,
    expected_return_type: Option<PlutoType>,
}

impl<'a> LowerContext<'a> {
    fn finalize(self) {
        self.builder.finalize();
    }

    /// Wrap a class pointer as a trait handle by calling __pluto_trait_wrap.
    fn wrap_class_as_trait(
        &mut self,
        class_val: Value,
        class_name: &str,
        trait_name: &str,
    ) -> Result<Value, CompileError> {
        let vtable_data_id = self.vtable_ids
            .get(&(class_name.to_string(), trait_name.to_string()))
            .ok_or_else(|| {
                CompileError::codegen(format!("no vtable for ({class_name}, {trait_name})"))
            })?;
        let gv = self.module.declare_data_in_func(*vtable_data_id, self.builder.func);
        let vtable_ptr = self.builder.ins().global_value(types::I64, gv);
        let func_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_trait_wrap"), self.builder.func);
        let call = self.builder.ins().call(func_ref, &[class_val, vtable_ptr]);
        Ok(self.builder.inst_results(call)[0])
    }

    /// Emit a return with the default value for the current function's return type.
    /// Used by raise and propagation to exit the function when an error occurs.
    fn emit_default_return(&mut self) {
        match &self.expected_return_type {
            Some(PlutoType::Void) | None => {
                self.builder.ins().return_(&[]);
            }
            Some(PlutoType::Float) => {
                let val = self.builder.ins().f64const(0.0);
                self.builder.ins().return_(&[val]);
            }
            Some(PlutoType::Bool) => {
                let val = self.builder.ins().iconst(types::I8, 0);
                self.builder.ins().return_(&[val]);
            }
            Some(_) => {
                // Int, String, Class, Array, Enum, Error — all I64
                let val = self.builder.ins().iconst(types::I64, 0);
                self.builder.ins().return_(&[val]);
            }
        }
    }

    /// Create a null-terminated string in the data section and return its pointer as a Value.
    fn create_data_str(&mut self, s: &str) -> Result<Value, CompileError> {
        let mut data_desc = DataDescription::new();
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0); // null terminator
        data_desc.define(bytes.into_boxed_slice());

        let data_id = self.module
            .declare_anonymous_data(false, false)
            .map_err(|e| CompileError::codegen(format!("declare data error: {e}")))?;
        self.module
            .define_data(data_id, &data_desc)
            .map_err(|e| CompileError::codegen(format!("define data error: {e}")))?;

        let gv = self.module.declare_data_in_func(data_id, self.builder.func);
        Ok(self.builder.ins().global_value(types::I64, gv))
    }

    fn lower_stmt(
        &mut self,
        stmt: &Stmt,
        terminated: &mut bool,
    ) -> Result<(), CompileError> {
        if *terminated {
            return Ok(());
        }
        match stmt {
            Stmt::Let { name, ty, value } => {
                let val = self.lower_expr(&value.node)?;
                let val_type = infer_type_for_expr(&value.node, self.env, &self.var_types);

                // Resolve declared type if present
                let declared_type = ty.as_ref().map(|t| resolve_type_expr_to_pluto(&t.node, self.env));

                // If assigning a class to a trait-typed variable, wrap it
                let (final_val, store_type) = match (&val_type, &declared_type) {
                    (PlutoType::Class(cn), Some(PlutoType::Trait(tn))) => {
                        let cn = cn.clone();
                        let tn = tn.clone();
                        let wrapped = self.wrap_class_as_trait(val, &cn, &tn)?;
                        (wrapped, PlutoType::Trait(tn))
                    }
                    (_, Some(dt)) => (val, dt.clone()),
                    _ => (val, val_type),
                };

                let cl_type = pluto_to_cranelift(&store_type);
                let var = Variable::from_u32(self.next_var);
                self.next_var += 1;
                self.builder.declare_var(var, cl_type);
                self.builder.def_var(var, final_val);
                self.variables.insert(name.node.clone(), var);
                self.var_types.insert(name.node.clone(), store_type);
            }
            Stmt::Return(value) => {
                match value {
                    Some(expr) => {
                        let val = self.lower_expr(&expr.node)?;
                        let val_type = infer_type_for_expr(&expr.node, self.env, &self.var_types);

                        // If returning a class where a trait is expected, wrap it
                        let expected = self.expected_return_type.clone();
                        let final_val = match (&val_type, &expected) {
                            (PlutoType::Class(cn), Some(PlutoType::Trait(tn))) => {
                                self.wrap_class_as_trait(val, cn, tn)?
                            }
                            _ => val,
                        };
                        self.builder.ins().return_(&[final_val]);
                    }
                    None => {
                        self.builder.ins().return_(&[]);
                    }
                }
                *terminated = true;
            }
            Stmt::Assign { target, value } => {
                let val = self.lower_expr(&value.node)?;
                let val_type = infer_type_for_expr(&value.node, self.env, &self.var_types);
                let target_type = self.var_types.get(&target.node).cloned();

                // If assigning a class to a trait-typed variable, wrap it
                let final_val = match (&val_type, &target_type) {
                    (PlutoType::Class(cn), Some(PlutoType::Trait(tn))) => {
                        self.wrap_class_as_trait(val, cn, tn)?
                    }
                    _ => val,
                };

                let var = self.variables.get(&target.node).ok_or_else(|| {
                    CompileError::codegen(format!("undefined variable '{}'", target.node))
                })?;
                self.builder.def_var(*var, final_val);
            }
            Stmt::FieldAssign { object, field, value } => {
                let ptr = self.lower_expr(&object.node)?;
                let val = self.lower_expr(&value.node)?;
                let obj_type = infer_type_for_expr(&object.node, self.env, &self.var_types);
                if let PlutoType::Class(class_name) = &obj_type {
                    if let Some(class_info) = self.env.classes.get(class_name) {
                        let offset = class_info.fields.iter()
                            .position(|(n, _, _)| *n == field.node)
                            .ok_or_else(|| CompileError::codegen(format!("unknown field '{}' on class '{class_name}'", field.node)))? as i32 * POINTER_SIZE;
                        self.builder.ins().store(MemFlags::new(), val, ptr, Offset32::new(offset));
                    }
                }
            }
            Stmt::IndexAssign { object, index, value } => {
                let handle = self.lower_expr(&object.node)?;
                let idx = self.lower_expr(&index.node)?;
                let val = self.lower_expr(&value.node)?;
                let obj_type = infer_type_for_expr(&object.node, self.env, &self.var_types);
                if let PlutoType::Array(elem) = &obj_type {
                    let slot = to_array_slot(val, elem, &mut self.builder);
                    let func_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_array_set"), self.builder.func);
                    self.builder.ins().call(func_ref, &[handle, idx, slot]);
                }
            }
            Stmt::If { condition, then_block, else_block } => {
                let cond_val = self.lower_expr(&condition.node)?;

                let then_bb = self.builder.create_block();
                let merge_bb = self.builder.create_block();

                if let Some(else_blk) = else_block {
                    let else_bb = self.builder.create_block();
                    self.builder.ins().brif(cond_val, then_bb, &[], else_bb, &[]);

                    self.builder.switch_to_block(then_bb);
                    self.builder.seal_block(then_bb);
                    let mut then_terminated = false;
                    for s in &then_block.node.stmts {
                        self.lower_stmt(&s.node, &mut then_terminated)?;
                    }
                    if !then_terminated {
                        self.builder.ins().jump(merge_bb, &[]);
                    }

                    self.builder.switch_to_block(else_bb);
                    self.builder.seal_block(else_bb);
                    let mut else_terminated = false;
                    for s in &else_blk.node.stmts {
                        self.lower_stmt(&s.node, &mut else_terminated)?;
                    }
                    if !else_terminated {
                        self.builder.ins().jump(merge_bb, &[]);
                    }

                    if then_terminated && else_terminated {
                        *terminated = true;
                    }
                } else {
                    self.builder.ins().brif(cond_val, then_bb, &[], merge_bb, &[]);

                    self.builder.switch_to_block(then_bb);
                    self.builder.seal_block(then_bb);
                    let mut then_terminated = false;
                    for s in &then_block.node.stmts {
                        self.lower_stmt(&s.node, &mut then_terminated)?;
                    }
                    if !then_terminated {
                        self.builder.ins().jump(merge_bb, &[]);
                    }
                }

                if !*terminated {
                    self.builder.switch_to_block(merge_bb);
                    self.builder.seal_block(merge_bb);
                }
            }
            Stmt::While { condition, body } => {
                let header_bb = self.builder.create_block();
                let body_bb = self.builder.create_block();
                let exit_bb = self.builder.create_block();

                self.builder.ins().jump(header_bb, &[]);

                self.builder.switch_to_block(header_bb);
                let cond_val = self.lower_expr(&condition.node)?;
                self.builder.ins().brif(cond_val, body_bb, &[], exit_bb, &[]);

                self.builder.switch_to_block(body_bb);
                self.builder.seal_block(body_bb);
                let mut body_terminated = false;
                for s in &body.node.stmts {
                    self.lower_stmt(&s.node, &mut body_terminated)?;
                }
                if !body_terminated {
                    self.builder.ins().jump(header_bb, &[]);
                }

                self.builder.seal_block(header_bb);
                self.builder.switch_to_block(exit_bb);
                self.builder.seal_block(exit_bb);
            }
            Stmt::For { var, iterable, body } => {
                // Lower iterable to get array handle
                let handle = self.lower_expr(&iterable.node)?;

                // Get element type from iterable
                let iter_type = infer_type_for_expr(&iterable.node, self.env, &self.var_types);
                let elem_type = match &iter_type {
                    PlutoType::Array(elem) => *elem.clone(),
                    _ => return Err(CompileError::codegen("for loop requires array".to_string())),
                };

                // Call len() on the array
                let len_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_array_len"), self.builder.func);
                let len_call = self.builder.ins().call(len_ref, &[handle]);
                let len_val = self.builder.inst_results(len_call)[0];

                // Create counter variable, init to 0
                let counter_var = Variable::from_u32(self.next_var);
                self.next_var += 1;
                self.builder.declare_var(counter_var, types::I64);
                let zero = self.builder.ins().iconst(types::I64, 0);
                self.builder.def_var(counter_var, zero);

                // Create blocks
                let header_bb = self.builder.create_block();
                let body_bb = self.builder.create_block();
                let exit_bb = self.builder.create_block();

                self.builder.ins().jump(header_bb, &[]);

                // Header: check counter < len
                self.builder.switch_to_block(header_bb);
                let counter = self.builder.use_var(counter_var);
                let cond = self.builder.ins().icmp(IntCC::SignedLessThan, counter, len_val);
                self.builder.ins().brif(cond, body_bb, &[], exit_bb, &[]);

                // Body
                self.builder.switch_to_block(body_bb);
                self.builder.seal_block(body_bb);

                // Get element: array_get(handle, counter)
                let counter_for_get = self.builder.use_var(counter_var);
                let get_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_array_get"), self.builder.func);
                let get_call = self.builder.ins().call(get_ref, &[handle, counter_for_get]);
                let raw_slot = self.builder.inst_results(get_call)[0];
                let elem_val = from_array_slot(raw_slot, &elem_type, &mut self.builder);

                // Create loop variable, saving any prior binding for restoration
                let prev_var = self.variables.get(&var.node).cloned();
                let prev_type = self.var_types.get(&var.node).cloned();

                let loop_var = Variable::from_u32(self.next_var);
                self.next_var += 1;
                let cl_elem_type = pluto_to_cranelift(&elem_type);
                self.builder.declare_var(loop_var, cl_elem_type);
                self.builder.def_var(loop_var, elem_val);
                self.variables.insert(var.node.clone(), loop_var);
                self.var_types.insert(var.node.clone(), elem_type);

                // Lower body statements
                let mut body_terminated = false;
                for s in &body.node.stmts {
                    self.lower_stmt(&s.node, &mut body_terminated)?;
                }

                // Restore prior variable binding if shadowed
                if let Some(pv) = prev_var {
                    self.variables.insert(var.node.clone(), pv);
                } else {
                    self.variables.remove(&var.node);
                }
                if let Some(pt) = prev_type {
                    self.var_types.insert(var.node.clone(), pt);
                } else {
                    self.var_types.remove(&var.node);
                }

                // Increment counter
                if !body_terminated {
                    let counter_inc = self.builder.use_var(counter_var);
                    let one = self.builder.ins().iconst(types::I64, 1);
                    let new_counter = self.builder.ins().iadd(counter_inc, one);
                    self.builder.def_var(counter_var, new_counter);
                    self.builder.ins().jump(header_bb, &[]);
                }

                self.builder.seal_block(header_bb);
                self.builder.switch_to_block(exit_bb);
                self.builder.seal_block(exit_bb);
            }
            Stmt::Match { expr, arms } => {
                let ptr = self.lower_expr(&expr.node)?;
                let tag = self.builder.ins().load(types::I64, MemFlags::new(), ptr, Offset32::new(0));

                let enum_name = match infer_type_for_expr(&expr.node, self.env, &self.var_types) {
                    PlutoType::Enum(name) => name,
                    _ => return Err(CompileError::codegen("match on non-enum".to_string())),
                };
                let enum_info = self.env.enums.get(&enum_name).ok_or_else(|| {
                    CompileError::codegen(format!("unknown enum '{enum_name}'"))
                })?.clone();

                let merge_bb = self.builder.create_block();
                let mut check_blocks = Vec::new();
                let mut body_blocks = Vec::new();

                for _ in 0..arms.len() {
                    check_blocks.push(self.builder.create_block());
                    body_blocks.push(self.builder.create_block());
                }

                // Jump to first check block
                self.builder.ins().jump(check_blocks[0], &[]);

                let mut all_terminated = true;

                for (i, arm) in arms.iter().enumerate() {
                    // Check block: compare tag
                    self.builder.switch_to_block(check_blocks[i]);
                    self.builder.seal_block(check_blocks[i]);

                    let variant_idx = enum_info.variants.iter()
                        .position(|(n, _)| *n == arm.variant_name.node)
                        .unwrap() as i64;
                    let expected_tag = self.builder.ins().iconst(types::I64, variant_idx);
                    let cmp = self.builder.ins().icmp(IntCC::Equal, tag, expected_tag);

                    let fallthrough = if i + 1 < arms.len() {
                        check_blocks[i + 1]
                    } else {
                        // Last arm: exhaustiveness guaranteed, so fallthrough to merge
                        merge_bb
                    };
                    self.builder.ins().brif(cmp, body_blocks[i], &[], fallthrough, &[]);

                    // Body block: extract bindings and lower body
                    self.builder.switch_to_block(body_blocks[i]);
                    self.builder.seal_block(body_blocks[i]);

                    let variant_fields = &enum_info.variants.iter()
                        .find(|(n, _)| *n == arm.variant_name.node)
                        .unwrap().1;

                    // Save previous variable bindings so we can restore after this arm
                    let mut prev_vars: Vec<(String, Option<Variable>, Option<PlutoType>)> = Vec::new();

                    for (binding_field, opt_rename) in &arm.bindings {
                        let field_idx = variant_fields.iter()
                            .position(|(n, _)| *n == binding_field.node)
                            .unwrap();
                        let field_type = &variant_fields[field_idx].1;
                        let offset = ((1 + field_idx) as i32) * POINTER_SIZE;
                        let raw = self.builder.ins().load(types::I64, MemFlags::new(), ptr, Offset32::new(offset));
                        let val = from_array_slot(raw, field_type, &mut self.builder);

                        let var_name = opt_rename.as_ref().map_or(&binding_field.node, |r| &r.node);
                        let cl_type = pluto_to_cranelift(field_type);
                        let var = Variable::from_u32(self.next_var);
                        self.next_var += 1;
                        self.builder.declare_var(var, cl_type);
                        self.builder.def_var(var, val);

                        prev_vars.push((
                            var_name.clone(),
                            self.variables.get(var_name).cloned(),
                            self.var_types.get(var_name).cloned(),
                        ));
                        self.variables.insert(var_name.clone(), var);
                        self.var_types.insert(var_name.clone(), field_type.clone());
                    }

                    let mut arm_terminated = false;
                    for s in &arm.body.node.stmts {
                        self.lower_stmt(&s.node, &mut arm_terminated)?;
                    }

                    // Restore previous variable bindings
                    for (name, prev_var, prev_type) in prev_vars {
                        if let Some(pv) = prev_var {
                            self.variables.insert(name.clone(), pv);
                        } else {
                            self.variables.remove(&name);
                        }
                        if let Some(pt) = prev_type {
                            self.var_types.insert(name, pt);
                        } else {
                            self.var_types.remove(&name);
                        }
                    }

                    if !arm_terminated {
                        self.builder.ins().jump(merge_bb, &[]);
                    }
                    if !arm_terminated {
                        all_terminated = false;
                    }
                }

                if all_terminated {
                    *terminated = true;
                }

                // Always switch to and seal the merge block — it's referenced by
                // the last arm's fallthrough even if unreachable.
                self.builder.switch_to_block(merge_bb);
                self.builder.seal_block(merge_bb);
                if *terminated {
                    // All arms returned; merge block is unreachable but needs a terminator.
                    self.builder.ins().trap(cranelift_codegen::ir::TrapCode::user(1).unwrap());
                }
            }
            Stmt::Raise { error_name, fields } => {
                let error_info = self.env.errors.get(&error_name.node).ok_or_else(|| {
                    CompileError::codegen(format!("unknown error '{}'", error_name.node))
                })?.clone();
                let num_fields = error_info.fields.len();
                let size = (num_fields as i64 * POINTER_SIZE as i64).max(POINTER_SIZE as i64);

                // Allocate error object
                let alloc_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_alloc"), self.builder.func);
                let size_val = self.builder.ins().iconst(types::I64, size);
                let call = self.builder.ins().call(alloc_ref, &[size_val]);
                let ptr = self.builder.inst_results(call)[0];

                // Store field values
                let field_info = error_info.fields.clone();
                for (lit_name, lit_val) in fields {
                    let val = self.lower_expr(&lit_val.node)?;
                    let offset = field_info.iter()
                        .position(|(n, _)| *n == lit_name.node)
                        .unwrap_or(0) as i32 * POINTER_SIZE;
                    self.builder.ins().store(MemFlags::new(), val, ptr, Offset32::new(offset));
                }

                // Set TLS error pointer
                let raise_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_raise_error"), self.builder.func);
                self.builder.ins().call(raise_ref, &[ptr]);

                // Return default value (caller checks TLS)
                self.emit_default_return();
                *terminated = true;
            }
            Stmt::Expr(expr) => {
                self.lower_expr(&expr.node)?;
            }
        }
        Ok(())
    }

    fn lower_expr(&mut self, expr: &Expr) -> Result<Value, CompileError> {
        match expr {
            Expr::IntLit(n) => Ok(self.builder.ins().iconst(types::I64, *n)),
            Expr::FloatLit(n) => Ok(self.builder.ins().f64const(*n)),
            Expr::BoolLit(b) => Ok(self.builder.ins().iconst(types::I8, if *b { 1 } else { 0 })),
            Expr::StringLit(s) => {
                let raw_ptr = self.create_data_str(s)?;
                let len_val = self.builder.ins().iconst(types::I64, s.len() as i64);
                let func_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_string_new"), self.builder.func);
                let call = self.builder.ins().call(func_ref, &[raw_ptr, len_val]);
                Ok(self.builder.inst_results(call)[0])
            }
            Expr::StringInterp { parts } => {
                // Convert each part to a string handle, then concat them all
                let mut string_vals: Vec<Value> = Vec::new();
                for part in parts {
                    match part {
                        StringInterpPart::Lit(s) => {
                            let raw_ptr = self.create_data_str(s)?;
                            let len_val = self.builder.ins().iconst(types::I64, s.len() as i64);
                            let func_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_string_new"), self.builder.func);
                            let call = self.builder.ins().call(func_ref, &[raw_ptr, len_val]);
                            string_vals.push(self.builder.inst_results(call)[0]);
                        }
                        StringInterpPart::Expr(e) => {
                            let val = self.lower_expr(&e.node)?;
                            let t = infer_type_for_expr(&e.node, self.env, &self.var_types);
                            let str_val = match t {
                                PlutoType::String => val,
                                PlutoType::Int => {
                                    let func_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_int_to_string"), self.builder.func);
                                    let call = self.builder.ins().call(func_ref, &[val]);
                                    self.builder.inst_results(call)[0]
                                }
                                PlutoType::Float => {
                                    let func_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_float_to_string"), self.builder.func);
                                    let call = self.builder.ins().call(func_ref, &[val]);
                                    self.builder.inst_results(call)[0]
                                }
                                PlutoType::Bool => {
                                    let widened = self.builder.ins().uextend(types::I32, val);
                                    let func_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_bool_to_string"), self.builder.func);
                                    let call = self.builder.ins().call(func_ref, &[widened]);
                                    self.builder.inst_results(call)[0]
                                }
                                _ => return Err(CompileError::codegen(format!("cannot interpolate {t}"))),
                            };
                            string_vals.push(str_val);
                        }
                    }
                }
                // Concat all parts left to right
                let mut result = string_vals[0];
                let concat_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_string_concat"), self.builder.func);
                for part_val in &string_vals[1..] {
                    let call = self.builder.ins().call(concat_ref, &[result, *part_val]);
                    result = self.builder.inst_results(call)[0];
                }
                Ok(result)
            }
            Expr::Ident(name) => {
                let var = self.variables.get(name).ok_or_else(|| {
                    CompileError::codegen(format!("undefined variable '{name}'"))
                })?;
                Ok(self.builder.use_var(*var))
            }
            Expr::BinOp { op, lhs, rhs } => {
                let l = self.lower_expr(&lhs.node)?;
                let r = self.lower_expr(&rhs.node)?;

                let lhs_type = infer_type_for_expr(&lhs.node, self.env, &self.var_types);
                let is_float = lhs_type == PlutoType::Float;
                let is_string = lhs_type == PlutoType::String;

                let result = match op {
                    BinOp::Add if is_string => {
                        let func_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_string_concat"), self.builder.func);
                        let call = self.builder.ins().call(func_ref, &[l, r]);
                        self.builder.inst_results(call)[0]
                    }
                    BinOp::Add if is_float => self.builder.ins().fadd(l, r),
                    BinOp::Add => self.builder.ins().iadd(l, r),
                    BinOp::Sub if is_float => self.builder.ins().fsub(l, r),
                    BinOp::Sub => self.builder.ins().isub(l, r),
                    BinOp::Mul if is_float => self.builder.ins().fmul(l, r),
                    BinOp::Mul => self.builder.ins().imul(l, r),
                    BinOp::Div if is_float => self.builder.ins().fdiv(l, r),
                    BinOp::Div => self.builder.ins().sdiv(l, r),
                    BinOp::Mod => self.builder.ins().srem(l, r),
                    BinOp::Eq if is_string => {
                        let func_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_string_eq"), self.builder.func);
                        let call = self.builder.ins().call(func_ref, &[l, r]);
                        let i32_result = self.builder.inst_results(call)[0];
                        self.builder.ins().ireduce(types::I8, i32_result)
                    }
                    BinOp::Eq if is_float => self.builder.ins().fcmp(FloatCC::Equal, l, r),
                    BinOp::Eq => self.builder.ins().icmp(IntCC::Equal, l, r),
                    BinOp::Neq if is_string => {
                        let func_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_string_eq"), self.builder.func);
                        let call = self.builder.ins().call(func_ref, &[l, r]);
                        let i32_result = self.builder.inst_results(call)[0];
                        let i8_result = self.builder.ins().ireduce(types::I8, i32_result);
                        let one = self.builder.ins().iconst(types::I8, 1);
                        self.builder.ins().bxor(i8_result, one)
                    }
                    BinOp::Neq if is_float => self.builder.ins().fcmp(FloatCC::NotEqual, l, r),
                    BinOp::Neq => self.builder.ins().icmp(IntCC::NotEqual, l, r),
                    BinOp::Lt if is_float => self.builder.ins().fcmp(FloatCC::LessThan, l, r),
                    BinOp::Lt => self.builder.ins().icmp(IntCC::SignedLessThan, l, r),
                    BinOp::Gt if is_float => self.builder.ins().fcmp(FloatCC::GreaterThan, l, r),
                    BinOp::Gt => self.builder.ins().icmp(IntCC::SignedGreaterThan, l, r),
                    BinOp::LtEq if is_float => self.builder.ins().fcmp(FloatCC::LessThanOrEqual, l, r),
                    BinOp::LtEq => self.builder.ins().icmp(IntCC::SignedLessThanOrEqual, l, r),
                    BinOp::GtEq if is_float => self.builder.ins().fcmp(FloatCC::GreaterThanOrEqual, l, r),
                    BinOp::GtEq => self.builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, l, r),
                    BinOp::And => self.builder.ins().band(l, r),
                    BinOp::Or => self.builder.ins().bor(l, r),
                };
                Ok(result)
            }
            Expr::UnaryOp { op, operand } => {
                let val = self.lower_expr(&operand.node)?;
                let operand_type = infer_type_for_expr(&operand.node, self.env, &self.var_types);
                match op {
                    UnaryOp::Neg if operand_type == PlutoType::Float => Ok(self.builder.ins().fneg(val)),
                    UnaryOp::Neg => Ok(self.builder.ins().ineg(val)),
                    UnaryOp::Not => {
                        let one = self.builder.ins().iconst(types::I8, 1);
                        Ok(self.builder.ins().bxor(val, one))
                    }
                }
            }
            Expr::Call { name, args } => {
                if name.node == "print" {
                    return self.lower_print(args);
                }

                // Check if calling a closure variable
                if let Some(PlutoType::Fn(ref param_types, ref ret_type)) = self.var_types.get(&name.node).cloned() {
                    let closure_var = self.variables[&name.node];
                    let closure_ptr = self.builder.use_var(closure_var);

                    // Load fn_ptr from closure object at offset 0
                    let fn_ptr = self.builder.ins().load(types::I64, MemFlags::new(), closure_ptr, Offset32::new(0));

                    // Build indirect call signature: (I64 env, param_types...) -> ret
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64)); // __env
                    for pt in param_types {
                        sig.params.push(AbiParam::new(pluto_to_cranelift(pt)));
                    }
                    if **ret_type != PlutoType::Void {
                        sig.returns.push(AbiParam::new(pluto_to_cranelift(ret_type)));
                    }
                    let sig_ref = self.builder.func.import_signature(sig);

                    let mut call_args = vec![closure_ptr]; // env ptr as first arg
                    for arg in args {
                        call_args.push(self.lower_expr(&arg.node)?);
                    }

                    let call = self.builder.ins().call_indirect(sig_ref, fn_ptr, &call_args);
                    let results = self.builder.inst_results(call);
                    return Ok(if results.is_empty() {
                        self.builder.ins().iconst(types::I64, 0)
                    } else {
                        results[0]
                    });
                }

                let func_id = self.func_ids.get(&name.node).ok_or_else(|| {
                    CompileError::codegen(format!("undefined function '{}'", name.node))
                })?;

                let func_ref = self.module.declare_func_in_func(*func_id, self.builder.func);

                // Look up the function signature to check for trait params
                let param_types: Vec<PlutoType> = self.env.functions.get(&name.node)
                    .map(|s| s.params.clone())
                    .unwrap_or_default();
                let mut arg_values = Vec::new();
                for (i, arg) in args.iter().enumerate() {
                    let val = self.lower_expr(&arg.node)?;
                    let arg_actual_type = infer_type_for_expr(&arg.node, self.env, &self.var_types);
                    let param_expected = param_types.get(i);

                    if let (PlutoType::Class(cn), Some(PlutoType::Trait(tn))) = (&arg_actual_type, param_expected) {
                        // Wrap class as trait handle (single pointer)
                        let wrapped = self.wrap_class_as_trait(val, cn, tn)?;
                        arg_values.push(wrapped);
                    } else {
                        arg_values.push(val);
                    }
                }

                let call = self.builder.ins().call(func_ref, &arg_values);
                let results = self.builder.inst_results(call);
                if results.is_empty() {
                    Ok(self.builder.ins().iconst(types::I64, 0))
                } else {
                    Ok(results[0])
                }
            }
            Expr::StructLit { name, fields, .. } => {
                let class_info = self.env.classes.get(&name.node).ok_or_else(|| {
                    CompileError::codegen(format!("unknown class '{}'", name.node))
                })?;
                let num_fields = class_info.fields.len() as i64;
                let size = num_fields * POINTER_SIZE as i64;

                let alloc_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_alloc"), self.builder.func);
                let size_val = self.builder.ins().iconst(types::I64, size);
                let call = self.builder.ins().call(alloc_ref, &[size_val]);
                let ptr = self.builder.inst_results(call)[0];

                // Clone field info to avoid borrow conflict with self.lower_expr
                let field_info: Vec<(String, PlutoType, bool)> = class_info.fields.clone();

                for (lit_name, lit_val) in fields {
                    let val = self.lower_expr(&lit_val.node)?;
                    let offset = field_info.iter()
                        .position(|(n, _, _)| *n == lit_name.node)
                        .ok_or_else(|| CompileError::codegen(format!("unknown field '{}' on class '{}'", lit_name.node, name.node)))? as i32 * POINTER_SIZE;
                    self.builder.ins().store(MemFlags::new(), val, ptr, Offset32::new(offset));
                }

                Ok(ptr)
            }
            Expr::ArrayLit { elements } => {
                let n = elements.len() as i64;
                let func_ref_new = self.module.declare_func_in_func(self.runtime.get("__pluto_array_new"), self.builder.func);
                let cap_val = self.builder.ins().iconst(types::I64, n);
                let call = self.builder.ins().call(func_ref_new, &[cap_val]);
                let handle = self.builder.inst_results(call)[0];

                let elem_type = infer_type_for_expr(&elements[0].node, self.env, &self.var_types);
                let func_ref_push = self.module.declare_func_in_func(self.runtime.get("__pluto_array_push"), self.builder.func);
                for elem in elements {
                    let val = self.lower_expr(&elem.node)?;
                    let slot = to_array_slot(val, &elem_type, &mut self.builder);
                    self.builder.ins().call(func_ref_push, &[handle, slot]);
                }

                Ok(handle)
            }
            Expr::Index { object, index } => {
                let handle = self.lower_expr(&object.node)?;
                let idx = self.lower_expr(&index.node)?;
                let obj_type = infer_type_for_expr(&object.node, self.env, &self.var_types);
                if let PlutoType::Array(elem) = &obj_type {
                    let func_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_array_get"), self.builder.func);
                    let call = self.builder.ins().call(func_ref, &[handle, idx]);
                    let raw = self.builder.inst_results(call)[0];
                    Ok(from_array_slot(raw, elem, &mut self.builder))
                } else {
                    Err(CompileError::codegen(format!("index on non-array type {obj_type}")))
                }
            }
            Expr::EnumUnit { enum_name, variant, .. } => {
                let enum_info = self.env.enums.get(&enum_name.node).ok_or_else(|| {
                    CompileError::codegen(format!("unknown enum '{}'", enum_name.node))
                })?;
                let max_fields = enum_info.variants.iter().map(|(_, f)| f.len()).max().unwrap_or(0);
                let alloc_size = (1 + max_fields) as i64 * POINTER_SIZE as i64;
                let variant_idx = enum_info.variants.iter().position(|(n, _)| *n == variant.node).unwrap();

                let alloc_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_alloc"), self.builder.func);
                let size_val = self.builder.ins().iconst(types::I64, alloc_size);
                let call = self.builder.ins().call(alloc_ref, &[size_val]);
                let ptr = self.builder.inst_results(call)[0];

                let tag_val = self.builder.ins().iconst(types::I64, variant_idx as i64);
                self.builder.ins().store(MemFlags::new(), tag_val, ptr, Offset32::new(0));

                Ok(ptr)
            }
            Expr::EnumData { enum_name, variant, fields, .. } => {
                let enum_info = self.env.enums.get(&enum_name.node).ok_or_else(|| {
                    CompileError::codegen(format!("unknown enum '{}'", enum_name.node))
                })?.clone();
                let max_fields = enum_info.variants.iter().map(|(_, f)| f.len()).max().unwrap_or(0);
                let alloc_size = (1 + max_fields) as i64 * POINTER_SIZE as i64;
                let variant_idx = enum_info.variants.iter().position(|(n, _)| *n == variant.node).unwrap();
                let variant_fields = &enum_info.variants[variant_idx].1;

                let alloc_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_alloc"), self.builder.func);
                let size_val = self.builder.ins().iconst(types::I64, alloc_size);
                let call = self.builder.ins().call(alloc_ref, &[size_val]);
                let ptr = self.builder.inst_results(call)[0];

                let tag_val = self.builder.ins().iconst(types::I64, variant_idx as i64);
                self.builder.ins().store(MemFlags::new(), tag_val, ptr, Offset32::new(0));

                for (lit_name, lit_val) in fields {
                    let val = self.lower_expr(&lit_val.node)?;
                    let field_idx = variant_fields.iter().position(|(n, _)| *n == lit_name.node).unwrap();
                    let field_type = &variant_fields[field_idx].1;
                    let slot = to_array_slot(val, field_type, &mut self.builder);
                    let offset = ((1 + field_idx) as i32) * POINTER_SIZE;
                    self.builder.ins().store(MemFlags::new(), slot, ptr, Offset32::new(offset));
                }

                Ok(ptr)
            }
            Expr::FieldAccess { object, field } => {
                let ptr = self.lower_expr(&object.node)?;
                let obj_type = infer_type_for_expr(&object.node, self.env, &self.var_types);
                if let PlutoType::Class(class_name) = &obj_type {
                    let class_info = self.env.classes.get(class_name).ok_or_else(|| {
                        CompileError::codegen(format!("unknown class '{class_name}'"))
                    })?;
                    let (field_idx, (_, field_type, _)) = class_info.fields.iter()
                        .enumerate()
                        .find(|(_, (n, _, _))| *n == field.node)
                        .ok_or_else(|| {
                            CompileError::codegen(format!("unknown field '{}'", field.node))
                        })?;
                    let offset = (field_idx as i32) * POINTER_SIZE;
                    let cl_type = pluto_to_cranelift(field_type);
                    Ok(self.builder.ins().load(cl_type, MemFlags::new(), ptr, Offset32::new(offset)))
                } else {
                    Err(CompileError::codegen(format!("field access on non-class type {obj_type}")))
                }
            }
            Expr::Propagate { expr: inner } => {
                // Lower the inner call
                let val = self.lower_expr(&inner.node)?;

                // Check TLS error state
                let has_err_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_has_error"), self.builder.func);
                let has_err_call = self.builder.ins().call(has_err_ref, &[]);
                let has_err = self.builder.inst_results(has_err_call)[0];
                let zero = self.builder.ins().iconst(types::I64, 0);
                let is_error = self.builder.ins().icmp(IntCC::NotEqual, has_err, zero);

                let propagate_bb = self.builder.create_block();
                let continue_bb = self.builder.create_block();
                self.builder.ins().brif(is_error, propagate_bb, &[], continue_bb, &[]);

                // Propagate block: return default (error stays in TLS for caller)
                self.builder.switch_to_block(propagate_bb);
                self.builder.seal_block(propagate_bb);
                self.emit_default_return();

                // Continue block: no error, use the call result
                self.builder.switch_to_block(continue_bb);
                self.builder.seal_block(continue_bb);
                Ok(val)
            }
            Expr::Catch { expr: inner, handler } => {
                // Lower the inner call
                let val = self.lower_expr(&inner.node)?;
                let val_type = infer_type_for_expr(&inner.node, self.env, &self.var_types);
                let cl_type = pluto_to_cranelift(&val_type);

                // Check TLS error state
                let has_err_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_has_error"), self.builder.func);
                let has_err_call = self.builder.ins().call(has_err_ref, &[]);
                let has_err = self.builder.inst_results(has_err_call)[0];
                let zero = self.builder.ins().iconst(types::I64, 0);
                let is_error = self.builder.ins().icmp(IntCC::NotEqual, has_err, zero);

                let catch_bb = self.builder.create_block();
                let no_error_bb = self.builder.create_block();
                let merge_bb = self.builder.create_block();
                self.builder.append_block_param(merge_bb, cl_type);

                self.builder.ins().brif(is_error, catch_bb, &[], no_error_bb, &[]);

                // No-error block: jump to merge with the call result
                self.builder.switch_to_block(no_error_bb);
                self.builder.seal_block(no_error_bb);
                self.builder.ins().jump(merge_bb, &[val]);

                // Catch block: handle the error
                self.builder.switch_to_block(catch_bb);
                self.builder.seal_block(catch_bb);

                let handler_val = match handler {
                    CatchHandler::Wildcard { var, body } => {
                        // Get error object BEFORE clearing
                        let get_err_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_get_error"), self.builder.func);
                        let get_err_call = self.builder.ins().call(get_err_ref, &[]);
                        let err_obj = self.builder.inst_results(get_err_call)[0];

                        // Clear the error
                        let clear_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_clear_error"), self.builder.func);
                        self.builder.ins().call(clear_ref, &[]);

                        // Bind the error variable
                        let err_var = Variable::from_u32(self.next_var);
                        self.next_var += 1;
                        self.builder.declare_var(err_var, types::I64);
                        self.builder.def_var(err_var, err_obj);

                        let prev_var = self.variables.get(&var.node).cloned();
                        let prev_type = self.var_types.get(&var.node).cloned();
                        self.variables.insert(var.node.clone(), err_var);
                        self.var_types.insert(var.node.clone(), PlutoType::Error);

                        let result = self.lower_expr(&body.node)?;

                        // Restore previous binding
                        if let Some(pv) = prev_var {
                            self.variables.insert(var.node.clone(), pv);
                        } else {
                            self.variables.remove(&var.node);
                        }
                        if let Some(pt) = prev_type {
                            self.var_types.insert(var.node.clone(), pt);
                        } else {
                            self.var_types.remove(&var.node);
                        }

                        result
                    }
                    CatchHandler::Shorthand(fallback) => {
                        // Clear the error
                        let clear_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_clear_error"), self.builder.func);
                        self.builder.ins().call(clear_ref, &[]);

                        self.lower_expr(&fallback.node)?
                    }
                };

                self.builder.ins().jump(merge_bb, &[handler_val]);

                // Merge block: result is the block parameter
                self.builder.switch_to_block(merge_bb);
                self.builder.seal_block(merge_bb);
                Ok(self.builder.block_params(merge_bb)[0])
            }
            Expr::MethodCall { object, method, args } => {
                let obj_ptr = self.lower_expr(&object.node)?;
                let obj_type = infer_type_for_expr(&object.node, self.env, &self.var_types);

                // Array methods
                if let PlutoType::Array(elem) = &obj_type {
                    match method.node.as_str() {
                        "len" => {
                            let func_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_array_len"), self.builder.func);
                            let call = self.builder.ins().call(func_ref, &[obj_ptr]);
                            return Ok(self.builder.inst_results(call)[0]);
                        }
                        "push" => {
                            let elem = elem.clone();
                            let arg_val = self.lower_expr(&args[0].node)?;
                            let slot = to_array_slot(arg_val, &elem, &mut self.builder);
                            let func_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_array_push"), self.builder.func);
                            self.builder.ins().call(func_ref, &[obj_ptr, slot]);
                            return Ok(self.builder.ins().iconst(types::I64, 0));
                        }
                        _ => {
                            return Err(CompileError::codegen(format!("array has no method '{}'", method.node)));
                        }
                    }
                }

                // String methods
                if obj_type == PlutoType::String {
                    if method.node == "len" {
                        let func_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_string_len"), self.builder.func);
                        let call = self.builder.ins().call(func_ref, &[obj_ptr]);
                        return Ok(self.builder.inst_results(call)[0]);
                    }
                    return Err(CompileError::codegen(format!("string has no method '{}'", method.node)));
                }

                // Trait dynamic dispatch via handle
                if let PlutoType::Trait(trait_name) = &obj_type {
                    let trait_info = self.env.traits.get(trait_name).ok_or_else(|| {
                        CompileError::codegen(format!("unknown trait '{trait_name}'"))
                    })?;
                    let method_idx = trait_info.methods.iter()
                        .position(|(n, _)| *n == method.node)
                        .ok_or_else(|| {
                            CompileError::codegen(format!("trait '{trait_name}' has no method '{}'", method.node))
                        })?;

                    // Clone method_sig to avoid borrow conflict with self.lower_expr
                    let method_sig = trait_info.methods[method_idx].1.clone();

                    // obj_ptr is a trait handle: pointer to [data_ptr, vtable_ptr]
                    let data_ptr = self.builder.ins().load(types::I64, MemFlags::new(), obj_ptr, Offset32::new(0));
                    let vtable_ptr = self.builder.ins().load(types::I64, MemFlags::new(), obj_ptr, Offset32::new(POINTER_SIZE));

                    // Load fn_ptr from vtable at offset method_idx * POINTER_SIZE
                    let offset = (method_idx as i32) * POINTER_SIZE;
                    let fn_ptr = self.builder.ins().load(types::I64, MemFlags::new(), vtable_ptr, Offset32::new(offset));

                    // Build indirect call signature
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64)); // self (data_ptr)
                    for param_ty in &method_sig.params[1..] {
                        let cl_ty = pluto_to_cranelift(param_ty);
                        sig.params.push(AbiParam::new(cl_ty));
                    }
                    if method_sig.return_type != PlutoType::Void {
                        sig.returns.push(AbiParam::new(pluto_to_cranelift(&method_sig.return_type)));
                    }
                    let sig_ref = self.builder.func.import_signature(sig);

                    let mut call_args = vec![data_ptr]; // data_ptr as self
                    for (i, arg) in args.iter().enumerate() {
                        let val = self.lower_expr(&arg.node)?;
                        let arg_type = infer_type_for_expr(&arg.node, self.env, &self.var_types);
                        let param_expected = method_sig.params.get(i + 1); // +1 to skip self
                        if let (PlutoType::Class(cn), Some(PlutoType::Trait(tn))) = (&arg_type, param_expected) {
                            let wrapped = self.wrap_class_as_trait(val, cn, tn)?;
                            call_args.push(wrapped);
                        } else {
                            call_args.push(val);
                        }
                    }

                    let call = self.builder.ins().call_indirect(sig_ref, fn_ptr, &call_args);
                    let results = self.builder.inst_results(call);
                    if results.is_empty() {
                        Ok(self.builder.ins().iconst(types::I64, 0))
                    } else {
                        Ok(results[0])
                    }
                } else if let PlutoType::Class(class_name) = &obj_type {
                    let mangled = format!("{}_{}", class_name, method.node);
                    let func_id = self.func_ids.get(&mangled).ok_or_else(|| {
                        CompileError::codegen(format!("undefined method '{}'", method.node))
                    })?;
                    let func_ref = self.module.declare_func_in_func(*func_id, self.builder.func);

                    let mut arg_values = vec![obj_ptr];
                    for arg in args {
                        arg_values.push(self.lower_expr(&arg.node)?);
                    }

                    let call = self.builder.ins().call(func_ref, &arg_values);
                    let results = self.builder.inst_results(call);
                    if results.is_empty() {
                        Ok(self.builder.ins().iconst(types::I64, 0))
                    } else {
                        Ok(results[0])
                    }
                } else {
                    Err(CompileError::codegen(format!("method call on non-class type {obj_type}")))
                }
            }
            Expr::Closure { .. } => {
                Err(CompileError::codegen("closures should be lifted before codegen"))
            }
            Expr::ClosureCreate { fn_name, captures } => {
                // 1. Look up the function ID for the lifted closure function
                let func_id = self.func_ids.get(fn_name).ok_or_else(|| {
                    CompileError::codegen(format!("undefined closure function '{}'", fn_name))
                })?;

                // 2. Allocate closure object: [fn_ptr: i64] [capture_0: i64] ...
                let obj_size = (1 + captures.len()) as i64 * POINTER_SIZE as i64;
                let alloc_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_alloc"), self.builder.func);
                let size_val = self.builder.ins().iconst(types::I64, obj_size);
                let call = self.builder.ins().call(alloc_ref, &[size_val]);
                let closure_ptr = self.builder.inst_results(call)[0];

                // 3. Store function pointer at offset 0
                let func_ref = self.module.declare_func_in_func(*func_id, self.builder.func);
                let fn_addr = self.builder.ins().func_addr(types::I64, func_ref);
                self.builder.ins().store(MemFlags::new(), fn_addr, closure_ptr, Offset32::new(0));

                // 4. Store each captured variable at offset 8, 16, 24, ...
                for (i, cap_name) in captures.iter().enumerate() {
                    let cap_var = self.variables.get(cap_name).ok_or_else(|| {
                        CompileError::codegen(format!("undefined capture variable '{}'", cap_name))
                    })?;
                    let cap_val = self.builder.use_var(*cap_var);
                    let cap_type = self.var_types.get(cap_name).cloned().unwrap_or(PlutoType::Int);
                    let slot = to_array_slot(cap_val, &cap_type, &mut self.builder);
                    let offset = (1 + i) as i32 * POINTER_SIZE;
                    self.builder.ins().store(MemFlags::new(), slot, closure_ptr, Offset32::new(offset));
                }

                Ok(closure_ptr)
            }
        }
    }

    fn lower_print(
        &mut self,
        args: &[crate::span::Spanned<Expr>],
    ) -> Result<Value, CompileError> {
        let arg = &args[0];
        let arg_type = infer_type_for_expr(&arg.node, self.env, &self.var_types);
        let arg_val = self.lower_expr(&arg.node)?;

        match arg_type {
            PlutoType::Int => {
                let func_id = self.runtime.get("__pluto_print_int");
                let func_ref = self.module.declare_func_in_func(func_id, self.builder.func);
                self.builder.ins().call(func_ref, &[arg_val]);
            }
            PlutoType::Float => {
                let func_id = self.runtime.get("__pluto_print_float");
                let func_ref = self.module.declare_func_in_func(func_id, self.builder.func);
                self.builder.ins().call(func_ref, &[arg_val]);
            }
            PlutoType::String => {
                let func_id = self.runtime.get("__pluto_print_string");
                let func_ref = self.module.declare_func_in_func(func_id, self.builder.func);
                self.builder.ins().call(func_ref, &[arg_val]);
            }
            PlutoType::Bool => {
                let func_id = self.runtime.get("__pluto_print_bool");
                let func_ref = self.module.declare_func_in_func(func_id, self.builder.func);
                // Widen I8 bool to I32 for the C function
                let widened = self.builder.ins().uextend(types::I32, arg_val);
                self.builder.ins().call(func_ref, &[widened]);
            }
            PlutoType::Void | PlutoType::Class(_) | PlutoType::Array(_) | PlutoType::Trait(_) | PlutoType::Enum(_) | PlutoType::Fn(_, _) | PlutoType::Error | PlutoType::TypeParam(_) => {
                return Err(CompileError::codegen(format!("cannot print {arg_type}")));
            }
        }

        // print returns void, so return a dummy value
        Ok(self.builder.ins().iconst(types::I64, 0))
    }
}

/// Lower a function body into Cranelift IR.
pub fn lower_function(
    func: &Function,
    mut builder: FunctionBuilder<'_>,
    env: &TypeEnv,
    module: &mut dyn Module,
    func_ids: &HashMap<String, FuncId>,
    runtime: &RuntimeRegistry,
    class_name: Option<&str>,
    vtable_ids: &HashMap<(String, String), DataId>,
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

    // Closure prologue: load captured variables from __env pointer
    if let Some(captures) = env.closure_fns.get(&func.name.node) {
        let env_var = variables.get("__env").ok_or_else(|| {
            CompileError::codegen(format!("closure '{}' missing __env param", func.name.node))
        })?;
        let env_ptr = builder.use_var(*env_var);
        for (i, (cap_name, cap_type)) in captures.iter().enumerate() {
            let offset = (1 + i) as i32 * POINTER_SIZE; // skip fn_ptr at offset 0
            let raw = builder.ins().load(types::I64, MemFlags::new(), env_ptr, Offset32::new(offset));
            let val = from_array_slot(raw, cap_type, &mut builder);
            let var = Variable::from_u32(next_var);
            next_var += 1;
            builder.declare_var(var, pluto_to_cranelift(cap_type));
            builder.def_var(var, val);
            variables.insert(cap_name.clone(), var);
            var_types.insert(cap_name.clone(), cap_type.clone());
        }
    }

    // Compute expected return type for class→trait wrapping in return statements
    let expected_return_type = if func.name.node == "main" && class_name.is_none() {
        Some(PlutoType::Int)
    } else {
        let lookup_name = if let Some(cn) = class_name {
            format!("{}_{}", cn, func.name.node)
        } else {
            func.name.node.clone()
        };
        env.functions.get(&lookup_name).map(|s| s.return_type.clone())
    };

    // Build context and lower body
    let is_main = func.name.node == "main" && class_name.is_none();
    let mut ctx = LowerContext {
        builder,
        module,
        env,
        func_ids,
        runtime,
        vtable_ids,
        variables,
        var_types,
        next_var,
        expected_return_type,
    };

    let mut terminated = false;
    for stmt in &func.body.node.stmts {
        if terminated {
            break;
        }
        let stmt_terminates = matches!(stmt.node, Stmt::Return(_));
        ctx.lower_stmt(&stmt.node, &mut terminated)?;
        if stmt_terminates {
            terminated = true;
        }
    }

    // If main and no explicit return, return 0
    if is_main && !terminated {
        let zero = ctx.builder.ins().iconst(types::I64, 0);
        ctx.builder.ins().return_(&[zero]);
    } else if !terminated {
        // Void function with no return
        let lookup_name = if let Some(cn) = class_name {
            format!("{}_{}", cn, func.name.node)
        } else {
            func.name.node.clone()
        };
        let ret_type = ctx.env.functions.get(&lookup_name).map(|s| &s.return_type);
        if ret_type == Some(&PlutoType::Void) {
            ctx.builder.ins().return_(&[]);
        }
    }

    ctx.finalize();
    Ok(())
}

fn resolve_param_type(param: &Param, env: &TypeEnv) -> PlutoType {
    resolve_type_expr_to_pluto(&param.ty.node, env)
}

pub fn resolve_type_expr_to_pluto(ty: &TypeExpr, env: &TypeEnv) -> PlutoType {
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
                } else if env.enums.contains_key(name) {
                    PlutoType::Enum(name.clone())
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
            } else if env.enums.contains_key(&prefixed) {
                PlutoType::Enum(prefixed)
            } else {
                PlutoType::Void
            }
        }
        TypeExpr::Fn { params, return_type } => {
            let param_types = params.iter()
                .map(|p| resolve_type_expr_to_pluto(&p.node, env))
                .collect();
            let ret = resolve_type_expr_to_pluto(&return_type.node, env);
            PlutoType::Fn(param_types, Box::new(ret))
        }
        TypeExpr::Generic { .. } => {
            panic!("Generic TypeExpr should not reach codegen — monomorphize should have resolved it")
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
        PlutoType::Enum(_) => types::I64,      // pointer to heap-allocated enum
        PlutoType::Fn(_, _) => types::I64,     // pointer to closure object
        PlutoType::Error => types::I64,        // pointer to error object
        PlutoType::TypeParam(_) => panic!("TypeParam should not reach codegen"),
    }
}

/// Quick type inference at codegen time (type checker has already validated).
fn infer_type_for_expr(expr: &Expr, env: &TypeEnv, var_types: &HashMap<String, PlutoType>) -> PlutoType {
    match expr {
        Expr::IntLit(_) => PlutoType::Int,
        Expr::FloatLit(_) => PlutoType::Float,
        Expr::BoolLit(_) => PlutoType::Bool,
        Expr::StringLit(_) => PlutoType::String,
        Expr::StringInterp { .. } => PlutoType::String,
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
            // Check if calling a closure variable first
            if let Some(PlutoType::Fn(_, ret)) = var_types.get(&name.node) {
                return *ret.clone();
            }
            env.functions.get(&name.node).map(|s| s.return_type.clone()).unwrap_or(PlutoType::Void)
        }
        Expr::StructLit { name, .. } => PlutoType::Class(name.node.clone()),
        Expr::FieldAccess { object, field } => {
            let obj_type = infer_type_for_expr(&object.node, env, var_types);
            if let PlutoType::Class(class_name) = &obj_type {
                if let Some(class_info) = env.classes.get(class_name) {
                    class_info.fields.iter()
                        .find(|(n, _, _)| *n == field.node)
                        .map(|(_, t, _)| t.clone())
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
        Expr::EnumUnit { enum_name, .. } | Expr::EnumData { enum_name, .. } => {
            PlutoType::Enum(enum_name.node.clone())
        }
        Expr::Propagate { expr } => {
            // Propagation returns the success type of the inner call
            infer_type_for_expr(&expr.node, env, var_types)
        }
        Expr::Catch { expr, .. } => {
            // Catch returns the success type (same as the inner call)
            infer_type_for_expr(&expr.node, env, var_types)
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
        Expr::Closure { params, return_type, .. } => {
            let param_types: Vec<PlutoType> = params.iter()
                .map(|p| resolve_type_expr_to_pluto(&p.ty.node, env))
                .collect();
            let ret = match return_type {
                Some(rt) => resolve_type_expr_to_pluto(&rt.node, env),
                None => PlutoType::Void,
            };
            PlutoType::Fn(param_types, Box::new(ret))
        }
        Expr::ClosureCreate { fn_name, .. } => {
            if let Some(sig) = env.functions.get(fn_name) {
                // Return fn type: skip the __env param (first param)
                let param_types = sig.params[1..].to_vec();
                PlutoType::Fn(param_types, Box::new(sig.return_type.clone()))
            } else {
                PlutoType::Void
            }
        }
    }
}
