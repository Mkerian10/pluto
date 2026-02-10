use std::collections::{HashMap, HashSet};

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
    source: &'a str,
    /// Class invariants: class_name → Vec<(expr, description_string)>
    class_invariants: &'a HashMap<String, Vec<(Expr, String)>>,
    // Per-function mutable state
    variables: HashMap<String, Variable>,
    var_types: HashMap<String, PlutoType>,
    next_var: u32,
    expected_return_type: Option<PlutoType>,
    /// Stack of (continue_target, break_target) blocks for break/continue
    loop_stack: Vec<(cranelift_codegen::ir::Block, cranelift_codegen::ir::Block)>,
    /// Variables holding Sender handles that need sender_dec on function exit
    sender_cleanup_vars: Vec<Variable>,
    /// If non-None, all returns jump here for sender cleanup before actual return
    exit_block: Option<cranelift_codegen::ir::Block>,
    /// True when lowering the program's main function (not a method or app main)
    is_main: bool,
}

impl<'a> LowerContext<'a> {
    fn finalize(self) {
        self.builder.finalize();
    }

    /// Call a runtime function that returns a value.
    fn call_runtime(&mut self, name: &str, args: &[Value]) -> Value {
        let func_ref = self.module.declare_func_in_func(self.runtime.get(name), self.builder.func);
        let call = self.builder.ins().call(func_ref, args);
        let results = self.builder.inst_results(call);
        debug_assert!(!results.is_empty(), "call_runtime used on void function {name}");
        results[0]
    }

    /// Call a runtime function that returns void.
    fn call_runtime_void(&mut self, name: &str, args: &[Value]) {
        let func_ref = self.module.declare_func_in_func(self.runtime.get(name), self.builder.func);
        self.builder.ins().call(func_ref, args);
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
        Ok(self.call_runtime("__pluto_trait_wrap", &[class_val, vtable_ptr]))
    }

