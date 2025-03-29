// src/handlers/formatting.rs

use pulldown_cmark::{CowStr, Event, LinkType, Options, Parser, Tag, TagEnd};
use pulldown_cmark_to_cmark::cmark;
use std::borrow::Cow;
use tower_lsp::lsp_types::Url;

// Import our custom commands module.
use crate::handlers::custom_commands;

/// An iterator adapter that transforms WikiLink events into Obsidian‑style links.
pub struct WikiLinkTransformer<I> {
    inner: I,
    state: State,
}

enum State {
    Normal,
    InWikiLink { dest: String, title: String },
}

impl<I> WikiLinkTransformer<I> {
    pub fn new(iter: I) -> Self {
        Self {
            inner: iter,
            state: State::Normal,
        }
    }
}

impl<'a, I> Iterator for WikiLinkTransformer<I>
where
    I: Iterator<Item = Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.state {
                State::Normal => {
                    let event = self.inner.next()?;
                    match event {
                        Event::Start(Tag::Link {
                            link_type,
                            dest_url,
                            title,
                            id: _,
                        }) => {
                            if let LinkType::WikiLink { .. } = link_type {
                                self.state = State::InWikiLink {
                                    dest: dest_url.to_string(),
                                    title: title.to_string(),
                                };
                                continue;
                            } else {
                                return Some(Event::Start(Tag::Link {
                                    link_type,
                                    dest_url,
                                    title,
                                    id: Cow::Borrowed("").into(),
                                }));
                            }
                        }
                        _ => return Some(event),
                    }
                }
                State::InWikiLink {
                    ref mut dest,
                    ref mut title,
                } => {
                    let event = self.inner.next()?;
                    match event {
                        Event::Text(text) => {
                            title.push_str(&text);
                            continue;
                        }
                        Event::End(tag_end) => {
                            let is_link_end = match tag_end {
                                TagEnd::Link => true,
                                _ => false,
                            };
                            if is_link_end {
                                let mut output = String::from("[[");
                                output.push_str(dest);
                                if !title.is_empty() {
                                    output.push_str("|");
                                    output.push_str(title);
                                }
                                output.push_str("]]");
                                self.state = State::Normal;
                                return Some(Event::Text(CowStr::from(output)));
                            } else {
                                continue;
                            }
                        }
                        _ => continue,
                    }
                }
            }
        }
    }
}

/// Formats the provided markdown text.
/// First it processes any custom workspace commands (lines starting with "%%"),
/// then it parses and formats the markdown using pulldown-cmark.
/// The file_uri is used to resolve file paths for the commands.
pub fn format_markdown(text: &str, file_uri: &Url) -> Result<String, String> {
    // Process and execute any custom commands, and remove them from the text.
    let processed_text = custom_commands::process_custom_commands(text, file_uri)?;

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    options.insert(Options::ENABLE_MATH);
    options.insert(Options::ENABLE_WIKILINKS);

    let parser = Parser::new_ext(&processed_text, options);
    let transformed = WikiLinkTransformer::new(parser);
    let mut formatted = String::new();
    cmark(transformed, &mut formatted).map_err(|e| e.to_string())?;
    let formatted = formatted.replace(r"\[[", "[[");
    Ok(formatted)
}
