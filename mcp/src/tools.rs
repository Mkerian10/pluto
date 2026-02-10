use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LoadModuleInput {
    #[schemars(description = "Absolute path to a .pluto source file or PLTO binary file")]
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListDeclarationsInput {
    #[schemars(description = "Path of the loaded module (as returned by load_module)")]
    pub path: String,
    #[schemars(description = "Optional kind filter: function, class, enum, trait, error, app")]
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct InspectInput {
    #[schemars(description = "Path of the loaded module")]
    pub path: String,
    #[schemars(description = "UUID of the declaration to inspect")]
    pub uuid: Option<String>,
    #[schemars(description = "Name of the declaration to inspect (may be ambiguous)")]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct XrefsInput {
    #[schemars(description = "Path of the loaded module")]
    pub path: String,
    #[schemars(description = "UUID of the declaration to query cross-references for")]
    pub uuid: String,
    #[schemars(
        description = "Kind of cross-reference: callers, constructors, enum_usages, raise_sites"
    )]
    pub kind: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ErrorsInput {
    #[schemars(description = "Path of the loaded module")]
    pub path: String,
    #[schemars(description = "UUID of the function to query error info for")]
    pub uuid: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SourceInput {
    #[schemars(description = "Path of the loaded module")]
    pub path: String,
    #[schemars(description = "Start byte offset (defaults to 0)")]
    pub start: Option<usize>,
    #[schemars(description = "End byte offset (defaults to end of source)")]
    pub end: Option<usize>,
}
