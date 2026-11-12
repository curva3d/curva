//! `FilletEdge`: rounds picked edges of solids by a radius, as in Rhino. `ChamferEdge` is the
//! same command bevelling by a distance, so both live here.

use super::{geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::{Document, EdgePick, Shape};
use curva_kernel::{Body, Curve, KernelError};
use std::cell::RefCell;

/// The last preview worked out: the size it was for, the result's edges and the result.
type Shown = RefCell<Option<(f64, Vec<Curve>, Vec<Body>)>>;

/// The size used until another is typed, in millimetres.
const STARTING_SIZE: f64 = 1.0;

/// The running command: rounds, or with `CHAMFER`, bevels.
#[derive(Default)]
pub struct BlendEdges<const CHAMFER: bool> {
    edges: Option<Vec<EdgePick>>,
    /// The last preview worked out, for the size it was worked out for.
    shown: Shown,
}

impl<const CHAMFER: bool> BlendEdges<CHAMFER> {
    fn memory() -> &'static str {
        if CHAMFER {
            "chamferedge.size"
        } else {
            "filletedge.size"
        }
    }

    fn size_of(doc: &Document) -> f64 {
        doc.remembered(Self::memory()).unwrap_or(STARTING_SIZE)
    }

    fn ask_edges() -> Step {
        Step::Ask(Request::new(
            if CHAMFER {
                "prompt.chamferedge.edges"
            } else {
                "prompt.filletedge.edges"
            },
            Kind::Edges,
        ))
    }

    fn ask_size(doc: &Document) -> Step {
        let key = if CHAMFER {
            "prompt.chamferedge.size"
        } else {
            "prompt.filletedge.size"
        };
        Step::Ask(Request {
            prompt: Message::with(key, vec![Self::size_of(doc).to_string()]),
            allow_empty: true,
            ..Request::new(key, Kind::Number)
        })
    }

    /// Each solid with the picked edges blended.
    fn blended(&self, doc: &Document, size: f64) -> Result<Vec<(u64, Body)>, KernelError> {
        let edges = self.edges.as_deref().unwrap_or_default();
        let mut owners: Vec<u64> = edges.iter().map(|(id, _)| *id).collect();
        owners.sort_unstable();
        owners.dedup();
        owners
            .into_iter()
            .filter_map(|id| {
                let body = doc
                    .state()
                    .objects
                    .iter()
                    .find(|o| o.id == id)?
                    .shape
                    .body()?;
                let mine: Vec<usize> = edges
                    .iter()
                    .filter(|(owner, _)| *owner == id)
                    .map(|(_, edge)| *edge)
                    .collect();
                Some(body.blended(&mine, size, CHAMFER).map(|made| (id, made)))
            })
            .collect()
    }

    fn make(&self, doc: &mut Document, size: f64) -> Step {
        match self.blended(doc, size) {
            Ok(made) => {
                let count = self.edges.as_ref().map_or(0, Vec::len);
                for (id, body) in made {
                    doc.replace_shape(id, Shape::Body(body));
                }
                let key = if CHAMFER {
                    "done.chamferedge"
                } else {
                    "done.filletedge"
                };
                Step::Done(Some(Message::with(key, vec![count.to_string()])))
            }
            Err(error) => geometry_failed(&error),
        }
    }
}

impl<const CHAMFER: bool> Command for BlendEdges<CHAMFER> {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if self.edges.is_none() {
            return match answer {
                Some(Answer::Edges(picked)) if !picked.is_empty() => {
                    self.edges = Some(picked);
                    Self::ask_size(doc)
                }
                _ => Self::ask_edges(),
            };
        }
        match answer {
            Some(Answer::Empty) => {
                let size = Self::size_of(doc);
                self.make(doc, size)
            }
            Some(Answer::Number(size)) if size > 0.0 => {
                doc.remember(Self::memory(), size);
                self.make(doc, size)
            }
            Some(Answer::Number(_)) => Step::Failed(Message::new("error.bad_distance")),
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        self.previewed(doc, live).0
    }

    fn preview_bodies(&self, doc: &Document, live: &Live) -> Vec<Body> {
        self.previewed(doc, live).1
    }
}

impl<const CHAMFER: bool> BlendEdges<CHAMFER> {
    /// The rounded or bevelled solids for the size shown, and their edges, kept while the size
    /// stays the same, as working them out takes the engine a while.
    fn previewed(&self, doc: &Document, live: &Live) -> (Vec<Curve>, Vec<Body>) {
        if self.edges.is_none() {
            return (Vec::new(), Vec::new());
        }
        let size = live
            .number
            .filter(|n| *n > 0.0)
            .unwrap_or_else(|| Self::size_of(doc));
        if let Some((held, curves, bodies)) = &*self.shown.borrow()
            && *held == size
        {
            return (curves.clone(), bodies.clone());
        }
        let bodies: Vec<Body> = self
            .blended(doc, size)
            .map(|made| made.into_iter().map(|(_, body)| body).collect())
            .unwrap_or_default();
        let curves: Vec<Curve> = bodies
            .iter()
            .flat_map(|body| body.edge_curves().unwrap_or_default())
            .collect();
        *self.shown.borrow_mut() = Some((size, curves.clone(), bodies.clone()));
        (curves, bodies)
    }
}

/// `FilletEdge`.
pub type FilletEdge = BlendEdges<false>;

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Body;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-6 * b.abs().max(1.0)
    }

    fn start(doc: &mut Document, command: &str) -> (Processor, u64) {
        let cube = Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap();
        let edge = cube.mesh().unwrap().edge_ids[0];
        let id = doc.add_body(cube);
        let mut processor = Processor::default();
        processor.submit(doc, command);
        doc.select_edges(&[(id, edge)]);
        processor.submit(doc, "");
        (processor, id)
    }

    #[test]
    fn a_picked_edge_is_rounded_and_the_solid_replaced() {
        let mut doc = Document::default();
        let (mut processor, id) = start(&mut doc, "filletedge");
        assert!(!processor.preview(&doc, None, "2").is_empty());
        processor.submit(&mut doc, "2");
        assert_eq!(doc.state().objects.len(), 1);
        assert_eq!(doc.state().objects[0].id, id);
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!(close(
            volume,
            1000.0 - (1.0 - std::f64::consts::PI / 4.0) * 40.0
        ));
        processor.submit(&mut doc, "undo");
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!(close(volume, 1000.0));
    }

    #[test]
    fn chamfer_bevels_the_edge() {
        let mut doc = Document::default();
        let (mut processor, _) = start(&mut doc, "chamferedge");
        processor.submit(&mut doc, "2");
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!(close(volume, 980.0));
    }

    #[test]
    fn without_picked_edges_it_keeps_asking() {
        let mut doc = Document::default();
        doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        let mut processor = Processor::default();
        processor.submit(&mut doc, "filletedge");
        processor.submit(&mut doc, "");
        assert!(processor.is_running());
    }
}
