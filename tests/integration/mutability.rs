mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

// ========== CALLEE-SIDE ENFORCEMENT ==========
// Reject field assignment in non-mut-self methods

#[test]
fn fail_field_assign_in_non_mut_method() {
    compile_should_fail_with(
        r#"
class Counter {
    count: int

    fn increment(self) {
        self.count = self.count + 1
    }
}

fn main() {
    let c = Counter { count: 0 }
    c.increment()
}
"#,
        "cannot assign to 'self.count' in a non-mut method",
    );
}

#[test]
fn fail_field_assign_in_conditional_non_mut_method() {
    compile_should_fail_with(
        r#"
class Counter {
    count: int

    fn increment_if_positive(self) {
        if self.count > 0 {
            self.count = self.count + 1
        }
    }
}

fn main() {
    let c = Counter { count: 1 }
}
"#,
        "cannot assign to 'self.count' in a non-mut method",
    );
}

#[test]
fn fail_field_assign_in_loop_non_mut_method() {
    compile_should_fail_with(
        r#"
class Counter {
    count: int

    fn increment_ten_times(self) {
        let mut i = 0
        while i < 10 {
            self.count = self.count + 1
            i = i + 1
        }
    }
}

fn main() {
    let c = Counter { count: 0 }
}
"#,
        "cannot assign to 'self.count' in a non-mut method",
    );
}

#[test]
fn field_assign_in_mut_self_method_allowed() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    count: int

    fn increment(mut self) {
        self.count = self.count + 1
    }

    fn get(self) int {
        return self.count
    }
}

fn main() {
    let mut c = Counter { count: 0 }
    c.increment()
    c.increment()
    print(c.get())
}
"#,
    );
    assert_eq!(out, "2\n");
}

// ========== CALLEE-SIDE: MUT METHOD CALLING MUT METHOD ==========

#[test]
fn fail_mut_method_call_in_non_mut_method() {
    compile_should_fail_with(
        r#"
class Counter {
    count: int

    fn increment(mut self) {
        self.count = self.count + 1
    }

    fn double_increment(self) {
        self.increment()
    }
}

fn main() {
    let c = Counter { count: 0 }
}
"#,
        "cannot call 'mut self' method 'increment' on self in a non-mut method",
    );
}

#[test]
fn mut_method_can_call_mut_method() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    count: int

    fn increment(mut self) {
        self.count = self.count + 1
    }

    fn double_increment(mut self) {
        self.increment()
        self.increment()
    }

    fn get(self) int {
        return self.count
    }
}

fn main() {
    let mut c = Counter { count: 0 }
    c.double_increment()
    print(c.get())
}
"#,
    );
    assert_eq!(out, "2\n");
}

// ========== CALLER-SIDE ENFORCEMENT ==========
// Reject mut-method calls on immutable bindings

#[test]
fn fail_mut_method_call_on_immutable_binding() {
    compile_should_fail_with(
        r#"
class Counter {
    count: int

    fn increment(mut self) {
        self.count = self.count + 1
    }
}

fn main() {
    let c = Counter { count: 0 }
    c.increment()
}
"#,
        "cannot call mutating method 'increment' on immutable variable 'c'",
    );
}

#[test]
fn mut_method_call_on_mutable_binding_allowed() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    count: int

    fn increment(mut self) {
        self.count = self.count + 1
    }

    fn get(self) int {
        return self.count
    }
}

fn main() {
    let mut c = Counter { count: 0 }
    c.increment()
    print(c.get())
}
"#,
    );
    assert_eq!(out, "1\n");
}

#[test]
fn fail_field_assign_on_immutable_binding() {
    compile_should_fail_with(
        r#"
class Point {
    x: int
    y: int
}

fn main() {
    let p = Point { x: 1, y: 2 }
    p.x = 10
}
"#,
        "cannot assign to field of immutable variable 'p'",
    );
}

#[test]
fn field_assign_on_mutable_binding_allowed() {
    let out = compile_and_run_stdout(
        r#"
class Point {
    x: int
    y: int
}

fn main() {
    let mut p = Point { x: 1, y: 2 }
    p.x = 10
    print(p.x)
    print(p.y)
}
"#,
    );
    assert_eq!(out, "10\n2\n");
}

