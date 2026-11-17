//! The saved state of the model: everything undo takes back and a file holds.

#[allow(clippy::wildcard_imports)]
use crate::*;
#[allow(unused_imports)]
use serde::{Deserialize, Serialize};

/// Everything undo restores and a file stores.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// The objects, oldest first.
    pub objects: Vec<std::sync::Arc<Object>>,
    /// The id the next object receives. Kept so ids are not reused after an undo.
    pub next_id: u64,
    /// The layers, oldest first.
    pub layers: Vec<Layer>,
    /// The layer new objects go on.
    pub current_layer: u64,
    /// The id the next layer receives.
    pub next_layer: u64,
    /// The model's unit.
    pub unit: Unit,
    /// The id the next group receives.
    pub next_group: u64,
    /// The block definitions.
    pub blocks: Vec<crate::blocks::BlockDefinition>,
    /// The id the next block definition receives.
    pub next_block: u64,
    /// A block definition being edited in place.
    pub block_edit: Option<crate::blocks::BlockEdit>,
    /// How feet and inches are shown; files leave out the usual display.
    #[serde(default, skip_serializing_if = "units::LengthDisplay::is_standard")]
    pub length_display: units::LengthDisplay,
    /// The paper sheets, in their order (spec `layouts.md`); files leave out an empty list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layouts: Vec<layouts::Layout>,
    /// The id the next layout receives.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_layout: u64,
    /// The clipping planes (spec `layouts.md` §3); files leave out an empty list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clipping_planes: Vec<clipping::ClippingPlane>,
    /// The document's own annotation styles; empty means the standard ones for its unit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotation_styles: Vec<annotation::AnnotationStyle>,
    /// The annotation style new annotation takes, by id.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub current_annotation_style: u64,
    /// Hatch patterns the document adds to the built-in ones, as loaded from `.pat` files; a
    /// pattern of a built-in's name takes its place.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hatch_patterns: Vec<hatch::Pattern>,
    /// Rhino's document user text: keys and values kept with the file, read by the
    /// `DocumentText` field; keys starting with a full stop are hidden from lists, as in Rhino.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub document_text: std::collections::BTreeMap<String, String>,
    /// The model's notes (Rhino's `Notes`), read by the `Notes` field.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
    /// Linetypes the document holds beside Curva's own, which [`Linetype::Custom`] names by
    /// place; files leave out an empty list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linetypes: Vec<LinetypeDefinition>,
}

impl State {
    /// A linetype's name: Curva's own, or the document's.
    #[must_use]
    pub fn linetype_name(&self, linetype: Linetype) -> String {
        match linetype {
            Linetype::Custom(i) => self
                .linetypes
                .get(usize::from(i))
                .map_or_else(|| "Continuous".to_owned(), |l| l.name.clone()),
            other => other.name().to_owned(),
        }
    }

    /// A linetype's pattern in millimetres on paper: lengths drawn and left out in turn; empty
    /// for a solid line. What the viewport and printing draw with.
    #[must_use]
    pub fn linetype_pattern(&self, linetype: Linetype) -> &[f64] {
        match linetype {
            Linetype::Custom(i) => self
                .linetypes
                .get(usize::from(i))
                .map_or(&[], |l| l.pattern.as_slice()),
            other => other.pattern(),
        }
    }

    /// The linetype with this name, ignoring case: Curva's own, then the document's.
    #[must_use]
    pub fn linetype_named(&self, name: &str) -> Option<Linetype> {
        Linetype::named(name).or_else(|| {
            self.linetypes
                .iter()
                .position(|l| l.name.eq_ignore_ascii_case(name.trim()))
                .and_then(|i| u16::try_from(i).ok())
                .map(Linetype::Custom)
        })
    }

    /// A linetype with this name and pattern, as files bring them: Curva's own of that name,
    /// the document's of that name, or a new one of the document's (see
    /// [`Document::add_linetype`]).
    pub fn add_linetype(&mut self, name: &str, pattern: &[f64]) -> Linetype {
        if let Some(found) = self.linetype_named(name) {
            return found;
        }
        let Ok(index) = u16::try_from(self.linetypes.len()) else {
            return Linetype::Continuous;
        };
        self.linetypes.push(LinetypeDefinition {
            name: name.trim().to_owned(),
            pattern: pattern.to_vec(),
        });
        Linetype::Custom(index)
    }

