pub mod lower;

use std::collections::HashMap;

use cranelift_codegen::ir::{types, AbiParam};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use cranelift_frontend::FunctionBuilderContext;
use cranelift_module::{DataDescription, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::typeck::env::TypeEnv;
use crate::typeck::types::PlutoType;
use lower::{lower_function, pluto_to_cranelift};

pub fn codegen(program: &Program, env: &TypeEnv) -> Result<Vec<u8>, CompileError> {
    let mut flag_builder = settings::builder();
    flag_builder.set("is_pic", "true").unwrap();

    let isa_builder = cranelift_codegen::isa::lookup_by_name("aarch64-apple-darwin")
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
    let mut func_ids = HashMap::new();

    // Declare print helper functions from the Pluto runtime (builtins.c)
    let mut print_ids = HashMap::new();

    // __pluto_print_int(long value)
    let mut sig_int = module.make_signature();
    sig_int.params.push(AbiParam::new(types::I64));
    let id = module.declare_function("__pluto_print_int", Linkage::Import, &sig_int)
        .map_err(|e| CompileError::codegen(format!("declare print_int error: {e}")))?;
    print_ids.insert("int", id);

    // __pluto_print_float(double value)
    let mut sig_float = module.make_signature();
    sig_float.params.push(AbiParam::new(types::F64));
    let id = module.declare_function("__pluto_print_float", Linkage::Import, &sig_float)
        .map_err(|e| CompileError::codegen(format!("declare print_float error: {e}")))?;
    print_ids.insert("float", id);

    // __pluto_print_string(const char *value)
    let mut sig_str = module.make_signature();
    sig_str.params.push(AbiParam::new(types::I64));
    let id = module.declare_function("__pluto_print_string", Linkage::Import, &sig_str)
        .map_err(|e| CompileError::codegen(format!("declare print_string error: {e}")))?;
    print_ids.insert("string", id);

    // __pluto_print_bool(int value) — we pass I8 widened to I32
    let mut sig_bool = module.make_signature();
    sig_bool.params.push(AbiParam::new(types::I32));
    let id = module.declare_function("__pluto_print_bool", Linkage::Import, &sig_bool)
        .map_err(|e| CompileError::codegen(format!("declare print_bool error: {e}")))?;
    print_ids.insert("bool", id);

    // Declare __pluto_alloc(I64) -> I64
    let mut sig_alloc = module.make_signature();
    sig_alloc.params.push(AbiParam::new(types::I64));
    sig_alloc.returns.push(AbiParam::new(types::I64));
    let alloc_id = module.declare_function("__pluto_alloc", Linkage::Import, &sig_alloc)
        .map_err(|e| CompileError::codegen(format!("declare alloc error: {e}")))?;

    // Declare __pluto_trait_wrap(I64, I64) -> I64
    let mut sig_trait_wrap = module.make_signature();
    sig_trait_wrap.params.push(AbiParam::new(types::I64));
    sig_trait_wrap.params.push(AbiParam::new(types::I64));
    sig_trait_wrap.returns.push(AbiParam::new(types::I64));
    let trait_wrap_id = module.declare_function("__pluto_trait_wrap", Linkage::Import, &sig_trait_wrap)
        .map_err(|e| CompileError::codegen(format!("declare trait_wrap error: {e}")))?;

    // Declare string runtime functions
    let mut string_ids = HashMap::new();

    // __pluto_string_new(I64, I64) -> I64
    let mut sig_str_new = module.make_signature();
    sig_str_new.params.push(AbiParam::new(types::I64));
    sig_str_new.params.push(AbiParam::new(types::I64));
    sig_str_new.returns.push(AbiParam::new(types::I64));
    let id = module.declare_function("__pluto_string_new", Linkage::Import, &sig_str_new)
        .map_err(|e| CompileError::codegen(format!("declare string_new error: {e}")))?;
    string_ids.insert("new", id);

    // __pluto_string_concat(I64, I64) -> I64
    let mut sig_str_concat = module.make_signature();
    sig_str_concat.params.push(AbiParam::new(types::I64));
    sig_str_concat.params.push(AbiParam::new(types::I64));
    sig_str_concat.returns.push(AbiParam::new(types::I64));
    let id = module.declare_function("__pluto_string_concat", Linkage::Import, &sig_str_concat)
        .map_err(|e| CompileError::codegen(format!("declare string_concat error: {e}")))?;
    string_ids.insert("concat", id);

    // __pluto_string_eq(I64, I64) -> I32
    let mut sig_str_eq = module.make_signature();
    sig_str_eq.params.push(AbiParam::new(types::I64));
    sig_str_eq.params.push(AbiParam::new(types::I64));
    sig_str_eq.returns.push(AbiParam::new(types::I32));
    let id = module.declare_function("__pluto_string_eq", Linkage::Import, &sig_str_eq)
        .map_err(|e| CompileError::codegen(format!("declare string_eq error: {e}")))?;
    string_ids.insert("eq", id);

    // __pluto_string_len(I64) -> I64
    let mut sig_str_len = module.make_signature();
    sig_str_len.params.push(AbiParam::new(types::I64));
    sig_str_len.returns.push(AbiParam::new(types::I64));
    let id = module.declare_function("__pluto_string_len", Linkage::Import, &sig_str_len)
        .map_err(|e| CompileError::codegen(format!("declare string_len error: {e}")))?;
    string_ids.insert("len", id);

    // __pluto_int_to_string(I64) -> I64
    let mut sig_int_to_str = module.make_signature();
    sig_int_to_str.params.push(AbiParam::new(types::I64));
    sig_int_to_str.returns.push(AbiParam::new(types::I64));
    let id = module.declare_function("__pluto_int_to_string", Linkage::Import, &sig_int_to_str)
        .map_err(|e| CompileError::codegen(format!("declare int_to_string error: {e}")))?;
    string_ids.insert("int_to_str", id);

    // __pluto_float_to_string(F64) -> I64
    let mut sig_float_to_str = module.make_signature();
    sig_float_to_str.params.push(AbiParam::new(types::F64));
    sig_float_to_str.returns.push(AbiParam::new(types::I64));
    let id = module.declare_function("__pluto_float_to_string", Linkage::Import, &sig_float_to_str)
        .map_err(|e| CompileError::codegen(format!("declare float_to_string error: {e}")))?;
    string_ids.insert("float_to_str", id);

    // __pluto_bool_to_string(I32) -> I64
    let mut sig_bool_to_str = module.make_signature();
    sig_bool_to_str.params.push(AbiParam::new(types::I32));
    sig_bool_to_str.returns.push(AbiParam::new(types::I64));
    let id = module.declare_function("__pluto_bool_to_string", Linkage::Import, &sig_bool_to_str)
        .map_err(|e| CompileError::codegen(format!("declare bool_to_string error: {e}")))?;
    string_ids.insert("bool_to_str", id);

    // Declare array runtime functions
    let mut array_ids = HashMap::new();

    // __pluto_array_new(I64) -> I64
    let mut sig_arr_new = module.make_signature();
    sig_arr_new.params.push(AbiParam::new(types::I64));
    sig_arr_new.returns.push(AbiParam::new(types::I64));
    let id = module.declare_function("__pluto_array_new", Linkage::Import, &sig_arr_new)
        .map_err(|e| CompileError::codegen(format!("declare array_new error: {e}")))?;
    array_ids.insert("new", id);

    // __pluto_array_push(I64, I64)
    let mut sig_arr_push = module.make_signature();
    sig_arr_push.params.push(AbiParam::new(types::I64));
    sig_arr_push.params.push(AbiParam::new(types::I64));
    let id = module.declare_function("__pluto_array_push", Linkage::Import, &sig_arr_push)
        .map_err(|e| CompileError::codegen(format!("declare array_push error: {e}")))?;
    array_ids.insert("push", id);

    // __pluto_array_get(I64, I64) -> I64
    let mut sig_arr_get = module.make_signature();
    sig_arr_get.params.push(AbiParam::new(types::I64));
    sig_arr_get.params.push(AbiParam::new(types::I64));
    sig_arr_get.returns.push(AbiParam::new(types::I64));
    let id = module.declare_function("__pluto_array_get", Linkage::Import, &sig_arr_get)
        .map_err(|e| CompileError::codegen(format!("declare array_get error: {e}")))?;
    array_ids.insert("get", id);

    // __pluto_array_set(I64, I64, I64)
    let mut sig_arr_set = module.make_signature();
    sig_arr_set.params.push(AbiParam::new(types::I64));
    sig_arr_set.params.push(AbiParam::new(types::I64));
    sig_arr_set.params.push(AbiParam::new(types::I64));
    let id = module.declare_function("__pluto_array_set", Linkage::Import, &sig_arr_set)
        .map_err(|e| CompileError::codegen(format!("declare array_set error: {e}")))?;
    array_ids.insert("set", id);

    // __pluto_array_len(I64) -> I64
    let mut sig_arr_len = module.make_signature();
    sig_arr_len.params.push(AbiParam::new(types::I64));
    sig_arr_len.returns.push(AbiParam::new(types::I64));
    let id = module.declare_function("__pluto_array_len", Linkage::Import, &sig_arr_len)
        .map_err(|e| CompileError::codegen(format!("declare array_len error: {e}")))?;
    array_ids.insert("len", id);

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
                let zeros = vec![0u8; num_methods * 8];
                data_desc.define(zeros.into_boxed_slice());

                for (i, (method_name, _)) in trait_info.methods.iter().enumerate() {
                    let mangled = format!("{}_{}", class_name, method_name);
                    let fid = func_ids.get(&mangled).ok_or_else(|| {
                        CompileError::codegen(format!("missing func_id for vtable entry {mangled}"))
                    })?;
                    let func_ref = module.declare_func_in_data(*fid, &mut data_desc);
                    data_desc.write_function_addr((i * 8) as u32, func_ref);
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
            lower_function(f, builder, env, &mut module, &func_ids, &print_ids, alloc_id, &string_ids, &array_ids, None, &vtable_ids, trait_wrap_id)?;
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
                lower_function(m, builder, env, &mut module, &func_ids, &print_ids, alloc_id, &string_ids, &array_ids, Some(&c.name.node), &vtable_ids, trait_wrap_id)?;
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
                                lower_function(&tmp_func, builder, env, &mut module, &func_ids, &print_ids, alloc_id, &string_ids, &array_ids, Some(class_name), &vtable_ids, trait_wrap_id)?;
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

    let object = module.finish();
    let bytes = object.emit().map_err(|e| CompileError::codegen(format!("emit error: {e}")))?;

    Ok(bytes)
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
    }
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
