use lsp_types::{DocumentSymbol, Position, Range, SymbolKind};

/// Scans the given text for markdown headings (lines starting with 1–6 '#' characters)
/// and returns a vector of DocumentSymbols.
pub fn document_symbols(text: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            // Count '#' characters to determine the heading level.
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            if level >= 1 && level <= 6 {
                // Extract heading text by removing the '#' characters and trimming whitespace.
                let heading = trimmed[level..].trim().to_string();
                // Create a DocumentSymbol for the heading.
                let symbol = DocumentSymbol {
                    name: heading,
                    detail: None,
                    // Use the Namespace kind to represent a markdown heading.
                    kind: SymbolKind::NAMESPACE,
                    range: Range {
                        start: Position {
                            line: i as u32,
                            character: 0,
                        },
                        end: Position {
                            line: i as u32,
                            character: line.len() as u32,
                        },
                    },
                    selection_range: Range {
                        start: Position {
                            line: i as u32,
                            character: 0,
                        },
                        end: Position {
                            line: i as u32,
                            character: line.len() as u32,
                        },
                    },
                    children: None,
                    // New required fields in lsp_types 0.93:
                    deprecated: None,
                    tags: None,
                };
                symbols.push(symbol);
            }
        }
    }
    symbols
}