    /// The linetypes on offer, after `ByLayer`: Curva's own, then the document's.
    #[must_use]
    pub fn linetypes_offered(&self) -> Vec<Linetype> {
        Linetype::ALL
            .into_iter()
            .chain(
                (0..self.linetypes.len())
                    .filter_map(|i| u16::try_from(i).ok().map(Linetype::Custom)),
            )
            .collect()
    }

    /// The pattern an object draws with: its own linetype's, or its layer's.
    #[must_use]
    pub fn object_pattern(&self, object: &Object) -> &[f64] {
        self.linetype_pattern(object.linetype.resolved(self.layer(object.layer)))
    }

    /// The colour an object draws with: its own, or its layer's.
    #[must_use]
    pub fn object_colour(&self, object: &Object) -> [f32; 3] {
        object.colour.unwrap_or_else(|| {
            self.layer(object.layer)
                .map_or(STANDARD_COLOUR, |l| l.colour)
        })
    }

    /// The hatch patterns on offer: the built-in ones, with the document's own added or in
    /// their place.
    #[must_use]
    pub fn patterns(&self) -> Vec<hatch::Pattern> {
        let mut all = hatch::built_in_patterns();
        for own in &self.hatch_patterns {
            match all
                .iter_mut()
                .find(|p| p.name.eq_ignore_ascii_case(&own.name))
            {
                Some(held) => *held = own.clone(),
                None => all.push(own.clone()),
            }
        }
        all
    }

    /// A layer by id.
    #[must_use]
    pub fn layer(&self, id: u64) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// A layer and its parents, the layer first. A broken or looping tree stops where it
    /// breaks.
    #[must_use]
    pub fn layer_chain(&self, id: u64) -> Vec<&Layer> {
        let mut chain = Vec::new();
        let mut next = Some(id);
        while let Some(at) = next {
            let Some(layer) = self.layer(at) else {
                break;
            };
            if chain.iter().any(|l: &&Layer| l.id == at) {
                break;
            }
            chain.push(layer);
            next = layer.parent;
        }
        chain
    }

    /// Whether a layer's objects show: it and all its parents are on (spec `sublayers.md` §1).
    /// An unknown layer shows.
    #[must_use]
    pub fn layer_shown(&self, id: u64) -> bool {
        self.layer_chain(id).iter().all(|l| l.visible)
    }

    /// Whether a layer's objects are locked: it or one of its parents is locked.
    #[must_use]
    pub fn layer_locked(&self, id: u64) -> bool {
        self.layer_chain(id).iter().any(|l| l.locked)
    }

    /// A layer's full name: the names down its tree joined by `::`, as Rhino writes them.
    #[must_use]
    pub fn layer_path(&self, id: u64) -> String {
        let mut names: Vec<&str> = self
            .layer_chain(id)
            .iter()
            .map(|l| l.name.as_str())
            .collect();
        names.reverse();
        names.join("::")
    }

    /// A layer's sublayers, in their order.
    #[must_use]
    pub fn sublayers(&self, id: u64) -> Vec<&Layer> {
        self.layers
            .iter()
            .filter(|l| l.parent == Some(id))
            .collect()
    }

    /// A layer and every layer under it, the layer first.
    #[must_use]
    pub fn layer_tree(&self, id: u64) -> Vec<u64> {
        let mut out = vec![id];
        let mut index = 0;
        while index < out.len() {
            let at = out[index];
            for child in self.sublayers(at) {
                if !out.contains(&child.id) {
                    out.push(child.id);
                }
            }
            index += 1;
        }
        out
    }

