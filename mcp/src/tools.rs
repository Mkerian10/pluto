use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LoadModuleInput {
    #[schemars(description = "Absolute path to a .pluto source file or PLTO binary file")]
    pub path: String,
    #[schemars(description = "Path to the stdlib root directory (needed for files that import std.* modules)")]
    pub stdlib: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListDeclarationsInput {
    #[schemars(description = "Path of the loaded module (as returned by load_module)")]
    pub path: String,
    #[schemars(description = "Optional kind filter: function, class, enum, trait, error, app")]
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetDeclarationInput {
    #[schemars(description = "Path of the loaded module")]
    pub path: String,
    #[schemars(description = "UUID of the declaration to inspect")]
    pub uuid: Option<String>,
    #[schemars(description = "Name of the declaration to inspect (may be ambiguous)")]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CallersOfInput {
    #[schemars(description = "UUID of the function to find callers of (searches all loaded modules)")]
    pub uuid: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ConstructorsOfInput {
    #[schemars(description = "UUID of the class to find constructors of (searches all loaded modules)")]
    pub uuid: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EnumUsagesOfInput {
    #[schemars(description = "UUID of the enum to find usages of (searches all loaded modules)")]
    pub uuid: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RaiseSitesOfInput {
    #[schemars(description = "UUID of the error to find raise sites of (searches all loaded modules)")]
    pub uuid: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UsagesOfInput {
    #[schemars(description = "UUID of the declaration to find all usages of (searches all loaded modules)")]
    pub uuid: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ErrorSetInput {
    #[schemars(description = "Path of the loaded module")]
    pub path: String,
    #[schemars(description = "UUID of the function to query error info for")]
    pub uuid: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CallGraphInput {
    #[schemars(description = "UUID of the function to build call graph from")]
    pub uuid: String,
    #[schemars(description = "Maximum depth to traverse (default: 5, max: 20)")]
    pub max_depth: Option<usize>,
    #[schemars(description = "Direction: 'callers' (who calls this) or 'callees' (who this calls). Default: 'callees'")]
    pub direction: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSourceInput {
    #[schemars(description = "Path of the loaded module")]
    pub path: String,
    #[schemars(description = "Start byte offset (defaults to 0)")]
    pub start: Option<usize>,
    #[schemars(description = "End byte offset (defaults to end of source)")]
    pub end: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LoadProjectInput {
    #[schemars(description = "Absolute path to the project root directory to scan for .pluto files")]
    pub path: String,
    #[schemars(description = "Path to the stdlib root directory (needed for files that import std.* modules)")]
    pub stdlib: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListModulesInput {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FindDeclarationInput {
    #[schemars(description = "Name of the declaration to search for across all loaded modules")]
    pub name: String,
    #[schemars(description = "Optional kind filter: function, class, enum, trait, error, app")]
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CheckInput {
    #[schemars(description = "Absolute path to the .pluto source file to type-check")]
    pub path: String,
    #[schemars(description = "Path to the stdlib root directory (needed for files that import std.* modules)")]
    pub stdlib: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CompileInput {
    #[schemars(description = "Absolute path to the .pluto source file to compile")]
    pub path: String,
    #[schemars(description = "Output path for the compiled binary. If omitted, uses a temp file")]
    pub output: Option<String>,
    #[schemars(description = "Path to the stdlib root directory (needed for files that import std.* modules)")]
    pub stdlib: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunInput {
    #[schemars(description = "Absolute path to the .pluto source file to compile and run")]
    pub path: String,
    #[schemars(description = "Path to the stdlib root directory (needed for files that import std.* modules)")]
    pub stdlib: Option<String>,
    #[schemars(description = "Execution timeout in milliseconds (default: 10000, max: 60000)")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Working directory for execution. Defaults to the source file's parent directory")]
    pub cwd: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TestInput {
    #[schemars(description = "Absolute path to the .pluto source file containing tests to compile and run")]
    pub path: String,
    #[schemars(description = "Path to the stdlib root directory (needed for files that import std.* modules)")]
    pub stdlib: Option<String>,
    #[schemars(description = "Execution timeout in milliseconds (default: 30000, max: 60000)")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "Working directory for execution. Defaults to the source file's parent directory")]
    pub cwd: Option<String>,
}

// --- Write tool inputs ---

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddDeclarationInput {
    #[schemars(description = "Path to the .pluto source file. Created if it doesn't exist.")]
    pub path: String,
    #[schemars(description = "Pluto source code for the declaration to add (e.g. a function, class, enum, etc.)")]
    pub source: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReplaceDeclarationInput {
    #[schemars(description = "Path of the .pluto source file")]
    pub path: String,
    #[schemars(description = "Name of the declaration to replace")]
    pub name: String,
    #[schemars(description = "Pluto source code for the replacement (must be the same kind)")]
    pub source: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteDeclarationInput {
    #[schemars(description = "Path of the .pluto source file")]
    pub path: String,
    #[schemars(description = "Name of the declaration to delete")]
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RenameDeclarationInput {
    #[schemars(description = "Path of the .pluto source file")]
    pub path: String,
    #[schemars(description = "Current name of the declaration")]
    pub old_name: String,
    #[schemars(description = "New name for the declaration")]
    pub new_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddMethodInput {
    #[schemars(description = "Path of the .pluto source file")]
    pub path: String,
    #[schemars(description = "Name of the class to add the method to")]
    pub class_name: String,
    #[schemars(description = "Pluto source code for the method (must include self param)")]
    pub source: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddFieldInput {
    #[schemars(description = "Path of the .pluto source file")]
    pub path: String,
    #[schemars(description = "Name of the class to add the field to")]
    pub class_name: String,
    #[schemars(description = "Name of the new field")]
    pub field_name: String,
    #[schemars(description = "Type of the new field (e.g. 'int', 'string', '[float]')")]
    pub field_type: String,
}

// --- Docs tool inputs ---

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DocsInput {
    #[schemars(description = "Optional topic filter: types, operators, statements, declarations, strings, errors, closures, generics, modules, contracts, concurrency, gotchas")]
    pub topic: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StdlibDocsInput {
    #[schemars(description = "Optional module name (e.g. 'strings', 'fs', 'math'). If omitted, lists all available stdlib modules")]
    pub module: Option<String>,
}

// --- File watching tool inputs ---

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReloadModuleInput {
    #[schemars(description = "Path of the module to reload from disk")]
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ModuleStatusInput {}

// --- Format tool inputs ---

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SyncPtInput {
    #[schemars(description = "Path to the .pt text file to sync from")]
    pub pt_path: String,
    #[schemars(description = "Path to the .pluto binary file to sync to (defaults to same name with .pluto extension)")]
    pub pluto_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PrettyPrintInput {
    #[schemars(description = "Path of the loaded module")]
    pub path: String,
    #[schemars(description = "UUID of a specific declaration to pretty-print (if omitted, prints entire module)")]
    pub uuid: Option<String>,
    #[schemars(description = "Include UUID hints in output (default: false)")]
    pub include_uuid_hints: Option<bool>,
}
