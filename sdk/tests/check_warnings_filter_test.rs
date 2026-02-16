use std::fs;

#[test]
fn test_check_filters_imported_module_warnings() {
    // Create a helper module with an unused variable
    let helper_source = r#"
pub fn helper_with_warning() int {
    let unused_var = 42
    return 100
}

pub class HelperClass {
    value: int
}
"#;

    // Create a main file that imports the helper but has no warnings itself
    let main_source = r#"
import helper

fn main() {
    helper.helper_with_warning()
}
"#;

    // Write files in a temp directory
    let test_dir = std::env::temp_dir().join("pluto_test_check_warnings_filter");
    fs::create_dir_all(&test_dir).unwrap();

    let helper_file = test_dir.join("helper.pluto");
    let main_file = test_dir.join("main.pluto");

    fs::write(&helper_file, helper_source).unwrap();
    fs::write(&main_file, main_source).unwrap();

    // Call analyze_file_with_warnings on the main file
    // This should filter out warnings from helper.pluto
    let result = pluto::analyze_file_with_warnings(&main_file, None);

    match result {
        Ok((_program, _source, _derived, warnings)) => {
            // Main file has no unused variables, so warnings should be empty
            // The unused_var warning from helper.pluto should be filtered out
            assert!(warnings.is_empty(),
                "Expected no warnings for main.pluto, but got {} warnings: {:?}",
                warnings.len(),
                warnings.iter().map(|w| &w.msg).collect::<Vec<_>>()
            );
        }
        Err(e) => {
            panic!("Failed to analyze file: {}", e);
        }
    }

    // Cleanup
    fs::remove_dir_all(&test_dir).ok();
}

#[test]
fn test_check_includes_own_file_warnings() {
    // Create a main file with its own warning
    let main_source = r#"
fn main() {
    let unused_in_main = 42
}
"#;

    let test_dir = std::env::temp_dir().join("pluto_test_check_own_warnings");
    fs::create_dir_all(&test_dir).unwrap();

    let main_file = test_dir.join("main.pluto");
    fs::write(&main_file, main_source).unwrap();

    let result = pluto::analyze_file_with_warnings(&main_file, None);

    match result {
        Ok((_program, _source, _derived, warnings)) => {
            assert_eq!(warnings.len(), 1,
                "Expected 1 warning for unused variable in main.pluto, got {}",
                warnings.len()
            );
            assert!(warnings[0].msg.contains("unused_in_main"),
                "Expected warning about 'unused_in_main', got: {}", warnings[0].msg
            );
        }
        Err(e) => {
            panic!("Failed to analyze file: {}", e);
        }
    }

    fs::remove_dir_all(&test_dir).ok();
}
