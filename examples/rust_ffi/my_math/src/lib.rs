pub fn fibonacci(n: i64) -> i64 {
    if n <= 1 {
        n
    } else {
        fibonacci(n - 1) + fibonacci(n - 2)
    }
}

pub fn add(a: f64, b: f64) -> f64 {
    a + b
}

pub fn is_even(n: i64) -> bool {
    n % 2 == 0
}

pub fn factorial(n: i64) -> i64 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}
