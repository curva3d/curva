//! Live booleans (spec `docs/specs/live-booleans.md`): a boolean's result keeps its input solids,
//! hidden, and is remade when they change.

use crate::{Document, Shape};
use serde::{Deserialize, Serialize};

/// Which boolean a live result is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiveHow {
    /// Every input joined.
    Union,
    /// The first inputs with the second ones cut away.
    Difference,
    /// Only what is inside both groups.
    Intersection,
}

/// How a live boolean's result is made, and what it was made from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiveBoolean {
    /// The operation.
    pub how: LiveHow,
    /// The first group of inputs (all of them for a union).
    pub first: Vec<u64>,
    /// The second group (cut away, or intersected with).
    pub second: Vec<u64>,
    /// A fingerprint of the inputs when the result was last made.
    pub inputs_print: u64,
    /// A fingerprint of the result as last made, to tell when it was edited.
    pub result_print: u64,
    /// Whether the inputs can no longer be combined, so the result is the last good one.
    #[serde(default)]
    pub stale: bool,
}

fn print_of(shapes: &[&Shape]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for shape in shapes {
        let text = match shape {
            Shape::Curve(c) => serde_json::to_string(c),
            Shape::Body(b) => serde_json::to_string(b),
            Shape::Instance(i) => serde_json::to_string(i),
        };
        text.unwrap_or_default().hash(&mut hasher);
    }
    hasher.finish()
}

