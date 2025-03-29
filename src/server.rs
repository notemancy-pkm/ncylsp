// src/server.rs
use async_trait::async_trait;
use lsp_types::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower_lsp::{Client, LanguageServer};

use crate::handlers::completion; // existing modules
use crate::handlers::custom_commands;
use crate::handlers::document_symbols::document_symbols;
use crate::handlers::formatting;
use crate::handlers::goto::goto_wikilink;
use crate::handlers::hover_wikilink;
use crate::handlers::workspace_symbols; // new formatting handler

pub struct NotemancyServer {
    client: Client,
    // Store open document texts by their URI – works for unsaved buffers too.
    documents: Arc<RwLock<HashMap<Url, String>>>,
}

impl NotemancyServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn get_document_text(&self, uri: &Url) -> Option<String> {
        let docs = self.documents.read().await;
        docs.get(uri).cloned()
    }
}

#[async_trait]
impl LanguageServer for NotemancyServer {
    async fn initialize(
        &self,
        _params: InitializeParams,
    ) -> Result<InitializeResult, tower_lsp::jsonrpc::Error> {
        self.client
            .log_message(MessageType::INFO, "Notemancy LSP initialized")
            .await;
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec!["[".to_string()]),
                    ..Default::default()
                }),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_formatting_provider: Some(OneOf::Left(true)), // Advertise formatting support
                ..Default::default()
            },
            server_info: None,
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .show_message(MessageType::INFO, "Notemancy LSP is ready")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let text_doc = params.text_document;
        self.documents
            .write()
            .await
            .insert(text_doc.uri, text_doc.text);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            self.documents.write().await.insert(uri, change.text);
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>, tower_lsp::jsonrpc::Error> {
        let uri = params.text_document.uri;
        let docs = self.documents.read().await;
        let text = docs.get(&uri).cloned().unwrap_or_default();
        drop(docs);

        let spinner = indicatif::ProgressBar::new_spinner();
        spinner.set_message("Processing document symbols...");
        spinner.enable_steady_tick(Duration::from_millis(100));

        let symbols = document_symbols(&text);

        spinner.finish_with_message("Finished processing document symbols");

        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>, tower_lsp::jsonrpc::Error> {
        let query = params.query;
        let symbols = workspace_symbols::get_workspace_symbols(&query).map_err(|e| {
            tower_lsp::jsonrpc::Error {
                code: tower_lsp::jsonrpc::ErrorCode::InternalError,
                message: e,
                data: None,
            }
        })?;
        Ok(Some(symbols))
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>, tower_lsp::jsonrpc::Error> {
        let uri = params.text_document_position.text_document.uri.clone();
        let docs = self.documents.read().await;
        let text = docs.get(&uri).cloned().unwrap_or_default();
        drop(docs);
        completion::provide_wiki_link_completions(params, &text)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>, tower_lsp::jsonrpc::Error> {
        let td_params = params.text_document_position_params;
        let uri = td_params.text_document.uri.clone();
        let docs = self.documents.read().await;
        let text = docs.get(&uri).cloned().unwrap_or_default();
        drop(docs);

        if let Some(location) = goto_wikilink(&text, td_params.position) {
            Ok(Some(GotoDefinitionResponse::Scalar(location)))
        } else {
            Ok(None)
        }
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>, tower_lsp::jsonrpc::Error> {
        let uri = params.text_document.uri;
        let docs = self.documents.read().await;
        let text = docs.get(&uri).cloned().unwrap_or_default();
        drop(docs);

        // Call the updated formatter with &text and &uri.
        let formatted = crate::handlers::formatting::format_markdown(&text, &uri).map_err(|e| {
            tower_lsp::jsonrpc::Error {
                code: tower_lsp::jsonrpc::ErrorCode::InternalError,
                message: format!("Markdown formatting error: {}", e),
                data: None,
            }
        })?;

        if formatted == text {
            Ok(None)
        } else {
            let lines: Vec<&str> = text.lines().collect();
            let last_line_len = lines.last().map(|l| l.len() as u32).unwrap_or(0);
            let full_range = tower_lsp::lsp_types::Range {
                start: tower_lsp::lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: tower_lsp::lsp_types::Position {
                    line: lines.len() as u32,
                    character: last_line_len,
                },
            };
            let edit = tower_lsp::lsp_types::TextEdit {
                range: full_range,
                new_text: formatted,
            };
            Ok(Some(vec![edit]))
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>, tower_lsp::jsonrpc::Error> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let document_text = self.get_document_text(&uri).await.unwrap_or_default();

        if let Some(hover) = hover_wikilink::hover_wikilink(&document_text, position) {
            Ok(Some(hover))
        } else {
            Ok(None)
        }
    }

    async fn shutdown(&self) -> Result<(), tower_lsp::jsonrpc::Error> {
        Ok(())
    }
}
