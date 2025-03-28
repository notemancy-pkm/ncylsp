use fuse_rust::{Fuse, ScoreResult};
use notemancy_core::notes::utils::list_all_notes;
use regex::Regex;
use std::fs;
use tower_lsp::lsp_types::*;

// Re-use the vault lookup helper from our completion module.
use crate::handlers::completion::get_vault_directory;

/// Scans all markdown files in the vault to extract headings (h1–h6) as workspace symbols.
/// If `query` is nonempty, fuzzy search (using fuse‑rust) is applied on the heading texts.
pub fn get_workspace_symbols(query: &str) -> Result<Vec<SymbolInformation>, String> {
    // Get the vault directory from the config.
    let vault_dir = get_vault_directory()?;
    // List all markdown files (relative paths) in the vault.
    let note_paths = list_all_notes(&vault_dir, true).map_err(|e| e.to_string())?;
    let mut symbols = Vec::new();
    // Regex to match markdown headings (allowing for leading whitespace)
    let re = Regex::new(r"^\s*(#{1,6})\s+(.*)$").unwrap();

    for note in note_paths.iter() {
        let full_path = vault_dir.join(note);
        let content = fs::read_to_string(&full_path)
            .map_err(|e| format!("Failed to read {}: {}", full_path.display(), e))?;
        for (i, line) in content.lines().enumerate() {
            if let Some(caps) = re.captures(line) {
                let heading_text = caps.get(2).unwrap().as_str().to_string();
                let range = Range {
                    start: Position {
                        line: i as u32,
                        character: 0,
                    },
                    end: Position {
                        line: i as u32,
                        character: line.len() as u32,
                    },
                };
                let uri = Url::from_file_path(&full_path)
                    .map_err(|_| format!("Invalid file path: {}", full_path.display()))?;
                let location = Location {
                    uri,
                    range: range.clone(),
                };
                let symbol = SymbolInformation {
                    name: heading_text,
                    kind: SymbolKind::STRING,
                    location,
                    container_name: Some(note.clone()),
                    deprecated: None,
                    tags: None,
                };
                symbols.push(symbol);
            }
        }
    }

    // If a query is provided, perform fuzzy matching using fuse‑rust.
    if !query.is_empty() {
        // let fuse = Fuse::default();
        let fuse = Fuse {
            threshold: 0.3,
            location: 0,
            distance: 80,
            max_pattern_length: 32,
            is_case_sensitive: false,
            tokenize: false,
        };
        let symbol_labels: Vec<String> = symbols.iter().map(|s| s.name.clone()).collect();
        // For each label, get an optional score.
        let mut scored: Vec<(usize, f64)> = symbol_labels
            .iter()
            .enumerate()
            .filter_map(|(i, label)| {
                fuse.search_text_in_string(query, label)
                    .map(|result: ScoreResult| (i, result.score))
            })
            .collect();
        // Sort by score (lower score means a better match)
        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let matched_indices: Vec<usize> = scored.into_iter().map(|(i, _)| i).collect();
        symbols = symbols
            .into_iter()
            .enumerate()
            .filter(|(i, _)| matched_indices.contains(i))
            .map(|(_, sym)| sym)
            .collect();
    }

    Ok(symbols)
}
