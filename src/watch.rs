use crossbeam_channel::{select, unbounded, Receiver, Sender};
use notify::{Event, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use crate::diagnostics::CompileError;

/// Watch a Pluto file and automatically recompile and rerun when changes are detected
pub fn watch_run(
    entry_file: &Path,
    stdlib: Option<&Path>,
    no_clear: bool,
) -> Result<(), CompileError> {
    println!("Watching {} for changes...", entry_file.display());

    // Initial compile and run
    let binary = compile_entry_file(entry_file, stdlib)?;
    let mut child = spawn_process(&binary)
        .map_err(|e| CompileError::codegen(format!("failed to spawn process: {}", e)))?;
    print_separator();

    // Get all files to watch (entry + transitive imports)
    let watched_files = get_watched_files(entry_file, stdlib)?;

    // Setup file watcher
    let (tx, rx) = unbounded();
    let mut watcher = create_watcher(tx)?;

    for file in &watched_files {
        watcher.watch(file, RecursiveMode::NonRecursive)
            .map_err(|e| CompileError::codegen(format!("failed to watch file {}: {}", file.display(), e)))?;
    }

    // Event loop
    loop {
        // Wait for file change
        wait_for_change(&rx);

        // Debounce
        debounce_events(&rx);

        // Kill running process
        graceful_kill(&mut child)
            .map_err(|e| CompileError::codegen(format!("failed to kill process: {}", e)))?;

        // Clear terminal
        if !no_clear {
            clearscreen::clear().ok();
        }

        // Recompile
        println!("File changed, recompiling...");
        match compile_entry_file(entry_file, stdlib) {
            Ok(new_binary) => {
                // Spawn new process
                match spawn_process(&new_binary) {
                    Ok(new_child) => {
                        child = new_child;
                        print_separator();
                    }
                    Err(e) => {
                        eprintln!("Error spawning process: {}", e);
                        print_separator();
                        // Continue watching even if spawn fails
                    }
                }
            }
            Err(e) => {
                eprintln!("Compilation failed: {}", e);
                print_separator();
                // Continue watching even on compilation error
                // Spawn a dummy child that exits immediately so we have something to kill later
                child = Command::new("true").spawn().unwrap();
            }
        }
    }
}

/// Compile the entry file and return the path to the binary
fn compile_entry_file(entry_file: &Path, stdlib: Option<&Path>) -> Result<PathBuf, CompileError> {
    // Use a temporary output file
    let output = std::env::temp_dir().join(format!(
        "pluto_watch_{}",
        entry_file.file_stem().unwrap().to_string_lossy()
    ));

    crate::compile_file_with_stdlib(entry_file, &output, stdlib)?;
    Ok(output)
}

/// Spawn a process from the given binary path
fn spawn_process(binary: &Path) -> std::io::Result<Child> {
    Command::new(binary)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
}

/// Kill a process gracefully (SIGTERM, then SIGKILL after timeout)
fn graceful_kill(child: &mut Child) -> std::io::Result<()> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    use std::thread;

    let pid = Pid::from_raw(child.id() as i32);

    // Send SIGTERM
    let _ = kill(pid, Signal::SIGTERM);

    // Poll for 1 second
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(1) {
        if let Some(_) = child.try_wait()? {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }

    // Still running, send SIGKILL
    let _ = kill(pid, Signal::SIGKILL);
    child.wait()?;
    Ok(())
}

/// Wait for the first file change event
fn wait_for_change(rx: &Receiver<Event>) {
    // Block until we get an event
    let _ = rx.recv();
}

/// Debounce events by waiting for a quiet period
fn debounce_events(rx: &Receiver<Event>) {
    loop {
        select! {
            recv(rx) -> _event => {
                // Got another event, keep waiting
            }
            default(Duration::from_millis(100)) => {
                // No events for 100ms, we're done
                break;
            }
        }
    }
}

/// Get all files to watch (entry file + transitive imports)
fn get_watched_files(entry_file: &Path, stdlib: Option<&Path>) -> Result<Vec<PathBuf>, CompileError> {
    // Create an empty package graph for module resolution
    let pkg_graph = crate::manifest::PackageGraph::empty();

    // Use module resolution to discover all imported files
    match crate::modules::resolve_modules(entry_file, stdlib, &pkg_graph) {
        Ok(graph) => {
            // Extract all file paths from the source map
            let mut files: Vec<PathBuf> = graph.source_map
                .files
                .iter()
                .filter_map(|(path, _source)| path.canonicalize().ok())
                .collect();

            // Always watch the entry file
            if let Ok(canonical_entry) = entry_file.canonicalize() {
                if !files.contains(&canonical_entry) {
                    files.push(canonical_entry);
                }
            }

            Ok(files)
        }
        Err(_) => {
            // If module resolution fails, fall back to watching just the entry file
            Ok(vec![entry_file.canonicalize().unwrap_or_else(|_| entry_file.to_path_buf())])
        }
    }
}

/// Create a file watcher with the given sender
fn create_watcher(tx: Sender<Event>) -> Result<notify::RecommendedWatcher, CompileError> {
    notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            // Only react to write events
            if matches!(event.kind, notify::EventKind::Modify(_) | notify::EventKind::Create(_)) {
                let _ = tx.send(event);
            }
        }
    })
    .map_err(|e| CompileError::codegen(format!("failed to create file watcher: {}", e)))
}

