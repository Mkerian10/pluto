mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

#[test]
fn scoped_class_parses() {
    let output = compile_and_run_stdout(
        "scoped class Foo {\n    x: int\n}\n\nfn main() {\n    let f = Foo { x: 1 }\n    print(f.x)\n}",
    );
    assert_eq!(output.trim(), "1");
}

#[test]
fn transient_class_parses() {
    let output = compile_and_run_stdout(
        "transient class Bar {\n    x: int\n}\n\nfn main() {\n    let b = Bar { x: 42 }\n    print(b.x)\n}",
    );
    assert_eq!(output.trim(), "42");
}

#[test]
fn pub_scoped_class_parses() {
    let output = compile_and_run_stdout(
        "pub scoped class Svc {\n    x: int\n}\n\nfn main() {\n    let s = Svc { x: 7 }\n    print(s.x)\n}",
    );
    assert_eq!(output.trim(), "7");
}

#[test]
fn lifecycle_inference_basic() {
    // Scoped class Ctx + class Svc depends on Ctx via DI.
    // Svc should be inferred as scoped (no error thrown).
    let output = compile_and_run_stdout(
        "scoped class Ctx {\n    fn name(self) string {\n        return \"ctx\"\n    }\n}\n\nclass Svc[ctx: Ctx] {\n    fn run(self) string {\n        return self.ctx.name()\n    }\n}\n\napp MyApp[svc: Svc] {\n    fn main(self) {\n        print(self.svc.run())\n    }\n}",
    );
    assert_eq!(output.trim(), "ctx");
}

#[test]
fn lifecycle_inference_transitive() {
    // A depends on B depends on scoped C -> all compile.
    let output = compile_and_run_stdout(
        "scoped class C {\n    fn val(self) string {\n        return \"deep\"\n    }\n}\n\nclass B[c: C] {\n    fn val(self) string {\n        return self.c.val()\n    }\n}\n\nclass A[b: B] {\n    fn val(self) string {\n        return self.b.val()\n    }\n}\n\napp MyApp[a: A] {\n    fn main(self) {\n        print(self.a.val())\n    }\n}",
    );
    assert_eq!(output.trim(), "deep");
}

#[test]
fn scoped_on_fn_rejected() {
    compile_should_fail_with(
        "scoped fn foo() {}\n\nfn main() {}",
        "lifecycle modifiers (scoped, transient) can only be used on classes",
    );
}

#[test]
fn transient_on_trait_rejected() {
    compile_should_fail_with(
        "transient trait Foo {\n    fn bar(self)\n}\n\nfn main() {}",
        "lifecycle modifiers (scoped, transient) can only be used on classes",
    );
}

#[test]
fn scoped_on_enum_rejected() {
    compile_should_fail_with(
        "scoped enum Color {\n    Red\n    Blue\n}\n\nfn main() {}",
        "lifecycle modifiers (scoped, transient) can only be used on classes",
    );
}

#[test]
fn scoped_generic_class_rejected() {
    compile_should_fail_with(
        "scoped class Box<T> {\n    value: T\n}\n\nfn main() {}",
        "generic classes cannot have lifecycle annotations",
    );
}

#[test]
fn existing_di_still_works() {
    // Basic DI chain from existing tests — backward compat.
    let output = compile_and_run_stdout(
        "class Database {\n    fn query(self, q: string) string {\n        return q\n    }\n}\n\nclass UserService[db: Database] {\n    fn get_user(self, id: string) string {\n        return self.db.query(id)\n    }\n}\n\napp MyApp[users: UserService] {\n    fn main(self) {\n        print(self.users.get_user(\"42\"))\n    }\n}",
    );
    assert_eq!(output.trim(), "42");
}

#[test]
fn scoped_class_with_methods() {
    let output = compile_and_run_stdout(
        "scoped class Counter {\n    count: int\n\n    fn increment(mut self) {\n        self.count = self.count + 1\n    }\n\n    fn value(self) int {\n        return self.count\n    }\n}\n\nfn main() {\n    let mut c = Counter { count: 0 }\n    c.increment()\n    c.increment()\n    print(c.value())\n}",
    );
    assert_eq!(output.trim(), "2");
}

#[test]
fn transient_class_in_di() {
    // Transient class as a dependency — should infer transient on the dependent.
    let output = compile_and_run_stdout(
        "transient class Logger {\n    fn log(self, msg: string) {\n        print(msg)\n    }\n}\n\nclass Service[logger: Logger] {\n    fn run(self) {\n        self.logger.log(\"hello\")\n    }\n}\n\napp MyApp[svc: Service] {\n    fn main(self) {\n        self.svc.run()\n    }\n}",
    );
    assert_eq!(output.trim(), "hello");
}
