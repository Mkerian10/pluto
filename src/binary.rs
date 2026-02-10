//! Binary container format for serialized Pluto ASTs.
//!
//! Container layout (16-byte header + two length-prefixed sections):
//!
//! ```text
//! [4B magic "PLTO"] [4B schema version u32 LE] [4B source offset u32 LE] [4B AST offset u32 LE]
//! [Source section: 4B length u32 LE + UTF-8 bytes]
//! [AST section: 4B length u32 LE + bincode bytes]
//! ```

use crate::parser::ast::Program;

/// Magic bytes identifying a binary Pluto file.
const MAGIC: &[u8; 4] = b"PLTO";

/// Current schema version.
const SCHEMA_VERSION: u32 = 1;

/// Header size in bytes: magic (4) + version (4) + source_offset (4) + ast_offset (4).
const HEADER_SIZE: usize = 16;

/// Errors that can occur during binary serialization/deserialization.
#[derive(Debug, thiserror::Error)]
pub enum BinaryError {
    #[error("invalid magic number: expected PLTO")]
    InvalidMagic,
    #[error("unsupported schema version {0} (expected {SCHEMA_VERSION})")]
    UnsupportedVersion(u32),
    #[error("truncated file: expected at least {expected} bytes, got {got}")]
    Truncated { expected: usize, got: usize },
    #[error("bincode encode error: {0}")]
    Encode(String),
    #[error("bincode decode error: {0}")]
    Decode(String),
    #[error("invalid UTF-8 in source section: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
}

