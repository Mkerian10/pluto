mod common;
use common::{compile_and_run_stdout, compile_should_fail};

#[test]
fn trait_basic_impl() {
    let out = compile_and_run_stdout(
        "trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n\n    fn bar(self) int {\n        return self.val\n    }\n}\n\nfn main() {\n    let x = X { val: 42 }\n    print(x.bar())\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn trait_as_function_param() {
    let out = compile_and_run_stdout(
        "trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n\n    fn bar(self) int {\n        return self.val\n    }\n}\n\nfn process(f: Foo) int {\n    return f.bar()\n}\n\nfn main() {\n    let x = X { val: 99 }\n    print(process(x))\n}",
    );
    assert_eq!(out, "99\n");
}

#[test]
fn trait_default_method() {
    let out = compile_and_run_stdout(
        "trait Greetable {\n    fn greet(self) int {\n        return 0\n    }\n}\n\nclass X impl Greetable {\n    val: int\n}\n\nfn main() {\n    let x = X { val: 5 }\n    print(x.greet())\n}",
    );
    assert_eq!(out, "0\n");
}

#[test]
fn trait_default_method_override() {
    let out = compile_and_run_stdout(
        "trait Greetable {\n    fn greet(self) int {\n        return 0\n    }\n}\n\nclass X impl Greetable {\n    val: int\n\n    fn greet(self) int {\n        return self.val\n    }\n}\n\nfn main() {\n    let x = X { val: 77 }\n    print(x.greet())\n}",
    );
    assert_eq!(out, "77\n");
}

#[test]
fn trait_multiple_classes() {
    let out = compile_and_run_stdout(
        "trait HasVal {\n    fn get(self) int\n}\n\nclass A impl HasVal {\n    x: int\n\n    fn get(self) int {\n        return self.x\n    }\n}\n\nclass B impl HasVal {\n    y: int\n\n    fn get(self) int {\n        return self.y * 2\n    }\n}\n\nfn show(v: HasVal) {\n    print(v.get())\n}\n\nfn main() {\n    let a = A { x: 10 }\n    let b = B { y: 20 }\n    show(a)\n    show(b)\n}",
    );
    assert_eq!(out, "10\n40\n");
}

#[test]
fn trait_multiple_traits() {
    let out = compile_and_run_stdout(
        "trait HasX {\n    fn get_x(self) int\n}\n\ntrait HasY {\n    fn get_y(self) int\n}\n\nclass Point impl HasX, HasY {\n    x: int\n    y: int\n\n    fn get_x(self) int {\n        return self.x\n    }\n\n    fn get_y(self) int {\n        return self.y\n    }\n}\n\nfn show_x(h: HasX) {\n    print(h.get_x())\n}\n\nfn show_y(h: HasY) {\n    print(h.get_y())\n}\n\nfn main() {\n    let p = Point { x: 3, y: 7 }\n    show_x(p)\n    show_y(p)\n}",
    );
    assert_eq!(out, "3\n7\n");
}

#[test]
fn trait_missing_method_rejected() {
    compile_should_fail(
        "trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n}\n\nfn main() {\n}",
    );
}

#[test]
fn trait_wrong_return_type_rejected() {
    compile_should_fail(
        "trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n\n    fn bar(self) bool {\n        return true\n    }\n}\n\nfn main() {\n}",
    );
}

#[test]
fn trait_unknown_trait_rejected() {
    compile_should_fail(
        "class X impl NonExistent {\n    val: int\n}\n\nfn main() {\n}",
    );
}

#[test]
fn trait_method_with_params() {
    let out = compile_and_run_stdout(
        "trait Adder {\n    fn add(self, x: int) int\n}\n\nclass MyAdder impl Adder {\n    base: int\n\n    fn add(self, x: int) int {\n        return self.base + x\n    }\n}\n\nfn do_add(a: Adder, val: int) int {\n    return a.add(val)\n}\n\nfn main() {\n    let a = MyAdder { base: 100 }\n    print(do_add(a, 23))\n}",
    );
    assert_eq!(out, "123\n");
}

// Trait handle tests (heap-allocated trait values)

#[test]
fn trait_typed_local() {
    let out = compile_and_run_stdout(
        "trait HasVal {\n    fn get(self) int\n}\n\nclass X impl HasVal {\n    val: int\n\n    fn get(self) int {\n        return self.val\n    }\n}\n\nfn main() {\n    let x: HasVal = X { val: 42 }\n    print(x.get())\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn trait_return_value() {
    let out = compile_and_run_stdout(
        "trait HasVal {\n    fn get(self) int\n}\n\nclass X impl HasVal {\n    val: int\n\n    fn get(self) int {\n        return self.val\n    }\n}\n\nfn make() HasVal {\n    return X { val: 77 }\n}\n\nfn main() {\n    print(make().get())\n}",
    );
    assert_eq!(out, "77\n");
}

#[test]
fn trait_forward() {
    let out = compile_and_run_stdout(
        "trait HasVal {\n    fn get(self) int\n}\n\nclass X impl HasVal {\n    val: int\n\n    fn get(self) int {\n        return self.val\n    }\n}\n\nfn show(v: HasVal) {\n    print(v.get())\n}\n\nfn forward(v: HasVal) {\n    show(v)\n}\n\nfn main() {\n    let x = X { val: 55 }\n    forward(x)\n}",
    );
    assert_eq!(out, "55\n");
}

#[test]
fn trait_method_on_call_result() {
    let out = compile_and_run_stdout(
        "trait HasVal {\n    fn get(self) int\n}\n\nclass X impl HasVal {\n    val: int\n\n    fn get(self) int {\n        return self.val\n    }\n}\n\nfn make_val() HasVal {\n    return X { val: 88 }\n}\n\nfn main() {\n    print(make_val().get())\n}",
    );
    assert_eq!(out, "88\n");
}

#[test]
fn trait_local_reassignment() {
    let out = compile_and_run_stdout(
        "trait HasVal {\n    fn get(self) int\n}\n\nclass A impl HasVal {\n    x: int\n\n    fn get(self) int {\n        return self.x\n    }\n}\n\nclass B impl HasVal {\n    y: int\n\n    fn get(self) int {\n        return self.y * 2\n    }\n}\n\nfn main() {\n    let v: HasVal = A { x: 10 }\n    print(v.get())\n    v = B { y: 20 }\n    print(v.get())\n}",
    );
    assert_eq!(out, "10\n40\n");
}

#[test]
fn trait_polymorphic_dispatch() {
    let out = compile_and_run_stdout(
        "trait Animal {\n    fn speak(self) int\n}\n\nclass Dog impl Animal {\n    volume: int\n\n    fn speak(self) int {\n        return self.volume\n    }\n}\n\nclass Cat impl Animal {\n    volume: int\n\n    fn speak(self) int {\n        return self.volume * 3\n    }\n}\n\nfn make_sound(a: Animal) {\n    print(a.speak())\n}\n\nfn main() {\n    let d = Dog { volume: 10 }\n    let c = Cat { volume: 5 }\n    make_sound(d)\n    make_sound(c)\n}",
    );
    assert_eq!(out, "10\n15\n");
}

#[test]
fn trait_default_method_via_handle() {
    let out = compile_and_run_stdout(
        "trait Greeter {\n    fn greet(self) int {\n        return 0\n    }\n}\n\nclass X impl Greeter {\n    val: int\n}\n\nfn show(g: Greeter) {\n    print(g.greet())\n}\n\nfn main() {\n    let x = X { val: 5 }\n    show(x)\n}",
    );
    assert_eq!(out, "0\n");
}