impl Document {
    fn shape_of(&self, id: u64) -> Option<&Shape> {
        self.state
            .objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| &o.shape)
    }

    fn inputs_print(&self, live: &LiveBoolean) -> Option<u64> {
        let shapes: Option<Vec<&Shape>> = live
            .first
            .iter()
            .chain(&live.second)
            .map(|id| self.shape_of(*id))
            .collect();
        shapes.map(|s| print_of(&s))
    }

    /// Makes `result` a live boolean of `first` and `second`: the inputs are hidden and kept
    /// with it.
    pub fn make_live(&mut self, result: u64, how: LiveHow, first: Vec<u64>, second: Vec<u64>) {
        for object in &mut self.state.objects {
            if first.contains(&object.id) || second.contains(&object.id) {
                let object = std::sync::Arc::make_mut(object);
                object.hidden = true;
                object.input_of = Some(result);
            }
        }
        self.selection
            .retain(|id| !first.contains(id) && !second.contains(id));
        let mut live = LiveBoolean {
            how,
            first,
            second,
            inputs_print: 0,
            result_print: 0,
            stale: false,
        };
        live.inputs_print = self.inputs_print(&live).unwrap_or(0);
        live.result_print = self.shape_of(result).map_or(0, |s| print_of(&[s]));
        if let Some(object) = self.state.objects.iter_mut().find(|o| o.id == result) {
            std::sync::Arc::make_mut(object).live = Some(live);
        }
    }

    /// The inputs of a live boolean.
    #[must_use]
    pub fn inputs_of(&self, result: u64) -> Vec<u64> {
        self.state
            .objects
            .iter()
            .filter(|o| o.input_of == Some(result))
            .map(|o| o.id)
            .collect()
    }

    /// The objects with the inputs of any live booleans among them added, to move them
    /// together.
    #[must_use]
    pub fn with_live_inputs(&self, ids: &[u64]) -> Vec<u64> {
        let mut out = ids.to_vec();
        for object in &self.state.objects {
            if object.input_of.is_some_and(|r| ids.contains(&r)) && !out.contains(&object.id) {
                out.push(object.id);
            }
        }
        out
    }

    /// Shows a live boolean's inputs to edit them, or hides them again; returns whether they are
    /// now shown.
    pub fn toggle_inputs(&mut self, result: u64) -> bool {
        let inputs = self.inputs_of(result);
        let show = self
            .state
            .objects
            .iter()
            .filter(|o| inputs.contains(&o.id))
            .any(|o| o.hidden);
        for object in &mut self.state.objects {
            if inputs.contains(&object.id) {
                std::sync::Arc::make_mut(object).hidden = !show;
            }
        }
        if !show {
            self.selection.retain(|id| !inputs.contains(id));
        }
        show
    }

    /// Drops a live boolean's inputs and link, leaving a plain solid.
    pub fn break_live(&mut self, result: u64) {
        let inputs = self.inputs_of(result);
        self.remove(&inputs);
        if let Some(object) = self.state.objects.iter_mut().find(|o| o.id == result) {
            std::sync::Arc::make_mut(object).live = None;
        }
    }

    /// Remakes every live boolean whose inputs changed. A result edited by itself, or whose
    /// inputs are gone, becomes a plain solid; inputs whose result is gone go too.
    pub(crate) fn refresh_live(&mut self) {
        // Inputs left without their result.
        let orphans: Vec<u64> = self
            .state
            .objects
            .iter()
            .filter(|o| {
                o.input_of.is_some_and(|r| {
                    !self
                        .state
                        .objects
                        .iter()
                        .any(|x| x.id == r && x.live.is_some())
                })
            })
            .map(|o| o.id)
            .collect();
        if !orphans.is_empty() {
            self.remove(&orphans);
        }
        let results: Vec<u64> = self
            .state
            .objects
            .iter()
            .filter(|o| o.live.is_some())
            .map(|o| o.id)
            .collect();
        for result in results {
            let Some(live) = self
                .state
                .objects
                .iter()
                .find(|o| o.id == result)
                .and_then(|o| o.live.clone())
            else {
                continue;
            };
            let Some(inputs_now) = self.inputs_print(&live) else {
                self.break_live(result);
                continue;
            };
            let result_now = self.shape_of(result).map_or(0, |s| print_of(&[s]));
            if inputs_now == live.inputs_print {
                if result_now != live.result_print {
                    // The result was edited by itself: History is broken, as in Rhino.
                    self.break_live(result);
                }
                continue;
            }
            let bodies = |ids: &[u64]| -> Option<Vec<curva_kernel::Body>> {
                ids.iter()
                    .map(|id| self.shape_of(*id).and_then(|s| s.body().cloned()))
                    .collect()
            };
            let (Some(first), Some(second)) = (bodies(&live.first), bodies(&live.second)) else {
                self.break_live(result);
                continue;
            };
            let remade = combine(live.how, &first, &second);
            let object = self.state.objects.iter_mut().find(|o| o.id == result);
            let Some(object) = object else { continue };
            match remade {
                Some(body) => {
                    std::sync::Arc::make_mut(object).shape = Shape::Body(body);
                    let print = print_of(&[&object.shape]);
                    if let Some(l) = std::sync::Arc::make_mut(object).live.as_mut() {
                        l.inputs_print = inputs_now;
                        l.result_print = print;
                        l.stale = false;
                    }
                }
                None => {
                    if let Some(l) = std::sync::Arc::make_mut(object).live.as_mut() {
                        l.inputs_print = inputs_now;
                        l.stale = true;
                    }
                }
            }
        }
    }
}

/// The boolean of the two groups, or `None` when it cannot be made or leaves nothing.
fn combine(
    how: LiveHow,
    first: &[curva_kernel::Body],
    second: &[curva_kernel::Body],
) -> Option<curva_kernel::Body> {
    use curva_kernel::Combine;
    let fold = |bodies: &[curva_kernel::Body]| -> Option<curva_kernel::Body> {
        let mut whole = bodies.first()?.clone();
        for body in &bodies[1..] {
            whole = whole.combined(body, Combine::Union).ok()??;
        }
        Some(whole)
    };
    match how {
        LiveHow::Union => {
            let all: Vec<curva_kernel::Body> = first.iter().chain(second).cloned().collect();
            fold(&all)
        }
        LiveHow::Difference => {
            let mut whole = fold(first)?;
            for cut in second {
                whole = whole.combined(cut, Combine::Difference).ok()??;
            }
            Some(whole)
        }
        LiveHow::Intersection => fold(first)?
            .combined(&fold(second)?, Combine::Intersection)
            .ok()?,
    }
}

