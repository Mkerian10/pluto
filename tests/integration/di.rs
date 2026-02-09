mod common;
use common::{compile_and_run, compile_and_run_stdout, compile_should_fail};

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