// ========== MIXED SCENARIOS ==========

#[test]
fn immutable_method_on_immutable_binding_allowed() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    count: int

    fn get(self) int {
        return self.count
    }
}

fn main() {
    let c = Counter { count: 42 }
    print(c.get())
}
"#,
    );
    assert_eq!(out, "42\n");
}

#[test]
fn immutable_method_on_mutable_binding_allowed() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    count: int

    fn get(self) int {
        return self.count
    }
}

fn main() {
    let mut c = Counter { count: 42 }
    print(c.get())
}
"#,
    );
    assert_eq!(out, "42\n");
}

#[test]
fn mut_method_chaining_on_mutable_binding() {
    let out = compile_and_run_stdout(
        r#"
class Builder {
    val: int

    fn set(mut self, x: int) {
        self.val = x
    }

    fn add(mut self, x: int) {
        self.val = self.val + x
    }

    fn get(self) int {
        return self.val
    }
}

fn main() {
    let mut b = Builder { val: 0 }
    b.set(10)
    b.add(5)
    b.add(3)
    print(b.get())
}
"#,
    );
    assert_eq!(out, "18\n");
}

// ── mut parameters (#278) ────────────────────────────────────────────────────
// Parameters are immutable by default; `mut name: type` opts into
// reassignment, mirroring `let` vs `let mut`.

#[test]
fn mut_fn_param_reassignable() {
    let out = compile_and_run_stdout(
        r#"
fn clamp(mut n: int) int {
    if n > 10 {
        n = 10
    }
    return n
}

fn main() {
    print(clamp(15))
    print(clamp(3))
}
"#,
    );
    assert_eq!(out.trim(), "10\n3");
}

#[test]
fn mut_method_param_reassignable() {
    let out = compile_and_run_stdout(
        r#"
class Adder {
    base: int

    fn add(self, mut n: int) int {
        n = n + self.base
        return n
    }
}

fn main() {
    let a = Adder { base: 10 }
    print(a.add(5))
}
"#,
    );
    assert_eq!(out.trim(), "15");
}

#[test]
fn mut_closure_param_reassignable() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let f = (mut x: int) => {
        x = x * 2
        return x
    }
    print(f(21))
}
"#,
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn plain_param_still_immutable() {
    compile_should_fail_with(
        "fn f(x: int) {\n    x = 2\n}\n\nfn main(){}",
        "cannot assign to immutable variable",
    );
}

// ========== MUTATING BUILTINS REQUIRE A MUTABLE RECEIVER (#395) ==========
// push/pop/insert/remove/... mutate the collection in place, so they are
// rejected exactly where `xs[i] = v` is rejected.

#[test]
fn fail_array_push_on_immutable_local() {
    compile_should_fail_with(
        r#"
fn main() {
    let xs = [1, 2]
    xs.push(3)
}
"#,
        "cannot call mutating method 'push' on immutable variable 'xs'; declare with 'let mut' to allow mutation",
    );
}

#[test]
fn fail_array_pop_on_immutable_local() {
    compile_should_fail_with(
        r#"
fn main() {
    let xs = [1, 2]
    xs.pop()
}
"#,
        "cannot call mutating method 'pop' on immutable variable 'xs'",
    );
}

#[test]
fn fail_array_clear_on_immutable_local() {
    compile_should_fail_with(
        r#"
fn main() {
    let xs = [1, 2]
    xs.clear()
}
"#,
        "cannot call mutating method 'clear' on immutable variable 'xs'",
    );
}

#[test]
fn fail_array_insert_at_on_immutable_local() {
    compile_should_fail_with(
        r#"
fn main() {
    let xs = [1, 2]
    xs.insert_at(0, 9)
}
"#,
        "cannot call mutating method 'insert_at' on immutable variable 'xs'",
    );
}

#[test]
fn fail_array_remove_at_on_immutable_local() {
    compile_should_fail_with(
        r#"
fn main() {
    let xs = [1, 2]
    xs.remove_at(0)
}
"#,
        "cannot call mutating method 'remove_at' on immutable variable 'xs'",
    );
}

