mod handlers;
mod server;

use server::NotemancyServer;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    let (service, socket) = LspService::new(|client| NotemancyServer::new(client));
    Server::new(stdin, stdout, socket).serve(service).await;
}
