//! Print widths and linetypes: how lines are drawn and printed.

#[allow(clippy::wildcard_imports)]
use crate::*;
#[allow(unused_imports)]
use serde::{Deserialize, Serialize};

/// How thick an object's lines are, as Rhino's Print Width (spec `line-weights.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum PrintWidth {
    /// As its layer says.
    #[default]
    ByLayer,
    /// The default thin line, whatever the layer says.
    Default,
    /// This many millimetres.
    Millimetres(f64),
}

impl PrintWidth {
    /// The widths Rhino offers in its list, in millimetres.
    pub const COMMON: [f64; 9] = [0.13, 0.18, 0.25, 0.35, 0.5, 0.7, 1.0, 1.4, 2.0];
    /// The thickest width that can be set, in millimetres.
    pub const MOST: f64 = 5.0;

    /// Whether it follows the layer, which files leave unwritten.
    #[must_use]
    pub fn is_by_layer(&self) -> bool {
        matches!(self, Self::ByLayer)
    }

    /// The width in millimetres an object on `layer` draws at, or `None` for the default line.
    #[must_use]
    pub fn resolved(self, layer: Option<&Layer>) -> Option<f64> {
        match self {
            Self::ByLayer => layer.and_then(|layer| layer.print_width),
            Self::Default => None,
            Self::Millimetres(mm) => Some(mm),
        }
    }

    /// A width kept to what can be set: 0 to [`Self::MOST`] millimetres, two decimals.
    #[must_use]
    pub fn clamped(mm: f64) -> f64 {
        (mm.clamp(0.0, Self::MOST) * 100.0).round() / 100.0
    }
}

/// How a line is drawn, as Rhino's linetypes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Linetype {
    /// As its layer says (objects only).
    #[default]
    ByLayer,
    /// A solid line.
    Continuous,
    /// Long dashes.
    Dashed,
    /// Dots.
    Dotted,
    /// Dashes and dots.
    DashDot,
    /// Short dashes, for hidden lines.
    Hidden,
    /// Long and short dashes, for centre lines.
    Center,
    /// Two dashes and a dot.
    Border,
    /// One of the document's own linetypes, by its place in [`State::linetypes`]: one a file
    /// brought that Curva does not have.
    Custom(u16),
}

/// A linetype the document holds beside Curva's own, as a file brought it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LinetypeDefinition {
    /// Its name.
    pub name: String,
    /// Its pattern in millimetres on paper: lengths drawn and left out in turn (a drawn length
    /// of 0 is a dot); empty for a solid line.
    pub pattern: Vec<f64>,
}

impl Linetype {
    /// The linetypes offered, after `ByLayer`.
    pub const ALL: [Self; 7] = [
        Self::Continuous,
        Self::Dashed,
        Self::Dotted,
        Self::DashDot,
        Self::Hidden,
        Self::Center,
        Self::Border,
    ];

    /// Whether it follows the layer, which files leave unwritten.
    #[must_use]
    pub fn is_by_layer(&self) -> bool {
        matches!(self, Self::ByLayer)
    }

    /// Whether it is a solid line, which files leave unwritten for layers.
    #[must_use]
    pub fn is_continuous(&self) -> bool {
        matches!(self, Self::Continuous)
    }

    /// Its name, as Rhino names it; a document's own linetype is named by
    /// [`State::linetype_name`].
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Custom(_) => "Custom",
            Self::ByLayer => "ByLayer",
            Self::Continuous => "Continuous",
            Self::Dashed => "Dashed",
            Self::Dotted => "Dots",
            Self::DashDot => "DashDot",
            Self::Hidden => "Hidden",
            Self::Center => "Center",
            Self::Border => "Border",
        }
    }

    /// The linetype with this name, ignoring case.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        std::iter::once(Self::ByLayer).chain(Self::ALL).find(|t| {
            t.name().eq_ignore_ascii_case(name)
                || (name.eq_ignore_ascii_case("dotted") && *t == Self::Dotted)
        })
    }

    /// Its pattern in millimetres on paper (as printed, and as Rhino keeps linetypes): lengths
    /// drawn and left out in turn; empty for a solid line. A document's own linetype has its
    /// pattern in the document: [`State::linetype_pattern`].
    #[must_use]
    pub fn pattern(self) -> &'static [f64] {
        match self {
            Self::ByLayer | Self::Continuous | Self::Custom(_) => &[],
            Self::Dashed => &[3.0, 1.5],
            Self::Dotted => &[0.25, 1.0],
            Self::DashDot => &[3.0, 1.0, 0.25, 1.0],
            Self::Hidden => &[1.5, 1.0],
            Self::Center => &[6.0, 1.0, 1.5, 1.0],
            Self::Border => &[3.0, 1.0, 3.0, 1.0, 0.25, 1.0],
        }
    }

    /// The linetype an object on `layer` draws with.
    #[must_use]
    pub fn resolved(self, layer: Option<&Layer>) -> Self {
        match self {
            Self::ByLayer => layer.map_or(Self::Continuous, |l| l.linetype),
            other => other,
        }
    }
}

/// Segments as files keep them (each a length and whether it is drawn, a drawn length of 0
/// being a dot) as a [`LinetypeDefinition`]'s pattern: lengths drawn and left out in turn,
/// starting with one drawn, neighbours of the same kind joined. A pattern starting with a gap
/// moves it to the end, where it repeats the same; one with nothing left out is solid (empty).
#[must_use]
pub fn alternating(segments: &[(f64, bool)]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    let mut lead_gap = 0.0;
    for &(length, drawn) in segments {
        let length = length.abs();
        if out.is_empty() && !drawn {
            lead_gap += length;
        } else if drawn == out.len().is_multiple_of(2) {
            out.push(length);
        } else if let Some(last) = out.last_mut() {
            *last += length;
        }
    }
    if lead_gap > 0.0 {
        if out.len().is_multiple_of(2) {
            if let Some(last) = out.last_mut() {
                *last += lead_gap;
            }
        } else {
            out.push(lead_gap);
        }
    }
    if out.len() < 2 { Vec::new() } else { out }
}

/// A layer's linetype when its file names none.
pub(crate) fn continuous() -> Linetype {
    Linetype::Continuous
}
