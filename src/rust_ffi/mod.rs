pub mod parser;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::{Span, Spanned};
use crate::typeck::types::PlutoType;

use parser::{RustFnSig, RustType};

pub struct RustCrateArtifact {
    pub static_lib: PathBuf,
    pub native_libs: Vec<String>,
    pub functions: Vec<BridgedFunction>,
}

pub struct BridgedFunction {
    pub pluto_name: String,
    pub symbol_name: String,
    pub params: Vec<PlutoType>,
    pub return_type: PlutoType,
    pub is_fallible: bool,
}

struct CrateMetadata {
    lib_name: String,
    lib_source_path: PathBuf,
    manifest_dir: PathBuf,
}

struct CrateInfo {
    alias: String,
    metadata: CrateMetadata,
    functions: Vec<RustFnSig>,
}

/// Top-level entry: process all `extern rust` declarations.
/// Checks for alias collisions and builds the combined glue crate.
pub fn resolve_rust_crates(
    program: &Program,
    entry_dir: &Path,
) -> Result<Vec<RustCrateArtifact>, CompileError> {
    if program.extern_rust_crates.is_empty() {
        return Ok(vec![]);
    }

    // Check for duplicate aliases
    let mut seen_aliases: HashMap<String, Span> = HashMap::new();
    for ext_rust in &program.extern_rust_crates {
        let alias = &ext_rust.node.alias.node;
        if let Some(prev_span) = seen_aliases.get(alias) {
            let _ = prev_span;
            return Err(CompileError::syntax(
                format!("duplicate extern rust alias '{}'", alias),
                ext_rust.node.alias.span,
            ));
        }
        seen_aliases.insert(alias.clone(), ext_rust.span);
    }

    // Discover all crates and extract signatures
    let mut crate_infos = Vec::new();

    for ext_rust in &program.extern_rust_crates {
        let crate_path_str = &ext_rust.node.crate_path.node;
        let alias = &ext_rust.node.alias.node;

        // Resolve path relative to entry directory
        let crate_path = entry_dir.join(crate_path_str);
        let crate_path = crate_path.canonicalize().map_err(|e| {
            CompileError::codegen(format!(
                "extern rust '{}': cannot resolve crate path '{}': {}",
                alias, crate_path_str, e
            ))
        })?;

        // Run cargo metadata
        let metadata = run_cargo_metadata(&crate_path, alias)?;

        // Parse Rust source to extract function signatures
        let source = std::fs::read_to_string(&metadata.lib_source_path).map_err(|e| {
            CompileError::codegen(format!(
                "extern rust '{}': cannot read lib source '{}': {}",
                alias,
                metadata.lib_source_path.display(),
                e
            ))
        })?;

        let (sigs, warnings) = parser::parse_rust_source(&source);

        // Print warnings for skipped functions
        for warn in &warnings {
            eprintln!("warning: extern rust '{}': {}", alias, warn);
        }

        if sigs.is_empty() {
            eprintln!(
                "warning: extern rust '{}': no supported functions found in '{}'",
                alias,
                metadata.lib_source_path.display()
            );
        }

        crate_infos.push(CrateInfo {
            alias: alias.clone(),
            metadata,
            functions: sigs,
        });
    }

    // Check for lib_name collisions
    let mut lib_name_to_alias: HashMap<String, String> = HashMap::new();
    for info in &crate_infos {
        let lib_name = &info.metadata.lib_name;
        if let Some(prev_alias) = lib_name_to_alias.get(lib_name) {
            return Err(CompileError::codegen(format!(
                "extern rust crates '{}' and '{}' have the same lib target name '{}'; this is not supported",
                prev_alias, info.alias, lib_name
            )));
        }
        lib_name_to_alias.insert(lib_name.clone(), info.alias.clone());
    }

    // Determine glue crate directory
    let entry_abs = entry_dir.canonicalize().unwrap_or_else(|_| entry_dir.to_path_buf());
    let hash = simple_hash(&format!("{}", entry_abs.display()));
    let glue_dir = std::env::temp_dir().join(format!("pluto_ffi_glue_{:x}", hash));
    std::fs::create_dir_all(&glue_dir).map_err(|e| {
        CompileError::codegen(format!("failed to create glue crate directory: {}", e))
    })?;

    // Generate the glue crate
    generate_glue_crate(&crate_infos, &glue_dir)?;

    // Build the glue crate
    let (static_lib, native_libs) = build_glue_crate(&glue_dir)?;

    // Create artifacts (one per crate, but all share the same static lib)
    let mut artifacts = Vec::new();
    for info in &crate_infos {
        let functions: Vec<BridgedFunction> = info
            .functions
            .iter()
            .map(|sig| {
                let pluto_name = format!("{}.{}", info.alias, sig.name);
                let params: Vec<PlutoType> = sig
                    .params
                    .iter()
                    .map(|(_, rt)| rust_type_to_pluto(rt))
                    .collect();
                let return_type = sig
                    .return_type
                    .as_ref()
                    .map(|rt| rust_type_to_pluto(rt))
                    .unwrap_or(PlutoType::Void);
                BridgedFunction {
                    pluto_name: pluto_name.clone(),
                    symbol_name: pluto_name,
                    params,
                    return_type,
                    is_fallible: sig.is_fallible,
                }
            })
            .collect();

        artifacts.push(RustCrateArtifact {
            static_lib: static_lib.clone(),
            native_libs: native_libs.clone(),
            functions,
        });
    }

    Ok(artifacts)
}

