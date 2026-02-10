/// Focused Rust-subset parser that extracts `pub fn` signatures from Rust source files.
/// Only discovers top-level (brace depth 0) `pub fn` with supported types.

#[derive(Debug, Clone, PartialEq)]
pub enum RustType {
    I64,
    F64,
    Bool,
}

#[derive(Debug, Clone)]
pub struct RustFnSig {
    pub name: String,
    pub params: Vec<(String, RustType)>,
    pub return_type: Option<RustType>,
    pub is_fallible: bool,
}

fn parse_rust_type(s: &str) -> Option<RustType> {
    match s.trim() {
        "i64" => Some(RustType::I64),
        "f64" => Some(RustType::F64),
        "bool" => Some(RustType::Bool),
        "()" => Some(RustType::Bool), // map () to None (void) — handled at call site
        _ => None,
    }
}

/// Try to parse a `Result<T, E>` return type.
/// Returns `Some((ok_type, true))` if it's a supported Result type.
/// Returns `None` if it starts with `Result<` but the Ok type is unsupported.
/// The E type is completely ignored.
fn parse_result_type(s: &str) -> Option<(Option<RustType>, bool)> {
    let trimmed = s.trim();
    if !trimmed.starts_with("Result<") || !trimmed.ends_with('>') {
        return None;
    }
    // Extract inner content between Result< and >
    let inner = &trimmed[7..trimmed.len() - 1];
    // Split on ',' at angle-bracket depth 0 to get T and E
    let mut depth = 0;
    let mut split_pos = None;
    for (i, ch) in inner.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                split_pos = Some(i);
                break;
            }
            _ => {}
        }
    }
    let ok_str = match split_pos {
        Some(pos) => inner[..pos].trim(),
        None => return None, // No comma found — malformed Result
    };
    // Parse the Ok type
    if ok_str == "()" {
        Some((None, true)) // Result<(), E> → void, fallible
    } else {
        match parse_rust_type(ok_str) {
            Some(rt) => Some((Some(rt), true)),
            None => None, // Unsupported Ok type
        }
    }
}