    /// Emit a return with the default value for the current function's return type.
    /// Used by raise and propagation to exit the function when an error occurs.
    fn emit_default_return(&mut self) {
        match &self.expected_return_type {
            Some(PlutoType::Void) | None => {
                if let Some(exit_bb) = self.exit_block {
                    self.builder.ins().jump(exit_bb, &[]);
                } else {
                    self.builder.ins().return_(&[]);
                }
            }
            Some(PlutoType::Float) => {
                let val = self.builder.ins().f64const(0.0);
                if let Some(exit_bb) = self.exit_block {
                    self.builder.ins().jump(exit_bb, &[val]);
                } else {
                    self.builder.ins().return_(&[val]);
                }
            }
            Some(PlutoType::Bool) | Some(PlutoType::Byte) => {
                let val = self.builder.ins().iconst(types::I8, 0);
                if let Some(exit_bb) = self.exit_block {
                    self.builder.ins().jump(exit_bb, &[val]);
                } else {
                    self.builder.ins().return_(&[val]);
                }
            }
            Some(_) => {
                // Int, String, Class, Array, Enum, Map, Set, Bytes, Error — all I64
                let val = self.builder.ins().iconst(types::I64, 0);
                if let Some(exit_bb) = self.exit_block {
                    self.builder.ins().jump(exit_bb, &[val]);
                } else {
                    self.builder.ins().return_(&[val]);
                }
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

    /// Emit runtime invariant checks for a class after construction or mutation.
    /// `class_name` is the class to check, `obj_ptr` is the pointer to the struct.
    fn emit_invariant_checks(
        &mut self,
        class_name: &str,
        obj_ptr: Value,
    ) -> Result<(), CompileError> {
        let invariants = match self.class_invariants.get(class_name) {
            Some(invs) if !invs.is_empty() => invs.clone(),
            _ => return Ok(()),
        };

        // Temporarily bind `self` to obj_ptr so invariant expressions resolve self.field
        let prev_self_var = self.variables.get("self").cloned();
        let prev_self_type = self.var_types.get("self").cloned();

        let self_var = Variable::from_u32(self.next_var);
        self.next_var += 1;
        self.builder.declare_var(self_var, types::I64);
        self.builder.def_var(self_var, obj_ptr);
        self.variables.insert("self".to_string(), self_var);
        self.var_types.insert("self".to_string(), PlutoType::Class(class_name.to_string()));

        for (inv_expr, inv_desc) in &invariants {
            let result = self.lower_expr(inv_expr)?;

            // Branch: if result is false (0), call violation handler
            let violation_bb = self.builder.create_block();
            let ok_bb = self.builder.create_block();

            self.builder.ins().brif(result, ok_bb, &[], violation_bb, &[]);

            // Violation block: create strings and call __pluto_invariant_violation
            self.builder.switch_to_block(violation_bb);
            self.builder.seal_block(violation_bb);

            // Create class name Pluto string
            let name_raw = self.create_data_str(class_name)?;
            let name_len = self.builder.ins().iconst(types::I64, class_name.len() as i64);
            let name_str = self.call_runtime("__pluto_string_new", &[name_raw, name_len]);

            // Create invariant description Pluto string
            let desc_raw = self.create_data_str(inv_desc)?;
            let desc_len = self.builder.ins().iconst(types::I64, inv_desc.len() as i64);
            let desc_str = self.call_runtime("__pluto_string_new", &[desc_raw, desc_len]);

            self.call_runtime_void("__pluto_invariant_violation", &[name_str, desc_str]);
            // __pluto_invariant_violation calls exit(), but Cranelift needs a terminator
            self.builder.ins().trap(cranelift_codegen::ir::TrapCode::unwrap_user(1));

            // OK block: continue
            self.builder.switch_to_block(ok_bb);
            self.builder.seal_block(ok_bb);
        }

        // Restore previous self binding
        if let Some(pv) = prev_self_var {
            self.variables.insert("self".to_string(), pv);
        } else {
            self.variables.remove("self");
        }
        if let Some(pt) = prev_self_type {
            self.var_types.insert("self".to_string(), pt);
        } else {
            self.var_types.remove("self");
        }

        Ok(())
    }

    // ── lower_stmt dispatch ──────────────────────────────────────────────

    fn lower_stmt(
        &mut self,
        stmt: &Stmt,
        terminated: &mut bool,
    ) -> Result<(), CompileError> {
        if *terminated {
            return Ok(());
        }
        match stmt {
            Stmt::Let { name, ty, value } => self.lower_let(name, ty, value),
            Stmt::LetChan { sender, receiver, elem_type, capacity } => self.lower_let_chan(sender, receiver, elem_type, capacity),
            Stmt::Return(value) => {
                match value {
                    Some(expr) => {
                        let val = self.lower_expr(&expr.node)?;
                        let val_type = infer_type_for_expr(&expr.node, self.env, &self.var_types);

                        // If returning a void expression (e.g., spawn closure wrapping a void function),
                        // lower the expr for side effects but emit return_(&[])
                        if val_type == PlutoType::Void {
                            if let Some(exit_bb) = self.exit_block {
                                self.builder.ins().jump(exit_bb, &[]);
                            } else {
                                self.builder.ins().return_(&[]);
                            }
                        } else {
                            // If returning a class where a trait is expected, wrap it
                            let expected = self.expected_return_type.clone();
                            let final_val = match (&val_type, &expected) {
                                (PlutoType::Class(cn), Some(PlutoType::Trait(tn))) => {
                                    self.wrap_class_as_trait(val, cn, tn)?
                                }
                                _ => val,
                            };
                            if let Some(exit_bb) = self.exit_block {
                                self.builder.ins().jump(exit_bb, &[final_val]);
                            } else {
                                self.builder.ins().return_(&[final_val]);
                            }
                        }
                    }
                    None => {
                        if let Some(exit_bb) = self.exit_block {
                            self.builder.ins().jump(exit_bb, &[]);
                        } else {
                            self.builder.ins().return_(&[]);
                        }
                    }
                }
                *terminated = true;
                Ok(())
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
                Ok(())
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
                Ok(())
            }
            Stmt::IndexAssign { object, index, value } => {
                let handle = self.lower_expr(&object.node)?;
                let idx = self.lower_expr(&index.node)?;
                let val = self.lower_expr(&value.node)?;
                let obj_type = infer_type_for_expr(&object.node, self.env, &self.var_types);
                if let PlutoType::Array(elem) = &obj_type {
                    let slot = to_array_slot(val, elem, &mut self.builder);
                    self.call_runtime_void("__pluto_array_set", &[handle, idx, slot]);
                } else if obj_type == PlutoType::Bytes {
                    let val_wide = self.builder.ins().uextend(types::I64, val);
                    self.call_runtime_void("__pluto_bytes_set", &[handle, idx, val_wide]);
                } else if let PlutoType::Map(key_ty, val_ty) = &obj_type {
                    let tag = self.builder.ins().iconst(types::I64, key_type_tag(key_ty));
                    let key_slot = to_array_slot(idx, key_ty, &mut self.builder);
                    let val_slot = to_array_slot(val, val_ty, &mut self.builder);
                    self.call_runtime_void("__pluto_map_insert", &[handle, tag, key_slot, val_slot]);
                }
                Ok(())
            }
            Stmt::If { condition, then_block, else_block } => {
                self.lower_if(condition, then_block, else_block, terminated)
            }
            Stmt::While { condition, body } => self.lower_while(condition, body),
            Stmt::For { var, iterable, body } => self.lower_for(var, iterable, body),
            Stmt::Match { expr, arms } => self.lower_match_stmt(expr, arms, terminated),
            Stmt::Raise { error_name, fields } => {
                self.lower_raise(error_name, fields)?;
                *terminated = true;
                Ok(())
            }
            Stmt::Break => {
                let (_, break_bb) = self.loop_stack.last().ok_or_else(|| {
                    CompileError::codegen("break outside of loop".to_string())
                })?;
                self.builder.ins().jump(*break_bb, &[]);
                *terminated = true;
                Ok(())
            }
            Stmt::Continue => {
                let (continue_bb, _) = self.loop_stack.last().ok_or_else(|| {
                    CompileError::codegen("continue outside of loop".to_string())
                })?;
                self.builder.ins().jump(*continue_bb, &[]);
                *terminated = true;
                Ok(())
            }
            Stmt::Select { arms, default } => self.lower_select(arms, default, terminated),
            Stmt::Expr(expr) => {
                self.lower_expr(&expr.node)?;
                Ok(())
            }
        }
    }

    // ── lower_stmt extracted helpers ─────────────────────────────────────

    fn lower_let(
        &mut self,
        name: &crate::span::Spanned<String>,
        ty: &Option<crate::span::Spanned<TypeExpr>>,
        value: &crate::span::Spanned<Expr>,
    ) -> Result<(), CompileError> {
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
        Ok(())
    }

    fn lower_let_chan(
        &mut self,
        sender: &crate::span::Spanned<String>,
        receiver: &crate::span::Spanned<String>,
        elem_type: &crate::span::Spanned<TypeExpr>,
        capacity: &Option<crate::span::Spanned<Expr>>,
    ) -> Result<(), CompileError> {
        let cap_val = if let Some(cap_expr) = capacity {
            self.lower_expr(&cap_expr.node)?
        } else {
            self.builder.ins().iconst(types::I64, 0)
        };
        let handle = self.call_runtime("__pluto_chan_create", &[cap_val]);

        let elem_ty = resolve_type_expr_to_pluto(&elem_type.node, self.env);

        // Define sender variable — reuse pre-declared cleanup variable if it exists
        if let Some(&existing_var) = self.variables.get(&sender.node) {
            self.builder.def_var(existing_var, handle);
        } else {
            let tx_var = Variable::from_u32(self.next_var);
            self.next_var += 1;
            self.builder.declare_var(tx_var, types::I64);
            self.builder.def_var(tx_var, handle);
            self.variables.insert(sender.node.clone(), tx_var);
        }
        self.var_types.insert(sender.node.clone(), PlutoType::Sender(Box::new(elem_ty.clone())));

        // Define receiver variable
        let rx_var = Variable::from_u32(self.next_var);
        self.next_var += 1;
        self.builder.declare_var(rx_var, types::I64);
        self.builder.def_var(rx_var, handle);
        self.variables.insert(receiver.node.clone(), rx_var);
        self.var_types.insert(receiver.node.clone(), PlutoType::Receiver(Box::new(elem_ty)));

        Ok(())
    }

    fn lower_if(
        &mut self,
        condition: &crate::span::Spanned<Expr>,
        then_block: &crate::span::Spanned<Block>,
        else_block: &Option<crate::span::Spanned<Block>>,
        terminated: &mut bool,
    ) -> Result<(), CompileError> {
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
        Ok(())
    }

    fn lower_while(
        &mut self,
        condition: &crate::span::Spanned<Expr>,
        body: &crate::span::Spanned<Block>,
    ) -> Result<(), CompileError> {
        let header_bb = self.builder.create_block();
        let body_bb = self.builder.create_block();
        let exit_bb = self.builder.create_block();

        self.builder.ins().jump(header_bb, &[]);

        self.builder.switch_to_block(header_bb);
        let cond_val = self.lower_expr(&condition.node)?;
        self.builder.ins().brif(cond_val, body_bb, &[], exit_bb, &[]);

        self.builder.switch_to_block(body_bb);
        self.builder.seal_block(body_bb);
        self.loop_stack.push((header_bb, exit_bb));
        let mut body_terminated = false;
        for s in &body.node.stmts {
            self.lower_stmt(&s.node, &mut body_terminated)?;
        }
        self.loop_stack.pop();
        if !body_terminated {
            self.builder.ins().jump(header_bb, &[]);
        }

        self.builder.seal_block(header_bb);
        self.builder.switch_to_block(exit_bb);
        self.builder.seal_block(exit_bb);
        Ok(())
    }

    fn lower_for(
        &mut self,
        var: &crate::span::Spanned<String>,
        iterable: &crate::span::Spanned<Expr>,
        body: &crate::span::Spanned<Block>,
    ) -> Result<(), CompileError> {
        let iter_type = infer_type_for_expr(&iterable.node, self.env, &self.var_types);
        match &iter_type {
            PlutoType::Range => self.lower_for_range(var, iterable, body),
            PlutoType::Array(_) => self.lower_for_array(var, iterable, body),
            PlutoType::Bytes => self.lower_for_bytes(var, iterable, body),
            PlutoType::String => self.lower_for_string(var, iterable, body),
            PlutoType::Receiver(_) => self.lower_for_receiver(var, iterable, body),
            _ => Err(CompileError::codegen("for loop requires array, range, string, or receiver".to_string())),
        }
    }

    fn lower_for_range(
        &mut self,
        var: &crate::span::Spanned<String>,
        iterable: &crate::span::Spanned<Expr>,
        body: &crate::span::Spanned<Block>,
    ) -> Result<(), CompileError> {
        // Extract start, end, inclusive from the Range expression
        let (start_expr, end_expr, inclusive) = match &iterable.node {
            Expr::Range { start, end, inclusive } => (&start.node, &end.node, *inclusive),
            _ => return Err(CompileError::codegen("expected range expression".to_string())),
        };

        let start_val = self.lower_expr(start_expr)?;
        let end_val = self.lower_expr(end_expr)?;

        // Create counter variable initialized to start
        let counter_var = Variable::from_u32(self.next_var);
        self.next_var += 1;
        self.builder.declare_var(counter_var, types::I64);
        self.builder.def_var(counter_var, start_val);

        // Create blocks
        let header_bb = self.builder.create_block();
        let body_bb = self.builder.create_block();
        let increment_bb = self.builder.create_block();
        let exit_bb = self.builder.create_block();

        self.builder.ins().jump(header_bb, &[]);

        // Header: check counter < end (exclusive) or counter <= end (inclusive)
        self.builder.switch_to_block(header_bb);
        let counter = self.builder.use_var(counter_var);
        let cmp = if inclusive {
            IntCC::SignedLessThanOrEqual
        } else {
            IntCC::SignedLessThan
        };
        let cond = self.builder.ins().icmp(cmp, counter, end_val);
        self.builder.ins().brif(cond, body_bb, &[], exit_bb, &[]);

        // Body
        self.builder.switch_to_block(body_bb);
        self.builder.seal_block(body_bb);

        // Loop variable = counter value directly (ranges iterate ints)
        let prev_var = self.variables.get(&var.node).cloned();
        let prev_type = self.var_types.get(&var.node).cloned();

        // Use counter_var as the loop variable directly
        self.variables.insert(var.node.clone(), counter_var);
        self.var_types.insert(var.node.clone(), PlutoType::Int);

        // Push loop stack: continue goes to increment, break goes to exit
        self.loop_stack.push((increment_bb, exit_bb));
        let mut body_terminated = false;
        for s in &body.node.stmts {
            self.lower_stmt(&s.node, &mut body_terminated)?;
        }
        self.loop_stack.pop();

        if !body_terminated {
            self.builder.ins().jump(increment_bb, &[]);
        }

        // Increment block
        self.builder.switch_to_block(increment_bb);
        self.builder.seal_block(increment_bb);
        let counter_inc = self.builder.use_var(counter_var);
        let one = self.builder.ins().iconst(types::I64, 1);
        let new_counter = self.builder.ins().iadd(counter_inc, one);
        self.builder.def_var(counter_var, new_counter);
        self.builder.ins().jump(header_bb, &[]);

        self.builder.seal_block(header_bb);
        self.builder.switch_to_block(exit_bb);
        self.builder.seal_block(exit_bb);

        // Restore prior variable binding
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
        Ok(())
    }

    fn lower_for_array(
        &mut self,
        var: &crate::span::Spanned<String>,
        iterable: &crate::span::Spanned<Expr>,
        body: &crate::span::Spanned<Block>,
    ) -> Result<(), CompileError> {
        // Lower iterable to get array handle
        let handle = self.lower_expr(&iterable.node)?;

        // Get element type from iterable
        let iter_type = infer_type_for_expr(&iterable.node, self.env, &self.var_types);
        let elem_type = match &iter_type {
            PlutoType::Array(elem) => *elem.clone(),
            _ => return Err(CompileError::codegen("for loop requires array".to_string())),
        };

        // Call len() on the array
        let len_val = self.call_runtime("__pluto_array_len", &[handle]);

        // Create counter variable, init to 0
        let counter_var = Variable::from_u32(self.next_var);
        self.next_var += 1;
        self.builder.declare_var(counter_var, types::I64);
        let zero = self.builder.ins().iconst(types::I64, 0);
        self.builder.def_var(counter_var, zero);

        // Create blocks
        let header_bb = self.builder.create_block();
        let body_bb = self.builder.create_block();
        let increment_bb = self.builder.create_block();
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
        let raw_slot = self.call_runtime("__pluto_array_get", &[handle, counter_for_get]);
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

        // Push loop stack: continue goes to increment, break goes to exit
        self.loop_stack.push((increment_bb, exit_bb));
        let mut body_terminated = false;
        for s in &body.node.stmts {
            self.lower_stmt(&s.node, &mut body_terminated)?;
        }
        self.loop_stack.pop();

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

        if !body_terminated {
            self.builder.ins().jump(increment_bb, &[]);
        }

        // Increment block
        self.builder.switch_to_block(increment_bb);
        self.builder.seal_block(increment_bb);
        let counter_inc = self.builder.use_var(counter_var);
        let one = self.builder.ins().iconst(types::I64, 1);
        let new_counter = self.builder.ins().iadd(counter_inc, one);
        self.builder.def_var(counter_var, new_counter);
        self.builder.ins().jump(header_bb, &[]);

        self.builder.seal_block(header_bb);
        self.builder.switch_to_block(exit_bb);
        self.builder.seal_block(exit_bb);
        Ok(())
    }

    fn lower_for_bytes(
        &mut self,
        var: &crate::span::Spanned<String>,
        iterable: &crate::span::Spanned<Expr>,
        body: &crate::span::Spanned<Block>,
    ) -> Result<(), CompileError> {
        let handle = self.lower_expr(&iterable.node)?;
        let len_val = self.call_runtime("__pluto_bytes_len", &[handle]);

        // Counter variable
        let counter_var = Variable::from_u32(self.next_var);
        self.next_var += 1;
        self.builder.declare_var(counter_var, types::I64);
        let zero = self.builder.ins().iconst(types::I64, 0);
        self.builder.def_var(counter_var, zero);

        let header_bb = self.builder.create_block();
        let body_bb = self.builder.create_block();
        let increment_bb = self.builder.create_block();
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

        let counter_for_get = self.builder.use_var(counter_var);
        let raw = self.call_runtime("__pluto_bytes_get", &[handle, counter_for_get]);
        let elem_val = self.builder.ins().ireduce(types::I8, raw);

        let prev_var = self.variables.get(&var.node).cloned();
        let prev_type = self.var_types.get(&var.node).cloned();

        let loop_var = Variable::from_u32(self.next_var);
        self.next_var += 1;
        self.builder.declare_var(loop_var, types::I8);
        self.builder.def_var(loop_var, elem_val);
        self.variables.insert(var.node.clone(), loop_var);
        self.var_types.insert(var.node.clone(), PlutoType::Byte);

        self.loop_stack.push((increment_bb, exit_bb));
        let mut body_terminated = false;
        for s in &body.node.stmts {
            self.lower_stmt(&s.node, &mut body_terminated)?;
        }
        self.loop_stack.pop();

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

        if !body_terminated {
            self.builder.ins().jump(increment_bb, &[]);
        }

        self.builder.switch_to_block(increment_bb);
        self.builder.seal_block(increment_bb);
        let counter_inc = self.builder.use_var(counter_var);
        let one = self.builder.ins().iconst(types::I64, 1);
        let new_counter = self.builder.ins().iadd(counter_inc, one);
        self.builder.def_var(counter_var, new_counter);
        self.builder.ins().jump(header_bb, &[]);

        self.builder.seal_block(header_bb);
        self.builder.switch_to_block(exit_bb);
        self.builder.seal_block(exit_bb);
        Ok(())
    }

    fn lower_for_string(
        &mut self,
        var: &crate::span::Spanned<String>,
        iterable: &crate::span::Spanned<Expr>,
        body: &crate::span::Spanned<Block>,
    ) -> Result<(), CompileError> {
        let handle = self.lower_expr(&iterable.node)?;

        // Get string length
        let len_val = self.call_runtime("__pluto_string_len", &[handle]);

        // Create counter variable, init to 0
        let counter_var = Variable::from_u32(self.next_var);
        self.next_var += 1;
        self.builder.declare_var(counter_var, types::I64);
        let zero = self.builder.ins().iconst(types::I64, 0);
        self.builder.def_var(counter_var, zero);

        // Create blocks
        let header_bb = self.builder.create_block();
        let body_bb = self.builder.create_block();
        let increment_bb = self.builder.create_block();
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

        // Get character: char_at(handle, counter)
        let counter_for_get = self.builder.use_var(counter_var);
        let char_val = self.call_runtime("__pluto_string_char_at", &[handle, counter_for_get]);

        // Create loop variable
        let prev_var = self.variables.get(&var.node).cloned();
        let prev_type = self.var_types.get(&var.node).cloned();

        let loop_var = Variable::from_u32(self.next_var);
        self.next_var += 1;
        self.builder.declare_var(loop_var, types::I64);
        self.builder.def_var(loop_var, char_val);
        self.variables.insert(var.node.clone(), loop_var);
        self.var_types.insert(var.node.clone(), PlutoType::String);

        // Push loop stack: continue goes to increment, break goes to exit
        self.loop_stack.push((increment_bb, exit_bb));
        let mut body_terminated = false;
        for s in &body.node.stmts {
            self.lower_stmt(&s.node, &mut body_terminated)?;
        }
        self.loop_stack.pop();

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

        if !body_terminated {
            self.builder.ins().jump(increment_bb, &[]);
        }

        // Increment block
        self.builder.switch_to_block(increment_bb);
        self.builder.seal_block(increment_bb);
        let counter_inc = self.builder.use_var(counter_var);
        let one = self.builder.ins().iconst(types::I64, 1);
        let new_counter = self.builder.ins().iadd(counter_inc, one);
        self.builder.def_var(counter_var, new_counter);
        self.builder.ins().jump(header_bb, &[]);

        self.builder.seal_block(header_bb);
        self.builder.switch_to_block(exit_bb);
        self.builder.seal_block(exit_bb);
        Ok(())
    }

    fn lower_for_receiver(
        &mut self,
        var: &crate::span::Spanned<String>,
        iterable: &crate::span::Spanned<Expr>,
        body: &crate::span::Spanned<Block>,
    ) -> Result<(), CompileError> {
        let handle = self.lower_expr(&iterable.node)?;

        let iter_type = infer_type_for_expr(&iterable.node, self.env, &self.var_types);
        let elem_type = match &iter_type {
            PlutoType::Receiver(elem) => *elem.clone(),
            _ => return Err(CompileError::codegen("for-in requires receiver".to_string())),
        };

        // Blocks: header tries recv, check_err tests for error, body runs loop, exit leaves
        let header_bb = self.builder.create_block();
        let check_bb = self.builder.create_block();
        let body_bb = self.builder.create_block();
        let exit_bb = self.builder.create_block();

        self.builder.ins().jump(header_bb, &[]);

        // Header: call recv, check for error
        self.builder.switch_to_block(header_bb);
        let raw_val = self.call_runtime("__pluto_chan_recv", &[handle]);
        let has_err = self.call_runtime("__pluto_has_error", &[]);
        let zero = self.builder.ins().iconst(types::I64, 0);
        let err_cond = self.builder.ins().icmp(IntCC::NotEqual, has_err, zero);
        self.builder.ins().brif(err_cond, check_bb, &[], body_bb, &[]);

        // Check block: recv errored (ChannelClosed) — clear error and exit loop
        self.builder.switch_to_block(check_bb);
        self.builder.seal_block(check_bb);
        self.call_runtime_void("__pluto_clear_error", &[]);
        self.builder.ins().jump(exit_bb, &[]);

        // Body block
        self.builder.switch_to_block(body_bb);
        self.builder.seal_block(body_bb);

        let elem_val = from_array_slot(raw_val, &elem_type, &mut self.builder);

        // Create loop variable
        let prev_var = self.variables.get(&var.node).cloned();
        let prev_type = self.var_types.get(&var.node).cloned();

        let loop_var = Variable::from_u32(self.next_var);
        self.next_var += 1;
        let cl_elem_type = pluto_to_cranelift(&elem_type);
        self.builder.declare_var(loop_var, cl_elem_type);
        self.builder.def_var(loop_var, elem_val);
        self.variables.insert(var.node.clone(), loop_var);
        self.var_types.insert(var.node.clone(), elem_type);

        // Push loop stack: continue goes to header (re-recv), break goes to exit
        self.loop_stack.push((header_bb, exit_bb));
        let mut body_terminated = false;
        for s in &body.node.stmts {
            self.lower_stmt(&s.node, &mut body_terminated)?;
        }
        self.loop_stack.pop();

        // Restore prior variable binding
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

        if !body_terminated {
            self.builder.ins().jump(header_bb, &[]);
        }

        self.builder.seal_block(header_bb);
        self.builder.switch_to_block(exit_bb);
        self.builder.seal_block(exit_bb);
        Ok(())
    }

    fn lower_match_stmt(
        &mut self,
        expr: &crate::span::Spanned<Expr>,
        arms: &[MatchArm],
        terminated: &mut bool,
    ) -> Result<(), CompileError> {
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
        Ok(())
    }

    fn lower_raise(
        &mut self,
        error_name: &crate::span::Spanned<String>,
        fields: &[(crate::span::Spanned<String>, crate::span::Spanned<Expr>)],
    ) -> Result<(), CompileError> {
        let error_info = self.env.errors.get(&error_name.node).ok_or_else(|| {
            CompileError::codegen(format!("unknown error '{}'", error_name.node))
        })?.clone();
        let num_fields = error_info.fields.len();
        let size = (num_fields as i64 * POINTER_SIZE as i64).max(POINTER_SIZE as i64);

        // Allocate error object
        let size_val = self.builder.ins().iconst(types::I64, size);
        let ptr = self.call_runtime("__pluto_alloc", &[size_val]);

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
        self.call_runtime_void("__pluto_raise_error", &[ptr]);

        // Return default value (caller checks TLS)
        self.emit_default_return();
        Ok(())
    }

    fn lower_select(
        &mut self,
        arms: &[SelectArm],
        default: &Option<crate::span::Spanned<Block>>,
        terminated: &mut bool,
    ) -> Result<(), CompileError> {
        let count = arms.len() as i64;

        // 1. Allocate buffer: 3 * count i64 slots
        //    [handles | ops | values]
        let buf_size = self.builder.ins().iconst(types::I64, 3 * count * POINTER_SIZE as i64);
        let buffer = self.call_runtime("__pluto_alloc", &[buf_size]);

        // 2. Eagerly evaluate all channel exprs and send values, store into buffer
        for (i, arm) in arms.iter().enumerate() {
            let slot_offset = (i as i32) * POINTER_SIZE;
            let op_offset = (count as i32 + i as i32) * POINTER_SIZE;
            let val_offset = (2 * count as i32 + i as i32) * POINTER_SIZE;

            match &arm.op {
                SelectOp::Recv { channel, .. } => {
                    let chan_val = self.lower_expr(&channel.node)?;
                    self.builder.ins().store(MemFlags::new(), chan_val, buffer, Offset32::new(slot_offset));
                    let op_val = self.builder.ins().iconst(types::I64, 0); // 0 = recv
                    self.builder.ins().store(MemFlags::new(), op_val, buffer, Offset32::new(op_offset));
                    // values[i] unused for recv (will be written by runtime)
                    let zero = self.builder.ins().iconst(types::I64, 0);
                    self.builder.ins().store(MemFlags::new(), zero, buffer, Offset32::new(val_offset));
                }
                SelectOp::Send { channel, value } => {
                    let chan_val = self.lower_expr(&channel.node)?;
                    self.builder.ins().store(MemFlags::new(), chan_val, buffer, Offset32::new(slot_offset));
                    let op_val = self.builder.ins().iconst(types::I64, 1); // 1 = send
                    self.builder.ins().store(MemFlags::new(), op_val, buffer, Offset32::new(op_offset));
                    let send_val = self.lower_expr(&value.node)?;
                    let slot = to_array_slot(send_val, &infer_type_for_expr(&value.node, self.env, &self.var_types), &mut self.builder);
                    self.builder.ins().store(MemFlags::new(), slot, buffer, Offset32::new(val_offset));
                }
            }
        }

        // 3. Call __pluto_select(buffer, count, has_default)
        let count_val = self.builder.ins().iconst(types::I64, count);
        let has_default_val = self.builder.ins().iconst(types::I64, if default.is_some() { 1 } else { 0 });
        let result = self.call_runtime("__pluto_select", &[buffer, count_val, has_default_val]);

        // 4. If no default and result == -2 → error path (TLS already set by runtime)
        let merge_bb = self.builder.create_block();

        if default.is_none() {
            let neg2 = self.builder.ins().iconst(types::I64, -2i64);
            let is_err = self.builder.ins().icmp(IntCC::Equal, result, neg2);
            let err_bb = self.builder.create_block();
            let dispatch_bb = self.builder.create_block();
            self.builder.ins().brif(is_err, err_bb, &[], dispatch_bb, &[]);

            // Error block: propagate (error is already in TLS)
            self.builder.switch_to_block(err_bb);
            self.builder.seal_block(err_bb);
            self.emit_default_return();

            // Continue to dispatch
            self.builder.switch_to_block(dispatch_bb);
            self.builder.seal_block(dispatch_bb);
        }

        // 5. Default check: if result == -1 and default exists → jump to default block
        let first_arm_check_bb = self.builder.create_block();

        if let Some(def) = default {
            let neg1 = self.builder.ins().iconst(types::I64, -1i64);
            let is_default = self.builder.ins().icmp(IntCC::Equal, result, neg1);
            let default_bb = self.builder.create_block();
            self.builder.ins().brif(is_default, default_bb, &[], first_arm_check_bb, &[]);

            // Default block
            self.builder.switch_to_block(default_bb);
            self.builder.seal_block(default_bb);
            let mut default_terminated = false;
            for s in &def.node.stmts {
                self.lower_stmt(&s.node, &mut default_terminated)?;
            }
            if !default_terminated {
                self.builder.ins().jump(merge_bb, &[]);
            }

            self.builder.switch_to_block(first_arm_check_bb);
            self.builder.seal_block(first_arm_check_bb);
        } else {
            // No default — fall through directly to arm dispatch
            self.builder.ins().jump(first_arm_check_bb, &[]);
            self.builder.switch_to_block(first_arm_check_bb);
            self.builder.seal_block(first_arm_check_bb);
        }

        // 6. Dispatch: sequential index checks like match codegen
        let mut all_terminated = true;
        for (i, arm) in arms.iter().enumerate() {
            let body_bb = self.builder.create_block();
            let next_bb = if i + 1 < arms.len() {
                self.builder.create_block()
            } else {
                merge_bb
            };

            let idx_val = self.builder.ins().iconst(types::I64, i as i64);
            let cmp = self.builder.ins().icmp(IntCC::Equal, result, idx_val);
            self.builder.ins().brif(cmp, body_bb, &[], next_bb, &[]);

            // Body block
            self.builder.switch_to_block(body_bb);
            self.builder.seal_block(body_bb);

            // For recv arms, bind the received value
            let mut prev_vars: Vec<(String, Option<Variable>, Option<PlutoType>)> = Vec::new();
            if let SelectOp::Recv { binding, channel } = &arm.op {
                let chan_type = infer_type_for_expr(&channel.node, self.env, &self.var_types);
                if let PlutoType::Receiver(elem_type) = &chan_type {
                    let val_offset = (2 * count as i32 + i as i32) * POINTER_SIZE;
                    let raw = self.builder.ins().load(types::I64, MemFlags::new(), buffer, Offset32::new(val_offset));
                    let val = from_array_slot(raw, elem_type, &mut self.builder);

                    let cl_type = pluto_to_cranelift(elem_type);
                    let var = Variable::from_u32(self.next_var);
                    self.next_var += 1;
                    self.builder.declare_var(var, cl_type);
                    self.builder.def_var(var, val);

                    prev_vars.push((
                        binding.node.clone(),
                        self.variables.get(&binding.node).cloned(),
                        self.var_types.get(&binding.node).cloned(),
                    ));
                    self.variables.insert(binding.node.clone(), var);
                    self.var_types.insert(binding.node.clone(), *elem_type.clone());
                }
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
                all_terminated = false;
            }

            // Switch to next check block (if not the last arm)
            if i + 1 < arms.len() {
                self.builder.switch_to_block(next_bb);
                self.builder.seal_block(next_bb);
            }
        }

        if all_terminated && default.is_none() {
            *terminated = true;
        }

        self.builder.switch_to_block(merge_bb);
        self.builder.seal_block(merge_bb);
        if *terminated {
            self.builder.ins().trap(cranelift_codegen::ir::TrapCode::user(1).unwrap());
        }
        Ok(())
    }

    // ── lower_expr dispatch ──────────────────────────────────────────────

    fn lower_expr(&mut self, expr: &Expr) -> Result<Value, CompileError> {
        match expr {
            Expr::IntLit(n) => Ok(self.builder.ins().iconst(types::I64, *n)),
            Expr::FloatLit(n) => Ok(self.builder.ins().f64const(*n)),
            Expr::BoolLit(b) => Ok(self.builder.ins().iconst(types::I8, if *b { 1 } else { 0 })),
            Expr::StringLit(s) => {
                let raw_ptr = self.create_data_str(s)?;
                let len_val = self.builder.ins().iconst(types::I64, s.len() as i64);
                Ok(self.call_runtime("__pluto_string_new", &[raw_ptr, len_val]))
            }
            Expr::StringInterp { parts } => self.lower_string_interp(parts),
            Expr::Ident(name) => {
                let var = self.variables.get(name).ok_or_else(|| {
                    CompileError::codegen(format!("undefined variable '{name}'"))
                })?;
                Ok(self.builder.use_var(*var))
            }
            Expr::BinOp { op, lhs, rhs } => self.lower_binop(op, lhs, rhs),
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
                    UnaryOp::BitNot => Ok(self.builder.ins().bnot(val)),
                }
            }
            Expr::Cast { expr: inner, target_type } => {
                let val = self.lower_expr(&inner.node)?;
                let source_type = infer_type_for_expr(&inner.node, self.env, &self.var_types);
                let target_type = resolve_type_expr_to_pluto(&target_type.node, self.env);
                match (source_type, target_type) {
                    (PlutoType::Int, PlutoType::Float) => Ok(self.builder.ins().fcvt_from_sint(types::F64, val)),
                    (PlutoType::Float, PlutoType::Int) => Ok(self.builder.ins().fcvt_to_sint_sat(types::I64, val)),
                    (PlutoType::Int, PlutoType::Bool) => {
                        let zero = self.builder.ins().iconst(types::I64, 0);
                        Ok(self.builder.ins().icmp(IntCC::NotEqual, val, zero))
                    }
                    (PlutoType::Bool, PlutoType::Int) => Ok(self.builder.ins().uextend(types::I64, val)),
                    (PlutoType::Int, PlutoType::Byte) => Ok(self.builder.ins().ireduce(types::I8, val)),
                    (PlutoType::Byte, PlutoType::Int) => Ok(self.builder.ins().uextend(types::I64, val)),
                    _ => Err(CompileError::codegen("invalid cast in lowered AST".to_string())),
                }
            }
            Expr::Call { name, args } => self.lower_call(name, args),
            Expr::StructLit { name, fields, .. } => self.lower_struct_lit(name, fields),
            Expr::ArrayLit { elements } => {
                let n = elements.len() as i64;
                let cap_val = self.builder.ins().iconst(types::I64, n);
                let handle = self.call_runtime("__pluto_array_new", &[cap_val]);

                let elem_type = infer_type_for_expr(&elements[0].node, self.env, &self.var_types);
                // Hoist func_ref before loop to avoid repeated HashMap lookups
                let func_ref_push = self.module.declare_func_in_func(self.runtime.get("__pluto_array_push"), self.builder.func);
                for elem in elements {
                    let val = self.lower_expr(&elem.node)?;
                    let slot = to_array_slot(val, &elem_type, &mut self.builder);
                    self.builder.ins().call(func_ref_push, &[handle, slot]);
                }

                Ok(handle)
            }
            Expr::MapLit { key_type, value_type, entries } => {
                let kt = resolve_type_expr_to_pluto(&key_type.node, self.env);
                let vt = resolve_type_expr_to_pluto(&value_type.node, self.env);
                let tag = self.builder.ins().iconst(types::I64, key_type_tag(&kt));
                let handle = self.call_runtime("__pluto_map_new", &[tag]);
                for (k_expr, v_expr) in entries {
                    let k_val = self.lower_expr(&k_expr.node)?;
                    let v_val = self.lower_expr(&v_expr.node)?;
                    let key_slot = to_array_slot(k_val, &kt, &mut self.builder);
                    let val_slot = to_array_slot(v_val, &vt, &mut self.builder);
                    self.call_runtime_void("__pluto_map_insert", &[handle, tag, key_slot, val_slot]);
                }
                Ok(handle)
            }
            Expr::SetLit { elem_type, elements } => {
                let et = resolve_type_expr_to_pluto(&elem_type.node, self.env);
                let tag = self.builder.ins().iconst(types::I64, key_type_tag(&et));
                let handle = self.call_runtime("__pluto_set_new", &[tag]);
                for elem in elements {
                    let val = self.lower_expr(&elem.node)?;
                    let slot = to_array_slot(val, &et, &mut self.builder);
                    self.call_runtime_void("__pluto_set_insert", &[handle, tag, slot]);
                }
                Ok(handle)
            }
            Expr::Index { object, index } => {
                let handle = self.lower_expr(&object.node)?;
                let idx = self.lower_expr(&index.node)?;
                let obj_type = infer_type_for_expr(&object.node, self.env, &self.var_types);
                if let PlutoType::Array(elem) = &obj_type {
                    let raw = self.call_runtime("__pluto_array_get", &[handle, idx]);
                    Ok(from_array_slot(raw, elem, &mut self.builder))
                } else if let PlutoType::Map(key_ty, val_ty) = &obj_type {
                    let tag = self.builder.ins().iconst(types::I64, key_type_tag(&key_ty));
                    let key_slot = to_array_slot(idx, &key_ty, &mut self.builder);
                    let raw = self.call_runtime("__pluto_map_get", &[handle, tag, key_slot]);
                    Ok(from_array_slot(raw, &val_ty, &mut self.builder))
                } else if obj_type == PlutoType::Bytes {
                    let raw = self.call_runtime("__pluto_bytes_get", &[handle, idx]);
                    Ok(self.builder.ins().ireduce(types::I8, raw))
                } else if obj_type == PlutoType::String {
                    Ok(self.call_runtime("__pluto_string_char_at", &[handle, idx]))
                } else {
                    Err(CompileError::codegen(format!("index on non-indexable type {obj_type}")))
                }
            }
            Expr::EnumUnit { enum_name, variant, .. } => {
                let enum_info = self.env.enums.get(&enum_name.node).ok_or_else(|| {
                    CompileError::codegen(format!("unknown enum '{}'", enum_name.node))
                })?;
                let max_fields = enum_info.variants.iter().map(|(_, f)| f.len()).max().unwrap_or(0);
                let alloc_size = (1 + max_fields) as i64 * POINTER_SIZE as i64;
                let variant_idx = enum_info.variants.iter().position(|(n, _)| *n == variant.node).unwrap();

                let size_val = self.builder.ins().iconst(types::I64, alloc_size);
                let ptr = self.call_runtime("__pluto_alloc", &[size_val]);

                let tag_val = self.builder.ins().iconst(types::I64, variant_idx as i64);
                self.builder.ins().store(MemFlags::new(), tag_val, ptr, Offset32::new(0));

                Ok(ptr)
            }
            Expr::EnumData { enum_name, variant, fields, .. } => {
                self.lower_enum_data(enum_name, variant, fields)
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
                } else if obj_type == PlutoType::Error && field.node == "message" {
                    Ok(self.builder.ins().load(types::I64, MemFlags::new(), ptr, Offset32::new(0)))
                } else {
                    Err(CompileError::codegen(format!("field access on non-class type {obj_type}")))
                }
            }
            Expr::Propagate { expr: inner } => {
                // Lower the inner call
                let val = self.lower_expr(&inner.node)?;

                // Check TLS error state
                let has_err = self.call_runtime("__pluto_has_error", &[]);
                let zero = self.builder.ins().iconst(types::I64, 0);
                let is_error = self.builder.ins().icmp(IntCC::NotEqual, has_err, zero);

                let propagate_bb = self.builder.create_block();
                let continue_bb = self.builder.create_block();
                self.builder.ins().brif(is_error, propagate_bb, &[], continue_bb, &[]);

                // Propagate block: return default (error stays in TLS for caller)
                self.builder.switch_to_block(propagate_bb);
                self.builder.seal_block(propagate_bb);
                if self.is_main {
                    // In main: print the uncaught error to stderr and return 1
                    self.call_runtime_void("__pluto_print_uncaught_error", &[]);
                    let one = self.builder.ins().iconst(types::I64, 1);
                    if let Some(exit_bb) = self.exit_block {
                        self.builder.ins().jump(exit_bb, &[one]);
                    } else {
                        self.builder.ins().return_(&[one]);
                    }
                } else {
                    self.emit_default_return();
                }

                // Continue block: no error, use the call result
                self.builder.switch_to_block(continue_bb);
                self.builder.seal_block(continue_bb);
                Ok(val)
            }
            Expr::Catch { expr: inner, handler } => self.lower_catch(inner, handler),
            Expr::MethodCall { object, method, args } => {
                self.lower_method_call(object, method, args)
            }
            Expr::Closure { .. } => {
                Err(CompileError::codegen("closures should be lifted before codegen"))
            }
            Expr::ClosureCreate { fn_name, captures } => {
                self.lower_closure_create(fn_name, captures)
            }
            Expr::Spawn { call } => {
                match &call.node {
                    Expr::ClosureCreate { fn_name, captures } => {
                        let closure_ptr = self.lower_closure_create(fn_name, captures)?;
                        // Inc refcount for each captured Sender
                        for cap_name in captures {
                            if let Some(PlutoType::Sender(_)) = self.var_types.get(cap_name) {
                                let var = self.variables.get(cap_name).unwrap();
                                let val = self.builder.use_var(*var);
                                self.call_runtime_void("__pluto_chan_sender_inc", &[val]);
                            }
                        }
                        Ok(self.call_runtime("__pluto_task_spawn", &[closure_ptr]))
                    }
                    _ => Err(CompileError::codegen("spawn should contain ClosureCreate after lifting"))
                }
            }
            Expr::Range { .. } => {
                Err(CompileError::codegen("range expressions can only be used as for loop iterables".to_string()))
            }
        }
    }