    /// The layer a name means: a full name (`Walls::Exterior`), or a plain name when only one
    /// layer has it. Case does not matter.
    #[must_use]
    pub fn layer_named(&self, name: &str) -> Option<u64> {
        let name = name.trim();
        if let Some(found) = self
            .layers
            .iter()
            .find(|l| self.layer_path(l.id).eq_ignore_ascii_case(name))
        {
            return Some(found.id);
        }
        let mut plain = self
            .layers
            .iter()
            .filter(|l| l.name.eq_ignore_ascii_case(name));
        match (plain.next(), plain.next()) {
            (Some(only), None) => Some(only.id),
            _ => None,
        }
    }
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

impl State {
    /// Whether `other` is this state, told quickly: objects are the same when they are the
    /// same shared copy. Any change to an object makes a new copy of it (`Arc::make_mut`)
    /// while an earlier state still holds the old one, so this never misses a change; it may
    /// call two equal objects different, which only costs drawing them again.
    #[must_use]
    pub fn same_as(&self, other: &Self) -> bool {
        // Listed field by field, so that a new field cannot be forgotten here.
        let Self {
            objects,
            next_id,
            layers,
            current_layer,
            next_layer,
            unit,
            next_group,
            blocks,
            next_block,
            block_edit,
            length_display,
            layouts,
            next_layout,
            clipping_planes,
            annotation_styles,
            current_annotation_style,
            hatch_patterns,
            document_text,
            notes,
            linetypes,
        } = self;
        objects.len() == other.objects.len()
            && objects
                .iter()
                .zip(&other.objects)
                .all(|(a, b)| std::sync::Arc::ptr_eq(a, b))
            && *next_id == other.next_id
            && *layers == other.layers
            && *current_layer == other.current_layer
            && *next_layer == other.next_layer
            && *unit == other.unit
            && *next_group == other.next_group
            && *blocks == other.blocks
            && *next_block == other.next_block
            && *block_edit == other.block_edit
            && *length_display == other.length_display
            && *layouts == other.layouts
            && *next_layout == other.next_layout
            && *clipping_planes == other.clipping_planes
            && *annotation_styles == other.annotation_styles
            && *current_annotation_style == other.current_annotation_style
            && *hatch_patterns == other.hatch_patterns
            && *document_text == other.document_text
            && *notes == other.notes
            && *linetypes == other.linetypes
    }
}

/// The unit a model is drawn in (spec `docs/specs/ui-look-and-layout.md` §7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    /// Millimetres, the unit new models start in.
    #[default]
    Millimetres,
    /// Metres.
    Metres,
    /// Feet, shown as feet and inches (spec `feedback-0.4.2.md` §3).
    Feet,
    /// Inches, shown as inches and fractions.
    Inches,
}

impl Unit {
    /// Its symbol, shown after numbers.
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Millimetres => "mm",
            Self::Metres => "m",
            Self::Feet => "ft",
            Self::Inches => "in",
        }
    }

    /// How many millimetres one of it is.
    #[must_use]
    pub fn millimetres(self) -> f64 {
        match self {
            Self::Millimetres => 1.0,
            Self::Metres => 1000.0,
            Self::Feet => 304.8,
            Self::Inches => 25.4,
        }
    }

    /// Every unit, in the order Settings offers them.
    pub const ALL: [Self; 4] = [Self::Millimetres, Self::Metres, Self::Feet, Self::Inches];

    /// The absolute tolerance that goes with it, as Rhino's templates set it: how far apart two
    /// points can be and still count as one.
    #[must_use]
    pub fn tolerance(self) -> f64 {
        match self {
            Self::Millimetres => 0.01,
            Self::Metres | Self::Inches => 0.001,
            Self::Feet => 0.0001,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            next_id: 0,
            layers: standard_layers(),
            current_layer: 0,
            next_layer: first_new_layer(),
            unit: Unit::Millimetres,
            next_group: 0,
            blocks: Vec::new(),
            next_block: 0,
            block_edit: None,
            length_display: units::LengthDisplay::default(),
            layouts: Vec::new(),
            next_layout: 0,
            clipping_planes: Vec::new(),
            annotation_styles: Vec::new(),
            current_annotation_style: 0,
            hatch_patterns: Vec::new(),
            document_text: std::collections::BTreeMap::new(),
            notes: String::new(),
            linetypes: Vec::new(),
        }
    }
}