#[test]
fn fail_array_reverse_on_immutable_local() {
    compile_should_fail_with(
        r#"
fn main() {
    let xs = [1, 2]
    xs.reverse()
}
"#,
        "cannot call mutating method 'reverse' on immutable variable 'xs'",
    );
}

#[test]
fn fail_map_insert_on_immutable_local() {
    compile_should_fail_with(
        r#"
fn main() {
    let m = Map<string, int> { "a": 1 }
    m.insert("b", 2)
}
"#,
        "cannot call mutating method 'insert' on immutable variable 'm'",
    );
}

#[test]
fn fail_map_remove_on_immutable_local() {
    compile_should_fail_with(
        r#"
fn main() {
    let m = Map<string, int> { "a": 1 }
    m.remove("a")
}
"#,
        "cannot call mutating method 'remove' on immutable variable 'm'",
    );
}

#[test]
fn fail_set_insert_on_immutable_local() {
    compile_should_fail_with(
        r#"
fn main() {
    let s = Set<int> { 1 }
    s.insert(2)
}
"#,
        "cannot call mutating method 'insert' on immutable variable 's'",
    );
}

#[test]
fn fail_set_remove_on_immutable_local() {
    compile_should_fail_with(
        r#"
fn main() {
    let s = Set<int> { 1 }
    s.remove(1)
}
"#,
        "cannot call mutating method 'remove' on immutable variable 's'",
    );
}

#[test]
fn fail_bytes_push_on_immutable_local() {
    compile_should_fail_with(
        r#"
fn main() {
    let buf = bytes_new()
    buf.push((7).to_byte())
}
"#,
        "cannot call mutating method 'push' on immutable variable 'buf'",
    );
}

#[test]
fn fail_bytes_extend_on_immutable_local() {
    compile_should_fail_with(
        r#"
fn main() {
    let buf = bytes_new()
    let mut other = bytes_new()
    other.push((1).to_byte())
    buf.extend(other)
}
"#,
        "cannot call mutating method 'extend' on immutable variable 'buf'",
    );
}

#[test]
fn fail_bytes_fill_on_immutable_local() {
    compile_should_fail_with(
        r#"
fn main() {
    let buf = bytes_new()
    buf.fill((0).to_byte())
}
"#,
        "cannot call mutating method 'fill' on immutable variable 'buf'",
    );
}

#[test]
fn fail_bytes_write_u8_on_non_mut_param() {
    compile_should_fail_with(
        r#"
fn stamp(buf: bytes) {
    buf.write_u8(0, 7)
}

fn main() {
    let mut buf = bytes_new()
    buf.push((0).to_byte())
    stamp(buf)
}
"#,
        "cannot call mutating method 'write_u8' on immutable variable 'buf'",
    );
}

#[test]
fn fail_bytes_copy_from_on_immutable_local() {
    compile_should_fail_with(
        r#"
fn main() {
    let mut src = bytes_new()
    src.push((1).to_byte())
    let dst = bytes_new()
    dst.copy_from(src, 0, 0, 1)
}
"#,
        "cannot call mutating method 'copy_from' on immutable variable 'dst'",
    );
}

#[test]
fn bytes_read_methods_on_immutable_binding_allowed() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let mut src = bytes_filled(8, (0).to_byte())
    src.write_i64_le(0, 7)
    let buf = src
    let part = buf.slice(0, 4)
    print(f"{buf.read_u8(0)} {buf.read_i64_le(0)} {buf.find((7).to_byte(), 0)} {buf.compare(src)} {part.len()}")
}
"#,
    );
    assert_eq!(out.trim(), "7 7 0 0 4");
}

#[test]
fn fail_array_push_on_non_mut_param() {
    compile_should_fail_with(
        r#"
fn grow(xs: [int]) {
    xs.push(3)
}

fn main() {
    let mut xs = [1]
    grow(xs)
}
"#,
        "cannot call mutating method 'push' on immutable variable 'xs'; declare with 'let mut' to allow mutation",
    );
}

