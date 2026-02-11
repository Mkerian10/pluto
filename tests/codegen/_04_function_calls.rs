// Category 4: Function Calls & Calling Conventions
// Tests proper code generation for function calls, method calls, closures, and parameter passing

use std::process::Command;

fn compile_and_run(source: &str) -> Result<String, String> {
    let temp_dir = tempfile::tempdir().unwrap();
    let source_file = temp_dir.path().join("test.pluto");
    let output_file = temp_dir.path().join("test");

    std::fs::write(&source_file, source).unwrap();

    let compile_result = Command::new("cargo")
        .args(&["run", "--", "compile"])
        .arg(&source_file)
        .arg("-o")
        .arg(&output_file)
        .output()
        .unwrap();

    if !compile_result.status.success() {
        return Err(String::from_utf8_lossy(&compile_result.stderr).to_string());
    }

    let run_result = Command::new(&output_file).output().unwrap();

    if !run_result.status.success() {
        return Err(format!(
            "Runtime error: {}",
            String::from_utf8_lossy(&run_result.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&run_result.stdout).to_string())
}

fn compile_and_run_exit_code(source: &str) -> Result<i32, String> {
    let temp_dir = tempfile::tempdir().unwrap();
    let source_file = temp_dir.path().join("test.pluto");
    let output_file = temp_dir.path().join("test");

    std::fs::write(&source_file, source).unwrap();

    let compile_result = Command::new("cargo")
        .args(&["run", "--", "compile"])
        .arg(&source_file)
        .arg("-o")
        .arg(&output_file)
        .output()
        .unwrap();

    if !compile_result.status.success() {
        return Err(String::from_utf8_lossy(&compile_result.stderr).to_string());
    }

    let run_result = Command::new(&output_file).output().unwrap();

    Ok(run_result.status.code().unwrap_or(-1))
}

// ============================================================================
// 1. Direct Function Calls (15 tests)
// ============================================================================

#[test]
fn test_call_zero_params() {
    let source = r#"
fn get_magic() int {
    return 42
}

fn main() int {
    return get_magic()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_call_one_param_int() {
    let source = r#"
fn double(x: int) int {
    return x * 2
}

fn main() int {
    return double(21)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_call_one_param_float() {
    let source = r#"
fn double_float(x: float) int {
    let result = x * 2.0
    return result as int
}

fn main() int {
    return double_float(21.0)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_call_one_param_string() {
    let source = r#"
fn print_greeting(name: string) {
    print("Hello, " + name)
}

fn main() {
    print_greeting("World")
}
"#;
    assert_eq!(compile_and_run(source).unwrap().trim(), "Hello, World");
}

#[test]
fn test_call_one_param_class() {
    let source = r#"
class Point {
    x: int
    y: int
}

fn get_x(p: Point) int {
    return p.x
}

fn main() int {
    let p = Point { x: 42, y: 10 }
    return get_x(p)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_call_two_params() {
    let source = r#"
fn add(a: int, b: int) int {
    return a + b
}

fn main() int {
    return add(20, 22)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_call_five_params() {
    let source = r#"
fn sum5(a: int, b: int, c: int, d: int, e: int) int {
    return a + b + c + d + e
}

fn main() int {
    return sum5(10, 10, 10, 10, 2)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_call_ten_params() {
    let source = r#"
fn sum10(a: int, b: int, c: int, d: int, e: int, f: int, g: int, h: int, i: int, j: int) int {
    return a + b + c + d + e + f + g + h + i + j
}

fn main() int {
    return sum10(1, 2, 3, 4, 5, 6, 7, 8, 9, -3)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_call_twenty_params() {
    let source = r#"
fn sum20(
    a: int, b: int, c: int, d: int, e: int,
    f: int, g: int, h: int, i: int, j: int,
    k: int, l: int, m: int, n: int, o: int,
    p: int, q: int, r: int, s: int, t: int
) int {
    return a+b+c+d+e+f+g+h+i+j+k+l+m+n+o+p+q+r+s+t
}

fn main() int {
    return sum20(
        1, 2, 3, 4, 5,
        6, 7, 8, 9, 10,
        1, 1, 1, 1, 1,
        1, 1, 1, 1, -22
    )
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_call_mixed_param_types() {
    let source = r#"
fn combine(i: int, f: float, b: bool) int {
    let base = i + (f as int)
    if b {
        return base + 10
    }
    return base
}

fn main() int {
    return combine(20, 12.0, true)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_return_void() {
    let source = r#"
fn do_nothing() {
    let x = 42
}

fn main() int {
    do_nothing()
    return 42
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_return_string() {
    let source = r#"
fn get_greeting() string {
    return "Hello"
}

fn main() {
    print(get_greeting())
}
"#;
    assert_eq!(compile_and_run(source).unwrap().trim(), "Hello");
}

#[test]
fn test_return_class() {
    let source = r#"
class Point {
    x: int
    y: int
}

fn make_point(x: int, y: int) Point {
    return Point { x: x, y: y }
}

fn main() int {
    let p = make_point(42, 10)
    return p.x
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_recursive_factorial() {
    let source = r#"
fn factorial(n: int) int {
    if n <= 1 {
        return 1
    }
    return n * factorial(n - 1)
}

fn main() int {
    return factorial(5)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 120);
}

#[test]
fn test_recursive_fibonacci() {
    let source = r#"
fn fib(n: int) int {
    if n <= 1 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}

fn main() int {
    return fib(10)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 55);
}

#[test]
fn test_mutually_recursive() {
    let source = r#"
fn is_even(n: int) bool {
    if n == 0 {
        return true
    }
    return is_odd(n - 1)
}

fn is_odd(n: int) bool {
    if n == 0 {
        return false
    }
    return is_even(n - 1)
}

fn main() int {
    if is_even(42) {
        return 1
    }
    return 0
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 1);
}

// ============================================================================
// 2. Method Calls (10 tests)
// ============================================================================

#[test]
fn test_method_with_self() {
    let source = r#"
class Counter {
    value: int

    fn get(self) int {
        return self.value
    }
}

fn main() int {
    let c = Counter { value: 42 }
    return c.get()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_method_with_mut_self() {
    let source = r#"
class Counter {
    value: int

    fn increment(mut self) {
        self.value = self.value + 1
    }

    fn get(self) int {
        return self.value
    }
}

fn main() int {
    let mut c = Counter { value: 41 }
    c.increment()
    return c.get()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_method_with_extra_params() {
    let source = r#"
class Calculator {
    base: int

    fn add(self, x: int, y: int) int {
        return self.base + x + y
    }
}

fn main() int {
    let calc = Calculator { base: 10 }
    return calc.add(20, 12)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_method_returning_self() {
    let source = r#"
class Builder {
    value: int

    fn add(mut self, x: int) Builder {
        self.value = self.value + x
        return self
    }

    fn build(self) int {
        return self.value
    }
}

fn main() int {
    let mut b = Builder { value: 10 }
    b = b.add(32)
    return b.build()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_chained_method_calls() {
    let source = r#"
class Chain {
    value: int

    fn add(mut self, x: int) Chain {
        self.value = self.value + x
        return self
    }

    fn multiply(mut self, x: int) Chain {
        self.value = self.value * x
        return self
    }

    fn get(self) int {
        return self.value
    }
}

fn main() int {
    let mut c = Chain { value: 5 }
    c = c.add(2).multiply(6)
    return c.get()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_method_on_class_with_many_fields() {
    let source = r#"
class BigStruct {
    f1: int
    f2: int
    f3: int
    f4: int
    f5: int
    f6: int
    f7: int
    f8: int
    f9: int
    f10: int

    fn sum(self) int {
        return self.f1 + self.f2 + self.f3 + self.f4 + self.f5 +
               self.f6 + self.f7 + self.f8 + self.f9 + self.f10
    }
}

fn main() int {
    let big = BigStruct {
        f1: 1, f2: 2, f3: 3, f4: 4, f5: 5,
        f6: 6, f7: 7, f8: 8, f9: 9, f10: -3
    }
    return big.sum()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_method_call_on_temp_object() {
    let source = r#"
class Point {
    x: int
    y: int

    fn get_x(self) int {
        return self.x
    }
}

fn make_point() Point {
    return Point { x: 42, y: 10 }
}

fn main() int {
    return make_point().get_x()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_method_multiple_params_mixed_types() {
    let source = r#"
class Processor {
    base: int

    fn process(self, i: int, f: float, s: string) int {
        return self.base + i + (f as int)
    }
}

fn main() int {
    let p = Processor { base: 10 }
    return p.process(20, 12.0, "ignored")
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_method_recursive() {
    let source = r#"
class Counter {
    value: int

    fn count_down(self, n: int) int {
        if n <= 0 {
            return self.value
        }
        return self.count_down(n - 1)
    }
}

fn main() int {
    let c = Counter { value: 42 }
    return c.count_down(100)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_method_calls_method() {
    let source = r#"
class Calculator {
    base: int

    fn double(self, x: int) int {
        return x * 2
    }

    fn quad(self, x: int) int {
        return self.double(self.double(x))
    }
}

fn main() int {
    let calc = Calculator { base: 0 }
    return calc.quad(10) + 2
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

// ============================================================================
// 3. Closure Calls (15 tests)
// ============================================================================

#[test]
fn test_closure_zero_captures() {
    let source = r#"
fn main() int {
    let f = () => 42
    return f()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_closure_one_capture() {
    let source = r#"
fn main() int {
    let x = 42
    let f = () => x
    return f()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_closure_five_captures() {
    let source = r#"
fn main() int {
    let a = 10
    let b = 10
    let c = 10
    let d = 10
    let e = 2
    let f = () => a + b + c + d + e
    return f()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_closure_returned_from_function() {
    let source = r#"
fn make_adder(x: int) fn() int {
    return () => x + 10
}

fn main() int {
    let f = make_adder(32)
    return f()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_closure_stored_in_struct() {
    let source = r#"
class Container {
    func: fn() int
}

fn call_it(c: Container) int {
    let f = c.func
    return f()
}

fn main() int {
    let x = 42
    let c = Container { func: () => x }
    return call_it(c)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_closure_as_function_parameter() {
    let source = r#"
fn apply(f: fn() int) int {
    return f()
}

fn main() int {
    let x = 42
    return apply(() => x)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_closure_calling_closure() {
    let source = r#"
fn apply(f: fn(int) int, x: int) int {
    return f(x)
}

fn main() int {
    let base = 20
    let adder = (x: int) => x + base
    return apply(adder, 22)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_closure_with_parameters() {
    let source = r#"
fn main() int {
    let base = 10
    let f = (x: int, y: int) => base + x + y
    return f(20, 12)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_closure_capturing_heap_string() {
    let source = r#"
fn main() {
    let s = "Hello"
    let f = () => print(s)
    f()
}
"#;
    assert_eq!(compile_and_run(source).unwrap().trim(), "Hello");
}

#[test]
fn test_closure_capturing_class() {
    let source = r#"
class Point {
    x: int
    y: int
}

fn main() int {
    let p = Point { x: 42, y: 10 }
    let f = () => p.x
    return f()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_closure_capturing_array() {
    let source = r#"
fn main() int {
    let arr = [10, 20, 30]
    let f = (i: int) => arr[i]
    return f(1) + 22
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_closure_nested_captures() {
    let source = r#"
fn main() int {
    let x = 10
    let outer = () => {
        let y = 20
        let inner = () => x + y + 12
        return inner()
    }
    return outer()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_closure_multiple_calls() {
    let source = r#"
fn main() int {
    let base = 10
    let f = (x: int) => base + x
    let a = f(5)
    let b = f(10)
    let c = f(15)
    return a + b + c - 18
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_closure_passed_multiple_times() {
    let source = r#"
fn apply_twice(f: fn(int) int, x: int) int {
    return f(f(x))
}

fn main() int {
    let add10 = (x: int) => x + 10
    return apply_twice(add10, 22)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_closure_array_of_closures() {
    let source = r#"
fn main() int {
    let x = 10
    let y = 20
    let z = 12
    let f1 = () => x
    let f2 = () => y
    let f3 = () => z
    return f1() + f2() + f3()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

// ============================================================================
// 4. Parameter Passing (10 tests)
// ============================================================================

#[test]
fn test_pass_by_value_int() {
    let source = r#"
fn modify(x: int) int {
    x = x + 100
    return x
}

fn main() int {
    let a = 42
    let b = modify(a)
    return a
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_pass_by_value_float() {
    let source = r#"
fn modify(x: float) float {
    x = x + 100.0
    return x
}

fn main() int {
    let a = 42.0
    let b = modify(a)
    return a as int
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_pass_by_value_bool() {
    let source = r#"
fn flip(x: bool) bool {
    x = !x
    return x
}

fn main() int {
    let a = true
    let b = flip(a)
    if a {
        return 42
    }
    return 0
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_pass_by_reference_class() {
    let source = r#"
class Counter {
    value: int

    fn increment(mut self) {
        self.value = self.value + 1
    }
}

fn main() int {
    let mut c = Counter { value: 41 }
    c.increment()
    return c.value
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_pass_by_reference_array() {
    let source = r#"
fn modify_array(arr: [int]) {
    arr[0] = 42
}

fn main() int {
    let arr = [0, 1, 2]
    modify_array(arr)
    return arr[0]
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_pass_by_reference_string() {
    let source = r#"
fn get_first_char(s: string) {
    print(s)
}

fn main() {
    let s = "Hello"
    get_first_char(s)
}
"#;
    assert_eq!(compile_and_run(source).unwrap().trim(), "Hello");
}

#[test]
fn test_large_struct_parameter() {
    let source = r#"
class BigStruct {
    f1: int
    f2: int
    f3: int
    f4: int
    f5: int
    f6: int
    f7: int
    f8: int
    f9: int
    f10: int
    f11: int
    f12: int
    f13: int
    f14: int
    f15: int
    f16: int
    f17: int
    f18: int
    f19: int
    f20: int
    f21: int
    f22: int
    f23: int
    f24: int
    f25: int
    f26: int
    f27: int
    f28: int
    f29: int
    f30: int
    f31: int
    f32: int
    f33: int
    f34: int
    f35: int
    f36: int
    f37: int
    f38: int
    f39: int
    f40: int
    f41: int
    f42: int
    f43: int
    f44: int
    f45: int
    f46: int
    f47: int
    f48: int
    f49: int
    f50: int
}

fn get_f42(s: BigStruct) int {
    return s.f42
}

fn main() int {
    let big = BigStruct {
        f1: 1, f2: 2, f3: 3, f4: 4, f5: 5,
        f6: 6, f7: 7, f8: 8, f9: 9, f10: 10,
        f11: 11, f12: 12, f13: 13, f14: 14, f15: 15,
        f16: 16, f17: 17, f18: 18, f19: 19, f20: 20,
        f21: 21, f22: 22, f23: 23, f24: 24, f25: 25,
        f26: 26, f27: 27, f28: 28, f29: 29, f30: 30,
        f31: 31, f32: 32, f33: 33, f34: 34, f35: 35,
        f36: 36, f37: 37, f38: 38, f39: 39, f40: 40,
        f41: 41, f42: 42, f43: 43, f44: 44, f45: 45,
        f46: 46, f47: 47, f48: 48, f49: 49, f50: 50
    }
    return get_f42(big)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_array_parameter_multiple_elements() {
    let source = r#"
fn sum_array(arr: [int]) int {
    let total = 0
    let i = 0
    while i < arr.len() {
        total = total + arr[i]
        i = i + 1
    }
    return total
}

fn main() int {
    let arr = [10, 10, 10, 10, 2]
    return sum_array(arr)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_closure_parameter_called_in_function() {
    let source = r#"
fn apply_and_double(f: fn() int) int {
    return f() * 2
}

fn main() int {
    let x = 21
    return apply_and_double(() => x)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_mixed_value_and_reference_params() {
    let source = r#"
class Point {
    x: int
    y: int
}

fn add_to_point(p: Point, a: int, b: int) int {
    return p.x + p.y + a + b
}

fn main() int {
    let p = Point { x: 10, y: 10 }
    return add_to_point(p, 10, 12)
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

// ============================================================================
// Additional Edge Cases (5+ tests)
// ============================================================================

#[test]
fn test_deeply_nested_function_calls() {
    let source = r#"
fn add1(x: int) int { return x + 1 }
fn add2(x: int) int { return add1(add1(x)) }
fn add4(x: int) int { return add2(add2(x)) }
fn add8(x: int) int { return add4(add4(x)) }

fn main() int {
    return add8(add8(add8(add8(add8(0) + 2))))
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_function_call_as_condition() {
    let source = r#"
fn is_magic(x: int) bool {
    return x == 42
}

fn main() int {
    if is_magic(42) {
        return 1
    }
    return 0
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 1);
}

#[test]
fn test_function_call_in_expression() {
    let source = r#"
fn get_base() int {
    return 20
}

fn get_offset() int {
    return 22
}

fn main() int {
    return get_base() + get_offset()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_function_call_as_array_index() {
    let source = r#"
fn get_index() int {
    return 1
}

fn main() int {
    let arr = [10, 42, 30]
    return arr[get_index()]
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_function_returning_function() {
    let source = r#"
fn make_multiplier(factor: int) fn(int) int {
    return (x: int) => x * factor
}

fn main() int {
    let double = make_multiplier(2)
    let triple = make_multiplier(3)
    return double(10) + triple(7) + 1
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_closure_capturing_closure() {
    let source = r#"
fn main() int {
    let x = 10
    let y = 20
    let z = 12
    let f = () => x + y + z
    return f()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_method_call_chain_with_params() {
    let source = r#"
class Builder {
    value: int

    fn add(mut self, x: int, y: int) Builder {
        self.value = self.value + x + y
        return self
    }

    fn get(self) int {
        return self.value
    }
}

fn main() int {
    let mut b = Builder { value: 10 }
    b = b.add(10, 10).add(5, 7)
    return b.get()
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}

#[test]
fn test_variadic_simulation_with_array() {
    let source = r#"
fn sum_all(values: [int]) int {
    let total = 0
    let i = 0
    while i < values.len() {
        total = total + values[i]
        i = i + 1
    }
    return total
}

fn main() int {
    return sum_all([1, 2, 3, 4, 5, 6, 7, 8, 9, -3])
}
"#;
    assert_eq!(compile_and_run_exit_code(source).unwrap(), 42);
}
