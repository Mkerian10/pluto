// Category 8: GC Integration Tests (30 tests)
// Validates memory allocation and garbage collection codegen correctness

use super::common::{compile_and_run, compile_and_run_stdout};

// ============================================================================
// Allocations (15 tests)
// ============================================================================

#[test]
fn test_allocate_string() {
    let src = r#"
        fn main() {
            let s = "hello world"
            print(s)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "hello world");
}

#[test]
fn test_allocate_empty_string() {
    let src = r#"
        fn main() {
            let s = ""
            print(s.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "0");
}

#[test]
fn test_allocate_large_string() {
    // 1000-character string
    let src = r#"
        fn main() {
            let s = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            print(s.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "1000");
}

#[test]
fn test_allocate_class_instance() {
    let src = r#"
        class Point {
            x: int
            y: int
        }

        fn main() {
            let p = Point { x: 10, y: 20 }
            print(p.x + p.y)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "30");
}

#[test]
fn test_allocate_nested_class_instances() {
    let src = r#"
        class Inner {
            value: int
        }

        class Outer {
            inner: Inner
            extra: int
        }

        fn main() {
            let outer = Outer { inner: Inner { value: 42 }, extra: 100 }
            print(outer.inner.value + outer.extra)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "142");
}

#[test]
fn test_allocate_array() {
    let src = r#"
        fn main() {
            let arr = [1, 2, 3, 4, 5]
            print(arr.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "5");
}

#[test]
fn test_allocate_empty_array() {
    let src = r#"
        fn main() {
            let arr: [int] = []
            print(arr.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "0");
}

#[test]
fn test_allocate_array_of_strings() {
    let src = r#"
        fn main() {
            let arr = ["hello", "world", "pluto"]
            print(arr[1])
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "world");
}

#[test]
fn test_allocate_array_of_classes() {
    let src = r#"
        class Point {
            x: int
            y: int
        }

        fn main() {
            let points = [Point { x: 1, y: 2 }, Point { x: 3, y: 4 }]
            print(points[0].x + points[1].y)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "5");
}

#[test]
fn test_allocate_closure() {
    let src = r#"
        fn main() {
            let x = 10
            let f = (y: int) => x + y
            print(f(5))
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "15");
}

#[test]
fn test_allocate_closure_with_multiple_captures() {
    let src = r#"
        fn main() {
            let a = 10
            let b = 20
            let c = 30
            let f = () => a + b + c
            print(f())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "60");
}

#[test]
fn test_allocate_1000_objects() {
    // Allocate 1000 small strings
    let src = r#"
        fn main() {
            let mut i = 0
            while i < 1000 {
                let s = "test"
                i = i + 1
            }
            print(i)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "1000");
}

#[test]
fn test_allocate_10000_objects() {
    // Allocate 10,000 objects - should trigger GC
    let src = r#"
        fn main() {
            let mut i = 0
            while i < 10000 {
                let s = "allocation test string"
                i = i + 1
            }
            print(i)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "10000");
}

#[test]
fn test_allocate_large_array() {
    // FIXED: Changed from `let arr: [int] = []` which had syntax error
    // push() mutates the array in place, doesn't return a value
    let src = r#"
        fn main() {
            let arr = [0]
            let mut i = 1
            while i < 1000 {
                arr.push(i)
                i = i + 1
            }
            print(arr.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "1000");
}

#[test]
fn test_allocate_map() {
    let src = r#"
        fn main() {
            let m = Map<string, int> { "a": 1, "b": 2, "c": 3 }
            print(m.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "3");
}

// ============================================================================
// GC Correctness (10 tests)
// ============================================================================

#[test]
fn test_object_reachable_through_local_variable() {
    // Object should NOT be collected while reachable via local
    let src = r#"
        fn main() {
            let s = "this should not be collected"
            print(s.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "28");
}

#[test]
fn test_object_reachable_through_multiple_locals() {
    let src = r#"
        fn main() {
            let s1 = "first"
            let s2 = "second"
            let s3 = "third"
            print(s1.len() + s2.len() + s3.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "16");
}

#[test]
fn test_object_unreachable_simple() {
    // Create temporary object that becomes unreachable
    let src = r#"
        fn main() {
            let mut i = 0
            while i < 100 {
                let temp = "temporary allocation"
                i = i + 1
            }
            // temp objects are unreachable after loop, should be collected
            print(i)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "100");
}

#[test]
fn test_object_reachable_through_array() {
    // Objects in arrays should not be collected
    let src = r#"
        fn main() {
            let arr = ["keep", "these", "strings"]
            let mut sum = 0
            let mut i = 0
            while i < arr.len() {
                sum = sum + arr[i].len()
                i = i + 1
            }
            print(sum)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "16");
}

#[test]
fn test_object_reachable_through_class_field() {
    // Objects referenced by class fields should not be collected
    let src = r#"
        class Container {
            data: string
        }

        fn main() {
            let c = Container { data: "important data" }
            print(c.data.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "14");
}

#[test]
fn test_object_reachable_through_nested_class_fields() {
    let src = r#"
        class Inner {
            value: string
        }

        class Outer {
            inner: Inner
        }

        fn main() {
            let outer = Outer { inner: Inner { value: "nested" } }
            print(outer.inner.value.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "6");
}

#[test]
fn test_circular_reference_two_objects() {
    // A -> B -> A circular reference
    // Both should remain alive if root is reachable
    let src = r#"
        class Node {
            value: int
            next: Node?
        }

        fn main() {
            let a = Node { value: 1, next: none }
            let b = Node { value: 2, next: a }
            // Can't actually create cycle with current syntax (no mutable fields after construction)
            // But we can test that both objects stay alive
            print(a.value + b.value)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "3");
}

#[test]
fn test_object_survival_across_allocations() {
    // Old objects should survive new allocations
    let src = r#"
        fn main() {
            let important = "keep this"
            let mut i = 0
            while i < 1000 {
                let temp = "temporary"
                i = i + 1
            }
            print(important.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "9");
}

#[test]
fn test_array_elements_survive_gc() {
    let src = r#"
        fn main() {
            let arr = ["a", "b", "c", "d", "e"]
            // Trigger some allocations
            let mut i = 0
            while i < 100 {
                let temp = "allocation"
                i = i + 1
            }
            // Array elements should still be valid
            print(arr[2])
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "c");
}

#[test]
fn test_closure_captures_survive_gc() {
    let src = r#"
        fn main() {
            let captured = "captured value"
            let f = () => captured
            // Trigger allocations
            let mut i = 0
            while i < 100 {
                let temp = "temp"
                i = i + 1
            }
            // Captured value should still be valid
            print(f())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "captured value");
}

// ============================================================================
// GC Tags (5 tests)
// ============================================================================

#[test]
fn test_string_tag_allocation() {
    // Verify string allocations work (tag is implicit via runtime)
    let src = r#"
        fn main() {
            let s1 = "string one"
            let s2 = "string two"
            let s3 = "string three"
            print(s1.len() + s2.len() + s3.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "32"); // 10 + 10 + 12
}

#[test]
fn test_class_tag_allocation() {
    // Verify class instance allocations work
    let src = r#"
        class Box {
            value: int
        }

        fn main() {
            let b1 = Box { value: 1 }
            let b2 = Box { value: 2 }
            let b3 = Box { value: 3 }
            print(b1.value + b2.value + b3.value)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "6");
}

#[test]
fn test_array_tag_allocation() {
    // Verify array allocations work
    let src = r#"
        fn main() {
            let arr1 = [1, 2, 3]
            let arr2 = [4, 5, 6]
            let arr3 = [7, 8, 9]
            print(arr1.len() + arr2.len() + arr3.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "9");
}

#[test]
fn test_map_set_tag_allocation() {
    // Verify map and set allocations work
    let src = r#"
        fn main() {
            let m = Map<int, int> { 1: 10, 2: 20 }
            let s = Set<int> { 1, 2, 3 }
            print(m.len() + s.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "5");
}

#[test]
fn test_mixed_type_allocations() {
    // Allocate multiple types in same function - verify tags don't collide
    let src = r#"
        class Point {
            x: int
            y: int
        }

        fn main() {
            let s = "string"
            let arr = [1, 2, 3]
            let p = Point { x: 10, y: 20 }
            let m = Map<int, int> { 1: 100 }

            print(s.len() + arr.len() + p.x + m.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "20");
}
