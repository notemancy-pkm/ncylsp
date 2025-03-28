use notemancy_core::notes::utils::{get_title, list_all_notes};
use serde::Deserialize;
use serde_yaml;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;

/// Configuration types corresponding to config.yaml.
#[derive(Debug, Deserialize)]
struct Vault {
    name: String,
    vault_directory: String,
    publish_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    vaults: Vec<Vault>,
    default_vault: String,
}

/// Reads the NOTEMANCY_CONF_DIR environment variable, loads config.yaml from that directory,
/// and returns the vault_directory for the default vault.
fn get_vault_directory() -> Result<PathBuf, String> {
    let conf_dir = env::var("NOTEMANCY_CONF_DIR")
        .map_err(|_| "Environment variable NOTEMANCY_CONF_DIR is not set".to_string())?;
    let config_path = Path::new(&conf_dir).join("config.yaml");
    let config_contents = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read {}: {}", config_path.display(), e))?;
    let config: ConfigFile = serde_yaml::from_str(&config_contents)
        .map_err(|e| format!("Failed to parse {}: {}", config_path.display(), e))?;
    // Find the default vault by name.
    let default_vault = config.default_vault;
    let vault = config
        .vaults
        .into_iter()
        .find(|v| v.name == default_vault)
        .ok_or_else(|| format!("Default vault '{}' not found in config", default_vault))?;
    Ok(PathBuf::from(vault.vault_directory))
}

/// Provides wiki-link completions when the trigger is detected.
/// It returns a completion item for each markdown note in the vault.
pub fn provide_wiki_link_completions(
    params: CompletionParams,
    document_text: &str,
) -> LspResult<Option<CompletionResponse>> {
    let pos = params.text_document_position.position;
    // Only offer completions if the current position is inside a wiki-link.
    if !is_inside_wiki_link(document_text, pos) {
        return Ok(None);
    }

    // Obtain the vault directory from the config.
    let vault_dir = get_vault_directory().map_err(|e| tower_lsp::jsonrpc::Error {
        code: tower_lsp::jsonrpc::ErrorCode::InternalError,
        message: e,
        data: None,
    })?;

    // List all markdown note paths (relative paths) in the vault.
    let note_paths = list_all_notes(&vault_dir, true).map_err(|err| tower_lsp::jsonrpc::Error {
        code: tower_lsp::jsonrpc::ErrorCode::InternalError,
        message: err.to_string(),
        data: None,
    })?;

    let mut items = Vec::new();

    // For each note, use get_title to extract its title, and build a completion item.
    for note in note_paths {
        let full_path = vault_dir.join(&note);
        let title = match get_title(&full_path) {
            Ok(t) => t,
            Err(_) => continue, // Skip note if its title cannot be determined.
        };
        let item = CompletionItem {
            label: title.clone(),
            kind: Some(CompletionItemKind::FILE),
            // Insert the title wrapped with wiki-link delimiters.
            insert_text: Some(format!("[[{}]]", title)),
            detail: Some(note),
            ..Default::default()
        };
        items.push(item);
    }

    Ok(Some(CompletionResponse::Array(items)))
}

/// Returns true if the cursor is considered to be “inside” a wiki-link.
/// This function looks at the current line, finds the last occurrence of "[[" before the cursor,
/// and if a closing "]]" exists it ensures the cursor is positioned before it.
fn is_inside_wiki_link(text: &str, position: Position) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    if (position.line as usize) >= lines.len() {
        return false;
    }
    let line = lines[position.line as usize];
    // Consider only the text before the cursor.
    let prefix = &line[..(position.character as usize).min(line.len())];
    if let Some(start_index) = prefix.rfind("[[") {
        // Look for a closing "]]" after the opening delimiter.
        let after_open = &line[start_index..];
        if let Some(close_offset) = after_open.find("]]") {
            let close_index = start_index + close_offset;
            // If the cursor is before the closing delimiter, we are inside.
            return (position.character as usize) <= close_index;
        } else {
            // No closing delimiter means it's a partial wiki-link.
            return true;
        }
    }
    false
}
