mod common;
use common::compile_and_run_stdout;

#[test]
fn class_construct_and_field_access() {
    let out = compile_and_run_stdout(
        "class Point {\n    x: int\n    y: int\n}\n\nfn main() {\n    let p = Point { x: 3, y: 4 }\n    print(p.x)\n    print(p.y)\n}",
    );
    assert_eq!(out, "3\n4\n");
}

#[test]
fn class_method_call() {
    let out = compile_and_run_stdout(
        "class Point {\n    x: int\n    y: int\n\n    fn sum(self) int {\n        return self.x + self.y\n    }\n}\n\nfn main() {\n    let p = Point { x: 3, y: 4 }\n    print(p.sum())\n}",
    );
    assert_eq!(out, "7\n");
}

#[test]
fn class_field_mutation() {
    let out = compile_and_run_stdout(
        "class Counter {\n    val: int\n\n    fn get(self) int {\n        return self.val\n    }\n}\n\nfn main() {\n    let c = Counter { val: 0 }\n    c.val = 42\n    print(c.get())\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn class_as_function_param() {
    let out = compile_and_run_stdout(
        "class Point {\n    x: int\n    y: int\n}\n\nfn get_x(p: Point) int {\n    return p.x\n}\n\nfn main() {\n    let p = Point { x: 99, y: 0 }\n    print(get_x(p))\n}",
    );
    assert_eq!(out, "99\n");
}

#[test]
fn class_method_with_params() {
    let out = compile_and_run_stdout(
        "class Adder {\n    base: int\n\n    fn add(self, n: int) int {\n        return self.base + n\n    }\n}\n\nfn main() {\n    let a = Adder { base: 10 }\n    print(a.add(5))\n}",
    );
    assert_eq!(out, "15\n");
}

#[test]
fn function_returning_class() {
    let out = compile_and_run_stdout(
        "class Point {\n    x: int\n    y: int\n}\n\nfn make_point(x: int, y: int) Point {\n    return Point { x: x, y: y }\n}\n\nfn main() {\n    let p = make_point(10, 20)\n    print(p.x)\n    print(p.y)\n}",
    );
    assert_eq!(out, "10\n20\n");
}

#[test]
fn class_multiple_methods() {
    let out = compile_and_run_stdout(
        "class Rect {\n    w: int\n    h: int\n\n    fn area(self) int {\n        return self.w * self.h\n    }\n\n    fn perimeter(self) int {\n        return 2 * (self.w + self.h)\n    }\n}\n\nfn main() {\n    let r = Rect { w: 3, h: 4 }\n    print(r.area())\n    print(r.perimeter())\n}",
    );
    assert_eq!(out, "12\n14\n");
}
