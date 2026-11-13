//! Wording, read from the translation file so none is hard-coded in commands.

use crate::Message;
use std::collections::HashMap;

/// The wording for one language.
#[derive(Clone, Debug)]
pub struct Translations {
    entries: HashMap<String, String>,
}

impl Translations {
    /// The English wording.
    #[must_use]
    pub fn english() -> Self {
        Self::parse(include_str!("../locales/en.txt"))
    }

    fn parse(text: &str) -> Self {
        let entries = text
            .lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(|line| line.split_once(" = "))
            .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
            .collect();
        Self { entries }
    }

    /// Whether a key has wording.
    #[must_use]
    pub fn has(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    /// The finished text of a message. A missing key shows as itself, so it is visible but safe.
    #[must_use]
    pub fn render(&self, message: &Message) -> String {
        let mut text = self
            .entries
            .get(message.key)
            .cloned()
            .unwrap_or_else(|| message.key.to_owned());
        for (index, value) in message.args.iter().enumerate() {
            text = text.replace(&format!("{{{index}}}"), value);
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn keys_in(folder: &Path, found: &mut Vec<String>) {
        for entry in std::fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                keys_in(&path, found);
            } else {
                let source = std::fs::read_to_string(&path).unwrap();
                found.extend(source.split('"').filter_map(|part| {
                    let is_key = ["prompt.", "done.", "note.", "error."]
                        .iter()
                        .any(|prefix| part.starts_with(prefix));
                    let plain = part.chars().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_'
                    });
                    (is_key && plain && !part.ends_with('.')).then(|| part.to_owned())
                }));
            }
        }
    }

    #[test]
    fn every_key_used_in_the_source_has_wording() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = Vec::new();
        keys_in(&source, &mut found);
        assert!(found.len() > 20, "the scan found too little: {found:?}");
        let english = Translations::english();
        for key in found {
            assert!(english.has(&key), "no wording for {key}");
        }
    }

    #[test]
    fn placeholders_are_filled_in() {
        let english = Translations::english();
        let message = Message::with("done.polyline", vec!["3".to_owned()]);
        assert_eq!(
            english.render(&message),
            "Created a polyline with 3 points."
        );
    }
}
