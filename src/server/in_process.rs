//! In-process compiler service implementation.
//!
//! This module provides `InProcessServer`, which implements the `CompilerService` trait
//! by calling compiler library functions directly.

use super::types::*;
use super::CompilerService;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use uuid::Uuid;

// ========== InProcessServer ==========

/// In-process compiler service.
///
/// Calls compiler library functions directly. This is a minimal implementation
/// that delegates most work to the compiler library.
pub struct InProcessServer {
    // Placeholder for future state (module cache, etc.)
    _state: Arc<RwLock<HashMap<PathBuf, SystemTime>>>,
}

impl InProcessServer {
    /// Create a new in-process server.
    pub fn new() -> Self {
        Self {
            _state: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InProcessServer {
    fn default() -> Self {
        Self::new()
    }
}

// ========== CompilerService Implementation ==========

impl CompilerService for InProcessServer {
    // ===== Module Management =====

    fn load_module(
        &mut self,
        path: &Path,
        opts: &LoadOptions,
    ) -> Result<ModuleSummary, ServiceError> {
        // For now, just verify the file exists and return a placeholder summary
        if !path.exists() {
            return Err(ServiceError::ModuleNotFound(path.to_path_buf()));
        }

        // Use analyze to load the file
        match crate::analyze_file(path, opts.stdlib.as_deref()) {
            Ok((program, _, _)) => {
                // Build summary from program
                Ok(ModuleSummary {
                    path: path.to_path_buf(),
                    name: path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    function_count: program.functions.len(),
                    class_count: program.classes.len(),
                    enum_count: program.enums.len(),
                    trait_count: program.traits.len(),
                    error_count: program.errors.len(),
                    app_count: if program.app.is_some() { 1 } else { 0 },
                })
            }
            Err(e) => Err(ServiceError::from(e)),
        }
    }

    fn load_project(
        &mut self,
        root: &Path,
        opts: &LoadOptions,
    ) -> Result<ProjectSummary, ServiceError> {
        if !root.is_dir() {
            return Err(ServiceError::InvalidPath(format!(
                "{} is not a directory",
                root.display()
            )));
        }

        // Discover all .pluto files
        let mut loaded = Vec::new();
        let mut failed = Vec::new();

        // Simple recursive search for .pluto files
        fn visit_dirs(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
            if dir.is_dir() {
                for entry in std::fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if !name.starts_with('.') {
                            visit_dirs(&path, files)?;
                        }
                    } else if path.extension().and_then(|e| e.to_str()) == Some("pluto") {
                        files.push(path);
                    }
                }
            }
            Ok(())
        }

        let mut pluto_files = Vec::new();
        visit_dirs(root, &mut pluto_files).map_err(ServiceError::Io)?;

        for file in pluto_files {
            match self.load_module(&file, opts) {
                Ok(_) => loaded.push(file),
                Err(e) => failed.push((file, e.to_string())),
            }
        }

        Ok(ProjectSummary {
            root: root.to_path_buf(),
            loaded,
            failed,
        })
    }

    fn list_modules(&self) -> Vec<ModuleInfo> {
        // TODO: Implement module tracking
        vec![]
    }

    fn reload_module(
        &mut self,
        path: &Path,
        opts: &LoadOptions,
    ) -> Result<ModuleSummary, ServiceError> {
        // For now, just delegate to load_module
        self.load_module(path, opts)
    }

    fn module_status(&self) -> Vec<ModuleStatus> {
        // TODO: Implement module tracking
        vec![]
    }

    // ===== Declaration Inspection =====

    fn list_declarations(
        &self,
        path: &Path,
        filter: Option<DeclKind>,
    ) -> Result<Vec<DeclSummary>, ServiceError> {
        // TODO: Implement using analyze_file and extracting declarations
        Err(ServiceError::Internal(
            "list_declarations not yet implemented".to_string(),
        ))
    }

    fn get_declaration(&self, path: &Path, id: Uuid) -> Result<DeclDetail, ServiceError> {
        // TODO: Implement using analyze_file
        Err(ServiceError::Internal(
            "get_declaration not yet implemented".to_string(),
        ))
    }

    fn find_declaration(&self, name: &str, filter: Option<DeclKind>) -> Vec<DeclMatch> {
        // TODO: Implement cross-module search
        vec![]
    }

    // ===== Cross-References & Analysis =====

    fn callers_of(&self, id: Uuid) -> Vec<XrefSite> {
        // TODO: Implement using xref module
        vec![]
    }

    fn constructors_of(&self, id: Uuid) -> Vec<XrefSite> {
        vec![]
    }

    fn enum_usages_of(&self, id: Uuid) -> Vec<XrefSite> {
        vec![]
    }

    fn raise_sites_of(&self, id: Uuid) -> Vec<XrefSite> {
        vec![]
    }

    fn usages_of(&self, id: Uuid) -> Vec<XrefSite> {
        vec![]
    }

    fn call_graph(
        &self,
        id: Uuid,
        opts: &CallGraphOptions,
    ) -> Result<CallGraphResult, ServiceError> {
        Err(ServiceError::Internal(
            "call_graph not yet implemented".to_string(),
        ))
    }

    fn error_set(&self, path: &Path, id: Uuid) -> Result<ErrorSetInfo, ServiceError> {
        Ok(ErrorSetInfo {
            is_fallible: false,
            errors: vec![],
        })
    }

    // ===== Source Access =====

    fn get_source(&self, path: &Path, range: Option<ByteRange>) -> Result<String, ServiceError> {
        let source = std::fs::read_to_string(path).map_err(ServiceError::Io)?;

        match range {
            Some(ByteRange { start, end }) => {
                if end > source.len() {
                    return Err(ServiceError::InvalidParameter(format!(
                        "Range {}-{} exceeds source length {}",
                        start,
                        end,
                        source.len()
                    )));
                }
                Ok(source[start..end].to_string())
            }
            None => Ok(source),
        }
    }

    fn pretty_print(
        &self,
        path: &Path,
        id: Option<Uuid>,
        include_uuids: bool,
    ) -> Result<String, ServiceError> {
        // Just return the source for now
        self.get_source(path, None)
    }

    // ===== Compilation & Execution =====

    fn check(&self, path: &Path, opts: &CompileOptions) -> CheckResult {
        match crate::analyze_file_with_warnings(path, opts.stdlib.as_deref()) {
            Ok((_program, _source, _derived, warnings)) => CheckResult {
                success: true,
                path: path.to_path_buf(),
                errors: vec![],
                warnings: warnings
                    .into_iter()
                    .map(|w| Diagnostic::from_compile_warning(&w, None))
                    .collect(),
            },
            Err(err) => CheckResult {
                success: false,
                path: path.to_path_buf(),
                errors: vec![Diagnostic::from_compile_error(&err, None)],
                warnings: vec![],
            },
        }
    }

    fn compile(&self, path: &Path, output: &Path, opts: &CompileOptions) -> CompileResult {
        match crate::compile_file_with_options(path, output, opts.stdlib.as_deref(), opts.gc) {
            Ok(()) => CompileResult {
                success: true,
                path: path.to_path_buf(),
                output: Some(output.to_path_buf()),
                errors: vec![],
                warnings: vec![],
            },
            Err(err) => CompileResult {
                success: false,
                path: path.to_path_buf(),
                output: None,
                errors: vec![Diagnostic::from_compile_error(&err, None)],
                warnings: vec![],
            },
        }
    }

    fn run(&self, path: &Path, opts: &RunOptions) -> RunResult {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        // Create temp output file
        let temp_dir = std::env::temp_dir();
        let output = temp_dir.join(format!("pluto_run_{}", uuid::Uuid::new_v4()));

        // Compile
        let compile_opts = CompileOptions {
            stdlib: opts.stdlib.clone(),
            gc: crate::GcBackend::MarkSweep,
            coverage: false,
        };

        match crate::compile_file_with_options(
            path,
            &output,
            compile_opts.stdlib.as_deref(),
            compile_opts.gc,
        ) {
            Ok(()) => {
                // Execute with timeout
                let mut cmd = Command::new(&output);
                cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
                if let Some(dir) = &opts.cwd {
                    cmd.current_dir(dir);
                }

                let timeout = Duration::from_millis(opts.timeout_ms.unwrap_or(10000));
                let start = Instant::now();

                match cmd.spawn() {
                    Ok(mut child) => {
                        // Simple wait with timeout
                        let result = loop {
                            if start.elapsed() >= timeout {
                                let _ = child.kill();
                                break Ok((String::new(), String::new(), None, true));
                            }

                            match child.try_wait() {
                                Ok(Some(status)) => {
                                    let output = child
                                        .wait_with_output()
                                        .unwrap_or_else(|_| std::process::Output {
                                            status,
                                            stdout: vec![],
                                            stderr: vec![],
                                        });
                                    break Ok((
                                        String::from_utf8_lossy(&output.stdout).to_string(),
                                        String::from_utf8_lossy(&output.stderr).to_string(),
                                        status.code(),
                                        false,
                                    ));
                                }
                                Ok(None) => {
                                    std::thread::sleep(Duration::from_millis(50));
                                }
                                Err(e) => {
                                    break Err(e.to_string());
                                }
                            }
                        };

                        // Clean up binary
                        let _ = std::fs::remove_file(&output);

                        match result {
                            Ok((stdout, stderr, exit_code, timed_out)) => RunResult {
                                success: exit_code == Some(0),
                                path: path.to_path_buf(),
                                stdout,
                                stderr,
                                exit_code,
                                timed_out,
                                compile_errors: vec![],
                            },
                            Err(e) => RunResult {
                                success: false,
                                path: path.to_path_buf(),
                                stdout: String::new(),
                                stderr: e,
                                exit_code: None,
                                timed_out: false,
                                compile_errors: vec![],
                            },
                        }
                    }
                    Err(e) => RunResult {
                        success: false,
                        path: path.to_path_buf(),
                        stdout: String::new(),
                        stderr: e.to_string(),
                        exit_code: None,
                        timed_out: false,
                        compile_errors: vec![],
                    },
                }
            }
            Err(err) => RunResult {
                success: false,
                path: path.to_path_buf(),
                stdout: String::new(),
                stderr: String::new(),
                exit_code: None,
                timed_out: false,
                compile_errors: vec![Diagnostic::from_compile_error(&err, None)],
            },
        }
    }

    fn test(&self, path: &Path, opts: &TestOptions) -> TestResult {
        // TODO: Use test compilation mode
        TestResult {
            success: false,
            path: path.to_path_buf(),
            stdout: String::new(),
            stderr: "test not yet implemented".to_string(),
            exit_code: None,
            timed_out: false,
            compile_errors: vec![],
        }
    }

    // ===== Editing Operations =====

    fn add_declaration(
        &mut self,
        path: &Path,
        source: &str,
    ) -> Result<EditResult, ServiceError> {
        Err(ServiceError::Internal(
            "add_declaration not yet implemented".to_string(),
        ))
    }

    fn replace_declaration(
        &mut self,
        path: &Path,
        name: &str,
        source: &str,
    ) -> Result<EditResult, ServiceError> {
        Err(ServiceError::Internal(
            "replace_declaration not yet implemented".to_string(),
        ))
    }

    fn delete_declaration(
        &mut self,
        path: &Path,
        name: &str,
    ) -> Result<DeleteResult, ServiceError> {
        Err(ServiceError::Internal(
            "delete_declaration not yet implemented".to_string(),
        ))
    }

    fn rename_declaration(
        &mut self,
        path: &Path,
        old_name: &str,
        new_name: &str,
    ) -> Result<EditResult, ServiceError> {
        Err(ServiceError::Internal(
            "rename_declaration not yet implemented".to_string(),
        ))
    }

    fn add_method(
        &mut self,
        path: &Path,
        class_name: &str,
        source: &str,
    ) -> Result<EditResult, ServiceError> {
        Err(ServiceError::Internal(
            "add_method not yet implemented".to_string(),
        ))
    }

    fn add_field(
        &mut self,
        path: &Path,
        class_name: &str,
        field_name: &str,
        field_type: &str,
    ) -> Result<EditResult, ServiceError> {
        Err(ServiceError::Internal(
            "add_field not yet implemented".to_string(),
        ))
    }

    // ===== Format & Sync =====

    fn sync_pt(&mut self, pt_path: &Path, pluto_path: &Path) -> Result<SyncResult, ServiceError> {
        Err(ServiceError::Internal(
            "sync_pt not yet implemented".to_string(),
        ))
    }

    fn analyze_and_update(&self, path: &Path, opts: &LoadOptions) -> Result<(), ServiceError> {
        Err(ServiceError::Internal(
            "analyze_and_update not yet implemented".to_string(),
        ))
    }

    // ===== Documentation =====

    fn language_docs(&self, topic: Option<&str>) -> Result<String, ServiceError> {
        Ok("Language documentation not yet implemented".to_string())
    }

    fn stdlib_docs(&self, module: Option<&str>) -> Result<String, ServiceError> {
        Ok("Stdlib documentation not yet implemented".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_server() {
        let server = InProcessServer::new();
        assert_eq!(server.list_modules().len(), 0);
    }

    #[test]
    fn test_module_status_empty() {
        let server = InProcessServer::new();
        assert_eq!(server.module_status().len(), 0);
    }
}
