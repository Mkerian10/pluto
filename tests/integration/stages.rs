mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

#[test]
fn stage_basic() {
    let output = compile_and_run_stdout(
        "stage Main {
    fn main(self) {
        print(\"hello from stage\")
    }
}",
    );
    assert_eq!(output.trim(), "hello from stage");
}

#[test]
fn stage_with_di() {
    let output = compile_and_run_stdout(
        "class Database {
    fn query(self, q: string) string {
        return q
    }
}

stage Api[db: Database] {
    fn main(self) {
        print(self.db.query(\"42\"))
    }
}",
    );
    assert_eq!(output.trim(), "42");
}

#[test]
fn stage_with_di_chain() {
    let output = compile_and_run_stdout(
        "class Config {
    fn get(self) string {
        return \"prod\"
    }
}

class Database[config: Config] {
    fn env(self) string {
        return self.config.get()
    }
}

stage Api[db: Database] {
    fn main(self) {
        print(self.db.env())
    }
}",
    );
    assert_eq!(output.trim(), "prod");
}

#[test]
fn stage_pub_and_private_methods() {
    let output = compile_and_run_stdout(
        "stage Api {
    fn helper(self) string {
        return \"internal\"
    }

    pub fn greet(self) string {
        return self.helper()
    }

    fn main(self) {
        print(self.greet())
    }
}",
    );
    assert_eq!(output.trim(), "internal");
}

#[test]
fn stage_without_main_rejected() {
    compile_should_fail_with(
        "stage Api {
    fn greet(self) string {
        return \"hi\"
    }
}",
        "stage must have a 'main' method",
    );
}

#[test]
fn stage_plus_app_rejected() {
    compile_should_fail_with(
        "app MyApp {
    fn main(self) {
    }
}

stage Api {
    fn main(self) {
    }
}",
        "cannot contain both 'stage' and 'app'",
    );
}

#[test]
fn stage_plus_top_level_main_rejected() {
    compile_should_fail_with(
        "fn main() {
}

stage Api {
    fn main(self) {
    }
}",
        "cannot have both a stage declaration and a top-level main",
    );
}

#[test]
fn stage_with_closures() {
    let output = compile_and_run_stdout(
        "stage Api {
    fn main(self) {
        let f = (x: int) => x * 2
        print(f(21))
    }
}",
    );
    assert_eq!(output.trim(), "42");
}

#[test]
fn stage_with_error_handling() {
    let output = compile_and_run_stdout(
        "error NotFound {
    code: int
}

fn find(id: int) int {
    if id == 0 {
        raise NotFound { code: 404 }
    }
    return id
}

stage Api {
    fn main(self) {
        let result = find(42) catch -1
        print(result)
    }
}",
    );
    assert_eq!(output.trim(), "42");
}

#[test]
fn stage_with_spawn() {
    let output = compile_and_run_stdout(
        "fn compute(x: int) int {
    return x * 2
}

stage Api {
    fn main(self) {
        let t = spawn compute(21)
        print(t.get())
    }
}",
    );
    assert_eq!(output.trim(), "42");
}

#[test]
fn stage_with_contracts() {
    let output = compile_and_run_stdout(
        "class Counter {
    count: int
    invariant self.count >= 0

    fn increment(mut self) {
        self.count = self.count + 1
    }

    fn value(self) int {
        return self.count
    }
}

stage Api {
    fn main(self) {
        let mut c = Counter { count: 0 }
        c.increment()
        c.increment()
        print(c.value())
    }
}",
    );
    assert_eq!(output.trim(), "2");
}

// ── Stage Inheritance Tests ─────────────────────────────────────────

#[test]
fn stage_basic_inheritance() {
    let output = compile_and_run_stdout(
        "stage Daemon {
    requires fn run(self)

    fn main(self) {
        self.run()
    }
}

stage Worker : Daemon {
    override fn run(self) {
        print(\"working\")
    }
}",
    );
    assert_eq!(output.trim(), "working");
}

#[test]
fn stage_three_level_chain() {
    let output = compile_and_run_stdout(
        "stage Base {
    requires fn execute(self)

    fn main(self) {
        self.execute()
    }
}

stage Middle : Base {
    requires fn task(self) string

    override fn execute(self) {
        print(self.task())
    }
}

stage App : Middle {
    override fn task(self) string {
        return \"done\"
    }
}",
    );
    assert_eq!(output.trim(), "done");
}

#[test]
fn stage_template_method_pattern() {
    let output = compile_and_run_stdout(
        "stage Lifecycle {
    requires fn start(self)
    requires fn stop(self)

    fn main(self) {
        self.start()
        print(\"running\")
        self.stop()
    }
}

stage Server : Lifecycle {
    override fn start(self) {
        print(\"started\")
    }

    override fn stop(self) {
        print(\"stopped\")
    }
}",
    );
    assert_eq!(output.trim(), "started\nrunning\nstopped");
}

#[test]
fn stage_inherit_concrete_method() {
    let output = compile_and_run_stdout(
        "stage Base {
    requires fn name(self) string

    fn greet(self) {
        print(\"hello \" + self.name())
    }

    fn main(self) {
        self.greet()
    }
}

stage Child : Base {
    override fn name(self) string {
        return \"world\"
    }
}",
    );
    assert_eq!(output.trim(), "hello world");
}

#[test]
fn stage_di_merging_across_levels() {
    let output = compile_and_run_stdout(
        "class Logger {
    fn log(self, msg: string) {
        print(msg)
    }
}

class Database {
    fn query(self) string {
        return \"data\"
    }
}

stage Base [logger: Logger] {
    requires fn run(self)

    fn main(self) {
        self.logger.log(\"starting\")
        self.run()
    }
}

stage Api : Base [db: Database] {
    override fn run(self) {
        self.logger.log(self.db.query())
    }
}",
    );
    assert_eq!(output.trim(), "starting\ndata");
}

#[test]
fn stage_missing_requires_rejected() {
    compile_should_fail_with(
        "stage Daemon {
    requires fn run(self)

    fn main(self) {
        self.run()
    }
}

stage Worker : Daemon {
    fn helper(self) {
        print(\"help\")
    }
}",
        "unimplemented required methods: run",
    );
}

#[test]
fn stage_spurious_override_rejected() {
    compile_should_fail_with(
        "stage Base {
    fn main(self) {
        print(\"hi\")
    }
}

stage Child : Base {
    override fn nonexistent(self) {
        print(\"nope\")
    }
}",
        "does not override any method from parent",
    );
}

#[test]
fn stage_missing_override_keyword_rejected() {
    compile_should_fail_with(
        "stage Base {
    requires fn run(self)

    fn main(self) {
        self.run()
    }
}

stage Child : Base {
    fn run(self) {
        print(\"shadowing\")
    }
}",
        "shadows a parent method",
    );
}

#[test]
fn stage_parent_not_found_rejected() {
    compile_should_fail_with(
        "stage Child : NonExistent {
    fn main(self) {
        print(\"hi\")
    }
}",
        "inherits from unknown stage",
    );
}

#[test]
fn stage_circular_inheritance_rejected() {
    compile_should_fail_with(
        "stage A : B {
    fn main(self) {}
}

stage B : A {
    fn main(self) {}
}",
        "circular stage inheritance",
    );
}

#[test]
fn stage_override_satisfies_requires() {
    let output = compile_and_run_stdout(
        "stage Base {
    requires fn value(self) int

    fn main(self) {
        print(self.value())
    }
}

stage Impl : Base {
    override fn value(self) int {
        return 42
    }
}",
    );
    assert_eq!(output.trim(), "42");
}
