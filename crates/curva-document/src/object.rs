//! One object in the model, and how it is saved.

#[allow(clippy::wildcard_imports)]
use crate::*;
#[allow(unused_imports)]
use serde::{Deserialize, Serialize};

/// One object in the model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StoredObject", into = "StoredObject")]
pub struct Object {
    /// Identifies the object for as long as the document lasts.
    pub id: u64,
    /// Its geometry.
    pub shape: Shape,
    /// The layer it is on. Files from before layers put everything on the first.
    #[serde(default)]
    pub layer: u64,
    /// Whether it is out of sight until `Show`.
    #[serde(default)]
    pub hidden: bool,
    /// Its name, empty if it has none.
    #[serde(default)]
    pub name: String,
    /// How it was made from another object, if it follows one.
    #[serde(default)]
    pub recipe: Option<Recipe>,
    /// How thick its lines are drawn and printed (spec `line-weights.md`).
    pub print_width: PrintWidth,
    /// Whether it shows but cannot be picked or changed, until `Unlock`.
    pub locked: bool,
    /// The group it belongs to, if any: picking one member picks them all.
    pub group: Option<u64>,
    /// For a live boolean's result: how it is made and from what.
    pub live: Option<crate::live::LiveBoolean>,
    /// For an input kept by a live boolean: the result it belongs to.
    pub input_of: Option<u64>,
    /// Its own colour, red, green and blue from 0 to 1; `None` takes its layer's.
    pub colour: Option<[f32; 3]>,
    /// How its lines are drawn: continuous, dashed and so on, or as its layer says.
    pub linetype: Linetype,
    /// Key–value text kept with it (`SetUserText`).
    pub user_text: std::collections::BTreeMap<String, String>,
    /// The layout whose sheet it is drawn on; `None` for the model (spec `layouts.md` §1).
    pub space: Option<u64>,
    /// Its own section style, for where clipping planes cut it; `None` takes its layer's
    /// (spec `layouts.md` §3).
    pub section: Option<crate::section::SectionStyle>,
    /// For a dimension, where each of its points is tied, in the order `tie::tied_points` gives
    /// them; empty when none is (spec `annotation.md` §3, decision 0035).
    pub ties: Vec<Option<crate::tie::Tie>>,
}

impl Object {
    /// The curve, if this object is one.
    #[must_use]
    pub fn curve(&self) -> Option<&Curve> {
        self.shape.curve()
    }
}

/// An object as a file holds it: a curve or a body under its own name, so that files from
/// before bodies, which have only `curve`, still open.
#[derive(Serialize, Deserialize)]
struct StoredObject {
    id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    curve: Option<Curve>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    body: Option<Body>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    instance: Option<crate::blocks::Instance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    annotation: Option<annotation::Annotation>,
    #[serde(default)]
    layer: u64,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    name: String,
    #[serde(default)]
    recipe: Option<Recipe>,
    #[serde(default, skip_serializing_if = "PrintWidth::is_by_layer")]
    print_width: PrintWidth,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    live: Option<crate::live::LiveBoolean>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    input_of: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    colour: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Linetype::is_by_layer")]
    linetype: Linetype,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    user_text: std::collections::BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    space: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    section: Option<crate::section::SectionStyle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    ties: Vec<Option<crate::tie::Tie>>,
}

impl TryFrom<StoredObject> for Object {
    type Error = String;

    fn try_from(stored: StoredObject) -> Result<Self, String> {
        let shape = match (
            stored.curve,
            stored.body,
            stored.instance,
            stored.annotation,
        ) {
            (Some(curve), None, None, None) => Shape::Curve(curve),
            (None, Some(body), None, None) => Shape::Body(body),
            (None, None, Some(instance), None) => Shape::Instance(instance),
            (None, None, None, Some(annotation)) => Shape::Annotation(annotation),
            _ => return Err(format!("object {} needs a curve or a body", stored.id)),
        };
        Ok(Self {
            id: stored.id,
            shape,
            layer: stored.layer,
            hidden: stored.hidden,
            name: stored.name,
            recipe: stored.recipe,
            print_width: stored.print_width,
            locked: stored.locked,
            group: stored.group,
            live: stored.live,
            input_of: stored.input_of,
            colour: stored.colour,
            linetype: stored.linetype,
            user_text: stored.user_text,
            space: stored.space,
            section: stored.section,
            ties: stored.ties,
        })
    }
}

impl From<Object> for StoredObject {
    fn from(object: Object) -> Self {
        let (curve, body, instance, annotation) = match object.shape {
            Shape::Curve(curve) => (Some(curve), None, None, None),
            Shape::Body(body) => (None, Some(body), None, None),
            Shape::Instance(instance) => (None, None, Some(instance), None),
            Shape::Annotation(annotation) => (None, None, None, Some(annotation)),
        };
        Self {
            id: object.id,
            curve,
            body,
            instance,
            annotation,
            layer: object.layer,
            hidden: object.hidden,
            name: object.name,
            recipe: object.recipe,
            print_width: object.print_width,
            locked: object.locked,
            group: object.group,
            live: object.live,
            input_of: object.input_of,
            colour: object.colour,
            linetype: object.linetype,
            user_text: object.user_text,
            space: object.space,
            section: object.section,
            ties: object.ties,
        }
    }
}

/// The colour of a new layer, red, green and blue from 0 to 1.
pub const STANDARD_COLOUR: [f32; 3] = [0.92, 0.92, 0.95];
