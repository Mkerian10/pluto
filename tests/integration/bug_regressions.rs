mod common;
use common::*;

// ============================================================================
// Bug #1: Nested Field Access Parsed as Enum Variant
// https://github.com/anthropics/pluto/issues/1
// Status: Active - blocks obj.field.field patterns
// ============================================================================

#[test]
#[ignore] // BUG #1: Parser treats nested field access as enum variant
fn test_bug_1_nested_field_access_basic() {
    let code = r#"
        class Inner {
            value: int
        }

        class Outer {
            inner: Inner
        }

        fn main() {
            let outer = Outer {
                inner: Inner { value: 42 }
            }
            let v = outer.inner.value
            print(v)
        }
    "#;
    assert_eq!(compile_and_run_stdout(code), "42\n");
}

#[test]
#[ignore] // BUG #1: Parser treats nested field access as enum variant
fn test_bug_1_nested_field_access_with_self() {
    let code = r#"
        class Gauge {
            count: int
        }

        class Registry {
            gauges: [Gauge]
        }

        class Metrics {
            registry: Registry

            fn get_count(self) int {
                return self.registry.gauges.len()
            }
        }

        fn main() {
            let m = Metrics {
                registry: Registry {
                    gauges: [Gauge { count: 1 }, Gauge { count: 2 }]
                }
            }
            print(m.get_count())
        }
    "#;
    assert_eq!(compile_and_run_stdout(code), "2\n");
}

#[test]
#[ignore] // BUG #1: Parser treats nested field access as enum variant
fn test_bug_1_triple_nested_field_access() {
    let code = r#"
        class A { value: int }
        class B { a: A }
        class C { b: B }

        fn main() {
            let c = C { b: B { a: A { value: 123 } } }
            print(c.b.a.value)
        }
    "#;
    assert_eq!(compile_and_run_stdout(code), "123\n");
}

#[test]
#[ignore] // BUG #1: Parser treats nested field access as enum variant
fn test_bug_1_nested_field_in_expression() {
    let code = r#"
        class Point { x: int, y: int }
        class Shape { center: Point }

        fn main() {
            let s = Shape { center: Point { x: 10, y: 20 } }
            let sum = s.center.x + s.center.y
            print(sum)
        }
    "#;
    assert_eq!(compile_and_run_stdout(code), "30\n");
}

// ============================================================================
// Bug #11: Non-Mut Self Enforcement Walker Incomplete
// Status: Active - silent correctness bug
// ============================================================================

#[test]
#[ignore] // BUG #11: Walker doesn't check if conditions
fn test_bug_11_mut_method_in_if_condition() {
    let code = r#"
        class Counter {
            value: int

            fn increment(mut self) {
                self.value = self.value + 1
            }

            fn check(self) bool {
                if self.increment() {  // ERROR: calling mut method in non-mut context
                    return true
                }
                return false
            }
        }

        fn main() {
            let c = Counter { value: 0 }
            let _ = c.check()
        }
    "#;
    compile_should_fail_with(code, "cannot call mutable method");
}

#[test]
#[ignore] // BUG #11: Walker doesn't check while conditions
fn test_bug_11_mut_method_in_while_condition() {
    let code = r#"
        class Counter {
            value: int

            fn decrement(mut self) bool {
                self.value = self.value - 1
                return self.value > 0
            }

            fn loop_down(self) {
                while self.decrement() {  // ERROR: calling mut method in non-mut context
                    print("tick")
                }
            }
        }

        fn main() {
            let c = Counter { value: 3 }
            c.loop_down()
        }
    "#;
    compile_should_fail_with(code, "cannot call mutable method");
}

#[test]
#[ignore] // BUG #11: Walker doesn't check select operands
fn test_bug_11_mut_method_in_select() {
    let code = r#"
        class Box {
            value: int

            fn take(mut self) int {
                let v = self.value
                self.value = 0
                return v
            }
        }

        fn race(self, b1: Box, b2: Box) int {
            return select {
                b1.take() => 1  // ERROR: calling mut method on immutable binding
                b2.take() => 2
            }
        }

        fn main() {
            let b1 = Box { value: 10 }
            let b2 = Box { value: 20 }
            print(race(b1, b2))
        }
    "#;
    compile_should_fail_with(code, "cannot call mutable method");
}