    // ── lower_expr extracted helpers ─────────────────────────────────────

    fn lower_string_interp(&mut self, parts: &[StringInterpPart]) -> Result<Value, CompileError> {
        // Convert each part to a string handle, then concat them all
        let mut string_vals: Vec<Value> = Vec::new();
        for part in parts {
            match part {
                StringInterpPart::Lit(s) => {
                    let raw_ptr = self.create_data_str(s)?;
                    let len_val = self.builder.ins().iconst(types::I64, s.len() as i64);
                    string_vals.push(self.call_runtime("__pluto_string_new", &[raw_ptr, len_val]));
                }
                StringInterpPart::Expr(e) => {
                    let val = self.lower_expr(&e.node)?;
                    let t = infer_type_for_expr(&e.node, self.env, &self.var_types);
                    let str_val = match t {
                        PlutoType::String => val,
                        PlutoType::Int => self.call_runtime("__pluto_int_to_string", &[val]),
                        PlutoType::Float => self.call_runtime("__pluto_float_to_string", &[val]),
                        PlutoType::Bool => {
                            let widened = self.builder.ins().uextend(types::I32, val);
                            self.call_runtime("__pluto_bool_to_string", &[widened])
                        }
                        PlutoType::Byte => {
                            let widened = self.builder.ins().uextend(types::I64, val);
                            self.call_runtime("__pluto_int_to_string", &[widened])
                        }
                        _ => return Err(CompileError::codegen(format!("cannot interpolate {t}"))),
                    };
                    string_vals.push(str_val);
                }
            }
        }
        // Concat all parts left to right — hoist func_ref before loop
        let mut result = string_vals[0];
        let concat_ref = self.module.declare_func_in_func(self.runtime.get("__pluto_string_concat"), self.builder.func);
        for part_val in &string_vals[1..] {
            let call = self.builder.ins().call(concat_ref, &[result, *part_val]);
            result = self.builder.inst_results(call)[0];
        }
        Ok(result)
    }

