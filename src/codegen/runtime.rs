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
        reg.declare(module, "__pluto_int_to_string", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_float_to_string", &[types::F64], &[types::I64])?;
        reg.declare(module, "__pluto_bool_to_string", &[types::I32], &[types::I64])?; // I32 for C ABI

        // Error handling
        reg.declare(module, "__pluto_raise_error", &[types::I64], &[])?;
        reg.declare(module, "__pluto_has_error", &[], &[types::I64])?;
        reg.declare(module, "__pluto_get_error", &[], &[types::I64])?;
        reg.declare(module, "__pluto_clear_error", &[], &[])?;

        // Time
        reg.declare(module, "__pluto_time_ns", &[], &[types::I64])?;

        // Array functions
        reg.declare(module, "__pluto_array_new", &[types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_array_push", &[types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_array_get", &[types::I64, types::I64], &[types::I64])?;
        reg.declare(module, "__pluto_array_set", &[types::I64, types::I64, types::I64], &[])?;
        reg.declare(module, "__pluto_array_len", &[types::I64], &[types::I64])?;

        // GC
        reg.declare(module, "__pluto_gc_init", &[], &[])?;

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
