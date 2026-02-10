pub fn add_i64(a: i64, b: i64) -> i64 {
    a + b
}

pub fn multiply_f64(a: f64, b: f64) -> f64 {
    a * b
}

pub fn is_positive(x: i64) -> bool {
    x > 0
}

pub fn negate(x: f64) -> f64 {
    -x
}

pub fn do_nothing() {
}

pub fn will_panic() {
    panic!("test panic");
}

// Should be skipped — unsupported types
pub fn greet(name: &str) -> String {
    format!("hi {name}")
}

// Should be skipped — cfg-gated
#[cfg(feature = "nope")]
pub fn cfg_gated(x: i64) -> i64 {
    x
}
