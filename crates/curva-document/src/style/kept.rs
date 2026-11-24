//! A model's own styles (`docs/specs/views-and-styles.md` §2.4, owner's decision C): every style
//! its views use is saved inside the `.curva` file, so the model looks as intended whoever opens
//! it. Built-in styles live in the program and cannot be changed, only copied under a new name.
//! Like saved views, a model's styles are kept in the file but are not part of undo: the Style
//! panel keeps its own undo.

use super::{BUILT_IN, ViewStyle};
use crate::Document;
use std::borrow::Cow;
use std::fmt;

/// Why a style could not be saved, renamed or found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StyleError {
    /// The name is a built-in style's, which cannot be changed.
    BuiltIn(String),
    /// The name is empty, or only spaces.
    NoName,
    /// Another of the model's styles already has the name.
    Taken(String),
    /// The model has no style of that name.
    Missing(String),
}

impl fmt::Display for StyleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuiltIn(name) => write!(
                f,
                "{name} is a built-in style; save a copy under a new name"
            ),
            Self::NoName => write!(f, "a style needs a name"),
            Self::Taken(name) => write!(f, "the model already has a style called {name}"),
            Self::Missing(name) => write!(f, "the model has no style called {name}"),
        }
    }
}

impl std::error::Error for StyleError {}

/// The built-in style's name `name` is, ignoring case and spaces around it.
#[must_use]
pub fn built_in_name(name: &str) -> Option<&'static str> {
    let name = name.trim();
    BUILT_IN
        .iter()
        .copied()
        .find(|built| built.eq_ignore_ascii_case(name))
}

/// Whether two style names are the same: names ignore case and spaces around them.
fn same_name(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// A name a style may be saved under: not empty and not a built-in style's.
fn usable(name: &str) -> Result<(), StyleError> {
    if name.trim().is_empty() {
        return Err(StyleError::NoName);
    }
    match built_in_name(name) {
        Some(built) => Err(StyleError::BuiltIn(built.to_owned())),
        None => Ok(()),
    }
}

impl Document {
    /// The model's own styles, in the order they were first saved. Built-in styles are not
    /// among them unless copied under a new name.
    #[must_use]
    pub fn styles(&self) -> &[ViewStyle] {
        &self.styles
    }

    /// The model's own style called `name`, ignoring case.
    #[must_use]
    pub fn model_style(&self, name: &str) -> Option<&ViewStyle> {
        self.styles
            .iter()
            .find(|style| same_name(&style.name, name))
    }

    /// The style a view called `name` draws with: the model's own copy, or else the built-in
    /// style of that name. `None` for a name neither has.
    #[must_use]
    pub fn style_named(&self, name: &str) -> Option<Cow<'_, ViewStyle>> {
        if let Some(built) = built_in_name(name) {
            return ViewStyle::built_in(built).map(Cow::Owned);
        }
        self.model_style(name).map(Cow::Borrowed)
    }

    /// Saves `style` in the model under its name, replacing the model's style of that name, as
    /// editing a style does. A built-in style's name is refused: built-ins are only copied.
    pub fn save_style(&mut self, style: ViewStyle) -> Result<(), StyleError> {
        usable(&style.name)?;
        let mut style = style;
        style.name = style.name.trim().to_owned();
        match self
            .styles
            .iter_mut()
            .find(|held| same_name(&held.name, &style.name))
        {
            Some(held) => *held = style,
            None => self.styles.push(style),
        }
        Ok(())
    }

    /// Forgets the model's style called `name`. Returns false if it had none of that name.
    pub fn delete_style(&mut self, name: &str) -> bool {
        let before = self.styles.len();
        self.styles.retain(|style| !same_name(&style.name, name));
        self.styles.len() != before
    }

    /// Renames the model's style `from` to `to`, and the saved views drawing with it follow. A
    /// new name only differing in case is allowed.
    pub fn rename_style(&mut self, from: &str, to: &str) -> Result<(), StyleError> {
        usable(to)?;
        if !same_name(from, to) && self.model_style(to).is_some() {
            return Err(StyleError::Taken(to.trim().to_owned()));
        }
        let style = self
            .styles
            .iter_mut()
            .find(|style| same_name(&style.name, from))
            .ok_or_else(|| StyleError::Missing(from.trim().to_owned()))?;
        to.trim().clone_into(&mut style.name);
        for view in &mut self.views {
            if view
                .style
                .as_deref()
                .is_some_and(|name| same_name(name, from))
            {
                view.style = Some(to.trim().to_owned());
            }
        }
        Ok(())
    }
}

/// A model's styles in its file, each as its `.curvastyle` text, so the file keeps settings a
/// later Curva adds and a bad setting never stops the model opening.
pub(crate) mod as_text {
    use super::ViewStyle;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(crate) fn serialize<S: Serializer>(
        styles: &[ViewStyle],
        out: S,
    ) -> Result<S::Ok, S::Error> {
        styles
            .iter()
            .map(ViewStyle::to_text)
            .collect::<Vec<_>>()
            .serialize(out)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        from: D,
    ) -> Result<Vec<ViewStyle>, D::Error> {
        let texts = Vec::<String>::deserialize(from)?;
        Ok(texts
            .iter()
            .map(|text| ViewStyle::from_text(text).style)
            .collect())
    }
}