#[test]
#[ignore] // BUG #11: Walker doesn't check nested expressions in match
fn test_bug_11_mut_method_in_match_guard() {
    let code = r#"
        class State {
            flag: bool

            fn toggle(mut self) {
                self.flag = !self.flag
            }
        }

        enum Option {
            Some { value: int }
            None
        }

        fn process(self, s: State, opt: Option) int {
            match opt {
                Option.Some { value: v } => {
                    s.toggle()  // ERROR: calling mut method on immutable binding
                    return v
                }
                Option.None => return 0
            }
        }

        fn main() {
            let s = State { flag: false }
            let opt = Option.Some { value: 42 }
            print(process(s, opt))
        }
    "#;
    compile_should_fail_with(code, "cannot call mutable method");
}

// ============================================================================
// Bug #12: Unused-Variable Tracking Keys Collide Across Functions
// Status: Active - false negatives/positives in warnings
// ============================================================================

#[test]
#[ignore] // BUG #12: Variable tracking keys don't include function context
fn test_bug_12_same_var_name_different_functions() {
    let code = r#"
        fn foo() {
            let x = 10  // Used
            print(x)
        }

        fn bar() {
            let x = 20  // Unused - should warn, but might not due to collision
        }

        fn main() {
            foo()
            bar()
        }
    "#;
    // This should compile and warn about unused 'x' in bar(), but the tracking
    // might collide if both are at the same scope depth
    // We can't easily test for warnings, but we document the expected behavior
    compile_and_run(code);
}

#[test]
#[ignore] // BUG #12: Nested scopes with same var name at same depth
fn test_bug_12_same_depth_different_functions() {
    let code = r#"
        fn func_a() {
            if true {
                let result = 100  // Used
                print(result)
            }
        }

        fn func_b() {
            if true {
                let result = 200  // Unused - should warn
            }
        }

        fn main() {
            func_a()
            func_b()
        }
    "#;
    // Both 'result' variables are at depth 1, keys will collide without function context
    compile_and_run(code);
}

// ============================================================================
// Bug #13: Monomorphize Walker Has Silent Coverage Gaps
// Status: Active - silent correctness bug
// ============================================================================

#[test]
#[ignore] // BUG #13: Generic types in Map literals not resolved
fn test_bug_13_generic_types_in_map_literal() {
    let code = r#"
        class Box<T> {
            value: T
        }

        fn make_map<T>(val: T) Map<string, Box<T>> {
            return Map<string, Box<T>> {
                "item": Box<T> { value: val }
            }
        }

        fn main() {
            let m = make_map<int>(42)
            let b = m["item"]
            print(b.value)
        }
    "#;
    assert_eq!(compile_and_run_stdout(code), "42\n");
}

#[test]
#[ignore] // BUG #13: Generic types in Set literals not resolved
fn test_bug_13_generic_types_in_set_literal() {
    let code = r#"
        class Wrapper<T> {
            data: T
        }

        fn make_set<T>(a: T, b: T) Set<Wrapper<T>> {
            return Set<Wrapper<T>> {
                Wrapper<T> { data: a },
                Wrapper<T> { data: b }
            }
        }

        fn main() {
            let s = make_set<int>(1, 2)
            print(s.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(code), "2\n");
}

#[test]
#[ignore] // BUG #13: Generic types in nested collections not resolved
fn test_bug_13_nested_generic_collections() {
    let code = r#"
        class Pair<A, B> {
            first: A
            second: B
        }

        fn make_nested<T>(val: T) Map<string, [Pair<T, T>]> {
            let pair = Pair<T, T> { first: val, second: val }
            return Map<string, [Pair<T, T>]> {
                "items": [pair]
            }
        }

        fn main() {
            let m = make_nested<int>(99)
            let arr = m["items"]
            print(arr.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(code), "1\n");
}

#[test]
#[ignore] // BUG #13: Generic types with trait bounds in collections
fn test_bug_13_generic_trait_in_map() {
    let code = r#"
        trait Printable {
            fn to_string(self) string
        }

        class Item<T> impl Printable {
            value: T

            fn to_string(self) string {
                return "Item"
            }
        }

        fn make_trait_map<T: Printable>(item: T) Map<string, T> {
            return Map<string, T> {
                "key": item
            }
        }

        fn main() {
            let item = Item<int> { value: 42 }
            let m = make_trait_map<Item<int>>(item)
            print(m.len())
        }
    "#;
    assert_eq!(compile_and_run_stdout(code), "1\n");
}
