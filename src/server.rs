use async_trait::async_trait;
use lsp_types::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower_lsp::{Client, LanguageServer};

use crate::handlers::completion; // our completion module
use crate::handlers::document_symbols::document_symbols;
use crate::handlers::goto::goto_wikilink;
use crate::handlers::hover_wikilink;
use crate::handlers::workspace_symbols; // workspace symbols handler // our new goto helper

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
                // Register the hover provider capability:
                hover_provider: Some(HoverProviderCapability::Simple(true)),
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
            // If we didn't detect a wiki-link, we return None.
            Ok(None)
        }
    }

    async fn shutdown(&self) -> Result<(), tower_lsp::jsonrpc::Error> {
        Ok(())
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>, tower_lsp::jsonrpc::Error> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Retrieve the document text. You might have a document cache or similar.
        // Replace this with your actual document retrieval code.
        let document_text = self.get_document_text(&uri).await.unwrap_or_default();

        if let Some(hover) = hover_wikilink::hover_wikilink(&document_text, position) {
            Ok(Some(hover))
        } else {
            Ok(None)
        }
    }
}
