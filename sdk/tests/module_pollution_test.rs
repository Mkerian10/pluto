use pluto_sdk::Module;
use std::path::Path;

#[test]
fn test_local_declarations_filter_imports() {
    // Create a simple helper module
    let helper_source = r#"
pub fn helper_function() int {
    return 100
}

pub class HelperClass {
    value: int
}
"#;

    // Create a main file that imports the helper
    let main_source = r#"
import helper

fn local_function() int {
    return 42
}

class LocalClass {
    value: int
}

fn main() {
    let h = helper.helper_function()
    local_function()
}
"#;

    // Write files in a temp directory
    let test_dir = std::env::temp_dir().join("pluto_test_module_pollution");
    std::fs::create_dir_all(&test_dir).unwrap();

    let helper_file = test_dir.join("helper.pluto");
    let main_file = test_dir.join("main.pluto");

    std::fs::write(&helper_file, helper_source).unwrap();
    std::fs::write(&main_file, main_source).unwrap();

    // Load main module
    let module = Module::from_source_file(&main_file)
        .expect("Failed to load module");

    // Check that local_* methods filter out imports
    let local_funcs = module.local_functions();
    let all_funcs = module.functions();

    // Local functions should include our user-defined functions but NOT imported helper.* functions
    let local_names: Vec<String> = local_funcs.iter()
        .map(|f| f.name().to_string())
        .collect();

    // Verify we have local_function and main
    assert!(local_names.contains(&"local_function".to_string()),
        "Expected 'local_function' in local functions, got: {:?}", local_names);
    assert!(local_names.contains(&"main".to_string()),
        "Expected 'main' in local functions, got: {:?}", local_names);

    // Verify we DON'T have helper.helper_function (it should be filtered out)
    assert!(!local_names.iter().any(|n| n.starts_with("helper.")),
        "Expected NO functions starting with 'helper.' in local functions, got: {:?}", local_names);

    // All functions should include BOTH local and imported functions
    let all_names: Vec<String> = all_funcs.iter()
        .map(|f| f.name().to_string())
        .collect();

    // Should have helper.helper_function in all functions
    assert!(all_names.iter().any(|n| n == "helper.helper_function"),
        "Expected 'helper.helper_function' in all functions, got: {:?}", all_names);

    // Check classes
    let local_classes = module.local_classes();
    let local_class_names: Vec<String> = local_classes.iter()
        .map(|c| c.name().to_string())
        .collect();

    // Should have LocalClass
    assert!(local_class_names.contains(&"LocalClass".to_string()),
        "Expected 'LocalClass' in local classes, got: {:?}", local_class_names);

    // Should NOT have helper.HelperClass
    assert!(!local_class_names.iter().any(|n| n.starts_with("helper.")),
        "Expected NO classes starting with 'helper.' in local classes, got: {:?}", local_class_names);

    // Verify that imported declarations have '.' in their names
    let imported_funcs: Vec<_> = all_funcs.iter()
        .filter(|f| f.name().contains('.'))
        .collect();

    assert!(imported_funcs.len() > 0, "Expected some imported functions with '.' in names");

    // Cleanup
    std::fs::remove_dir_all(&test_dir).ok();
}
