pub mod lower;
pub mod runtime;

use std::collections::HashMap;

use cranelift_codegen::ir::immediates::Offset32;
use cranelift_codegen::ir::{types, AbiParam, InstBuilder, MemFlags, Value};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use cranelift_frontend::FunctionBuilderContext;
use cranelift_module::{DataDescription, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::typeck::env::TypeEnv;
use crate::typeck::types::PlutoType;
use lower::{lower_function, pluto_to_cranelift, resolve_type_expr_to_pluto, POINTER_SIZE};
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

pub fn codegen(program: &Program, env: &TypeEnv) -> Result<Vec<u8>, CompileError> {
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
            lower_function(f, builder, env, &mut module, &func_ids, &runtime, None, &vtable_ids)?;
        }

        module
            .define_function(func_id, &mut fn_ctx)
            .map_err(|e| CompileError::codegen(format!("define function error: {e}")))?;
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
                lower_function(m, builder, env, &mut module, &func_ids, &runtime, Some(&c.name.node), &vtable_ids)?;
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
                                name: trait_method.name.clone(),
                                type_params: vec![],
                                params: trait_method.params.clone(),
                                return_type: trait_method.return_type.clone(),
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
                                lower_function(&tmp_func, builder, env, &mut module, &func_ids, &runtime, Some(class_name), &vtable_ids)?;
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
                lower_function(m, builder, env, &mut module, &func_ids, &runtime, Some(app_name), &vtable_ids)?;
            }

            module
                .define_function(func_id, &mut fn_ctx)
                .map_err(|e| CompileError::codegen(format!("define app method error: {e}")))?;
        }
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
