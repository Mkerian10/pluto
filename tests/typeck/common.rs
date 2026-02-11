//! Common test utilities for typeck tests.
//! Re-exports helpers from tests/integration/common.

// Include the integration common module
#[path = "../integration/common/mod.rs"]
mod integration_common;

// Re-export the compile_should_fail_with helper
pub use integration_common::compile_should_fail_with;
