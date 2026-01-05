use lsp_document::{IndexedText, TextAdapter, TextMap};
use std::collections::HashMap;
use tower_lsp::{
    Client, LanguageServer, LspService, Server,
    jsonrpc::Result,
    lsp_types::{
        CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams,
        CompletionResponse, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
        DidOpenTextDocumentParams, Documentation, InitializeParams, InitializeResult,
        InitializedParams, MarkupContent, MarkupKind, MessageType, ServerCapabilities,
        TextDocumentSyncCapability, TextDocumentSyncKind,
    },
};
use tree_sitter::{Node, Parser, Point, Tree};

struct DocumentState {
    parser: Parser,
    tree: Option<Tree>,
    text: String,
}

struct Backend {
    client: Client,
    documents: tokio::sync::Mutex<HashMap<String, DocumentState>>, // uri -> document
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
        let uri = params.text_document.uri.to_string();
        self.client
            .log_message(MessageType::INFO, format!("Open URI: {}", uri))
            .await;
        let text = params.text_document.text.clone();

        let mut parser = tree_sitter::Parser::new();

        let language = tttx_tree_sitter_cql::LANGUAGE;

        parser
            .set_language(&language.into())
            .expect("Error loading CQL parser");

        let tree = parser.parse(&text, None);

        let state = DocumentState { parser, tree, text };

        self.documents.lock().await.insert(uri, state);
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        self.documents.lock().await.remove(&uri);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        if params.content_changes.len() > 1 {
            self.client
                .log_message(
                    MessageType::INFO,
                    "Incremental changes is not yet supported",
                )
                .await;

            return;
        }

        let content = params.content_changes[0].text.clone();

        let mut docs = self.documents.lock().await;

        if let Some(doc) = docs.get_mut(&uri) {
            let new_tree = doc.parser.parse(&content, None);

            self.client
                .log_message(
                    MessageType::INFO,
                    format!(
                        "NEW TREE: {}",
                        new_tree.as_ref().unwrap().root_node().to_sexp()
                    ),
                )
                .await;

            doc.tree = new_tree;

            doc.text = content;
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.to_string();

        let position = params.text_document_position.position;

        let docs = self.documents.lock().await;

        let doc = match docs.get(&uri) {
            Some(d) => d,
            None => {
                self.client
                    .log_message(MessageType::INFO, "Returning early")
                    .await;

                return Ok(None);
            }
        };

        let tree = match &doc.tree {
            Some(t) => t,
            None => return Ok(None),
        };

        let text = &doc.text;
        let text = IndexedText::new(text.clone());

        let root_node = tree.root_node();

        let position = text.lsp_pos_to_pos(&position).unwrap();

        let offset = text.pos_to_offset(&position).unwrap();

        let ts_point = Point {
            row: position.line as usize,
            column: position.col as usize,
        };

        self.client
            .log_message(MessageType::INFO, format!("Position {:?}", ts_point))
            .await;

        let node = root_node.descendant_for_byte_range(offset, offset);

        if node.is_none() {
            self.client
                .log_message(
                    MessageType::INFO,
                    "Cannot find a node corresponding to the cursor position",
                )
                .await;
        }

        self.client
            .log_message(MessageType::INFO, root_node.to_sexp())
            .await;

        self.client
            .log_message(MessageType::INFO, node.unwrap().to_sexp())
            .await;

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
        documents: Default::default(),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}
