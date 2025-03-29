// src/handlers/custom_commands.rs

use regex::Regex;
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::Url;

// Use the vault lookup helper to get the vault directory.
use crate::handlers::completion::get_vault_directory;

// Import the actual CRUD functions from notemancy-core.
use notemancy_core::workspaces::crud;

/// Processes custom workspace commands in the markdown text.
/// This function looks for lines beginning with "%%" that specify a command,
/// executes the appropriate notemancy-core workspace function, and then returns
/// the text with those command lines removed.
///
/// Supported commands:
/// - %%nw workspace_name  => Create a new workspace and add the current note.
/// - %%atw workspace_name => Append the current note to the workspace.
/// - %%dfw workspace_name => Remove the current note from the workspace.
pub fn process_custom_commands(text: &str, file_uri: &Url) -> Result<String, String> {
    // Obtain the vault directory from the configuration.
    let vault_dir = get_vault_directory().map_err(|e| e)?;

    // Derive the current file path from the document URI.
    let file_path_buf: PathBuf = file_uri
        .to_file_path()
        .map_err(|_| "Invalid file URI".to_string())?;
    let file_path_str = file_path_buf
        .to_str()
        .ok_or("Failed to convert file path to string")?;

    // Regex to match commands like: %%nw workspace_name
    let command_re = Regex::new(r"^%%(nw|atw|dfw)\s+(\S+)$").map_err(|e| e.to_string())?;

    let mut cleaned_lines = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(caps) = command_re.captures(trimmed) {
            let command = caps.get(1).unwrap().as_str();
            let workspace_name = caps.get(2).unwrap().as_str();

            match command {
                "nw" => {
                    // Create a new workspace with the current note.
                    if let Err(e) =
                        crud::create_workspace(&vault_dir, workspace_name, file_path_str)
                    {
                        eprintln!(
                            "Error creating workspace '{}' with note '{}': {}",
                            workspace_name, file_path_str, e
                        );
                    }
                }
                "atw" => {
                    // Append the current note to the workspace.
                    if let Err(e) =
                        crud::append_to_workspace(&vault_dir, workspace_name, file_path_str)
                    {
                        eprintln!(
                            "Error appending note '{}' to workspace '{}': {}",
                            file_path_str, workspace_name, e
                        );
                    }
                }
                "dfw" => {
                    // Remove the current note from the workspace.
                    if let Err(e) =
                        crud::remove_from_workspace(&vault_dir, workspace_name, file_path_str)
                    {
                        eprintln!(
                            "Error removing note '{}' from workspace '{}': {}",
                            file_path_str, workspace_name, e
                        );
                    }
                }
                _ => {
                    eprintln!("Unknown command: {}", command);
                }
            }
            // Skip this command line from the output.
        } else {
            cleaned_lines.push(line);
        }
    }

    Ok(cleaned_lines.join("\n"))
}
