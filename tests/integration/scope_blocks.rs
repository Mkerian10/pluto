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
