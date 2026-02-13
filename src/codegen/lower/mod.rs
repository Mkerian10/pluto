use std::collections::{HashMap, HashSet};

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::immediates::Offset32;
use cranelift_codegen::ir::{types, AbiParam, InstBuilder, MemFlags, Value};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::{DataDescription, DataId, FuncId, Module};

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::typeck::env::{mangle_method, TypeEnv};
use crate::typeck::types::PlutoType;
use crate::visit::{walk_stmt, Visitor};

use super::runtime::RuntimeRegistry;

/// Size of a pointer in bytes. All heap-allocated objects use pointer-sized slots.
pub const POINTER_SIZE: i32 = 8;

/// Pre/post condition contracts for a function.
pub struct FnContracts {
    pub requires: Vec<(Expr, String)>,  // (expr, description)
    pub ensures: Vec<(Expr, String)>,
}

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
    /// Function contracts: fn_mangled_name → FnContracts (used during function setup)
    #[allow(dead_code)]
    fn_contracts: &'a HashMap<String, FnContracts>,
    /// Module-level globals holding DI singleton pointers, used by scope blocks.
    singleton_globals: &'a HashMap<String, DataId>,
    /// Module-level globals holding rwlock pointers for synchronized singletons.
    rwlock_globals: &'a HashMap<String, DataId>,
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
    /// old() snapshots: keyed by description string → Variable holding the snapshot value
    old_snapshots: HashMap<String, Variable>,
    /// If non-None, all returns jump here for ensures checks before exit_block/return
    ensures_block: Option<cranelift_codegen::ir::Block>,
    /// Display name for the current function (for contract violation messages)
    fn_display_name: String,
    /// Whether this function is a spawn closure (return values must be I64-encoded)
    is_spawn_closure: bool,
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

    /// Load a singleton pointer from its module-level global.
    /// Used by scope block codegen to wire singleton dependencies into scoped instances.
    fn load_singleton(&mut self, class_name: &str) -> Result<Value, CompileError> {
        let data_id = self.singleton_globals.get(class_name).ok_or_else(|| {
            CompileError::codegen(format!("no singleton global for '{}'", class_name))
        })?;
        let gv = self.module.declare_data_in_func(*data_id, self.builder.func);
        let addr = self.builder.ins().global_value(types::I64, gv);
        Ok(self.builder.ins().load(types::I64, MemFlags::new(), addr, Offset32::new(0)))
    }

    /// Check if a class is a stage (for RPC routing).
    fn is_stage(&self, class_name: &str) -> bool {
        self.env.stages.iter().any(|(name, _)| name == class_name)
    }

    /// Create a string literal value at runtime.
    fn make_string_literal(&mut self, s: &str) -> Result<Value, CompileError> {
        let raw_ptr = self.create_data_str(s)?;
        let len_val = self.builder.ins().iconst(types::I64, s.len() as i64);
        Ok(self.call_runtime("__pluto_string_new", &[raw_ptr, len_val]))
    }

    /// Emit a return with the default value for the current function's return type.
    /// Used by raise and propagation to exit the function when an error occurs.
    fn emit_default_return(&mut self) {
        // Spawn closures always return I64, so default is always iconst 0
        if self.is_spawn_closure && !matches!(&self.expected_return_type, Some(PlutoType::Void) | None) {
            let val = self.builder.ins().iconst(types::I64, 0);
            if let Some(exit_bb) = self.exit_block {
                self.builder.ins().jump(exit_bb, &[val]);
            } else {
                self.builder.ins().return_(&[val]);
            }
            return;
        }
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

    /// Box a value type for T → T? coercion. Allocates 8 bytes and stores the value.
    /// Heap types (string, class, array, etc.) are no-ops since the pointer IS the value.
    fn emit_nullable_wrap(&mut self, val: Value, inner_type: &PlutoType) -> Value {
        match inner_type {
            PlutoType::Int | PlutoType::Byte => {
                // Widen byte to I64 if needed, then store in 8-byte allocation
                let store_val = if matches!(inner_type, PlutoType::Byte) {
                    self.builder.ins().uextend(types::I64, val)
                } else {
                    val
                };
                let size = self.builder.ins().iconst(types::I64, 8);
                let ptr = self.call_runtime("__pluto_alloc", &[size]);
                self.builder.ins().store(MemFlags::new(), store_val, ptr, Offset32::new(0));
                ptr
            }
            PlutoType::Float => {
                let raw = self.builder.ins().bitcast(types::I64, MemFlags::new(), val);
                let size = self.builder.ins().iconst(types::I64, 8);
                let ptr = self.call_runtime("__pluto_alloc", &[size]);
                self.builder.ins().store(MemFlags::new(), raw, ptr, Offset32::new(0));
                ptr
            }
            PlutoType::Bool => {
                let widened = self.builder.ins().uextend(types::I64, val);
                let size = self.builder.ins().iconst(types::I64, 8);
                let ptr = self.call_runtime("__pluto_alloc", &[size]);
                self.builder.ins().store(MemFlags::new(), widened, ptr, Offset32::new(0));
                ptr
            }
            _ => {
                // Heap types: pointer IS the value, no boxing needed
                val
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

    /// Emit runtime requires checks at function entry.
    fn emit_requires_checks(
        &mut self,
        requires: &[(Expr, String)],
    ) -> Result<(), CompileError> {
        for (req_expr, req_desc) in requires {
            let result = self.lower_expr(req_expr)?;

            let violation_bb = self.builder.create_block();
            let ok_bb = self.builder.create_block();

            self.builder.ins().brif(result, ok_bb, &[], violation_bb, &[]);

            // Violation block
            self.builder.switch_to_block(violation_bb);
            self.builder.seal_block(violation_bb);

            let name_raw = self.create_data_str(&self.fn_display_name.clone())?;
            let name_len = self.builder.ins().iconst(types::I64, self.fn_display_name.len() as i64);
            let name_str = self.call_runtime("__pluto_string_new", &[name_raw, name_len]);

            let desc_raw = self.create_data_str(req_desc)?;
            let desc_len = self.builder.ins().iconst(types::I64, req_desc.len() as i64);
            let desc_str = self.call_runtime("__pluto_string_new", &[desc_raw, desc_len]);

            self.call_runtime_void("__pluto_requires_violation", &[name_str, desc_str]);
            self.builder.ins().trap(cranelift_codegen::ir::TrapCode::unwrap_user(1));

            // OK block: continue
            self.builder.switch_to_block(ok_bb);
            self.builder.seal_block(ok_bb);
        }
        Ok(())
    }

    /// Emit runtime ensures checks (called from ensures_block).
    /// `result_var` is the Variable holding the return value (None for void functions).
    fn emit_ensures_checks(
        &mut self,
        ensures: &[(Expr, String)],
        result_var: Option<Variable>,
    ) -> Result<(), CompileError> {
        // Temporarily bind `result` if available
        if let Some(rv) = result_var {
            self.variables.insert("result".to_string(), rv);
            // We need the return type for result — use expected_return_type
            if let Some(ref ret_ty) = self.expected_return_type.clone() {
                self.var_types.insert("result".to_string(), ret_ty.clone());
            }
        }

        for (ens_expr, ens_desc) in ensures {
            let result = self.lower_expr(ens_expr)?;

            let violation_bb = self.builder.create_block();
            let ok_bb = self.builder.create_block();

            self.builder.ins().brif(result, ok_bb, &[], violation_bb, &[]);

            // Violation block
            self.builder.switch_to_block(violation_bb);
            self.builder.seal_block(violation_bb);

            let name_raw = self.create_data_str(&self.fn_display_name.clone())?;
            let name_len = self.builder.ins().iconst(types::I64, self.fn_display_name.len() as i64);
            let name_str = self.call_runtime("__pluto_string_new", &[name_raw, name_len]);

            let desc_raw = self.create_data_str(ens_desc)?;
            let desc_len = self.builder.ins().iconst(types::I64, ens_desc.len() as i64);
            let desc_str = self.call_runtime("__pluto_string_new", &[desc_raw, desc_len]);

            self.call_runtime_void("__pluto_ensures_violation", &[name_str, desc_str]);
            self.builder.ins().trap(cranelift_codegen::ir::TrapCode::unwrap_user(1));

            // OK block: continue
            self.builder.switch_to_block(ok_bb);
            self.builder.seal_block(ok_bb);
        }

        // Clean up result binding
        if result_var.is_some() {
            self.variables.remove("result");
            self.var_types.remove("result");
        }

        Ok(())
    }

    /// Recursively collect all old(expr) descriptions from an expression.
    fn collect_old_exprs(expr: &Expr, out: &mut Vec<(Expr, String)>) {
        match expr {
            Expr::Call { name, args, .. } if name.node == "old" && args.len() == 1 => {
                let desc = super::format_invariant_expr(&args[0].node);
                out.push((args[0].node.clone(), desc));
            }
            Expr::BinOp { lhs, rhs, .. } => {
                Self::collect_old_exprs(&lhs.node, out);
                Self::collect_old_exprs(&rhs.node, out);
            }
            Expr::UnaryOp { operand, .. } => {
                Self::collect_old_exprs(&operand.node, out);
            }
            Expr::FieldAccess { object, .. } => {
                Self::collect_old_exprs(&object.node, out);
            }
            Expr::MethodCall { object, args, .. } => {
                Self::collect_old_exprs(&object.node, out);
                for arg in args {
                    Self::collect_old_exprs(&arg.node, out);
                }
            }
            _ => {}
        }
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
            Stmt::Let { name, ty, value, .. } => self.lower_let(name, ty, value),
            Stmt::LetChan { sender, receiver, elem_type, capacity } => self.lower_let_chan(sender, receiver, elem_type, capacity),
            Stmt::Return(value) => {
                // Return target priority: ensures_block > exit_block > direct return
                let target_block = self.ensures_block.or(self.exit_block);
                match value {
                    Some(expr) => {
                        let val = self.lower_expr(&expr.node)?;
                        let val_type = infer_type_for_expr(&expr.node, self.env, &self.var_types);

                        // If returning a void expression (e.g., spawn closure wrapping a void function),
                        // lower the expr for side effects but emit default return
                        if val_type == PlutoType::Void {
                            self.emit_default_return();
                        } else {
                            // If returning a class where a trait is expected, wrap it
                            // If returning T where T? is expected, box value types
                            let expected = self.expected_return_type.clone();
                            let final_val = match (&val_type, &expected) {
                                (PlutoType::Class(cn), Some(PlutoType::Trait(tn))) => {
                                    self.wrap_class_as_trait(val, cn, tn)?
                                }
                                (inner, Some(PlutoType::Nullable(expected_inner))) if !matches!(inner, PlutoType::Nullable(_)) && **expected_inner != PlutoType::Void => {
                                    self.emit_nullable_wrap(val, inner)
                                }
                                _ => val,
                            };
                            // Spawn closures must return I64 (C runtime reads integer register)
                            let final_val = if self.is_spawn_closure && val_type != PlutoType::Void {
                                to_array_slot(final_val, &val_type, &mut self.builder)
                            } else {
                                final_val
                            };
                            if let Some(bb) = target_block {
                                self.builder.ins().jump(bb, &[final_val]);
                            } else {
                                self.builder.ins().return_(&[final_val]);
                            }
                        }
                    }
                    None => {
                        self.emit_default_return();
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
                if let PlutoType::Class(class_name) = &obj_type
                    && let Some(class_info) = self.env.classes.get(class_name)
                {
                    let offset = class_info.fields.iter()
                        .position(|(n, _, _)| *n == field.node)
                        .ok_or_else(|| CompileError::codegen(format!("unknown field '{}' on class '{class_name}'", field.node)))? as i32 * POINTER_SIZE;
                    self.builder.ins().store(MemFlags::new(), val, ptr, Offset32::new(offset));
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
            Stmt::Raise { error_name, fields, .. } => {
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
            Stmt::Scope { seeds, bindings, body } => self.lower_scope(seeds, bindings, body),
            Stmt::Yield { .. } => {
                // Generator yield is handled by lower_generator_next, not lower_stmt
                unreachable!("Stmt::Yield should only appear in generator next function codegen")
            }
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
        // If assigning T to T?, box value types
        let (final_val, store_type) = match (&val_type, &declared_type) {
            (PlutoType::Class(cn), Some(PlutoType::Trait(tn))) => {
                let cn = cn.clone();
                let tn = tn.clone();
                let wrapped = self.wrap_class_as_trait(val, &cn, &tn)?;
                (wrapped, PlutoType::Trait(tn))
            }
            // T → T? coercion: box value types
            (inner, Some(PlutoType::Nullable(expected_inner))) if !matches!(inner, PlutoType::Nullable(_)) && **expected_inner != PlutoType::Void => {
                let wrapped = self.emit_nullable_wrap(val, inner);
                (wrapped, PlutoType::Nullable(expected_inner.clone()))
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
            // Safepoint check before loop back-edge
            self.call_runtime_void("__pluto_safepoint", &[]);
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
            PlutoType::Stream(_) => self.lower_for_stream(var, iterable, body),
            other => Err(CompileError::codegen(
                format!("for loop requires array, range, string, bytes, receiver, or stream, found {}", other)
            )),
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
        // Safepoint check before loop back-edge
        self.call_runtime_void("__pluto_safepoint", &[]);
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
            other => return Err(CompileError::codegen(
                format!("for loop requires array, found {}", other)
            )),
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
        // Safepoint check before loop back-edge
        self.call_runtime_void("__pluto_safepoint", &[]);
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
        // Safepoint check before loop back-edge
        self.call_runtime_void("__pluto_safepoint", &[]);
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
        // Safepoint check before loop back-edge
        self.call_runtime_void("__pluto_safepoint", &[]);
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
            other => return Err(CompileError::codegen(
                format!("for-in requires receiver, found {}", other)
            )),
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
            // Safepoint check before loop back-edge
            self.call_runtime_void("__pluto_safepoint", &[]);
            self.builder.ins().jump(header_bb, &[]);
        }

        self.builder.seal_block(header_bb);
        self.builder.switch_to_block(exit_bb);
        self.builder.seal_block(exit_bb);
        Ok(())
    }

    fn lower_for_stream(
        &mut self,
        var: &crate::span::Spanned<String>,
        iterable: &crate::span::Spanned<Expr>,
        body: &crate::span::Spanned<Block>,
    ) -> Result<(), CompileError> {
        let gen_ptr = self.lower_expr(&iterable.node)?;

        let iter_type = infer_type_for_expr(&iterable.node, self.env, &self.var_types);
        let elem_type = match &iter_type {
            PlutoType::Stream(elem) => *elem.clone(),
            other => return Err(CompileError::codegen(
                format!("for-in requires stream, found {}", other)
            )),
        };

        // Blocks: header calls next, body processes value, exit leaves loop
        let header_bb = self.builder.create_block();
        let body_bb = self.builder.create_block();
        let exit_bb = self.builder.create_block();

        self.builder.ins().jump(header_bb, &[]);

        // Header: load next_fn_ptr from gen_ptr[0], call indirect, check done flag
        self.builder.switch_to_block(header_bb);

        // Load the next function pointer from offset 0
        let next_fn_ptr = self.builder.ins().load(types::I64, MemFlags::new(), gen_ptr, Offset32::new(0));

        // Build signature for the next function: (I64) -> void
        let mut next_sig = self.module.make_signature();
        next_sig.params.push(AbiParam::new(types::I64));
        let next_sig_ref = self.builder.func.import_signature(next_sig);

        // Call next function indirectly
        self.builder.ins().call_indirect(next_sig_ref, next_fn_ptr, &[gen_ptr]);

        // Check done flag at offset 16
        let done = self.builder.ins().load(types::I64, MemFlags::new(), gen_ptr, Offset32::new(16));
        let zero = self.builder.ins().iconst(types::I64, 0);
        let is_done = self.builder.ins().icmp(IntCC::NotEqual, done, zero);
        self.builder.ins().brif(is_done, exit_bb, &[], body_bb, &[]);

        // Body: load result from gen_ptr[24], convert to typed value
        self.builder.switch_to_block(body_bb);
        self.builder.seal_block(body_bb);

        let raw_result = self.builder.ins().load(types::I64, MemFlags::new(), gen_ptr, Offset32::new(24));
        let elem_val = from_array_slot(raw_result, &elem_type, &mut self.builder);

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

        // Push loop stack: continue goes to header (re-call next), break goes to exit
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
            // Safepoint check before loop back-edge
            self.call_runtime_void("__pluto_safepoint", &[]);
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
            other_type => return Err(CompileError::codegen(
                format!("match requires enum type, found {}", other_type)
            )),
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
                .expect("match arm variant should exist after typeck") as i64;
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
                .expect("match arm variant should exist after typeck").1;

            // Save previous variable bindings so we can restore after this arm
            let mut prev_vars: Vec<(String, Option<Variable>, Option<PlutoType>)> = Vec::new();

            for (binding_field, opt_rename) in &arm.bindings {
                let field_idx = variant_fields.iter()
                    .position(|(n, _)| *n == binding_field.node)
                    .expect("binding field should exist in variant after typeck");
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

    fn lower_scope(
        &mut self,
        seeds: &[crate::span::Spanned<Expr>],
        bindings: &[ScopeBinding],
        body: &crate::span::Spanned<Block>,
    ) -> Result<(), CompileError> {
        use crate::typeck::env::FieldWiring;

        // Look up the ScopeResolution computed during typeck
        // We need to find the Stmt::Scope span — use the first seed's start and body's end
        // Actually, the key is (stmt span.start, stmt span.end). The stmt span encompasses
        // everything from `scope` keyword to end of body `}`. We need to match what check.rs stored.
        // The dispatch in lower_stmt gives us the stmt node directly, and the span is on the outer
        // Spanned<Stmt>. We can reconstruct from seeds[0] start to body end.
        // Better: find the resolution by scanning for one that matches our seeds/body span range.
        let scope_key = self.env.scope_resolutions.keys()
            .find(|&&(start, end)| {
                // The scope stmt span starts before the first seed and ends at body end
                seeds.first().map_or(false, |s| start <= s.span.start) &&
                end == body.span.end
            })
            .copied()
            .ok_or_else(|| {
                CompileError::codegen("missing ScopeResolution for scope block".to_string())
            })?;
        let resolution = self.env.scope_resolutions.get(&scope_key).unwrap().clone();

        // 1. Evaluate seed expressions and store in locals
        let mut seed_vals: Vec<Value> = Vec::new();
        for seed in seeds {
            let val = self.lower_expr(&seed.node)?;
            seed_vals.push(val);
        }

        // 2. Allocate scoped instances in topological order
        let mut scoped_locals: HashMap<String, Value> = HashMap::new();

        // Also map seed class names to their values for wiring
        // We need to know seed class names — get from typeck env
        // For now, infer from ScopeResolution binding_sources: Seed(idx) gives us the idx
        // But we need the class names. Get them from the env by checking seed types.
        // Actually we stored seed indices in FieldWiring::Seed — the class name for each seed
        // can be recovered from FieldWiring entries. Let's just iterate all wirings.
        // Simpler: store seed values by looking at binding_sources + field_wirings.
        // We need a map from seed index to value (already have seed_vals), and from class_name to value.

        // Build seed_name_to_val by inspecting what classes the seeds produce.
        // We can look at the binding_sources: if a FieldWiring::Seed(idx) appears, that seed
        // provides some class. But we need the actual class names for ScopedInstance wiring.
        // Let's use the field_wirings: scan for Seed(idx) entries to discover which class name
        // each seed provides. Or better: just look at the ScopeResolution field_wirings for
        // FieldWiring::Seed values.
        // Actually the simplest approach: any class that's NOT in creation_order but IS referenced
        // as ScopedInstance must be a seed. But that's fragile.
        // The cleanest: we know seed types from typeck. The ScopeResolution doesn't store them
        // directly, but we can reconstruct: for binding_sources, Seed(idx) tells us that binding
        // is a seed. For field_wirings, Seed(idx) tells us which seed provides a dep.
        // We just need a map from class_name → Value for ALL scope-available classes.

        for class_name in &resolution.creation_order {
            let class_info = self.env.classes.get(class_name).ok_or_else(|| {
                CompileError::codegen(format!("scope: unknown class '{class_name}'"))
            })?.clone();
            let num_fields = class_info.fields.len() as i64;
            let size = num_fields * POINTER_SIZE as i64;
            let size_val = self.builder.ins().iconst(types::I64, size);
            let ptr = self.call_runtime("__pluto_alloc", &[size_val]);

            // Wire fields
            if let Some(wirings) = resolution.field_wirings.get(class_name) {
                for (field_name, wiring) in wirings {
                    let field_idx = class_info.fields.iter()
                        .position(|(n, _, _)| n == field_name)
                        .ok_or_else(|| {
                            CompileError::codegen(format!("scope: unknown field '{field_name}' on '{class_name}'"))
                        })?;
                    let offset = field_idx as i32 * POINTER_SIZE;

                    let dep_val = match wiring {
                        FieldWiring::Seed(idx) => seed_vals[*idx],
                        FieldWiring::Singleton(name) => self.load_singleton(name)?,
                        FieldWiring::ScopedInstance(name) => {
                            *scoped_locals.get(name).ok_or_else(|| {
                                CompileError::codegen(format!(
                                    "scope: scoped instance '{name}' not yet created (ordering bug)"
                                ))
                            })?
                        }
                    };

                    self.builder.ins().store(
                        MemFlags::new(),
                        dep_val,
                        ptr,
                        Offset32::new(offset),
                    );
                }
            }

            scoped_locals.insert(class_name.clone(), ptr);
        }

        // 3. Save current variable bindings and define scope bindings
        let mut saved_vars: Vec<(String, Option<Variable>, Option<PlutoType>)> = Vec::new();

        for (i, binding) in bindings.iter().enumerate() {
            let name = &binding.name.node;
            let prev_var = self.variables.get(name).cloned();
            let prev_type = self.var_types.get(name).cloned();
            saved_vars.push((name.clone(), prev_var, prev_type));

            // Get the value for this binding
            let val = match &resolution.binding_sources[i] {
                FieldWiring::Seed(idx) => seed_vals[*idx],
                FieldWiring::ScopedInstance(class_name) => {
                    *scoped_locals.get(class_name).ok_or_else(|| {
                        CompileError::codegen(format!(
                            "scope: binding class '{class_name}' not available"
                        ))
                    })?
                }
                FieldWiring::Singleton(class_name) => self.load_singleton(class_name)?,
            };

            // Create a new Cranelift variable for the binding
            let var = Variable::from_u32(self.next_var);
            self.next_var += 1;
            self.builder.declare_var(var, types::I64);
            self.builder.def_var(var, val);

            let ty = resolve_type_expr_to_pluto(&binding.ty.node, self.env);
            self.variables.insert(name.clone(), var);
            self.var_types.insert(name.clone(), ty);
        }

        // 4. Lower body
        let mut body_terminated = false;
        for s in &body.node.stmts {
            self.lower_stmt(&s.node, &mut body_terminated)?;
        }

        // 5. Restore previous variable bindings
        for (name, prev_var, prev_type) in saved_vars {
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
            Expr::NoneLit => Ok(self.builder.ins().iconst(types::I64, 0)),
            Expr::NullPropagate { expr: inner } => {
                // Lower the inner expression (must be Nullable(T))
                let val = self.lower_expr(&inner.node)?;
                let inner_type = infer_type_for_expr(&inner.node, self.env, &self.var_types);

                // Compare with 0 (none)
                let zero = self.builder.ins().iconst(types::I64, 0);
                let is_none = self.builder.ins().icmp(IntCC::Equal, val, zero);

                let propagate_bb = self.builder.create_block();
                let continue_bb = self.builder.create_block();
                self.builder.ins().brif(is_none, propagate_bb, &[], continue_bb, &[]);

                // Propagate block: early-return none (0), or void return for void functions
                self.builder.switch_to_block(propagate_bb);
                self.builder.seal_block(propagate_bb);
                let is_void_return = matches!(&self.expected_return_type, Some(PlutoType::Void) | None);
                if is_void_return {
                    if let Some(exit_bb) = self.exit_block {
                        self.builder.ins().jump(exit_bb, &[]);
                    } else {
                        self.builder.ins().return_(&[]);
                    }
                } else {
                    let none_val = self.builder.ins().iconst(types::I64, 0);
                    if let Some(exit_bb) = self.exit_block {
                        self.builder.ins().jump(exit_bb, &[none_val]);
                    } else {
                        self.builder.ins().return_(&[none_val]);
                    }
                }

                // Continue block: unwrap the value
                self.builder.switch_to_block(continue_bb);
                self.builder.seal_block(continue_bb);

                // Unbox value types (int, float, bool stored as boxed pointer)
                if let PlutoType::Nullable(unwrapped) = &inner_type {
                    match unwrapped.as_ref() {
                        PlutoType::Int => {
                            Ok(self.builder.ins().load(types::I64, MemFlags::new(), val, Offset32::new(0)))
                        }
                        PlutoType::Float => {
                            let raw = self.builder.ins().load(types::I64, MemFlags::new(), val, Offset32::new(0));
                            Ok(self.builder.ins().bitcast(types::F64, MemFlags::new(), raw))
                        }
                        PlutoType::Bool => {
                            let raw = self.builder.ins().load(types::I64, MemFlags::new(), val, Offset32::new(0));
                            Ok(self.builder.ins().ireduce(types::I8, raw))
                        }
                        PlutoType::Byte => {
                            let raw = self.builder.ins().load(types::I64, MemFlags::new(), val, Offset32::new(0));
                            Ok(self.builder.ins().ireduce(types::I8, raw))
                        }
                        _ => {
                            // Heap types (string, class, array, etc.) — pointer IS the value
                            Ok(val)
                        }
                    }
                } else {
                    // Shouldn't happen — typeck ensures ? is only on nullable types
                    Ok(val)
                }
            }
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
                match (source_type.clone(), target_type.clone()) {
                    (PlutoType::Int, PlutoType::Float) => Ok(self.builder.ins().fcvt_from_sint(types::F64, val)),
                    (PlutoType::Float, PlutoType::Int) => Ok(self.builder.ins().fcvt_to_sint_sat(types::I64, val)),
                    (PlutoType::Int, PlutoType::Bool) => {
                        let zero = self.builder.ins().iconst(types::I64, 0);
                        Ok(self.builder.ins().icmp(IntCC::NotEqual, val, zero))
                    }
                    (PlutoType::Bool, PlutoType::Int) => Ok(self.builder.ins().uextend(types::I64, val)),
                    (PlutoType::Int, PlutoType::Byte) => Ok(self.builder.ins().ireduce(types::I8, val)),
                    (PlutoType::Byte, PlutoType::Int) => Ok(self.builder.ins().uextend(types::I64, val)),
                    (src, tgt) => Err(CompileError::codegen(
                        format!("invalid cast from {} to {} in lowered AST", src, tgt)
                    )),
                }
            }
            Expr::Call { name, args, .. } => self.lower_call(name, args),
            Expr::StructLit { name, fields, .. } => self.lower_struct_lit(name, fields),
            Expr::ArrayLit { elements } => {
                let n = elements.len() as i64;
                let cap_val = self.builder.ins().iconst(types::I64, n);
                let handle = self.call_runtime("__pluto_array_new", &[cap_val]);

                if !elements.is_empty() {
                    let elem_type = infer_type_for_expr(&elements[0].node, self.env, &self.var_types);
                    // Hoist func_ref before loop to avoid repeated HashMap lookups
                    let func_ref_push = self.module.declare_func_in_func(self.runtime.get("__pluto_array_push"), self.builder.func);
                    for elem in elements {
                        let val = self.lower_expr(&elem.node)?;
                        let slot = to_array_slot(val, &elem_type, &mut self.builder);
                        self.builder.ins().call(func_ref_push, &[handle, slot]);
                    }
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
                    let tag = self.builder.ins().iconst(types::I64, key_type_tag(key_ty));
                    let key_slot = to_array_slot(idx, key_ty, &mut self.builder);
                    let raw = self.call_runtime("__pluto_map_get", &[handle, tag, key_slot]);
                    Ok(from_array_slot(raw, val_ty, &mut self.builder))
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
                let variant_idx = enum_info.variants.iter().position(|(n, _)| *n == variant.node)
                    .expect("variant should exist after typeck validation");

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
                self.emit_default_return();

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
            Expr::ClosureCreate { fn_name, captures, .. } => {
                self.lower_closure_create(fn_name, captures)
            }
            Expr::Spawn { call } => {
                match &call.node {
                    Expr::ClosureCreate { fn_name, captures, .. } => {
                        let closure_ptr = self.lower_closure_create(fn_name, captures)?;
                        // Deep-copy heap-type captures so spawned task gets isolated data.
                        // DI singletons and the app instance are shared by reference (not copied).
                        for (i, cap_name) in captures.iter().enumerate() {
                            let cap_type = self.var_types.get(cap_name).cloned().unwrap_or(PlutoType::Int);
                            let is_di_singleton = if let PlutoType::Class(name) = &cap_type {
                                self.env.di_order.contains(name)
                                    || self.env.app.as_ref().map_or(false, |(app_name, _)| app_name == name)
                            } else {
                                false
                            };
                            if !is_di_singleton && needs_deep_copy(&cap_type) {
                                let offset = ((1 + i) * 8) as i32;
                                let original = self.builder.ins().load(
                                    types::I64, MemFlags::new(), closure_ptr, Offset32::new(offset),
                                );
                                let copied = self.call_runtime("__pluto_deep_copy", &[original]);
                                self.builder.ins().store(
                                    MemFlags::new(), copied, closure_ptr, Offset32::new(offset),
                                );
                            }
                        }
                        // Inc refcount for each captured Sender
                        for cap_name in captures {
                            if let Some(PlutoType::Sender(_)) = self.var_types.get(cap_name) {
                                let var = self.variables.get(cap_name)
                                    .expect("captured sender should have a variable in scope");
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
            Expr::StaticTraitCall { trait_name, method_name, type_args, args } => {
                self.lower_static_trait_call(trait_name, method_name, type_args, args)
            }
            Expr::If { condition, then_block, else_block } => {
                self.lower_if_expr(condition, then_block, else_block)
            }
            Expr::QualifiedAccess { segments } => {
                panic!(
                    "QualifiedAccess should be resolved by module flattening before codegen. Segments: {:?}",
                    segments.iter().map(|s| &s.node).collect::<Vec<_>>()
                )
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
        // old(expr) in ensures — resolve to snapshot variable
        if name.node == "old" && args.len() == 1 {
            let desc = super::format_invariant_expr(&args[0].node);
            if let Some(&var) = self.old_snapshots.get(&desc) {
                return Ok(self.builder.use_var(var));
            }
            // Fallback: if not found in snapshots, just evaluate the expr normally
            return self.lower_expr(&args[0].node);
        }
        if name.node == "expect" {
            // Passthrough — just return the lowered arg
            return self.lower_expr(&args[0].node);
        }
        if name.node == "print" {
            return self.lower_print(args);
        }
        // Table-driven zero-arg builtins
        const ZERO_ARG_BUILTINS: &[(&str, &str)] = &[
            ("time_ns", "__pluto_time_ns"),
            ("gc_heap_size", "__pluto_gc_heap_size"),
            ("bytes_new", "__pluto_bytes_new"),
        ];
        if let Some((_, rt_fn)) = ZERO_ARG_BUILTINS.iter().find(|(n, _)| *n == name.node.as_str()) {
            return Ok(self.call_runtime(rt_fn, &[]));
        }

        // Table-driven type-dispatched unary builtins (int/float)
        const TYPED_UNARY: &[(&str, &str, &str)] = &[
            ("abs", "__pluto_abs_int", "__pluto_abs_float"),
        ];
        if let Some((_, int_fn, float_fn)) = TYPED_UNARY.iter().find(|(n, _, _)| *n == name.node.as_str()) {
            let arg = self.lower_expr(&args[0].node)?;
            let arg_ty = infer_type_for_expr(&args[0].node, self.env, &self.var_types);
            return Ok(match arg_ty {
                PlutoType::Int => self.call_runtime(int_fn, &[arg]),
                PlutoType::Float => self.call_runtime(float_fn, &[arg]),
                _ => return Err(CompileError::codegen(format!("invalid {}() argument type in lowered AST", name.node))),
            });
        }

        // Table-driven type-dispatched binary builtins (int/float)
        const TYPED_BINARY: &[(&str, &str, &str)] = &[
            ("min", "__pluto_min_int", "__pluto_min_float"),
            ("max", "__pluto_max_int", "__pluto_max_float"),
            ("pow", "__pluto_pow_int", "__pluto_pow_float"),
        ];
        if let Some((_, int_fn, float_fn)) = TYPED_BINARY.iter().find(|(n, _, _)| *n == name.node.as_str()) {
            let a = self.lower_expr(&args[0].node)?;
            let b = self.lower_expr(&args[1].node)?;
            let arg_ty = infer_type_for_expr(&args[0].node, self.env, &self.var_types);
            return Ok(match arg_ty {
                PlutoType::Int => self.call_runtime(int_fn, &[a, b]),
                PlutoType::Float => self.call_runtime(float_fn, &[a, b]),
                _ => return Err(CompileError::codegen(format!("invalid {}() argument type in lowered AST", name.node))),
            });
        }

        // Table-driven single-arg float math builtins
        const MATH_UNARY_FLOAT: &[(&str, &str)] = &[
            ("sqrt", "__pluto_sqrt"), ("floor", "__pluto_floor"),
            ("ceil", "__pluto_ceil"), ("round", "__pluto_round"),
            ("sin", "__pluto_sin"),   ("cos", "__pluto_cos"),
            ("tan", "__pluto_tan"),   ("log", "__pluto_log"),
        ];
        if let Some((_, rt_fn)) = MATH_UNARY_FLOAT.iter().find(|(n, _)| *n == name.node.as_str()) {
            let arg = self.lower_expr(&args[0].node)?;
            return Ok(self.call_runtime(rt_fn, &[arg]));
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

    fn lower_static_trait_call(
        &mut self,
        trait_name: &crate::span::Spanned<String>,
        method_name: &crate::span::Spanned<String>,
        type_args: &[crate::span::Spanned<crate::parser::ast::TypeExpr>],
        args: &[crate::span::Spanned<Expr>],
    ) -> Result<Value, CompileError> {
        // Mangle the function name: TraitName_methodName
        // For generic calls like TypeInfo::kind<int>(), we'll add type args to the mangling
        let mangled_name = if type_args.is_empty() {
            format!("{}_{}", trait_name.node, method_name.node)
        } else {
            // For generic static trait methods, mangle with type arguments
            // TypeInfo::kind<int>() becomes TypeInfo_kind__int
            let type_arg_str = type_args.iter()
                .map(|ta| self.mangle_type_expr(&ta.node))
                .collect::<Vec<_>>()
                .join("_");
            format!("{}_{}_{}", trait_name.node, method_name.node, type_arg_str)
        };

        // Look up the function
        let func_id = self.func_ids.get(&mangled_name).ok_or_else(|| {
            CompileError::codegen(format!(
                "undefined static trait method '{}::{}' (looking for function '{}')",
                trait_name.node, method_name.node, mangled_name
            ))
        })?;

        let func_ref = self.module.declare_func_in_func(*func_id, self.builder.func);

        // Lower all arguments
        let mut arg_values = Vec::new();
        for arg in args {
            let val = self.lower_expr(&arg.node)?;
            arg_values.push(val);
        }

        // Make the call
        let call = self.builder.ins().call(func_ref, &arg_values);
        let results = self.builder.inst_results(call);
        if results.is_empty() {
            Ok(self.builder.ins().iconst(types::I64, 0))
        } else {
            Ok(results[0])
        }
    }

    /// Helper to mangle a type expression into a string for function name mangling
    fn mangle_type_expr(&self, ty: &crate::parser::ast::TypeExpr) -> String {
        use crate::parser::ast::TypeExpr;
        match ty {
            TypeExpr::Named(name) => name.clone(),
            TypeExpr::Array(elem) => format!("array_{}", self.mangle_type_expr(&elem.node)),
            TypeExpr::Generic { name, type_args } => {
                let arg_strs: Vec<_> = type_args.iter().map(|a| self.mangle_type_expr(&a.node)).collect();
                format!("{}_{}", name, arg_strs.join("_"))
            }
            TypeExpr::Nullable(inner) => format!("nullable_{}", self.mangle_type_expr(&inner.node)),
            TypeExpr::Qualified { module, name } => format!("{}_{}", module, name),
            TypeExpr::Fn { .. } => "fn".to_string(), // Function types in type args (rare)
            TypeExpr::Stream(inner) => format!("stream_{}", self.mangle_type_expr(&inner.node)),
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
            let val_type = infer_type_for_expr(&lit_val.node, self.env, &self.var_types);

            let field_idx = field_info.iter()
                .position(|(n, _, _)| *n == lit_name.node)
                .ok_or_else(|| CompileError::codegen(format!("unknown field '{}' on class '{}'", lit_name.node, name.node)))?;
            let field_type = &field_info[field_idx].1;

            // Handle T → T? coercion
            let final_val = match (&val_type, field_type) {
                (inner, PlutoType::Nullable(expected_inner))
                    if !matches!(inner, PlutoType::Nullable(_)) && **expected_inner != PlutoType::Void => {
                    self.emit_nullable_wrap(val, inner)
                }
                _ => val,
            };

            let offset = (field_idx as i32) * POINTER_SIZE;
            self.builder.ins().store(MemFlags::new(), final_val, ptr, Offset32::new(offset));
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
        let variant_idx = enum_info.variants.iter().position(|(n, _)| *n == variant.node)
            .expect("variant should exist after typeck validation");
        let variant_fields = &enum_info.variants[variant_idx].1;

        let size_val = self.builder.ins().iconst(types::I64, alloc_size);
        let ptr = self.call_runtime("__pluto_alloc", &[size_val]);

        let tag_val = self.builder.ins().iconst(types::I64, variant_idx as i64);
        self.builder.ins().store(MemFlags::new(), tag_val, ptr, Offset32::new(0));

        for (lit_name, lit_val) in fields {
            let val = self.lower_expr(&lit_val.node)?;
            let val_type = infer_type_for_expr(&lit_val.node, self.env, &self.var_types);

            let field_idx = variant_fields.iter().position(|(n, _)| *n == lit_name.node)
                .expect("field should exist after typeck validation");
            let field_type = &variant_fields[field_idx].1;

            // Handle T → T? coercion first, then array slot conversion
            let wrapped_val = match (&val_type, field_type) {
                (inner, PlutoType::Nullable(expected_inner))
                    if !matches!(inner, PlutoType::Nullable(_)) && **expected_inner != PlutoType::Void => {
                    self.emit_nullable_wrap(val, inner)
                }
                _ => val,
            };
            let slot = to_array_slot(wrapped_val, field_type, &mut self.builder);
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

                let stmts = &body.node.stmts;
                let mut block_terminated = false;

                // Lower all statements except the last
                for stmt in stmts.iter().take(stmts.len().saturating_sub(1)) {
                    self.lower_stmt(&stmt.node, &mut block_terminated)?;
                }

                // Determine result from the last statement
                let (result, did_terminate) = if block_terminated {
                    // Already terminated (e.g., early return in non-last stmt)
                    (None, true)
                } else if let Some(last) = stmts.last() {
                    match &last.node {
                        Stmt::Expr(e) => (Some(self.lower_expr(&e.node)?), false),
                        Stmt::Return(_) => {
                            self.lower_stmt(&last.node, &mut block_terminated)?;
                            (None, true)
                        }
                        _ => {
                            self.lower_stmt(&last.node, &mut block_terminated)?;
                            if block_terminated {
                                (None, true)
                            } else {
                                (Some(self.builder.ins().iconst(types::I64, 0)), false)
                            }
                        }
                    }
                } else {
                    (Some(self.builder.ins().iconst(types::I64, 0)), false)
                };

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

                if did_terminate {
                    // Block terminated (e.g., return), don't jump to merge
                    self.builder.switch_to_block(merge_bb);
                    self.builder.seal_block(merge_bb);
                    return Ok(self.builder.block_params(merge_bb)[0]);
                }

                result.expect("catch block should produce a result value")
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

    fn lower_if_expr(
        &mut self,
        condition: &crate::span::Spanned<Expr>,
        then_block: &crate::span::Spanned<crate::parser::ast::Block>,
        else_block: &crate::span::Spanned<crate::parser::ast::Block>,
    ) -> Result<Value, CompileError> {
        let cond_val = self.lower_expr(&condition.node)?;

        // Infer the type of the if-expression
        let if_type = infer_type_for_expr_if(then_block, else_block, self.env, &self.var_types);
        let cl_type = pluto_to_cranelift(&if_type);

        // Create blocks
        let then_bb = self.builder.create_block();
        let else_bb = self.builder.create_block();
        let merge_bb = self.builder.create_block();

        // Add parameter to merge block to receive result
        self.builder.append_block_param(merge_bb, cl_type);

        // Branch based on condition
        self.builder.ins().brif(cond_val, then_bb, &[], else_bb, &[]);

        // Lower then branch
        self.builder.switch_to_block(then_bb);
        self.builder.seal_block(then_bb);
        let then_val = self.lower_block_value(&then_block.node)?;
        self.builder.ins().jump(merge_bb, &[then_val]);

        // Lower else branch
        self.builder.switch_to_block(else_bb);
        self.builder.seal_block(else_bb);
        let else_val = self.lower_block_value(&else_block.node)?;
        self.builder.ins().jump(merge_bb, &[else_val]);

        // Switch to merge block and get result from parameter
        self.builder.switch_to_block(merge_bb);
        self.builder.seal_block(merge_bb);

        Ok(self.builder.block_params(merge_bb)[0])
    }

    fn lower_block_value(
        &mut self,
        block: &crate::parser::ast::Block
    ) -> Result<Value, CompileError> {
        use crate::parser::ast::Stmt;

        if block.stmts.is_empty() {
            // Empty block → void → return 0
            return Ok(self.builder.ins().iconst(types::I64, 0));
        }

        // Lower all statements except the last
        let mut terminated = false;
        for stmt in &block.stmts[..block.stmts.len() - 1] {
            self.lower_stmt(&stmt.node, &mut terminated)?;
        }

        // Last statement determines the value
        let last = &block.stmts[block.stmts.len() - 1];
        match &last.node {
            Stmt::Expr(expr) => {
                // Last is an expression → return its value
                self.lower_expr(&expr.node)
            }
            Stmt::If { condition, then_block, else_block: Some(else_block) } => {
                // If-statement with else clause can act as an expression
                // Generate the same code as for Expr::If
                let cond_val = self.lower_expr(&condition.node)?;

                // Infer the type of the if branches
                let if_type = infer_type_for_expr_if(then_block, else_block, self.env, &self.var_types);
                let cl_type = pluto_to_cranelift(&if_type);

                // Create blocks
                let then_bb = self.builder.create_block();
                let else_bb = self.builder.create_block();
                let merge_bb = self.builder.create_block();

                // Add parameter to merge block to receive result
                self.builder.append_block_param(merge_bb, cl_type);

                // Branch based on condition
                self.builder.ins().brif(cond_val, then_bb, &[], else_bb, &[]);

                // Lower then branch
                self.builder.switch_to_block(then_bb);
                self.builder.seal_block(then_bb);
                let then_val = self.lower_block_value(&then_block.node)?;
                self.builder.ins().jump(merge_bb, &[then_val]);

                // Lower else branch
                self.builder.switch_to_block(else_bb);
                self.builder.seal_block(else_bb);
                let else_val = self.lower_block_value(&else_block.node)?;
                self.builder.ins().jump(merge_bb, &[else_val]);

                // Switch to merge block and get result from parameter
                self.builder.switch_to_block(merge_bb);
                self.builder.seal_block(merge_bb);

                Ok(self.builder.block_params(merge_bb)[0])
            }
            _ => {
                // Last is a statement → lower it, return void (0)
                self.lower_stmt(&last.node, &mut terminated)?;
                Ok(self.builder.ins().iconst(types::I64, 0))
            }
        }
    }

    fn lower_method_call(
        &mut self,
        object: &crate::span::Spanned<Expr>,
        method: &crate::span::Spanned<String>,
        args: &[crate::span::Spanned<Expr>],
    ) -> Result<Value, CompileError> {
        // Check for expect() intrinsic pattern
        if let Expr::Call { name, args: expect_args, .. } = &object.node
            && name.node == "expect" && expect_args.len() == 1
        {
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

        let obj_ptr = self.lower_expr(&object.node)?;
        let obj_type = infer_type_for_expr(&object.node, self.env, &self.var_types);

        // Task methods
        if let PlutoType::Task(inner) = &obj_type {
            match method.node.as_str() {
                "get" => {
                    let raw = self.call_runtime("__pluto_task_get", &[obj_ptr]);
                    return Ok(from_array_slot(raw, inner, &mut self.builder));
                }
                "detach" => {
                    self.call_runtime_void("__pluto_task_detach", &[obj_ptr]);
                    return Ok(self.builder.ins().iconst(types::I64, 0));
                }
                "cancel" => {
                    self.call_runtime_void("__pluto_task_cancel", &[obj_ptr]);
                    return Ok(self.builder.ins().iconst(types::I64, 0));
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
                    return Ok(from_array_slot(raw, inner, &mut self.builder));
                }
                "try_recv" => {
                    let raw = self.call_runtime("__pluto_chan_try_recv", &[obj_ptr]);
                    return Ok(from_array_slot(raw, inner, &mut self.builder));
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
                "pop" => {
                    let elem = elem.clone();
                    let raw = self.call_runtime("__pluto_array_pop", &[obj_ptr]);
                    return Ok(from_array_slot(raw, &elem, &mut self.builder));
                }
                "last" => {
                    let elem = elem.clone();
                    let raw = self.call_runtime("__pluto_array_last", &[obj_ptr]);
                    return Ok(from_array_slot(raw, &elem, &mut self.builder));
                }
                "first" => {
                    let elem = elem.clone();
                    let raw = self.call_runtime("__pluto_array_first", &[obj_ptr]);
                    return Ok(from_array_slot(raw, &elem, &mut self.builder));
                }
                "is_empty" => {
                    let len_val = self.call_runtime("__pluto_array_len", &[obj_ptr]);
                    let zero = self.builder.ins().iconst(types::I64, 0);
                    let cmp = self.builder.ins().icmp(IntCC::Equal, len_val, zero);
                    return Ok(cmp);
                }
                "clear" => {
                    self.call_runtime_void("__pluto_array_clear", &[obj_ptr]);
                    return Ok(self.builder.ins().iconst(types::I64, 0));
                }
                "remove_at" => {
                    let elem = elem.clone();
                    let idx = self.lower_expr(&args[0].node)?;
                    let raw = self.call_runtime("__pluto_array_remove_at", &[obj_ptr, idx]);
                    return Ok(from_array_slot(raw, &elem, &mut self.builder));
                }
                "insert_at" => {
                    let elem = elem.clone();
                    let idx = self.lower_expr(&args[0].node)?;
                    let arg_val = self.lower_expr(&args[1].node)?;
                    let slot = to_array_slot(arg_val, &elem, &mut self.builder);
                    self.call_runtime_void("__pluto_array_insert_at", &[obj_ptr, idx, slot]);
                    return Ok(self.builder.ins().iconst(types::I64, 0));
                }
                "slice" => {
                    let start = self.lower_expr(&args[0].node)?;
                    let end = self.lower_expr(&args[1].node)?;
                    return Ok(self.call_runtime("__pluto_array_slice", &[obj_ptr, start, end]));
                }
                "reverse" => {
                    self.call_runtime_void("__pluto_array_reverse", &[obj_ptr]);
                    return Ok(self.builder.ins().iconst(types::I64, 0));
                }
                "contains" => {
                    let elem = elem.clone();
                    let arg_val = self.lower_expr(&args[0].node)?;
                    let slot = to_array_slot(arg_val, &elem, &mut self.builder);
                    let tag = self.builder.ins().iconst(types::I64, key_type_tag(&elem));
                    let result = self.call_runtime("__pluto_array_contains", &[obj_ptr, slot, tag]);
                    return Ok(self.builder.ins().ireduce(types::I8, result));
                }
                "index_of" => {
                    let elem = elem.clone();
                    let arg_val = self.lower_expr(&args[0].node)?;
                    let slot = to_array_slot(arg_val, &elem, &mut self.builder);
                    let tag = self.builder.ins().iconst(types::I64, key_type_tag(&elem));
                    return Ok(self.call_runtime("__pluto_array_index_of", &[obj_ptr, slot, tag]));
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
                "byte_at" => {
                    let idx = self.lower_expr(&args[0].node)?;
                    Ok(self.call_runtime("__pluto_string_byte_at", &[obj_ptr, idx]))
                }
                "to_bytes" => Ok(self.call_runtime("__pluto_string_to_bytes", &[obj_ptr])),
                "to_int" => Ok(self.call_runtime("__pluto_string_to_int", &[obj_ptr])),
                "to_float" => Ok(self.call_runtime("__pluto_string_to_float", &[obj_ptr])),
                "trim_start" => Ok(self.call_runtime("__pluto_string_trim_start", &[obj_ptr])),
                "trim_end" => Ok(self.call_runtime("__pluto_string_trim_end", &[obj_ptr])),
                "repeat" => {
                    let count = self.lower_expr(&args[0].node)?;
                    Ok(self.call_runtime("__pluto_string_repeat", &[obj_ptr, count]))
                }
                "last_index_of" => {
                    let needle = self.lower_expr(&args[0].node)?;
                    Ok(self.call_runtime("__pluto_string_last_index_of", &[obj_ptr, needle]))
                }
                "count" => {
                    let needle = self.lower_expr(&args[0].node)?;
                    Ok(self.call_runtime("__pluto_string_count", &[obj_ptr, needle]))
                }
                "is_empty" => {
                    let result = self.call_runtime("__pluto_string_is_empty", &[obj_ptr]);
                    Ok(self.builder.ins().ireduce(types::I8, result))
                }

                "is_whitespace" => {
                    let result = self.call_runtime("__pluto_string_is_whitespace", &[obj_ptr]);
                    Ok(self.builder.ins().ireduce(types::I8, result))
                }
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

            // Check if this is a cross-stage RPC call
            // Only generate RPC if:
            //   1. The target class is a stage
            //   2. The object is NOT 'self' (to avoid RPC for same-stage calls)
            let is_self_call = matches!(&object.node, Expr::Ident(name) if name == "self");
            if self.is_stage(&class_name) && !is_self_call {
                return self.lower_rpc_call(&class_name, &method.node, args, object);
            }

            let mangled = mangle_method(&class_name, &method.node);
            let func_id = self.func_ids.get(&mangled).ok_or_else(|| {
                CompileError::codegen(format!("undefined method '{}'", method.node))
            })?;
            let func_ref = self.module.declare_func_in_func(*func_id, self.builder.func);

            // Look up the method signature to check parameter types
            let method_sig = self.env.functions.get(&mangled).cloned();

            let mut arg_values = vec![obj_ptr];
            for (i, arg) in args.iter().enumerate() {
                let val = self.lower_expr(&arg.node)?;
                let arg_type = infer_type_for_expr(&arg.node, self.env, &self.var_types);

                // Check if we're passing a class instance to a trait parameter
                let param_expected = method_sig.as_ref().and_then(|sig| sig.params.get(i + 1)); // +1 to skip self
                if let (PlutoType::Class(cn), Some(PlutoType::Trait(tn))) = (&arg_type, param_expected) {
                    // Wrap the class instance as a trait handle
                    let wrapped = self.wrap_class_as_trait(val, &cn, &tn)?;
                    arg_values.push(wrapped);
                } else {
                    arg_values.push(val);
                }
            }

            // Acquire rwlock if this singleton is synchronized (Phase 4b)
            let needs_sync = self.rwlock_globals.contains_key(&class_name);
            if needs_sync {
                let data_id = self.rwlock_globals[&class_name];
                let gv = self.module.declare_data_in_func(data_id, self.builder.func);
                let addr = self.builder.ins().global_value(types::I64, gv);
                let lock_ptr = self.builder.ins().load(types::I64, MemFlags::new(), addr, Offset32::new(0));
                if self.env.mut_self_methods.contains(&mangled) {
                    self.call_runtime_void("__pluto_rwlock_wrlock", &[lock_ptr]);
                } else {
                    self.call_runtime_void("__pluto_rwlock_rdlock", &[lock_ptr]);
                }
            }

            let call = self.builder.ins().call(func_ref, &arg_values);
            let results = self.builder.inst_results(call);
            let result = if results.is_empty() {
                self.builder.ins().iconst(types::I64, 0)
            } else {
                results[0]
            };

            // Emit invariant checks only after mut self methods — only mutations can break invariants
            // (runs inside lock scope so invariants are checked atomically)
            if self.env.mut_self_methods.contains(&mangled) {
                self.emit_invariant_checks(&class_name, obj_ptr)?;
            }

            // Release rwlock after method call + invariant checks
            if needs_sync {
                let data_id = self.rwlock_globals[&class_name];
                let gv = self.module.declare_data_in_func(data_id, self.builder.func);
                let addr = self.builder.ins().global_value(types::I64, gv);
                let lock_ptr = self.builder.ins().load(types::I64, MemFlags::new(), addr, Offset32::new(0));
                self.call_runtime_void("__pluto_rwlock_unlock", &[lock_ptr]);
            }

            Ok(result)
        } else {
            Err(CompileError::codegen(format!("method call on non-class type {obj_type}")))
        }
    }

    /// Generate RPC call for cross-stage method invocation.
    /// For MVP: supports simple types (int, string, bool, float) in args and return value.
    fn lower_rpc_call(
        &mut self,
        stage_name: &str,
        method_name: &str,
        args: &[crate::span::Spanned<Expr>],
        _object: &crate::span::Spanned<Expr>,
    ) -> Result<Value, CompileError> {
        // 1. Build endpoint URL: "http://localhost:8000/{stage_name}/{method_name}"
        let endpoint = format!("http://localhost:8000/{}/{}", stage_name, method_name);
        let endpoint_str = self.make_string_literal(&endpoint)?;

        // 2. Build JSON request body (simplified for MVP - just serialize args as JSON array)
        // For MVP: build a simple JSON array by concatenating strings
        // This is inefficient but works for prototype
        let mut body_val = self.make_string_literal("[")?;

        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                let comma = self.make_string_literal(",")?;
                body_val = self.call_runtime("__pluto_string_concat", &[body_val, comma]);
            }

            let arg_val = self.lower_expr(&arg.node)?;
            let arg_type = infer_type_for_expr(&arg.node, self.env, &self.var_types);

            let json_val = match arg_type {
                PlutoType::Int => self.call_runtime("__pluto_int_to_string", &[arg_val]),
                PlutoType::String => {
                    let quote = self.make_string_literal("\"")?;
                    let concat1 = self.call_runtime("__pluto_string_concat", &[quote, arg_val]);
                    self.call_runtime("__pluto_string_concat", &[concat1, quote])
                }
                PlutoType::Bool => {
                    let true_str = self.make_string_literal("true")?;
                    let false_str = self.make_string_literal("false")?;
                    let extended = self.builder.ins().uextend(types::I64, arg_val);
                    let zero = self.builder.ins().iconst(types::I64, 0);
                    let is_true = self.builder.ins().icmp(IntCC::NotEqual, extended, zero);
                    self.builder.ins().select(is_true, true_str, false_str)
                }
                PlutoType::Float => self.call_runtime("__pluto_float_to_string", &[arg_val]),
                _ => {
                    return Err(CompileError::codegen(format!(
                        "RPC MVP does not support argument type {:?}", arg_type
                    )));
                }
            };

            body_val = self.call_runtime("__pluto_string_concat", &[body_val, json_val]);
        }

        let close_bracket = self.make_string_literal("]")?;
        body_val = self.call_runtime("__pluto_string_concat", &[body_val, close_bracket]);

        // 3. Call std.rpc.http_post(endpoint, body)
        // This is an external function call to "rpc.http_post"
        let rpc_func_name = "rpc.http_post";
        let rpc_func_id = self.func_ids.get(rpc_func_name).ok_or_else(|| {
            CompileError::codegen(format!(
                "std.rpc module not found - ensure 'import std.rpc' is included"
            ))
        })?;
        let rpc_func_ref = self.module.declare_func_in_func(*rpc_func_id, self.builder.func);

        let call = self.builder.ins().call(rpc_func_ref, &[endpoint_str, body_val]);
        let response = self.builder.inst_results(call)[0];

        // 4. Unmarshal response based on method's return type
        // Look up the method signature to get the return type
        let mangled_method = mangle_method(stage_name, method_name);
        let return_type = self.env.functions.get(&mangled_method)
            .map(|sig| sig.return_type.clone())
            .ok_or_else(|| CompileError::codegen(format!(
                "Could not find method signature for {}", mangled_method
            )))?;

        // Extract the result value from the JSON response based on return type
        let result = match return_type {
            PlutoType::Int => {
                self.call_runtime("__pluto_rpc_extract_int", &[response])
            }
            PlutoType::Float => {
                self.call_runtime("__pluto_rpc_extract_float", &[response])
            }
            PlutoType::String => {
                self.call_runtime("__pluto_rpc_extract_string", &[response])
            }
            PlutoType::Bool => {
                let extracted = self.call_runtime("__pluto_rpc_extract_bool", &[response]);
                // Convert i64 to i8 for bool
                self.builder.ins().ireduce(types::I8, extracted)
            }
            PlutoType::Void => {
                // For void return, we don't need to extract anything
                let zero = self.builder.ins().iconst(types::I64, 0);
                zero
            }
            _ => {
                return Err(CompileError::codegen(format!(
                    "RPC MVP does not support return type {:?}", return_type
                )));
            }
        };

        Ok(result)
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
            PlutoType::Void | PlutoType::Class(_) | PlutoType::Array(_) | PlutoType::Trait(_) | PlutoType::Enum(_) | PlutoType::Fn(_, _) | PlutoType::Map(_, _) | PlutoType::Set(_) | PlutoType::Task(_) | PlutoType::Sender(_) | PlutoType::Receiver(_) | PlutoType::Range | PlutoType::Error | PlutoType::TypeParam(_) | PlutoType::Bytes | PlutoType::GenericInstance(_, _, _) | PlutoType::Nullable(_) | PlutoType::Stream(_) => {
                return Err(CompileError::codegen(format!("cannot print {arg_type}")));
            }
        }

        // print returns void, so return a dummy value
        Ok(self.builder.ins().iconst(types::I64, 0))
    }
}

/// Collect sender variable names from LetChan statements in a function body.
struct SenderVarCollector<'a> {
    names: &'a mut Vec<String>,
    seen: &'a mut HashSet<String>,
}

impl Visitor for SenderVarCollector<'_> {
    fn visit_stmt(&mut self, stmt: &crate::span::Spanned<Stmt>) {
        if let Stmt::LetChan { sender, .. } = &stmt.node {
            if self.seen.insert(sender.node.clone()) {
                self.names.push(sender.node.clone());
            }
        }
        walk_stmt(self, stmt);
    }
}

fn collect_sender_var_names(stmts: &[crate::span::Spanned<Stmt>]) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    let mut collector = SenderVarCollector {
        names: &mut names,
        seen: &mut seen,
    };
    for stmt in stmts {
        collector.visit_stmt(stmt);
    }
    names
}

/// Lower a function body into Cranelift IR./// Lower a function body into Cranelift IR.
#[allow(clippy::too_many_arguments)]
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
    fn_contracts: &HashMap<String, FnContracts>,
    singleton_globals: &HashMap<String, DataId>,
    rwlock_globals: &HashMap<String, DataId>,
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
    for (cranelift_param_idx, param) in func.params.iter().enumerate() {
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
                if matches!(cap_type, PlutoType::Sender(_))
                    && let Some(&var) = variables.get(cap_name)
                {
                    sender_cleanup_vars.push(var);
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
            mangle_method(cn, &func.name.node)
        } else {
            func.name.node.clone()
        };
        env.functions.get(&lookup_name).map(|s| s.return_type.clone())
    };

    let is_spawn_closure = spawn_closure_fns.contains(&func.name.node);

    // Create exit block if we have sender cleanup vars
    let exit_block = if !sender_cleanup_vars.is_empty() {
        let exit_bb = builder.create_block();
        // Add return value as block param if function returns non-void
        let is_void_return = matches!(&expected_return_type, Some(PlutoType::Void) | None);
        if !is_void_return {
            // Spawn closures return I64 regardless of actual type
            let ret_cl_type = if is_spawn_closure {
                types::I64
            } else {
                pluto_to_cranelift(expected_return_type.as_ref()
                    .expect("non-void return type should be set"))
            };
            builder.append_block_param(exit_bb, ret_cl_type);
        }
        Some(exit_bb)
    } else {
        None
    };

    // Compute function lookup name (mangled for methods)
    let fn_lookup = if let Some(cn) = class_name {
        mangle_method(cn, &func.name.node)
    } else {
        func.name.node.clone()
    };

    // Display name for contract violation messages
    let fn_display_name = fn_lookup.clone();

    // Check if this function has ensures contracts — if so, create ensures_block
    let is_void_return = matches!(&expected_return_type, Some(PlutoType::Void) | None);
    let has_ensures = fn_contracts.get(&fn_lookup).is_some_and(|c| !c.ensures.is_empty());
    let ensures_block = if has_ensures {
        let ens_bb = builder.create_block();
        if !is_void_return {
            let ret_cl_type = if is_spawn_closure {
                types::I64
            } else {
                pluto_to_cranelift(expected_return_type.as_ref()
                    .expect("non-void return type should be set"))
            };
            builder.append_block_param(ens_bb, ret_cl_type);
        }
        Some(ens_bb)
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
        fn_contracts,
        singleton_globals,
        rwlock_globals,
        variables,
        var_types,
        next_var,
        expected_return_type,
        loop_stack: Vec::new(),
        sender_cleanup_vars,
        exit_block,
        old_snapshots: HashMap::new(),
        ensures_block,
        fn_display_name,
        is_spawn_closure,
    };

    // Initialize GC at start of non-app main
    if is_main {
        ctx.call_runtime_void("__pluto_gc_init", &[]);
    }

    // Emit requires checks at function entry
    if let Some(contracts) = fn_contracts.get(&fn_lookup) {
        if !contracts.requires.is_empty() {
            let requires = contracts.requires.clone();
            ctx.emit_requires_checks(&requires)?;
        }

        // Compute old() snapshots for ensures clauses
        if !contracts.ensures.is_empty() {
            let mut old_exprs: Vec<(Expr, String)> = Vec::new();
            for (ens_expr, _) in &contracts.ensures {
                LowerContext::collect_old_exprs(ens_expr, &mut old_exprs);
            }
            // Deduplicate by description key
            let mut seen = HashSet::new();
            let unique_old_exprs: Vec<(Expr, String)> = old_exprs.into_iter()
                .filter(|(_, desc)| seen.insert(desc.clone()))
                .collect();
            for (old_inner_expr, desc) in &unique_old_exprs {
                let snapshot_val = ctx.lower_expr(old_inner_expr)?;
                let old_inner_type = infer_type_for_expr(old_inner_expr, ctx.env, &ctx.var_types);
                let var = Variable::from_u32(ctx.next_var);
                ctx.next_var += 1;
                ctx.builder.declare_var(var, pluto_to_cranelift(&old_inner_type));
                ctx.builder.def_var(var, snapshot_val);
                ctx.old_snapshots.insert(desc.clone(), var);
            }
        }
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
        let target = ctx.ensures_block.or(ctx.exit_block);
        if let Some(bb) = target {
            ctx.builder.ins().jump(bb, &[zero]);
        } else {
            ctx.builder.ins().return_(&[zero]);
        }
    } else if !terminated {
        // Void function with no return
        let ret_type = ctx.env.functions.get(&fn_lookup).map(|s| &s.return_type);
        if ret_type == Some(&PlutoType::Void) {
            let target = ctx.ensures_block.or(ctx.exit_block);
            if let Some(bb) = target {
                ctx.builder.ins().jump(bb, &[]);
            } else {
                ctx.builder.ins().return_(&[]);
            }
        }
    }

    // Emit ensures block: ensures checks, then jump to exit_block or return
    if let Some(ens_bb) = ctx.ensures_block {
        ctx.builder.switch_to_block(ens_bb);
        ctx.builder.seal_block(ens_bb);

        // Get the result variable (block param for non-void functions)
        let result_var = if !is_void_return {
            let ret_val = ctx.builder.block_params(ens_bb)[0];
            let var = Variable::from_u32(ctx.next_var);
            ctx.next_var += 1;
            let ret_cl_type = if ctx.is_spawn_closure {
                types::I64
            } else {
                pluto_to_cranelift(ctx.expected_return_type.as_ref()
                    .expect("non-void return type should be set"))
            };
            ctx.builder.declare_var(var, ret_cl_type);
            ctx.builder.def_var(var, ret_val);
            Some(var)
        } else {
            None
        };

        let ensures = fn_contracts.get(&fn_lookup)
            .expect("function should have contracts entry")
            .ensures.clone();
        ctx.emit_ensures_checks(&ensures, result_var)?;

        // After ensures pass, jump to exit_block or return directly
        if let Some(exit_bb) = ctx.exit_block {
            if !is_void_return {
                let ret_val = ctx.builder.use_var(result_var.expect("result_var should be set for non-void returns"));
                ctx.builder.ins().jump(exit_bb, &[ret_val]);
            } else {
                ctx.builder.ins().jump(exit_bb, &[]);
            }
        } else if !is_void_return {
            let ret_val = ctx.builder.use_var(result_var.expect("result_var should be set for non-void returns"));
            ctx.builder.ins().return_(&[ret_val]);
        } else {
            ctx.builder.ins().return_(&[]);
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

// ── Generator codegen ────────────────────────────────────────────────────

/// Count yield points in a block (recursively enters if/while/for/match bodies).
struct YieldCounter {
    count: u32,
}

impl Visitor for YieldCounter {
    fn visit_stmt(&mut self, stmt: &crate::span::Spanned<Stmt>) {
        if let Stmt::Yield { .. } = &stmt.node {
            self.count += 1;
        }
        walk_stmt(self, stmt);
    }
}

fn count_yields_in_block(stmts: &[crate::span::Spanned<Stmt>]) -> u32 {
    let mut counter = YieldCounter { count: 0 };
    for stmt in stmts {
        counter.visit_stmt(stmt);
    }
    counter.count
}

/// Collect all local variable declarations in a function body.
/// Returns (name, type) pairs. Walks into if/while/for/match bodies.
struct LocalDeclCollector<'a> {
    env: &'a TypeEnv,
    locals: &'a mut Vec<(String, PlutoType)>,
    seen: &'a mut HashSet<String>,
}

impl Visitor for LocalDeclCollector<'_> {
    fn visit_stmt(&mut self, stmt: &crate::span::Spanned<Stmt>) {
        match &stmt.node {
            Stmt::Let { name, ty, value, .. } => {
                if self.seen.insert(name.node.clone()) {
                    let pty = if let Some(t) = ty {
                        resolve_type_expr_to_pluto(&t.node, self.env)
                    } else {
                        infer_type_for_expr(&value.node, self.env, &HashMap::new())
                    };
                    self.locals.push((name.node.clone(), pty));
                }
            }
            Stmt::For { var, iterable, body, .. } => {
                if self.seen.insert(var.node.clone()) {
                    let iter_type = infer_type_for_expr(&iterable.node, self.env, &HashMap::new());
                    let elem_type = match iter_type {
                        PlutoType::Array(e) => *e,
                        PlutoType::Range => PlutoType::Int,
                        PlutoType::String => PlutoType::String,
                        PlutoType::Bytes => PlutoType::Byte,
                        PlutoType::Receiver(e) => *e,
                        PlutoType::Stream(e) => *e,
                        _ => PlutoType::Int,
                    };
                    self.locals.push((var.node.clone(), elem_type));
                }
                // Manually visit the body
                for s in &body.node.stmts {
                    self.visit_stmt(s);
                }
                return;
            }
            _ => {}
        }
        walk_stmt(self, stmt);
    }
}

fn collect_local_decls(stmts: &[crate::span::Spanned<Stmt>], env: &TypeEnv) -> Vec<(String, PlutoType)> {
    let mut locals = Vec::new();
    let mut seen = HashSet::new();
    let mut collector = LocalDeclCollector {
        env,
        locals: &mut locals,
        seen: &mut seen,
    };
    for stmt in stmts {
        collector.visit_stmt(stmt);
    }
    locals
}

/// Lower the generator creator function./// Lower the generator creator function.
/// Allocates a generator object with [next_fn_ptr, state, done, result, params..., locals...]
/// and stores the next function pointer + initial parameter values.
#[allow(clippy::too_many_arguments)]
pub fn lower_generator_creator(
    func: &Function,
    mut builder: FunctionBuilder<'_>,
    env: &TypeEnv,
    module: &mut dyn Module,
    func_ids: &HashMap<String, FuncId>,
    runtime: &RuntimeRegistry,
) -> Result<(), CompileError> {
    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);
    builder.seal_block(entry_block);

    let num_params = func.params.len();
    let local_decls = collect_local_decls(&func.body.node.stmts, env);
    let num_locals = local_decls.len();

    // Layout: [next_fn_ptr(0) | state(8) | done(16) | result(24) | params(32..) | locals(32+P*8..)]
    let total_slots = 4 + num_params + num_locals;
    let alloc_size = (total_slots as i64) * POINTER_SIZE as i64;

    // Allocate generator object
    let size_val = builder.ins().iconst(types::I64, alloc_size);
    let alloc_ref = module.declare_func_in_func(runtime.get("__pluto_alloc"), builder.func);
    let call = builder.ins().call(alloc_ref, &[size_val]);
    let gen_ptr = builder.inst_results(call)[0];

    // Store next function pointer at offset 0
    let next_name = format!("__gen_next_{}", func.name.node);
    let next_func_id = func_ids.get(&next_name).ok_or_else(|| {
        CompileError::codegen(format!("missing generator next function '{next_name}'"))
    })?;
    let next_func_ref = module.declare_func_in_func(*next_func_id, builder.func);
    let next_fn_addr = builder.ins().func_addr(types::I64, next_func_ref);
    builder.ins().store(MemFlags::new(), next_fn_addr, gen_ptr, Offset32::new(0));

    // Store state = 0 at offset 8
    let zero = builder.ins().iconst(types::I64, 0);
    builder.ins().store(MemFlags::new(), zero, gen_ptr, Offset32::new(8));

    // Store done = 0 at offset 16
    builder.ins().store(MemFlags::new(), zero, gen_ptr, Offset32::new(16));

    // Store params at offsets 32+
    for (i, _param) in func.params.iter().enumerate() {
        let param_val = builder.block_params(entry_block)[i];
        let param_type = resolve_type_expr_to_pluto(&func.params[i].ty.node, env);
        let slot_val = to_array_slot(param_val, &param_type, &mut builder);
        let offset = (4 + i) as i32 * POINTER_SIZE;
        builder.ins().store(MemFlags::new(), slot_val, gen_ptr, Offset32::new(offset));
    }

    // Initialize local slots to 0
    for i in 0..num_locals {
        let offset = (4 + num_params + i) as i32 * POINTER_SIZE;
        builder.ins().store(MemFlags::new(), zero, gen_ptr, Offset32::new(offset));
    }

    // Return gen_ptr
    builder.ins().return_(&[gen_ptr]);
    builder.finalize();
    Ok(())
}

/// Lower the generator next function (state machine).
/// Takes gen_ptr as the single parameter. On each call:
/// - Dispatches to the correct resume point based on state
/// - Executes until the next yield (stores result, saves locals, sets next state)
/// - Or runs to completion (sets done flag)
#[allow(clippy::too_many_arguments)]
pub fn lower_generator_next(
    func: &Function,
    mut builder: FunctionBuilder<'_>,
    env: &TypeEnv,
    module: &mut dyn Module,
    func_ids: &HashMap<String, FuncId>,
    runtime: &RuntimeRegistry,
    vtable_ids: &HashMap<(String, String), DataId>,
    source: &str,
    class_invariants: &HashMap<String, Vec<(Expr, String)>>,
    fn_contracts: &HashMap<String, FnContracts>,
    singleton_globals: &HashMap<String, DataId>,
    rwlock_globals: &HashMap<String, DataId>,
) -> Result<(), CompileError> {
    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);

    let num_params = func.params.len();
    let local_decls = collect_local_decls(&func.body.node.stmts, env);
    let _num_locals = local_decls.len();
    let num_yields = count_yields_in_block(&func.body.node.stmts);

    // gen_ptr is the single parameter
    let gen_ptr_val = builder.block_params(entry_block)[0];
    let gen_ptr_var = Variable::from_u32(0);
    builder.declare_var(gen_ptr_var, types::I64);
    builder.def_var(gen_ptr_var, gen_ptr_val);

    // Create dispatch blocks
    let state_0_bb = builder.create_block();
    let done_bb = builder.create_block();
    let mut resume_blocks = Vec::new();
    for _ in 0..num_yields {
        resume_blocks.push(builder.create_block());
    }

    // Dispatch: load state, branch to appropriate block
    let gen_ptr = builder.use_var(gen_ptr_var);
    let state = builder.ins().load(types::I64, MemFlags::new(), gen_ptr, Offset32::new(8));

    // Chain of brif comparisons: if state == 0 → state_0, if state == 1 → resume_1, ...
    let zero = builder.ins().iconst(types::I64, 0);
    let is_state_0 = builder.ins().icmp(IntCC::Equal, state, zero);
    if num_yields > 0 {
        let mut next_check = builder.create_block();
        builder.ins().brif(is_state_0, state_0_bb, &[], next_check, &[]);

        // Generate comparison chain for each resume block
        for (i, resume_bb) in resume_blocks.iter().enumerate() {
            builder.switch_to_block(next_check);
            builder.seal_block(next_check);
            let state_val = builder.ins().iconst(types::I64, (i + 1) as i64);
            let gen_ptr = builder.use_var(gen_ptr_var);
            let state = builder.ins().load(types::I64, MemFlags::new(), gen_ptr, Offset32::new(8));
            let is_this_state = builder.ins().icmp(IntCC::Equal, state, state_val);
            if i + 1 < resume_blocks.len() {
                let next_next_check = builder.create_block();
                builder.ins().brif(is_this_state, *resume_bb, &[], next_next_check, &[]);
                next_check = next_next_check;
            } else {
                builder.ins().brif(is_this_state, *resume_bb, &[], done_bb, &[]);
            }
        }
    } else {
        builder.ins().brif(is_state_0, state_0_bb, &[], done_bb, &[]);
    }

    builder.seal_block(entry_block);

    // Done block: set done flag and return
    builder.switch_to_block(done_bb);
    builder.seal_block(done_bb);
    let gen_ptr = builder.use_var(gen_ptr_var);
    let one = builder.ins().iconst(types::I64, 1);
    builder.ins().store(MemFlags::new(), one, gen_ptr, Offset32::new(16));
    builder.ins().return_(&[]);

    // State 0: load params from gen object, then execute body
    builder.switch_to_block(state_0_bb);
    builder.seal_block(state_0_bb);

    // Set up LowerContext for the generator body
    let mut variables = HashMap::new();
    let mut var_types = HashMap::new();
    let mut next_var_id = 1u32; // 0 is gen_ptr_var

    // Load params from gen object and build param_slots for save/restore across yields
    let mut param_slots: Vec<(String, PlutoType, Variable)> = Vec::new();
    for (i, param) in func.params.iter().enumerate() {
        let param_type = resolve_type_expr_to_pluto(&param.ty.node, env);
        let gen_ptr = builder.use_var(gen_ptr_var);
        let offset = (4 + i) as i32 * POINTER_SIZE;
        let raw = builder.ins().load(types::I64, MemFlags::new(), gen_ptr, Offset32::new(offset));
        let val = from_array_slot(raw, &param_type, &mut builder);

        let var = Variable::from_u32(next_var_id);
        next_var_id += 1;
        builder.declare_var(var, pluto_to_cranelift(&param_type));
        builder.def_var(var, val);
        variables.insert(param.name.node.clone(), var);
        var_types.insert(param.name.node.clone(), param_type.clone());
        param_slots.push((param.name.node.clone(), param_type, var));
    }

    // Pre-declare all local variables (so they exist across all yield points)
    let local_slots: Vec<(String, PlutoType, Variable)> = local_decls.iter().enumerate().map(|(_i, (name, ty))| {
        let var = Variable::from_u32(next_var_id);
        next_var_id += 1;
        builder.declare_var(var, pluto_to_cranelift(ty));
        // Initialize to zero
        let init_val = match ty {
            PlutoType::Float => builder.ins().f64const(0.0),
            PlutoType::Bool | PlutoType::Byte => builder.ins().iconst(types::I8, 0),
            _ => builder.ins().iconst(types::I64, 0),
        };
        builder.def_var(var, init_val);
        variables.insert(name.clone(), var);
        var_types.insert(name.clone(), ty.clone());
        (name.clone(), ty.clone(), var)
    }).collect();

    // Build context
    let mut ctx = LowerContext {
        builder,
        module,
        env,
        func_ids,
        runtime,
        vtable_ids,
        source,
        class_invariants,
        fn_contracts,
        singleton_globals,
        rwlock_globals,
        variables,
        var_types,
        next_var: next_var_id,
        expected_return_type: Some(PlutoType::Void),
        loop_stack: Vec::new(),
        sender_cleanup_vars: Vec::new(),
        exit_block: None,
        old_snapshots: HashMap::new(),
        ensures_block: None,
        fn_display_name: func.name.node.clone(),
        is_spawn_closure: false,
    };

    // Generator-specific state
    let mut yield_counter = 0u32;

    // Lower the body with generator-aware statement handling
    let mut terminated = false;
    lower_generator_block(
        &func.body.node.stmts,
        &mut ctx,
        &mut terminated,
        &mut yield_counter,
        &resume_blocks,
        &param_slots,
        &local_slots,
        num_params,
        gen_ptr_var,
        done_bb,
    )?;

    // If body didn't terminate, set done and return
    if !terminated {
        let gen_ptr = ctx.builder.use_var(gen_ptr_var);
        let one = ctx.builder.ins().iconst(types::I64, 1);
        ctx.builder.ins().store(MemFlags::new(), one, gen_ptr, Offset32::new(16));
        ctx.builder.ins().return_(&[]);
    }

    // Fill any unused resume blocks (e.g., yields after an early return are dead code,
    // but we pre-created resume blocks for them — they need valid terminators).
    for i in (yield_counter as usize)..resume_blocks.len() {
        ctx.builder.switch_to_block(resume_blocks[i]);
        ctx.builder.seal_block(resume_blocks[i]);
        let gen_ptr = ctx.builder.use_var(gen_ptr_var);
        let one = ctx.builder.ins().iconst(types::I64, 1);
        ctx.builder.ins().store(MemFlags::new(), one, gen_ptr, Offset32::new(16));
        ctx.builder.ins().return_(&[]);
    }

    ctx.finalize();
    Ok(())
}

/// Lower a block of statements inside a generator, handling Yield specially.
fn lower_generator_block(
    stmts: &[crate::span::Spanned<Stmt>],
    ctx: &mut LowerContext<'_>,
    terminated: &mut bool,
    yield_counter: &mut u32,
    resume_blocks: &[cranelift_codegen::ir::Block],
    param_slots: &[(String, PlutoType, Variable)],
    local_slots: &[(String, PlutoType, Variable)],
    num_params: usize,
    gen_ptr_var: Variable,
    done_bb: cranelift_codegen::ir::Block,
) -> Result<(), CompileError> {
    for stmt in stmts {
        if *terminated {
            break;
        }
        match &stmt.node {
            Stmt::Yield { value } => {
                // 1. Lower the yield value expression
                let val = ctx.lower_expr(&value.node)?;
                let val_type = infer_type_for_expr(&value.node, ctx.env, &ctx.var_types);

                // 2. Store result at gen_ptr[24]
                let slot_val = to_array_slot(val, &val_type, &mut ctx.builder);
                let gen_ptr = ctx.builder.use_var(gen_ptr_var);
                ctx.builder.ins().store(MemFlags::new(), slot_val, gen_ptr, Offset32::new(24));

                // 3. Save params to gen object (needed for resume across yield points)
                for (i, (_, ty, var)) in param_slots.iter().enumerate() {
                    let param_val = ctx.builder.use_var(*var);
                    let slot = to_array_slot(param_val, ty, &mut ctx.builder);
                    let offset = (4 + i) as i32 * POINTER_SIZE;
                    let gen_ptr = ctx.builder.use_var(gen_ptr_var);
                    ctx.builder.ins().store(MemFlags::new(), slot, gen_ptr, Offset32::new(offset));
                }

                // 4. Save ALL locals to gen object
                for (i, (_, ty, var)) in local_slots.iter().enumerate() {
                    let local_val = ctx.builder.use_var(*var);
                    let slot = to_array_slot(local_val, ty, &mut ctx.builder);
                    let offset = (4 + num_params + i) as i32 * POINTER_SIZE;
                    let gen_ptr = ctx.builder.use_var(gen_ptr_var);
                    ctx.builder.ins().store(MemFlags::new(), slot, gen_ptr, Offset32::new(offset));
                }

                // 5. Store next state
                let yield_idx = *yield_counter;
                *yield_counter += 1;
                let next_state = ctx.builder.ins().iconst(types::I64, (yield_idx + 1) as i64);
                let gen_ptr = ctx.builder.use_var(gen_ptr_var);
                ctx.builder.ins().store(MemFlags::new(), next_state, gen_ptr, Offset32::new(8));

                // 6. Return (yield point)
                ctx.builder.ins().return_(&[]);

                // 7. Switch to the resume block for this yield
                let resume_bb = resume_blocks[yield_idx as usize];
                ctx.builder.switch_to_block(resume_bb);
                ctx.builder.seal_block(resume_bb);

                // 8. Restore params from gen object
                for (i, (_, ty, var)) in param_slots.iter().enumerate() {
                    let offset = (4 + i) as i32 * POINTER_SIZE;
                    let gen_ptr = ctx.builder.use_var(gen_ptr_var);
                    let raw = ctx.builder.ins().load(types::I64, MemFlags::new(), gen_ptr, Offset32::new(offset));
                    let val = from_array_slot(raw, ty, &mut ctx.builder);
                    ctx.builder.def_var(*var, val);
                }

                // 9. Restore ALL locals from gen object
                for (i, (_, ty, var)) in local_slots.iter().enumerate() {
                    let offset = (4 + num_params + i) as i32 * POINTER_SIZE;
                    let gen_ptr = ctx.builder.use_var(gen_ptr_var);
                    let raw = ctx.builder.ins().load(types::I64, MemFlags::new(), gen_ptr, Offset32::new(offset));
                    let val = from_array_slot(raw, ty, &mut ctx.builder);
                    ctx.builder.def_var(*var, val);
                }
            }
            Stmt::Return(_) => {
                // Bare return in generator means "done"
                let gen_ptr = ctx.builder.use_var(gen_ptr_var);
                let one = ctx.builder.ins().iconst(types::I64, 1);
                ctx.builder.ins().store(MemFlags::new(), one, gen_ptr, Offset32::new(16));
                ctx.builder.ins().return_(&[]);
                *terminated = true;
            }
            Stmt::Let { name, value, ty, .. } => {
                // In generators, local variables are pre-declared in local_slots.
                // We must NOT call lower_let() because it creates a new Cranelift Variable,
                // which would shadow the pre-declared one used for save/restore across yields.
                // Instead, evaluate the value and def_var on the existing pre-declared variable.
                let val = ctx.lower_expr(&value.node)?;
                let val_type = infer_type_for_expr(&value.node, ctx.env, &ctx.var_types);

                // Handle type coercions (trait wrapping, nullable boxing)
                let declared_type = ty.as_ref().map(|t| resolve_type_expr_to_pluto(&t.node, ctx.env));
                let final_val = match (&val_type, &declared_type) {
                    (PlutoType::Class(cn), Some(PlutoType::Trait(tn))) => {
                        ctx.wrap_class_as_trait(val, cn, tn)?
                    }
                    (inner, Some(PlutoType::Nullable(expected_inner))) if !matches!(inner, PlutoType::Nullable(_)) && **expected_inner != PlutoType::Void => {
                        ctx.emit_nullable_wrap(val, inner)
                    }
                    _ => val,
                };

                // Use the pre-declared variable from local_slots
                let var = ctx.variables.get(&name.node).ok_or_else(|| {
                    CompileError::codegen(format!("generator local variable '{}' not found in pre-declared slots", name.node))
                })?;
                ctx.builder.def_var(*var, final_val);
            }
            Stmt::If { condition, then_block, else_block } => {
                lower_generator_if(ctx, condition, then_block, else_block.as_ref(), terminated, yield_counter, resume_blocks, param_slots, local_slots, num_params, gen_ptr_var, done_bb)?;
            }
            Stmt::While { condition, body } => {
                lower_generator_while(ctx, condition, body, terminated, yield_counter, resume_blocks, param_slots, local_slots, num_params, gen_ptr_var, done_bb)?;
            }
            Stmt::For { var, iterable, body } => {
                lower_generator_for(ctx, var, iterable, body, terminated, yield_counter, resume_blocks, param_slots, local_slots, num_params, gen_ptr_var, done_bb)?;
            }
            _ => {
                // For all other statements, delegate to the normal lower_stmt
                ctx.lower_stmt(&stmt.node, terminated)?;
            }
        }
    }
    Ok(())
}

/// Lower an if statement inside a generator body, handling yields in branches.
#[allow(clippy::too_many_arguments)]
fn lower_generator_if(
    ctx: &mut LowerContext<'_>,
    condition: &crate::span::Spanned<Expr>,
    then_body: &crate::span::Spanned<Block>,
    else_body: Option<&crate::span::Spanned<Block>>,
    terminated: &mut bool,
    yield_counter: &mut u32,
    resume_blocks: &[cranelift_codegen::ir::Block],
    param_slots: &[(String, PlutoType, Variable)],
    local_slots: &[(String, PlutoType, Variable)],
    num_params: usize,
    gen_ptr_var: Variable,
    done_bb: cranelift_codegen::ir::Block,
) -> Result<(), CompileError> {
    let cond_val = ctx.lower_expr(&condition.node)?;
    let cond_type = infer_type_for_expr(&condition.node, ctx.env, &ctx.var_types);
    let cond_i8 = if cond_type == PlutoType::Bool {
        cond_val
    } else {
        let zero = ctx.builder.ins().iconst(types::I64, 0);
        ctx.builder.ins().icmp(IntCC::NotEqual, cond_val, zero)
    };

    let then_bb = ctx.builder.create_block();
    let merge_bb = ctx.builder.create_block();
    let else_bb = if else_body.is_some() {
        ctx.builder.create_block()
    } else {
        merge_bb
    };

    ctx.builder.ins().brif(cond_i8, then_bb, &[], else_bb, &[]);

    // Then branch
    ctx.builder.switch_to_block(then_bb);
    ctx.builder.seal_block(then_bb);
    let mut then_terminated = false;
    lower_generator_block(
        &then_body.node.stmts, ctx, &mut then_terminated, yield_counter,
        resume_blocks, param_slots, local_slots, num_params, gen_ptr_var, done_bb,
    )?;
    if !then_terminated {
        ctx.builder.ins().jump(merge_bb, &[]);
    }

    // Else branch
    if let Some(eb) = else_body {
        ctx.builder.switch_to_block(else_bb);
        ctx.builder.seal_block(else_bb);
        let mut else_terminated = false;
        lower_generator_block(
            &eb.node.stmts, ctx, &mut else_terminated, yield_counter,
            resume_blocks, param_slots, local_slots, num_params, gen_ptr_var, done_bb,
        )?;
        if !else_terminated {
            ctx.builder.ins().jump(merge_bb, &[]);
        }
        *terminated = then_terminated && else_terminated;
    } else {
        *terminated = false;
    }

    ctx.builder.switch_to_block(merge_bb);
    ctx.builder.seal_block(merge_bb);
    Ok(())
}

/// Lower a while loop inside a generator body, handling yields in the loop body.
#[allow(clippy::too_many_arguments)]
fn lower_generator_while(
    ctx: &mut LowerContext<'_>,
    condition: &crate::span::Spanned<Expr>,
    body: &crate::span::Spanned<Block>,
    _terminated: &mut bool,
    yield_counter: &mut u32,
    resume_blocks: &[cranelift_codegen::ir::Block],
    param_slots: &[(String, PlutoType, Variable)],
    local_slots: &[(String, PlutoType, Variable)],
    num_params: usize,
    gen_ptr_var: Variable,
    done_bb: cranelift_codegen::ir::Block,
) -> Result<(), CompileError> {
    let header_bb = ctx.builder.create_block();
    let body_bb = ctx.builder.create_block();
    let exit_bb = ctx.builder.create_block();

    ctx.builder.ins().jump(header_bb, &[]);

    // Header: evaluate condition
    ctx.builder.switch_to_block(header_bb);
    let cond_val = ctx.lower_expr(&condition.node)?;
    let cond_type = infer_type_for_expr(&condition.node, ctx.env, &ctx.var_types);
    let cond_i8 = if cond_type == PlutoType::Bool {
        cond_val
    } else {
        let zero = ctx.builder.ins().iconst(types::I64, 0);
        ctx.builder.ins().icmp(IntCC::NotEqual, cond_val, zero)
    };
    ctx.builder.ins().brif(cond_i8, body_bb, &[], exit_bb, &[]);

    // Body
    ctx.builder.switch_to_block(body_bb);
    ctx.builder.seal_block(body_bb);

    ctx.loop_stack.push((header_bb, exit_bb));
    let mut body_terminated = false;
    lower_generator_block(
        &body.node.stmts, ctx, &mut body_terminated, yield_counter,
        resume_blocks, param_slots, local_slots, num_params, gen_ptr_var, done_bb,
    )?;
    ctx.loop_stack.pop();

    if !body_terminated {
        ctx.builder.ins().jump(header_bb, &[]);
    }

    ctx.builder.seal_block(header_bb);
    ctx.builder.switch_to_block(exit_bb);
    ctx.builder.seal_block(exit_bb);
    Ok(())
}

/// Lower a for loop inside a generator body, handling yields in the loop body.
#[allow(clippy::too_many_arguments)]
fn lower_generator_for(
    ctx: &mut LowerContext<'_>,
    var: &crate::span::Spanned<String>,
    iterable: &crate::span::Spanned<Expr>,
    body: &crate::span::Spanned<Block>,
    _terminated: &mut bool,
    yield_counter: &mut u32,
    resume_blocks: &[cranelift_codegen::ir::Block],
    param_slots: &[(String, PlutoType, Variable)],
    local_slots: &[(String, PlutoType, Variable)],
    num_params: usize,
    gen_ptr_var: Variable,
    done_bb: cranelift_codegen::ir::Block,
) -> Result<(), CompileError> {
    // For loops in generators: delegate to normal for-loop lowering for the iteration
    // mechanism, but the body needs generator-aware handling.
    // For simplicity in Phase 1, we only support range-based for loops in generators
    // with yields. The for-loop variable is already pre-declared as a local slot.

    let iter_type = infer_type_for_expr(&iterable.node, ctx.env, &ctx.var_types);

    match &iter_type {
        PlutoType::Range => {
            // Lower range for loop with generator-aware body
            let range_val = ctx.lower_expr(&iterable.node)?;

            // Extract start and end from Range struct
            let start = ctx.builder.ins().load(types::I64, MemFlags::new(), range_val, Offset32::new(0));
            let end = ctx.builder.ins().load(types::I64, MemFlags::new(), range_val, Offset32::new(8));

            // The loop variable should already exist in variables from local_slots
            let loop_var = *ctx.variables.get(&var.node).ok_or_else(|| {
                CompileError::codegen(format!("generator for-loop variable '{}' not found", var.node))
            })?;
            ctx.builder.def_var(loop_var, start);

            let header_bb = ctx.builder.create_block();
            let body_bb = ctx.builder.create_block();
            let exit_bb = ctx.builder.create_block();

            ctx.builder.ins().jump(header_bb, &[]);

            // Header: check i < end
            ctx.builder.switch_to_block(header_bb);
            let i_val = ctx.builder.use_var(loop_var);
            let cond = ctx.builder.ins().icmp(IntCC::SignedLessThan, i_val, end);
            ctx.builder.ins().brif(cond, body_bb, &[], exit_bb, &[]);

            // Body
            ctx.builder.switch_to_block(body_bb);
            ctx.builder.seal_block(body_bb);

            ctx.loop_stack.push((header_bb, exit_bb));
            let mut body_terminated = false;
            lower_generator_block(
                &body.node.stmts, ctx, &mut body_terminated, yield_counter,
                resume_blocks, param_slots, local_slots, num_params, gen_ptr_var, done_bb,
            )?;
            ctx.loop_stack.pop();

            if !body_terminated {
                // Increment loop variable
                let i_val = ctx.builder.use_var(loop_var);
                let one = ctx.builder.ins().iconst(types::I64, 1);
                let next_i = ctx.builder.ins().iadd(i_val, one);
                ctx.builder.def_var(loop_var, next_i);
                ctx.builder.ins().jump(header_bb, &[]);
            }

            ctx.builder.seal_block(header_bb);
            ctx.builder.switch_to_block(exit_bb);
            ctx.builder.seal_block(exit_bb);
            Ok(())
        }
        PlutoType::Array(_) => {
            // Array for loop: use index-based iteration
            let arr_val = ctx.lower_expr(&iterable.node)?;
            let elem_type = match &iter_type {
                PlutoType::Array(e) => *e.clone(),
                _ => unreachable!(),
            };

            // Get array length
            let len_ref = ctx.module.declare_func_in_func(ctx.runtime.get("__pluto_array_len"), ctx.builder.func);
            let call = ctx.builder.ins().call(len_ref, &[arr_val]);
            let len = ctx.builder.inst_results(call)[0];

            // Index variable
            let idx_var = Variable::from_u32(ctx.next_var);
            ctx.next_var += 1;
            ctx.builder.declare_var(idx_var, types::I64);
            let zero = ctx.builder.ins().iconst(types::I64, 0);
            ctx.builder.def_var(idx_var, zero);

            let loop_var = *ctx.variables.get(&var.node).ok_or_else(|| {
                CompileError::codegen(format!("generator for-loop variable '{}' not found", var.node))
            })?;

            let header_bb = ctx.builder.create_block();
            let body_bb = ctx.builder.create_block();
            let exit_bb = ctx.builder.create_block();

            ctx.builder.ins().jump(header_bb, &[]);

            // Header
            ctx.builder.switch_to_block(header_bb);
            let i_val = ctx.builder.use_var(idx_var);
            let cond = ctx.builder.ins().icmp(IntCC::SignedLessThan, i_val, len);
            ctx.builder.ins().brif(cond, body_bb, &[], exit_bb, &[]);

            // Body
            ctx.builder.switch_to_block(body_bb);
            ctx.builder.seal_block(body_bb);

            // Load element: array_get(arr, idx)
            let get_ref = ctx.module.declare_func_in_func(ctx.runtime.get("__pluto_array_get"), ctx.builder.func);
            let i_val = ctx.builder.use_var(idx_var);
            let get_call = ctx.builder.ins().call(get_ref, &[arr_val, i_val]);
            let raw_elem = ctx.builder.inst_results(get_call)[0];
            let elem_val = from_array_slot(raw_elem, &elem_type, &mut ctx.builder);
            ctx.builder.def_var(loop_var, elem_val);

            ctx.loop_stack.push((header_bb, exit_bb));
            let mut body_terminated = false;
            lower_generator_block(
                &body.node.stmts, ctx, &mut body_terminated, yield_counter,
                resume_blocks, param_slots, local_slots, num_params, gen_ptr_var, done_bb,
            )?;
            ctx.loop_stack.pop();

            if !body_terminated {
                let i_val = ctx.builder.use_var(idx_var);
                let one = ctx.builder.ins().iconst(types::I64, 1);
                let next_i = ctx.builder.ins().iadd(i_val, one);
                ctx.builder.def_var(idx_var, next_i);
                ctx.builder.ins().jump(header_bb, &[]);
            }

            ctx.builder.seal_block(header_bb);
            ctx.builder.switch_to_block(exit_bb);
            ctx.builder.seal_block(exit_bb);
            Ok(())
        }
        _ => {
            // For other iterable types in generators, fall back to normal lowering
            // (no yields expected inside)
            ctx.lower_for(var, iterable, body)
        }
    }
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
        TypeExpr::Nullable(inner) => {
            let inner_ty = resolve_type_expr_to_pluto(&inner.node, env);
            PlutoType::Nullable(Box::new(inner_ty))
        }
        TypeExpr::Stream(inner) => {
            let inner_ty = resolve_type_expr_to_pluto(&inner.node, env);
            PlutoType::Stream(Box::new(inner_ty))
        }
    }
}

/// Quick type inference for if-expression at codegen time
fn infer_type_for_expr_if(
    then_block: &crate::span::Spanned<crate::parser::ast::Block>,
    else_block: &crate::span::Spanned<crate::parser::ast::Block>,
    env: &TypeEnv,
    var_types: &HashMap<String, PlutoType>
) -> PlutoType {
    use crate::parser::ast::Stmt;

    // Try to infer from then block's last statement
    if let Some(last) = then_block.node.stmts.last() {
        if let Stmt::Expr(expr) = &last.node {
            return infer_type_for_expr(&expr.node, env, var_types);
        }
    }

    // Fallback to else block's last statement
    if let Some(last) = else_block.node.stmts.last() {
        if let Stmt::Expr(expr) = &last.node {
            return infer_type_for_expr(&expr.node, env, var_types);
        }
    }

    // Default to void if both blocks are empty or end with statements
    PlutoType::Void
}

/// Whether a type needs deep-copying at spawn sites.
/// Heap-allocated mutable types need copying; primitives, immutable strings,
/// and shared-by-reference types (tasks, channels) do not.
fn needs_deep_copy(ty: &PlutoType) -> bool {
    match ty {
        PlutoType::Int | PlutoType::Float | PlutoType::Bool | PlutoType::Byte
        | PlutoType::Void | PlutoType::Range | PlutoType::String
        | PlutoType::Sender(_) | PlutoType::Receiver(_) | PlutoType::Task(_)
        | PlutoType::Error | PlutoType::TypeParam(_) | PlutoType::GenericInstance(..) => false,
        PlutoType::Class(_) | PlutoType::Array(_) | PlutoType::Map(..)
        | PlutoType::Set(_) | PlutoType::Enum(_) | PlutoType::Bytes
        | PlutoType::Fn(..) | PlutoType::Trait(_) => true,
        PlutoType::Nullable(inner) => needs_deep_copy(inner),
        PlutoType::Stream(_) => false, // generator pointer, not deep-copied
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
        PlutoType::Nullable(_) => types::I64,   // pointer (0 = none)
        PlutoType::Stream(_) => types::I64,    // pointer to generator object
        PlutoType::GenericInstance(_, name, _) => panic!("ICE: generic instance '{name}' reached codegen unresolved"),
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
        Expr::Call { name, args, .. } => {
            // old(expr) has same type as expr
            if name.node == "old" && args.len() == 1 {
                return infer_type_for_expr(&args[0].node, env, var_types);
            }
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
            if elements.is_empty() {
                // Empty array — type comes from context (var_types), default to Void
                PlutoType::Array(Box::new(PlutoType::Void))
            } else {
                let first = infer_type_for_expr(&elements[0].node, env, var_types);
                PlutoType::Array(Box::new(first))
            }
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
            if let Expr::Call { name, .. } = &object.node
                && name.node == "expect"
            {
                return PlutoType::Void;
            }
            let obj_type = infer_type_for_expr(&object.node, env, var_types);
            if let PlutoType::Array(elem) = &obj_type {
                return match method.node.as_str() {
                    "len" | "index_of" => PlutoType::Int,
                    "pop" | "last" | "first" | "remove_at" => (**elem).clone(),
                    "is_empty" | "contains" => PlutoType::Bool,
                    "slice" => PlutoType::Array(elem.clone()),
                    _ => PlutoType::Void, // push, clear, insert_at, reverse
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
                    "len" | "index_of" | "last_index_of" | "count" | "byte_at" => PlutoType::Int,
                    "contains" | "starts_with" | "ends_with" | "is_empty" | "is_whitespace" => PlutoType::Bool,
                    "substring" | "trim" | "to_upper" | "to_lower" | "replace" | "char_at" | "trim_start" | "trim_end" | "repeat" => PlutoType::String,
                    "split" => PlutoType::Array(Box::new(PlutoType::String)),
                    "to_bytes" => PlutoType::Bytes,
                    "to_int" => PlutoType::Nullable(Box::new(PlutoType::Int)),
                    "to_float" => PlutoType::Nullable(Box::new(PlutoType::Float)),
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
                let mangled = mangle_method(class_name, &method.node);
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
        Expr::NoneLit => PlutoType::Nullable(Box::new(PlutoType::Void)),
        Expr::NullPropagate { expr } => {
            // The result of `expr?` is the inner type if nullable, otherwise same type
            let inner = infer_type_for_expr(&expr.node, env, var_types);
            match inner {
                PlutoType::Nullable(t) => *t,
                other => other,
            }
        }
        Expr::StaticTraitCall { trait_name, method_name, .. } => {
            // Look up the method signature in the trait
            if let Some(trait_info) = env.traits.get(&trait_name.node) {
                if let Some((_, sig)) = trait_info.methods.iter().find(|(n, _)| n == &method_name.node) {
                    return sig.return_type.clone();
                }
            }
            // Fallback to Void if not found (shouldn't happen after typeck)
            PlutoType::Void
        }
        Expr::If { then_block, else_block, .. } => {
            // Use the helper function we already defined
            infer_type_for_expr_if(then_block, else_block, env, var_types)
        }
        Expr::QualifiedAccess { segments } => {
            panic!(
                "QualifiedAccess should be resolved by module flattening before codegen. Segments: {:?}",
                segments.iter().map(|s| &s.node).collect::<Vec<_>>()
            )
        }
    }
}

/// Convert a byte offset to a 1-based line number.
fn byte_to_line(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())].bytes().filter(|b| *b == b'\n').count() + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_codegen::ir::types;

    // ===== needs_deep_copy tests =====

    #[test]
    fn test_needs_deep_copy_primitives() {
        assert!(!needs_deep_copy(&PlutoType::Int));
        assert!(!needs_deep_copy(&PlutoType::Float));
        assert!(!needs_deep_copy(&PlutoType::Bool));
        assert!(!needs_deep_copy(&PlutoType::Byte));
        assert!(!needs_deep_copy(&PlutoType::Void));
        assert!(!needs_deep_copy(&PlutoType::Range));
    }

    #[test]
    fn test_needs_deep_copy_string() {
        // Strings are immutable, shared by reference
        assert!(!needs_deep_copy(&PlutoType::String));
    }

    #[test]
    fn test_needs_deep_copy_concurrency_types() {
        assert!(!needs_deep_copy(&PlutoType::Sender(Box::new(PlutoType::Int))));
        assert!(!needs_deep_copy(&PlutoType::Receiver(Box::new(PlutoType::Int))));
        assert!(!needs_deep_copy(&PlutoType::Task(Box::new(PlutoType::Int))));
        assert!(!needs_deep_copy(&PlutoType::Stream(Box::new(PlutoType::Int))));
    }

    #[test]
    fn test_needs_deep_copy_error() {
        assert!(!needs_deep_copy(&PlutoType::Error));
    }

    #[test]
    fn test_needs_deep_copy_heap_types() {
        assert!(needs_deep_copy(&PlutoType::Class("User".to_string())));
        assert!(needs_deep_copy(&PlutoType::Array(Box::new(PlutoType::Int))));
        assert!(needs_deep_copy(&PlutoType::Map(Box::new(PlutoType::String), Box::new(PlutoType::Int))));
        assert!(needs_deep_copy(&PlutoType::Set(Box::new(PlutoType::Int))));
        assert!(needs_deep_copy(&PlutoType::Enum("Option".to_string())));
        assert!(needs_deep_copy(&PlutoType::Bytes));
        assert!(needs_deep_copy(&PlutoType::Trait("Printable".to_string())));
    }

    #[test]
    fn test_needs_deep_copy_closures() {
        assert!(needs_deep_copy(&PlutoType::Fn(vec![PlutoType::Int], Box::new(PlutoType::String))));
    }

    #[test]
    fn test_needs_deep_copy_nullable() {
        assert!(!needs_deep_copy(&PlutoType::Nullable(Box::new(PlutoType::Int))));
        assert!(!needs_deep_copy(&PlutoType::Nullable(Box::new(PlutoType::String))));
        assert!(needs_deep_copy(&PlutoType::Nullable(Box::new(PlutoType::Array(Box::new(PlutoType::Int))))));
        assert!(needs_deep_copy(&PlutoType::Nullable(Box::new(PlutoType::Class("User".to_string())))));
    }

    #[test]
    fn test_needs_deep_copy_type_param() {
        assert!(!needs_deep_copy(&PlutoType::TypeParam("T".to_string())));
    }

    #[test]
    fn test_needs_deep_copy_generic_instance() {
        assert!(!needs_deep_copy(&PlutoType::GenericInstance(
            crate::typeck::types::GenericKind::Class,
            "Pair".to_string(),
            vec![PlutoType::Int, PlutoType::String],
        )));
    }

    // ===== pluto_to_cranelift tests =====

    #[test]
    fn test_pluto_to_cranelift_primitives() {
        assert_eq!(pluto_to_cranelift(&PlutoType::Int), types::I64);
        assert_eq!(pluto_to_cranelift(&PlutoType::Float), types::F64);
        assert_eq!(pluto_to_cranelift(&PlutoType::Bool), types::I8);
        assert_eq!(pluto_to_cranelift(&PlutoType::Byte), types::I8);
        assert_eq!(pluto_to_cranelift(&PlutoType::Void), types::I64);
    }

    #[test]
    fn test_pluto_to_cranelift_heap_types() {
        assert_eq!(pluto_to_cranelift(&PlutoType::String), types::I64);
        assert_eq!(pluto_to_cranelift(&PlutoType::Class("User".to_string())), types::I64);
        assert_eq!(pluto_to_cranelift(&PlutoType::Array(Box::new(PlutoType::Int))), types::I64);
        assert_eq!(pluto_to_cranelift(&PlutoType::Trait("Printable".to_string())), types::I64);
        assert_eq!(pluto_to_cranelift(&PlutoType::Enum("Option".to_string())), types::I64);
        assert_eq!(pluto_to_cranelift(&PlutoType::Bytes), types::I64);
    }

    #[test]
    fn test_pluto_to_cranelift_closures() {
        assert_eq!(
            pluto_to_cranelift(&PlutoType::Fn(vec![PlutoType::Int], Box::new(PlutoType::String))),
            types::I64
        );
    }

    #[test]
    fn test_pluto_to_cranelift_collections() {
        assert_eq!(
            pluto_to_cranelift(&PlutoType::Map(Box::new(PlutoType::String), Box::new(PlutoType::Int))),
            types::I64
        );
        assert_eq!(pluto_to_cranelift(&PlutoType::Set(Box::new(PlutoType::Int))), types::I64);
    }

    #[test]
    fn test_pluto_to_cranelift_concurrency() {
        assert_eq!(pluto_to_cranelift(&PlutoType::Task(Box::new(PlutoType::Int))), types::I64);
        assert_eq!(pluto_to_cranelift(&PlutoType::Sender(Box::new(PlutoType::Int))), types::I64);
        assert_eq!(pluto_to_cranelift(&PlutoType::Receiver(Box::new(PlutoType::Int))), types::I64);
        assert_eq!(pluto_to_cranelift(&PlutoType::Stream(Box::new(PlutoType::Int))), types::I64);
    }

    #[test]
    fn test_pluto_to_cranelift_error() {
        assert_eq!(pluto_to_cranelift(&PlutoType::Error), types::I64);
    }

    #[test]
    fn test_pluto_to_cranelift_nullable() {
        assert_eq!(pluto_to_cranelift(&PlutoType::Nullable(Box::new(PlutoType::Int))), types::I64);
    }

    #[test]
    fn test_pluto_to_cranelift_range() {
        assert_eq!(pluto_to_cranelift(&PlutoType::Range), types::I64);
    }

    #[test]
    #[should_panic(expected = "ICE: generic type parameter")]
    fn test_pluto_to_cranelift_type_param_panics() {
        pluto_to_cranelift(&PlutoType::TypeParam("T".to_string()));
    }

    #[test]
    #[should_panic(expected = "ICE: generic instance")]
    fn test_pluto_to_cranelift_generic_instance_panics() {
        pluto_to_cranelift(&PlutoType::GenericInstance(
            crate::typeck::types::GenericKind::Class,
            "Pair".to_string(),
            vec![PlutoType::Int, PlutoType::String],
        ));
    }

    // ===== key_type_tag tests =====

    #[test]
    fn test_key_type_tag_int() {
        assert_eq!(key_type_tag(&PlutoType::Int), 0);
    }

    #[test]
    fn test_key_type_tag_float() {
        assert_eq!(key_type_tag(&PlutoType::Float), 1);
    }

    #[test]
    fn test_key_type_tag_bool() {
        assert_eq!(key_type_tag(&PlutoType::Bool), 2);
    }

    #[test]
    fn test_key_type_tag_string() {
        assert_eq!(key_type_tag(&PlutoType::String), 3);
    }

    #[test]
    fn test_key_type_tag_byte() {
        // Byte hashes as integer
        assert_eq!(key_type_tag(&PlutoType::Byte), 0);
    }

    #[test]
    fn test_key_type_tag_enum() {
        assert_eq!(key_type_tag(&PlutoType::Enum("Color".to_string())), 4);
    }

    #[test]
    fn test_key_type_tag_fallback() {
        // Other types fall back to 0
        assert_eq!(key_type_tag(&PlutoType::Array(Box::new(PlutoType::Int))), 0);
        assert_eq!(key_type_tag(&PlutoType::Class("User".to_string())), 0);
    }

    // ===== byte_to_line tests =====

    #[test]
    fn test_byte_to_line_first_line() {
        assert_eq!(byte_to_line("hello", 0), 1);
        assert_eq!(byte_to_line("hello", 3), 1);
    }

    #[test]
    fn test_byte_to_line_multiple_lines() {
        let source = "line1\nline2\nline3";
        assert_eq!(byte_to_line(source, 0), 1);  // 'l' in line1
        assert_eq!(byte_to_line(source, 5), 1);  // '\n' after line1
        assert_eq!(byte_to_line(source, 6), 2);  // 'l' in line2
        assert_eq!(byte_to_line(source, 11), 2); // '\n' after line2
        assert_eq!(byte_to_line(source, 12), 3); // 'l' in line3
    }

    #[test]
    fn test_byte_to_line_empty_string() {
        assert_eq!(byte_to_line("", 0), 1);
    }

    #[test]
    fn test_byte_to_line_offset_beyond_end() {
        let source = "hello\nworld";
        assert_eq!(byte_to_line(source, 100), 2);
    }

    #[test]
    fn test_byte_to_line_crlf() {
        let source = "line1\r\nline2\r\nline3";
        // CRLF has two bytes but only one '\n'
        assert_eq!(byte_to_line(source, 0), 1);
        assert_eq!(byte_to_line(source, 6), 1);  // '\r'
        assert_eq!(byte_to_line(source, 7), 2);  // after '\n'
    }

    #[test]
    fn test_byte_to_line_only_newlines() {
        let source = "\n\n\n";
        assert_eq!(byte_to_line(source, 0), 1);
        assert_eq!(byte_to_line(source, 1), 2);
        assert_eq!(byte_to_line(source, 2), 3);
        assert_eq!(byte_to_line(source, 3), 4);
    }

    #[test]
    fn test_byte_to_line_trailing_newline() {
        let source = "hello\n";
        assert_eq!(byte_to_line(source, 0), 1);
        assert_eq!(byte_to_line(source, 5), 1);
        assert_eq!(byte_to_line(source, 6), 2);
    }

    #[test]
    fn test_byte_to_line_multiline_document() {
        let source = "fn main() {\n    print(\"hello\")\n}\n";
        assert_eq!(byte_to_line(source, 0), 1);  // 'f' in fn
        assert_eq!(byte_to_line(source, 11), 1); // '\n' after {
        assert_eq!(byte_to_line(source, 12), 2); // space before print
        assert_eq!(byte_to_line(source, 31), 3); // '}' - has 2 newlines before it
        assert_eq!(byte_to_line(source, 32), 3); // '\n' after }
    }

    // ===== resolve_type_expr_to_pluto tests =====

    // Helper to create a minimal TypeEnv for testing
    fn make_test_env() -> TypeEnv {
        let mut env = TypeEnv::new();

        // Add some test classes
        env.classes.insert("User".to_string(), crate::typeck::env::ClassInfo {
            fields: vec![],
            methods: vec![],
            impl_traits: vec![],
            lifecycle: crate::parser::ast::Lifecycle::Singleton,
        });

        // Add some test traits
        env.traits.insert("Printable".to_string(), crate::typeck::env::TraitInfo {
            methods: vec![],
            default_methods: vec![],
            mut_self_methods: HashSet::new(),
            static_methods: HashSet::new(),
            method_contracts: HashMap::new(),
            method_type_exprs: HashMap::new(),
        });

        // Add some test enums
        env.enums.insert("Option".to_string(), crate::typeck::env::EnumInfo {
            variants: vec![],
            variant_type_exprs: vec![],
        });

        env
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_primitives() {
        let env = make_test_env();

        assert_eq!(
            resolve_type_expr_to_pluto(&TypeExpr::Named("int".to_string()), &env),
            PlutoType::Int
        );
        assert_eq!(
            resolve_type_expr_to_pluto(&TypeExpr::Named("float".to_string()), &env),
            PlutoType::Float
        );
        assert_eq!(
            resolve_type_expr_to_pluto(&TypeExpr::Named("bool".to_string()), &env),
            PlutoType::Bool
        );
        assert_eq!(
            resolve_type_expr_to_pluto(&TypeExpr::Named("string".to_string()), &env),
            PlutoType::String
        );
        assert_eq!(
            resolve_type_expr_to_pluto(&TypeExpr::Named("byte".to_string()), &env),
            PlutoType::Byte
        );
        assert_eq!(
            resolve_type_expr_to_pluto(&TypeExpr::Named("bytes".to_string()), &env),
            PlutoType::Bytes
        );
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_class() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(&TypeExpr::Named("User".to_string()), &env);
        assert_eq!(result, PlutoType::Class("User".to_string()));
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_trait() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(&TypeExpr::Named("Printable".to_string()), &env);
        assert_eq!(result, PlutoType::Trait("Printable".to_string()));
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_enum() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(&TypeExpr::Named("Option".to_string()), &env);
        assert_eq!(result, PlutoType::Enum("Option".to_string()));
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_unknown() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(&TypeExpr::Named("Unknown".to_string()), &env);
        assert_eq!(result, PlutoType::Void);
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_array() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Array(Box::new(crate::span::Spanned::dummy(TypeExpr::Named("int".to_string())))),
            &env
        );
        assert_eq!(result, PlutoType::Array(Box::new(PlutoType::Int)));
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_array_nested() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Array(Box::new(crate::span::Spanned::dummy(
                TypeExpr::Array(Box::new(crate::span::Spanned::dummy(TypeExpr::Named("int".to_string()))))
            ))),
            &env
        );
        assert_eq!(
            result,
            PlutoType::Array(Box::new(PlutoType::Array(Box::new(PlutoType::Int))))
        );
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_qualified_class() {
        let mut env = make_test_env();
        env.classes.insert("math.Vector".to_string(), crate::typeck::env::ClassInfo {
            fields: vec![],
            methods: vec![],
            impl_traits: vec![],
            lifecycle: crate::parser::ast::Lifecycle::Singleton,
        });

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Qualified {
                module: "math".to_string(),
                name: "Vector".to_string(),
            },
            &env
        );
        assert_eq!(result, PlutoType::Class("math.Vector".to_string()));
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_qualified_trait() {
        let mut env = make_test_env();
        env.traits.insert("io.Reader".to_string(), crate::typeck::env::TraitInfo {
            methods: vec![],
            default_methods: vec![],
            mut_self_methods: HashSet::new(),
            static_methods: HashSet::new(),
            method_contracts: HashMap::new(),
            method_type_exprs: HashMap::new(),
        });

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Qualified {
                module: "io".to_string(),
                name: "Reader".to_string(),
            },
            &env
        );
        assert_eq!(result, PlutoType::Trait("io.Reader".to_string()));
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_qualified_enum() {
        let mut env = make_test_env();
        env.enums.insert("result.Result".to_string(), crate::typeck::env::EnumInfo {
            variants: vec![],
            variant_type_exprs: vec![],
        });

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Qualified {
                module: "result".to_string(),
                name: "Result".to_string(),
            },
            &env
        );
        assert_eq!(result, PlutoType::Enum("result.Result".to_string()));
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_qualified_unknown() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Qualified {
                module: "foo".to_string(),
                name: "Bar".to_string(),
            },
            &env
        );
        assert_eq!(result, PlutoType::Void);
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_fn_type() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Fn {
                params: vec![
                    Box::new(crate::span::Spanned::dummy(TypeExpr::Named("int".to_string()))),
                    Box::new(crate::span::Spanned::dummy(TypeExpr::Named("string".to_string()))),
                ],
                return_type: Box::new(crate::span::Spanned::dummy(TypeExpr::Named("bool".to_string()))),
            },
            &env
        );
        assert_eq!(
            result,
            PlutoType::Fn(vec![PlutoType::Int, PlutoType::String], Box::new(PlutoType::Bool))
        );
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_fn_type_void_return() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Fn {
                params: vec![Box::new(crate::span::Spanned::dummy(TypeExpr::Named("int".to_string())))],
                return_type: Box::new(crate::span::Spanned::dummy(TypeExpr::Named("void".to_string()))),
            },
            &env
        );
        // "void" is not a primitive, so it maps to PlutoType::Void
        assert_eq!(
            result,
            PlutoType::Fn(vec![PlutoType::Int], Box::new(PlutoType::Void))
        );
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_map() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Generic {
                name: "Map".to_string(),
                type_args: vec![
                    crate::span::Spanned::dummy(TypeExpr::Named("string".to_string())),
                    crate::span::Spanned::dummy(TypeExpr::Named("int".to_string())),
                ],
            },
            &env
        );
        assert_eq!(
            result,
            PlutoType::Map(Box::new(PlutoType::String), Box::new(PlutoType::Int))
        );
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_set() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Generic {
                name: "Set".to_string(),
                type_args: vec![
                    crate::span::Spanned::dummy(TypeExpr::Named("int".to_string())),
                ],
            },
            &env
        );
        assert_eq!(result, PlutoType::Set(Box::new(PlutoType::Int)));
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_task() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Generic {
                name: "Task".to_string(),
                type_args: vec![
                    crate::span::Spanned::dummy(TypeExpr::Named("string".to_string())),
                ],
            },
            &env
        );
        assert_eq!(result, PlutoType::Task(Box::new(PlutoType::String)));
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_sender() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Generic {
                name: "Sender".to_string(),
                type_args: vec![
                    crate::span::Spanned::dummy(TypeExpr::Named("int".to_string())),
                ],
            },
            &env
        );
        assert_eq!(result, PlutoType::Sender(Box::new(PlutoType::Int)));
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_receiver() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Generic {
                name: "Receiver".to_string(),
                type_args: vec![
                    crate::span::Spanned::dummy(TypeExpr::Named("float".to_string())),
                ],
            },
            &env
        );
        assert_eq!(result, PlutoType::Receiver(Box::new(PlutoType::Float)));
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_nullable() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Nullable(Box::new(crate::span::Spanned::dummy(TypeExpr::Named("int".to_string())))),
            &env
        );
        assert_eq!(result, PlutoType::Nullable(Box::new(PlutoType::Int)));
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_nullable_class() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Nullable(Box::new(crate::span::Spanned::dummy(TypeExpr::Named("User".to_string())))),
            &env
        );
        assert_eq!(result, PlutoType::Nullable(Box::new(PlutoType::Class("User".to_string()))));
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_stream() {
        let env = make_test_env();

        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Stream(Box::new(crate::span::Spanned::dummy(TypeExpr::Named("int".to_string())))),
            &env
        );
        assert_eq!(result, PlutoType::Stream(Box::new(PlutoType::Int)));
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_complex_nested() {
        let env = make_test_env();

        // Map<string, Array<int>>
        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Generic {
                name: "Map".to_string(),
                type_args: vec![
                    crate::span::Spanned::dummy(TypeExpr::Named("string".to_string())),
                    crate::span::Spanned::dummy(TypeExpr::Array(
                        Box::new(crate::span::Spanned::dummy(TypeExpr::Named("int".to_string())))
                    )),
                ],
            },
            &env
        );
        assert_eq!(
            result,
            PlutoType::Map(
                Box::new(PlutoType::String),
                Box::new(PlutoType::Array(Box::new(PlutoType::Int)))
            )
        );
    }

    #[test]
    fn test_resolve_type_expr_to_pluto_array_of_maps() {
        let env = make_test_env();

        // Array<Map<string, int>>
        let result = resolve_type_expr_to_pluto(
            &TypeExpr::Array(Box::new(crate::span::Spanned::dummy(
                TypeExpr::Generic {
                    name: "Map".to_string(),
                    type_args: vec![
                        crate::span::Spanned::dummy(TypeExpr::Named("string".to_string())),
                        crate::span::Spanned::dummy(TypeExpr::Named("int".to_string())),
                    ],
                }
            ))),
            &env
        );
        assert_eq!(
            result,
            PlutoType::Array(Box::new(PlutoType::Map(
                Box::new(PlutoType::String),
                Box::new(PlutoType::Int)
            )))
        );
    }

    #[test]
    #[should_panic(expected = "Generic TypeExpr should not reach codegen")]
    fn test_resolve_type_expr_to_pluto_unresolved_generic() {
        let env = make_test_env();

        // User-defined generic (not Map/Set/Task/Sender/Receiver) should panic
        let _ = resolve_type_expr_to_pluto(
            &TypeExpr::Generic {
                name: "Pair".to_string(),
                type_args: vec![
                    crate::span::Spanned::dummy(TypeExpr::Named("int".to_string())),
                    crate::span::Spanned::dummy(TypeExpr::Named("string".to_string())),
                ],
            },
            &env
        );
    }
}
