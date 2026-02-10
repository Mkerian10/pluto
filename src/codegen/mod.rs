pub mod lower;
pub mod runtime;

use std::collections::{HashMap, HashSet};

use cranelift_codegen::ir::immediates::Offset32;
use cranelift_codegen::ir::{types, AbiParam, InstBuilder, MemFlags, Value};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use cranelift_frontend::FunctionBuilderContext;
use cranelift_module::{DataDescription, DataId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use uuid::Uuid;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::typeck::env::TypeEnv;
use crate::typeck::types::PlutoType;
use lower::{lower_function, pluto_to_cranelift, resolve_type_expr_to_pluto, FnContracts, POINTER_SIZE};
use runtime::RuntimeRegistry;

fn host_target_triple() -> Result<&'static str, CompileError> {
    if cfg!(all(target_arch = "aarch64", target_os = "macos")) {
        Ok("aarch64-apple-darwin")
    } else if cfg!(all(target_arch = "x86_64", target_os = "macos")) {
        Ok("x86_64-apple-darwin")
    } else if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        Ok("x86_64-unknown-linux-gnu")
    } else if cfg!(all(target_arch = "aarch64", target_os = "linux")) {
        Ok("aarch64-unknown-linux-gnu")
    } else {
        Err(CompileError::codegen(format!(
            "unsupported host target: {}-{}",
            std::env::consts::ARCH,
            std::env::consts::OS
        )))
    }
}

/// Declare a writable, zero-initialized 8-byte global for each DI singleton.
/// These globals hold singleton pointers so that scope block codegen (Phase 3)
/// can load them when wiring scoped instances.
fn declare_singleton_globals(
    env: &TypeEnv,
    module: &mut ObjectModule,
) -> Result<HashMap<String, DataId>, CompileError> {
    let mut globals = HashMap::new();
    for class_name in &env.di_order {
        let data_name = format!("__pluto_singleton_{}", class_name);
        let data_id = module
            .declare_data(&data_name, Linkage::Local, true, false)
            .map_err(|e| CompileError::codegen(format!("declare singleton global: {e}")))?;
        let mut data_desc = DataDescription::new();
        data_desc.define_zeroinit(8);
        module
            .define_data(data_id, &data_desc)
            .map_err(|e| CompileError::codegen(format!("define singleton global: {e}")))?;
        globals.insert(class_name.clone(), data_id);
    }
    Ok(globals)
}

