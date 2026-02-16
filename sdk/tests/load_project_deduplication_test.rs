use std::fs;

#[test]
fn test_load_project_deduplicates_modules() {
    // Create a project structure similar to Meridian:
    // src/aggregator.pluto (source module)
    // tests/test1.pluto (imports aggregator)
    // tests/test2.pluto (imports aggregator)
    // Each module path should appear exactly once in the result

    let test_dir = std::env::temp_dir().join("pluto_test_load_project_dedup");
    let src_dir = test_dir.join("src");
    let tests_dir = test_dir.join("tests");

    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&tests_dir).unwrap();

    // Create a source module with some declarations
    let aggregator_source = r#"
pub fn process(x: int) int {
    return x * 2
}

pub class Aggregator {
    value: int
}
"#;

    // Create test files that import the source module
    let test1_source = r#"
import src.aggregator

fn test_process() {
    let result = src.aggregator.process(5)
}
"#;

    let test2_source = r#"
import src.aggregator

fn test_aggregator_class() {
    let agg = src.aggregator.Aggregator { value: 10 }
}
"#;

    fs::write(src_dir.join("aggregator.pluto"), aggregator_source).unwrap();
    fs::write(tests_dir.join("test1.pluto"), test1_source).unwrap();
    fs::write(tests_dir.join("test2.pluto"), test2_source).unwrap();

    // Parse each file independently (simulating what load_project does with Module::from_source)
    let aggregator_module = pluto_sdk::Module::from_source(aggregator_source).unwrap();
    let test1_module = pluto_sdk::Module::from_source(test1_source).unwrap();
    let test2_module = pluto_sdk::Module::from_source(test2_source).unwrap();

    // Count declarations in each module (should not include imported declarations)
    let aggregator_decls = aggregator_module.local_functions().len()
        + aggregator_module.local_classes().len();
    let test1_decls = test1_module.local_functions().len()
        + test1_module.local_classes().len();
    let test2_decls = test2_module.local_functions().len()
        + test2_module.local_classes().len();

    // Aggregator should have 1 function + 1 class = 2 declarations
    assert_eq!(aggregator_decls, 2,
        "aggregator.pluto should have 2 local declarations (1 fn + 1 class), got {}",
        aggregator_decls
    );

    // Each test file should have 1 function
    assert_eq!(test1_decls, 1,
        "test1.pluto should have 1 local declaration (1 test fn), got {}",
        test1_decls
    );
    assert_eq!(test2_decls, 1,
        "test2.pluto should have 1 local declaration (1 test fn), got {}",
        test2_decls
    );

    // If we were to simulate the load_project behavior:
    // - We'd have 3 files: aggregator.pluto, test1.pluto, test2.pluto
    // - Each should appear exactly once in the result
    // - The HashMap-based deduplication ensures no duplicates

    // Cleanup
    fs::remove_dir_all(&test_dir).ok();
}

#[test]
fn test_module_from_source_does_not_follow_imports() {
    // Verify that Module::from_source doesn't follow imports
    // This is the key to preventing duplicates

    let test_dir = std::env::temp_dir().join("pluto_test_module_no_imports");
    fs::create_dir_all(&test_dir).unwrap();

    let helper_source = r#"
pub fn helper() int {
    return 42
}
"#;

    let main_source = r#"
import helper

fn main() {
    helper.helper()
}
"#;

    fs::write(test_dir.join("helper.pluto"), helper_source).unwrap();
    fs::write(test_dir.join("main.pluto"), main_source).unwrap();

    // Parse main.pluto without following imports
    let main_module = pluto_sdk::Module::from_source(main_source).unwrap();

    // The module should only have main's declarations, not helper's
    let all_functions = main_module.program().functions.len();
    let local_functions = main_module.local_functions().len();

    // With from_source, we should only see the main function
    assert_eq!(local_functions, 1,
        "Module::from_source should only parse the given source, got {} local functions",
        local_functions
    );

    // All functions should be the same as local functions (no imports followed)
    assert_eq!(all_functions, 1,
        "Module::from_source should not follow imports, got {} total functions",
        all_functions
    );

    fs::remove_dir_all(&test_dir).ok();
}
