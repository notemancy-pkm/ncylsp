// src/handlers/formatting_alt.rs
use pulldown_cmark::{CowStr, Event, LinkType, Options, Parser, Tag, TagEnd};
use pulldown_cmark_to_cmark::cmark;
use std::borrow::Cow;

/// The state of the WikiLink transformer.
enum State {
    /// Not currently inside a wiki-link.
    Normal,
    /// Currently inside a wiki-link. We store the destination and an accumulator for inner text.
    InWikiLink { dest: String, title: String },
}

/// An iterator adapter that transforms WikiLink events into a single Text event
/// in Obsidian style (e.g. `[[destination|title]]` when a piped title is present)
/// and otherwise passes events through unchanged.
pub struct WikiLinkTransformer<I> {
    inner: I,
    state: State,
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
                        // If we see a start of a link…
                        Event::Start(Tag::Link {
                            link_type,
                            dest_url,
                            title,
                            id: _,
                        }) => {
                            if let LinkType::WikiLink { .. } = link_type {
                                // Enter the InWikiLink state.
                                // We'll use the dest from dest_url.
                                // (Note: pulldown-cmark sometimes provides an initial title,
                                // but usually the piped text comes as inner text.)
                                self.state = State::InWikiLink {
                                    dest: dest_url.to_string(),
                                    title: title.to_string(),
                                };
                                // Do not output the start event.
                                continue;
                            } else {
                                // Not a wiki-link: pass through.
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
                        // Accumulate inner text events into our title buffer.
                        Event::Text(text) => {
                            title.push_str(&text);
                            continue;
                        }
                        // When we see the end of a link…
                        Event::End(tag_end) => {
                            // Depending on pulldown-cmark version the End event can be either a TagEnd
                            // or a Tag::Link. Here we accept both.
                            let is_link_end = match tag_end {
                                TagEnd::Link => true,
                                _ => false,
                            };
                            if is_link_end {
                                // Construct the wiki-link output.
                                let mut output = String::from("[[");
                                output.push_str(dest);
                                if !title.is_empty() {
                                    output.push_str("|");
                                    output.push_str(title);
                                }
                                output.push_str("]]");
                                // Return the single Text event and revert to Normal.
                                self.state = State::Normal;
                                return Some(Event::Text(CowStr::from(output)));
                            } else {
                                // If it's some other end tag, skip it.
                                continue;
                            }
                        }
                        // Skip any other events (like SoftBreak, HardBreak, etc.) inside a wiki-link.
                        _ => continue,
                    }
                }
            }
        }
    }
}

/// Formats the provided markdown text using pulldown_cmark_to_cmark,
/// but first transforms WikiLink events to output Obsidian-style wikilinks.
/// Returns the formatted markdown as a String or an error message.
pub fn format_markdown(text: &str) -> Result<String, String> {
    let mut options = Options::empty();
    // Enable various GitHub Flavored Markdown features.
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    options.insert(Options::ENABLE_MATH);
    options.insert(Options::ENABLE_WIKILINKS);

    let parser = Parser::new_ext(text, options);
    let transformed = WikiLinkTransformer::new(parser);
    let mut formatted = String::new();
    // Use the built‑in cmark renderer (which takes only two arguments in pulldown-cmark-to-cmark 21.0.0).
    cmark(transformed, &mut formatted).map_err(|e| e.to_string())?;
    // (Optional) Remove any unwanted escapes.
    let formatted = formatted.replace(r"\[[", "[[");
    Ok(formatted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formatting_wikilink() {
        let input = "This is a wiki-link: [[Design-Doc|Design Document]]. And some more text.";
        let output = format_markdown(input).unwrap();
        // The output should include the piped title.
        assert!(
            output.contains("[[Design-Doc|Design Document]]"),
            "Output was: {}",
            output
        );
    }
}
