use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{DidOpenTextDocument, DidSaveTextDocument, Notification as _};
use lsp_types::request::{
    DocumentSymbolRequest, GotoDefinition, HoverRequest, Request as _,
};
use lsp_types::{
    DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams, HoverParams,
    InitializeParams, OneOf, PublishDiagnosticsParams, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, Uri,
};

use super::analysis::{self, AnalysisResult};
use super::diagnostics::compile_error_to_diagnostics;
use super::goto_def::handle_goto_definition;
use super::hover::handle_hover;
use super::symbols::document_symbols;
use super::{path_to_uri, uri_to_path};

struct ServerState {
    analysis: Option<AnalysisResult>,
    stdlib_root: Option<PathBuf>,
}

/// Run the LSP server on stdin/stdout. Blocks until the client disconnects.
pub fn run_lsp_server() -> Result<(), Box<dyn Error + Sync + Send>> {
    let (connection, io_threads) = Connection::stdio();

    let server_capabilities = serde_json::to_value(ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::NONE),
                save: Some(TextDocumentSyncSaveOptions::SaveOptions(lsp_types::SaveOptions {
                    include_text: Some(false),
                })),
                ..Default::default()
            },
        )),
        definition_provider: Some(OneOf::Left(true)),
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        ..Default::default()
    })?;

    let init_params = serde_json::from_value::<InitializeParams>(
        connection.initialize(server_capabilities)?,
    )?;

    let stdlib_root = init_params
        .initialization_options
        .as_ref()
        .and_then(|opts| opts.get("stdlibRoot"))
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .or_else(|| std::env::var("PLUTO_STDLIB").ok().map(PathBuf::from));

    let mut state = ServerState {
        analysis: None,
        stdlib_root,
    };

    eprintln!("[plutoc-lsp] server initialized");

    main_loop(&connection, &mut state)?;

    io_threads.join()?;

    eprintln!("[plutoc-lsp] server shut down");
    Ok(())
}

fn main_loop(
    connection: &Connection,
    state: &mut ServerState,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                handle_request(connection, state, req)?;
            }
            Message::Notification(not) => {
                handle_notification(connection, state, not)?;
            }
            Message::Response(_) => {}
        }
    }
    Ok(())
}

fn handle_request(
    connection: &Connection,
    state: &mut ServerState,
    req: Request,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let req_id = req.id.clone();
    let method = req.method.as_str();

    match method {
        GotoDefinition::METHOD => {
            let params: GotoDefinitionParams = serde_json::from_value(req.params)?;
            let uri = &params.text_document_position_params.text_document.uri;
            let position = params.text_document_position_params.position;

            let result = state
                .analysis
                .as_ref()
                .and_then(|a| handle_goto_definition(a, uri, position));

            send_response(connection, req_id, result)?;
        }
        HoverRequest::METHOD => {
            let params: HoverParams = serde_json::from_value(req.params)?;
            let uri = &params.text_document_position_params.text_document.uri;
            let position = params.text_document_position_params.position;

            let result = state
                .analysis
                .as_ref()
                .and_then(|a| handle_hover(a, uri, position));

            send_response(connection, req_id, result)?;
        }
        DocumentSymbolRequest::METHOD => {
            let params: DocumentSymbolParams = serde_json::from_value(req.params)?;
            let uri = &params.text_document.uri;

            let result = state.analysis.as_ref().and_then(|a| {
                let file_path = uri_to_path(uri)?;
                let file_id = a
                    .file_paths
                    .iter()
                    .find(|(_, p)| **p == file_path)
                    .map(|(id, _)| *id)?;
                let line_index = a.line_indices.get(&file_id)?;
                let syms = document_symbols(&a.program, file_id, line_index);
                Some(DocumentSymbolResponse::Nested(syms))
            });

            send_response(connection, req_id, result)?;
        }
        _ => {
            let resp = Response::new_err(
                req_id,
                lsp_server::ErrorCode::MethodNotFound as i32,
                format!("unknown method: {method}"),
            );
            connection.sender.send(Message::Response(resp))?;
        }
    }

    Ok(())
}

