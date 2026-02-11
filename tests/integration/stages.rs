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
