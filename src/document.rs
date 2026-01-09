use anyhow::Result;
use ropey::{Rope, RopeSlice};
use thiserror::Error;
use tower_lsp::lsp_types::{Position, TextDocumentContentChangeEvent};
use tree_sitter::{InputEdit, Parser, Point, Tree};

pub struct TextDocument {
    pub rope: Rope,
    pub tree: Option<Tree>,
    parser: Parser,
}

#[derive(Debug, Error)]
pub enum DocumentError {
    #[error("position {0}:{1} is out of bounds")]
    PositionOutOfBounds(u32, u32),
}

/// We redeclare this enum here because the `lsp_types` crate exports a Cow
/// type that is unconvenient to deal with.
#[derive(Debug, Clone, Copy)]
pub enum PositionEncodingKind {
    #[allow(dead_code)]
    UTF8,
    UTF16,
    #[allow(dead_code)]
    UTF32,
}

impl TextDocument {
    // Creates a rope, tree, and parser from a given text (CQL code)
    pub fn new(text: &str) -> Self {
        let rope = Rope::from_str(text);

        let mut parser = Parser::new();

        let language = tree_sitter_cql3::LANGUAGE;

        // Set parser language should always succeed, but we're required to provide an error
        // message nevertheless
        parser
            .set_language(&language.into())
            .expect("Could not load language for Tree-sitter parser");

        // parser will always return a tree if the language is set properly and no timeout was
        // specified
        let tree = parser
            .parse(text, None)
            .expect("Could not parse CQL code with Tree-sitter");

        Self {
            rope,
            tree: Some(tree),
            parser,
        }
    }

    pub fn apply_content_change(
        &mut self,
        change: TextDocumentContentChangeEvent,
        position_encoding: PositionEncodingKind,
    ) -> Result<(), DocumentError> {
        match change.range {
            Some(range) => {
                // Make sure start of the line position is behind the end of the line or if on
                // the same line make sure the start character position is either the same or
                // behind the end character position
                assert!(
                    range.start.line < range.end.line
                        || (range.start.line == range.end.line
                            && range.start.character <= range.end.character)
                );

                let same_line = range.start.line == range.end.line;
                let same_character = range.start.character == range.end.character;

                let change_start_line_cu_idx = range.start.line as usize;
                let change_end_line_cu_idx = range.start.line as usize;

                // 1. Get the line at which the change starts
                let change_start_line_idx = range.start.line as usize;
                let change_start_line = match self.rope.get_line(change_start_line_idx) {
                    Some(line) => line,
                    None => {
                        return Err(DocumentError::PositionOutOfBounds(
                            range.start.line,
                            range.start.character,
                        ));
                    }
                };

                // 2. Get the line at which the change ends (Small optimization where we first
                //    check if it's the same line O(log N) lookup. we repeat this throughout this
                //    function)
                let change_end_line_idx = range.end.line as usize;
                let change_end_line = match same_line {
                    true => change_start_line,
                    false => match self.rope.get_line(change_end_line_idx) {
                        Some(line) => line,
                        None => {
                            return Err(DocumentError::PositionOutOfBounds(
                                range.end.line,
                                range.end.character,
                            ));
                        }
                    },
                };

                fn compute_char_idx(
                    position_encoding: PositionEncodingKind,
                    position: &Position,
                    slice: &RopeSlice,
                ) -> Result<usize, DocumentError> {
                    match position_encoding {
                        PositionEncodingKind::UTF8 => {
                            slice.try_byte_to_char(position.character as usize)
                        }
                        PositionEncodingKind::UTF16 => {
                            slice.try_utf16_cu_to_char(position.character as usize)
                        }
                        PositionEncodingKind::UTF32 => Ok(position.character as usize),
                    }
                    .map_err(|_| {
                        DocumentError::PositionOutOfBounds(position.line, position.character)
                    })
                }

                // 3. Compute the character offset into the start/end line where the change
                //    starts/ends
                let change_start_line_char_idx =
                    compute_char_idx(position_encoding, &range.start, &change_start_line)?;
                let change_end_line_char_idx = match same_line && same_character {
                    true => change_start_line_char_idx,
                    false => compute_char_idx(position_encoding, &range.end, &change_end_line)?,
                };

                // 4. Compute the character and byte offset into the document where the change
                //    starts/ends
                let change_start_doc_char_idx =
                    self.rope.line_to_char(change_start_line_idx) + change_start_line_char_idx;

                let change_end_doc_char_idx = match same_line && same_character {
                    true => change_start_doc_char_idx,
                    false => self.rope.line_to_char(change_end_line_idx) + change_end_line_char_idx,
                };

                let change_start_doc_byte_idx = self.rope.char_to_byte(change_start_doc_char_idx);
                let change_end_doc_byte_idx = match same_line && same_character {
                    true => change_start_doc_byte_idx,
                    false => self.rope.char_to_byte(change_end_doc_char_idx),
                };

                // 5. Compute the byte offset into the start/end line where the change starts/end.
                //    Required for tree-sitter
                let change_start_line_byte_idx = match position_encoding {
                    PositionEncodingKind::UTF8 => change_start_line_cu_idx,
                    PositionEncodingKind::UTF16 => {
                        change_end_line.char_to_utf16_cu(change_start_line_char_idx)
                    }
                    PositionEncodingKind::UTF32 => change_start_line_char_idx,
                };
                let change_end_line_byte_idx = match same_line && same_character {
                    true => change_start_line_byte_idx,
                    false => match position_encoding {
                        PositionEncodingKind::UTF8 => change_end_line_cu_idx,
                        PositionEncodingKind::UTF16 => {
                            change_end_line.char_to_utf16_cu(change_end_line_char_idx)
                        }
                        PositionEncodingKind::UTF32 => change_end_line_char_idx,
                    },
                };

                self.rope
                    .remove(change_start_doc_char_idx..change_end_doc_char_idx);

                self.rope.insert(change_start_doc_char_idx, &change.text);

                if let Some(tree) = &mut self.tree {
                    // 6. Compute the byte index into the new end line where the change ends.
                    //    Required for tree-sitter
                    let change_new_end_line_idx = self
                        .rope
                        .byte_to_line(change_start_doc_byte_idx + change.text.len());
                    let change_new_end_line_byte_idx =
                        change_start_doc_byte_idx + change.text.len();

                    // 7. Construct the tree-sitter edit. We stay mindful that tree-sitter
                    //    Point::column is a byte offset
                    let edit = InputEdit {
                        start_byte: change_start_doc_byte_idx,
                        old_end_byte: change_end_doc_byte_idx,
                        new_end_byte: change_start_doc_byte_idx + change.text.len(),
                        start_position: Point {
                            row: change_start_line_idx,
                            column: change_start_line_byte_idx,
                        },
                        old_end_position: Point {
                            row: change_end_line_idx,
                            column: change_end_line_byte_idx,
                        },
                        new_end_position: Point {
                            row: change_new_end_line_idx,
                            column: change_new_end_line_byte_idx,
                        },
                    };

                    tree.edit(&edit);

                    self.tree = Some(
                        self.parser
                            .parse(self.rope.to_string(), Some(tree))
                            .expect("Could not construct a tree for the current edit"),
                    );
                };
            }
            None => {
                self.rope = Rope::from_str(&change.text);
                self.tree = self.parser.parse(&change.text, None);
            }
        }

        Ok(())
    }
}