#[test]
fn fail_bytes_push_on_non_mut_param() {
    compile_should_fail_with(
        r#"
fn stamp(buf: bytes) {
    buf.push((7).to_byte())
}

fn main() {
    let mut buf = bytes_new()
    stamp(buf)
}
"#,
        "cannot call mutating method 'push' on immutable variable 'buf'",
    );
}

#[test]
fn fail_array_push_on_field_of_immutable_local() {
    compile_should_fail_with(
        r#"
class Holder {
    xs: [int]
}

fn main() {
    let h = Holder { xs: [] }
    h.xs.push(1)
}
"#,
        "cannot call mutating method 'push' on immutable variable 'h'",
    );
}

#[test]
fn fail_array_push_on_self_field_in_non_mut_method() {
    compile_should_fail_with(
        r#"
class Holder {
    xs: [int]

    fn add_one(self) {
        self.xs.push(1)
    }
}

fn main() {
    let mut h = Holder { xs: [] }
    h.add_one()
}
"#,
        "cannot call mutating method 'push' on self's data in a non-mut method; declare 'mut self'",
    );
}

#[test]
fn fail_map_insert_on_self_field_in_non_mut_method() {
    compile_should_fail_with(
        r#"
class Holder {
    m: Map<string, int>

    fn put(self) {
        self.m.insert("a", 1)
    }
}

fn main() {
    let mut h = Holder { m: Map<string, int> {} }
    h.put()
}
"#,
        "cannot call mutating method 'insert' on self's data in a non-mut method; declare 'mut self'",
    );
}

#[test]
fn array_push_on_mut_local_allowed() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let mut xs = [1, 2]
    xs.push(3)
    xs.insert_at(0, 0)
    xs.remove_at(0)
    xs.reverse()
    xs.pop()
    print(f"{xs.len()}")
    xs.clear()
    print(f"{xs.len()}")
}
"#,
    );
    assert_eq!(out.trim(), "2\n0");
}

#[test]
fn map_set_bytes_mutators_on_mut_local_allowed() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let mut m = Map<string, int> { "a": 1 }
    m.insert("b", 2)
    m.remove("a")
    let mut s = Set<int> { 1 }
    s.insert(2)
    s.remove(1)
    let mut buf = bytes_new()
    buf.push((7).to_byte())
    print(f"{m.len()} {s.len()} {buf.len()}")
}
"#,
    );
    assert_eq!(out.trim(), "1 1 1");
}

#[test]
fn array_push_on_mut_param_allowed() {
    let out = compile_and_run_stdout(
        r#"
fn grow(mut xs: [int]) {
    xs.push(3)
}

fn main() {
    let mut xs = [1]
    grow(xs)
    print(f"{xs.len()}")
}
"#,
    );
    // Arrays are heap references; the callee's push is visible here.
    assert_eq!(out.trim(), "2");
}

#[test]
fn bytes_push_on_mut_param_allowed() {
    let out = compile_and_run_stdout(
        r#"
fn stamp(mut buf: bytes) {
    buf.push((7).to_byte())
}

fn main() {
    let mut buf = bytes_new()
    stamp(buf)
    print(f"{buf.len()}")
}
"#,
    );
    assert_eq!(out.trim(), "1");
}

#[test]
fn array_push_on_self_field_in_mut_method_allowed() {
    let out = compile_and_run_stdout(
        r#"
class Holder {
    xs: [int]

    fn add_one(mut self) {
        self.xs.push(1)
    }
}

fn main() {
    let mut h = Holder { xs: [] }
    h.add_one()
    print(f"{h.xs.len()}")
}
"#,
    );
    assert_eq!(out.trim(), "1");
}

#[test]
fn non_mutating_builtins_on_immutable_binding_allowed() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let xs = [3, 1, 2]
    let m = Map<string, int> { "a": 1 }
    let s = Set<int> { 1 }
    let part = xs.slice(0, 2)
    print(f"{xs.len()} {xs.contains(1)} {xs.first()} {part.len()} {m.contains("a")} {s.contains(1)}")
}
"#,
    );
    assert_eq!(out.trim(), "3 true 3 2 true true");
}