    fn lower_binop(
        &mut self,
        op: &BinOp,
        lhs: &crate::span::Spanned<Expr>,
        rhs: &crate::span::Spanned<Expr>,
    ) -> Result<Value, CompileError> {
        let l = self.lower_expr(&lhs.node)?;
        let r = self.lower_expr(&rhs.node)?;

        let lhs_type = infer_type_for_expr(&lhs.node, self.env, &self.var_types);
        let is_float = lhs_type == PlutoType::Float;
        let is_string = lhs_type == PlutoType::String;
        let is_byte = lhs_type == PlutoType::Byte;

        let result = match op {
            BinOp::Add if is_string => self.call_runtime("__pluto_string_concat", &[l, r]),
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
                let i32_result = self.call_runtime("__pluto_string_eq", &[l, r]);
                self.builder.ins().ireduce(types::I8, i32_result)
            }
            BinOp::Eq if is_float => self.builder.ins().fcmp(FloatCC::Equal, l, r),
            BinOp::Eq => self.builder.ins().icmp(IntCC::Equal, l, r),
            BinOp::Neq if is_string => {
                let i32_result = self.call_runtime("__pluto_string_eq", &[l, r]);
                let i8_result = self.builder.ins().ireduce(types::I8, i32_result);
                let one = self.builder.ins().iconst(types::I8, 1);
                self.builder.ins().bxor(i8_result, one)
            }
            BinOp::Neq if is_float => self.builder.ins().fcmp(FloatCC::NotEqual, l, r),
            BinOp::Neq => self.builder.ins().icmp(IntCC::NotEqual, l, r),
            BinOp::Lt if is_float => self.builder.ins().fcmp(FloatCC::LessThan, l, r),
            BinOp::Lt if is_byte => self.builder.ins().icmp(IntCC::UnsignedLessThan, l, r),
            BinOp::Lt => self.builder.ins().icmp(IntCC::SignedLessThan, l, r),
            BinOp::Gt if is_float => self.builder.ins().fcmp(FloatCC::GreaterThan, l, r),
            BinOp::Gt if is_byte => self.builder.ins().icmp(IntCC::UnsignedGreaterThan, l, r),
            BinOp::Gt => self.builder.ins().icmp(IntCC::SignedGreaterThan, l, r),
            BinOp::LtEq if is_float => self.builder.ins().fcmp(FloatCC::LessThanOrEqual, l, r),
            BinOp::LtEq if is_byte => self.builder.ins().icmp(IntCC::UnsignedLessThanOrEqual, l, r),
            BinOp::LtEq => self.builder.ins().icmp(IntCC::SignedLessThanOrEqual, l, r),
            BinOp::GtEq if is_float => self.builder.ins().fcmp(FloatCC::GreaterThanOrEqual, l, r),
            BinOp::GtEq if is_byte => self.builder.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, l, r),
            BinOp::GtEq => self.builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, l, r),
            BinOp::And => self.builder.ins().band(l, r),
            BinOp::Or => self.builder.ins().bor(l, r),
            BinOp::BitAnd => self.builder.ins().band(l, r),
            BinOp::BitOr => self.builder.ins().bor(l, r),
            BinOp::BitXor => self.builder.ins().bxor(l, r),
            BinOp::Shl => self.builder.ins().ishl(l, r),
            BinOp::Shr => self.builder.ins().sshr(l, r),
        };
        Ok(result)
    }

    fn lower_call(
        &mut self,
        name: &crate::span::Spanned<String>,
        args: &[crate::span::Spanned<Expr>],
    ) -> Result<Value, CompileError> {
        if name.node == "expect" {
            // Passthrough — just return the lowered arg
            return self.lower_expr(&args[0].node);
        }
        if name.node == "print" {
            return self.lower_print(args);
        }
        if name.node == "time_ns" {
            return Ok(self.call_runtime("__pluto_time_ns", &[]));
        }
        if name.node == "abs" {
            let arg = self.lower_expr(&args[0].node)?;
            let arg_ty = infer_type_for_expr(&args[0].node, self.env, &self.var_types);
            return Ok(match arg_ty {
                PlutoType::Int => self.call_runtime("__pluto_abs_int", &[arg]),
                PlutoType::Float => self.call_runtime("__pluto_abs_float", &[arg]),
                _ => return Err(CompileError::codegen("invalid abs() argument type in lowered AST".to_string())),
            });
        }
        if name.node == "min" {
            let a = self.lower_expr(&args[0].node)?;
            let b = self.lower_expr(&args[1].node)?;
            let arg_ty = infer_type_for_expr(&args[0].node, self.env, &self.var_types);
            return Ok(match arg_ty {
                PlutoType::Int => self.call_runtime("__pluto_min_int", &[a, b]),
                PlutoType::Float => self.call_runtime("__pluto_min_float", &[a, b]),
                _ => return Err(CompileError::codegen("invalid min() argument type in lowered AST".to_string())),
            });
        }
        if name.node == "max" {
            let a = self.lower_expr(&args[0].node)?;
            let b = self.lower_expr(&args[1].node)?;
            let arg_ty = infer_type_for_expr(&args[0].node, self.env, &self.var_types);
            return Ok(match arg_ty {
                PlutoType::Int => self.call_runtime("__pluto_max_int", &[a, b]),
                PlutoType::Float => self.call_runtime("__pluto_max_float", &[a, b]),
                _ => return Err(CompileError::codegen("invalid max() argument type in lowered AST".to_string())),
            });
        }
        if name.node == "pow" {
            let base = self.lower_expr(&args[0].node)?;
            let exp = self.lower_expr(&args[1].node)?;
            let arg_ty = infer_type_for_expr(&args[0].node, self.env, &self.var_types);
            return Ok(match arg_ty {
                PlutoType::Int => self.call_runtime("__pluto_pow_int", &[base, exp]),
                PlutoType::Float => self.call_runtime("__pluto_pow_float", &[base, exp]),
                _ => return Err(CompileError::codegen("invalid pow() argument type in lowered AST".to_string())),
            });
        }
        if name.node == "sqrt" {
            let arg = self.lower_expr(&args[0].node)?;
            return Ok(self.call_runtime("__pluto_sqrt", &[arg]));
        }
        if name.node == "floor" {
            let arg = self.lower_expr(&args[0].node)?;
            return Ok(self.call_runtime("__pluto_floor", &[arg]));
        }
        if name.node == "ceil" {
            let arg = self.lower_expr(&args[0].node)?;
            return Ok(self.call_runtime("__pluto_ceil", &[arg]));
        }
        if name.node == "round" {
            let arg = self.lower_expr(&args[0].node)?;
            return Ok(self.call_runtime("__pluto_round", &[arg]));
        }
        if name.node == "sin" {
            let arg = self.lower_expr(&args[0].node)?;
            return Ok(self.call_runtime("__pluto_sin", &[arg]));
        }
        if name.node == "cos" {
            let arg = self.lower_expr(&args[0].node)?;
            return Ok(self.call_runtime("__pluto_cos", &[arg]));
        }
        if name.node == "tan" {
            let arg = self.lower_expr(&args[0].node)?;
            return Ok(self.call_runtime("__pluto_tan", &[arg]));
        }
        if name.node == "log" {
            let arg = self.lower_expr(&args[0].node)?;
            return Ok(self.call_runtime("__pluto_log", &[arg]));
        }
        if name.node == "gc_heap_size" {
            return Ok(self.call_runtime("__pluto_gc_heap_size", &[]));
        }
        if name.node == "bytes_new" {
            return Ok(self.call_runtime("__pluto_bytes_new", &[]));
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

    fn lower_struct_lit(
        &mut self,
        name: &crate::span::Spanned<String>,
        fields: &[(crate::span::Spanned<String>, crate::span::Spanned<Expr>)],
    ) -> Result<Value, CompileError> {
        let class_info = self.env.classes.get(&name.node).ok_or_else(|| {
            CompileError::codegen(format!("unknown class '{}'", name.node))
        })?;
        let num_fields = class_info.fields.len() as i64;
        let size = num_fields * POINTER_SIZE as i64;

        let size_val = self.builder.ins().iconst(types::I64, size);
        let ptr = self.call_runtime("__pluto_alloc", &[size_val]);

        // Clone field info to avoid borrow conflict with self.lower_expr
        let field_info: Vec<(String, PlutoType, bool)> = class_info.fields.clone();

        for (lit_name, lit_val) in fields {
            let val = self.lower_expr(&lit_val.node)?;
            let offset = field_info.iter()
                .position(|(n, _, _)| *n == lit_name.node)
                .ok_or_else(|| CompileError::codegen(format!("unknown field '{}' on class '{}'", lit_name.node, name.node)))? as i32 * POINTER_SIZE;
            self.builder.ins().store(MemFlags::new(), val, ptr, Offset32::new(offset));
        }

        // Emit invariant checks after struct construction
        self.emit_invariant_checks(&name.node, ptr)?;

        Ok(ptr)
    }

    fn lower_enum_data(
        &mut self,
        enum_name: &crate::span::Spanned<String>,
        variant: &crate::span::Spanned<String>,
        fields: &[(crate::span::Spanned<String>, crate::span::Spanned<Expr>)],
    ) -> Result<Value, CompileError> {
        let enum_info = self.env.enums.get(&enum_name.node).ok_or_else(|| {
            CompileError::codegen(format!("unknown enum '{}'", enum_name.node))
        })?.clone();
        let max_fields = enum_info.variants.iter().map(|(_, f)| f.len()).max().unwrap_or(0);
        let alloc_size = (1 + max_fields) as i64 * POINTER_SIZE as i64;
        let variant_idx = enum_info.variants.iter().position(|(n, _)| *n == variant.node).unwrap();
        let variant_fields = &enum_info.variants[variant_idx].1;

        let size_val = self.builder.ins().iconst(types::I64, alloc_size);
        let ptr = self.call_runtime("__pluto_alloc", &[size_val]);

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

    fn lower_catch(
        &mut self,
        inner: &crate::span::Spanned<Expr>,
        handler: &CatchHandler,
    ) -> Result<Value, CompileError> {
        // Lower the inner call
        let val = self.lower_expr(&inner.node)?;
        let val_type = infer_type_for_expr(&inner.node, self.env, &self.var_types);
        let cl_type = pluto_to_cranelift(&val_type);

        // Check TLS error state
        let has_err = self.call_runtime("__pluto_has_error", &[]);
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
                let err_obj = self.call_runtime("__pluto_get_error", &[]);

                // Clear the error
                self.call_runtime_void("__pluto_clear_error", &[]);

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
                self.call_runtime_void("__pluto_clear_error", &[]);

                self.lower_expr(&fallback.node)?
            }
        };

        self.builder.ins().jump(merge_bb, &[handler_val]);

        // Merge block: result is the block parameter
        self.builder.switch_to_block(merge_bb);
        self.builder.seal_block(merge_bb);
        Ok(self.builder.block_params(merge_bb)[0])
    }

    fn lower_method_call(
        &mut self,
        object: &crate::span::Spanned<Expr>,
        method: &crate::span::Spanned<String>,
        args: &[crate::span::Spanned<Expr>],
    ) -> Result<Value, CompileError> {
        // Check for expect() intrinsic pattern
        if let Expr::Call { name, args: expect_args, .. } = &object.node {
            if name.node == "expect" && expect_args.len() == 1 {
                let actual_val = self.lower_expr(&expect_args[0].node)?;
                let inner_type = infer_type_for_expr(&expect_args[0].node, self.env, &self.var_types);
                let line_no = byte_to_line(self.source, object.span.start);
                let line_val = self.builder.ins().iconst(types::I64, line_no as i64);

                match method.node.as_str() {
                    "to_equal" => {
                        let expected_val = self.lower_expr(&args[0].node)?;
                        match inner_type {
                            PlutoType::Int => self.call_runtime_void("__pluto_expect_equal_int", &[actual_val, expected_val, line_val]),
                            PlutoType::Float => self.call_runtime_void("__pluto_expect_equal_float", &[actual_val, expected_val, line_val]),
                            PlutoType::Bool => {
                                let a = self.builder.ins().uextend(types::I64, actual_val);
                                let e = self.builder.ins().uextend(types::I64, expected_val);
                                self.call_runtime_void("__pluto_expect_equal_bool", &[a, e, line_val]);
                            }
                            PlutoType::String => self.call_runtime_void("__pluto_expect_equal_string", &[actual_val, expected_val, line_val]),
                            PlutoType::Byte => {
                                let a = self.builder.ins().uextend(types::I64, actual_val);
                                let e = self.builder.ins().uextend(types::I64, expected_val);
                                self.call_runtime_void("__pluto_expect_equal_int", &[a, e, line_val]);
                            }
                            _ => return Err(CompileError::codegen(format!("to_equal not supported for {inner_type}"))),
                        }
                    }
                    "to_be_true" => {
                        let a = self.builder.ins().uextend(types::I64, actual_val);
                        self.call_runtime_void("__pluto_expect_true", &[a, line_val]);
                    }
                    "to_be_false" => {
                        let a = self.builder.ins().uextend(types::I64, actual_val);
                        self.call_runtime_void("__pluto_expect_false", &[a, line_val]);
                    }
                    _ => return Err(CompileError::codegen(format!("unknown assertion method: {}", method.node))),
                }
                return Ok(self.builder.ins().iconst(types::I64, 0)); // void
            }
        }

        let obj_ptr = self.lower_expr(&object.node)?;
        let obj_type = infer_type_for_expr(&object.node, self.env, &self.var_types);

        // Task methods
        if let PlutoType::Task(inner) = &obj_type {
            match method.node.as_str() {
                "get" => {
                    let raw = self.call_runtime("__pluto_task_get", &[obj_ptr]);
                    return Ok(from_array_slot(raw, &inner, &mut self.builder));
                }
                _ => return Err(CompileError::codegen(format!("Task has no method '{}'", method.node)))
            }
        }

        // Sender methods
        if let PlutoType::Sender(inner) = &obj_type {
            match method.node.as_str() {
                "send" => {
                    let inner = inner.clone();
                    let arg_val = self.lower_expr(&args[0].node)?;
                    let slot = to_array_slot(arg_val, &inner, &mut self.builder);
                    self.call_runtime("__pluto_chan_send", &[obj_ptr, slot]);
                    return Ok(self.builder.ins().iconst(types::I64, 0));
                }
                "try_send" => {
                    let inner = inner.clone();
                    let arg_val = self.lower_expr(&args[0].node)?;
                    let slot = to_array_slot(arg_val, &inner, &mut self.builder);
                    self.call_runtime("__pluto_chan_try_send", &[obj_ptr, slot]);
                    return Ok(self.builder.ins().iconst(types::I64, 0));
                }
                "close" => {
                    self.call_runtime_void("__pluto_chan_sender_dec", &[obj_ptr]);
                    return Ok(self.builder.ins().iconst(types::I64, 0));
                }
                _ => return Err(CompileError::codegen(format!("Sender has no method '{}'", method.node)))
            }
        }

        // Receiver methods
        if let PlutoType::Receiver(inner) = &obj_type {
            match method.node.as_str() {
                "recv" => {
                    let raw = self.call_runtime("__pluto_chan_recv", &[obj_ptr]);
                    return Ok(from_array_slot(raw, &inner, &mut self.builder));
                }
                "try_recv" => {
                    let raw = self.call_runtime("__pluto_chan_try_recv", &[obj_ptr]);
                    return Ok(from_array_slot(raw, &inner, &mut self.builder));
                }
                _ => return Err(CompileError::codegen(format!("Receiver has no method '{}'", method.node)))
            }
        }

        // Array methods
        if let PlutoType::Array(elem) = &obj_type {
            match method.node.as_str() {
                "len" => {
                    return Ok(self.call_runtime("__pluto_array_len", &[obj_ptr]));
                }
                "push" => {
                    let elem = elem.clone();
                    let arg_val = self.lower_expr(&args[0].node)?;
                    let slot = to_array_slot(arg_val, &elem, &mut self.builder);
                    self.call_runtime_void("__pluto_array_push", &[obj_ptr, slot]);
                    return Ok(self.builder.ins().iconst(types::I64, 0));
                }
                _ => {
                    return Err(CompileError::codegen(format!("array has no method '{}'", method.node)));
                }
            }
        }

        // Map methods
        if let PlutoType::Map(key_ty, val_ty) = &obj_type {
            let tag = self.builder.ins().iconst(types::I64, key_type_tag(key_ty));
            match method.node.as_str() {
                "len" => return Ok(self.call_runtime("__pluto_map_len", &[obj_ptr])),
                "contains" => {
                    let k = self.lower_expr(&args[0].node)?;
                    let key_slot = to_array_slot(k, key_ty, &mut self.builder);
                    let result = self.call_runtime("__pluto_map_contains", &[obj_ptr, tag, key_slot]);
                    return Ok(self.builder.ins().ireduce(types::I8, result));
                }
                "insert" => {
                    let k = self.lower_expr(&args[0].node)?;
                    let v = self.lower_expr(&args[1].node)?;
                    let key_slot = to_array_slot(k, key_ty, &mut self.builder);
                    let val_slot = to_array_slot(v, val_ty, &mut self.builder);
                    self.call_runtime_void("__pluto_map_insert", &[obj_ptr, tag, key_slot, val_slot]);
                    return Ok(self.builder.ins().iconst(types::I64, 0));
                }
                "remove" => {
                    let k = self.lower_expr(&args[0].node)?;
                    let key_slot = to_array_slot(k, key_ty, &mut self.builder);
                    self.call_runtime_void("__pluto_map_remove", &[obj_ptr, tag, key_slot]);
                    return Ok(self.builder.ins().iconst(types::I64, 0));
                }
                "keys" => return Ok(self.call_runtime("__pluto_map_keys", &[obj_ptr])),
                "values" => return Ok(self.call_runtime("__pluto_map_values", &[obj_ptr])),
                _ => return Err(CompileError::codegen(format!("Map has no method '{}'", method.node))),
            }
        }

        // Set methods
        if let PlutoType::Set(elem_ty) = &obj_type {
            let tag = self.builder.ins().iconst(types::I64, key_type_tag(elem_ty));
            match method.node.as_str() {
                "len" => return Ok(self.call_runtime("__pluto_set_len", &[obj_ptr])),
                "contains" => {
                    let e = self.lower_expr(&args[0].node)?;
                    let slot = to_array_slot(e, elem_ty, &mut self.builder);
                    let result = self.call_runtime("__pluto_set_contains", &[obj_ptr, tag, slot]);
                    return Ok(self.builder.ins().ireduce(types::I8, result));
                }
                "insert" => {
                    let e = self.lower_expr(&args[0].node)?;
                    let slot = to_array_slot(e, elem_ty, &mut self.builder);
                    self.call_runtime_void("__pluto_set_insert", &[obj_ptr, tag, slot]);
                    return Ok(self.builder.ins().iconst(types::I64, 0));
                }
                "remove" => {
                    let e = self.lower_expr(&args[0].node)?;
                    let slot = to_array_slot(e, elem_ty, &mut self.builder);
                    self.call_runtime_void("__pluto_set_remove", &[obj_ptr, tag, slot]);
                    return Ok(self.builder.ins().iconst(types::I64, 0));
                }
                "to_array" => return Ok(self.call_runtime("__pluto_set_to_array", &[obj_ptr])),
                _ => return Err(CompileError::codegen(format!("Set has no method '{}'", method.node))),
            }
        }

        // Bytes methods
        if obj_type == PlutoType::Bytes {
            return match method.node.as_str() {
                "len" => Ok(self.call_runtime("__pluto_bytes_len", &[obj_ptr])),
                "push" => {
                    let arg_val = self.lower_expr(&args[0].node)?;
                    let widened = self.builder.ins().uextend(types::I64, arg_val);
                    self.call_runtime_void("__pluto_bytes_push", &[obj_ptr, widened]);
                    Ok(self.builder.ins().iconst(types::I64, 0))
                }
                "to_string" => Ok(self.call_runtime("__pluto_bytes_to_string", &[obj_ptr])),
                _ => Err(CompileError::codegen(format!("bytes has no method '{}'", method.node))),
            };
        }

        // String methods
        if obj_type == PlutoType::String {
            return match method.node.as_str() {
                "len" => Ok(self.call_runtime("__pluto_string_len", &[obj_ptr])),
                "contains" => {
                    let arg = self.lower_expr(&args[0].node)?;
                    let result = self.call_runtime("__pluto_string_contains", &[obj_ptr, arg]);
                    Ok(self.builder.ins().ireduce(types::I8, result))
                }
                "starts_with" => {
                    let arg = self.lower_expr(&args[0].node)?;
                    let result = self.call_runtime("__pluto_string_starts_with", &[obj_ptr, arg]);
                    Ok(self.builder.ins().ireduce(types::I8, result))
                }
                "ends_with" => {
                    let arg = self.lower_expr(&args[0].node)?;
                    let result = self.call_runtime("__pluto_string_ends_with", &[obj_ptr, arg]);
                    Ok(self.builder.ins().ireduce(types::I8, result))
                }
                "index_of" => {
                    let arg = self.lower_expr(&args[0].node)?;
                    Ok(self.call_runtime("__pluto_string_index_of", &[obj_ptr, arg]))
                }
                "substring" => {
                    let start = self.lower_expr(&args[0].node)?;
                    let len = self.lower_expr(&args[1].node)?;
                    Ok(self.call_runtime("__pluto_string_substring", &[obj_ptr, start, len]))
                }
                "trim" => Ok(self.call_runtime("__pluto_string_trim", &[obj_ptr])),
                "to_upper" => Ok(self.call_runtime("__pluto_string_to_upper", &[obj_ptr])),
                "to_lower" => Ok(self.call_runtime("__pluto_string_to_lower", &[obj_ptr])),
                "replace" => {
                    let old = self.lower_expr(&args[0].node)?;
                    let new = self.lower_expr(&args[1].node)?;
                    Ok(self.call_runtime("__pluto_string_replace", &[obj_ptr, old, new]))
                }
                "split" => {
                    let delim = self.lower_expr(&args[0].node)?;
                    Ok(self.call_runtime("__pluto_string_split", &[obj_ptr, delim]))
                }
                "char_at" => {
                    let idx = self.lower_expr(&args[0].node)?;
                    Ok(self.call_runtime("__pluto_string_char_at", &[obj_ptr, idx]))
                }
                "to_bytes" => Ok(self.call_runtime("__pluto_string_to_bytes", &[obj_ptr])),
                _ => Err(CompileError::codegen(format!("string has no method '{}'", method.node))),
            };
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
            let class_name = class_name.clone();
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
            let result = if results.is_empty() {
                self.builder.ins().iconst(types::I64, 0)
            } else {
                results[0]
            };

            // Emit invariant checks after method call (conservative: all methods)
            self.emit_invariant_checks(&class_name, obj_ptr)?;

            Ok(result)
        } else {
            Err(CompileError::codegen(format!("method call on non-class type {obj_type}")))
        }
    }

    fn lower_closure_create(
        &mut self,
        fn_name: &str,
        captures: &[String],
    ) -> Result<Value, CompileError> {
        // 1. Look up the function ID for the lifted closure function
        let func_id = self.func_ids.get(fn_name).ok_or_else(|| {
            CompileError::codegen(format!("undefined closure function '{}'", fn_name))
        })?;

        // 2. Allocate closure object: [fn_ptr: i64] [capture_0: i64] ...
        let obj_size = (1 + captures.len()) as i64 * POINTER_SIZE as i64;
        let size_val = self.builder.ins().iconst(types::I64, obj_size);
        let closure_ptr = self.call_runtime("__pluto_alloc", &[size_val]);

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

    fn lower_print(
        &mut self,
        args: &[crate::span::Spanned<Expr>],
    ) -> Result<Value, CompileError> {
        let arg = &args[0];
        let arg_type = infer_type_for_expr(&arg.node, self.env, &self.var_types);
        let arg_val = self.lower_expr(&arg.node)?;

        match arg_type {
            PlutoType::Int => {
                self.call_runtime_void("__pluto_print_int", &[arg_val]);
            }
            PlutoType::Float => {
                self.call_runtime_void("__pluto_print_float", &[arg_val]);
            }
            PlutoType::String => {
                self.call_runtime_void("__pluto_print_string", &[arg_val]);
            }
            PlutoType::Bool => {
                // Widen I8 bool to I32 for the C function
                let widened = self.builder.ins().uextend(types::I32, arg_val);
                self.call_runtime_void("__pluto_print_bool", &[widened]);
            }
            PlutoType::Byte => {
                // Widen I8 byte to I64 and print as int
                let widened = self.builder.ins().uextend(types::I64, arg_val);
                self.call_runtime_void("__pluto_print_int", &[widened]);
            }
            PlutoType::Void | PlutoType::Class(_) | PlutoType::Array(_) | PlutoType::Trait(_) | PlutoType::Enum(_) | PlutoType::Fn(_, _) | PlutoType::Map(_, _) | PlutoType::Set(_) | PlutoType::Task(_) | PlutoType::Sender(_) | PlutoType::Receiver(_) | PlutoType::Range | PlutoType::Error | PlutoType::TypeParam(_) | PlutoType::Bytes => {
                return Err(CompileError::codegen(format!("cannot print {arg_type}")));
            }
        }

        // print returns void, so return a dummy value
        Ok(self.builder.ins().iconst(types::I64, 0))
    }
}

/// Collect sender variable names from LetChan statements in a function body.
fn collect_sender_var_names(stmts: &[crate::span::Spanned<Stmt>]) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    for stmt in stmts {
        collect_sender_var_names_stmt(&stmt.node, &mut names, &mut seen);
    }
    names
}