fn handle_notification(
    connection: &Connection,
    state: &mut ServerState,
    not: Notification,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    match not.method.as_str() {
        DidSaveTextDocument::METHOD => {
            let params: lsp_types::DidSaveTextDocumentParams =
                serde_json::from_value(not.params)?;
            let uri = params.text_document.uri;
            run_analysis(connection, state, &uri)?;
        }
        DidOpenTextDocument::METHOD => {
            let params: lsp_types::DidOpenTextDocumentParams =
                serde_json::from_value(not.params)?;
            let uri = params.text_document.uri;
            run_analysis(connection, state, &uri)?;
        }
        _ => {}
    }
    Ok(())
}

fn run_analysis(
    connection: &Connection,
    state: &mut ServerState,
    uri: &Uri,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let file_path: PathBuf = match uri_to_path(uri) {
        Some(p) => p,
        None => return Ok(()),
    };

    if file_path.extension().and_then(|e| e.to_str()) != Some("pluto") {
        return Ok(());
    }

    let stdlib = state.stdlib_root.as_deref();

    match analysis::check_file(&file_path, stdlib) {
        Ok(result) => {
            publish_clear_diagnostics(connection, &result.file_paths)?;
            state.analysis = Some(result);
        }
        Err(analysis_err) => {
            let source = std::fs::read_to_string(&file_path).unwrap_or_default();
            let line_index = super::line_index::LineIndex::new(&source);
            let mut line_indices = HashMap::new();
            let mut file_paths = HashMap::new();
            line_indices.insert(0, line_index);
            file_paths.insert(0, file_path.clone());

            if let Some(prev) = &state.analysis {
                for (fid, path) in &prev.file_paths {
                    if !file_paths.contains_key(fid) {
                        file_paths.insert(*fid, path.clone());
                    }
                }
                for (fid, _) in &prev.file_paths {
                    if !line_indices.contains_key(fid) {
                        if let Some((_, src)) = prev.source_map.files.get(*fid as usize) {
                            line_indices.insert(*fid, super::line_index::LineIndex::new(src));
                        }
                    }
                }
            }

            let diags = compile_error_to_diagnostics(
                &analysis_err.error,
                &line_indices,
                &file_paths,
            );

            publish_clear_diagnostics(connection, &file_paths)?;

            let mut by_uri: HashMap<Uri, Vec<lsp_types::Diagnostic>> = HashMap::new();
            for (diag_uri, diag) in diags {
                by_uri.entry(diag_uri).or_default().push(diag);
            }
            for (diag_uri, diagnostics) in by_uri {
                publish_diagnostics(connection, diag_uri, diagnostics)?;
            }

            state.analysis = None;
        }
    }

    Ok(())
}

fn publish_diagnostics(
    connection: &Connection,
    uri: Uri,
    diagnostics: Vec<lsp_types::Diagnostic>,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let params = PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: None,
    };
    let not = Notification::new(
        lsp_types::notification::PublishDiagnostics::METHOD.to_string(),
        serde_json::to_value(params)?,
    );
    connection.sender.send(Message::Notification(not))?;
    Ok(())
}

fn publish_clear_diagnostics(
    connection: &Connection,
    file_paths: &HashMap<u32, PathBuf>,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    for (_, path) in file_paths {
        publish_diagnostics(connection, path_to_uri(path), vec![])?;
    }
    Ok(())
}

fn send_response<T: serde::Serialize>(
    connection: &Connection,
    id: RequestId,
    result: Option<T>,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let resp = match result {
        Some(val) => Response::new_ok(id, serde_json::to_value(val)?),
        None => Response::new_ok(id, serde_json::Value::Null),
    };
    connection.sender.send(Message::Response(resp))?;
    Ok(())
}
