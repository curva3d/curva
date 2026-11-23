//! How each kind of setting is written in a style file, and read back: yes and no, numbers,
//! `#rrggbb` colours, and plain words.

use super::*;

/// A setting's value as text in a style file.
pub(super) trait Value: Sized {
    /// The text it is written as.
    fn write(&self) -> String;
    /// The value `text` stands for; `None` when it is not one.
    fn read(text: &str) -> Option<Self>;
}

impl Value for bool {
    fn write(&self) -> String {
        if *self { "yes" } else { "no" }.into()
    }
    fn read(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "yes" | "true" | "on" => Some(true),
            "no" | "false" | "off" => Some(false),
            _ => None,
        }
    }
}

impl Value for f32 {
    fn write(&self) -> String {
        // Rust writes the shortest text that reads back as the same number.
        format!("{self}")
    }
    fn read(text: &str) -> Option<Self> {
        text.parse::<f32>().ok().filter(|v| v.is_finite())
    }
}

impl Value for String {
    fn write(&self) -> String {
        self.clone()
    }
    fn read(text: &str) -> Option<Self> {
        Some(text.to_owned())
    }
}

impl Value for Colour {
    fn write(&self) -> String {
        let [r, g, b] = self.0;
        format!("#{r:02x}{g:02x}{b:02x}")
    }
    fn read(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Self([byte(0)?, byte(2)?, byte(4)?]))
    }
}

/// `auto` for none: the standard colour, or the edges' colour, as the setting says.
impl Value for Option<Colour> {
    fn write(&self) -> String {
        self.map_or_else(|| "auto".into(), |c| c.write())
    }
    fn read(text: &str) -> Option<Self> {
        if text.eq_ignore_ascii_case("auto") {
            Some(None)
        } else {
            Colour::read(text).map(Some)
        }
    }
}

/// `auto` for the edges' colour, else as [`LineColour`].
impl Value for Option<LineColour> {
    fn write(&self) -> String {
        self.map_or_else(|| "auto".into(), |c| c.write())
    }
    fn read(text: &str) -> Option<Self> {
        if text.eq_ignore_ascii_case("auto") {
            Some(None)
        } else {
            LineColour::read(text).map(Some)
        }
    }
}

impl Value for ColourFrom {
    fn write(&self) -> String {
        match self {
            Self::Object => "object".into(),
            Self::Layer => "layer".into(),
            Self::Material => "material".into(),
            Self::One(c) => c.write(),
        }
    }
    fn read(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "object" => Some(Self::Object),
            "layer" => Some(Self::Layer),
            "material" => Some(Self::Material),
            _ => Colour::read(text).map(Self::One),
        }
    }
}

impl Value for LineColour {
    fn write(&self) -> String {
        match self {
            Self::Object => "object".into(),
            Self::One(c) => c.write(),
        }
    }
    fn read(text: &str) -> Option<Self> {
        if text.eq_ignore_ascii_case("object") {
            Some(Self::Object)
        } else {
            Colour::read(text).map(Self::One)
        }
    }
}

impl Value for BackFaces {
    fn write(&self) -> String {
        match self {
            Self::AsFront => "as front".into(),
            Self::Hidden => "hidden".into(),
            Self::One(c) => c.write(),
        }
    }
    fn read(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "as front" => Some(Self::AsFront),
            "hidden" => Some(Self::Hidden),
            _ => Colour::read(text).map(Self::One),
        }
    }
}

/// `object`, `object x1.5`, or `0.35 mm`.
impl Value for LineWidth {
    fn write(&self) -> String {
        match self {
            Self::Object(f) if *f == 1.0 => "object".into(),
            Self::Object(f) => format!("object x{}", f.write()),
            Self::Fixed(mm) => format!("{} mm", mm.write()),
        }
    }
    fn read(text: &str) -> Option<Self> {
        let text = text.trim().to_ascii_lowercase();
        if text == "object" {
            return Some(Self::Object(1.0));
        }
        if let Some(factor) = text.strip_prefix("object") {
            let factor = factor.trim().strip_prefix('x')?.trim();
            return f32::read(factor)
                .filter(|f| (0.0..=10.0).contains(f))
                .map(Self::Object);
        }
        let mm = text.strip_suffix("mm").unwrap_or(&text).trim();
        f32::read(mm)
            .filter(|mm| (0.0..=20.0).contains(mm))
            .map(Self::Fixed)
    }
}

/// Plain words for each choice, written as given and read in any case.
macro_rules! words {
    ($($kind:ty { $($variant:ident => $word:literal),+ $(,)? })+) => {$(
        impl Value for $kind {
            fn write(&self) -> String {
                match self { $(Self::$variant => $word),+ }.into()
            }
            fn read(text: &str) -> Option<Self> {
                match text.to_ascii_lowercase().as_str() { $($word => Some(Self::$variant),)+ _ => None }
            }
        }
    )+};
}

words! {
    Engine { Vector => "vector", Occlusion => "occlusion", Rendered => "rendered" }
    Fill { Theme => "theme", OneColour => "one colour", Gradient => "gradient", Paper => "paper" }
    VectorFaces { Flat => "flat", Tones => "tones" }
    OcclusionLight { Studio => "studio", Sun => "sun", Flat => "flat" }
    RenderedLight { SunAndSky => "sun and sky", Studio => "studio", ModelLights => "model lights" }
    ShadowEdge { Crisp => "crisp", Soft => "soft" }
    MaterialFrom { Material => "material", Layer => "layer", ObjectColour => "object colour" }
    Environment { Studio => "studio", Sky => "sky", Plain => "plain" }
    Pattern { Solid => "solid", Dashed => "dashed", Dotted => "dotted" }
    OutlineChoice { AsStyle => "as style", Thicker => "thicker", Off => "off" }
    WireWidth { OnePixel => "one pixel", AsEdges => "as edges" }
    LineEnds { Round => "round", Flat => "flat", Square => "square" }
    PointShape { Square => "square", Round => "round", Cross => "cross" }
    Hatch { Plain => "plain", Diagonal => "diagonal", Cross => "cross", Concrete => "concrete" }
    CutFill { Solid => "solid", Hatch => "hatch", ObjectColour => "object colour", Empty => "empty" }
    Above { Hide => "hide", Dashed => "dashed", Solid => "solid" }
    Locked { Normal => "normal", Greyed => "greyed", Ghosted => "ghosted" }
}
