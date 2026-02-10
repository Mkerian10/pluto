mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

#[test]
fn scope_basic_seed_and_binding() {
    let output = compile_and_run_stdout(r#"
scoped class RequestCtx {
    request_id: string

    fn get_id(self) string {
        return self.request_id
    }
}

scoped class UserService[ctx: RequestCtx] {
    fn get_request_id(self) string {
        return self.ctx.get_id()
    }
}

app MyApp {
    fn main(self) {
        scope(RequestCtx { request_id: "abc" }) |svc: UserService| {
            print(svc.get_request_id())
        }
    }
}
"#);
    assert_eq!(output.trim(), "abc");
}

#[test]
fn scope_seed_is_also_binding() {
    let output = compile_and_run_stdout(r#"
scoped class Ctx {
    value: int

    fn get_value(self) int {
        return self.value
    }
}

app MyApp {
    fn main(self) {
        scope(Ctx { value: 42 }) |c: Ctx| {
            print(c.get_value())
        }
    }
}
"#);
    assert_eq!(output.trim(), "42");
}

#[test]
fn scope_auto_create_scoped_class() {
    // UserService has only injected fields so it can be auto-created
    let output = compile_and_run_stdout(r#"
scoped class RequestCtx {
    id: int

    fn get_id(self) int {
        return self.id
    }
}

scoped class UserService[ctx: RequestCtx] {
    fn get_id(self) int {
        return self.ctx.get_id()
    }
}

scoped class OrderService[ctx: RequestCtx] {
    fn get_id(self) int {
        return self.ctx.get_id()
    }
}

app MyApp {
    fn main(self) {
        scope(RequestCtx { id: 99 }) |user_svc: UserService, order_svc: OrderService| {
            print(user_svc.get_id())
            print(order_svc.get_id())
        }
    }
}
"#);
    assert_eq!(output.trim(), "99\n99");
}

#[test]
fn scope_with_singleton_dep() {
    // A scoped class that depends on a singleton
    let output = compile_and_run_stdout(r#"
class Database {
    fn query(self, q: string) string {
        return q
    }
}

scoped class RequestCtx {
    request_id: string

    fn get_id(self) string {
        return self.request_id
    }
}

scoped class UserService[db: Database, ctx: RequestCtx] {
    fn get_user(self) string {
        return self.db.query(self.ctx.get_id())
    }
}

app MyApp[db: Database] {
    fn main(self) {
        scope(RequestCtx { request_id: "user-123" }) |svc: UserService| {
            print(svc.get_user())
        }
    }
}
"#);
    assert_eq!(output.trim(), "user-123");
}

#[test]
fn scope_multiple_seeds() {
    let output = compile_and_run_stdout(r#"
scoped class AuthCtx {
    user_id: int
}

scoped class RequestCtx {
    trace_id: string

    fn get_trace(self) string {
        return self.trace_id
    }
}

scoped class Handler[auth: AuthCtx, req: RequestCtx] {
    fn info(self) string {
        return self.req.get_trace()
    }
}

app MyApp {
    fn main(self) {
        scope(AuthCtx { user_id: 1 }, RequestCtx { trace_id: "t-abc" }) |h: Handler| {
            print(h.info())
        }
    }
}
"#);
    assert_eq!(output.trim(), "t-abc");
}

#[test]
fn scope_method_calls_on_bindings() {
    let output = compile_and_run_stdout(r#"
scoped class Counter {
    value: int

    fn get(self) int {
        return self.value
    }
}

app MyApp {
    fn main(self) {
        scope(Counter { value: 10 }) |c: Counter| {
            let x = c.get()
            print(x + 5)
        }
    }
}
"#);
    assert_eq!(output.trim(), "15");
}

#[test]
fn scope_chain_of_scoped_deps() {
    // A -> B -> C, all scoped, only C needs a seed
    let output = compile_and_run_stdout(r#"
scoped class Config {
    port: int

    fn get_port(self) int {
        return self.port
    }
}

scoped class Logger[cfg: Config] {
    fn info(self) int {
        return self.cfg.get_port()
    }
}

scoped class Server[logger: Logger] {
    fn start(self) int {
        return self.logger.info()
    }
}

app MyApp {
    fn main(self) {
        scope(Config { port: 8080 }) |s: Server| {
            print(s.start())
        }
    }
}
"#);
    assert_eq!(output.trim(), "8080");
}

#[test]
fn scope_body_with_multiple_statements() {
    let output = compile_and_run_stdout(r#"
scoped class Ctx {
    name: string

    fn get_name(self) string {
        return self.name
    }
}

app MyApp {
    fn main(self) {
        scope(Ctx { name: "world" }) |c: Ctx| {
            let greeting = "hello"
            print(greeting)
            print(c.get_name())
        }
    }
}
"#);
    assert_eq!(output.trim(), "hello\nworld");
}

// Error tests

#[test]
fn scope_error_seed_not_scoped() {
    compile_should_fail_with(r#"
class NotScoped {
    x: int

    fn get_x(self) int {
        return self.x
    }
}

app MyApp {
    fn main(self) {
        scope(NotScoped { x: 1 }) |n: NotScoped| {
            print(n.get_x())
        }
    }
}
"#, "scoped class");
}

#[test]
fn scope_error_non_injected_fields_not_seed() {
    compile_should_fail_with(r#"
scoped class Ctx {
    id: int
}

scoped class Svc[ctx: Ctx] {
    extra: int
}

app MyApp {
    fn main(self) {
        scope(Ctx { id: 1 }) |svc: Svc| {
            print(svc.extra)
        }
    }
}
"#, "non-injected fields");
}

#[test]
fn scope_error_binding_not_class() {
    compile_should_fail_with(r#"
scoped class Ctx {
    id: int
}

app MyApp {
    fn main(self) {
        scope(Ctx { id: 1 }) |x: int| {
            print(x)
        }
    }
}
"#, "class type");
}

#[test]
fn scope_error_seed_not_class() {
    compile_should_fail_with(r#"
app MyApp {
    fn main(self) {
        scope(42) |x: int| {
            print(x)
        }
    }
}
"#, "class instance");
}

// === Phase 5a: Nested Scope Tests ===

#[test]
fn nested_scope_basic() {
    // Inner scope has a different seed type; both scopes produce correct results
    let output = compile_and_run_stdout(r#"
scoped class Outer {
    value: int

    fn get(self) int {
        return self.value
    }
}

scoped class Inner {
    name: string

    fn get(self) string {
        return self.name
    }
}

app MyApp {
    fn main(self) {
        scope(Outer { value: 10 }) |o: Outer| {
            print(o.get())
            scope(Inner { name: "nested" }) |i: Inner| {
                print(i.get())
            }
        }
    }
}
"#);
    assert_eq!(output.trim(), "10\nnested");
}

#[test]
fn nested_scope_shadowing() {
    // Inner scope binding shadows outer binding name; outer restored after inner exits
    let output = compile_and_run_stdout(r#"
scoped class Ctx {
    value: int

    fn get(self) int {
        return self.value
    }
}

app MyApp {
    fn main(self) {
        scope(Ctx { value: 1 }) |c: Ctx| {
            print(c.get())
            scope(Ctx { value: 2 }) |c: Ctx| {
                print(c.get())
            }
            print(c.get())
        }
    }
}
"#);
    assert_eq!(output.trim(), "1\n2\n1");
}

#[test]
fn nested_scope_sequential() {
    // Two scope blocks side-by-side inside an outer scope; each gets fresh instances
    let output = compile_and_run_stdout(r#"
scoped class Seed {
    id: int

    fn get_id(self) int {
        return self.id
    }
}

scoped class Svc[seed: Seed] {
    fn info(self) int {
        return self.seed.get_id()
    }
}

app MyApp {
    fn main(self) {
        scope(Seed { id: 100 }) |outer: Svc| {
            print(outer.info())
            scope(Seed { id: 200 }) |inner1: Svc| {
                print(inner1.info())
            }
            scope(Seed { id: 300 }) |inner2: Svc| {
                print(inner2.info())
            }
        }
    }
}
"#);
    assert_eq!(output.trim(), "100\n200\n300");
}

// === Phase 5b: Scope + Ambient DI Tests ===

#[test]
fn scope_ambient_singleton_dep() {
    // Scoped class `uses Logger` (singleton), Logger wired from singleton global
    let output = compile_and_run_stdout(r#"
class Logger {
    fn log(self, msg: string) string {
        return msg
    }
}

scoped class RequestCtx {
    request_id: string

    fn get_id(self) string {
        return self.request_id
    }
}

scoped class Handler uses Logger [ctx: RequestCtx] {
    fn handle(self) string {
        let id = self.ctx.get_id()
        return self.logger.log(id)
    }
}

app MyApp {
    ambient Logger

    fn main(self) {
        scope(RequestCtx { request_id: "req-1" }) |h: Handler| {
            print(h.handle())
        }
    }
}
"#);
    assert_eq!(output.trim(), "req-1");
}

#[test]
fn scope_ambient_scoped_dep() {
    // Scoped class `uses RequestCtx` (scoped seed), ambient wired from scope seed
    let output = compile_and_run_stdout(r#"
scoped class RequestCtx {
    trace: string

    fn get_trace(self) string {
        return self.trace
    }
}

scoped class Handler uses RequestCtx {
    fn handle(self) string {
        return self.requestCtx.get_trace()
    }
}

app MyApp {
    ambient RequestCtx

    fn main(self) {
        scope(RequestCtx { trace: "t-abc" }) |h: Handler| {
            print(h.handle())
        }
    }
}
"#);
    assert_eq!(output.trim(), "t-abc");
}

#[test]
fn scope_ambient_chain() {
    // A `uses B`, B `uses C` (singleton), seed A in scope block, verify full chain
    let output = compile_and_run_stdout(r#"
class Config {
    fn get_db(self) string {
        return "postgres"
    }
}

scoped class Ctx {
    request_id: string
}

scoped class Service uses Config [ctx: Ctx] {
    fn info(self) string {
        return self.config.get_db()
    }
}

app MyApp {
    ambient Config

    fn main(self) {
        scope(Ctx { request_id: "r1" }) |svc: Service| {
            print(svc.info())
        }
    }
}
"#);
    assert_eq!(output.trim(), "postgres");
}

// === Phase 5c: Scope + Spawn Safety Tests ===

#[test]
fn scope_spawn_captures_binding_rejected() {
    // spawn inside scope body that captures a scope binding → error
    compile_should_fail_with(r#"
fn do_work(x: int) int {
    return x
}

scoped class Ctx {
    value: int
}

app MyApp {
    fn main(self) {
        scope(Ctx { value: 1 }) |c: Ctx| {
            let t = spawn do_work(c.value)
            print(t.get())
        }
    }
}
"#, "cannot spawn inside scope block");
}

#[test]
fn scope_spawn_no_binding_ok() {
    // spawn in a program that also has scope blocks but spawn is outside scopes
    // Verifies scope safety checks don't interfere with normal spawns
    let output = compile_and_run_stdout(r#"
fn add(a: int, b: int) int {
    return a + b
}

fn main() {
    let t = spawn add(10, 20)
    print(t.get())
}
"#);
    assert_eq!(output.trim(), "30");
}

#[test]
fn scope_spawn_binding_in_expr_rejected() {
    // spawn with binding used in a method call expression → error
    compile_should_fail_with(r#"
fn do_work(x: int) int {
    return x
}

scoped class Ctx {
    value: int

    fn get_value(self) int {
        return self.value
    }
}

app MyApp {
    fn main(self) {
        scope(Ctx { value: 5 }) |c: Ctx| {
            let t = spawn do_work(c.get_value())
            print(t.get())
        }
    }
}
"#, "cannot spawn inside scope block");
}

#[test]
fn scope_spawn_outside_scope_ok() {
    // Normal spawn outside any scope → OK (regression test)
    let output = compile_and_run_stdout(r#"
fn double(x: int) int {
    return x * 2
}

fn main() {
    let t = spawn double(21)
    print(t.get())
}
"#);
    assert_eq!(output.trim(), "42");
}

// === Phase 5d: Scope with app-overridden class ===

#[test]
fn scope_with_app_overridden_class() {
    // End-to-end: override class to scoped in app + use in scope block
    let output = compile_and_run_stdout(r#"
class Database {
    url: string

    fn query(self) string {
        return self.url
    }
}

scoped class RequestCtx {
    id: string
}

scoped class Service[db: Database, ctx: RequestCtx] {
    fn run(self) string {
        return self.db.query()
    }
}

app MyApp {
    scoped Database

    fn main(self) {
        scope(RequestCtx { id: "r1" }, Database { url: "pg://db" }) |svc: Service| {
            print(svc.run())
        }
    }
}
"#);
    assert_eq!(output.trim(), "pg://db");
}

// === Phase 5e: Scope + Closures (Escape Analysis) Tests ===

#[test]
fn scope_closure_local_ok() {
    // Closure captures scope binding, stored locally and called within scope body — OK
    let output = compile_and_run_stdout(r#"
scoped class Ctx {
    value: int
}

fn apply(f: fn() int) int {
    return f()
}

app MyApp {
    fn main(self) {
        scope(Ctx { value: 42 }) |c: Ctx| {
            let f = () => c.value
            print(f())
        }
    }
}
"#);
    assert_eq!(output.trim(), "42");
}

#[test]
fn scope_closure_no_capture_return_ok() {
    // Closure inside scope block that does NOT capture scope bindings can be returned freely
    let output = compile_and_run_stdout(r#"
scoped class Ctx {
    value: int
}

fn make_adder() fn(int) int {
    return (x: int) => x + 1
}

app MyApp {
    fn main(self) {
        scope(Ctx { value: 1 }) |c: Ctx| {
            let f = make_adder()
            print(f(10))
        }
    }
}
"#);
    assert_eq!(output.trim(), "11");
}

#[test]
fn scope_closure_passed_to_fn_ok() {
    // Closure capturing scope binding passed as argument to a function — OK
    let output = compile_and_run_stdout(r#"
scoped class Ctx {
    value: int
}

fn apply(f: fn() int) int {
    return f()
}

app MyApp {
    fn main(self) {
        scope(Ctx { value: 99 }) |c: Ctx| {
            let result = apply(() => c.value)
            print(result)
        }
    }
}
"#);
    assert_eq!(output.trim(), "99");
}

#[test]
fn scope_closure_return_rejected() {
    // Closure capturing scope binding in a return statement — Error
    compile_should_fail_with(r#"
scoped class Ctx {
    value: int
}

fn make_fn() fn() int {
    return () => 1
}

app MyApp {
    fn get_closure(self) fn() int {
        scope(Ctx { value: 1 }) |c: Ctx| {
            return () => c.value
        }
        return () => 0
    }

    fn main(self) {
        let f = self.get_closure()
        print(f())
    }
}
"#, "closure capturing scope binding cannot escape scope block via return");
}

#[test]
fn scope_closure_assign_outer_rejected() {
    // Closure capturing scope binding assigned to a variable declared before the scope block — Error
    compile_should_fail_with(r#"
scoped class Ctx {
    value: int
}

app MyApp {
    fn main(self) {
        let mut f = () => 0
        scope(Ctx { value: 1 }) |c: Ctx| {
            f = () => c.value
        }
        print(f())
    }
}
"#, "closure capturing scope binding cannot escape scope block via assignment to outer variable");
}

#[test]
fn scope_closure_taint_var_return_rejected() {
    // let f = <tainted>; return f — taint flows through local variable — Error
    compile_should_fail_with(r#"
scoped class Ctx {
    value: int
}

app MyApp {
    fn get_closure(self) fn() int {
        scope(Ctx { value: 1 }) |c: Ctx| {
            let f = () => c.value
            return f
        }
        return () => 0
    }

    fn main(self) {
        let f = self.get_closure()
        print(f())
    }
}
"#, "closure capturing scope binding cannot escape scope block via return");
}

#[test]
fn scope_closure_taint_var_assign_outer_rejected() {
    // let f = <tainted>; outer = f — taint flows to outer variable — Error
    compile_should_fail_with(r#"
scoped class Ctx {
    value: int
}

app MyApp {
    fn main(self) {
        let mut f = () => 0
        scope(Ctx { value: 1 }) |c: Ctx| {
            let g = () => c.value
            f = g
        }
        print(f())
    }
}
"#, "closure capturing scope binding cannot escape scope block via assignment to outer variable");
}

// ===== Error message quality tests (Phase 6a) =====

#[test]
fn error_suggests_scoped_keyword_for_non_scoped_seed() {
    compile_should_fail_with(r#"
class Ctx {
    id: int
}

app MyApp {
    fn main(self) {
        scope(Ctx { id: 1 }) |c: Ctx| {
            print(c.id)
        }
    }
}
"#, "add 'scoped' keyword");
}

#[test]
fn error_suggests_seed_expression_for_non_auto_constructible() {
    compile_should_fail_with(r#"
scoped class Ctx {
    id: int
}

scoped class Other {
    tag: string
}

app MyApp {
    fn main(self) {
        scope(Other { tag: "x" }) |c: Ctx| {
            print(c.id)
        }
    }
}
"#, "provide it as a seed expression");
}

#[test]
fn error_shows_cycle_class_names_in_di_graph() {
    // Circular DI dependencies are caught by the global DI graph validation,
    // which includes class names in the error message
    compile_should_fail_with(r#"
scoped class A[b: B] {
    fn run(self) int { return 1 }
}

scoped class B[a: A] {
    fn run(self) int { return 2 }
}

app MyApp {
    fn main(self) {
        print(1)
    }
}
"#, "circular dependency detected:");
}
