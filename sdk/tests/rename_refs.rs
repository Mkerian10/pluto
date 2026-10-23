use pluto_sdk::Module;

fn rename(src: &str, old: &str, new: &str) -> String {
    let module = Module::from_source(src).expect("parse");
    let id = module.find(old).iter().find(|d| d.name() == old).expect("decl").id();
    let mut editor = module.edit();
    editor.rename(id, new).expect("rename");
    editor.commit().source().to_string()
}

#[test]
fn rename_function_updates_value_references_but_not_shadowing_locals() {
    let src = r#"
fn double(x: int) int {
    return x * 2
}

fn apply(f: fn(int) int, v: int) int {
    return f(v)
}

fn shadow(double: int) int {
    return double + 1
}

class C {
    n: int

    fn run(self) int {
        let g = double
        return apply(g, self.n)
    }
}

fn main() {
    let g = double
    print(apply(double, 3))
    let h = (y: int) => apply(double, y)
    let fs = [double]
    for double in [1, 2] {
        print(double)
    }
    let double = 5
    print(double)
}
"#;
    let out = rename(src, "double", "twice");
    assert!(out.contains("fn twice(x: int) int"), "{out}");
    assert!(out.contains("let g = twice"), "{out}");
    assert!(out.contains("print(apply(twice, 3))"), "{out}");
    assert!(out.contains("apply(twice, y)"), "{out}");
    assert!(out.contains("[twice]"), "{out}");
    // Locals named `double` are untouched, and so are their uses.
    assert!(out.contains("fn shadow(double: int) int {\n    return double + 1"), "{out}");
    assert!(out.contains("for double in [1, 2] {\n        print(double)"), "{out}");
    assert!(out.contains("let double = 5\n    print(double)"), "{out}");
    assert!(!out.contains("print(twice)"), "{out}");
}

#[test]
fn rename_error_updates_typed_catch() {
    let src = r#"
error Oops {
    code: int
}

fn risky() int {
    raise Oops { code: 1 }
}

fn main() {
    let v = risky() catch err: Oops {
        0
    }
    print(v)
}
"#;
    let out = rename(src, "Oops", "Failure");
    assert!(out.contains("raise Failure { code: 1 }"), "{out}");
    assert!(out.contains("catch err: Failure"), "{out}");
    assert!(!out.contains("Oops"), "{out}");
}