#[cfg(test)]
mod tests {
    use super::LiveHow;
    use crate::{Document, Shape};
    use curva_kernel::{Body, Combine};

    fn two_boxes() -> (Document, u64, u64, u64) {
        let mut doc = Document::default();
        let a = Body::cuboid([0.0; 3], [2.0, 2.0, 2.0]).unwrap();
        let b = Body::cuboid([1.0, 0.0, 0.0], [2.0, 2.0, 2.0]).unwrap();
        let union = a.combined(&b, Combine::Union).unwrap().unwrap();
        let ia = doc.add_body(a);
        let ib = doc.add_body(b);
        let r = doc.add_body(union);
        doc.make_live(r, LiveHow::Union, vec![ia, ib], Vec::new());
        (doc, ia, ib, r)
    }

    fn volume(doc: &Document, id: u64) -> f64 {
        doc.state()
            .objects
            .iter()
            .find(|o| o.id == id)
            .unwrap()
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap()
    }

    #[test]
    fn moving_an_input_remakes_the_result() {
        let (mut doc, _, ib, r) = two_boxes();
        assert!((volume(&doc, r) - 12.0).abs() < 1e-6);
        doc.translate(&[ib], [2.0, 0.0, 0.0]);
        doc.refresh_history();
        // Now just touching: 8 + 8.
        assert!((volume(&doc, r) - 16.0).abs() < 1e-6, "{}", volume(&doc, r));
    }

    #[test]
    fn inputs_are_hidden_kept_and_shown_on_request() {
        let (mut doc, ia, ib, r) = two_boxes();
        assert!(!doc.selectable().contains(&ia));
        assert_eq!(doc.show_all(), 0, "Show does not bring inputs back");
        assert!(doc.toggle_inputs(r));
        assert!(doc.selectable().contains(&ib));
        assert!(!doc.toggle_inputs(r));
    }

    #[test]
    fn editing_the_result_or_breaking_it_leaves_a_plain_solid() {
        let (mut doc, ia, _, r) = two_boxes();
        let moved = doc
            .state()
            .objects
            .iter()
            .find(|o| o.id == r)
            .unwrap()
            .shape
            .translated([0.0, 0.0, 5.0]);
        doc.replace_shape(r, moved);
        doc.refresh_history();
        assert!(
            doc.state()
                .objects
                .iter()
                .find(|o| o.id == r)
                .unwrap()
                .live
                .is_none()
        );
        assert!(!doc.state().objects.iter().any(|o| o.id == ia));
        let (mut doc, _, _, r) = two_boxes();
        doc.break_live(r);
        assert_eq!(doc.state().objects.len(), 1);
        assert!(matches!(doc.state().objects[0].shape, Shape::Body(_)));
    }

    #[test]
    fn a_failed_remake_keeps_the_last_result() {
        let mut doc = Document::default();
        let a = Body::cuboid([0.0; 3], [2.0, 2.0, 2.0]).unwrap();
        let b = Body::cuboid([1.0, 1.0, 1.0], [2.0, 2.0, 2.0]).unwrap();
        let both = a.combined(&b, Combine::Intersection).unwrap().unwrap();
        let (ia, ib) = (doc.add_body(a), doc.add_body(b));
        let r = doc.add_body(both);
        doc.make_live(r, LiveHow::Intersection, vec![ia], vec![ib]);
        doc.translate(&[ib], [10.0, 0.0, 0.0]);
        doc.refresh_history();
        assert!((volume(&doc, r) - 1.0).abs() < 1e-6);
        assert!(
            doc.state()
                .objects
                .iter()
                .find(|o| o.id == r)
                .unwrap()
                .live
                .as_ref()
                .unwrap()
                .stale
        );
    }
}
