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

// Fallible functions (Result<T, E>)
pub fn safe_divide(a: f64, b: f64) -> Result<f64, String> {
    if b == 0.0 { Err("division by zero".to_string()) } else { Ok(a / b) }
}

pub fn checked_negate(x: i64) -> Result<i64, String> {
    if x == i64::MIN { Err("cannot negate i64::MIN".to_string()) } else { Ok(-x) }
}

pub fn validate_positive(x: i64) -> Result<bool, String> {
    if x == 0 { Err("zero is neither positive nor negative".to_string()) } else { Ok(x > 0) }
}

pub fn assert_nonzero(x: i64) -> Result<(), String> {
    if x == 0 { Err("value is zero".to_string()) } else { Ok(()) }
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
