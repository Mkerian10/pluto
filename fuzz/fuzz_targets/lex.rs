#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Feed arbitrary bytes to lexer - should never panic
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = plutoc::lexer::lex(s);
    }
});