fn collect_sender_var_names_stmt(stmt: &Stmt, names: &mut Vec<String>, seen: &mut HashSet<String>) {
    match stmt {
        Stmt::LetChan { sender, .. } => {
            if seen.insert(sender.node.clone()) {
                names.push(sender.node.clone());
            }
        }
        Stmt::If { then_block, else_block, .. } => {
            for s in &then_block.node.stmts { collect_sender_var_names_stmt(&s.node, names, seen); }
            if let Some(eb) = else_block {
                for s in &eb.node.stmts { collect_sender_var_names_stmt(&s.node, names, seen); }
            }
        }
        Stmt::While { body, .. } | Stmt::For { body, .. } => {
            for s in &body.node.stmts { collect_sender_var_names_stmt(&s.node, names, seen); }
        }
        Stmt::Match { arms, .. } => {
            for arm in arms {
                for s in &arm.body.node.stmts { collect_sender_var_names_stmt(&s.node, names, seen); }
            }
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                for s in &arm.body.node.stmts { collect_sender_var_names_stmt(&s.node, names, seen); }
            }
            if let Some(def) = default {
                for s in &def.node.stmts { collect_sender_var_names_stmt(&s.node, names, seen); }
            }
        }
        _ => {}
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
    source: &str,
    spawn_closure_fns: &HashSet<String>,
    class_invariants: &HashMap<String, Vec<(Expr, String)>>,
) -> Result<(), CompileError> {
    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);
    builder.seal_block(entry_block);

    let mut variables = HashMap::new();
    let mut var_types = HashMap::new();
    let mut next_var = 0u32;
    let mut sender_cleanup_vars: Vec<Variable> = Vec::new();

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

        // For spawn closure functions, register captured Sender vars for cleanup
        if spawn_closure_fns.contains(&func.name.node) {
            for (cap_name, cap_type) in captures.iter() {
                if matches!(cap_type, PlutoType::Sender(_)) {
                    if let Some(&var) = variables.get(cap_name) {
                        sender_cleanup_vars.push(var);
                    }
                }
            }
        }
    }

    // Pre-scan body for LetChan senders and pre-declare cleanup variables
    let sender_names = collect_sender_var_names(&func.body.node.stmts);
    let null_val = builder.ins().iconst(types::I64, 0);
    for name in &sender_names {
        let var = Variable::from_u32(next_var);
        next_var += 1;
        builder.declare_var(var, types::I64);
        builder.def_var(var, null_val);
        variables.insert(name.clone(), var);
        var_types.insert(name.clone(), PlutoType::Sender(Box::new(PlutoType::Void)));
        sender_cleanup_vars.push(var);
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

    // Create exit block if we have sender cleanup vars
    let exit_block = if !sender_cleanup_vars.is_empty() {
        let exit_bb = builder.create_block();
        // Add return value as block param if function returns non-void
        let is_void_return = matches!(&expected_return_type, Some(PlutoType::Void) | None);
        if !is_void_return {
            let ret_cl_type = pluto_to_cranelift(expected_return_type.as_ref().unwrap());
            builder.append_block_param(exit_bb, ret_cl_type);
        }
        Some(exit_bb)
    } else {
        None
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
        source,
        class_invariants,
        variables,
        var_types,
        next_var,
        expected_return_type,
        loop_stack: Vec::new(),
        sender_cleanup_vars,
        exit_block,
        is_main,
    };

    // Initialize GC at start of non-app main
    if is_main {
        ctx.call_runtime_void("__pluto_gc_init", &[]);
    }

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
        if let Some(exit_bb) = ctx.exit_block {
            ctx.builder.ins().jump(exit_bb, &[zero]);
        } else {
            ctx.builder.ins().return_(&[zero]);
        }
    } else if !terminated {
        // Void function with no return
        let lookup_name = if let Some(cn) = class_name {
            format!("{}_{}", cn, func.name.node)
        } else {
            func.name.node.clone()
        };
        let ret_type = ctx.env.functions.get(&lookup_name).map(|s| &s.return_type);
        if ret_type == Some(&PlutoType::Void) {
            if let Some(exit_bb) = ctx.exit_block {
                ctx.builder.ins().jump(exit_bb, &[]);
            } else {
                ctx.builder.ins().return_(&[]);
            }
        }
    }

    // Emit exit block: sender cleanup + actual return
    if let Some(exit_bb) = ctx.exit_block {
        ctx.builder.switch_to_block(exit_bb);
        ctx.builder.seal_block(exit_bb);

        // Call sender_dec for each cleanup variable
        let dec_ref = ctx.module.declare_func_in_func(ctx.runtime.get("__pluto_chan_sender_dec"), ctx.builder.func);
        for &var in &ctx.sender_cleanup_vars {
            let val = ctx.builder.use_var(var);
            ctx.builder.ins().call(dec_ref, &[val]);
        }

        // Emit actual return
        let is_void_return = matches!(&ctx.expected_return_type, Some(PlutoType::Void) | None);
        if !is_void_return {
            let ret_val = ctx.builder.block_params(exit_bb)[0];
            ctx.builder.ins().return_(&[ret_val]);
        } else {
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
            "byte" => PlutoType::Byte,
            "bytes" => PlutoType::Bytes,
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
        TypeExpr::Generic { name, type_args } => {
            if name == "Map" && type_args.len() == 2 {
                let k = resolve_type_expr_to_pluto(&type_args[0].node, env);
                let v = resolve_type_expr_to_pluto(&type_args[1].node, env);
                PlutoType::Map(Box::new(k), Box::new(v))
            } else if name == "Set" && type_args.len() == 1 {
                let t = resolve_type_expr_to_pluto(&type_args[0].node, env);
                PlutoType::Set(Box::new(t))
            } else if name == "Task" && type_args.len() == 1 {
                let t = resolve_type_expr_to_pluto(&type_args[0].node, env);
                PlutoType::Task(Box::new(t))
            } else if name == "Sender" && type_args.len() == 1 {
                let t = resolve_type_expr_to_pluto(&type_args[0].node, env);
                PlutoType::Sender(Box::new(t))
            } else if name == "Receiver" && type_args.len() == 1 {
                let t = resolve_type_expr_to_pluto(&type_args[0].node, env);
                PlutoType::Receiver(Box::new(t))
            } else {
                panic!("Generic TypeExpr should not reach codegen — monomorphize should have resolved it")
            }
        }
    }
}

