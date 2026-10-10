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

use crate::coverage::CoverageMap;
use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::Spanned;
use crate::typeck::env::{mangle_method, TypeEnv};
use crate::typeck::types::PlutoType;
use crate::visit::{walk_expr, Visitor};
use lower::{lower_function, lower_serve_handler, lower_generator_creator, lower_generator_next, pluto_to_cranelift, resolve_type_expr_to_pluto, FnContracts, POINTER_SIZE};
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

/// Declare a writable, zero-initialized 8-byte global for each name in the iterator.
/// Allocate and wire a fresh instance of a transient class during startup
/// wiring. Singleton deps come from the shared map; transient deps recurse
/// (typeck rejects transient cycles). Scoped deps cannot appear at startup
/// (captive-dependency validation).
fn emit_startup_transient(
    class_name: &str,
    env: &crate::typeck::env::TypeEnv,
    builder: &mut cranelift_frontend::FunctionBuilder,
    alloc_ref: cranelift_codegen::ir::FuncRef,
    singletons: &HashMap<String, Value>,
) -> Result<Value, CompileError> {
    let class_info = env.classes.get(class_name).ok_or_else(|| {
        CompileError::codegen(format!("DI: unknown transient class '{class_name}'"))
    })?;
    let size = class_info.fields.len() as i64 * POINTER_SIZE as i64;
    let size_val = builder.ins().iconst(types::I64, size);
    let call = builder.ins().call(alloc_ref, &[size_val]);
    let ptr = builder.inst_results(call)[0];
    let fields = class_info.fields.clone();
    for (i, (_, field_ty, is_injected)) in fields.iter().enumerate() {
        if !*is_injected {
            continue;
        }
        if let PlutoType::Class(dep_name) = field_ty {
            let dep_ptr = if env.classes.get(dep_name).map(|c| c.lifecycle)
                == Some(crate::parser::ast::Lifecycle::Transient)
            {
                emit_startup_transient(dep_name, env, builder, alloc_ref, singletons)?
            } else if let Some(&v) = singletons.get(dep_name) {
                v
            } else {
                continue;
            };
            let offset = (i as i32) * POINTER_SIZE;
            builder.ins().store(MemFlags::new(), dep_ptr, ptr, Offset32::new(offset));
        }
    }
    Ok(ptr)
}

/// Emit the zero-STATE value for a non-injected field of a DI-synthesized
/// instance. Returns `None` when the allocator's all-zero bit pattern
/// already IS the zero state (numbers, bools, bytes, nullables as `none`).
/// Heap-typed fields get real empty values — `""`, empty bytes/array/map/
/// set, an enum's first unit variant — and class/entity fields get a
/// recursively zero-constructed instance, so a colocated `domain` dependency
/// hands out live objects instead of null pointers.
///
/// Types with no zero state never reach here: typeck rejects them
/// (`validate_startup_zero_construction`), so hitting one is an ICE.
fn emit_startup_zero_value(
    ty: &PlutoType,
    env: &crate::typeck::env::TypeEnv,
    builder: &mut cranelift_frontend::FunctionBuilder,
    module: &mut ObjectModule,
    runtime: &RuntimeRegistry,
) -> Result<Option<Value>, CompileError> {
    match ty {
        PlutoType::Int
        | PlutoType::Float
        | PlutoType::Bool
        | PlutoType::Byte
        | PlutoType::Nullable(_) => Ok(None),
        PlutoType::String => {
            // Empty string: __pluto_string_new over a 1-byte data symbol.
            let data_id = module
                .declare_anonymous_data(false, false)
                .map_err(|e| CompileError::codegen(format!("declare empty-string data: {e}")))?;
            let mut desc = DataDescription::new();
            desc.define(vec![0u8].into_boxed_slice());
            module
                .define_data(data_id, &desc)
                .map_err(|e| CompileError::codegen(format!("define empty-string data: {e}")))?;
            let gv = module.declare_data_in_func(data_id, builder.func);
            let ptr = builder.ins().global_value(types::I64, gv);
            let len = builder.ins().iconst(types::I64, 0);
            let f = module.declare_func_in_func(runtime.get("__pluto_string_new"), builder.func);
            let call = builder.ins().call(f, &[ptr, len]);
            Ok(Some(builder.inst_results(call)[0]))
        }
        PlutoType::Bytes => {
            let f = module.declare_func_in_func(runtime.get("__pluto_bytes_new"), builder.func);
            let call = builder.ins().call(f, &[]);
            Ok(Some(builder.inst_results(call)[0]))
        }
        PlutoType::Array(_) => {
            let cap = builder.ins().iconst(types::I64, 0);
            let f = module.declare_func_in_func(runtime.get("__pluto_array_new"), builder.func);
            let call = builder.ins().call(f, &[cap]);
            Ok(Some(builder.inst_results(call)[0]))
        }
        PlutoType::Map(key_ty, _) => {
            let tag = builder.ins().iconst(types::I64, lower::key_type_tag(key_ty));
            let f = module.declare_func_in_func(runtime.get("__pluto_map_new"), builder.func);
            let call = builder.ins().call(f, &[tag]);
            Ok(Some(builder.inst_results(call)[0]))
        }
        PlutoType::Set(elem_ty) => {
            let tag = builder.ins().iconst(types::I64, lower::key_type_tag(elem_ty));
            let f = module.declare_func_in_func(runtime.get("__pluto_set_new"), builder.func);
            let call = builder.ins().call(f, &[tag]);
            Ok(Some(builder.inst_results(call)[0]))
        }
        PlutoType::Enum(ename) => {
            // First variant, which typeck guaranteed is a unit variant:
            // [tag=0, zeroed payload slots] (the allocator zeroes memory).
            let info = env.enums.get(ename).ok_or_else(|| {
                CompileError::codegen(format!("DI zero-construction: unknown enum '{ename}'"))
            })?;
            let max_fields = info.variants.iter().map(|(_, f)| f.len()).max().unwrap_or(0);
            let size = (1 + max_fields) as i64 * POINTER_SIZE as i64;
            let size_val = builder.ins().iconst(types::I64, size);
            let f = module.declare_func_in_func(runtime.get("__pluto_alloc"), builder.func);
            let call = builder.ins().call(f, &[size_val]);
            Ok(Some(builder.inst_results(call)[0]))
        }
        PlutoType::Class(cname) => Ok(Some(emit_startup_zero_instance(
            cname, env, builder, module, runtime,
        )?)),
        PlutoType::Trait(_)
        | PlutoType::Fn(..)
        | PlutoType::Task(_)
        | PlutoType::Sender(_)
        | PlutoType::Receiver(_)
        | PlutoType::Stream(_)
        | PlutoType::Range
        | PlutoType::Error
        | PlutoType::Void
        | PlutoType::TypeParam(_)
        | PlutoType::GenericInstance(..) => Err(CompileError::codegen(format!(
            "internal: type '{ty}' has no zero state and should have been rejected at typeck \
             (validate_startup_zero_construction)"
        ))),
    }
}

/// Zero-construct an instance of `class_name` for DI startup wiring: an
/// entity-tagged allocation for objects (identity semantics, per-instance
/// lock slot) or a plain one for classes, with every field set to its zero
/// state. Typeck guarantees the class has no injected dependencies and no
/// recursive non-nullable shape.
fn emit_startup_zero_instance(
    class_name: &str,
    env: &crate::typeck::env::TypeEnv,
    builder: &mut cranelift_frontend::FunctionBuilder,
    module: &mut ObjectModule,
    runtime: &RuntimeRegistry,
) -> Result<Value, CompileError> {
    let class_info = env.classes.get(class_name).ok_or_else(|| {
        CompileError::codegen(format!("DI zero-construction: unknown class '{class_name}'"))
    })?;
    let fields = class_info.fields.clone();
    let size = fields.len() as i64 * POINTER_SIZE as i64;
    let alloc_name = if env.object_types.contains(class_name) {
        "__pluto_alloc_entity"
    } else {
        "__pluto_alloc"
    };
    let size_val = builder.ins().iconst(types::I64, size);
    let alloc = module.declare_func_in_func(runtime.get(alloc_name), builder.func);
    let call = builder.ins().call(alloc, &[size_val]);
    let ptr = builder.inst_results(call)[0];
    for (i, (fname, fty, inj)) in fields.iter().enumerate() {
        if *inj {
            return Err(CompileError::codegen(format!(
                "internal: zero-constructing '{class_name}' with injected field '{fname}' \
                 (typeck should have rejected this)"
            )));
        }
        if let Some(v) = emit_startup_zero_value(fty, env, builder, module, runtime)? {
            let offset = (i as i32) * POINTER_SIZE;
            builder.ins().store(MemFlags::new(), v, ptr, Offset32::new(offset));
        }
    }
    Ok(ptr)
}

/// Fill the non-injected (data) fields of a freshly allocated DI singleton
/// with their zero-state values. Fields whose zero state is the all-zero
/// bit pattern are left to the allocator.
fn emit_startup_zero_data_fields(
    class_info: &crate::typeck::env::ClassInfo,
    instance: Value,
    env: &crate::typeck::env::TypeEnv,
    builder: &mut cranelift_frontend::FunctionBuilder,
    module: &mut ObjectModule,
    runtime: &RuntimeRegistry,
) -> Result<(), CompileError> {
    let fields = class_info.fields.clone();
    for (i, (_, fty, inj)) in fields.iter().enumerate() {
        if *inj {
            continue;
        }
        if let Some(v) = emit_startup_zero_value(fty, env, builder, module, runtime)? {
            let offset = (i as i32) * POINTER_SIZE;
            builder.ins().store(MemFlags::new(), v, instance, Offset32::new(offset));
        }
    }
    Ok(())
}

fn declare_global_data<'a>(
    names: impl Iterator<Item = &'a String>,
    prefix: &str,
    module: &mut ObjectModule,
) -> Result<HashMap<String, DataId>, CompileError> {
    let mut globals = HashMap::new();
    for name in names {
        let data_name = format!("{prefix}{name}");
        let data_id = module
            .declare_data(&data_name, Linkage::Local, true, false)
            .map_err(|e| CompileError::codegen(format!("declare {prefix} global: {e}")))?;
        let mut data_desc = DataDescription::new();
        data_desc.define_zeroinit(8);
        module
            .define_data(data_id, &data_desc)
            .map_err(|e| CompileError::codegen(format!("define {prefix} global: {e}")))?;
        globals.insert(name.clone(), data_id);
    }
    Ok(globals)
}

/// Extract requires contracts from a contract list into a FnContracts, if any exist.
fn extract_fn_contracts(contracts: &[Spanned<ContractClause>]) -> Option<FnContracts> {
    let requires: Vec<(Expr, String)> = contracts.iter()
        .filter(|c| c.node.kind == ContractKind::Requires)
        .map(|c| (c.node.expr.node.clone(), format_invariant_expr(&c.node.expr.node)))
        .collect();
    if requires.is_empty() {
        None
    } else {
        Some(FnContracts { requires })
    }
}

