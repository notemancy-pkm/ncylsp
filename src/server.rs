use async_trait::async_trait;
use lsp_types::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower_lsp::{Client, LanguageServer};

use crate::handlers::document_symbols::document_symbols;

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
}

#[async_trait]
impl LanguageServer for NotemancyServer {
    async fn initialize(
        &self,
        _params: InitializeParams,
    ) -> Result<InitializeResult, tower_lsp::jsonrpc::Error> {
        // Log a message when the LSP initializes.
        self.client
            .log_message(MessageType::INFO, "Notemancy LSP initialized")
            .await;

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // Register document symbol support.
                document_symbol_provider: Some(OneOf::Left(true)),
                // Use full text sync so that unsaved buffers are handled.
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            server_info: None,
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        // Notify that the server is ready.
        self.client
            .show_message(MessageType::INFO, "Notemancy LSP is ready")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let text_doc = params.text_document;
        let uri = text_doc.uri;
        let text = text_doc.text;
        self.documents.write().await.insert(uri, text);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        // Use the last change (for full sync, there is only one change)
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

        // Display a terminal spinner while processing the symbols.
        let spinner = indicatif::ProgressBar::new_spinner();
        spinner.set_message("Processing document symbols...");
        spinner.enable_steady_tick(Duration::from_millis(100));

        // Process the document to extract markdown headings.
        let symbols = document_symbols(&text);

        spinner.finish_with_message("Finished processing document symbols");

        // Return a nested response as we have a Vec<DocumentSymbol>
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn shutdown(&self) -> Result<(), tower_lsp::jsonrpc::Error> {
        Ok(())
    }
}