/// Convert a Pluto value to an i64 slot for array storage.
fn to_array_slot(val: Value, ty: &PlutoType, builder: &mut FunctionBuilder<'_>) -> Value {
    match ty {
        PlutoType::Float => builder.ins().bitcast(types::I64, MemFlags::new(), val),
        PlutoType::Bool | PlutoType::Byte => builder.ins().uextend(types::I64, val),
        _ => val, // int, string, class, array are already I64
    }
}

/// Convert an i64 slot from array storage back to the Pluto type's representation.
fn from_array_slot(val: Value, ty: &PlutoType, builder: &mut FunctionBuilder<'_>) -> Value {
    match ty {
        PlutoType::Float => builder.ins().bitcast(types::F64, MemFlags::new(), val),
        PlutoType::Bool | PlutoType::Byte => builder.ins().ireduce(types::I8, val),
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
        PlutoType::Map(_, _) => types::I64,    // pointer to map handle
        PlutoType::Set(_) => types::I64,       // pointer to set handle
        PlutoType::Task(_) => types::I64,      // pointer to task handle
        PlutoType::Sender(_) => types::I64,    // pointer to channel handle
        PlutoType::Receiver(_) => types::I64,  // pointer to channel handle
        PlutoType::Error => types::I64,        // pointer to error object
        PlutoType::Range => types::I64,           // not used as a value type
        PlutoType::TypeParam(name) => panic!("ICE: generic type parameter '{name}' reached codegen unresolved"),
        PlutoType::Byte => types::I8,          // unsigned 8-bit value
        PlutoType::Bytes => types::I64,        // pointer to bytes handle
    }
}