pub fn codegen(program: &Program, env: &TypeEnv, source: &str, coverage_map: Option<&CoverageMap>) -> Result<Vec<u8>, CompileError> {
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
    let singleton_data_ids = declare_global_data(env.di_order.iter(), "__pluto_singleton_", &mut module)?;

    // Declare module-level globals for rwlock pointers (Phase 4b). Entity
    // (object) types are excluded: their methods serialize on a PER-INSTANCE
    // lock carried in a hidden trailing slot of each entity allocation, not
    // on a per-type global (rfc-objects.md).
    let rwlock_data_ids = declare_global_data(
        env.synchronized_singletons
            .iter()
            .filter(|c| !env.object_types.contains(*c)),
        "__pluto_rwlock_",
        &mut module,
    )?;

    // Pre-pass: collect spawn closure function names (needed before declarations)
    let spawn_closure_fns = collect_spawn_closure_names(program);

    // Build coverage lookup table (empty when coverage is disabled)
    let empty_lookup = HashMap::new();
    let coverage_lookup = coverage_map
        .map(|m| m.build_span_lookup())
        .unwrap_or(empty_lookup);

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

    // Serve handler functions: one per served class, run on a thread per
    // accepted connection (rfc-objects.md phase 2 slice 2). Declared before
    // user functions so lower_serve can take their address.
    let served_classes: Vec<String> = {
        let mut v: Vec<String> = crate::marshal::collect_served_classes(program)
            .into_iter()
            .filter(|c| env.classes.contains_key(c))
            .collect();
        v.sort();
        v
    };
    for served in &served_classes {
        let mut hsig = module.make_signature();
        hsig.params.push(AbiParam::new(types::I64)); // svc
        hsig.params.push(AbiParam::new(types::I64)); // conn
        hsig.returns.push(AbiParam::new(types::I64));
        let hname = format!("__pluto_serve_handler_{served}");
        let hid = module
            .declare_function(&hname, Linkage::Local, &hsig)
            .map_err(|e| CompileError::codegen(format!("declare serve handler error: {e}")))?;
        func_ids.insert(hname, hid);
    }

    // Pass 1: Declare all top-level functions
    for func in &program.functions {
        let f = &func.node;
        let mut sig = build_signature(f, &module, env);

        // Spawn closure functions must return I64 so the C runtime reads the integer register
        if spawn_closure_fns.contains(&f.name.node) && !sig.returns.is_empty() {
            sig.returns.clear();
            sig.returns.push(AbiParam::new(types::I64));
        }

        let linkage = if f.name.node == "main" {
            Linkage::Export
        } else {
            Linkage::Local
        };

        let func_id = module
            .declare_function(&f.name.node, linkage, &sig)
            .map_err(|e| CompileError::codegen(format!("declare function error: {e}")))?;

        func_ids.insert(f.name.node.clone(), func_id);

        // For generators, also declare the __gen_next_{name} function: (I64) -> void
        if env.generators.contains(&f.name.node) {
            let next_name = format!("__gen_next_{}", f.name.node);
            let mut next_sig = module.make_signature();
            next_sig.params.push(AbiParam::new(types::I64)); // gen_ptr
            let next_id = module
                .declare_function(&next_name, Linkage::Local, &next_sig)
                .map_err(|e| CompileError::codegen(format!("declare generator next error: {e}")))?;
            func_ids.insert(next_name, next_id);
        }
    }

    // Pass 1b: Declare all methods with mangled names
    for class in &program.classes {
        let c = &class.node;
        for method in &c.methods {
            let m = &method.node;
            let mangled = mangle_method(&c.name.node, &m.name.node);
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
            let trait_name = &trait_name_spanned.name.node;
            if let Some(trait_info) = env.traits.get(trait_name) {
                for (method_name, _) in &trait_info.methods {
                    if !class_method_names.contains(method_name) && trait_info.default_methods.contains(method_name) {
                        let mangled = mangle_method(class_name, method_name);
                        if let std::collections::hash_map::Entry::Vacant(entry) = func_ids.entry(mangled.clone()) {
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
                            entry.insert(func_id);
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
            let trait_name = &trait_name_spanned.name.node;
            if let Some(trait_info) = env.traits.get(trait_name) {
                let num_methods = trait_info.methods.len();
                let mut data_desc = DataDescription::new();
                let zeros = vec![0u8; num_methods * POINTER_SIZE as usize];
                data_desc.define(zeros.into_boxed_slice());

                for (i, (method_name, _)) in trait_info.methods.iter().enumerate() {
                    let mangled = mangle_method(class_name, method_name);
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

    // Class invariants are discharged statically at compile time
    // (src/typeck/discharge.rs) -- no runtime checks are emitted. The only
    // runtime invariant validation lives at wire/marshal decode boundaries,
    // generated as ordinary AST by src/marshal.rs.

    // Build function contracts map for codegen
    let mut fn_contracts: HashMap<String, FnContracts> = HashMap::new();
    for func in &program.functions {
        if let Some(fc) = extract_fn_contracts(&func.node.contracts) {
            fn_contracts.insert(func.node.name.node.clone(), fc);
        }
    }
    for class in &program.classes {
        let c = &class.node;
        for method in &c.methods {
            if let Some(fc) = extract_fn_contracts(&method.node.contracts) {
                fn_contracts.insert(mangle_method(&c.name.node, &method.node.name.node), fc);
            }
        }
    }
    if let Some(app_spanned) = &program.app {
        let app = &app_spanned.node;
        for method in &app.methods {
            if let Some(fc) = extract_fn_contracts(&method.node.contracts) {
                fn_contracts.insert(mangle_method(&app.name.node, &method.node.name.node), fc);
            }
        }
    }
    for stage_spanned in &program.stages {
        let stage = &stage_spanned.node;
        for method in &stage.methods {
            if let Some(fc) = extract_fn_contracts(&method.node.contracts) {
                fn_contracts.insert(mangle_method(&stage.name.node, &method.node.name.node), fc);
            }
        }
    }
    // Trait default methods — contracts from default method declarations
    for class in &program.classes {
        let c = &class.node;
        let class_method_names: Vec<String> = c.methods.iter().map(|m| m.node.name.node.clone()).collect();
        for trait_name_spanned in &c.impl_traits {
            let trait_name = &trait_name_spanned.name.node;
            for trait_decl in &program.traits {
                if trait_decl.node.name.node == *trait_name {
                    for trait_method in &trait_decl.node.methods {
                        if trait_method.body.is_some() && !class_method_names.contains(&trait_method.name.node) {
                            if let Some(fc) = extract_fn_contracts(&trait_method.contracts) {
                                fn_contracts.insert(mangle_method(&c.name.node, &trait_method.name.node), fc);
                            }
                        }
                    }
                }
            }
        }
    }
    // Propagate trait contracts to implementing class methods
    // Trait requires are prepended (checked first)
    for class in &program.classes {
        let c = &class.node;
        for trait_name_spanned in &c.impl_traits {
            let trait_name = &trait_name_spanned.name.node;
            if let Some(trait_info) = env.traits.get(trait_name) {
                for (method_name, contracts) in &trait_info.method_contracts {
                    let mangled = mangle_method(&c.name.node, method_name);
                    let trait_requires: Vec<(Expr, String)> = contracts.iter()
                        .filter(|c| c.node.kind == ContractKind::Requires)
                        .map(|c| (c.node.expr.node.clone(), format_invariant_expr(&c.node.expr.node)))
                        .collect();
                    if !trait_requires.is_empty() {
                        let entry = fn_contracts.entry(mangled).or_insert_with(|| FnContracts {
                            requires: Vec::new(),
                        });
                        // Prepend trait requires (checked first)
                        let mut merged_requires = trait_requires;
                        merged_requires.append(&mut entry.requires);
                        entry.requires = merged_requires;
                    }
                }
            }
        }
    }

    // Static requires discharge (contracts.md phase 6, slice 1): for every
    // callee that carries requires checks AND has at least one call site
    // whose clauses typeck proved, declare an *unchecked twin*
    // `<name>$nochk` — the same body lowered without the entry checks.
    // Proven call sites route to the twin; every other caller (unproven
    // sites, fn refs, trait dispatch, spawn, serve/RPC entries, wire
    // boundaries) keeps the checked symbol, so runtime behavior at
    // unproven sites is exactly as before. Defensive gates: the typeck
    // clause count must match the emitted check count (a divergence
    // between the two collections suppresses elision, never a check), and
    // generators are excluded (their checks live in the generator-next
    // body).
    let proven_callees: HashSet<&String> = env.proven_requires_sites.values().collect();
    let mut nochk_fns: HashSet<String> = HashSet::new();
    for func in &program.functions {
        let f = &func.node;
        let name = &f.name.node;
        if env.generators.contains(name) || spawn_closure_fns.contains(name) {
            continue;
        }
        if !proven_callees.contains(name) {
            continue;
        }
        let Some(fc) = fn_contracts.get(name) else { continue };
        if env.requires_summaries.get(name).map(|s| s.clauses.len()) != Some(fc.requires.len()) {
            continue;
        }
        let twin = format!("{name}$nochk");
        if func_ids.contains_key(&twin) {
            continue;
        }
        let sig = build_signature(f, &module, env);
        let twin_id = module
            .declare_function(&twin, Linkage::Local, &sig)
            .map_err(|e| CompileError::codegen(format!("declare unchecked twin error: {e}")))?;
        func_ids.insert(twin, twin_id);
        nochk_fns.insert(name.clone());
    }
    for class in &program.classes {
        let c = &class.node;
        for method in &c.methods {
            let m = &method.node;
            let mangled = mangle_method(&c.name.node, &m.name.node);
            if !proven_callees.contains(&mangled) {
                continue;
            }
            let Some(fc) = fn_contracts.get(&mangled) else { continue };
            if env.requires_summaries.get(&mangled).map(|s| s.clauses.len())
                != Some(fc.requires.len())
            {
                continue;
            }
            let twin = format!("{mangled}$nochk");
            if func_ids.contains_key(&twin) {
                continue;
            }
            let sig = build_method_signature(m, &module, &c.name.node, env);
            let twin_id = module
                .declare_function(&twin, Linkage::Local, &sig)
                .map_err(|e| CompileError::codegen(format!("declare unchecked twin error: {e}")))?;
            func_ids.insert(twin, twin_id);
            nochk_fns.insert(mangled);
        }
    }
    // Twins are lowered against an empty contracts map (the map's only use
    // is prologue check emission).
    let empty_contracts: HashMap<String, FnContracts> = HashMap::new();

    // Pass 2: Define all top-level functions
    for func in &program.functions {
        let f = &func.node;
        let func_id = func_ids[&f.name.node];

        if env.generators.contains(&f.name.node) {
            // Generator: define creator function
            let sig = build_signature(f, &module, env);
            let mut fn_ctx = Context::new();
            fn_ctx.func.signature = sig;

            let mut builder_ctx = FunctionBuilderContext::new();
            {
                let builder = cranelift_frontend::FunctionBuilder::new(&mut fn_ctx.func, &mut builder_ctx);
                lower_generator_creator(f, builder, env, &mut module, &func_ids, &runtime)?;
            }
            module
                .define_function(func_id, &mut fn_ctx)
                .map_err(|e| CompileError::codegen(format!("define generator creator error for '{}': {e}", f.name.node)))?;

            // Generator: define next function
            let next_name = format!("__gen_next_{}", f.name.node);
            let next_id = func_ids[&next_name];
            let mut next_sig = module.make_signature();
            next_sig.params.push(AbiParam::new(types::I64)); // gen_ptr
            let mut next_ctx = Context::new();
            next_ctx.func.signature = next_sig;

            let mut next_builder_ctx = FunctionBuilderContext::new();
            {
                let builder = cranelift_frontend::FunctionBuilder::new(&mut next_ctx.func, &mut next_builder_ctx);
                lower_generator_next(f, builder, env, &mut module, &func_ids, &runtime, &vtable_ids, source, &fn_contracts, &singleton_data_ids, &rwlock_data_ids, &coverage_lookup)?;
            }
            module
                .define_function(next_id, &mut next_ctx)
                .map_err(|e| CompileError::codegen(format!("define generator next error for '{}': {e}", f.name.node)))?;
        } else {
            // Normal function
            let mut sig = build_signature(f, &module, env);

            // Spawn closure functions must return I64 (matches declaration)
            if spawn_closure_fns.contains(&f.name.node) && !sig.returns.is_empty() {
                sig.returns.clear();
                sig.returns.push(AbiParam::new(types::I64));
            }
            let mut fn_ctx = Context::new();
            fn_ctx.func.signature = sig;

            let mut builder_ctx = FunctionBuilderContext::new();
            {
                let builder = cranelift_frontend::FunctionBuilder::new(&mut fn_ctx.func, &mut builder_ctx);
                lower_function(f, builder, env, &mut module, &func_ids, &runtime, None, &vtable_ids, source, &spawn_closure_fns, &fn_contracts, &singleton_data_ids, &rwlock_data_ids, &coverage_lookup)?;
            }

            module
                .define_function(func_id, &mut fn_ctx)
                .map_err(|e| CompileError::codegen(format!("define function error for '{}': {e}", f.name.node)))?;

            // Unchecked twin: same body, no entry requires checks.
            if nochk_fns.contains(&f.name.node) {
                let twin_id = func_ids[&format!("{}$nochk", f.name.node)];
                let sig = build_signature(f, &module, env);
                let mut twin_ctx = Context::new();
                twin_ctx.func.signature = sig;
                let mut twin_builder_ctx = FunctionBuilderContext::new();
                {
                    let builder = cranelift_frontend::FunctionBuilder::new(&mut twin_ctx.func, &mut twin_builder_ctx);
                    lower_function(f, builder, env, &mut module, &func_ids, &runtime, None, &vtable_ids, source, &spawn_closure_fns, &empty_contracts, &singleton_data_ids, &rwlock_data_ids, &coverage_lookup)?;
                }
                module
                    .define_function(twin_id, &mut twin_ctx)
                    .map_err(|e| CompileError::codegen(format!("define unchecked twin error for '{}': {e}", f.name.node)))?;
            }
        }
    }

    // Pass 2b: Define all methods
    for class in &program.classes {
        let c = &class.node;
        for method in &c.methods {
            let m = &method.node;
            let mangled = mangle_method(&c.name.node, &m.name.node);
            let func_id = func_ids[&mangled];
            let sig = build_method_signature(m, &module, &c.name.node, env);

            let mut fn_ctx = Context::new();
            fn_ctx.func.signature = sig;

            let mut builder_ctx = FunctionBuilderContext::new();
            {
                let builder = cranelift_frontend::FunctionBuilder::new(&mut fn_ctx.func, &mut builder_ctx);
                lower_function(m, builder, env, &mut module, &func_ids, &runtime, Some(&c.name.node), &vtable_ids, source, &spawn_closure_fns, &fn_contracts, &singleton_data_ids, &rwlock_data_ids, &coverage_lookup)?;
            }

            module
                .define_function(func_id, &mut fn_ctx)
                .map_err(|e| CompileError::codegen(format!("define method error for '{mangled}': {e}")))?;

            // Unchecked twin: same body, no entry requires checks.
            if nochk_fns.contains(&mangled) {
                let twin_id = func_ids[&format!("{mangled}$nochk")];
                let sig = build_method_signature(m, &module, &c.name.node, env);
                let mut twin_ctx = Context::new();
                twin_ctx.func.signature = sig;
                let mut twin_builder_ctx = FunctionBuilderContext::new();
                {
                    let builder = cranelift_frontend::FunctionBuilder::new(&mut twin_ctx.func, &mut twin_builder_ctx);
                    lower_function(m, builder, env, &mut module, &func_ids, &runtime, Some(&c.name.node), &vtable_ids, source, &spawn_closure_fns, &empty_contracts, &singleton_data_ids, &rwlock_data_ids, &coverage_lookup)?;
                }
                module
                    .define_function(twin_id, &mut twin_ctx)
                    .map_err(|e| CompileError::codegen(format!("define unchecked twin error for '{mangled}': {e}")))?;
            }
        }
    }

    // Pass 2c: Define default trait method bodies
    for class in &program.classes {
        let c = &class.node;
        let class_name = &c.name.node;
        let class_method_names: Vec<String> = c.methods.iter().map(|m| m.node.name.node.clone()).collect();

        for trait_name_spanned in &c.impl_traits {
            let trait_name = &trait_name_spanned.name.node;
            // Find the trait AST to get default method bodies
            for trait_decl in &program.traits {
                if trait_decl.node.name.node == *trait_name {
                    for trait_method in &trait_decl.node.methods {
                        if let Some(body) = &trait_method.body
                            && !class_method_names.contains(&trait_method.name.node)
                        {
                            let tmp_func = Function {
                                id: Uuid::new_v4(),
                                name: trait_method.name.clone(),
                                type_params: vec![],
                                type_param_bounds: std::collections::HashMap::new(),
                                params: trait_method.params.clone(),
                                return_type: trait_method.return_type.clone(),
                                contracts: trait_method.contracts.clone(),
                                body: body.clone(),
                                is_pub: false,
                                is_override: false,
                                is_generator: false,
        provides: Vec::new(),
                            };

                            let mangled = mangle_method(class_name, &trait_method.name.node);
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
                                lower_function(&tmp_func, builder, env, &mut module, &func_ids, &runtime, Some(class_name), &vtable_ids, source, &spawn_closure_fns, &fn_contracts, &singleton_data_ids, &rwlock_data_ids, &coverage_lookup)?;
                            }

                            module
                                .define_function(func_id, &mut fn_ctx)
                                .map_err(|e| CompileError::codegen(format!("define default method error for '{mangled}': {e}")))?;
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
            let mangled = mangle_method(app_name, &m.name.node);
            let sig = build_method_signature(m, &module, app_name, env);
            let func_id = module
                .declare_function(&mangled, Linkage::Local, &sig)
                .map_err(|e| CompileError::codegen(format!("declare app method error: {e}")))?;
            func_ids.insert(mangled, func_id);
        }
    }

    // Pass 1e: Declare stage methods
    for stage_spanned in &program.stages {
        let stage = &stage_spanned.node;
        let stage_name = &stage.name.node;
        for method in &stage.methods {
            let m = &method.node;
            let mangled = mangle_method(stage_name, &m.name.node);
            let sig = build_method_signature(m, &module, stage_name, env);
            let func_id = module
                .declare_function(&mangled, Linkage::Local, &sig)
                .map_err(|e| CompileError::codegen(format!("declare stage method error: {e}")))?;
            func_ids.insert(mangled, func_id);
        }
    }

    // Pass 2d: Define app method bodies
    if let Some(app_spanned) = &program.app {
        let app = &app_spanned.node;
        let app_name = &app.name.node;
        for method in &app.methods {
            let m = &method.node;
            let mangled = mangle_method(app_name, &m.name.node);
            let func_id = func_ids[&mangled];
            let sig = build_method_signature(m, &module, app_name, env);

            let mut fn_ctx = Context::new();
            fn_ctx.func.signature = sig;

            let mut builder_ctx = FunctionBuilderContext::new();
            {
                let builder = cranelift_frontend::FunctionBuilder::new(&mut fn_ctx.func, &mut builder_ctx);
                lower_function(m, builder, env, &mut module, &func_ids, &runtime, Some(app_name), &vtable_ids, source, &spawn_closure_fns, &fn_contracts, &singleton_data_ids, &rwlock_data_ids, &coverage_lookup)?;
            }

            module
                .define_function(func_id, &mut fn_ctx)
                .map_err(|e| CompileError::codegen(format!("define app method error for '{mangled}': {e}")))?;
        }
    }

    // Pass 2e: Define stage method bodies
    for stage_spanned in &program.stages {
        let stage = &stage_spanned.node;
        let stage_name = &stage.name.node;
        for method in &stage.methods {
            let m = &method.node;
            let mangled = mangle_method(stage_name, &m.name.node);
            let func_id = func_ids[&mangled];
            let sig = build_method_signature(m, &module, stage_name, env);

            let mut fn_ctx = Context::new();
            fn_ctx.func.signature = sig;

            let mut builder_ctx = FunctionBuilderContext::new();
            {
                let builder = cranelift_frontend::FunctionBuilder::new(&mut fn_ctx.func, &mut builder_ctx);
                lower_function(m, builder, env, &mut module, &func_ids, &runtime, Some(stage_name), &vtable_ids, source, &spawn_closure_fns, &fn_contracts, &singleton_data_ids, &rwlock_data_ids, &coverage_lookup)?;
            }

            module
                .define_function(func_id, &mut fn_ctx)
                .map_err(|e| CompileError::codegen(format!("define stage method error for '{mangled}': {e}")))?;
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
            // Pass a real stack anchor: the address of a slot in this entry
            // frame marks the bottom (highest address) of the program stack
            // for conservative root scanning. Previously this call passed no
            // argument while the C side reads one, so gc_stack_bottom was
            // whatever happened to sit in the first argument register —
            // sometimes NULL, which silently disabled collection entirely.
            let gc_anchor_slot = builder.create_sized_stack_slot(
                cranelift_codegen::ir::StackSlotData::new(
                    cranelift_codegen::ir::StackSlotKind::ExplicitSlot, 8, 3));
            let gc_anchor = builder.ins().stack_addr(types::I64, gc_anchor_slot, 0);
            builder.ins().call(gc_init_ref, &[gc_anchor]);

            // Initialize rwlocks for synchronized singletons/objects (test
            // binaries run no DI synthesis; see the app-main equivalent)
            for class_name in &env.synchronized_singletons {
                if let Some(&data_id) = rwlock_data_ids.get(class_name) {
                    let rwlock_init_ref = module.declare_func_in_func(runtime.get("__pluto_rwlock_init"), builder.func);
                    let call = builder.ins().call(rwlock_init_ref, &[]);
                    let lock_ptr = builder.inst_results(call)[0];
                    let gv = module.declare_data_in_func(data_id, builder.func);
                    let addr = builder.ins().global_value(types::I64, gv);
                    builder.ins().store(MemFlags::new(), lock_ptr, addr, Offset32::new(0));
                }
            }

            // Initialize coverage if enabled
            if let Some(cov_map) = coverage_map {
                let cov_init_ref = module.declare_func_in_func(runtime.get("__pluto_coverage_init"), builder.func);
                let num_points_val = builder.ins().iconst(types::I64, cov_map.num_points() as i64);

                // Embed the coverage output path as a C string
                let cov_path = ".pluto-coverage/coverage-data.bin";
                let mut cov_path_bytes = cov_path.as_bytes().to_vec();
                cov_path_bytes.push(0);
                let mut cov_data_desc = DataDescription::new();
                cov_data_desc.define(cov_path_bytes.into_boxed_slice());
                let cov_data_id = module.declare_anonymous_data(false, false)
                    .map_err(|e| CompileError::codegen(format!("declare coverage path data error: {e}")))?;
                module.define_data(cov_data_id, &cov_data_desc)
                    .map_err(|e| CompileError::codegen(format!("define coverage path data error: {e}")))?;
                let cov_gv = module.declare_data_in_func(cov_data_id, builder.func);
                let cov_path_ptr = builder.ins().global_value(types::I64, cov_gv);

                builder.ins().call(cov_init_ref, &[num_points_val, cov_path_ptr]);
            }

            let test_start_ref = module.declare_func_in_func(runtime.get("__pluto_test_start"), builder.func);
            let test_finish_ref = module.declare_func_in_func(runtime.get("__pluto_test_finish"), builder.func);
            let string_new_ref = module.declare_func_in_func(runtime.get("__pluto_string_new"), builder.func);
            let test_run_ref = module.declare_func_in_func(runtime.get("__pluto_test_run"), builder.func);

            // Block-level config from program.tests (bare tests → Sequential,
            // 1 iteration). CLI env vars still override at runtime.
            let (block_strategy, block_seed, block_iterations) = if let Some(tests_decl) = &program.tests {
                let s = match tests_decl.node.strategy.as_str() {
                    "RoundRobin" => 1i64,
                    "Random" => 2i64,
                    "Exhaustive" => 3i64,
                    _ => 0i64, // Sequential
                };
                (
                    s,
                    tests_decl.node.seed.unwrap_or(0) as i64,
                    tests_decl.node.iterations.unwrap_or(100) as i64,
                )
            } else {
                (0i64, 0i64, 1i64)
            };

            for test in &program.test_info {
                // Per-test pins override the block (rfc-test-harness):
                // a schedule pin replays its trace; a seed pin runs exactly
                // Random iteration M of seed N (one run, run_seed = N + M).
                let (strategy_int, seed_int, iterations_int) = if test.schedule.is_some() {
                    (4i64, 0i64, 1i64) // Replay
                } else if let Some(s) = test.seed {
                    let run_seed = s.wrapping_add(test.iteration.unwrap_or(0)) as i64;
                    (2i64, run_seed, 1i64) // Random, single pinned run
                } else {
                    (block_strategy, block_seed, block_iterations)
                };
                let strategy_val = builder.ins().iconst(types::I64, strategy_int);
                let seed_val = builder.ins().iconst(types::I64, seed_int);
                let iterations_val = builder.ins().iconst(types::I64, iterations_int);

                // Pinned schedule token as a NUL-terminated data object
                // (0 = no pin).
                let schedule_val = if let Some(token) = &test.schedule {
                    let mut data_desc = DataDescription::new();
                    let mut bytes = token.as_bytes().to_vec();
                    bytes.push(0);
                    data_desc.define(bytes.into_boxed_slice());
                    let data_id = module.declare_anonymous_data(false, false)
                        .map_err(|e| CompileError::codegen(format!("declare schedule data error: {e}")))?;
                    module.define_data(data_id, &data_desc)
                        .map_err(|e| CompileError::codegen(format!("define schedule data error: {e}")))?;
                    let gv = module.declare_data_in_func(data_id, builder.func);
                    builder.ins().global_value(types::I64, gv)
                } else {
                    builder.ins().iconst(types::I64, 0)
                };
                // Create Pluto string for the test name
                let mut data_desc = DataDescription::new();
                let mut bytes = test.display_name.as_bytes().to_vec();
                bytes.push(0);
                data_desc.define(bytes.into_boxed_slice());
                let data_id = module.declare_anonymous_data(false, false)
                    .map_err(|e| CompileError::codegen(format!("declare test name data error: {e}")))?;
                module.define_data(data_id, &data_desc)
                    .map_err(|e| CompileError::codegen(format!("define test name data error: {e}")))?;
                let gv = module.declare_data_in_func(data_id, builder.func);
                let raw_ptr = builder.ins().global_value(types::I64, gv);
                let len_val = builder.ins().iconst(types::I64, test.display_name.len() as i64);
                let call = builder.ins().call(string_new_ref, &[raw_ptr, len_val]);
                let name_str = builder.inst_results(call)[0];

                // call __pluto_test_start(name_str)
                builder.ins().call(test_start_ref, &[name_str]);

                // Get function pointer for the test function
                let test_func_id = func_ids.get(&test.fn_name).ok_or_else(|| {
                    CompileError::codegen(format!("missing test function '{}'", test.fn_name))
                })?;
                let test_func_ref = module.declare_func_in_func(*test_func_id, builder.func);
                let fn_addr = builder.ins().func_addr(types::I64, test_func_ref);

                // call __pluto_test_run(fn_ptr, strategy, seed, iterations, schedule)
                builder.ins().call(test_run_ref, &[fn_addr, strategy_val, seed_val, iterations_val, schedule_val]);

                // call __pluto_test_finish() — reports ok/FAILED based on whether
                // an uncaught error escaped the test body (#496).
                builder.ins().call(test_finish_ref, &[]);
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

    // Define serve handler bodies (dispatch chains over the served class's
    // methods plus entity handle calls). Done after user functions so every
    // callee FuncId exists.
    for served in &served_classes {
        let hname = format!("__pluto_serve_handler_{served}");
        let hid = func_ids[&hname];
        let mut hsig = module.make_signature();
        hsig.params.push(AbiParam::new(types::I64));
        hsig.params.push(AbiParam::new(types::I64));
        hsig.returns.push(AbiParam::new(types::I64));
        let mut fn_ctx = Context::new();
        fn_ctx.func.signature = hsig;
        let mut builder_ctx = FunctionBuilderContext::new();
        {
            let builder = cranelift_frontend::FunctionBuilder::new(&mut fn_ctx.func, &mut builder_ctx);
            lower_serve_handler(served, builder, env, &mut module, &func_ids, &runtime, &vtable_ids, source, &fn_contracts, &singleton_data_ids, &rwlock_data_ids, &coverage_lookup)?;
        }
        module
            .define_function(hid, &mut fn_ctx)
            .map_err(|e| CompileError::codegen(format!("define serve handler error: {e}")))?;
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
            // Pass a real stack anchor: the address of a slot in this entry
            // frame marks the bottom (highest address) of the program stack
            // for conservative root scanning. Previously this call passed no
            // argument while the C side reads one, so gc_stack_bottom was
            // whatever happened to sit in the first argument register —
            // sometimes NULL, which silently disabled collection entirely.
            let gc_anchor_slot = builder.create_sized_stack_slot(
                cranelift_codegen::ir::StackSlotData::new(
                    cranelift_codegen::ir::StackSlotKind::ExplicitSlot, 8, 3));
            let gc_anchor = builder.ins().stack_addr(types::I64, gc_anchor_slot, 0);
            builder.ins().call(gc_init_ref, &[gc_anchor]);

            // Initialize coverage if enabled
            if let Some(cov_map) = coverage_map {
                let cov_init_ref = module.declare_func_in_func(runtime.get("__pluto_coverage_init"), builder.func);
                let num_points_val = builder.ins().iconst(types::I64, cov_map.num_points() as i64);

                let cov_path = ".pluto-coverage/coverage-data.bin";
                let mut cov_path_bytes = cov_path.as_bytes().to_vec();
                cov_path_bytes.push(0);
                let mut cov_data_desc = DataDescription::new();
                cov_data_desc.define(cov_path_bytes.into_boxed_slice());
                let cov_data_id = module.declare_anonymous_data(false, false)
                    .map_err(|e| CompileError::codegen(format!("declare coverage path data error: {e}")))?;
                module.define_data(cov_data_id, &cov_data_desc)
                    .map_err(|e| CompileError::codegen(format!("define coverage path data error: {e}")))?;
                let cov_gv = module.declare_data_in_func(cov_data_id, builder.func);
                let cov_path_ptr = builder.ins().global_value(types::I64, cov_gv);

                builder.ins().call(cov_init_ref, &[num_points_val, cov_path_ptr]);
            }

            let alloc_ref = module.declare_func_in_func(runtime.get("__pluto_alloc"), builder.func);

            // Create singletons in topological order
            let mut singletons: HashMap<String, Value> = HashMap::new();

            for class_name in &env.di_order {
                let class_info = env.classes.get(class_name).ok_or_else(|| {
                    CompileError::codegen(format!("DI: unknown class '{}'", class_name))
                })?;
                // Transient classes have no shared instance — fresh ones are
                // created at each injection point below
                if class_info.lifecycle == crate::parser::ast::Lifecycle::Transient {
                    continue;
                }
                let size = class_info.fields.len() as i64 * POINTER_SIZE as i64;
                let size_val = builder.ins().iconst(types::I64, size);
                let call = builder.ins().call(alloc_ref, &[size_val]);
                let ptr = builder.inst_results(call)[0];

                // Wire injected fields
                for (i, (_, field_ty, is_injected)) in class_info.fields.iter().enumerate() {
                    if *is_injected && let PlutoType::Class(dep_name) = field_ty {
                        let dep_ptr = if env.classes.get(dep_name).map(|c| c.lifecycle)
                            == Some(crate::parser::ast::Lifecycle::Transient)
                        {
                            Some(emit_startup_transient(dep_name, env, &mut builder, alloc_ref, &singletons)?)
                        } else {
                            singletons.get(dep_name).copied()
                        };
                        if let Some(dep_ptr) = dep_ptr {
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

                // Non-injected (data) fields: real zero-STATE values, not
                // null pointers — empty string/containers, recursively
                // zero-constructed class/entity instances. This is what a
                // colocated `domain` dependency hands out.
                emit_startup_zero_data_fields(class_info, ptr, env, &mut builder, &mut module, &runtime)?;

                singletons.insert(class_name.clone(), ptr);

                // Store pointer to module-level global for scope block access (Phase 2)
                // and register the global as a GC root: a singleton consumed only
                // through scope blocks has no stack presence after startup, so
                // without this the collector would free it (issue #434).
                if let Some(&data_id) = singleton_data_ids.get(class_name) {
                    let gv = module.declare_data_in_func(data_id, builder.func);
                    let addr = builder.ins().global_value(types::I64, gv);
                    builder.ins().store(MemFlags::new(), ptr, addr, Offset32::new(0));
                    let reg_root = module.declare_func_in_func(
                        runtime.get("__pluto_gc_register_global_root"),
                        builder.func,
                    );
                    builder.ins().call(reg_root, &[addr]);
                }
            }

            // Initialize rwlocks for synchronized singletons (Phase 4b)
            for class_name in &env.synchronized_singletons {
                if let Some(&data_id) = rwlock_data_ids.get(class_name) {
                    let rwlock_init_ref = module.declare_func_in_func(runtime.get("__pluto_rwlock_init"), builder.func);
                    let call = builder.ins().call(rwlock_init_ref, &[]);
                    let lock_ptr = builder.inst_results(call)[0];
                    let gv = module.declare_data_in_func(data_id, builder.func);
                    let addr = builder.ins().global_value(types::I64, gv);
                    builder.ins().store(MemFlags::new(), lock_ptr, addr, Offset32::new(0));
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
                if *is_injected && let PlutoType::Class(dep_name) = field_ty {
                    let dep_ptr = if env.classes.get(dep_name).map(|c| c.lifecycle)
                        == Some(crate::parser::ast::Lifecycle::Transient)
                    {
                        Some(emit_startup_transient(dep_name, env, &mut builder, alloc_ref, &singletons)?)
                    } else {
                        singletons.get(dep_name).copied()
                    };
                    if let Some(dep_ptr) = dep_ptr {
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

            // Call AppName$main(app_ptr)
            let app_main_mangled = mangle_method(app_name, "main");
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

    // Generate synthetic main for stage (Phase 0: single stage → standalone binary)
    if !program.stages.is_empty() && program.app.is_none() {
        let stage = &program.stages[0].node;
        let stage_name = &stage.name.node;

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
            // Pass a real stack anchor: the address of a slot in this entry
            // frame marks the bottom (highest address) of the program stack
            // for conservative root scanning. Previously this call passed no
            // argument while the C side reads one, so gc_stack_bottom was
            // whatever happened to sit in the first argument register —
            // sometimes NULL, which silently disabled collection entirely.
            let gc_anchor_slot = builder.create_sized_stack_slot(
                cranelift_codegen::ir::StackSlotData::new(
                    cranelift_codegen::ir::StackSlotKind::ExplicitSlot, 8, 3));
            let gc_anchor = builder.ins().stack_addr(types::I64, gc_anchor_slot, 0);
            builder.ins().call(gc_init_ref, &[gc_anchor]);

            // Initialize coverage if enabled
            if let Some(cov_map) = coverage_map {
                let cov_init_ref = module.declare_func_in_func(runtime.get("__pluto_coverage_init"), builder.func);
                let num_points_val = builder.ins().iconst(types::I64, cov_map.num_points() as i64);

                let cov_path = ".pluto-coverage/coverage-data.bin";
                let mut cov_path_bytes = cov_path.as_bytes().to_vec();
                cov_path_bytes.push(0);
                let mut cov_data_desc = DataDescription::new();
                cov_data_desc.define(cov_path_bytes.into_boxed_slice());
                let cov_data_id = module.declare_anonymous_data(false, false)
                    .map_err(|e| CompileError::codegen(format!("declare coverage path data error: {e}")))?;
                module.define_data(cov_data_id, &cov_data_desc)
                    .map_err(|e| CompileError::codegen(format!("define coverage path data error: {e}")))?;
                let cov_gv = module.declare_data_in_func(cov_data_id, builder.func);
                let cov_path_ptr = builder.ins().global_value(types::I64, cov_gv);

                builder.ins().call(cov_init_ref, &[num_points_val, cov_path_ptr]);
            }

            let alloc_ref = module.declare_func_in_func(runtime.get("__pluto_alloc"), builder.func);

            // Create singletons in topological order
            let mut singletons: HashMap<String, Value> = HashMap::new();

            for class_name in &env.di_order {
                let class_info = env.classes.get(class_name).ok_or_else(|| {
                    CompileError::codegen(format!("DI: unknown class '{}'", class_name))
                })?;
                // Transient classes have no shared instance — fresh ones are
                // created at each injection point below
                if class_info.lifecycle == crate::parser::ast::Lifecycle::Transient {
                    continue;
                }
                let size = class_info.fields.len() as i64 * POINTER_SIZE as i64;
                let size_val = builder.ins().iconst(types::I64, size);
                let call = builder.ins().call(alloc_ref, &[size_val]);
                let ptr = builder.inst_results(call)[0];

                // Wire injected fields
                for (i, (_, field_ty, is_injected)) in class_info.fields.iter().enumerate() {
                    if *is_injected && let PlutoType::Class(dep_name) = field_ty {
                        let dep_ptr = if env.classes.get(dep_name).map(|c| c.lifecycle)
                            == Some(crate::parser::ast::Lifecycle::Transient)
                        {
                            Some(emit_startup_transient(dep_name, env, &mut builder, alloc_ref, &singletons)?)
                        } else {
                            singletons.get(dep_name).copied()
                        };
                        if let Some(dep_ptr) = dep_ptr {
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

                // Non-injected (data) fields: real zero-STATE values, as in
                // the app startup wiring above.
                emit_startup_zero_data_fields(class_info, ptr, env, &mut builder, &mut module, &runtime)?;

                singletons.insert(class_name.clone(), ptr);

                // Store to the module-level global and register it as a GC
                // root (see the app-main path above; issue #434).
                if let Some(&data_id) = singleton_data_ids.get(class_name) {
                    let gv = module.declare_data_in_func(data_id, builder.func);
                    let addr = builder.ins().global_value(types::I64, gv);
                    builder.ins().store(MemFlags::new(), ptr, addr, Offset32::new(0));
                    let reg_root = module.declare_func_in_func(
                        runtime.get("__pluto_gc_register_global_root"),
                        builder.func,
                    );
                    builder.ins().call(reg_root, &[addr]);
                }
            }

            // Initialize rwlocks for synchronized singletons
            for class_name in &env.synchronized_singletons {
                if let Some(&data_id) = rwlock_data_ids.get(class_name) {
                    let rwlock_init_ref = module.declare_func_in_func(runtime.get("__pluto_rwlock_init"), builder.func);
                    let call = builder.ins().call(rwlock_init_ref, &[]);
                    let lock_ptr = builder.inst_results(call)[0];
                    let gv = module.declare_data_in_func(data_id, builder.func);
                    let addr = builder.ins().global_value(types::I64, gv);
                    builder.ins().store(MemFlags::new(), lock_ptr, addr, Offset32::new(0));
                }
            }

            // Allocate and wire the stage itself
            let stage_info = env.classes.get(stage_name).ok_or_else(|| {
                CompileError::codegen(format!("DI: unknown stage class '{}'", stage_name))
            })?;
            let stage_size = stage_info.fields.len() as i64 * POINTER_SIZE as i64;
            let stage_size_val = builder.ins().iconst(types::I64, stage_size);
            let stage_call = builder.ins().call(alloc_ref, &[stage_size_val]);
            let stage_ptr = builder.inst_results(stage_call)[0];

            for (i, (_, field_ty, is_injected)) in stage_info.fields.iter().enumerate() {
                if *is_injected && let PlutoType::Class(dep_name) = field_ty {
                    let dep_ptr = if env.classes.get(dep_name).map(|c| c.lifecycle)
                        == Some(crate::parser::ast::Lifecycle::Transient)
                    {
                        Some(emit_startup_transient(dep_name, env, &mut builder, alloc_ref, &singletons)?)
                    } else {
                        singletons.get(dep_name).copied()
                    };
                    if let Some(dep_ptr) = dep_ptr {
                        let offset = (i as i32) * POINTER_SIZE;
                        builder.ins().store(
                            MemFlags::new(),
                            dep_ptr,
                            stage_ptr,
                            Offset32::new(offset),
                        );
                    }
                }
            }

            // Call StageName$main(stage_ptr)
            let stage_main_mangled = mangle_method(stage_name, "main");
            let stage_main_id = func_ids.get(&stage_main_mangled).ok_or_else(|| {
                CompileError::codegen(format!("DI: missing stage main function '{}'", stage_main_mangled))
            })?;
            let stage_main_ref = module.declare_func_in_func(*stage_main_id, builder.func);
            builder.ins().call(stage_main_ref, &[stage_ptr]);

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

    if let Some(ty) = ret_type
        && ty != PlutoType::Void
    {
        sig.returns.push(AbiParam::new(pluto_to_cranelift(&ty)));
    }

    sig
}

/// Collect the set of function names that are spawn closure bodies.
/// These functions need sender_dec cleanup for captured Sender variables.
struct SpawnClosureCollector<'a> {
    names: &'a mut HashSet<String>,
}

impl Visitor for SpawnClosureCollector<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::Spawn { call, .. } = &expr.node {
            if let Expr::ClosureCreate { fn_name, .. } = &call.node {
                self.names.insert(fn_name.clone());
            }
        }
        walk_expr(self, expr);
    }
}

fn collect_spawn_closure_names(program: &Program) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut collector = SpawnClosureCollector { names: &mut names };
    collector.visit_program(program);
    names
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

    if let Some(ty) = ret_type
        && ty != PlutoType::Void
    {
        sig.returns.push(AbiParam::new(pluto_to_cranelift(&ty)));
    }

    let _ = class_name;

    sig
}

/// Format an invariant expression as a human-readable string for error messages.
pub(crate) fn format_invariant_expr(expr: &Expr) -> String {
    match expr {
        Expr::IntLit(n) => n.to_string(),
        Expr::FloatLit(f) => f.to_string(),
        Expr::BoolLit(b) => b.to_string(),
        Expr::Ident(name) => name.clone(),
        Expr::FieldAccess { object, field } => {
            format!("{}.{}", format_invariant_expr(&object.node), field.node)
        }
        Expr::MethodCall { object, method, args, .. } => {
            let arg_strs: Vec<String> = args.iter().map(|a| format_invariant_expr(&a.node)).collect();
            format!("{}.{}({})", format_invariant_expr(&object.node), method.node, arg_strs.join(", "))
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
        Expr::Call { name, args, .. } => {
            let arg_strs: Vec<String> = args.iter().map(|a| format_invariant_expr(&a.node)).collect();
            format!("{}({})", name.node, arg_strs.join(", "))
        }
        Expr::StringLit(s) => format!("\"{}\"", s),
        Expr::Index { object, index } => {
            format!("{}[{}]", format_invariant_expr(&object.node), format_invariant_expr(&index.node))
        }
        _ => "<expr>".to_string(),
    }
}

/// A stable hash of a service's dispatchable interface: its method names
/// with parameter and return signatures, PLUS the type-level contract
/// clauses of every type that crosses the boundary (rfc-properties.md
/// "Evolution and honesty"). Computed identically for the served class and
/// a remote consumer's interface class (same method filter, same
/// module-prefix-independent type strings), so a version-skewed pairing of
/// independently-compiled binaries is caught at the boundary. The hash is
/// folded into the RPC method token (`method#hash`): a mismatch matches no
/// dispatch arm, so the server rejects the call instead of running it with
/// misparsed arguments.
///
/// # What hashes (the evolution surface)
///
/// - Dispatchable method signatures (as before).
/// - The FIELD LAYOUT (ordered field names + types, including nullability;
///   for enums, ordered variant names + payload field shapes) of every
///   value class/enum transitively reachable through the dispatchable
///   signatures. The record encoding's field shape is wire surface: two
///   peers that disagree on it (a reorder, rename, type change, or
///   nullable flip) would otherwise pass the hash check and silently
///   mis-assign field values (issue #424), so layout skew is refused
///   pre-dispatch like any other skew. The served class's OWN fields are
///   deliberately excluded (unless it itself crosses as a value): a
///   consumer stub mirrors the dispatchable surface and contracts, not
///   the server's private implementation fields.
/// - Type-level contract clauses of the hashed type itself and of every
///   value class transitively reachable through the dispatchable
///   signatures (through arrays/maps/sets/nullables/streams and value
///   class/enum fields; recursion stops at entities — they cross as
///   identity handles, and their own interface hash carries their own
///   contracts): `invariant` clauses (single- and two-state), `guarded_by`
///   clauses, and `satisfies` instantiations by resolved short name +
///   arguments. Property *bodies* participate through the desugared
///   invariant/guard clauses the instantiation injects, so changing a
///   property body changes every dependent interface hash.
/// - Canonicalization is the span-free pretty rendering
///   ([`format_invariant_expr`]) with type names reduced to their last
///   segment (module-prefix independence, matching the signature strings).
///
/// Method-level clauses (`requires` / `ensures` / fn-level `provides`) are
/// deliberately EXCLUDED for now: a consumer's interface stub would have
/// to carry them to keep the hashes aligned, and a stub body cannot
/// honestly discharge an `ensures` (it would have to implement it). The
/// evolution rule for what does hash: changing a contract clause on a
/// boundary-crossing type is a BREAKING change, exactly like changing a
/// signature — downstream proofs assume the clauses, so a consumer
/// compiled against the old contract must be refused, and consumers must
/// mirror the clauses in their interface declarations.
///
/// # Short-name injectivity (issue #419)
///
/// The reduction to last segments is sound only while it is injective over
/// ONE interface's wire-type set: if `a.Foo` and `b.Foo` both crossed the
/// same boundary, their contracts and signature strings would label
/// identically, and a producer/consumer pair that swapped the clauses
/// between the two types would hash equal — silent version skew. No
/// deterministic disambiguation can be derived from the hashed surface
/// itself (two same-named types may be structurally identical and differ
/// only in contracts, which is exactly the skew to catch), so
/// [`check_interface_name_collisions`] REJECTS such interfaces at compile
/// time; this function may assume injectivity.
fn short_name(n: &str) -> &str {
    n.rsplit('.').next().unwrap_or(n)
}

/// Collect the named types a wire value of type `t` can carry, recursing
/// through value-class and enum-variant fields. Entities are excluded:
/// they cross as handles, and `interface_hash(entity)` carries their own
/// contracts.
fn collect_wire_types(
    env: &crate::typeck::env::TypeEnv,
    t: &PlutoType,
    out: &mut std::collections::BTreeSet<String>,
) {
    match t {
        PlutoType::Class(n) => {
            if env.object_types.contains(n) {
                return;
            }
            if out.insert(n.clone()) {
                if let Some(info) = env.classes.get(n) {
                    for (_, fty, _) in &info.fields {
                        collect_wire_types(env, fty, out);
                    }
                }
            }
        }
        PlutoType::Enum(n) => {
            if out.insert(n.clone()) {
                if let Some(info) = env.enums.get(n) {
                    for (_, fields) in &info.variants {
                        for (_, fty) in fields {
                            collect_wire_types(env, fty, out);
                        }
                    }
                }
            }
        }
        PlutoType::Array(e)
        | PlutoType::Nullable(e)
        | PlutoType::Set(e)
        | PlutoType::Stream(e) => collect_wire_types(env, e, out),
        PlutoType::Map(k, v) => {
            collect_wire_types(env, k, out);
            collect_wire_types(env, v, out);
        }
        _ => {}
    }
}

/// Canonical, module-prefix-independent rendering of a wire type, shared
/// by the signature, contract, and layout sections of the hash.
fn sig(t: &PlutoType) -> String {
    match t {
        PlutoType::Int => "int".to_string(),
        PlutoType::Float => "float".to_string(),
        PlutoType::Bool => "bool".to_string(),
        PlutoType::Byte => "byte".to_string(),
        PlutoType::Bytes => "bytes".to_string(),
        PlutoType::String => "string".to_string(),
        PlutoType::Void => "void".to_string(),
        PlutoType::Class(n) | PlutoType::Enum(n) => short_name(n).to_string(),
        PlutoType::Array(e) => format!("[{}]", sig(e)),
        PlutoType::Nullable(i) => format!("{}?", sig(i)),
        other => format!("{other}"),
    }
}

/// The dispatchable surface of `class_name`: sorted signature strings and
/// the set of boundary-crossing VALUE types reachable through those
/// signatures (the hashed class itself is excluded; consumers add it where
/// it belongs). Shared by [`interface_hash`] and
/// [`check_interface_name_collisions`] so the injectivity check covers
/// exactly the set the hash folds.
fn interface_surface(
    env: &crate::typeck::env::TypeEnv,
    class_name: &str,
) -> (Vec<String>, std::collections::BTreeSet<String>) {
    let supported = |t: &PlutoType| {
        // Top-level entities cross as handles; entities NESTED in values
        // are still untransferable (a copy would fork identity).
        if let PlutoType::Class(n) = t
            && env.object_types.contains(n)
        {
            return true;
        }
        crate::typeck::types::wire_supported(t)
            && !crate::typeck::types::contains_object_type(t, env)
    };

    let mut sigs: Vec<String> = Vec::new();
    // Value classes/enums reachable through the dispatchable signatures:
    // their field layout AND contracts are wire surface. The hashed type
    // itself is NOT in this set — each consumer adds it where it belongs
    // (its contracts hash and collide; its own field layout is not wire
    // surface, since consumer stubs do not mirror implementation fields).
    let mut value_types: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    if let Some(info) = env.classes.get(class_name) {
        for mname in &info.methods {
            let mangled = crate::typeck::env::mangle_method(class_name, mname);
            let Some(fsig) = env.functions.get(&mangled) else { continue };
            let arg_types: Vec<PlutoType> = fsig.params.iter().skip(1).cloned().collect();
            let ret = fsig.return_type.clone();
            let ret_ok = supported(&ret) || ret == PlutoType::Void;
            if arg_types.iter().all(supported) && ret_ok {
                let params = arg_types.iter().map(sig).collect::<Vec<_>>().join(",");
                sigs.push(format!("{mname}({params}){}", sig(&ret)));
                for t in arg_types.iter().chain(std::iter::once(&ret)) {
                    collect_wire_types(env, t, &mut value_types);
                }
            }
        }
    }
    sigs.sort();
    (sigs, value_types)
}

pub fn interface_hash(env: &crate::typeck::env::TypeEnv, class_name: &str) -> String {
    let (sigs, value_types) = interface_surface(env, class_name);

    // The hashed type itself always contributes its contracts (an entity's
    // own clauses live on its own hash) — but not its field layout, which
    // a consumer stub does not mirror (implementation fields are not wire
    // surface; only value types crossing the boundary are).
    let mut wire_types: std::collections::BTreeSet<String> = value_types.clone();
    wire_types.insert(class_name.to_string());

    // Contract section: every clause of every boundary-crossing type, in a
    // canonical, span-free, module-prefix-independent rendering.
    let mut contracts: Vec<String> = Vec::new();
    for tname in &wire_types {
        let label = short_name(tname);
        for spec in env.class_invariants.get(tname).map(Vec::as_slice).unwrap_or(&[]) {
            contracts.push(format!("inv {label} {}", spec.desc));
        }
        for spec in env.guarded_fields.get(tname).map(Vec::as_slice).unwrap_or(&[]) {
            contracts.push(format!(
                "guard {label}.{} ({}: {}) {}",
                spec.field_name,
                spec.binder_name,
                short_name(&spec.binder_class),
                format_invariant_expr(&spec.predicate.node)
            ));
        }
        for prop in env.class_properties.get(tname).map(Vec::as_slice).unwrap_or(&[]) {
            contracts.push(format!(
                "sat {label} {}({})",
                short_name(&prop.name),
                prop.args.join(", ")
            ));
        }
    }
    contracts.sort();

    // Layout section: the field shape of every value class/enum crossing
    // the boundary, in declaration order — a reorder, rename, type change,
    // or nullable flip on a wire-crossing type changes the hash, so
    // layout-skewed peers refuse to pair instead of swapping field values
    // (issue #424). Hidden injected fields are not wire surface (the
    // marshalers skip them). Entities never enter `value_types` — they
    // cross as identity handles, and their own hash covers their surface.
    let mut layouts: Vec<String> = Vec::new();
    for tname in &value_types {
        let label = short_name(tname);
        if let Some(info) = env.classes.get(tname) {
            let fields = info
                .fields
                .iter()
                .filter(|(_, _, is_injected)| !*is_injected)
                .map(|(fname, fty, _)| format!("{fname}:{}", sig(fty)))
                .collect::<Vec<_>>()
                .join(",");
            layouts.push(format!("class {label}{{{fields}}}"));
        } else if let Some(info) = env.enums.get(tname) {
            let variants = info
                .variants
                .iter()
                .map(|(vname, vfields)| {
                    let fs = vfields
                        .iter()
                        .map(|(fname, fty)| format!("{fname}:{}", sig(fty)))
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("{vname}{{{fs}}}")
                })
                .collect::<Vec<_>>()
                .join(",");
            layouts.push(format!("enum {label}{{{variants}}}"));
        }
    }
    // Sorted by the short-name rendering, not the BTreeSet's full-name
    // order, so module prefixes cannot affect the hash.
    layouts.sort();

    // FNV-1a over the joined surface — deterministic across builds (unlike
    // DefaultHasher), so two separately-compiled binaries agree on the hash.
    let joined = format!(
        "{};contracts:{};layout:{}",
        sigs.join(";"),
        contracts.join(";"),
        layouts.join(";")
    );
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in joined.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Short-name ambiguity in one interface's hashed surface (issue #419):
/// groups of DISTINCT full names that fold to one label. Checked over the
/// wire-type set and over the `satisfies` property names those types
/// export, since both are reduced to last segments by the hash.
pub fn interface_short_name_collisions(
    env: &crate::typeck::env::TypeEnv,
    class_name: &str,
) -> Vec<(String, Vec<String>)> {
    use std::collections::{BTreeMap, BTreeSet};
    let (_, mut wire_types) = interface_surface(env, class_name);
    // The hashed type's own contracts and properties fold into the hash
    // too, so its short name participates in the injectivity requirement.
    wire_types.insert(class_name.to_string());
    let mut groups: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for t in &wire_types {
        groups
            .entry(format!("type name '{}'", short_name(t)))
            .or_default()
            .insert(t.clone());
    }
    for t in &wire_types {
        for prop in env.class_properties.get(t).map(Vec::as_slice).unwrap_or(&[]) {
            groups
                .entry(format!("property name '{}'", short_name(&prop.name)))
                .or_default()
                .insert(prop.name.clone());
        }
    }
    groups
        .into_iter()
        .filter(|(_, g)| g.len() > 1)
        .map(|(k, g)| (k, g.into_iter().collect()))
        .collect()
}

/// Reject interfaces whose hashed surface is ambiguous under short-name
/// folding (issue #419). The hash reduces type and property names to their
/// last segment so that independently-compiled producer and consumer
/// binaries — whose module prefixes differ — agree on the hash of the same
/// interface. That reduction must be injective within one interface's wire
/// surface: with `a.Foo` and `b.Foo` both crossing one boundary, swapping
/// a contract clause between them would be hash-invisible, silencing
/// exactly the version-skew rejection #385 added. No canonical
/// disambiguation is derivable from the hashed surface itself (the
/// colliding types may be structurally identical, differing only in the
/// contracts whose movement must be detected), so the honest answer is to
/// refuse the interface and ask for a rename — which preserves the
/// cross-program agreement property for every non-colliding interface
/// unchanged.
///
/// Runs once per compile over every class that participates in a boundary:
/// served classes, remote interface types, and — when any boundary exists
/// — object (entity) types, whose methods are dispatched through the same
/// served connections keyed by their own interface hash.
pub fn check_interface_name_collisions(
    program: &crate::parser::ast::Program,
    env: &crate::typeck::env::TypeEnv,
) -> Result<(), crate::diagnostics::CompileError> {
    let mut boundary: std::collections::BTreeSet<String> =
        crate::marshal::collect_served_classes(program)
            .into_iter()
            .collect();
    boundary.extend(env.remote_types.iter().cloned());
    if !boundary.is_empty() {
        boundary.extend(env.object_types.iter().cloned());
    }
    for cname in &boundary {
        let collisions = interface_short_name_collisions(env, cname);
        let Some((label, names)) = collisions.first() else {
            continue;
        };
        let span = program
            .classes
            .iter()
            .find(|c| c.node.name.node == *cname)
            .map(|c| c.node.name.span)
            .unwrap_or_else(crate::span::Span::dummy);
        let list = names.join("', '");
        return Err(crate::diagnostics::CompileError::type_err(
            format!(
                "interface of '{cname}' is ambiguous under module-prefix-independent \
                 hashing: {label} abbreviates distinct boundary-crossing declarations \
                 '{list}'. The interface hash reduces names to their last segment so \
                 independently-compiled binaries agree on it, which requires short \
                 names to be unique within one interface's wire surface — otherwise a \
                 contract clause could move between the same-named declarations \
                 without changing the hash, and version skew would go undetected. \
                 Rename one of them (or wrap one in a differently-named type) so \
                 everything crossing this boundary has a unique name"
            ),
            span,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Spanned;

    // Helper to create Spanned nodes with dummy spans
    fn sp<T>(node: T) -> Spanned<T> {
        Spanned::new(node, crate::span::Span::dummy())
    }

    // ===== format_invariant_expr tests =====

    #[test]
    fn test_format_invariant_expr_int_lit() {
        let expr = Expr::IntLit(42);
        assert_eq!(format_invariant_expr(&expr), "42");
    }

    #[test]
    fn test_format_invariant_expr_negative_int() {
        let expr = Expr::IntLit(-100);
        assert_eq!(format_invariant_expr(&expr), "-100");
    }

    #[test]
    fn test_format_invariant_expr_float_lit() {
        let expr = Expr::FloatLit(3.14);
        assert_eq!(format_invariant_expr(&expr), "3.14");
    }

    #[test]
    fn test_format_invariant_expr_bool_true() {
        let expr = Expr::BoolLit(true);
        assert_eq!(format_invariant_expr(&expr), "true");
    }

    #[test]
    fn test_format_invariant_expr_bool_false() {
        let expr = Expr::BoolLit(false);
        assert_eq!(format_invariant_expr(&expr), "false");
    }

    #[test]
    fn test_format_invariant_expr_ident() {
        let expr = Expr::Ident("balance".to_string());
        assert_eq!(format_invariant_expr(&expr), "balance");
    }

    #[test]
    fn test_format_invariant_expr_field_access_simple() {
        let expr = Expr::FieldAccess {
            object: Box::new(sp(Expr::Ident("self".to_string()))),
            field: sp("balance".to_string()),
        };
        assert_eq!(format_invariant_expr(&expr), "self.balance");
    }

    #[test]
    fn test_format_invariant_expr_field_access_nested() {
        let expr = Expr::FieldAccess {
            object: Box::new(sp(Expr::FieldAccess {
                object: Box::new(sp(Expr::Ident("self".to_string()))),
                field: sp("account".to_string()),
            })),
            field: sp("balance".to_string()),
        };
        assert_eq!(format_invariant_expr(&expr), "self.account.balance");
    }

    #[test]
    fn test_format_invariant_expr_method_call() {
        let expr = Expr::MethodCall {
            object: Box::new(sp(Expr::Ident("self".to_string()))),
            method: sp("len".to_string()),
            args: vec![],
            type_args: vec![],
        };
        assert_eq!(format_invariant_expr(&expr), "self.len()");
    }

    #[test]
    fn test_format_invariant_expr_binop_add() {
        let expr = Expr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(sp(Expr::IntLit(1))),
            rhs: Box::new(sp(Expr::IntLit(2))),
        };
        assert_eq!(format_invariant_expr(&expr), "1 + 2");
    }

    #[test]
    fn test_format_invariant_expr_binop_all_arithmetic() {
        let ops = vec![
            (BinOp::Add, "+"),
            (BinOp::Sub, "-"),
            (BinOp::Mul, "*"),
            (BinOp::Div, "/"),
            (BinOp::Mod, "%"),
        ];

        for (op, expected_str) in ops {
            let expr = Expr::BinOp {
                op,
                lhs: Box::new(sp(Expr::Ident("a".to_string()))),
                rhs: Box::new(sp(Expr::Ident("b".to_string()))),
            };
            assert_eq!(format_invariant_expr(&expr), format!("a {} b", expected_str));
        }
    }

    #[test]
    fn test_format_invariant_expr_binop_all_comparison() {
        let ops = vec![
            (BinOp::Eq, "=="),
            (BinOp::Neq, "!="),
            (BinOp::Lt, "<"),
            (BinOp::Gt, ">"),
            (BinOp::LtEq, "<="),
            (BinOp::GtEq, ">="),
        ];

        for (op, expected_str) in ops {
            let expr = Expr::BinOp {
                op,
                lhs: Box::new(sp(Expr::Ident("x".to_string()))),
                rhs: Box::new(sp(Expr::IntLit(0))),
            };
            assert_eq!(format_invariant_expr(&expr), format!("x {} 0", expected_str));
        }
    }

    #[test]
    fn test_format_invariant_expr_binop_all_logical() {
        let ops = vec![
            (BinOp::And, "&&"),
            (BinOp::Or, "||"),
        ];

        for (op, expected_str) in ops {
            let expr = Expr::BinOp {
                op,
                lhs: Box::new(sp(Expr::BoolLit(true))),
                rhs: Box::new(sp(Expr::BoolLit(false))),
            };
            assert_eq!(format_invariant_expr(&expr), format!("true {} false", expected_str));
        }
    }

    #[test]
    fn test_format_invariant_expr_binop_all_bitwise() {
        let ops = vec![
            (BinOp::BitAnd, "&"),
            (BinOp::BitOr, "|"),
            (BinOp::BitXor, "^"),
            (BinOp::Shl, "<<"),
            (BinOp::Shr, ">>"),
        ];

        for (op, expected_str) in ops {
            let expr = Expr::BinOp {
                op,
                lhs: Box::new(sp(Expr::IntLit(5))),
                rhs: Box::new(sp(Expr::IntLit(3))),
            };
            assert_eq!(format_invariant_expr(&expr), format!("5 {} 3", expected_str));
        }
    }

    #[test]
    fn test_format_invariant_expr_unary_neg() {
        let expr = Expr::UnaryOp {
            op: UnaryOp::Neg,
            operand: Box::new(sp(Expr::Ident("x".to_string()))),
        };
        assert_eq!(format_invariant_expr(&expr), "-x");
    }

    #[test]
    fn test_format_invariant_expr_unary_not() {
        let expr = Expr::UnaryOp {
            op: UnaryOp::Not,
            operand: Box::new(sp(Expr::BoolLit(true))),
        };
        assert_eq!(format_invariant_expr(&expr), "!true");
    }

    #[test]
    fn test_format_invariant_expr_unary_bitnot() {
        let expr = Expr::UnaryOp {
            op: UnaryOp::BitNot,
            operand: Box::new(sp(Expr::IntLit(42))),
        };
        assert_eq!(format_invariant_expr(&expr), "~42");
    }

    #[test]
    fn test_format_invariant_expr_old_call() {
        let expr = Expr::Call {
            name: sp("old".to_string()),
            args: vec![sp(Expr::FieldAccess {
                object: Box::new(sp(Expr::Ident("self".to_string()))),
                field: sp("balance".to_string()),
            })],
            type_args: vec![],
            target_id: None,
        };
        assert_eq!(format_invariant_expr(&expr), "old(self.balance)");
    }

    #[test]
    fn test_format_invariant_expr_complex_nested() {
        // self.balance > 0 && self.balance <= old(self.balance)
        let expr = Expr::BinOp {
            op: BinOp::And,
            lhs: Box::new(sp(Expr::BinOp {
                op: BinOp::Gt,
                lhs: Box::new(sp(Expr::FieldAccess {
                    object: Box::new(sp(Expr::Ident("self".to_string()))),
                    field: sp("balance".to_string()),
                })),
                rhs: Box::new(sp(Expr::IntLit(0))),
            })),
            rhs: Box::new(sp(Expr::BinOp {
                op: BinOp::LtEq,
                lhs: Box::new(sp(Expr::FieldAccess {
                    object: Box::new(sp(Expr::Ident("self".to_string()))),
                    field: sp("balance".to_string()),
                })),
                rhs: Box::new(sp(Expr::Call {
                    name: sp("old".to_string()),
                    args: vec![sp(Expr::FieldAccess {
                        object: Box::new(sp(Expr::Ident("self".to_string()))),
                        field: sp("balance".to_string()),
                    })],
                    type_args: vec![],
                    target_id: None,
                })),
            })),
        };
        assert_eq!(
            format_invariant_expr(&expr),
            "self.balance > 0 && self.balance <= old(self.balance)"
        );
    }

    #[test]
    fn test_format_invariant_expr_default_fallback() {
        // Unsupported expressions should return "<expr>"
        let expr = Expr::NoneLit;
        assert_eq!(format_invariant_expr(&expr), "<expr>");
    }

    #[test]
    fn test_format_invariant_expr_function_call() {
        // Function calls are formatted with name and args
        let expr = Expr::Call {
            name: sp("foo".to_string()),
            args: vec![sp(Expr::IntLit(1))],
            type_args: vec![],
            target_id: None,
        };
        assert_eq!(format_invariant_expr(&expr), "foo(1)");
    }

    #[test]
    fn test_format_invariant_expr_function_call_multiple_args() {
        // Function calls with multiple arguments
        let expr = Expr::Call {
            name: sp("bar".to_string()),
            args: vec![sp(Expr::IntLit(1)), sp(Expr::IntLit(2))],
            type_args: vec![],
            target_id: None,
        };
        assert_eq!(format_invariant_expr(&expr), "bar(1, 2)");
    }

    #[test]
    fn test_format_invariant_expr_string_lit() {
        let expr = Expr::StringLit("test".to_string());
        assert_eq!(format_invariant_expr(&expr), "\"test\"");
    }

    // ===== extract_fn_contracts tests =====

    #[test]
    fn test_extract_fn_contracts_empty() {
        let contracts: Vec<Spanned<ContractClause>> = vec![];
        assert!(extract_fn_contracts(&contracts).is_none());
    }

    #[test]
    fn test_extract_fn_contracts_only_requires() {
        let contracts = vec![
            sp(ContractClause {
                kind: ContractKind::Requires,
                expr: sp(Expr::BinOp {
                    op: BinOp::Gt,
                    lhs: Box::new(sp(Expr::Ident("x".to_string()))),
                    rhs: Box::new(sp(Expr::IntLit(0))),
                }),
                provenance: None,
            }),
        ];
        let result = extract_fn_contracts(&contracts).unwrap();
        assert_eq!(result.requires.len(), 1);
        assert_eq!(result.requires[0].1, "x > 0");
    }

    #[test]
    fn test_extract_fn_contracts_filtered_invariants() {
        let contracts = vec![
            sp(ContractClause {
                kind: ContractKind::Invariant,
                expr: sp(Expr::BoolLit(true)),
                provenance: None,
            }),
            sp(ContractClause {
                kind: ContractKind::Requires,
                expr: sp(Expr::BoolLit(true)),
                provenance: None,
            }),
        ];
        let result = extract_fn_contracts(&contracts).unwrap();
        assert_eq!(result.requires.len(), 1);
    }

    #[test]
    fn test_extract_fn_contracts_multiple_requires() {
        let contracts = vec![
            sp(ContractClause {
                kind: ContractKind::Requires,
                expr: sp(Expr::BinOp {
                    op: BinOp::Gt,
                    lhs: Box::new(sp(Expr::Ident("x".to_string()))),
                    rhs: Box::new(sp(Expr::IntLit(0))),
                }),
                provenance: None,
            }),
            sp(ContractClause {
                kind: ContractKind::Requires,
                expr: sp(Expr::BinOp {
                    op: BinOp::Lt,
                    lhs: Box::new(sp(Expr::Ident("x".to_string()))),
                    rhs: Box::new(sp(Expr::IntLit(100))),
                }),
                provenance: None,
            }),
        ];
        let result = extract_fn_contracts(&contracts).unwrap();
        assert_eq!(result.requires.len(), 2);
        assert_eq!(result.requires[0].1, "x > 0");
        assert_eq!(result.requires[1].1, "x < 100");
    }

    // ===== host_target_triple tests =====

    #[test]
    fn test_host_target_triple_returns_valid_string() {
        let result = host_target_triple();
        assert!(result.is_ok());
        let triple = result.unwrap();

        // Should be one of the supported triples
        let valid_triples = vec![
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "x86_64-unknown-linux-gnu",
            "aarch64-unknown-linux-gnu",
        ];
        assert!(valid_triples.contains(&triple), "Got unexpected triple: {}", triple);
    }

    #[test]
    fn test_host_target_triple_format() {
        let result = host_target_triple().unwrap();

        // Should have at least two hyphens (arch-vendor-os format, or arch-vendor-os-env on Linux)
        let hyphen_count = result.chars().filter(|c| *c == '-').count();
        assert!(hyphen_count >= 2, "Triple should have at least 2 hyphens (got {})", hyphen_count);
    }

    #[test]
    fn test_host_target_triple_arch_prefix() {
        let result = host_target_triple().unwrap();

        // Should start with a known arch
        assert!(
            result.starts_with("aarch64") || result.starts_with("x86_64"),
            "Triple should start with known arch"
        );
    }

    #[test]
    fn test_host_target_triple_os_suffix() {
        let result = host_target_triple().unwrap();

        // Should end with a known OS
        assert!(
            result.ends_with("darwin") || result.ends_with("linux-gnu"),
            "Triple should end with known OS"
        );
    }

    #[test]
    fn test_host_target_triple_consistency() {
        // Calling multiple times should return the same result
        let result1 = host_target_triple().unwrap();
        let result2 = host_target_triple().unwrap();
        assert_eq!(result1, result2);
    }

    // ===== interface_hash tests (contract-aware hashing, phase 5.5) =====

    /// Run the full frontend on a source string and return the TypeEnv the
    /// codegen would see.
    fn hash_env(src: &str) -> crate::typeck::env::TypeEnv {
        let mut program = crate::parse_source(src).expect("parses");
        crate::modules::resolve_qualified_access_single_file(&mut program)
            .expect("qualified-access resolution succeeds");
        let result = crate::run_frontend(&mut program, false).expect("frontend succeeds");
        result.env
    }

    const HASH_BASE: &str = "class Receipt {\n    amount: int\n}\n\nclass Billing {\n    rate: int\n\n    fn charge(self, amount: int) Receipt {\n        return Receipt { amount: amount }\n    }\n}\n\nfn main() {}\n";

    #[test]
    fn interface_hash_is_deterministic() {
        let e1 = hash_env(HASH_BASE);
        let e2 = hash_env(HASH_BASE);
        assert_eq!(interface_hash(&e1, "Billing"), interface_hash(&e2, "Billing"));
    }

    #[test]
    fn invariant_on_served_class_changes_hash() {
        let plain = hash_env(HASH_BASE);
        let with_inv = hash_env(
            "class Receipt {\n    amount: int\n}\n\nclass Billing {\n    rate: int\n    invariant self.rate >= 0\n\n    fn charge(self, amount: int) Receipt {\n        return Receipt { amount: amount }\n    }\n}\n\nfn main() {}\n",
        );
        assert_ne!(
            interface_hash(&plain, "Billing"),
            interface_hash(&with_inv, "Billing"),
            "an invariant on the served class is a wire-visible contract change"
        );
    }

    #[test]
    fn two_state_invariant_changes_hash() {
        let plain = hash_env(HASH_BASE);
        let with_inv = hash_env(
            "class Receipt {\n    amount: int\n}\n\nclass Billing {\n    rate: int\n    invariant self.rate >= old(self.rate)\n\n    fn charge(self, amount: int) Receipt {\n        return Receipt { amount: amount }\n    }\n}\n\nfn main() {}\n",
        );
        assert_ne!(interface_hash(&plain, "Billing"), interface_hash(&with_inv, "Billing"));
    }

    #[test]
    fn invariant_on_nested_wire_type_changes_hash() {
        // Receipt crosses the boundary as charge's return type: its
        // contracts are part of the interface.
        let plain = hash_env(HASH_BASE);
        let with_inv = hash_env(
            "class Receipt {\n    amount: int\n    invariant self.amount >= 0\n}\n\nclass Billing {\n    rate: int\n\n    fn charge(self, amount: int) Receipt {\n        return Receipt { amount: 1 }\n    }\n}\n\nfn main() {}\n",
        );
        assert_ne!(interface_hash(&plain, "Billing"), interface_hash(&with_inv, "Billing"));
    }

    #[test]
    fn contract_on_unrelated_type_does_not_change_hash() {
        let plain = hash_env(HASH_BASE);
        let with_other = hash_env(
            "class Receipt {\n    amount: int\n}\n\nclass Elsewhere {\n    n: int\n    invariant self.n >= 0\n}\n\nclass Billing {\n    rate: int\n\n    fn charge(self, amount: int) Receipt {\n        return Receipt { amount: amount }\n    }\n}\n\nfn main() {}\n",
        );
        assert_eq!(
            interface_hash(&plain, "Billing"),
            interface_hash(&with_other, "Billing"),
            "a contract on a type that does not cross this boundary is not part of it"
        );
    }

    #[test]
    fn satisfies_instantiation_changes_hash() {
        let plain = hash_env(HASH_BASE);
        let with_sat = hash_env(
            "property monotonic(f: field<int>) {\n    invariant f >= old(f)\n}\n\nclass Receipt {\n    amount: int\n}\n\nclass Billing satisfies monotonic(self.rate) {\n    rate: int\n\n    fn charge(self, amount: int) Receipt {\n        return Receipt { amount: amount }\n    }\n}\n\nfn main() {}\n",
        );
        assert_ne!(interface_hash(&plain, "Billing"), interface_hash(&with_sat, "Billing"));
    }

    #[test]
    fn guarded_by_changes_hash() {
        let plain = hash_env(
            "class Grant {\n    token: int\n}\n\nclass Billing {\n    rate: int\n    data: int\n\n    fn read(self) int {\n        return self.data\n    }\n}\n\nfn main() {}\n",
        );
        let with_guard = hash_env(
            "class Grant {\n    token: int\n}\n\nclass Billing {\n    rate: int\n    data: int guarded_by (g: Grant) g.token == self.rate\n\n    fn read(self) int {\n        return self.data\n    }\n}\n\nfn main() {}\n",
        );
        assert_ne!(interface_hash(&plain, "Billing"), interface_hash(&with_guard, "Billing"));
    }

    #[test]
    fn method_level_clauses_do_not_change_hash() {
        // Scope decision (documented on interface_hash): requires/ensures
        // on methods are excluded — a consumer stub cannot honestly mirror
        // them. Pin the exclusion so widening it is a deliberate act.
        let plain = hash_env(HASH_BASE);
        let with_requires = hash_env(
            "class Receipt {\n    amount: int\n}\n\nclass Billing {\n    rate: int\n\n    fn charge(self, amount: int) Receipt\n        requires amount > 0\n    {\n        return Receipt { amount: amount }\n    }\n}\n\nfn main() {}\n",
        );
        assert_eq!(
            interface_hash(&plain, "Billing"),
            interface_hash(&with_requires, "Billing")
        );
    }

    // ===== short-name injectivity (issue #419) =====

    /// Hand-build the minimal TypeEnv shape the surface traversal reads:
    /// an interface class `Billing` with one dispatchable method, plus the
    /// named wire types it carries. Module-prefixed names (`a.Foo`) cannot
    /// be produced by single-file parsing, so these tests construct the
    /// post-flattening state directly.
    fn synth_env(
        method_params: Vec<crate::typeck::types::PlutoType>,
        wire_classes: &[&str],
        invariants: &[(&str, &str)],
    ) -> crate::typeck::env::TypeEnv {
        use crate::typeck::env::{ClassInfo, FuncSig};
        use crate::typeck::types::PlutoType;
        let mut env = crate::typeck::env::TypeEnv::new();
        for cname in wire_classes {
            env.classes.insert(
                cname.to_string(),
                ClassInfo {
                    fields: vec![("n".to_string(), PlutoType::Int, false)],
                    methods: vec![],
                    impl_traits: vec![],
                    lifecycle: crate::parser::ast::Lifecycle::Singleton,
                },
            );
        }
        env.classes.insert(
            "Billing".to_string(),
            ClassInfo {
                fields: vec![],
                methods: vec!["m".to_string()],
                impl_traits: vec![],
                lifecycle: crate::parser::ast::Lifecycle::Singleton,
            },
        );
        let mut params = vec![PlutoType::Class("Billing".to_string())];
        params.extend(method_params);
        env.functions.insert(
            crate::typeck::env::mangle_method("Billing", "m"),
            FuncSig { params, return_type: PlutoType::Void },
        );
        for (cname, desc) in invariants {
            env.class_invariants.insert(
                cname.to_string(),
                vec![crate::typeck::discharge::InvariantSpec {
                    expr: Expr::BoolLit(true),
                    desc: desc.to_string(),
                    span: crate::span::Span::dummy(),
                    two_state: false,
                    provenance: None,
                }],
            );
        }
        env
    }

    #[test]
    fn colliding_wire_type_short_names_detected() {
        use crate::typeck::types::PlutoType;
        let env = synth_env(
            vec![
                PlutoType::Class("a.Foo".to_string()),
                PlutoType::Class("b.Foo".to_string()),
            ],
            &["a.Foo", "b.Foo"],
            &[],
        );
        let collisions = interface_short_name_collisions(&env, "Billing");
        assert_eq!(collisions.len(), 1);
        assert_eq!(collisions[0].0, "type name 'Foo'");
        assert_eq!(collisions[0].1, vec!["a.Foo".to_string(), "b.Foo".to_string()]);
    }

    #[test]
    fn non_colliding_cross_module_names_report_nothing() {
        use crate::typeck::types::PlutoType;
        let env = synth_env(
            vec![
                PlutoType::Class("a.Foo".to_string()),
                PlutoType::Class("b.Bar".to_string()),
            ],
            &["a.Foo", "b.Bar"],
            &[],
        );
        assert!(interface_short_name_collisions(&env, "Billing").is_empty());
    }

    #[test]
    fn module_prefix_independence_preserved_for_unique_short_names() {
        // The cross-program agreement property #385 built the reduction
        // for: the same interface compiled under different module prefixes
        // hashes identically — including its contracts.
        use crate::typeck::types::PlutoType;
        let server = synth_env(
            vec![PlutoType::Class("a.Foo".to_string())],
            &["a.Foo"],
            &[("a.Foo", "self.n >= 0")],
        );
        let consumer = synth_env(
            vec![PlutoType::Class("b.Foo".to_string())],
            &["b.Foo"],
            &[("b.Foo", "self.n >= 0")],
        );
        assert_eq!(
            interface_hash(&server, "Billing"),
            interface_hash(&consumer, "Billing")
        );
        assert!(interface_short_name_collisions(&server, "Billing").is_empty());
    }

    #[test]
    fn contract_swap_between_same_named_types_is_caught_by_rejection() {
        // The sensitivity gap the issue demonstrates: with `a.Foo` and
        // `b.Foo` in one surface, moving the invariant from one to the
        // other is hash-INVISIBLE — which is exactly why such surfaces are
        // refused before hashing rather than disambiguated.
        use crate::typeck::types::PlutoType;
        let params = || {
            vec![
                PlutoType::Class("a.Foo".to_string()),
                PlutoType::Class("b.Foo".to_string()),
            ]
        };
        let producer = synth_env(params(), &["a.Foo", "b.Foo"], &[("a.Foo", "self.n >= 0")]);
        let skewed = synth_env(params(), &["a.Foo", "b.Foo"], &[("b.Foo", "self.n >= 0")]);
        assert_eq!(
            interface_hash(&producer, "Billing"),
            interface_hash(&skewed, "Billing"),
            "the collision really is hash-invisible (why rejection is required)"
        );
        assert!(!interface_short_name_collisions(&producer, "Billing").is_empty());
        assert!(!interface_short_name_collisions(&skewed, "Billing").is_empty());
    }

    #[test]
    fn colliding_property_short_names_detected() {
        use crate::typeck::env::ProvidedProperty;
        use crate::typeck::types::PlutoType;
        let mut env = synth_env(
            vec![PlutoType::Class("a.Foo".to_string())],
            &["a.Foo"],
            &[],
        );
        env.class_properties.insert(
            "Billing".to_string(),
            vec![ProvidedProperty {
                name: "verify.monotonic".to_string(),
                args: vec!["self.n".to_string()],
            }],
        );
        env.class_properties.insert(
            "a.Foo".to_string(),
            vec![ProvidedProperty {
                name: "monotonic".to_string(),
                args: vec!["self.n".to_string()],
            }],
        );
        let collisions = interface_short_name_collisions(&env, "Billing");
        assert_eq!(collisions.len(), 1);
        assert_eq!(collisions[0].0, "property name 'monotonic'");
        assert_eq!(
            collisions[0].1,
            vec!["monotonic".to_string(), "verify.monotonic".to_string()]
        );
    }

    #[test]
    fn same_property_name_on_two_types_is_not_a_collision() {
        // The SAME full property name exported by two wire types folds to
        // one entry — injective, no ambiguity.
        use crate::typeck::env::ProvidedProperty;
        use crate::typeck::types::PlutoType;
        let mut env = synth_env(
            vec![PlutoType::Class("a.Foo".to_string())],
            &["a.Foo"],
            &[],
        );
        for t in ["Billing", "a.Foo"] {
            env.class_properties.insert(
                t.to_string(),
                vec![ProvidedProperty {
                    name: "verify.monotonic".to_string(),
                    args: vec!["self.n".to_string()],
                }],
            );
        }
        assert!(interface_short_name_collisions(&env, "Billing").is_empty());
    }

    #[test]
    fn same_contracts_same_hash_across_programs() {
        // The consumer-mirroring story: two independently-compiled programs
        // declaring the same interface WITH the same contracts agree.
        let server = hash_env(
            "class Receipt {\n    amount: int\n    invariant self.amount >= 0\n}\n\nclass Billing {\n    rate: int\n    invariant self.rate >= 0\n\n    fn charge(self, amount: int) Receipt {\n        return Receipt { amount: 1 }\n    }\n}\n\nfn main() {}\n",
        );
        let stub = hash_env(
            "class Receipt {\n    amount: int\n    invariant self.amount >= 0\n}\n\nclass Billing {\n    rate: int\n    invariant self.rate >= 0\n\n    fn charge(self, amount: int) Receipt {\n        return Receipt { amount: 0 }\n    }\n}\n\nfn main() {}\n",
        );
        assert_eq!(interface_hash(&server, "Billing"), interface_hash(&stub, "Billing"));
    }

    // ===== interface_hash layout tests (field shape is wire surface, #424) =====

    const LAYOUT_BASE: &str = "class Person {\n    first: string\n    last: string\n}\n\nclass Registry {\n    pad: int\n\n    fn whois(self, key: int) Person {\n        return Person { first: \"a\", last: \"b\" }\n    }\n}\n\nfn main() {}\n";

    #[test]
    fn field_reorder_on_wire_type_changes_hash() {
        // The #424 repro shape: same fields, same types, different order.
        // Positionally-compatible on the wire — and silently value-swapping
        // — so the hash MUST split them.
        let base = hash_env(LAYOUT_BASE);
        let reordered = hash_env(
            "class Person {\n    last: string\n    first: string\n}\n\nclass Registry {\n    pad: int\n\n    fn whois(self, key: int) Person {\n        return Person { first: \"a\", last: \"b\" }\n    }\n}\n\nfn main() {}\n",
        );
        assert_ne!(
            interface_hash(&base, "Registry"),
            interface_hash(&reordered, "Registry"),
            "field order of a wire-crossing class is interface surface"
        );
    }

    #[test]
    fn field_rename_on_wire_type_changes_hash() {
        let base = hash_env(LAYOUT_BASE);
        let renamed = hash_env(
            "class Person {\n    given: string\n    last: string\n}\n\nclass Registry {\n    pad: int\n\n    fn whois(self, key: int) Person {\n        return Person { given: \"a\", last: \"b\" }\n    }\n}\n\nfn main() {}\n",
        );
        assert_ne!(interface_hash(&base, "Registry"), interface_hash(&renamed, "Registry"));
    }

    #[test]
    fn nullable_flip_on_wire_type_changes_hash() {
        // The latent skew from #424's narrative: `nick: string?` vs
        // `nick: string` only failed when a none actually flowed. With
        // layout hashed, the pairing is refused pre-dispatch.
        let nullable = hash_env(
            "class Rec {\n    nick: string?\n    age: int\n}\n\nclass Svc {\n    pad: int\n\n    fn get(self, k: int) Rec {\n        return Rec { nick: none, age: 1 }\n    }\n}\n\nfn main() {}\n",
        );
        let plain = hash_env(
            "class Rec {\n    nick: string\n    age: int\n}\n\nclass Svc {\n    pad: int\n\n    fn get(self, k: int) Rec {\n        return Rec { nick: \"x\", age: 1 }\n    }\n}\n\nfn main() {}\n",
        );
        assert_ne!(
            interface_hash(&nullable, "Svc"),
            interface_hash(&plain, "Svc"),
            "a nullable flip on a wire-crossing field is a layout change"
        );
    }

    #[test]
    fn enum_variant_shape_changes_hash() {
        let base = hash_env(
            "enum Shape {\n    Circle { radius: int }\n    Dot\n}\n\nclass Svc {\n    pad: int\n\n    fn get(self, k: int) Shape {\n        return Shape.Dot\n    }\n}\n\nfn main() {}\n",
        );
        let changed = hash_env(
            "enum Shape {\n    Circle { diameter: int }\n    Dot\n}\n\nclass Svc {\n    pad: int\n\n    fn get(self, k: int) Shape {\n        return Shape.Dot\n    }\n}\n\nfn main() {}\n",
        );
        assert_ne!(
            interface_hash(&base, "Svc"),
            interface_hash(&changed, "Svc"),
            "a variant payload shape change on a wire-crossing enum is a layout change"
        );
    }

    #[test]
    fn same_layout_same_hash_across_programs() {
        // Two independently-compiled programs declaring the same wire-type
        // layout agree — method bodies and field values do not hash.
        let server = hash_env(LAYOUT_BASE);
        let stub = hash_env(
            "class Person {\n    first: string\n    last: string\n}\n\nclass Registry {\n    pad: int\n\n    fn whois(self, key: int) Person {\n        return Person { first: \"x\", last: \"y\" }\n    }\n}\n\nfn main() {}\n",
        );
        assert_eq!(interface_hash(&server, "Registry"), interface_hash(&stub, "Registry"));
    }

    #[test]
    fn served_class_own_fields_do_not_change_hash() {
        // A consumer stub mirrors the dispatchable surface, not the
        // server's private implementation fields (the existing stubs in the
        // distributed tests omit them) — so the served class's own layout
        // must stay out of the hash unless it itself crosses as a value.
        let server = hash_env(LAYOUT_BASE);
        let stub = hash_env(
            "class Person {\n    first: string\n    last: string\n}\n\nclass Registry {\n    cache: string\n    hits: int\n\n    fn whois(self, key: int) Person {\n        return Person { first: \"x\", last: \"y\" }\n    }\n}\n\nfn main() {}\n",
        );
        assert_eq!(
            interface_hash(&server, "Registry"),
            interface_hash(&stub, "Registry"),
            "implementation fields of the served class are not wire surface"
        );
    }
}
