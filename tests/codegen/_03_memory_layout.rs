// Category 3: Memory Layout & Alignment Tests (40+ tests)
// Validates struct field layout, alignment requirements, and field access patterns

use super::common::{compile_and_run, compile_and_run_stdout};

// ============================================================================
// Struct Field Layout - Single Field (4 tests)
// ============================================================================

#[test]
fn test_single_field_int() {
    let src = r#"
        class Single {
            value: int
        }

        fn main() int {
            let s = Single { value: 42 }
            return s.value
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn test_single_field_float() {
    let src = r#"
        class Single {
            value: float
        }

        fn main() {
            let s = Single { value: 3.14 }
            print(s.value)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "3.140000");
}

#[test]
fn test_single_field_string() {
    let src = r#"
        class Single {
            value: string
        }

        fn main() {
            let s = Single { value: "hello" }
            print(s.value)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "hello");
}

#[test]
fn test_single_field_class() {
    let src = r#"
        class Inner {
            x: int
        }

        class Outer {
            inner: Inner
        }

        fn main() int {
            let i = Inner { x: 99 }
            let o = Outer { inner: i }
            let inner_obj = o.inner
            return inner_obj.x
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

// ============================================================================
// Struct Field Layout - Two Fields (3 tests)
// ============================================================================

#[test]
fn test_two_fields_int_int() {
    let src = r#"
        class TwoInts {
            first: int
            second: int
        }

        fn main() {
            let t = TwoInts { first: 10, second: 20 }
            print(t.first)
            print(t.second)
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("10"));
    assert!(output.contains("20"));
}

#[test]
fn test_two_fields_int_float() {
    let src = r#"
        class IntFloat {
            a: int
            b: float
        }

        fn main() {
            let x = IntFloat { a: 5, b: 2.5 }
            print(x.a)
            print(x.b)
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("5"));
    assert!(output.contains("2.5"));
}

#[test]
fn test_two_fields_float_float() {
    let src = r#"
        class TwoFloats {
            x: float
            y: float
        }

        fn main() {
            let t = TwoFloats { x: 1.1, y: 2.2 }
            print(t.x + t.y)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "3.300000");
}

// ============================================================================
// Struct Field Layout - Three Fields (3 tests)
// ============================================================================

#[test]
fn test_three_fields_int_int_int() {
    let src = r#"
        class ThreeInts {
            a: int
            b: int
            c: int
        }

        fn main() int {
            let t = ThreeInts { a: 1, b: 2, c: 3 }
            return t.a + t.b + t.c
        }
    "#;
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn test_three_fields_int_float_string() {
    let src = r#"
        class Mixed {
            id: int
            score: float
            name: string
        }

        fn main() {
            let m = Mixed { id: 42, score: 95.5, name: "Alice" }
            print(m.id)
            print(m.score)
            print(m.name)
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("42"));
    assert!(output.contains("95.5"));
    assert!(output.contains("Alice"));
}

#[test]
fn test_three_fields_different_order_access() {
    let src = r#"
        class ABC {
            a: int
            b: int
            c: int
        }

        fn main() {
            let x = ABC { a: 10, b: 20, c: 30 }
            // Access in different order than declaration
            print(x.c)
            print(x.a)
            print(x.b)
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "30");
    assert_eq!(lines[1], "10");
    assert_eq!(lines[2], "20");
}

// ============================================================================
// Struct Field Layout - Mixed Size Fields with Padding (3 tests)
// ============================================================================

#[test]
fn test_byte_int_byte_padding() {
    // Tests that padding is handled correctly between fields of different sizes
    let src = r#"
        class ByteIntByte {
            first: byte
            middle: int
            last: byte
        }

        fn main() {
            let x = ByteIntByte { first: 1 as byte, middle: 100, last: 2 as byte }
            print(x.first as int)
            print(x.middle)
            print(x.last as int)
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("1"));
    assert!(output.contains("100"));
    assert!(output.contains("2"));
}

#[test]
fn test_byte_float_byte_padding() {
    let src = r#"
        class ByteFloatByte {
            a: byte
            b: float
            c: byte
        }

        fn main() {
            let x = ByteFloatByte { a: 5 as byte, b: 3.14, c: 7 as byte }
            print(x.a as int)
            print(x.b)
            print(x.c as int)
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("5"));
    assert!(output.contains("3.14"));
    assert!(output.contains("7"));
}

#[test]
fn test_bool_int_bool_padding() {
    let src = r#"
        class BoolIntBool {
            flag1: bool
            value: int
            flag2: bool
        }

        fn main() {
            let x = BoolIntBool { flag1: true, value: 42, flag2: false }
            if x.flag1 {
                print("flag1 is true")
            }
            print(x.value)
            if x.flag2 {
                print("flag2 is true")
            }
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("flag1 is true"));
    assert!(output.contains("42"));
    assert!(!output.contains("flag2 is true"));
}

// ============================================================================
// Struct Field Layout - Large Structs (3 tests)
// ============================================================================

#[test]
fn test_large_struct_20_fields() {
    let src = r#"
        class Large20 {
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
        }

        fn main() int {
            let x = Large20 {
                f1: 1, f2: 2, f3: 3, f4: 4, f5: 5,
                f6: 6, f7: 7, f8: 8, f9: 9, f10: 10,
                f11: 11, f12: 12, f13: 13, f14: 14, f15: 15,
                f16: 16, f17: 17, f18: 18, f19: 19, f20: 20
            }
            return x.f1 + x.f10 + x.f20
        }
    "#;
    assert_eq!(compile_and_run(src), 31); // 1 + 10 + 20
}

#[test]
fn test_large_struct_50_fields() {
    // Generate a struct with 50 int fields
    let mut fields = String::new();
    let mut init = String::new();
    let mut access = String::new();

    for i in 1..=50 {
        fields.push_str(&format!("            f{}: int\n", i));
        if i > 1 { init.push_str(", "); }
        init.push_str(&format!("f{}: {}", i, i));
    }

    // Access fields 1, 25, 50
    access = "x.f1 + x.f25 + x.f50".to_string();

    let src = format!(r#"
        class Large50 {{
{}
        }}

        fn main() int {{
            let x = Large50 {{ {} }}
            return {}
        }}
    "#, fields, init, access);

    assert_eq!(compile_and_run(&src), 76); // 1 + 25 + 50
}

#[test]
fn test_large_struct_mixed_types_30_fields() {
    // 30 fields of mixed types
    let src = r#"
        class MixedLarge {
            f1: int
            f2: float
            f3: bool
            f4: string
            f5: int
            f6: float
            f7: bool
            f8: string
            f9: int
            f10: float
            f11: bool
            f12: string
            f13: int
            f14: float
            f15: bool
            f16: string
            f17: int
            f18: float
            f19: bool
            f20: string
            f21: int
            f22: float
            f23: bool
            f24: string
            f25: int
            f26: float
            f27: bool
            f28: string
            f29: int
            f30: float
        }

        fn main() {
            let x = MixedLarge {
                f1: 1, f2: 2.0, f3: true, f4: "a",
                f5: 5, f6: 6.0, f7: false, f8: "b",
                f9: 9, f10: 10.0, f11: true, f12: "c",
                f13: 13, f14: 14.0, f15: false, f16: "d",
                f17: 17, f18: 18.0, f19: true, f20: "e",
                f21: 21, f22: 22.0, f23: false, f24: "f",
                f25: 25, f26: 26.0, f27: true, f28: "g",
                f29: 29, f30: 30.0
            }
            print(x.f1 + x.f15 as int + x.f29)  // 1 + 0 + 29 = 30
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "30");
}

// ============================================================================
// Struct Field Layout - Nested Structs (4 tests)
// ============================================================================

#[test]
fn test_nested_struct_one_level() {
    let src = r#"
        class Inner {
            value: int
        }

        class Outer {
            x: int
            inner: Inner
            y: int
        }

        fn main() int {
            let i = Inner { value: 100 }
            let o = Outer { x: 1, inner: i, y: 2 }
            let inner_obj = o.inner
            return o.x + inner_obj.value + o.y
        }
    "#;
    assert_eq!(compile_and_run(src), 103); // 1 + 100 + 2
}

#[test]
fn test_nested_struct_two_levels() {
    let src = r#"
        class Level3 {
            value: int
        }

        class Level2 {
            inner: Level3
        }

        class Level1 {
            inner: Level2
        }

        fn main() int {
            let l3 = Level3 { value: 42 }
            let l2 = Level2 { inner: l3 }
            let l1 = Level1 { inner: l2 }
            let level2 = l1.inner
            let level3 = level2.inner
            return level3.value
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn test_nested_struct_mixed_fields() {
    let src = r#"
        class Address {
            street: string
            number: int
        }

        class Person {
            name: string
            age: int
            address: Address
        }

        fn main() {
            let addr = Address { street: "Main St", number: 123 }
            let person = Person { name: "Bob", age: 30, address: addr }
            let addr_obj = person.address
            print(person.name)
            print(person.age)
            print(addr_obj.street)
            print(addr_obj.number)
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("Bob"));
    assert!(output.contains("30"));
    assert!(output.contains("Main St"));
    assert!(output.contains("123"));
}

#[test]
fn test_nested_struct_multiple_inner() {
    let src = r#"
        class Point {
            x: int
            y: int
        }

        class Rectangle {
            top_left: Point
            bottom_right: Point
        }

        fn main() int {
            let p1 = Point { x: 0, y: 0 }
            let p2 = Point { x: 10, y: 20 }
            let rect = Rectangle { top_left: p1, bottom_right: p2 }
            let br = rect.bottom_right
            return br.x + br.y
        }
    "#;
    assert_eq!(compile_and_run(src), 30); // 10 + 20
}

// ============================================================================
// Field Access Order Validation (3 tests)
// ============================================================================

#[test]
fn test_field_access_order_forward() {
    let src = r#"
        class Ordered {
            a: int
            b: int
            c: int
            d: int
            e: int
        }

        fn main() {
            let x = Ordered { a: 1, b: 2, c: 3, d: 4, e: 5 }
            print(x.a)
            print(x.b)
            print(x.c)
            print(x.d)
            print(x.e)
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "1");
    assert_eq!(lines[1], "2");
    assert_eq!(lines[2], "3");
    assert_eq!(lines[3], "4");
    assert_eq!(lines[4], "5");
}

#[test]
fn test_field_access_order_reverse() {
    let src = r#"
        class Ordered {
            a: int
            b: int
            c: int
            d: int
            e: int
        }

        fn main() {
            let x = Ordered { a: 1, b: 2, c: 3, d: 4, e: 5 }
            print(x.e)
            print(x.d)
            print(x.c)
            print(x.b)
            print(x.a)
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "5");
    assert_eq!(lines[1], "4");
    assert_eq!(lines[2], "3");
    assert_eq!(lines[3], "2");
    assert_eq!(lines[4], "1");
}

#[test]
fn test_field_access_order_random() {
    let src = r#"
        class Ordered {
            f1: int
            f2: int
            f3: int
            f4: int
            f5: int
            f6: int
            f7: int
        }

        fn main() {
            let x = Ordered { f1: 10, f2: 20, f3: 30, f4: 40, f5: 50, f6: 60, f7: 70 }
            print(x.f5)
            print(x.f2)
            print(x.f7)
            print(x.f1)
            print(x.f4)
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "50");
    assert_eq!(lines[1], "20");
    assert_eq!(lines[2], "70");
    assert_eq!(lines[3], "10");
    assert_eq!(lines[4], "40");
}

// ============================================================================
// Alignment Requirements (10 tests)
// ============================================================================

#[test]
fn test_byte_alignment() {
    let src = r#"
        class ByteStruct {
            a: byte
            b: byte
            c: byte
        }

        fn main() {
            let x = ByteStruct { a: 1 as byte, b: 2 as byte, c: 3 as byte }
            print(x.a as int)
            print(x.b as int)
            print(x.c as int)
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("1"));
    assert!(output.contains("2"));
    assert!(output.contains("3"));
}

#[test]
fn test_int_alignment() {
    let src = r#"
        class IntStruct {
            a: int
            b: int
        }

        fn main() int {
            let x = IntStruct { a: 100, b: 200 }
            return x.a + x.b
        }
    "#;
    assert_eq!(compile_and_run(src), 44); // 300 % 256 = 44 (exit code wrapping)
}

#[test]
fn test_float_alignment() {
    let src = r#"
        class FloatStruct {
            a: float
            b: float
        }

        fn main() {
            let x = FloatStruct { a: 1.5, b: 2.5 }
            print(x.a + x.b)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "4.000000");
}

#[test]
fn test_pointer_alignment_string() {
    let src = r#"
        class StringStruct {
            a: string
            b: string
        }

        fn main() {
            let x = StringStruct { a: "hello", b: "world" }
            print(x.a)
            print(x.b)
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("hello"));
    assert!(output.contains("world"));
}

#[test]
fn test_pointer_alignment_class() {
    let src = r#"
        class Inner {
            value: int
        }

        class PointerStruct {
            a: Inner
            b: Inner
        }

        fn main() int {
            let i1 = Inner { value: 10 }
            let i2 = Inner { value: 20 }
            let x = PointerStruct { a: i1, b: i2 }
            let a_obj = x.a
            let b_obj = x.b
            return a_obj.value + b_obj.value
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn test_struct_alignment_max_field() {
    // Struct alignment should be max of field alignments
    let src = r#"
        class MixedAlign {
            byte_field: byte
            int_field: int
            float_field: float
        }

        fn main() {
            let x = MixedAlign { byte_field: 1 as byte, int_field: 42, float_field: 3.14 }
            print(x.byte_field as int)
            print(x.int_field)
            print(x.float_field)
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("1"));
    assert!(output.contains("42"));
    assert!(output.contains("3.14"));
}

#[test]
fn test_array_element_alignment_int() {
    let src = r#"
        fn main() {
            let arr = [10, 20, 30, 40, 50]
            print(arr[0])
            print(arr[2])
            print(arr[4])
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("10"));
    assert!(output.contains("30"));
    assert!(output.contains("50"));
}

#[test]
fn test_array_element_alignment_float() {
    let src = r#"
        fn main() {
            let arr = [1.1, 2.2, 3.3, 4.4, 5.5]
            print(arr[1])
            print(arr[3])
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("2.2"));
    assert!(output.contains("4.4"));
}

#[test]
fn test_array_element_alignment_class() {
    let src = r#"
        class Point {
            x: int
            y: int
        }

        fn main() {
            let arr = [
                Point { x: 1, y: 2 },
                Point { x: 3, y: 4 },
                Point { x: 5, y: 6 }
            ]
            print(arr[0].x)
            print(arr[1].y)
            print(arr[2].x)
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("1"));
    assert!(output.contains("4"));
    assert!(output.contains("5"));
}

#[test]
fn test_alignment_bool_fields() {
    let src = r#"
        class BoolStruct {
            a: bool
            b: bool
            c: bool
        }

        fn main() {
            let x = BoolStruct { a: true, b: false, c: true }
            let count = 0
            if x.a { count = count + 1 }
            if x.b { count = count + 1 }
            if x.c { count = count + 1 }
            print(count)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "2");
}

// ============================================================================
// Field Access - Read/Write (10 tests)
// ============================================================================

#[test]
fn test_field_access_read_first() {
    let src = r#"
        class Data {
            first: int
            second: int
            third: int
        }

        fn main() int {
            let d = Data { first: 111, second: 222, third: 333 }
            return d.first
        }
    "#;
    assert_eq!(compile_and_run(src), 111);
}

#[test]
fn test_field_access_read_middle() {
    let src = r#"
        class Data {
            first: int
            second: int
            third: int
        }

        fn main() int {
            let d = Data { first: 111, second: 222, third: 333 }
            return d.second
        }
    "#;
    assert_eq!(compile_and_run(src), 222);
}

#[test]
fn test_field_access_read_last() {
    let src = r#"
        class Data {
            first: int
            second: int
            third: int
        }

        fn main() int {
            let d = Data { first: 111, second: 222, third: 333 }
            return d.third
        }
    "#;
    assert_eq!(compile_and_run(src), 77); // 333 % 256 = 77
}

#[test]
fn test_field_access_write_first() {
    let src = r#"
        class Data {
            first: int
            second: int
        }

        fn main() int {
            let mut d = Data { first: 10, second: 20 }
            d.first = 100
            return d.first
        }
    "#;
    assert_eq!(compile_and_run(src), 100);
}

#[test]
fn test_field_access_write_middle() {
    let src = r#"
        class Data {
            a: int
            b: int
            c: int
        }

        fn main() int {
            let mut d = Data { a: 1, b: 2, c: 3 }
            d.b = 200
            return d.b
        }
    "#;
    assert_eq!(compile_and_run(src), 200);
}

#[test]
fn test_field_access_write_last() {
    let src = r#"
        class Data {
            a: int
            b: int
            c: int
        }

        fn main() int {
            let mut d = Data { a: 1, b: 2, c: 3 }
            d.c = 42
            return d.c
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn test_field_access_read_50th_field() {
    // Generate a class with 50 fields and read the 50th
    let mut fields = String::new();
    let mut init = String::new();

    for i in 1..=50 {
        fields.push_str(&format!("            f{}: int\n", i));
        if i > 1 { init.push_str(", "); }
        init.push_str(&format!("f{}: {}", i, i * 10));
    }

    let src = format!(r#"
        class Large {{
{}
        }}

        fn main() int {{
            let x = Large {{ {} }}
            return x.f50
        }}
    "#, fields, init);

    assert_eq!(compile_and_run(&src), 244); // 500 % 256 = 244
}

#[test]
fn test_field_access_sequential_pattern() {
    let src = r#"
        class Sequential {
            f1: int
            f2: int
            f3: int
            f4: int
            f5: int
        }

        fn main() int {
            let s = Sequential { f1: 1, f2: 2, f3: 3, f4: 4, f5: 5 }
            let sum = 0
            sum = sum + s.f1
            sum = sum + s.f2
            sum = sum + s.f3
            sum = sum + s.f4
            sum = sum + s.f5
            return sum
        }
    "#;
    assert_eq!(compile_and_run(src), 15); // 1+2+3+4+5
}

#[test]
fn test_field_access_random_pattern() {
    let src = r#"
        class Random {
            f1: int
            f2: int
            f3: int
            f4: int
            f5: int
            f6: int
            f7: int
        }

        fn main() int {
            let r = Random { f1: 10, f2: 20, f3: 30, f4: 40, f5: 50, f6: 60, f7: 70 }
            let sum = 0
            sum = sum + r.f7
            sum = sum + r.f2
            sum = sum + r.f5
            sum = sum + r.f1
            sum = sum + r.f4
            return sum
        }
    "#;
    assert_eq!(compile_and_run(src), 190); // 70+20+50+10+40
}

#[test]
fn test_field_access_multiple_modifications() {
    let src = r#"
        class Counter {
            value: int
        }

        fn main() int {
            let mut c = Counter { value: 0 }
            c.value = c.value + 10
            c.value = c.value + 20
            c.value = c.value + 30
            return c.value
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}
