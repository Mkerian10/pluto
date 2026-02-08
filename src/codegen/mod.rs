pub mod lower;

use std::collections::HashMap;

use cranelift_codegen::ir::AbiParam;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use cranelift_frontend::FunctionBuilderContext;
use cranelift_module::{Linkage, Module};
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

    // Pass 1: Declare all functions
    for func in &program.functions {
        let f = &func.node;
        let sig = build_signature(f, &module);

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

    // Pass 2: Define all functions
    for func in &program.functions {
        let f = &func.node;
        let func_id = func_ids[&f.name.node];
        let sig = build_signature(f, &module);

        let mut fn_ctx = Context::new();
        fn_ctx.func.signature = sig;

        let mut builder_ctx = FunctionBuilderContext::new();
        {
            let builder = cranelift_frontend::FunctionBuilder::new(&mut fn_ctx.func, &mut builder_ctx);
            lower_function(f, builder, env, &mut module, &func_ids)?;
        }

        module
            .define_function(func_id, &mut fn_ctx)
            .map_err(|e| CompileError::codegen(format!("define function error: {e}")))?;
    }

    let object = module.finish();
    let bytes = object.emit().map_err(|e| CompileError::codegen(format!("emit error: {e}")))?;

    Ok(bytes)
}

fn build_signature(func: &Function, module: &impl Module) -> cranelift_codegen::ir::Signature {
    let mut sig = module.make_signature();

    for param in &func.params {
        let ty = match &param.ty.node {
            TypeExpr::Named(name) => match name.as_str() {
                "int" => PlutoType::Int,
                "float" => PlutoType::Float,
                "bool" => PlutoType::Bool,
                "string" => PlutoType::String,
                _ => PlutoType::Void,
            },
        };
        sig.params.push(AbiParam::new(pluto_to_cranelift(&ty)));
    }

    let ret_type = if func.name.node == "main" {
        Some(PlutoType::Int)
    } else {
        func.return_type.as_ref().map(|t| match &t.node {
            TypeExpr::Named(name) => match name.as_str() {
                "int" => PlutoType::Int,
                "float" => PlutoType::Float,
                "bool" => PlutoType::Bool,
                "string" => PlutoType::String,
                _ => PlutoType::Void,
            },
        })
    };

    if let Some(ty) = ret_type {
        if ty != PlutoType::Void {
            sig.returns.push(AbiParam::new(pluto_to_cranelift(&ty)));
        }
    }

    sig
}