pub fn codegen(program: &Program, env: &TypeEnv, source: &str) -> Result<Vec<u8>, CompileError> {
    let mut flag_builder = settings::builder();
    flag_builder.set("is_pic", "true").unwrap();

    let isa_builder = cranelift_codegen::isa::lookup_by_name(host_target_triple()?)
        .map_err(|e| CompileError::codegen(format!("unsupported target: {e}")))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| CompileError::codegen(format!("ISA error: {e}")))?;

    let obj_builder = ObjectBuilder::new(
        isa,
        "pluto_module",
        cranelift_module::default_libcall_names(),
    )
    .map_err(|e| CompileError::codegen(format!("object builder error: {e}")))?;

    let mut module = ObjectModule::new(obj_builder);
    let runtime = RuntimeRegistry::new(&mut module)?;
    let mut func_ids = HashMap::new();

    // Declare module-level globals for DI singleton pointers (Phase 2)
    let singleton_data_ids = declare_singleton_globals(env, &mut module)?;

    // Pass 0: Declare extern fns with Import linkage
    for ext in &program.extern_fns {
        let e = &ext.node;
        if let Some(func_sig) = env.functions.get(&e.name.node) {
            let mut sig = module.make_signature();
            for param_ty in &func_sig.params {
                sig.params.push(AbiParam::new(pluto_to_cranelift(param_ty)));
            }
            if func_sig.return_type != PlutoType::Void {
                sig.returns.push(AbiParam::new(pluto_to_cranelift(&func_sig.return_type)));
            }
            let func_id = module
                .declare_function(&e.name.node, Linkage::Import, &sig)
                .map_err(|e| CompileError::codegen(format!("declare extern fn error: {e}")))?;
            func_ids.insert(e.name.node.clone(), func_id);
        }
    }

    // Pass 1: Declare all top-level functions
    for func in &program.functions {
        let f = &func.node;
        let sig = build_signature(f, &module, env);

        let linkage = if f.name.node == "main" {
            Linkage::Export
        } else {
            Linkage::Local
        };

        let func_id = module
            .declare_function(&f.name.node, linkage, &sig)
            .map_err(|e| CompileError::codegen(format!("declare function error: {e}")))?;

        func_ids.insert(f.name.node.clone(), func_id);
    }

    // Pass 1b: Declare all methods with mangled names
    for class in &program.classes {
        let c = &class.node;
        for method in &c.methods {
            let m = &method.node;
            let mangled = format!("{}_{}", c.name.node, m.name.node);
            let sig = build_method_signature(m, &module, &c.name.node, env);
            let func_id = module
                .declare_function(&mangled, Linkage::Local, &sig)
                .map_err(|e| CompileError::codegen(format!("declare method error: {e}")))?;
            func_ids.insert(mangled, func_id);
        }
    }

    // Pass 1c: Declare default trait method functions for classes that inherit them
    for class in &program.classes {
        let c = &class.node;
        let class_name = &c.name.node;
        let class_method_names: Vec<String> = c.methods.iter().map(|m| m.node.name.node.clone()).collect();

        for trait_name_spanned in &c.impl_traits {
            let trait_name = &trait_name_spanned.node;
            if let Some(trait_info) = env.traits.get(trait_name) {
                for (method_name, _) in &trait_info.methods {
                    if !class_method_names.contains(method_name) && trait_info.default_methods.contains(method_name) {
                        let mangled = format!("{}_{}", class_name, method_name);
                        if !func_ids.contains_key(&mangled) {
                            // Build signature from the function signature in env
                            let func_sig = env.functions.get(&mangled).ok_or_else(|| {
                                CompileError::codegen(format!("missing sig for default method {mangled}"))
                            })?;
                            let mut sig = module.make_signature();
                            for param_ty in &func_sig.params {
                                sig.params.push(AbiParam::new(pluto_to_cranelift(param_ty)));
                            }
                            if func_sig.return_type != PlutoType::Void {
                                sig.returns.push(AbiParam::new(pluto_to_cranelift(&func_sig.return_type)));
                            }
                            let func_id = module
                                .declare_function(&mangled, Linkage::Local, &sig)
                                .map_err(|e| CompileError::codegen(format!("declare default method error: {e}")))?;
                            func_ids.insert(mangled, func_id);
                        }
                    }
                }
            }
        }
    }

    // Build vtables for (class, trait) pairs
    let mut vtable_ids: HashMap<(String, String), cranelift_module::DataId> = HashMap::new();
    for class in &program.classes {
        let c = &class.node;
        let class_name = &c.name.node;

        for trait_name_spanned in &c.impl_traits {
            let trait_name = &trait_name_spanned.node;
            if let Some(trait_info) = env.traits.get(trait_name) {
                let num_methods = trait_info.methods.len();
                let mut data_desc = DataDescription::new();
                let zeros = vec![0u8; num_methods * POINTER_SIZE as usize];
                data_desc.define(zeros.into_boxed_slice());

                for (i, (method_name, _)) in trait_info.methods.iter().enumerate() {
                    let mangled = format!("{}_{}", class_name, method_name);
                    let fid = func_ids.get(&mangled).ok_or_else(|| {
                        CompileError::codegen(format!("missing func_id for vtable entry {mangled}"))
                    })?;
                    let func_ref = module.declare_func_in_data(*fid, &mut data_desc);
                    data_desc.write_function_addr((i as u32) * POINTER_SIZE as u32, func_ref);
                }

                let data_id = module.declare_anonymous_data(false, false)
                    .map_err(|e| CompileError::codegen(format!("declare vtable data error: {e}")))?;
                module.define_data(data_id, &data_desc)
                    .map_err(|e| CompileError::codegen(format!("define vtable data error: {e}")))?;

                vtable_ids.insert((class_name.clone(), trait_name.clone()), data_id);
            }
        }
    }

    // Pre-pass: collect spawn closure function names for sender cleanup
    let spawn_closure_fns = collect_spawn_closure_names(program);

    // Build class invariants map for codegen
    let class_invariants: HashMap<String, Vec<(Expr, String)>> = program.classes.iter()
        .filter(|c| !c.node.invariants.is_empty())
        .map(|c| {
            let name = c.node.name.node.clone();
            let invs = c.node.invariants.iter().map(|inv| {
                let desc = format_invariant_expr(&inv.node.expr.node);
                (inv.node.expr.node.clone(), desc)
            }).collect();
            (name, invs)
        })
        .collect();

    // Build function contracts map for codegen
    let mut fn_contracts: HashMap<String, FnContracts> = HashMap::new();
    for func in &program.functions {
        let f = &func.node;
        if !f.contracts.is_empty() {
            let requires: Vec<(Expr, String)> = f.contracts.iter()
                .filter(|c| c.node.kind == ContractKind::Requires)
                .map(|c| (c.node.expr.node.clone(), format_invariant_expr(&c.node.expr.node)))
                .collect();
            let ensures: Vec<(Expr, String)> = f.contracts.iter()
                .filter(|c| c.node.kind == ContractKind::Ensures)
                .map(|c| (c.node.expr.node.clone(), format_invariant_expr(&c.node.expr.node)))
                .collect();
            if !requires.is_empty() || !ensures.is_empty() {
                fn_contracts.insert(f.name.node.clone(), FnContracts { requires, ensures });
            }
        }
    }
    for class in &program.classes {
        let c = &class.node;
        for method in &c.methods {
            let m = &method.node;
            if !m.contracts.is_empty() {
                let mangled = format!("{}_{}", c.name.node, m.name.node);
                let requires: Vec<(Expr, String)> = m.contracts.iter()
                    .filter(|c| c.node.kind == ContractKind::Requires)
                    .map(|c| (c.node.expr.node.clone(), format_invariant_expr(&c.node.expr.node)))
                    .collect();
                let ensures: Vec<(Expr, String)> = m.contracts.iter()
                    .filter(|c| c.node.kind == ContractKind::Ensures)
                    .map(|c| (c.node.expr.node.clone(), format_invariant_expr(&c.node.expr.node)))
                    .collect();
                if !requires.is_empty() || !ensures.is_empty() {
                    fn_contracts.insert(mangled, FnContracts { requires, ensures });
                }
            }
        }
    }
    if let Some(app_spanned) = &program.app {
        let app = &app_spanned.node;
        for method in &app.methods {
            let m = &method.node;
            if !m.contracts.is_empty() {
                let mangled = format!("{}_{}", app.name.node, m.name.node);
                let requires: Vec<(Expr, String)> = m.contracts.iter()
                    .filter(|c| c.node.kind == ContractKind::Requires)
                    .map(|c| (c.node.expr.node.clone(), format_invariant_expr(&c.node.expr.node)))
                    .collect();
                let ensures: Vec<(Expr, String)> = m.contracts.iter()
                    .filter(|c| c.node.kind == ContractKind::Ensures)
                    .map(|c| (c.node.expr.node.clone(), format_invariant_expr(&c.node.expr.node)))
                    .collect();
                if !requires.is_empty() || !ensures.is_empty() {
                    fn_contracts.insert(mangled, FnContracts { requires, ensures });
                }
            }
        }
    }
    // Trait default methods — contracts from default method declarations
    for class in &program.classes {
        let c = &class.node;
        let class_method_names: Vec<String> = c.methods.iter().map(|m| m.node.name.node.clone()).collect();
        for trait_name_spanned in &c.impl_traits {
            let trait_name = &trait_name_spanned.node;
            for trait_decl in &program.traits {
                if trait_decl.node.name.node == *trait_name {
                    for trait_method in &trait_decl.node.methods {
                        if trait_method.body.is_some() && !class_method_names.contains(&trait_method.name.node) {
                            if !trait_method.contracts.is_empty() {
                                let mangled = format!("{}_{}", c.name.node, trait_method.name.node);
                                let requires: Vec<(Expr, String)> = trait_method.contracts.iter()
                                    .filter(|c| c.node.kind == ContractKind::Requires)
                                    .map(|c| (c.node.expr.node.clone(), format_invariant_expr(&c.node.expr.node)))
                                    .collect();
                                let ensures: Vec<(Expr, String)> = trait_method.contracts.iter()
                                    .filter(|c| c.node.kind == ContractKind::Ensures)
                                    .map(|c| (c.node.expr.node.clone(), format_invariant_expr(&c.node.expr.node)))
                                    .collect();
                                if !requires.is_empty() || !ensures.is_empty() {
                                    fn_contracts.insert(mangled, FnContracts { requires, ensures });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // Propagate trait contracts to implementing class methods
    // Trait requires are prepended (checked first), trait ensures are appended
    for class in &program.classes {
        let c = &class.node;
        for trait_name_spanned in &c.impl_traits {
            let trait_name = &trait_name_spanned.node;
            if let Some(trait_info) = env.traits.get(trait_name) {
                for (method_name, contracts) in &trait_info.method_contracts {
                    let mangled = format!("{}_{}", c.name.node, method_name);
                    let trait_requires: Vec<(Expr, String)> = contracts.iter()
                        .filter(|c| c.node.kind == ContractKind::Requires)
                        .map(|c| (c.node.expr.node.clone(), format_invariant_expr(&c.node.expr.node)))
                        .collect();
                    let trait_ensures: Vec<(Expr, String)> = contracts.iter()
                        .filter(|c| c.node.kind == ContractKind::Ensures)
                        .map(|c| (c.node.expr.node.clone(), format_invariant_expr(&c.node.expr.node)))
                        .collect();
                    if !trait_requires.is_empty() || !trait_ensures.is_empty() {
                        let entry = fn_contracts.entry(mangled).or_insert_with(|| FnContracts {
                            requires: Vec::new(),
                            ensures: Vec::new(),
                        });
                        // Prepend trait requires (checked first)
                        let mut merged_requires = trait_requires;
                        merged_requires.append(&mut entry.requires);
                        entry.requires = merged_requires;
                        // Append trait ensures
                        entry.ensures.extend(trait_ensures);
                    }
                }
            }
        }
    }

    // Pass 2: Define all top-level functions
    for func in &program.functions {
        let f = &func.node;
        let func_id = func_ids[&f.name.node];
        let sig = build_signature(f, &module, env);

        let mut fn_ctx = Context::new();
        fn_ctx.func.signature = sig;

        let mut builder_ctx = FunctionBuilderContext::new();
        {
            let builder = cranelift_frontend::FunctionBuilder::new(&mut fn_ctx.func, &mut builder_ctx);
            lower_function(f, builder, env, &mut module, &func_ids, &runtime, None, &vtable_ids, source, &spawn_closure_fns, &class_invariants, &fn_contracts, &singleton_data_ids)?;
        }

        module
            .define_function(func_id, &mut fn_ctx)
            .map_err(|e| CompileError::codegen(format!("define function error for '{}': {e}", f.name.node)))?;
    }

    // Pass 2b: Define all methods
    for class in &program.classes {
        let c = &class.node;
        for method in &c.methods {
            let m = &method.node;
            let mangled = format!("{}_{}", c.name.node, m.name.node);
            let func_id = func_ids[&mangled];
            let sig = build_method_signature(m, &module, &c.name.node, env);

            let mut fn_ctx = Context::new();
            fn_ctx.func.signature = sig;

            let mut builder_ctx = FunctionBuilderContext::new();
            {
                let builder = cranelift_frontend::FunctionBuilder::new(&mut fn_ctx.func, &mut builder_ctx);
                lower_function(m, builder, env, &mut module, &func_ids, &runtime, Some(&c.name.node), &vtable_ids, source, &spawn_closure_fns, &class_invariants, &fn_contracts, &singleton_data_ids)?;
            }

            module
                .define_function(func_id, &mut fn_ctx)
                .map_err(|e| CompileError::codegen(format!("define method error: {e}")))?;
        }
    }

    // Pass 2c: Define default trait method bodies
    for class in &program.classes {
        let c = &class.node;
        let class_name = &c.name.node;
        let class_method_names: Vec<String> = c.methods.iter().map(|m| m.node.name.node.clone()).collect();

        for trait_name_spanned in &c.impl_traits {
            let trait_name = &trait_name_spanned.node;
            // Find the trait AST to get default method bodies
            for trait_decl in &program.traits {
                if trait_decl.node.name.node == *trait_name {
                    for trait_method in &trait_decl.node.methods {
                        if trait_method.body.is_some() && !class_method_names.contains(&trait_method.name.node) {
                            let body = trait_method.body.as_ref().unwrap();
                            let tmp_func = Function {
                                id: Uuid::new_v4(),
                                name: trait_method.name.clone(),
                                type_params: vec![],
                                params: trait_method.params.clone(),
                                return_type: trait_method.return_type.clone(),
                                contracts: trait_method.contracts.clone(),
                                body: body.clone(),
                                is_pub: false,
                            };

                            let mangled = format!("{}_{}", class_name, trait_method.name.node);
                            let func_id = func_ids[&mangled];

                            // Build signature from env
                            let func_sig = env.functions.get(&mangled).unwrap();
                            let mut sig = module.make_signature();
                            for param_ty in &func_sig.params {
                                sig.params.push(AbiParam::new(pluto_to_cranelift(param_ty)));
                            }
                            if func_sig.return_type != PlutoType::Void {
                                sig.returns.push(AbiParam::new(pluto_to_cranelift(&func_sig.return_type)));
                            }

                            let mut fn_ctx = Context::new();
                            fn_ctx.func.signature = sig;

                            let mut builder_ctx = FunctionBuilderContext::new();
                            {
                                let builder = cranelift_frontend::FunctionBuilder::new(&mut fn_ctx.func, &mut builder_ctx);
                                lower_function(&tmp_func, builder, env, &mut module, &func_ids, &runtime, Some(class_name), &vtable_ids, source, &spawn_closure_fns, &class_invariants, &fn_contracts, &singleton_data_ids)?;
                            }

                            module
                                .define_function(func_id, &mut fn_ctx)
                                .map_err(|e| CompileError::codegen(format!("define default method error: {e}")))?;
                        }
                    }
                }
            }
        }
    }

    // Pass 1d: Declare app methods
    if let Some(app_spanned) = &program.app {
        let app = &app_spanned.node;
        let app_name = &app.name.node;
        for method in &app.methods {
            let m = &method.node;
            let mangled = format!("{}_{}", app_name, m.name.node);
            let sig = build_method_signature(m, &module, app_name, env);
            let func_id = module
                .declare_function(&mangled, Linkage::Local, &sig)
                .map_err(|e| CompileError::codegen(format!("declare app method error: {e}")))?;
            func_ids.insert(mangled, func_id);
        }
    }

    // Pass 2d: Define app method bodies
    if let Some(app_spanned) = &program.app {
        let app = &app_spanned.node;
        let app_name = &app.name.node;
        for method in &app.methods {
            let m = &method.node;
            let mangled = format!("{}_{}", app_name, m.name.node);
            let func_id = func_ids[&mangled];
            let sig = build_method_signature(m, &module, app_name, env);

            let mut fn_ctx = Context::new();
            fn_ctx.func.signature = sig;

            let mut builder_ctx = FunctionBuilderContext::new();
            {
                let builder = cranelift_frontend::FunctionBuilder::new(&mut fn_ctx.func, &mut builder_ctx);
                lower_function(m, builder, env, &mut module, &func_ids, &runtime, Some(app_name), &vtable_ids, source, &spawn_closure_fns, &class_invariants, &fn_contracts, &singleton_data_ids)?;
            }

            module
                .define_function(func_id, &mut fn_ctx)
                .map_err(|e| CompileError::codegen(format!("define app method error: {e}")))?;
        }
    }

    // Generate test runner main (when test_info is non-empty)
    if !program.test_info.is_empty() {
        let mut main_sig = module.make_signature();
        main_sig.returns.push(AbiParam::new(types::I64));
        let main_id = module
            .declare_function("main", Linkage::Export, &main_sig)
            .map_err(|e| CompileError::codegen(format!("declare test main error: {e}")))?;

        let mut fn_ctx = Context::new();
        fn_ctx.func.signature = main_sig;

        let mut builder_ctx = FunctionBuilderContext::new();
        {
            let mut builder = cranelift_frontend::FunctionBuilder::new(&mut fn_ctx.func, &mut builder_ctx);
            let entry_block = builder.create_block();
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            // Initialize GC
            let gc_init_ref = module.declare_func_in_func(runtime.get("__pluto_gc_init"), builder.func);
            builder.ins().call(gc_init_ref, &[]);

            let test_start_ref = module.declare_func_in_func(runtime.get("__pluto_test_start"), builder.func);
            let test_pass_ref = module.declare_func_in_func(runtime.get("__pluto_test_pass"), builder.func);
            let string_new_ref = module.declare_func_in_func(runtime.get("__pluto_string_new"), builder.func);

            for (display_name, fn_name) in &program.test_info {
                // Create Pluto string for the test name
                let mut data_desc = DataDescription::new();
                let mut bytes = display_name.as_bytes().to_vec();
                bytes.push(0);
                data_desc.define(bytes.into_boxed_slice());
                let data_id = module.declare_anonymous_data(false, false)
                    .map_err(|e| CompileError::codegen(format!("declare test name data error: {e}")))?;
                module.define_data(data_id, &data_desc)
                    .map_err(|e| CompileError::codegen(format!("define test name data error: {e}")))?;
                let gv = module.declare_data_in_func(data_id, builder.func);
                let raw_ptr = builder.ins().global_value(types::I64, gv);
                let len_val = builder.ins().iconst(types::I64, display_name.len() as i64);
                let call = builder.ins().call(string_new_ref, &[raw_ptr, len_val]);
                let name_str = builder.inst_results(call)[0];

                // call __pluto_test_start(name_str)
                builder.ins().call(test_start_ref, &[name_str]);

                // call test function
                let test_func_id = func_ids.get(fn_name).ok_or_else(|| {
                    CompileError::codegen(format!("missing test function '{fn_name}'"))
                })?;
                let test_func_ref = module.declare_func_in_func(*test_func_id, builder.func);
                builder.ins().call(test_func_ref, &[]);

                // call __pluto_test_pass()
                builder.ins().call(test_pass_ref, &[]);
            }

            // call __pluto_test_summary(count)
            let test_summary_ref = module.declare_func_in_func(runtime.get("__pluto_test_summary"), builder.func);
            let count_val = builder.ins().iconst(types::I64, program.test_info.len() as i64);
            builder.ins().call(test_summary_ref, &[count_val]);

            let zero = builder.ins().iconst(types::I64, 0);
            builder.ins().return_(&[zero]);

            builder.finalize();
        }

        module
            .define_function(main_id, &mut fn_ctx)
            .map_err(|e| CompileError::codegen(format!("define test main error: {e}")))?;
    }

    // Generate synthetic main for DI wiring (when app exists)
    if let Some(app_spanned) = &program.app {
        let app = &app_spanned.node;
        let app_name = &app.name.node;

        // Declare main with Export linkage
        let mut main_sig = module.make_signature();
        main_sig.returns.push(AbiParam::new(types::I64));
        let main_id = module
            .declare_function("main", Linkage::Export, &main_sig)
            .map_err(|e| CompileError::codegen(format!("declare synthetic main error: {e}")))?;

        let mut fn_ctx = Context::new();
        fn_ctx.func.signature = main_sig;

        let mut builder_ctx = FunctionBuilderContext::new();
        {
            let mut builder = cranelift_frontend::FunctionBuilder::new(&mut fn_ctx.func, &mut builder_ctx);
            let entry_block = builder.create_block();
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            // Initialize GC before any allocations
            let gc_init_ref = module.declare_func_in_func(runtime.get("__pluto_gc_init"), builder.func);
            builder.ins().call(gc_init_ref, &[]);

            let alloc_ref = module.declare_func_in_func(runtime.get("__pluto_alloc"), builder.func);

            // Create singletons in topological order
            let mut singletons: HashMap<String, Value> = HashMap::new();

            for class_name in &env.di_order {
                let class_info = env.classes.get(class_name).ok_or_else(|| {
                    CompileError::codegen(format!("DI: unknown class '{}'", class_name))
                })?;
                let size = class_info.fields.len() as i64 * POINTER_SIZE as i64;
                let size_val = builder.ins().iconst(types::I64, size);
                let call = builder.ins().call(alloc_ref, &[size_val]);
                let ptr = builder.inst_results(call)[0];

                // Wire injected fields
                for (i, (_, field_ty, is_injected)) in class_info.fields.iter().enumerate() {
                    if *is_injected {
                        if let PlutoType::Class(dep_name) = field_ty {
                            if let Some(&dep_ptr) = singletons.get(dep_name) {
                                let offset = (i as i32) * POINTER_SIZE;
                                builder.ins().store(
                                    MemFlags::new(),
                                    dep_ptr,
                                    ptr,
                                    Offset32::new(offset),
                                );
                            }
                        }
                    }
                    // Non-injected fields are zero-initialized by calloc
                }

                singletons.insert(class_name.clone(), ptr);

                // Store pointer to module-level global for scope block access (Phase 2)
                if let Some(&data_id) = singleton_data_ids.get(class_name) {
                    let gv = module.declare_data_in_func(data_id, builder.func);
                    let addr = builder.ins().global_value(types::I64, gv);
                    builder.ins().store(MemFlags::new(), ptr, addr, Offset32::new(0));
                }
            }

            // Allocate and wire the app itself
            let app_info = env.classes.get(app_name).ok_or_else(|| {
                CompileError::codegen(format!("DI: unknown app class '{}'", app_name))
            })?;
            let app_size = app_info.fields.len() as i64 * POINTER_SIZE as i64;
            let app_size_val = builder.ins().iconst(types::I64, app_size);
            let app_call = builder.ins().call(alloc_ref, &[app_size_val]);
            let app_ptr = builder.inst_results(app_call)[0];

            for (i, (_, field_ty, is_injected)) in app_info.fields.iter().enumerate() {
                if *is_injected {
                    if let PlutoType::Class(dep_name) = field_ty {
                        if let Some(&dep_ptr) = singletons.get(dep_name) {
                            let offset = (i as i32) * POINTER_SIZE;
                            builder.ins().store(
                                MemFlags::new(),
                                dep_ptr,
                                app_ptr,
                                Offset32::new(offset),
                            );
                        }
                    }
                }
            }

            // Call AppName_main(app_ptr)
            let app_main_mangled = format!("{}_main", app_name);
            let app_main_id = func_ids.get(&app_main_mangled).ok_or_else(|| {
                CompileError::codegen(format!("DI: missing app main function '{}'", app_main_mangled))
            })?;
            let app_main_ref = module.declare_func_in_func(*app_main_id, builder.func);
            builder.ins().call(app_main_ref, &[app_ptr]);

            // Return 0
            let zero = builder.ins().iconst(types::I64, 0);
            builder.ins().return_(&[zero]);

            builder.finalize();
        }

        module
            .define_function(main_id, &mut fn_ctx)
            .map_err(|e| CompileError::codegen(format!("define synthetic main error: {e}")))?;
    }

    let object = module.finish();
    let bytes = object.emit().map_err(|e| CompileError::codegen(format!("emit error: {e}")))?;

    Ok(bytes)
}

fn resolve_param_pluto_type(param: &Param, env: &TypeEnv) -> PlutoType {
    resolve_type_expr_to_pluto(&param.ty.node, env)
}

fn build_signature(func: &Function, module: &impl Module, env: &TypeEnv) -> cranelift_codegen::ir::Signature {
    let mut sig = module.make_signature();

    for param in &func.params {
        let ty = resolve_param_pluto_type(param, env);
        sig.params.push(AbiParam::new(pluto_to_cranelift(&ty)));
    }

    let ret_type = if func.name.node == "main" {
        Some(PlutoType::Int)
    } else {
        func.return_type.as_ref().map(|t| resolve_type_expr_to_pluto(&t.node, env))
    };

    if let Some(ty) = ret_type {
        if ty != PlutoType::Void {
            sig.returns.push(AbiParam::new(pluto_to_cranelift(&ty)));
        }
    }

    sig
}

/// Collect the set of function names that are spawn closure bodies.
/// These functions need sender_dec cleanup for captured Sender variables.
fn collect_spawn_closure_names(program: &Program) -> HashSet<String> {
    let mut result = HashSet::new();

    fn walk_expr(expr: &Expr, result: &mut HashSet<String>) {
        match expr {
            Expr::Spawn { call } => {
                if let Expr::ClosureCreate { fn_name, .. } = &call.node {
                    result.insert(fn_name.clone());
                }
                walk_expr(&call.node, result);
            }
            Expr::BinOp { lhs, rhs, .. } => {
                walk_expr(&lhs.node, result);
                walk_expr(&rhs.node, result);
            }
            Expr::UnaryOp { operand, .. } => walk_expr(&operand.node, result),
            Expr::Call { args, .. } => {
                for a in args { walk_expr(&a.node, result); }
            }
            Expr::MethodCall { object, args, .. } => {
                walk_expr(&object.node, result);
                for a in args { walk_expr(&a.node, result); }
            }
            Expr::FieldAccess { object, .. } => walk_expr(&object.node, result),
            Expr::Index { object, index } => {
                walk_expr(&object.node, result);
                walk_expr(&index.node, result);
            }
            Expr::StructLit { fields, .. } => {
                for (_, v) in fields { walk_expr(&v.node, result); }
            }
            Expr::EnumData { fields, .. } => {
                for (_, v) in fields { walk_expr(&v.node, result); }
            }
            Expr::ArrayLit { elements } => {
                for e in elements { walk_expr(&e.node, result); }
            }
            Expr::SetLit { elements, .. } => {
                for e in elements { walk_expr(&e.node, result); }
            }
            Expr::MapLit { entries, .. } => {
                for (k, v) in entries { walk_expr(&k.node, result); walk_expr(&v.node, result); }
            }
            Expr::StringInterp { parts } => {
                for p in parts {
                    if let StringInterpPart::Expr(e) = p { walk_expr(&e.node, result); }
                }
            }
            Expr::Propagate { expr } | Expr::Cast { expr, .. } => walk_expr(&expr.node, result),
            Expr::Catch { expr, handler } => {
                walk_expr(&expr.node, result);
                match handler {
                    CatchHandler::Wildcard { body, .. } => walk_block(&body.node, result),
                    CatchHandler::Shorthand(e) => walk_expr(&e.node, result),
                }
            }
            Expr::Closure { body, .. } => walk_block(&body.node, result),
            Expr::ClosureCreate { .. } => {}
            Expr::Range { start, end, .. } => {
                walk_expr(&start.node, result);
                walk_expr(&end.node, result);
            }
            Expr::NullPropagate { expr } => walk_expr(&expr.node, result),
            Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_)
            | Expr::StringLit(_) | Expr::Ident(_) | Expr::EnumUnit { .. }
            | Expr::NoneLit => {}
        }
    }

    fn walk_stmt(stmt: &Stmt, result: &mut HashSet<String>) {
        match stmt {
            Stmt::Let { value, .. } => walk_expr(&value.node, result),
            Stmt::LetChan { capacity, .. } => {
                if let Some(cap) = capacity { walk_expr(&cap.node, result); }
            }
            Stmt::Return(Some(e)) => walk_expr(&e.node, result),
            Stmt::Return(None) | Stmt::Break | Stmt::Continue => {}
            Stmt::Assign { value, .. } => walk_expr(&value.node, result),
            Stmt::FieldAssign { object, value, .. } => {
                walk_expr(&object.node, result);
                walk_expr(&value.node, result);
            }
            Stmt::IndexAssign { object, index, value } => {
                walk_expr(&object.node, result);
                walk_expr(&index.node, result);
                walk_expr(&value.node, result);
            }
            Stmt::If { condition, then_block, else_block } => {
                walk_expr(&condition.node, result);
                walk_block(&then_block.node, result);
                if let Some(eb) = else_block { walk_block(&eb.node, result); }
            }
            Stmt::While { condition, body } => {
                walk_expr(&condition.node, result);
                walk_block(&body.node, result);
            }
            Stmt::For { iterable, body, .. } => {
                walk_expr(&iterable.node, result);
                walk_block(&body.node, result);
            }
            Stmt::Match { expr, arms } => {
                walk_expr(&expr.node, result);
                for arm in arms { walk_block(&arm.body.node, result); }
            }
            Stmt::Raise { fields, .. } => {
                for (_, v) in fields { walk_expr(&v.node, result); }
            }
            Stmt::Select { arms, default } => {
                for arm in arms {
                    match &arm.op {
                        SelectOp::Recv { channel, .. } => walk_expr(&channel.node, result),
                        SelectOp::Send { channel, value } => {
                            walk_expr(&channel.node, result);
                            walk_expr(&value.node, result);
                        }
                    }
                    walk_block(&arm.body.node, result);
                }
                if let Some(def) = default { walk_block(&def.node, result); }
            }
            Stmt::Scope { seeds, body, .. } => {
                for seed in seeds {
                    walk_expr(&seed.node, result);
                }
                walk_block(&body.node, result);
            }
            Stmt::Expr(e) => walk_expr(&e.node, result),
        }
    }

    fn walk_block(block: &Block, result: &mut HashSet<String>) {
        for stmt in &block.stmts { walk_stmt(&stmt.node, result); }
    }

    fn walk_function(func: &Function, result: &mut HashSet<String>) {
        walk_block(&func.body.node, result);
    }

    for func in &program.functions {
        walk_function(&func.node, &mut result);
    }
    for class in &program.classes {
        for method in &class.node.methods {
            walk_function(&method.node, &mut result);
        }
    }
    if let Some(app) = &program.app {
        for method in &app.node.methods {
            walk_function(&method.node, &mut result);
        }
    }

    result
}

fn build_method_signature(func: &Function, module: &impl Module, class_name: &str, env: &TypeEnv) -> cranelift_codegen::ir::Signature {
    let mut sig = module.make_signature();

    for param in &func.params {
        if param.name.node == "self" {
            sig.params.push(AbiParam::new(types::I64));
        } else {
            let ty = resolve_param_pluto_type(param, env);
            sig.params.push(AbiParam::new(pluto_to_cranelift(&ty)));
        }
    }

    let ret_type = func.return_type.as_ref().map(|t| resolve_type_expr_to_pluto(&t.node, env));

    if let Some(ty) = ret_type {
        if ty != PlutoType::Void {
            sig.returns.push(AbiParam::new(pluto_to_cranelift(&ty)));
        }
    }

    let _ = class_name;

    sig
}

/// Format an invariant expression as a human-readable string for error messages.
pub(super) fn format_invariant_expr(expr: &Expr) -> String {
    match expr {
        Expr::IntLit(n) => n.to_string(),
        Expr::FloatLit(f) => f.to_string(),
        Expr::BoolLit(b) => b.to_string(),
        Expr::Ident(name) => name.clone(),
        Expr::FieldAccess { object, field } => {
            format!("{}.{}", format_invariant_expr(&object.node), field.node)
        }
        Expr::MethodCall { object, method, .. } => {
            format!("{}.{}()", format_invariant_expr(&object.node), method.node)
        }
        Expr::BinOp { op, lhs, rhs } => {
            let op_str = match op {
                BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*",
                BinOp::Div => "/", BinOp::Mod => "%",
                BinOp::Eq => "==", BinOp::Neq => "!=",
                BinOp::Lt => "<", BinOp::Gt => ">",
                BinOp::LtEq => "<=", BinOp::GtEq => ">=",
                BinOp::And => "&&", BinOp::Or => "||",
                BinOp::BitAnd => "&", BinOp::BitOr => "|", BinOp::BitXor => "^",
                BinOp::Shl => "<<", BinOp::Shr => ">>",
            };
            format!("{} {} {}", format_invariant_expr(&lhs.node), op_str, format_invariant_expr(&rhs.node))
        }
        Expr::UnaryOp { op, operand } => {
            let op_str = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
                UnaryOp::BitNot => "~",
            };
            format!("{}{}", op_str, format_invariant_expr(&operand.node))
        }
        Expr::Call { name, args, .. } if name.node == "old" && args.len() == 1 => {
            format!("old({})", format_invariant_expr(&args[0].node))
        }
        _ => "<contract>".to_string(),
    }
}
