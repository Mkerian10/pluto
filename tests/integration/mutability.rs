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
