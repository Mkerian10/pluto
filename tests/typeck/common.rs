//! Common test utilities for typeck tests.
//! Re-exports helpers from tests/integration/common.

// Include the integration common module
#[path = "../integration/common/mod.rs"]
mod integration_common;

// Re-export test helpers
// Each typeck test crate includes this file and uses its own subset.
#[allow(unused_imports)]
pub use integration_common::{compile_and_run, compile_and_run_stdout, compile_should_fail_with};