/// Parse a Rust source file and extract supported `pub fn` signatures at brace depth 0.
/// Returns (signatures, warnings).
pub fn parse_rust_source(source: &str) -> (Vec<RustFnSig>, Vec<String>) {
    let mut sigs = Vec::new();
    let mut warnings = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut brace_depth: i32 = 0;
    let mut had_cfg_attr = false;

    while i < len {
        // Skip line comments
        if i + 1 < len && chars[i] == '/' && chars[i + 1] == '/' {
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Skip block comments
        if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
            i += 2;
            let mut depth = 1;
            while i + 1 < len && depth > 0 {
                if chars[i] == '/' && chars[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }

        // Skip string literals (double-quoted)
        if chars[i] == '"' {
            i += 1;
            while i < len && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1; // skip escape
                }
                i += 1;
            }
            if i < len {
                i += 1; // skip closing quote
            }
            continue;
        }

        // Skip raw string literals r#"..."#
        if chars[i] == 'r' && i + 1 < len && (chars[i + 1] == '"' || chars[i + 1] == '#') {
            let start = i;
            i += 1;
            let mut hash_count = 0;
            while i < len && chars[i] == '#' {
                hash_count += 1;
                i += 1;
            }
            if i < len && chars[i] == '"' {
                i += 1;
                // Find closing "###
                'raw_loop: while i < len {
                    if chars[i] == '"' {
                        let mut end_hashes = 0;
                        let mut j = i + 1;
                        while j < len && chars[j] == '#' && end_hashes < hash_count {
                            end_hashes += 1;
                            j += 1;
                        }
                        if end_hashes == hash_count {
                            i = j;
                            break 'raw_loop;
                        }
                    }
                    i += 1;
                }
            } else {
                i = start + 1; // not a raw string, backtrack
            }
            continue;
        }

        // Skip char literals
        if chars[i] == '\'' && i + 1 < len && chars[i + 1] != '\'' {
            // Could be a char literal or a lifetime — heuristic: if next char is alpha/_, it's a lifetime
            if i + 1 < len && (chars[i + 1].is_alphabetic() || chars[i + 1] == '_') {
                // Check if it's a lifetime (no closing quote after identifier) or a char literal
                let mut j = i + 1;
                while j < len && (chars[j].is_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                if j < len && chars[j] == '\'' {
                    // char literal like 'a' or 'ab' — skip
                    i = j + 1;
                    continue;
                }
                // Otherwise it's a lifetime annotation — skip just the tick
                i += 1;
                continue;
            }
            // Escape sequences like '\n', '\\'
            if i + 1 < len && chars[i + 1] == '\\' {
                i += 4; // '\X'
                continue;
            }
            i += 1;
            continue;
        }

        // Track brace depth
        if chars[i] == '{' {
            brace_depth += 1;
            i += 1;
            continue;
        }
        if chars[i] == '}' {
            brace_depth -= 1;
            i += 1;
            continue;
        }

        // At depth 0, check for #[cfg(
        if brace_depth == 0 && chars[i] == '#' && i + 1 < len && chars[i + 1] == '[' {
            // Check if this is #[cfg(
            let _attr_start = i;
            i += 2; // skip #[
            // Skip whitespace
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
            let word_start = i;
            while i < len && chars[i].is_alphanumeric() || (i < len && chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[word_start..i].iter().collect();
            if word == "cfg" {
                had_cfg_attr = true;
            }
            // Skip to end of attribute (find matching ])
            let mut bracket_depth = 1;
            while i < len && bracket_depth > 0 {
                if chars[i] == '[' {
                    bracket_depth += 1;
                } else if chars[i] == ']' {
                    bracket_depth -= 1;
                }
                i += 1;
            }
            continue;
        }

        // At depth 0, look for `pub fn` or `pub(` patterns
        if brace_depth == 0 && starts_with_word(&chars, i, "pub") {
            let pub_start = i;
            i += 3;
            // Skip whitespace
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }

            // Check for pub(crate), pub(super), etc. — skip those
            if i < len && chars[i] == '(' {
                // It's pub(...) — skip to closing paren, then skip the item
                had_cfg_attr = false;
                let mut paren_depth = 1;
                i += 1;
                while i < len && paren_depth > 0 {
                    if chars[i] == '(' { paren_depth += 1; }
                    if chars[i] == ')' { paren_depth -= 1; }
                    i += 1;
                }
                continue;
            }

            // Check for `fn` after `pub`
            if starts_with_word(&chars, i, "fn") {
                // Check if preceded by async/unsafe/const
                // Look backwards from pub_start (skipping whitespace) for those keywords
                let before_pub = get_word_before(&chars, pub_start);
                if before_pub == "async" || before_pub == "unsafe" || before_pub == "const" {
                    had_cfg_attr = false;
                    i += 2;
                    continue;
                }

                // Check for #[cfg] attribute
                if had_cfg_attr {
                    had_cfg_attr = false;
                    i += 2;
                    // Skip to end of function (find opening brace and skip it)
                    continue;
                }
                had_cfg_attr = false;

                i += 2; // skip "fn"
                // Skip whitespace
                while i < len && chars[i].is_whitespace() {
                    i += 1;
                }

                // Read function name
                let name_start = i;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let fn_name: String = chars[name_start..i].iter().collect();

                // Skip whitespace
                while i < len && chars[i].is_whitespace() {
                    i += 1;
                }

                // Check for generics — skip generic functions
                if i < len && chars[i] == '<' {
                    continue;
                }

                // Expect '('
                if i >= len || chars[i] != '(' {
                    continue;
                }
                i += 1; // skip (

                // Parse params
                let mut params = Vec::new();
                let mut unsupported = false;
                let mut unsupported_type = String::new();

                // Skip whitespace
                while i < len && chars[i].is_whitespace() {
                    i += 1;
                }

                // Check for empty param list
                if i < len && chars[i] == ')' {
                    i += 1;
                } else {
                    // Parse param list
                    loop {
                        // Skip whitespace
                        while i < len && chars[i].is_whitespace() {
                            i += 1;
                        }

                        if i >= len || chars[i] == ')' {
                            if i < len { i += 1; }
                            break;
                        }

                        // Skip `&self`, `&mut self`, `self`, `mut self` — indicates a method
                        if starts_with_word(&chars, i, "self") || (chars[i] == '&' && {
                            let mut j = i + 1;
                            while j < len && chars[j].is_whitespace() { j += 1; }
                            starts_with_word(&chars, j, "self") || starts_with_word(&chars, j, "mut")
                        }) {
                            unsupported = true;
                            unsupported_type = "self parameter (method)".to_string();
                            break;
                        }

                        // Read param name
                        let pname_start = i;
                        while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                            i += 1;
                        }
                        let pname: String = chars[pname_start..i].iter().collect();

                        // Skip whitespace
                        while i < len && chars[i].is_whitespace() {
                            i += 1;
                        }

                        // Expect ':'
                        if i >= len || chars[i] != ':' {
                            unsupported = true;
                            unsupported_type = "malformed parameter".to_string();
                            break;
                        }
                        i += 1;

                        // Skip whitespace
                        while i < len && chars[i].is_whitespace() {
                            i += 1;
                        }

                        // Read type (everything until ',' or ')')
                        let type_start = i;
                        let mut paren_d = 0;
                        while i < len {
                            if chars[i] == '(' { paren_d += 1; }
                            else if chars[i] == ')' {
                                if paren_d == 0 { break; }
                                paren_d -= 1;
                            }
                            else if chars[i] == ',' && paren_d == 0 { break; }
                            i += 1;
                        }
                        let type_str: String = chars[type_start..i].iter().collect();

                        match parse_rust_type(&type_str) {
                            Some(rt) => {
                                params.push((pname, rt));
                            }
                            None => {
                                unsupported = true;
                                unsupported_type = type_str.trim().to_string();
                                break;
                            }
                        }

                        // Skip comma
                        if i < len && chars[i] == ',' {
                            i += 1;
                        }
                    }
                }

                if unsupported {
                    warnings.push(format!(
                        "skipping function '{}': unsupported type '{}'",
                        fn_name, unsupported_type
                    ));
                    // Skip to end of function body
                    skip_to_brace_close(&chars, &mut i, &mut brace_depth);
                    continue;
                }

                // Skip whitespace
                while i < len && chars[i].is_whitespace() {
                    i += 1;
                }

                // Parse optional return type
                let mut is_fallible = false;
                let return_type = if i + 1 < len && chars[i] == '-' && chars[i + 1] == '>' {
                    i += 2;
                    while i < len && chars[i].is_whitespace() {
                        i += 1;
                    }
                    // Read return type (everything until '{')
                    let ret_start = i;
                    while i < len && chars[i] != '{' {
                        i += 1;
                    }
                    let ret_str: String = chars[ret_start..i].iter().collect();
                    let trimmed = ret_str.trim();
                    // Try Result<T, E> first
                    if trimmed.starts_with("Result<") {
                        match parse_result_type(trimmed) {
                            Some((rt, fallible)) => {
                                is_fallible = fallible;
                                rt
                            }
                            None => {
                                warnings.push(format!(
                                    "skipping function '{}': unsupported Ok type in '{}'",
                                    fn_name, trimmed
                                ));
                                skip_to_brace_close(&chars, &mut i, &mut brace_depth);
                                continue;
                            }
                        }
                    } else if trimmed == "()" {
                        None // void return
                    } else {
                        match parse_rust_type(trimmed) {
                            Some(rt) => Some(rt),
                            None => {
                                warnings.push(format!(
                                    "skipping function '{}': unsupported return type '{}'",
                                    fn_name, trimmed
                                ));
                                skip_to_brace_close(&chars, &mut i, &mut brace_depth);
                                continue;
                            }
                        }
                    }
                } else {
                    None // no return type = void
                };

                sigs.push(RustFnSig {
                    name: fn_name,
                    params,
                    return_type,
                    is_fallible,
                });

                // Skip to end of function body
                skip_to_brace_close(&chars, &mut i, &mut brace_depth);
                continue;
            } else {
                // Not `pub fn` — could be `pub struct`, `pub enum`, etc.
                had_cfg_attr = false;
            }
        } else if brace_depth == 0 && !chars[i].is_whitespace() && chars[i] != '\n' {
            // Non-whitespace at depth 0 that isn't part of `pub` — reset cfg tracking
            // (but only for keyword-like tokens, not punctuation)
            if chars[i].is_alphabetic() && !starts_with_word(&chars, i, "pub") {
                had_cfg_attr = false;
            }
        }

        i += 1;
    }

    (sigs, warnings)
}

