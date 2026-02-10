mod common;
use common::{compile_and_run, compile_and_run_stdout, compile_should_fail, compile_should_fail_with};

#[test]
fn app_basic() {
    let output = compile_and_run_stdout(
        "app Main {\n    fn main(self) {\n        print(\"hello\")\n    }\n}",
    );
    assert_eq!(output.trim(), "hello");
}

#[test]
fn app_inject_simple() {
    let output = compile_and_run_stdout(
        "class Database {\n    fn query(self, q: string) string {\n        return q\n    }\n}\n\nclass UserService[db: Database] {\n    fn get_user(self, id: string) string {\n        return self.db.query(id)\n    }\n}\n\napp MyApp[users: UserService] {\n    fn main(self) {\n        print(self.users.get_user(\"42\"))\n    }\n}",
    );
    assert_eq!(output.trim(), "42");
}

#[test]
fn app_inject_chain() {
    let output = compile_and_run_stdout(
        "class C {\n    fn value(self) string {\n        return \"deep\"\n    }\n}\n\nclass B[c: C] {\n    fn value(self) string {\n        return self.c.value()\n    }\n}\n\nclass A[b: B] {\n    fn value(self) string {\n        return self.b.value()\n    }\n}\n\napp MyApp[a: A] {\n    fn main(self) {\n        print(self.a.value())\n    }\n}",
    );
    assert_eq!(output.trim(), "deep");
}

#[test]
fn app_inject_shared() {
    let output = compile_and_run_stdout(
        "class Database {\n    fn name(self) string {\n        return \"shared_db\"\n    }\n}\n\nclass ServiceA[db: Database] {\n    fn info(self) string {\n        return self.db.name()\n    }\n}\n\nclass ServiceB[db: Database] {\n    fn info(self) string {\n        return self.db.name()\n    }\n}\n\napp MyApp[a: ServiceA, b: ServiceB] {\n    fn main(self) {\n        print(self.a.info())\n        print(self.b.info())\n    }\n}",
    );
    assert_eq!(output.trim(), "shared_db\nshared_db");
}

#[test]
fn app_class_with_regular_and_inject_fields() {
    let output = compile_and_run_stdout(
        "class Logger {\n    fn log(self, msg: string) {\n        print(msg)\n    }\n}\n\nclass Service[logger: Logger] {\n    count: int\n\n    fn run(self) {\n        self.logger.log(\"running\")\n        print(self.count)\n    }\n}\n\napp MyApp[svc: Service] {\n    fn main(self) {\n        self.svc.run()\n    }\n}",
    );
    assert_eq!(output.trim(), "running\n0");
}

#[test]
fn di_cycle_rejected() {
    compile_should_fail(
        "class A[b: B] {\n}\n\nclass B[a: A] {\n}\n\napp MyApp[a: A] {\n    fn main(self) {\n    }\n}",
    );
}

#[test]
fn di_struct_lit_rejected() {
    compile_should_fail(
        "class Database {\n}\n\nclass UserService[db: Database] {\n}\n\nfn main() {\n    let u = UserService { db: Database { } }\n}",
    );
}

#[test]
fn app_and_main_rejected() {
    compile_should_fail(
        "fn main() {\n}\n\napp MyApp {\n    fn main(self) {\n    }\n}",
    );
}

#[test]
fn app_main_signature_errors() {
    compile_should_fail(
        "app MyApp {\n    fn main() {\n    }\n}",
    );
}

#[test]
fn app_no_deps() {
    let exit_code = compile_and_run(
        "app MyApp {\n    fn main(self) {\n    }\n}",
    );
    assert_eq!(exit_code, 0);
}

#[test]
fn app_multiple_methods() {
    let output = compile_and_run_stdout(
        "app MyApp {\n    fn helper(self) string {\n        return \"helped\"\n    }\n\n    fn main(self) {\n        print(self.helper())\n    }\n}",
    );
    assert_eq!(output.trim(), "helped");
}