/// Returns the key type tag integer for the runtime hash table.
/// 0=int, 1=float, 2=bool, 3=string, 4=enum
fn key_type_tag(ty: &PlutoType) -> i64 {
    match ty {
        PlutoType::Int => 0,
        PlutoType::Float => 1,
        PlutoType::Bool => 2,
        PlutoType::String => 3,
        PlutoType::Byte => 0, // hashes as integer value
        PlutoType::Enum(_) => 4,
        _ => 0, // fallback
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
                UnaryOp::BitNot => PlutoType::Int,
                UnaryOp::Neg => infer_type_for_expr(&operand.node, env, var_types),
            }
        }
        Expr::Cast { target_type, .. } => resolve_type_expr_to_pluto(&target_type.node, env),
        Expr::Call { name, args } => {
            // Check if calling a closure variable first
            if let Some(PlutoType::Fn(_, ret)) = var_types.get(&name.node) {
                return *ret.clone();
            }
            // Check builtins
            if name.node == "expect" && !args.is_empty() {
                return infer_type_for_expr(&args[0].node, env, var_types);
            }
            if name.node == "print" {
                return PlutoType::Void;
            }
            if name.node == "time_ns" {
                return PlutoType::Int;
            }
            if name.node == "abs" || name.node == "min" || name.node == "max" || name.node == "pow" {
                return infer_type_for_expr(&args[0].node, env, var_types);
            }
            if matches!(
                name.node.as_str(),
                "sqrt" | "floor" | "ceil" | "round" | "sin" | "cos" | "tan" | "log"
            ) {
                return PlutoType::Float;
            }
            if name.node == "gc_heap_size" {
                return PlutoType::Int;
            }
            if name.node == "bytes_new" {
                return PlutoType::Bytes;
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
            } else if obj_type == PlutoType::Error && field.node == "message" {
                PlutoType::String
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
            } else if let PlutoType::Map(_, v) = obj_type {
                *v
            } else if obj_type == PlutoType::Bytes {
                PlutoType::Byte
            } else if obj_type == PlutoType::String {
                PlutoType::String
            } else {
                PlutoType::Void
            }
        }
        Expr::MapLit { key_type, value_type, .. } => {
            let kt = resolve_type_expr_to_pluto(&key_type.node, env);
            let vt = resolve_type_expr_to_pluto(&value_type.node, env);
            PlutoType::Map(Box::new(kt), Box::new(vt))
        }
        Expr::SetLit { elem_type, .. } => {
            let et = resolve_type_expr_to_pluto(&elem_type.node, env);
            PlutoType::Set(Box::new(et))
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
            // expect() intrinsic methods always return Void
            if let Expr::Call { name, .. } = &object.node {
                if name.node == "expect" {
                    return PlutoType::Void;
                }
            }
            let obj_type = infer_type_for_expr(&object.node, env, var_types);
            if matches!(&obj_type, PlutoType::Array(_)) {
                return match method.node.as_str() {
                    "len" => PlutoType::Int,
                    _ => PlutoType::Void,
                };
            }
            if let PlutoType::Map(key_ty, val_ty) = &obj_type {
                return match method.node.as_str() {
                    "len" => PlutoType::Int,
                    "contains" => PlutoType::Bool,
                    "keys" => PlutoType::Array(key_ty.clone()),
                    "values" => PlutoType::Array(val_ty.clone()),
                    _ => PlutoType::Void, // insert, remove
                };
            }
            if let PlutoType::Set(elem_ty) = &obj_type {
                return match method.node.as_str() {
                    "len" => PlutoType::Int,
                    "contains" => PlutoType::Bool,
                    "to_array" => PlutoType::Array(elem_ty.clone()),
                    _ => PlutoType::Void, // insert, remove
                };
            }
            if let PlutoType::Task(inner) = &obj_type {
                return match method.node.as_str() {
                    "get" => *inner.clone(),
                    _ => PlutoType::Void,
                };
            }
            if obj_type == PlutoType::Bytes {
                return match method.node.as_str() {
                    "len" => PlutoType::Int,
                    "to_string" => PlutoType::String,
                    _ => PlutoType::Void, // push
                };
            }
            if let PlutoType::Sender(_) = &obj_type {
                return PlutoType::Void; // send/try_send/close all return void
            }
            if let PlutoType::Receiver(inner) = &obj_type {
                return match method.node.as_str() {
                    "recv" | "try_recv" => *inner.clone(),
                    _ => PlutoType::Void,
                };
            }
            if obj_type == PlutoType::String {
                return match method.node.as_str() {
                    "len" | "index_of" => PlutoType::Int,
                    "contains" | "starts_with" | "ends_with" => PlutoType::Bool,
                    "substring" | "trim" | "to_upper" | "to_lower" | "replace" | "char_at" => PlutoType::String,
                    "split" => PlutoType::Array(Box::new(PlutoType::String)),
                    "to_bytes" => PlutoType::Bytes,
                    _ => PlutoType::Void,
                };
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
        Expr::Spawn { call } => {
            let closure_type = infer_type_for_expr(&call.node, env, var_types);
            match closure_type {
                PlutoType::Fn(_, ret) => PlutoType::Task(ret),
                _ => PlutoType::Void,
            }
        }
        Expr::Range { .. } => PlutoType::Range,
    }
}

/// Convert a byte offset to a 1-based line number.
fn byte_to_line(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())].bytes().filter(|b| *b == b'\n').count() + 1
}