/// Check if chars starting at pos match a word (followed by non-alphanumeric or end).
fn starts_with_word(chars: &[char], pos: usize, word: &str) -> bool {
    let wchars: Vec<char> = word.chars().collect();
    if pos + wchars.len() > chars.len() {
        return false;
    }
    for (j, wc) in wchars.iter().enumerate() {
        if chars[pos + j] != *wc {
            return false;
        }
    }
    // Ensure it's a word boundary
    let after = pos + wchars.len();
    if after < chars.len() && (chars[after].is_alphanumeric() || chars[after] == '_') {
        return false;
    }
    true
}

/// Get the last word before a position (looking backwards, skipping whitespace).
fn get_word_before(chars: &[char], pos: usize) -> String {
    let mut i = pos;
    // Skip whitespace backwards
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    // Read word backwards
    let end = i;
    while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
        i -= 1;
    }
    chars[i..end].iter().collect()
}

/// Skip from current position to end of function body (find opening brace, track depth).
fn skip_to_brace_close(chars: &[char], i: &mut usize, brace_depth: &mut i32) {
    let len = chars.len();
    // Find opening brace
    while *i < len && chars[*i] != '{' {
        *i += 1;
    }
    if *i < len {
        *brace_depth += 1;
        *i += 1;
        // Skip to matching close brace
        while *i < len && *brace_depth > 0 {
            if chars[*i] == '{' {
                *brace_depth += 1;
            } else if chars[*i] == '}' {
                *brace_depth -= 1;
            } else if chars[*i] == '"' {
                // Skip string literal inside function body
                *i += 1;
                while *i < len && chars[*i] != '"' {
                    if chars[*i] == '\\' {
                        *i += 1;
                    }
                    *i += 1;
                }
            } else if *i + 1 < len && chars[*i] == '/' && chars[*i + 1] == '/' {
                // Skip line comment
                while *i < len && chars[*i] != '\n' {
                    *i += 1;
                }
            } else if *i + 1 < len && chars[*i] == '/' && chars[*i + 1] == '*' {
                // Skip block comment
                *i += 2;
                while *i + 1 < len && !(chars[*i] == '*' && chars[*i + 1] == '/') {
                    *i += 1;
                }
                if *i + 1 < len {
                    *i += 2;
                    continue;
                }
            }
            *i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_add() {
        let source = "pub fn add(a: i64, b: i64) -> i64 { a + b }";
        let (sigs, warns) = parse_rust_source(source);
        assert!(warns.is_empty());
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].name, "add");
        assert_eq!(sigs[0].params.len(), 2);
        assert_eq!(sigs[0].params[0], ("a".to_string(), RustType::I64));
        assert_eq!(sigs[0].params[1], ("b".to_string(), RustType::I64));
        assert_eq!(sigs[0].return_type, Some(RustType::I64));
    }

    #[test]
    fn no_return_type() {
        let source = "pub fn noop() { }";
        let (sigs, warns) = parse_rust_source(source);
        assert!(warns.is_empty());
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].name, "noop");
        assert!(sigs[0].params.is_empty());
        assert_eq!(sigs[0].return_type, None);
    }

    #[test]
    fn bool_params_and_return() {
        let source = "pub fn is_even(n: i64) -> bool { n % 2 == 0 }";
        let (sigs, warns) = parse_rust_source(source);
        assert!(warns.is_empty());
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].return_type, Some(RustType::Bool));
    }

    #[test]
    fn float_types() {
        let source = "pub fn add_f(a: f64, b: f64) -> f64 { a + b }";
        let (sigs, warns) = parse_rust_source(source);
        assert!(warns.is_empty());
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].params[0].1, RustType::F64);
        assert_eq!(sigs[0].return_type, Some(RustType::F64));
    }

    #[test]
    fn multiline_signature() {
        let source = r#"
pub fn multi(
    a: i64,
    b: f64,
) -> bool {
    true
}
"#;
        let (sigs, warns) = parse_rust_source(source);
        assert!(warns.is_empty());
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].name, "multi");
        assert_eq!(sigs[0].params.len(), 2);
    }

    #[test]
    fn skip_private_fn() {
        let source = "fn private(a: i64) -> i64 { a }";
        let (sigs, _) = parse_rust_source(source);
        assert!(sigs.is_empty());
    }

    #[test]
    fn skip_pub_crate_fn() {
        let source = "pub(crate) fn internal(a: i64) -> i64 { a }";
        let (sigs, _) = parse_rust_source(source);
        assert!(sigs.is_empty());
    }

    #[test]
    fn skip_generic_fn() {
        let source = "pub fn identity<T>(x: T) -> T { x }";
        let (sigs, _) = parse_rust_source(source);
        assert!(sigs.is_empty());
    }

    #[test]
    fn skip_async_fn() {
        let source = "async pub fn fetch() { }
pub fn sync_fn(x: i64) -> i64 { x }";
        let (sigs, _) = parse_rust_source(source);
        // sync_fn should be found, async fn should be skipped
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].name, "sync_fn");
    }

    #[test]
    fn skip_unsupported_types() {
        let source = r#"
pub fn greet(name: &str) -> String { format!("hi {name}") }
pub fn good(x: i64) -> i64 { x }
"#;
        let (sigs, warns) = parse_rust_source(source);
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].name, "good");
        assert_eq!(warns.len(), 1);
        assert!(warns[0].contains("greet"));
    }

    #[test]
    fn skip_methods_inside_impl() {
        let source = r#"
impl Foo {
    pub fn bar(self, x: i64) -> i64 { x }
}
pub fn free_fn(x: i64) -> i64 { x }
"#;
        let (sigs, _warns) = parse_rust_source(source);
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].name, "free_fn");
    }

    #[test]
    fn skip_cfg_gated_fn() {
        let source = r#"
#[cfg(feature = "nope")]
pub fn cfg_gated(x: i64) -> i64 { x }

pub fn normal(x: i64) -> i64 { x }
"#;
        let (sigs, _) = parse_rust_source(source);
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].name, "normal");
    }

    #[test]
    fn non_cfg_attr_not_skipped() {
        let source = r#"
#[inline]
pub fn inlined(x: i64) -> i64 { x }
"#;
        let (sigs, _) = parse_rust_source(source);
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].name, "inlined");
    }

    #[test]
    fn multiple_functions() {
        let source = r#"
pub fn add(a: i64, b: i64) -> i64 { a + b }
pub fn mul(a: f64, b: f64) -> f64 { a * b }
pub fn negate(x: f64) -> f64 { -x }
pub fn is_positive(x: i64) -> bool { x > 0 }
pub fn bad(data: Vec<u8>) -> Vec<u8> { data }
"#;
        let (sigs, warns) = parse_rust_source(source);
        assert_eq!(sigs.len(), 4);
        assert_eq!(warns.len(), 1);
        assert!(warns[0].contains("bad"));
    }

    #[test]
    fn void_return_explicit() {
        let source = "pub fn do_thing() -> () { }";
        let (sigs, warns) = parse_rust_source(source);
        assert!(warns.is_empty());
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].return_type, None); // () maps to void
    }

    #[test]
    fn handles_comments() {
        let source = r#"
// This is a comment
pub fn add(a: i64, b: i64) -> i64 {
    // inner comment
    a + b
}
/* block comment
pub fn hidden(x: i64) -> i64 { x }
*/
pub fn visible(x: i64) -> i64 { x }
"#;
        let (sigs, _) = parse_rust_source(source);
        assert_eq!(sigs.len(), 2);
        assert_eq!(sigs[0].name, "add");
        assert_eq!(sigs[1].name, "visible");
    }

    #[test]
    fn handles_string_with_braces() {
        let source = r#"
pub fn stringy() -> i64 {
    let s = "{ } { }";
    42
}
"#;
        let (sigs, _) = parse_rust_source(source);
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].name, "stringy");
    }

    // ── Result<T, E> tests ──────────────────────────────────────

    #[test]
    fn result_i64() {
        let source = r#"pub fn checked(x: i64) -> Result<i64, String> { Ok(x) }"#;
        let (sigs, warns) = parse_rust_source(source);
        assert!(warns.is_empty());
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].name, "checked");
        assert_eq!(sigs[0].return_type, Some(RustType::I64));
        assert!(sigs[0].is_fallible);
    }

    #[test]
    fn result_f64() {
        let source = r#"pub fn divide(a: f64, b: f64) -> Result<f64, Box<dyn Error>> { Ok(a / b) }"#;
        let (sigs, warns) = parse_rust_source(source);
        assert!(warns.is_empty());
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].return_type, Some(RustType::F64));
        assert!(sigs[0].is_fallible);
    }

    #[test]
    fn result_bool() {
        let source = r#"pub fn validate(x: i64) -> Result<bool, MyError> { Ok(true) }"#;
        let (sigs, warns) = parse_rust_source(source);
        assert!(warns.is_empty());
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].return_type, Some(RustType::Bool));
        assert!(sigs[0].is_fallible);
    }

    #[test]
    fn result_void() {
        let source = r#"pub fn check(x: i64) -> Result<(), String> { Ok(()) }"#;
        let (sigs, warns) = parse_rust_source(source);
        assert!(warns.is_empty());
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].return_type, None);
        assert!(sigs[0].is_fallible);
    }

    #[test]
    fn result_unsupported_ok() {
        let source = r#"pub fn bad(x: i64) -> Result<Vec<i64>, String> { Ok(vec![x]) }"#;
        let (sigs, warns) = parse_rust_source(source);
        assert_eq!(sigs.len(), 0);
        assert_eq!(warns.len(), 1);
        assert!(warns[0].contains("unsupported Ok type"));
    }

    #[test]
    fn result_nested_generics() {
        let source = r#"pub fn checked(x: i64) -> Result<i64, Box<dyn std::error::Error>> { Ok(x) }"#;
        let (sigs, warns) = parse_rust_source(source);
        assert!(warns.is_empty());
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].return_type, Some(RustType::I64));
        assert!(sigs[0].is_fallible);
    }

    #[test]
    fn plain_types_not_fallible() {
        let source = "pub fn add(a: i64, b: i64) -> i64 { a + b }";
        let (sigs, _) = parse_rust_source(source);
        assert_eq!(sigs.len(), 1);
        assert!(!sigs[0].is_fallible);
    }

    #[test]
    fn void_fn_not_fallible() {
        let source = "pub fn noop() { }";
        let (sigs, _) = parse_rust_source(source);
        assert_eq!(sigs.len(), 1);
        assert!(!sigs[0].is_fallible);
    }
}
