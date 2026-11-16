//! Annotation styles.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A named set of looks for annotation (spec `annotation.md` §1). It grows with dimensions and
/// leaders; text uses its font and height.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnnotationStyle {
    /// Identifies it.
    pub id: u64,
    /// Its name.
    pub name: String,
    /// A font installed on the computer, by name; Curva's own type when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// The height of capital letters, in the unit it is drawn in: millimetres for ISO styles,
    /// inches for ANSI ones (paper sizes).
    pub text_height: f64,
    /// Whether its sizes are in inches rather than millimetres.
    pub imperial: bool,
    /// Rhino's Arrow size, when set; see [`Self::arrow_size`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow_size: Option<f64>,
    /// Rhino's Extension line offset, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_offset: Option<f64>,
    /// Rhino's Extension line extension, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_extension: Option<f64>,
    /// Rhino's Text gap, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_gap: Option<f64>,
    /// Rhino's Linear resolution, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linear_resolution: Option<u8>,
}

/// The built-in styles (decision 0020, point 4): ISO text 2.5, 3.5, 5 and 7 mm (ISO 3098); ANSI
/// text 3/32", 1/8" and 3/16".
#[must_use]
pub fn standard_styles(imperial: bool) -> Vec<AnnotationStyle> {
    let make = |id: u64, name: &str, text_height: f64| AnnotationStyle {
        id,
        name: name.to_owned(),
        font: None,
        text_height,
        imperial,
        arrow_size: None,
        extension_offset: None,
        extension_extension: None,
        text_gap: None,
        linear_resolution: None,
    };
    if imperial {
        vec![
            make(0, "ANSI 1/8\"", 0.125),
            make(1, "ANSI 3/32\"", 3.0 / 32.0),
            make(2, "ANSI 3/16\"", 0.1875),
        ]
    } else {
        vec![
            make(0, "ISO 3.5", 3.5),
            make(1, "ISO 2.5", 2.5),
            make(2, "ISO 5", 5.0),
            make(3, "ISO 7", 7.0),
        ]
    }
}

impl AnnotationStyle {
    /// Its text height in a model of `unit` (on a layout, sizes are paper sizes and this is not
    /// needed).
    #[must_use]
    pub fn height_in(&self, unit: Unit) -> f64 {
        let millimetres = if self.imperial {
            self.text_height * 25.4
        } else {
            self.text_height
        };
        millimetres / unit.millimetres()
    }

    /// How many model units (or paper units on a layout of `paper`) one of the style's own size
    /// units is: its sizes are millimetres for ISO styles and inches for ANSI ones.
    #[must_use]
    pub fn scale_to(&self, unit: Unit, paper: Option<crate::layouts::PaperUnit>) -> f64 {
        let style_mm = if self.imperial { 25.4 } else { 1.0 };
        style_mm
            / paper.map_or_else(
                || unit.millimetres(),
                crate::layouts::PaperUnit::millimetres,
            )
    }

    /// Rhino's Arrow size: the arrowhead's length, the text height unless set.
    #[must_use]
    pub fn arrow_size(&self) -> f64 {
        self.arrow_size.unwrap_or(self.text_height)
    }

    /// Rhino's Extension line offset: the gap between a measured point and its extension line.
    #[must_use]
    pub fn extension_offset(&self) -> f64 {
        self.extension_offset.unwrap_or(self.text_height * 0.3)
    }

    /// Rhino's Extension line extension: how far extension lines reach past the dimension line.
    #[must_use]
    pub fn extension_extension(&self) -> f64 {
        self.extension_extension.unwrap_or(self.text_height * 0.4)
    }

    /// Rhino's Text gap: between the dimension line and its text.
    #[must_use]
    pub fn text_gap(&self) -> f64 {
        self.text_gap.unwrap_or(self.text_height * 0.3)
    }

    /// Rhino's Linear resolution: decimal places of millimetre and metre values.
    #[must_use]
    pub fn linear_resolution(&self) -> usize {
        self.linear_resolution.unwrap_or(2).into()
    }
}