/// Serialize a parsed `Program` and its source text into the binary container format.
pub fn serialize_program(program: &Program, source: &str) -> Result<Vec<u8>, BinaryError> {
    let config = bincode::config::standard();
    let ast_bytes = bincode::serde::encode_to_vec(program, config)
        .map_err(|e| BinaryError::Encode(e.to_string()))?;

    let source_bytes = source.as_bytes();
    let source_section_size = 4 + source_bytes.len(); // length prefix + data
    let ast_section_size = 4 + ast_bytes.len();

    let source_offset = HEADER_SIZE as u32;
    let ast_offset = (HEADER_SIZE + source_section_size) as u32;

    let total_size = HEADER_SIZE + source_section_size + ast_section_size;
    let mut buf = Vec::with_capacity(total_size);

    // Header
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
    buf.extend_from_slice(&source_offset.to_le_bytes());
    buf.extend_from_slice(&ast_offset.to_le_bytes());

    // Source section
    buf.extend_from_slice(&(source_bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(source_bytes);

    // AST section
    buf.extend_from_slice(&(ast_bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(&ast_bytes);

    Ok(buf)
}

/// Deserialize a binary container back into a `Program` and its source text.
pub fn deserialize_program(data: &[u8]) -> Result<(Program, String), BinaryError> {
    validate_header(data)?;

    let source = read_source_section(data)?;
    let program = read_ast_section(data)?;

    Ok((program, source))
}

/// Check whether a byte slice starts with the Pluto binary magic number.
pub fn is_binary_format(data: &[u8]) -> bool {
    data.len() >= 4 && &data[..4] == MAGIC
}

/// Read only the source text from a binary container, without deserializing the AST.
pub fn read_source_only(data: &[u8]) -> Result<String, BinaryError> {
    validate_header(data)?;
    read_source_section(data)
}

// --- internal helpers ---

fn validate_header(data: &[u8]) -> Result<(), BinaryError> {
    if data.len() < HEADER_SIZE {
        return Err(BinaryError::Truncated {
            expected: HEADER_SIZE,
            got: data.len(),
        });
    }
    if &data[..4] != MAGIC {
        return Err(BinaryError::InvalidMagic);
    }
    let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
    if version != SCHEMA_VERSION {
        return Err(BinaryError::UnsupportedVersion(version));
    }
    Ok(())
}

fn read_source_section(data: &[u8]) -> Result<String, BinaryError> {
    let source_offset = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;

    if data.len() < source_offset + 4 {
        return Err(BinaryError::Truncated {
            expected: source_offset + 4,
            got: data.len(),
        });
    }

    let source_len =
        u32::from_le_bytes(data[source_offset..source_offset + 4].try_into().unwrap()) as usize;

    let source_end = source_offset + 4 + source_len;
    if data.len() < source_end {
        return Err(BinaryError::Truncated {
            expected: source_end,
            got: data.len(),
        });
    }

    let source = String::from_utf8(data[source_offset + 4..source_end].to_vec())?;
    Ok(source)
}

fn read_ast_section(data: &[u8]) -> Result<Program, BinaryError> {
    let ast_offset = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;

    if data.len() < ast_offset + 4 {
        return Err(BinaryError::Truncated {
            expected: ast_offset + 4,
            got: data.len(),
        });
    }

    let ast_len =
        u32::from_le_bytes(data[ast_offset..ast_offset + 4].try_into().unwrap()) as usize;

    let ast_end = ast_offset + 4 + ast_len;
    if data.len() < ast_end {
        return Err(BinaryError::Truncated {
            expected: ast_end,
            got: data.len(),
        });
    }

    let config = bincode::config::standard();
    let (program, _bytes_read): (Program, usize) =
        bincode::serde::decode_from_slice(&data[ast_offset + 4..ast_end], config)
            .map_err(|e| BinaryError::Decode(e.to_string()))?;

    Ok(program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser::Parser;

    /// Parse a source string into a Program (no typeck/codegen, just lex+parse).
    fn parse(source: &str) -> Program {
        let tokens = lexer::lex(source).expect("lex failed");
        let mut parser = Parser::new(&tokens, source);
        parser.parse_program().expect("parse failed")
    }

    #[test]
    fn round_trip_empty_program() {
        let source = "fn main() {\n}\n";
        let program = parse(source);
        let bytes = serialize_program(&program, source).unwrap();
        let (program2, source2) = deserialize_program(&bytes).unwrap();
        assert_eq!(source, source2);
        assert_eq!(program.functions.len(), program2.functions.len());
        assert_eq!(
            program.functions[0].node.name.node,
            program2.functions[0].node.name.node
        );
    }

    #[test]
    fn round_trip_with_expressions() {
        let source = r#"fn main() {
    let x = 1 + 2
    let y = add(x, 3)
}

fn add(a: int, b: int) int {
    return a + b
}
"#;
        let program = parse(source);
        let bytes = serialize_program(&program, source).unwrap();
        let (program2, source2) = deserialize_program(&bytes).unwrap();
        assert_eq!(source, source2);
        assert_eq!(program.functions.len(), program2.functions.len());
        // Re-serialize and check bytes match
        let bytes2 = serialize_program(&program2, &source2).unwrap();
        assert_eq!(bytes, bytes2);
    }

    #[test]
    fn round_trip_class_and_enum() {
        let source = r#"class Point {
    x: int
    y: int
}

enum Color {
    Red
    Green
    Blue
}

fn main() {
    let p = Point { x: 1, y: 2 }
    let c = Color.Red
}
"#;
        let program = parse(source);
        let bytes = serialize_program(&program, source).unwrap();
        let (program2, source2) = deserialize_program(&bytes).unwrap();
        assert_eq!(source, source2);
        assert_eq!(program.classes.len(), program2.classes.len());
        assert_eq!(program.enums.len(), program2.enums.len());
        let bytes2 = serialize_program(&program2, &source2).unwrap();
        assert_eq!(bytes, bytes2);
    }

    #[test]
    fn round_trip_closures_and_errors() {
        let source = r#"error NotFound {
    message: string
}

fn find(name: string) int {
    raise NotFound { message: "not found" }
}

fn main() {
    let f = (x: int) => x + 1
    let result = find("foo") catch 0
}
"#;
        let program = parse(source);
        let bytes = serialize_program(&program, source).unwrap();
        let (program2, source2) = deserialize_program(&bytes).unwrap();
        assert_eq!(source, source2);
        assert_eq!(program.errors.len(), program2.errors.len());
        let bytes2 = serialize_program(&program2, &source2).unwrap();
        assert_eq!(bytes, bytes2);
    }

    #[test]
    fn round_trip_all_expr_types() {
        let source = r#"fn main() {
    let m = Map<string, int> { "a": 1, "b": 2 }
    let s = Set<int> { 1, 2, 3 }
    let r = 0..10
    let msg = "hello {1 + 2}"
    let x = 42 as float
    let y = ~5
}
"#;
        let program = parse(source);
        let bytes = serialize_program(&program, source).unwrap();
        let (program2, source2) = deserialize_program(&bytes).unwrap();
        assert_eq!(source, source2);
        let bytes2 = serialize_program(&program2, &source2).unwrap();
        assert_eq!(bytes, bytes2);
    }

    #[test]
    fn round_trip_preserves_uuids() {
        let source = r#"class Foo {
    x: int
}

fn bar() {
}

fn main() {
}
"#;
        let program = parse(source);
        let class_id = program.classes[0].node.id;
        let fn_id = program.functions[0].node.id;

        let bytes = serialize_program(&program, source).unwrap();
        let (program2, _) = deserialize_program(&bytes).unwrap();

        assert_eq!(class_id, program2.classes[0].node.id);
        assert_eq!(fn_id, program2.functions[0].node.id);
    }

    #[test]
    fn magic_number_detection() {
        let source = "fn main() {\n}\n";
        let program = parse(source);
        let bytes = serialize_program(&program, source).unwrap();
        assert!(is_binary_format(&bytes));

        // Plain text is not binary
        assert!(!is_binary_format(source.as_bytes()));

        // Empty slice is not binary
        assert!(!is_binary_format(&[]));
    }

    #[test]
    fn invalid_magic_rejected() {
        let data = b"NOPExxxxxxxxxxxxmore data here";
        let result = deserialize_program(data);
        assert!(matches!(result, Err(BinaryError::InvalidMagic)));
    }

    #[test]
    fn wrong_version_rejected() {
        let mut data = vec![0u8; 32];
        data[..4].copy_from_slice(MAGIC);
        data[4..8].copy_from_slice(&99u32.to_le_bytes()); // future version
        let result = deserialize_program(&data);
        assert!(matches!(result, Err(BinaryError::UnsupportedVersion(99))));
    }

    #[test]
    fn source_only_read() {
        let source = "fn main() {\n    let x = 42\n}\n";
        let program = parse(source);
        let bytes = serialize_program(&program, source).unwrap();
        let extracted = read_source_only(&bytes).unwrap();
        assert_eq!(source, extracted);
    }

    #[test]
    fn truncated_file_rejected() {
        // Less than header size
        let data = b"PLT";
        let result = deserialize_program(data);
        assert!(matches!(
            result,
            Err(BinaryError::Truncated {
                expected: 16,
                got: 3
            })
        ));

        // Valid header but truncated source section
        let mut data = vec![0u8; 16];
        data[..4].copy_from_slice(MAGIC);
        data[4..8].copy_from_slice(&SCHEMA_VERSION.to_le_bytes());
        data[8..12].copy_from_slice(&16u32.to_le_bytes()); // source at offset 16
        data[12..16].copy_from_slice(&20u32.to_le_bytes()); // ast at offset 20
        let result = deserialize_program(&data);
        assert!(matches!(result, Err(BinaryError::Truncated { .. })));
    }
}
