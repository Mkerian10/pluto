use std::collections::HashMap;

use cranelift_codegen::ir::{types, AbiParam};
use cranelift_module::{FuncId, Linkage, Module};

use crate::diagnostics::CompileError;

/// Registry of runtime (builtins.c) functions declared in the Cranelift module.
pub struct RuntimeRegistry {
    ids: HashMap<&'static str, FuncId>,
}

impl RuntimeRegistry {
    /// Declare all runtime functions in the module. Each entry specifies raw Cranelift
    /// types for parameters and returns, preserving exact C ABI compatibility.
    pub fn new(module: &mut dyn Module) -> Result<Self, CompileError> {
        let mut reg = RuntimeRegistry {
            ids: HashMap::new(),
        };

        // Builtin methods on primitive/collection receivers: these externs
        // derive from the registry (typeck/builtins.rs), so declaring a new
        // method there links it automatically. ABI mirrors the derived call
        // shape in lower_builtin_runtime_call: receiver first (F64 for a
        // float receiver, pointer/I64 otherwise), optional key-type tag,
        // then arguments — F64 for floats, I64 for everything else (slots,
        // pointers, and bytes widened for the C ABI). Bool returns are I64
        // at the C boundary (reduced to I8 after the call); Void returns
        // nothing. Inline-lowered methods don't call a dedicated runtime
        // function and are declared by hand below where they need one
        // (e.g. __pluto_bool_to_string).
        {
            use crate::typeck::builtins::{Lowering, Receiver, SymTy, Tag, BUILTIN_METHODS};
            for m in BUILTIN_METHODS {
                let Lowering::Runtime { symbol, tag } = m.lowering else {
                    continue;
                };
                let mut params = vec![match m.receiver {
                    Receiver::Float => types::F64,
                    _ => types::I64,
                }];
                if tag == Tag::AfterRecv {
                    params.push(types::I64);
                }
                for p in m.params {
                    params.push(match p.ty {
                        SymTy::Float => types::F64,
                        _ => types::I64,
                    });
                }
                if tag == Tag::AfterArgs {
                    params.push(types::I64);
                }
                let returns: &[types::Type] = match m.ret {
                    SymTy::Void => &[],
                    SymTy::Float => &[types::F64],
                    _ => &[types::I64],
                };
                reg.declare(module, symbol, &params, returns)?;
            }
        }

        // Print functions
        reg.declare(module, "__pluto_print_int", &[types::I64], &[])?;
        reg.declare(module, "__pluto_print_float", &[types::F64], &[])?;
        reg.declare(module, "__pluto_print_string", &[types::I64], &[])?;
        reg.declare(module, "__pluto_print_bool", &[types::I32], &[])?; // I32 for C ABI

        // Memory
        reg.declare(module, "__pluto_alloc", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_trait_wrap", &[types::I64, types::I64], &[types::I64])?;

        // String functions
        reg.declare(module, "__pluto_string_new", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_string_concat", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_string_eq", &[types::I64, types::I64], &[types::I32])?; // I32 for C ABI
        reg.declare(module, "__pluto_bool_to_string", &[types::I32], &[types::I64])?; // I32 for C ABI

        // String slice escape (materializes slices to owned strings at escape boundaries)
        reg.declare(module, "__pluto_string_escape", &[types::I64], &[types::I64])?;

        // Remote calls (Phase 2 transport)
        reg.declare(module, "__pluto_remote_request", &[types::I64, types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_domain_request", &[types::I64, types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_domain_bound", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_parse_long", &[types::I64], &[types::I64])?;
        // Serve side (generated RPC server)
        reg.declare(module, "__pluto_serve_listen", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_serve_accept", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_serve_port", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_fork", &[], &[types::I64])?;
        reg.declare(module, "__pluto_process_exit", &[types::I64], &[])?;
        reg.declare(module, "__pluto_wire_escape", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_wire_parse_float", &[types::I64], &[types::F64])?;
        reg.declare(module, "__pluto_wire_opt_wrap", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_wire_opt_is_none", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_wire_opt_payload", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_wire_unescape", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_request_field", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_socket_read", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_socket_write", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_socket_close", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_read_framed", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_write_framed", &[types::I64, types::I64], &[types::I64])?;

        // Error handling
        reg.declare(module, "__pluto_raise_error", &[types::I64], &[])?;
        reg.declare(module, "__pluto_has_error", &[], &[types::I64])?;
        reg.declare(module, "__pluto_get_error", &[], &[types::I64])?;
        reg.declare(module, "__pluto_clear_error", &[], &[])?;
        reg.declare(module, "__pluto_set_error_type", &[types::I64], &[])?;
        reg.declare(module, "__pluto_error_type", &[], &[types::I64])?;

        // Marshal cycle guard (#425): ancestor stack bracketing __marshal_<T>
        reg.declare(module, "__pluto_marshal_enter", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_marshal_exit", &[], &[])?;

        // Boundary-failure classification (definite vs ambiguous)
        reg.declare(module, "__pluto_boundary_failure_definite", &[], &[types::I64])?;
        reg.declare(module, "__pluto_boundary_failure_reason", &[], &[types::I64])?;

        // Time
        reg.declare(module, "__pluto_time_ns", &[], &[types::I64])?;
        reg.declare(module, "__pluto_time_wall_ns", &[], &[types::I64])?;
        reg.declare(module, "__pluto_time_sleep_ns", &[types::I64], &[])?;

        // Random
        reg.declare(module, "__pluto_random_seed", &[types::I64], &[])?;
        reg.declare(module, "__pluto_random_int", &[], &[types::I64])?;
        reg.declare(module, "__pluto_random_float", &[], &[types::F64])?;

        // Environment variables
        reg.declare(module, "__pluto_env_get", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_env_get_or", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_env_set", &[types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_env_exists", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_env_list_names", &[], &[types::I64])?;
        reg.declare(module, "__pluto_env_clear", &[types::I64], &[types::I64])?;

        // Math builtins
        reg.declare(module, "__pluto_min_int", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_max_int", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_pow_int", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_min_float", &[types::F64, types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_max_float", &[types::F64, types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_pow_float", &[types::F64, types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_sin", &[types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_cos", &[types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_tan", &[types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_log", &[types::F64], &[types::F64])?;

        // Array functions
        reg.declare(module, "__pluto_array_new", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_array_get", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_array_set", &[types::I64, types::I64, types::I64], &[])?;

        // Bytes functions
        reg.declare(module, "__pluto_bytes_new", &[], &[types::I64])?;
        reg.declare(module, "__pluto_bytes_get", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_bytes_set", &[types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_bytes_filled", &[types::I64, types::I64], &[types::I64])?;

        // Map functions
        reg.declare(module, "__pluto_map_new", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_map_get", &[types::I64, types::I64, types::I64], &[types::I64])?;

        // Set functions
        reg.declare(module, "__pluto_set_new", &[types::I64], &[types::I64])?;

        // GC
        reg.declare(module, "__pluto_gc_init", &[types::I64], &[])?;
        reg.declare(module, "__pluto_gc_register_global_root", &[types::I64], &[])?;
        reg.declare(module, "__pluto_gc_heap_size", &[], &[types::I64])?;
        reg.declare(module, "__pluto_safepoint", &[], &[])?;

        // Concurrency
        reg.declare(module, "__pluto_task_spawn", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_task_get", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_task_detach", &[types::I64], &[])?;
        reg.declare(module, "__pluto_task_cancel", &[types::I64], &[])?;
        reg.declare(module, "__pluto_deep_copy", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_deep_eq", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_alloc_entity", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_entity_encode", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_entity_decode", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_entity_guard", &[types::I64], &[])?;
        reg.declare(module, "__pluto_entity_request", &[types::I64, types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_entity_resolve_local", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_serve_set_self_addr", &[types::I64], &[])?;
        reg.declare(module, "__pluto_serve_handler_spawn", &[types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_handle_is", &[types::I64], &[types::I64])?;

        // Rwlock synchronization
        reg.declare(module, "__pluto_rwlock_init", &[], &[types::I64])?;
        reg.declare(module, "__pluto_rwlock_rdlock", &[types::I64], &[])?;
        reg.declare(module, "__pluto_rwlock_wrlock", &[types::I64], &[])?;
        reg.declare(module, "__pluto_rwlock_unlock", &[types::I64], &[])?;
        // Per-instance entity locks (hidden trailing slot of the allocation)
        reg.declare(module, "__pluto_entity_rdlock", &[types::I64], &[])?;
        reg.declare(module, "__pluto_entity_wrlock", &[types::I64], &[])?;
        reg.declare(module, "__pluto_entity_unlock", &[types::I64], &[])?;

        // Channels
        reg.declare(module, "__pluto_chan_create", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_chan_send", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_chan_recv", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_chan_try_send", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_chan_try_recv", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_chan_recv_timeout", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_chan_close", &[types::I64], &[])?;
        reg.declare(module, "__pluto_chan_sender_inc", &[types::I64], &[])?;
        reg.declare(module, "__pluto_chan_sender_dec", &[types::I64], &[])?;
        reg.declare(module, "__pluto_select", &[types::I64, types::I64, types::I64, types::I64], &[types::I64])?;

        // Contracts
        reg.declare(module, "__pluto_requires_violation", &[types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_assert_failure", &[types::I64], &[])?;

        // Defects (integer overflow, division by zero — issue #416)
        reg.declare(module, "__pluto_defect_binop", &[types::I64, types::I64, types::I64], &[])?;

        // Test framework
        reg.declare(module, "__pluto_expect_equal_int", &[types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_expect_equal_float", &[types::F64, types::F64, types::I64], &[])?;
        reg.declare(module, "__pluto_expect_equal_bool", &[types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_expect_equal_string", &[types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_expect_true", &[types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_expect_false", &[types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_expect_raises_no_error", &[types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_expect_raises_wrong_type", &[types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_test_start", &[types::I64], &[])?;
        reg.declare(module, "__pluto_test_pass", &[], &[])?;
        reg.declare(module, "__pluto_test_summary", &[types::I64], &[])?;
        reg.declare(module, "__pluto_test_run", &[types::I64, types::I64, types::I64, types::I64, types::I64], &[])?;

        // RPC functions
        reg.declare(module, "__pluto_rpc_extract_int", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_rpc_extract_float", &[types::I64], &[types::F64])?;
        reg.declare(module, "__pluto_rpc_extract_string", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_rpc_extract_bool", &[types::I64], &[types::I64])?;

        // Coverage functions
        reg.declare(module, "__pluto_coverage_init", &[types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_coverage_hit", &[types::I64], &[])?;

        Ok(reg)
    }

    /// Look up a runtime function by its full C name.
    pub fn get(&self, name: &str) -> FuncId {
        self.ids[name]
    }

    fn declare(
        &mut self,
        module: &mut dyn Module,
        name: &'static str,
        params: &[types::Type],
        returns: &[types::Type],
    ) -> Result<(), CompileError> {
        let mut sig = module.make_signature();
        for &p in params {
            sig.params.push(AbiParam::new(p));
        }
        for &r in returns {
            sig.returns.push(AbiParam::new(r));
        }
        let id = module
            .declare_function(name, Linkage::Import, &sig)
            .map_err(|e| CompileError::codegen(format!("declare {name} error: {e}")))?;
        self.ids.insert(name, id);
        Ok(())
    }
}