/// Inject bridged functions into the program as ExternFnDecl entries.
/// Also populates `program.fallible_extern_fns` for functions returning `Result`.
pub fn inject_extern_fns(program: &mut Program, artifacts: &[RustCrateArtifact]) {
    for artifact in artifacts {
        for func in &artifact.functions {
            let params: Vec<Param> = func
                .params
                .iter()
                .enumerate()
                .map(|(i, ty)| Param {
                    id: Uuid::new_v4(),
                    name: Spanned::new(format!("p{}", i), Span::dummy()),
                    ty: Spanned::new(pluto_type_to_type_expr(ty), Span::dummy()),
                })
                .collect();

            let return_type = if func.return_type != PlutoType::Void {
                Some(Spanned::new(
                    pluto_type_to_type_expr(&func.return_type),
                    Span::dummy(),
                ))
            } else {
                None
            };

            program.extern_fns.push(Spanned::new(
                ExternFnDecl {
                    name: Spanned::new(func.pluto_name.clone(), Span::dummy()),
                    params,
                    return_type,
                    is_pub: false,
                },
                Span::dummy(),
            ));

            if func.is_fallible {
                program.fallible_extern_fns.push(func.pluto_name.clone());
            }
        }
    }
}

fn run_cargo_metadata(crate_path: &Path, alias: &str) -> Result<CrateMetadata, CompileError> {
    let manifest_path = crate_path.join("Cargo.toml");
    if !manifest_path.is_file() {
        return Err(CompileError::codegen(format!(
            "extern rust '{}': no Cargo.toml found at '{}'",
            alias,
            manifest_path.display()
        )));
    }

    let output = std::process::Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--no-deps")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .output()
        .map_err(|e| {
            CompileError::codegen(format!(
                "extern rust '{}': failed to run cargo metadata: {}",
                alias, e
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CompileError::codegen(format!(
            "extern rust '{}': cargo metadata failed: {}",
            alias, stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse JSON minimally — extract packages[].targets[] where kind == ["lib"]
    // We use a simple JSON parser approach to avoid adding serde_json dependency.
    parse_cargo_metadata_json(&stdout, &manifest_path, alias)
}

/// Minimal JSON parsing for cargo metadata output.
/// Extracts the lib target name and source path from the package matching our manifest.
fn parse_cargo_metadata_json(
    json: &str,
    manifest_path: &Path,
    alias: &str,
) -> Result<CrateMetadata, CompileError> {
    let canonical_manifest = manifest_path
        .canonicalize()
        .unwrap_or_else(|_| manifest_path.to_path_buf());
    let manifest_str = canonical_manifest.to_string_lossy();

    // Find the "packages" array start
    let packages_start = json.find("\"packages\"").ok_or_else(|| {
        CompileError::codegen(format!(
            "extern rust '{}': cargo metadata output missing 'packages' field",
            alias
        ))
    })?;

    let arr_start = json[packages_start..].find('[').map(|p| packages_start + p).ok_or_else(|| {
        CompileError::codegen(format!(
            "extern rust '{}': malformed cargo metadata output",
            alias
        ))
    })?;

    // Extract each package object from the packages array using brace depth tracking
    let pkg_object = find_matching_package(&json[arr_start..], &manifest_str)
        .ok_or_else(|| {
            CompileError::codegen(format!(
                "extern rust '{}': could not find package in cargo metadata (looking for manifest_path: {})",
                alias, manifest_str
            ))
        })?;

    // Now find "targets" within this specific package object
    let targets_pos = pkg_object.find("\"targets\"").ok_or_else(|| {
        CompileError::codegen(format!(
            "extern rust '{}': package has no 'targets' field",
            alias
        ))
    })?;

    let targets_arr_start = pkg_object[targets_pos..].find('[').map(|p| targets_pos + p).ok_or_else(|| {
        CompileError::codegen(format!(
            "extern rust '{}': malformed targets in cargo metadata",
            alias
        ))
    })?;

    // Find the lib target within the targets array
    let targets_region = &pkg_object[targets_arr_start..];

    let mut lib_name = None;
    let mut lib_src_path = None;

    let mut depth = 0;
    let mut obj_start = None;
    for (ci, ch) in targets_region.char_indices() {
        if ch == '[' && depth == 0 {
            depth = 1;
            continue;
        }
        if depth == 0 { continue; }
        if ch == '{' {
            if depth == 1 {
                obj_start = Some(ci);
            }
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 1 {
                if let Some(start) = obj_start {
                    let obj = &targets_region[start..=ci];
                    if obj.contains("\"lib\"") {
                        if let Some(name) = extract_json_string(obj, "name") {
                            lib_name = Some(name);
                        }
                        if let Some(src) = extract_json_string(obj, "src_path") {
                            lib_src_path = Some(PathBuf::from(src));
                        }
                        break;
                    }
                }
                obj_start = None;
            }
        } else if ch == ']' && depth == 1 {
            break;
        }
    }

    let lib_name = lib_name.ok_or_else(|| {
        CompileError::codegen(format!(
            "extern rust '{}': no lib target found in crate",
            alias
        ))
    })?;
    let lib_source_path = lib_src_path.ok_or_else(|| {
        CompileError::codegen(format!(
            "extern rust '{}': lib target has no src_path",
            alias
        ))
    })?;

    let manifest_dir = canonical_manifest
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    Ok(CrateMetadata {
        lib_name,
        lib_source_path,
        manifest_dir,
    })
}

/// Find the package object in the packages JSON array whose manifest_path matches.
/// Returns the full JSON text of the matching package object `{...}`.
fn find_matching_package<'a>(packages_array: &'a str, manifest_str: &str) -> Option<&'a str> {
    let bytes = packages_array.as_bytes();
    let mut i = 0;
    let mut depth: i32 = 0;
    let mut obj_start = None;

    while i < bytes.len() {
        let ch = bytes[i];
        match ch {
            b'"' => {
                // Skip over string content (handles escapes)
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i += 2; // skip escape sequence
                    } else if bytes[i] == b'"' {
                        break;
                    } else {
                        i += 1;
                    }
                }
            }
            b'[' if depth == 0 => { depth = 1; }
            b'[' => { depth += 1; }
            b']' => {
                if depth == 1 { break; }
                depth -= 1;
            }
            b'{' => {
                if depth == 1 {
                    obj_start = Some(i);
                }
                depth += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 1 {
                    if let Some(start) = obj_start {
                        let obj = &packages_array[start..=i];
                        if let Some(mp) = extract_json_string(obj, "manifest_path") {
                            if mp == manifest_str || mp.replace('\\', "/") == manifest_str.replace('\\', "/") {
                                return Some(obj);
                            }
                        }
                    }
                    obj_start = None;
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Extract a string value from a JSON object for a given key.
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let search = format!("\"{}\"", key);
    let pos = json.find(&search)?;
    let after_key = &json[pos + search.len()..];
    // Skip : and whitespace
    let colon_pos = after_key.find(':')?;
    let after_colon = &after_key[colon_pos + 1..];
    let trimmed = after_colon.trim_start();
    if !trimmed.starts_with('"') {
        return None;
    }
    let start = 1; // skip opening quote
    let mut end = start;
    let chars: Vec<char> = trimmed.chars().collect();
    while end < chars.len() && chars[end] != '"' {
        if chars[end] == '\\' {
            end += 1; // skip escape
        }
        end += 1;
    }
    Some(chars[start..end].iter().collect())
}

fn generate_glue_crate(crates: &[CrateInfo], glue_dir: &Path) -> Result<(), CompileError> {
    let src_dir = glue_dir.join("src");
    std::fs::create_dir_all(&src_dir).map_err(|e| {
        CompileError::codegen(format!("failed to create glue crate src dir: {}", e))
    })?;

    // Generate Cargo.toml
    let mut cargo_toml = String::from(
        "[package]\nname = \"pluto_ffi_glue\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ncrate-type = [\"staticlib\"]\n\n[dependencies]\n",
    );
    for info in crates {
        let dep_path = info.metadata.manifest_dir.to_string_lossy().replace('\\', "/");
        cargo_toml.push_str(&format!(
            "{} = {{ path = \"{}\" }}\n",
            info.metadata.lib_name, dep_path
        ));
    }

    std::fs::write(glue_dir.join("Cargo.toml"), &cargo_toml).map_err(|e| {
        CompileError::codegen(format!("failed to write glue Cargo.toml: {}", e))
    })?;

    // Check if any function is fallible (needs extern C decls for Pluto error runtime)
    let has_fallible = crates.iter().any(|info| info.functions.iter().any(|sig| sig.is_fallible));

    // Generate src/lib.rs
    let mut lib_rs = String::from(
        "use std::panic::catch_unwind;\nuse std::process::abort;\n\n",
    );

    if has_fallible {
        lib_rs.push_str("extern \"C\" {\n");
        lib_rs.push_str("    fn __pluto_alloc(size: i64) -> *mut u8;\n");
        lib_rs.push_str("    fn __pluto_string_new(data: *const u8, len: i64) -> *mut u8;\n");
        lib_rs.push_str("    fn __pluto_raise_error(error_obj: *mut u8);\n");
        lib_rs.push_str("}\n\n");
    }

    for info in crates {
        lib_rs.push_str(&format!("// --- crate: {}, alias: {} ---\n\n", info.metadata.lib_name, info.alias));

        for sig in &info.functions {
            let pluto_name = format!("{}.{}", info.alias, sig.name);
            let rust_fn_name = format!("__{}", pluto_name.replace('.', "_"));

            // Build parameter list for the wrapper
            let params_decl: Vec<String> = sig
                .params
                .iter()
                .map(|(name, rt)| format!("{}: {}", name, rust_type_to_c_type(rt)))
                .collect();
            let params_str = params_decl.join(", ");

            // Build arguments to pass to the real function
            let args: Vec<String> = sig
                .params
                .iter()
                .map(|(name, rt)| match rt {
                    RustType::Bool => format!("{} != 0", name),
                    _ => name.clone(),
                })
                .collect();
            let args_str = args.join(", ");

            if sig.is_fallible {
                // Generate wrapper that handles Result<T, E>
                let (ret_type, ok_arm, dummy_ret) = match &sig.return_type {
                    None => (
                        String::new(),
                        "Ok(Ok(())) => {}".to_string(),
                        String::new(),
                    ),
                    Some(RustType::Bool) => (
                        " -> i8".to_string(),
                        "Ok(Ok(true)) => 1i8,\n            Ok(Ok(false)) => 0i8,".to_string(),
                        "0i8".to_string(),
                    ),
                    Some(RustType::I64) => (
                        " -> i64".to_string(),
                        "Ok(Ok(v)) => v,".to_string(),
                        "0i64".to_string(),
                    ),
                    Some(RustType::F64) => (
                        " -> f64".to_string(),
                        "Ok(Ok(v)) => v,".to_string(),
                        "0.0f64".to_string(),
                    ),
                };

                let err_arm = if sig.return_type.is_some() {
                    format!(
                        "Ok(Err(e)) => unsafe {{\n\
                         \x20               let msg = format!(\"{{}}\", e);\n\
                         \x20               let msg_ptr = __pluto_string_new(msg.as_ptr(), msg.len() as i64);\n\
                         \x20               let err_obj = __pluto_alloc(8);\n\
                         \x20               *(err_obj as *mut i64) = msg_ptr as i64;\n\
                         \x20               __pluto_raise_error(err_obj);\n\
                         \x20               {}\n\
                         \x20           }},",
                        dummy_ret
                    )
                } else {
                    "Ok(Err(e)) => unsafe {\n\
                     \x20               let msg = format!(\"{}\", e);\n\
                     \x20               let msg_ptr = __pluto_string_new(msg.as_ptr(), msg.len() as i64);\n\
                     \x20               let err_obj = __pluto_alloc(8);\n\
                     \x20               *(err_obj as *mut i64) = msg_ptr as i64;\n\
                     \x20               __pluto_raise_error(err_obj);\n\
                     \x20           },".to_string()
                };

                let panic_arm = format!(
                    "Err(_) => {{ eprintln!(\"fatal: panic in Rust FFI '{}'\"); abort(); }}",
                    pluto_name
                );

                lib_rs.push_str(&format!(
                    "#[export_name = \"{}\"]\npub extern \"C\" fn {}({}){} {{\n    match catch_unwind(|| {}::{}({})) {{\n        {}\n        {}\n        {}\n    }}\n}}\n\n",
                    pluto_name,
                    rust_fn_name,
                    params_str,
                    ret_type,
                    info.metadata.lib_name,
                    sig.name,
                    args_str,
                    ok_arm,
                    err_arm,
                    panic_arm,
                ));
            } else {
                // Non-fallible: existing codegen
                let (ret_type, ok_handler, err_handler) = match &sig.return_type {
                    None => (
                        String::new(),
                        "Ok(()) => {}".to_string(),
                        format!(
                            "Err(_) => {{ eprintln!(\"fatal: panic in Rust FFI '{}'\"); abort(); }}",
                            pluto_name
                        ),
                    ),
                    Some(RustType::Bool) => (
                        " -> i8".to_string(),
                        "Ok(true) => 1,\n        Ok(false) => 0,".to_string(),
                        format!(
                            "Err(_) => {{ eprintln!(\"fatal: panic in Rust FFI '{}'\"); abort(); }}",
                            pluto_name
                        ),
                    ),
                    Some(rt) => (
                        format!(" -> {}", rust_type_to_c_type(rt)),
                        "Ok(v) => v,".to_string(),
                        format!(
                            "Err(_) => {{ eprintln!(\"fatal: panic in Rust FFI '{}'\"); abort(); }}",
                            pluto_name
                        ),
                    ),
                };

                lib_rs.push_str(&format!(
                    "#[export_name = \"{}\"]\npub extern \"C\" fn {}({}){} {{\n    match catch_unwind(|| {}::{}({})) {{\n        {}\n        {}\n    }}\n}}\n\n",
                    pluto_name,
                    rust_fn_name,
                    params_str,
                    ret_type,
                    info.metadata.lib_name,
                    sig.name,
                    args_str,
                    ok_handler,
                    err_handler,
                ));
            }
        }
    }

    std::fs::write(src_dir.join("lib.rs"), &lib_rs).map_err(|e| {
        CompileError::codegen(format!("failed to write glue lib.rs: {}", e))
    })?;

    Ok(())
}

fn build_glue_crate(glue_dir: &Path) -> Result<(PathBuf, Vec<String>), CompileError> {
    let output = std::process::Command::new("cargo")
        .arg("rustc")
        .arg("--release")
        .arg("--color")
        .arg("never")
        .arg("--crate-type")
        .arg("staticlib")
        .arg("--")
        .arg("--print")
        .arg("native-static-libs")
        .current_dir(glue_dir)
        .output()
        .map_err(|e| {
            CompileError::codegen(format!("failed to run cargo build for glue crate: {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CompileError::codegen(format!(
            "glue crate build failed:\n{}",
            stderr
        )));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Parse native-static-libs from stderr
    let native_libs = parse_native_static_libs(&stderr);

    // Find the static lib
    let target_dir = glue_dir.join("target").join("release");
    let lib_path = find_static_lib(&target_dir)?;

    Ok((lib_path, native_libs))
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until 'm' (end of ANSI escape sequence)
            for c2 in chars.by_ref() {
                if c2 == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn parse_native_static_libs(stderr: &str) -> Vec<String> {
    let stderr = &strip_ansi(stderr);
    for line in stderr.lines() {
        if let Some(pos) = line.find("native-static-libs:") {
            let libs_str = &line[pos + "native-static-libs:".len()..];
            return libs_str
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
        }
    }
    // Fallback for macOS
    if cfg!(target_os = "macos") {
        vec!["-lSystem".to_string(), "-lc".to_string(), "-lm".to_string()]
    } else {
        vec![
            "-lgcc_s".to_string(),
            "-lpthread".to_string(),
            "-lm".to_string(),
            "-ldl".to_string(),
            "-lc".to_string(),
        ]
    }
}

fn find_static_lib(target_dir: &Path) -> Result<PathBuf, CompileError> {
    // Look for libpluto_ffi_glue.a (Unix) or pluto_ffi_glue.lib (Windows)
    let unix_name = target_dir.join("libpluto_ffi_glue.a");
    if unix_name.is_file() {
        return Ok(unix_name);
    }
    let win_name = target_dir.join("pluto_ffi_glue.lib");
    if win_name.is_file() {
        return Ok(win_name);
    }

    Err(CompileError::codegen(format!(
        "glue crate static lib not found in '{}'",
        target_dir.display()
    )))
}

fn rust_type_to_pluto(rt: &RustType) -> PlutoType {
    match rt {
        RustType::I64 => PlutoType::Int,
        RustType::F64 => PlutoType::Float,
        RustType::Bool => PlutoType::Bool,
    }
}

fn rust_type_to_c_type(rt: &RustType) -> &'static str {
    match rt {
        RustType::I64 => "i64",
        RustType::F64 => "f64",
        RustType::Bool => "i8",
    }
}

fn pluto_type_to_type_expr(ty: &PlutoType) -> TypeExpr {
    match ty {
        PlutoType::Int => TypeExpr::Named("int".to_string()),
        PlutoType::Float => TypeExpr::Named("float".to_string()),
        PlutoType::Bool => TypeExpr::Named("bool".to_string()),
        PlutoType::Void => TypeExpr::Named("void".to_string()),
        _ => unreachable!("unsupported pluto type in rust ffi bridge"),
    }
}

fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for b in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    hash
}
