// Codegen test module
// Entry point for all codegen tests

#[path = "../integration/common/mod.rs"]
pub mod common;

mod _01_type_representation;
mod _02_arithmetic;
mod _03_memory_layout;
mod _04_function_calls;
mod _05_control_flow;
mod _06_error_handling;
mod _07_concurrency;
mod _08_gc_integration;
mod _09_dependency_injection;
mod _10_contracts;
mod _11_nullable;
mod _12_edge_cases;
mod _13_codegen_correctness;
mod _14_abi_compliance;
mod _15_platform_specific;
