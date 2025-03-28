use crate::handlers::completion::get_vault_directory;
use regex::Regex;
use tower_lsp::lsp_types::*;

/// Attempts to resolve a wiki-link at the current position.
/// It looks for a pattern like:
///   [[ relative_path | title ]]
/// where whitespace is optional.
/// error messages are logged to help trace the computed path.
pub fn goto_wikilink(document_text: &str, position: Position) -> Option<Location> {
    let lines: Vec<&str> = document_text.lines().collect();
    if (position.line as usize) >= lines.len() {
        eprintln!(
            "Position.line {} out of range ({} total lines)",
            position.line,
            lines.len()
        );
        return None;
    }
    let line = lines[position.line as usize];

    // Regex with named capture groups for path and optional title.
    let re = Regex::new(r"\[\[\s*(?P<path>[^|\]]+?)\s*(?:\|\s*(?P<title>[^\]]+?)\s*)?\]\]").ok()?;
    for caps in re.captures_iter(line) {
        let mat = caps.get(0)?;
        let start = mat.start();
        let end = mat.end();

        if (position.character as usize) >= start && (position.character as usize) <= end {
            let relative_path = caps.name("path")?.as_str().trim();
            if relative_path.is_empty() {
                return None;
            }
            let vault_dir = match get_vault_directory() {
                Ok(dir) => dir,
                Err(err) => {
                    eprintln!("Failed to get vault directory: {}", err);
                    return None;
                }
            };

            let abs_path = vault_dir.join(relative_path);
            let uri = match Url::from_file_path(&abs_path) {
                Ok(u) => u,
                Err(()) => {
                    eprintln!(
                        "Failed to create file URI from path: {}",
                        abs_path.display()
                    );
                    return None;
                }
            };

            return Some(Location {
                uri,
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 0,
                    },
                },
            });
        } else {
            eprintln!(
                "Cursor position {} not within match range {}-{}",
                position.character, start, end
            );
        }
    }
    eprintln!(
        "No matching wiki-link found at cursor position {}.",
        position.character
    );
    None
}
