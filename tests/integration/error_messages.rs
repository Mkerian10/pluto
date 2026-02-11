mod common;
use common::compile_should_fail_with;

// Tests for error message quality - ensuring error messages include helpful context
// These tests verify that compiler error messages include type information
// to help users understand what went wrong

#[test]
fn for_loop_on_invalid_type_shows_type() {
    // Verify that when a for loop is used with an invalid type,
    // the error message shows what type was actually found
    compile_should_fail_with(
        r#"
fn main() {
    let x = 42
    for i in x {
        print(i)
    }
}
"#,
        "for loop requires array, range, string, bytes, receiver, or stream, found int",
    );
}

#[test]
fn invalid_cast_shows_types() {
    // Verify that invalid casts show both source and target types
    compile_should_fail_with(
        r#"
fn main() {
    let x = "hello"
    let y = x as int
    print(y)
}
"#,
        "cannot cast from string to int",
    );
}

// Note: Some error messages (like "match requires enum type") are only
// reachable during codegen if there's an internal compiler inconsistency,
// so they're not easily testable without creating invalid compiler states.
// The improvements to those messages are still valuable for debugging.
