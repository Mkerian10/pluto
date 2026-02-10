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
        reg.declare(module, "__pluto_string_len", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_string_contains", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_string_starts_with", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_string_ends_with", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_string_index_of", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_string_substring", &[types::I64, types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_string_trim", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_string_to_upper", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_string_to_lower", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_string_replace", &[types::I64, types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_string_split", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_string_char_at", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_int_to_string", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_float_to_string", &[types::F64], &[types::I64])?;
        reg.declare(module, "__pluto_bool_to_string", &[types::I32], &[types::I64])?; // I32 for C ABI
        reg.declare(module, "__pluto_string_to_int", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_string_to_float", &[types::I64], &[types::I64])?;

        // Error handling
        reg.declare(module, "__pluto_raise_error", &[types::I64], &[])?;
        reg.declare(module, "__pluto_has_error", &[], &[types::I64])?;
        reg.declare(module, "__pluto_get_error", &[], &[types::I64])?;
        reg.declare(module, "__pluto_clear_error", &[], &[])?;

        // Time
        reg.declare(module, "__pluto_time_ns", &[], &[types::I64])?;
        reg.declare(module, "__pluto_time_wall_ns", &[], &[types::I64])?;
        reg.declare(module, "__pluto_time_sleep_ns", &[types::I64], &[])?;

        // Random
        reg.declare(module, "__pluto_random_seed", &[types::I64], &[])?;
        reg.declare(module, "__pluto_random_int", &[], &[types::I64])?;
        reg.declare(module, "__pluto_random_float", &[], &[types::F64])?;

        // Math builtins
        reg.declare(module, "__pluto_abs_int", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_min_int", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_max_int", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_pow_int", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_abs_float", &[types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_min_float", &[types::F64, types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_max_float", &[types::F64, types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_pow_float", &[types::F64, types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_sqrt", &[types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_floor", &[types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_ceil", &[types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_round", &[types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_sin", &[types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_cos", &[types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_tan", &[types::F64], &[types::F64])?;
        reg.declare(module, "__pluto_log", &[types::F64], &[types::F64])?;

        // Array functions
        reg.declare(module, "__pluto_array_new", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_array_push", &[types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_array_get", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_array_set", &[types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_array_len", &[types::I64], &[types::I64])?;

        // Bytes functions
        reg.declare(module, "__pluto_bytes_new", &[], &[types::I64])?;
        reg.declare(module, "__pluto_bytes_push", &[types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_bytes_get", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_bytes_set", &[types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_bytes_len", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_bytes_to_string", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_string_to_bytes", &[types::I64], &[types::I64])?;

        // Map functions
        reg.declare(module, "__pluto_map_new", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_map_insert", &[types::I64, types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_map_get", &[types::I64, types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_map_contains", &[types::I64, types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_map_remove", &[types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_map_len", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_map_keys", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_map_values", &[types::I64], &[types::I64])?;

        // Set functions
        reg.declare(module, "__pluto_set_new", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_set_insert", &[types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_set_contains", &[types::I64, types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_set_remove", &[types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_set_len", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_set_to_array", &[types::I64], &[types::I64])?;

        // GC
        reg.declare(module, "__pluto_gc_init", &[], &[])?;
        reg.declare(module, "__pluto_gc_heap_size", &[], &[types::I64])?;

        // Concurrency
        reg.declare(module, "__pluto_task_spawn", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_task_get", &[types::I64], &[types::I64])?;

        // Channels
        reg.declare(module, "__pluto_chan_create", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_chan_send", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_chan_recv", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_chan_try_send", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_chan_try_recv", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_chan_close", &[types::I64], &[])?;
        reg.declare(module, "__pluto_chan_sender_inc", &[types::I64], &[])?;
        reg.declare(module, "__pluto_chan_sender_dec", &[types::I64], &[])?;
        reg.declare(module, "__pluto_select", &[types::I64, types::I64, types::I64], &[types::I64])?;

        // Contracts
        reg.declare(module, "__pluto_invariant_violation", &[types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_requires_violation", &[types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_ensures_violation", &[types::I64, types::I64], &[])?;

        // Test framework
        reg.declare(module, "__pluto_expect_equal_int", &[types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_expect_equal_float", &[types::F64, types::F64, types::I64], &[])?;
        reg.declare(module, "__pluto_expect_equal_bool", &[types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_expect_equal_string", &[types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_expect_true", &[types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_expect_false", &[types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_test_start", &[types::I64], &[])?;
        reg.declare(module, "__pluto_test_pass", &[], &[])?;
        reg.declare(module, "__pluto_test_summary", &[types::I64], &[])?;

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
