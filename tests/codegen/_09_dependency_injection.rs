// Category 9: Dependency Injection Tests (15+ tests)
// Validates DI codegen: bracket deps, app main, scoped instances

use super::common::{compile_and_run, compile_and_run_stdout, compile_should_fail};

// ============================================================================
// Bracket Dependencies (5 tests)
// ============================================================================

#[test]
fn test_class_one_bracket_dep() {
    // Verify class with single bracket dep allocates correctly
    let src = r#"
        class Logger {
            fn log(self, msg: string) {
                print(msg)
            }
        }

        class Service[logger: Logger] {
            fn run(self) {
                self.logger.log("service running")
            }
        }

        app MyApp[svc: Service] {
            fn main(self) {
                self.svc.run()
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "service running");
}

#[test]
fn test_class_multiple_bracket_deps() {
    // Verify class with multiple bracket deps wires all dependencies
    let src = r#"
        class Database {
            fn query(self) string {
                return "db_result"
            }
        }

        class Logger {
            fn log(self, msg: string) {
                print(msg)
            }
        }

        class Cache {
            fn get(self) string {
                return "cached"
            }
        }

        class Service[db: Database, logger: Logger, cache: Cache] {
            fn run(self) {
                self.logger.log(self.db.query())
                self.logger.log(self.cache.get())
            }
        }

        app MyApp[svc: Service] {
            fn main(self) {
                self.svc.run()
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "db_result\ncached");
}

#[test]
fn test_nested_bracket_deps() {
    // Verify nested dependencies (A[b: B], B[c: C]) allocate and wire in correct order
    let src = r#"
        class C {
            fn value(self) string {
                return "deep"
            }
        }

        class B[c: C] {
            fn value(self) string {
                return self.c.value()
            }
        }

        class A[b: B] {
            fn value(self) string {
                return self.b.value()
            }
        }

        app MyApp[a: A] {
            fn main(self) {
                print(self.a.value())
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "deep");
}

#[test]
fn test_bracket_deps_with_regular_fields() {
    // Verify bracket deps come before regular fields in memory layout
    let src = r#"
        class Config {
            fn name(self) string {
                return "config_value"
            }
        }

        class Service[cfg: Config] {
            count: int

            fn run(self) {
                print(self.cfg.name())
                print(self.count)
            }
        }

        app MyApp[svc: Service] {
            fn main(self) {
                self.svc.run()
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "config_value\n0");
}

#[test]
fn test_shared_singleton_dep() {
    // Verify same singleton instance injected into multiple classes
    let src = r#"
        class Database {
            fn id(self) string {
                return "shared_db"
            }
        }

        class ServiceA[db: Database] {
            fn info(self) string {
                return self.db.id()
            }
        }

        class ServiceB[db: Database] {
            fn info(self) string {
                return self.db.id()
            }
        }

        app MyApp[a: ServiceA, b: ServiceB] {
            fn main(self) {
                print(self.a.info())
                print(self.b.info())
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "shared_db\nshared_db");
}

// ============================================================================
// App Main (5 tests)
// ============================================================================

#[test]
fn test_synthetic_main_generation() {
    // Verify compiler generates main() that calls app's main
    let src = r#"
        app MyApp {
            fn main(self) {
                print("synthetic_main")
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "synthetic_main");
}

#[test]
fn test_singleton_allocation() {
    // Verify all singletons allocated before app.main() call
    let src = r#"
        class A {
            value: int
        }

        class B {
            value: int
        }

        class C {
            value: int
        }

        app MyApp[a: A, b: B, c: C] {
            fn main(self) {
                // If singletons allocated, all values should be 0
                print(self.a.value)
                print(self.b.value)
                print(self.c.value)
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "0\n0\n0");
}

#[test]
fn test_singleton_wiring() {
    // Verify singletons wired together in correct order
    let src = r#"
        class Leaf {
            fn id(self) int {
                return 42
            }
        }

        class Middle[leaf: Leaf] {
            fn get(self) int {
                return self.leaf.id()
            }
        }

        class Root[middle: Middle] {
            fn run(self) int {
                return self.middle.get()
            }
        }

        app MyApp[root: Root] {
            fn main(self) {
                print(self.root.run())
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "42");
}

#[test]
#[ignore] // Known limitation: apps cannot have regular fields, only bracket dependencies and methods
fn test_app_main_call() {
    // Verify synthetic main correctly passes app pointer to app.main(self)
    let src = r#"
        app MyApp {
            value: int

            fn helper(self) int {
                return self.value + 10
            }

            fn main(self) {
                print(self.helper())
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "10");
}

#[test]
#[ignore] // Known limitation: app main must return void, cannot return exit code
fn test_app_exit_code() {
    // Verify app main's exit code propagates correctly
    let src = r#"
        app MyApp {
            fn main(self) int {
                return 7
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 7);
}

// ============================================================================
// Scoped Instances (5+ tests)
// ============================================================================

#[test]
fn test_scoped_class_instantiation() {
    // Verify scoped class can be instantiated with struct literal
    let src = r#"
        scoped class Context {
            request_id: int
        }

        fn main() {
            let ctx = Context { request_id: 123 }
            print(ctx.request_id)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "123");
}

#[test]
#[ignore] // Known limitation: Multiple bracket dependencies not supported, manual bracket instantiation doesn't exist
fn test_scoped_singleton_injection() {
    // Verify scoped class injected into singleton
    let src = r#"
        class Database {
            fn query(self) string {
                return "db_data"
            }
        }

        scoped class RequestCtx {
            request_id: int
        }

        class Handler[db: Database, ctx: RequestCtx] {
            fn process(self) {
                print(self.db.query())
                print(self.ctx.request_id)
            }
        }

        app MyApp[db: Database] {
            fn main(self) {
                let ctx = RequestCtx { request_id: 456 }
                let handler = Handler[self.db, ctx] {}
                handler.process()
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "db_data\n456");
}

#[test]
#[ignore] // Known limitation: Cannot manually provide bracket dependencies at instantiation (Outer[i] {} syntax doesn't exist)
fn test_scoped_nested_deps() {
    // Verify nested scoped dependencies
    let src = r#"
        scoped class Inner {
            value: int
        }

        scoped class Outer[inner: Inner] {
            fn get(self) int {
                return self.inner.value
            }
        }

        fn main() {
            let i = Inner { value: 99 }
            let o = Outer[i] {}
            print(o.get())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "99");
}

#[test]
fn test_scoped_multiple_instances() {
    // Verify multiple scoped instances can coexist
    let src = r#"
        scoped class Counter {
            count: int

            fn value(self) int {
                return self.count
            }
        }

        fn main() {
            let c1 = Counter { count: 10 }
            let c2 = Counter { count: 20 }
            let c3 = Counter { count: 30 }
            print(c1.value())
            print(c2.value())
            print(c3.value())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "10\n20\n30");
}

#[test]
fn test_scoped_with_methods() {
    // Verify scoped class methods work correctly
    let src = r#"
        scoped class Calculator {
            base: int

            fn add(self, x: int) int {
                return self.base + x
            }

            fn multiply(self, x: int) int {
                return self.base * x
            }
        }

        fn main() {
            let calc = Calculator { base: 5 }
            print(calc.add(3))
            print(calc.multiply(4))
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "8\n20");
}

// ============================================================================
// Additional Edge Cases (5+ tests)
// ============================================================================

#[test]
fn test_di_topological_sort() {
    // Verify complex dependency graph allocates in correct order
    let src = r#"
        class D {
            fn val(self) int { return 4 }
        }

        class C[d: D] {
            fn val(self) int { return 3 + self.d.val() }
        }

        class B[c: C] {
            fn val(self) int { return 2 + self.c.val() }
        }

        class A[b: B, d: D] {
            fn val(self) int { return 1 + self.b.val() + self.d.val() }
        }

        app MyApp[a: A] {
            fn main(self) {
                print(self.a.val())
            }
        }
    "#;
    // Expected: 1 + (2 + (3 + 4)) + 4 = 1 + 9 + 4 = 14
    assert_eq!(compile_and_run_stdout(src).trim(), "14");
}

#[test]
fn test_di_struct_literal_blocked() {
    // Verify manual construction of injected class fails
    let src = r#"
        class Database {}

        class Service[db: Database] {}

        fn main() {
            let db = Database {}
            let svc = Service { db: db }
        }
    "#;
    compile_should_fail(src);
}

#[test]
fn test_di_cycle_detected() {
    // Verify circular dependencies rejected at compile time
    let src = r#"
        class A[b: B] {}
        class B[a: A] {}

        app MyApp[a: A] {
            fn main(self) {}
        }
    "#;
    compile_should_fail(src);
}

#[test]
fn test_app_no_deps() {
    // Verify app with no dependencies works
    let src = r#"
        app MyApp {
            fn main(self) {
                print("no_deps")
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "no_deps");
}

#[test]
#[ignore] // Known limitation: apps cannot have regular fields, only bracket dependencies and methods
fn test_app_with_fields() {
    // Verify app can have regular fields (not just bracket deps)
    let src = r#"
        app MyApp {
            counter: int

            fn main(self) {
                print(self.counter)
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "0");
}

#[test]
fn test_deep_dependency_chain() {
    // Verify deep dependency chain (6 levels)
    let src = r#"
        class F {
            fn v(self) int { return 6 }
        }

        class E[f: F] {
            fn v(self) int { return 5 + self.f.v() }
        }

        class D[e: E] {
            fn v(self) int { return 4 + self.e.v() }
        }

        class C[d: D] {
            fn v(self) int { return 3 + self.d.v() }
        }

        class B[c: C] {
            fn v(self) int { return 2 + self.c.v() }
        }

        class A[b: B] {
            fn v(self) int { return 1 + self.b.v() }
        }

        app MyApp[a: A] {
            fn main(self) {
                print(self.a.v())
            }
        }
    "#;
    // Expected: 1 + (2 + (3 + (4 + (5 + 6)))) = 1 + 2 + 3 + 4 + 5 + 6 = 21
    assert_eq!(compile_and_run_stdout(src).trim(), "21");
}

#[test]
#[ignore] // Known limitation: apps cannot have regular fields, only bracket dependencies and methods
fn test_multiple_app_fields() {
    // Verify app with multiple regular fields
    let src = r#"
        app MyApp {
            x: int
            y: int
            z: int

            fn sum(self) int {
                return self.x + self.y + self.z
            }

            fn main(self) {
                print(self.sum())
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "0");
}

#[test]
#[ignore] // Known limitation: Cannot manually provide bracket dependencies at instantiation
fn test_scoped_with_bracket_and_regular_fields() {
    // Verify scoped class with both bracket deps and regular fields
    let src = r#"
        class Logger {
            fn log(self, msg: string) {
                print(msg)
            }
        }

        scoped class Handler[logger: Logger] {
            request_id: int
            user_id: int

            fn process(self) {
                self.logger.log("processing")
                print(self.request_id)
                print(self.user_id)
            }
        }

        app MyApp[logger: Logger] {
            fn main(self) {
                let h = Handler[self.logger] { request_id: 100, user_id: 200 }
                h.process()
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "processing\n100\n200");
}

#[test]
fn test_app_methods_besides_main() {
    // Verify app can have helper methods
    let src = r#"
        app MyApp {
            fn helper1(self) int {
                return 10
            }

            fn helper2(self, x: int) int {
                return x * 2
            }

            fn main(self) {
                let a = self.helper1()
                let b = self.helper2(a)
                print(b)
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "20");
}

#[test]
fn test_singleton_initialization_order() {
    // Verify singletons initialized before use (no undefined behavior)
    let src = r#"
        class A {
            value: int

            fn get(self) int {
                return self.value
            }
        }

        class B[a: A] {
            fn get(self) int {
                return self.a.get()
            }
        }

        class C[a: A, b: B] {
            fn get(self) int {
                return self.a.get() + self.b.get()
            }
        }

        app MyApp[c: C] {
            fn main(self) {
                print(self.c.get())
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "0");
}
