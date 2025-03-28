use crate::handlers::completion::get_vault_directory;
use regex::Regex;
use std::fs;
use tower_lsp::lsp_types::*;

/// Provides a hover preview for a wiki-link.
/// When the cursor is over a wiki-link, this function extracts the relative path,
/// joins it with the vault directory, reads the file content, and returns a Hover
/// containing the full file content.
pub fn hover_wikilink(document_text: &str, position: Position) -> Option<Hover> {
    let lines: Vec<&str> = document_text.lines().collect();
    if (position.line as usize) >= lines.len() {
        return None;
    }
    let line = lines[position.line as usize];

    // Regex with named capture groups:
    // - "path": captures the relative path (up to the first '|' or closing brackets)
    // - "title": optionally captures the title if provided.
    let re = Regex::new(r"\[\[\s*(?P<path>[^|\]]+?)\s*(?:\|\s*(?P<title>[^\]]+?)\s*)?\]\]").ok()?;
    for caps in re.captures_iter(line) {
        let mat = caps.get(0)?;
        let start = mat.start();
        let end = mat.end();
        // Check if the cursor falls within the wiki-link match.
        if (position.character as usize) >= start && (position.character as usize) <= end {
            let relative_path = caps.name("path")?.as_str().trim();
            if relative_path.is_empty() {
                return None;
            }
            // Join the relative path with the vault directory.
            let vault_dir = get_vault_directory().ok()?;
            let abs_path = vault_dir.join(relative_path);
            // Read the entire file content.
            let file_content = fs::read_to_string(&abs_path).ok()?;
            // Create a Hover with the full file content as Markdown.
            let hover_contents = HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: file_content,
            });
            return Some(Hover {
                contents: hover_contents,
                // Mark the range corresponding to the wiki-link in the document.
                range: Some(Range {
                    start: Position {
                        line: position.line,
                        character: start as u32,
                    },
                    end: Position {
                        line: position.line,
                        character: end as u32,
                    },
                }),
            });
        }
    }
    None
}