/// Watch a Pluto file and automatically recompile and rerun tests when changes are detected
pub fn watch_test(
    entry_file: &Path,
    stdlib: Option<&Path>,
    no_clear: bool,
    use_cache: bool,
) -> Result<(), CompileError> {
    println!("Watching {} for test changes...", entry_file.display());

    // Initial compile and run tests
    let binary = compile_test_file(entry_file, stdlib, use_cache)?;
    let exit_code = run_tests(&binary)
        .map_err(|e| CompileError::codegen(format!("failed to run tests: {}", e)))?;
    print_test_separator(exit_code);

    // Get all files to watch (entry + transitive imports)
    let watched_files = get_watched_files(entry_file, stdlib)?;

    // Setup file watcher
    let (tx, rx) = unbounded();
    let mut watcher = create_watcher(tx)?;

    for file in &watched_files {
        watcher.watch(file, RecursiveMode::NonRecursive)
            .map_err(|e| CompileError::codegen(format!("failed to watch file {}: {}", file.display(), e)))?;
    }

    // Event loop
    loop {
        // Wait for file change
        wait_for_change(&rx);

        // Debounce
        debounce_events(&rx);

        // Clear terminal
        if !no_clear {
            clearscreen::clear().ok();
        }

        // Recompile and run tests
        println!("File changed, recompiling tests...");
        match compile_test_file(entry_file, stdlib, use_cache) {
            Ok(new_binary) => {
                match run_tests(&new_binary) {
                    Ok(exit_code) => {
                        print_test_separator(exit_code);
                    }
                    Err(e) => {
                        eprintln!("Error running tests: {}", e);
                        print_test_separator(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("Compilation failed: {}", e);
                print_test_separator(1);
                // Continue watching even on compilation error
            }
        }
    }
}

/// Compile the entry file for tests and return the path to the binary
fn compile_test_file(entry_file: &Path, stdlib: Option<&Path>, use_cache: bool) -> Result<PathBuf, CompileError> {
    // Use a temporary output file
    let output = std::env::temp_dir().join(format!(
        "pluto_watch_test_{}",
        entry_file.file_stem().unwrap().to_string_lossy()
    ));

    crate::compile_file_for_tests(entry_file, &output, stdlib, use_cache)?;
    Ok(output)
}

/// Run tests and return the exit code
fn run_tests(binary: &Path) -> std::io::Result<i32> {
    let status = Command::new(binary)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    Ok(status.code().unwrap_or(1))
}

/// Print a separator line with test status
fn print_test_separator(exit_code: i32) {
    let status = if exit_code == 0 {
        "✓ TESTS PASSED"
    } else {
        "✗ TESTS FAILED"
    };
    println!("\n{} {}\n", "=".repeat(25), status);
}

/// Print a separator line
fn print_separator() {
    println!("\n{}\n", "=".repeat(60));
}
