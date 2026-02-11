mod common;
use common::{compile_and_run, compile_and_run_stdout, compile_should_fail_with};

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
fn scoped_generic_class_allowed() {
    let exit = compile_and_run(
        "scoped class Box<T> {\n    value: T\n}\n\nfn main() {\n    let b = Box<int> { value: 42 }\n    print(b.value)\n}",
    );
    assert_eq!(exit, 0);
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

// === Phase 2: Singleton globals ===

#[test]
fn singleton_globals_app_with_deps() {
    // App with a 3-class DI chain: C -> B -> A. Verifies wiring still works
    // after singleton pointers are stored to module-level globals.
    let output = compile_and_run_stdout(
        r#"class Config {
    fn db_url(self) string {
        return "postgres://localhost"
    }
}

class Database[config: Config] {
    fn url(self) string {
        return self.config.db_url()
    }
}

class UserService[db: Database] {
    fn get_url(self) string {
        return self.db.url()
    }
}

app MyApp[users: UserService] {
    fn main(self) {
        print(self.users.get_url())
    }
}"#,
    );
    assert_eq!(output.trim(), "postgres://localhost");
}

#[test]
fn singleton_globals_no_app() {
    // Program with no app — empty singleton_data_ids map, no globals stored.
    // Should compile and run without issue.
    let output = compile_and_run_stdout(
        r#"fn main() {
    print("no app")
}"#,
    );
    assert_eq!(output.trim(), "no app");
}

// === Phase 5d: App-level lifecycle overrides ===

#[test]
fn app_override_to_scoped() {
    // Override a default-singleton class to scoped, use in scope block
    let output = compile_and_run_stdout(r#"
class Pool {
    name: string

    fn query(self) string {
        return self.name
    }
}

scoped class ReqCtx {
    id: string
}

scoped class Handler[pool: Pool, ctx: ReqCtx] {
    fn handle(self) string {
        return self.pool.query()
    }
}

app MyApp {
    scoped Pool

    fn main(self) {
        scope(ReqCtx { id: "r1" }, Pool { name: "db" }) |h: Handler| {
            print(h.handle())
        }
    }
}
"#);
    assert_eq!(output.trim(), "db");
}

#[test]
fn app_override_to_transient() {
    // Override a default-singleton class to transient via app directive
    let output = compile_and_run_stdout(r#"
class Logger {
    tag: string

    fn log(self) string {
        return self.tag
    }
}

app MyApp {
    transient Logger

    fn main(self) {
        let l = Logger { tag: "logged" }
        print(l.log())
    }
}
"#);
    assert_eq!(output.trim(), "logged");
}

#[test]
fn app_override_lengthen_rejected() {
    // Cannot lengthen: transient → singleton is an error
    compile_should_fail_with(r#"
transient class Foo {
    x: int
}

app MyApp {
    scoped Foo

    fn main(self) {
    }
}
"#, "cannot lengthen lifecycle");
}

#[test]
fn app_override_unknown_class_rejected() {
    // Override non-existent class → error
    compile_should_fail_with(r#"
app MyApp {
    scoped NonExistent

    fn main(self) {
    }
}
"#, "unknown class");
}

#[test]
fn app_override_bracket_dep_rejected() {
    // App bracket dep on overridden class → error
    compile_should_fail_with(r#"
class Pool {
    name: string

    fn query(self) string {
        return self.name
    }
}

app MyApp[pool: Pool] {
    scoped Pool

    fn main(self) {
        print(self.pool.query())
    }
}
"#, "overridden lifecycle");
}

#[test]
fn app_override_propagates() {
    // Override B to scoped → A (depends on B) also excluded from singletons
    let output = compile_and_run_stdout(r#"
class B {
    tag: string

    fn val(self) string {
        return self.tag
    }
}

class A[b: B] {
    fn val(self) string {
        return self.b.val()
    }
}

scoped class Seed {
    id: int
}

scoped class Handler[a: A, seed: Seed] {
    fn handle(self) string {
        return self.a.val()
    }
}

app MyApp {
    scoped B

    fn main(self) {
        scope(Seed { id: 1 }, B { tag: "b" }) |h: Handler| {
            print(h.handle())
        }
    }
}
"#);
    assert_eq!(output.trim(), "b");
}
