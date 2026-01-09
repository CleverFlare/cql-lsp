mod document;

use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tower_lsp::{
    Client, LanguageServer, LspService, Server,
    jsonrpc::Result,
    lsp_types::{
        CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams,
        CompletionResponse, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
        DidOpenTextDocumentParams, Documentation, InitializeParams, InitializeResult,
        InitializedParams, MarkupContent, MarkupKind, MessageType, ServerCapabilities,
        TextDocumentSyncCapability, TextDocumentSyncKind, Url,
    },
};
use tree_sitter::Node;

use crate::document::TextDocument;

struct Backend {
    client: Client,
    map: Arc<RwLock<HashMap<Url, TextDocument>>>, // uri -> document
}

/// Walk up the AST parents starting from `node` and return:
/// - the nearest statement node, OR
/// - the nearest ERROR node
///
/// Returns `None` if neither is found before reaching the root.
pub fn find_statement_or_error(mut node: Node) -> Option<Node> {
    loop {
        let kind = node.kind();

        if kind == "statement" || kind == "ERROR" {
            return Some(node);
        }

        match node.parent() {
            Some(parent) => node = parent,
            None => return None, // reached root
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![" ".into(), ".".into()]),
                    resolve_provider: Some(false),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;

        self.client
            .log_message(MessageType::INFO, format!("Open URI: {}", uri))
            .await;

        let text = params.text_document.text.clone();

        let mut wr = self.map.write().await;

        wr.insert(uri, TextDocument::new(&text));
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;

        let mut wr = self.map.write().await;

        wr.remove(&uri);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;

        let mut wr = self.map.write().await;

        if let Some(doc) = wr.get_mut(&uri) {
            for change in params.content_changes {
                doc.apply_content_change(change, document::PositionEncodingKind::UTF16)
                    .unwrap();
            }
        }
    }

    async fn completion(&self, _params: CompletionParams) -> Result<Option<CompletionResponse>> {
        // let uri = params.text_document_position.text_document.uri.to_string();
        //
        // let position = params.text_document_position.position;

        let completions = vec![
                CompletionItem {
                    label: "CREATE TABLE".into(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    documentation: Some(Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: "Creates a new table in the selected keyspace. Use `IF NOT EXISTS` to suppress the error message if the table already exists; no table is created.".to_string(),
                    })),
                    ..Default::default()
                },
                CompletionItem {
                    label: "CREATE TYPE".into(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    documentation: Some(Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: [
                            "Creates a custom data type in the keyspace that contains one or more fields of related information, such as address (street, city, state, and postal code).",
                            "\nThe scope of a user-defined type (UDT) is keyspace-wide.",
                            ">[!WARNING]IMPORTANT",
                            ">UDTs cannot contain counter fields."
                        ].join("\n"),
                    })),
                    ..Default::default()
                },
                CompletionItem {
                    label: "CREATE USER".into(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    deprecated: Some(true),
                    documentation: Some(Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: [
                            "`CREATE USER` is deprecated and included for backwards compatibility only. Authentication and authorization for DataStax Enterprise 5.0 and later are based on `ROLES`, and use `CREATE ROLE` instead.",
                            "`CREATE USER` defines a new database user account. By default users accounts do not have superuser status. Only a [superuser](https://docs.datastax.com/en/glossary/index.html#superuser) can issue `CREATE USER` requests. See [CREATE ROLE](https://docs.datastax.com/en/cql/hcd/reference/cql-commands/create-role.html) for more information about `SUPERUSER` and `NOSUPERUSER`.",
                            "User accounts are required for logging in under [internal authentication](https://docs.datastax.com/en/dse/6.9/securing/authorization-authentication/enable-unified-authentication.html) and authorization.",
                            "Enclose the user name in single quotation marks if it contains non-alphanumeric characters. You cannot recreate an existing user. To change the superuser status, password or hashed password, use [ALTER USER](https://docs.datastax.com/en/cql/hcd/reference/cql-commands/alter-user.html)."
                        ].join("\n"),
                    })),
                    ..Default::default()
                },
                ];

        Ok(Some(CompletionResponse::Array(completions)))
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        map: Default::default(),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}