// === Ambient DI tests ===

#[test]
fn ambient_basic() {
    let src = "\
class Logger {
    fn info(self, msg: string) {
        print(msg)
    }
}

class OrderService uses Logger {
    fn process(self) {
        logger.info(\"processing\")
    }
}

app MyApp[svc: OrderService] {
    ambient Logger

    fn main(self) {
        self.svc.process()
    }
}";
    let output = compile_and_run_stdout(src);
    assert_eq!(output.trim(), "processing");
}

#[test]
fn ambient_with_bracket_deps() {
    let src = "\
class Logger {
    fn info(self, msg: string) {
        print(msg)
    }
}

class Database {
    fn query(self, q: string) string {
        return q
    }
}

class OrderService uses Logger [db: Database] {
    fn process(self) {
        logger.info(\"processing\")
        print(self.db.query(\"SELECT 1\"))
    }
}

app MyApp[svc: OrderService] {
    ambient Logger

    fn main(self) {
        self.svc.process()
    }
}";
    let output = compile_and_run_stdout(src);
    assert_eq!(output.trim(), "processing\nSELECT 1");
}

#[test]
fn ambient_multiple_types() {
    let src = "\
class Logger {
    fn info(self, msg: string) {
        print(msg)
    }
}

class Config {
    fn get(self, key: string) string {
        return \"value\"
    }
}

class Service uses Logger, Config {
    fn run(self) {
        logger.info(\"running\")
        print(config.get(\"key\"))
    }
}

app MyApp[svc: Service] {
    ambient Logger
    ambient Config

    fn main(self) {
        self.svc.run()
    }
}";
    let output = compile_and_run_stdout(src);
    assert_eq!(output.trim(), "running\nvalue");
}

#[test]
fn ambient_app_method_access() {
    let src = "\
class Logger {
    fn info(self, msg: string) {
        print(msg)
    }
}

app MyApp {
    ambient Logger

    fn main(self) {
        logger.info(\"from app\")
    }
}";
    let output = compile_and_run_stdout(src);
    assert_eq!(output.trim(), "from app");
}

#[test]
fn ambient_shared_singleton() {
    let src = "\
class Counter {
    count: int

    fn inc(mut self) {
        self.count = self.count + 1
    }

    fn value(self) int {
        return self.count
    }
}

class ServiceA uses Counter {
    fn run(self) {
        counter.inc()
        counter.inc()
    }

    fn get_count(self) int {
        return counter.value()
    }
}

class ServiceB uses Counter {
    fn run(self) {
        counter.inc()
    }

    fn get_count(self) int {
        return counter.value()
    }
}

app MyApp[a: ServiceA, b: ServiceB] {
    ambient Counter

    fn main(self) {
        self.a.run()
        self.b.run()
        print(self.a.get_count())
        print(self.b.get_count())
    }
}";
    // Counter is a singleton shared between A and B: 2 + 1 = 3
    // Both see the same counter, so both get_count() returns 3
    let output = compile_and_run_stdout(src);
    assert_eq!(output.trim(), "3\n3");
}

#[test]
fn ambient_variable_shadowing() {
    let src = "\
class Logger {
    fn info(self, msg: string) {
        print(msg)
    }
}

class Service uses Logger {
    fn run(self) {
        logger.info(\"before shadow\")
        let logger = 42
        print(logger)
    }
}

app MyApp[svc: Service] {
    ambient Logger

    fn main(self) {
        self.svc.run()
    }
}";
    let output = compile_and_run_stdout(src);
    assert_eq!(output.trim(), "before shadow\n42");
}

#[test]
fn ambient_closure_param_shadow() {
    let src = "\
class Logger {
    fn info(self, msg: string) {
        print(msg)
    }
}

class Service uses Logger {
    fn run(self) {
        logger.info(\"outer\")
        let f = (logger: int) => logger + 1
        print(f(10))
    }
}

app MyApp[svc: Service] {
    ambient Logger

    fn main(self) {
        self.svc.run()
    }
}";
    let output = compile_and_run_stdout(src);
    assert_eq!(output.trim(), "outer\n11");
}

#[test]
fn ambient_match_binding_shadow() {
    let src = "\
class Logger {
    fn info(self, msg: string) {
        print(msg)
    }
}

enum Wrapper {
    Val { logger: int }
}

class Service uses Logger {
    fn run(self) {
        logger.info(\"before match\")
        let w = Wrapper.Val { logger: 99 }
        match w {
            Wrapper.Val { logger } {
                print(logger)
            }
        }
    }
}

app MyApp[svc: Service] {
    ambient Logger

    fn main(self) {
        self.svc.run()
    }
}";
    let output = compile_and_run_stdout(src);
    assert_eq!(output.trim(), "before match\n99");
}

#[test]
fn ambient_for_loop_shadow() {
    let src = "\
class Logger {
    fn info(self, msg: string) {
        print(msg)
    }
}

class Service uses Logger {
    fn run(self) {
        logger.info(\"before loop\")
        let items = [10, 20]
        for logger in items {
            print(logger)
        }
        logger.info(\"after loop\")
    }
}

app MyApp[svc: Service] {
    ambient Logger

    fn main(self) {
        self.svc.run()
    }
}";
    let output = compile_and_run_stdout(src);
    assert_eq!(output.trim(), "before loop\n10\n20\nafter loop");
}

#[test]
fn ambient_assignment() {
    let src = "\
class Logger {
    level: int

    fn get_level(self) int {
        return self.level
    }
}

class Service uses Logger {
    fn run(self) {
        logger.level = 5
        print(logger.get_level())
    }
}

app MyApp[svc: Service] {
    ambient Logger

    fn main(self) {
        self.svc.run()
    }
}";
    let output = compile_and_run_stdout(src);
    assert_eq!(output.trim(), "5");
}

#[test]
fn ambient_without_app_rejected() {
    compile_should_fail_with(
        "\
class Logger {
}

class Service uses Logger {
    fn run(self) {
    }
}

app MyApp[svc: Service] {
    fn main(self) {
    }
}",
        "is not declared ambient in the app",
    );
}

#[test]
fn ambient_unknown_type_rejected() {
    compile_should_fail_with(
        "\
app MyApp {
    ambient NonExistent

    fn main(self) {
    }
}",
        "unknown type 'NonExistent'",
    );
}

#[test]
fn ambient_generic_class_rejected() {
    compile_should_fail_with(
        "\
class Logger {
}

class Box<T> uses Logger {
    value: T
}

app MyApp {
    ambient Logger

    fn main(self) {
    }
}",
        "generic class 'Box' cannot use ambient dependencies",
    );
}

#[test]
fn ambient_no_app_rejected() {
    compile_should_fail_with(
        "\
class Logger {
}

class Service uses Logger {
    fn run(self) {
    }
}

fn main() {
}",
        "no app declaration exists",
    );
}

#[test]
fn ambient_catch_wildcard_shadow() {
    let src = "\
class Logger {
    fn info(self, msg: string) {
        print(msg)
    }
}

error MyError {
    msg: string
}

fn maybe_fail() int {
    raise MyError { msg: \"oops\" }
    return 42
}

class Service uses Logger {
    fn run(self) {
        logger.info(\"before catch\")
        let result = maybe_fail() catch logger {
            0
        }
        print(result)
    }
}

app MyApp[svc: Service] {
    ambient Logger

    fn main(self) {
        self.svc.run()
    }
}";
    // maybe_fail always raises, so catch fires; `logger` in catch body is the error, not the ambient
    let output = compile_and_run_stdout(src);
    assert_eq!(output.trim(), "before catch\n0");
}
