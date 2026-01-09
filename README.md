# CQL Language Server (Experimental)

This project is an experimental LSP (Language Server Protocol) implementation for CQL (Cassandra Query Language), written in Rust using tower-lsp and tree-sitter.

In this project, I'm attempting to bring proper code editing support for CQL, including parsing, document management, and context-aware schema-aware completions.

> [!NOTE]
> I've recently started making my own CQL tree-sitter grammar called tree_sitter_cql3 for parsing CQL statements. Check out [here](https://github.com/CleverFlare/tree-sitter-cql3).

## Why?

Working on a Node.js project that relies heavily on Cassandra, I realized the tooling is significantly lacking behind compared to SQL and noSQL support; From the lack of ORMs or query builders to tree-sitter parsers and language server support.

As a Node.js developer, I deeply value robust developer tooling. As a Rustacean, I believe I can contribute meaningfully in that space. And as a Cassandra user, I consider it a powerful yet underappreciated database that deserves better recognition in the community.

This project is a step toward improving Cassandra's tooling ecosystem and making it more hassle-free database.

## Features

- Full-document synchronization
- Incremental document updates for blazingly fast parsing
- Tree-sitter-based parsing ([`tree_sitter_cql3`](https://github.com/CleverFlare/tree-sitter-cql3))
- Keyword auto-completion (testing support for `CREATE` statements)
- AST-aware cursor position analysis
- Structured markdown documentation for completion items
- Designed for NeoVim, VS Code, and other LSP-compatible editors

## Roadmap

- [ ] Dedicated Tree Sitter grammar for better parsing support.
- [ ] Incremental parsing support.
- [ ] Context-sensitive completions.
- [ ] Schema-aware suggestions (keyspaces, tables, columns etc.).
- [ ] Diagnostics.
- [ ] Semantic analysis.
- [ ] Formatting
- [ ] Hover supports.

## Running the Server

### Prerequisites

- Rust (stable)
- Tree-sitter CQL grammar ([`tree_sitter_cql3`](https://github.com/CleverFlare/tree-sitter-cql3))
- An LSP-compatible editor (e.g., NeoVim)

### Build

```bash
cargo build
```

### Run

```bash
cargo run
```

The server communicates over `stdio` (`stdin`/`stdout`) as per Microsoft's LSP specs.

## Example NeoVim Setup

```LUA
vim.lsp.config("cqlls", {
  cmd = { "cql-lsp/target/debug/cql-lsp" },
  filetypes = { "cql" },
})

vim.lsp.enable("cqlls") -- Enabling it manually
vim.lsp.set_log_level("debug") -- Important for debugging
```

## Logging & Debugging

Due to the `vim.lsp.set_log_level` line, NeoVim will log LSP communication messages into a file.

> [!NOTE]
> The path is usually `/home/user/.local/state/nvim/lsp.log` on Linux.

Type `:LspInfo` to check that both the server is active and running correctly, and for the path of the LSP log file.

You can use the `tail` command to display and watch changes of the log file, e.g.:

```bash
tail -f /home/user/.local/state/nvim/lsp.log
```

Messages are logged using `client.log_message` in tower-lsp, which according to the docs corresponds to `window/logMessage` as per [Microsoft's LSP specs](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#window_logMessage).

## Status

**Experimental / Work in Progress**

The project is under active development and APIs may change frequently.

## Contributing

Contributions are welcome, particularly from developers with experience in Rust, LSP implementations, or Tree-sitter grammars.

This project is still in an experimental phase, so clarity, correctness, and maintainability are prioritized over feature velocity.

### Areas Where Contributions Are Most Valuable

- Dedicated tree-sitter CQL grammar for better LSP support
- Performance optimizations for large CQL files
- Editor-specific integration testing (NeoVim, VS Code, etc.)
- Documentation & examples

However, feel free to contribute in any of the unfinished features on the roadmap section, or improve/fix finished ones.
