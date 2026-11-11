//! `Shell`: hollows a solid to a wall thickness, opening the picked faces, as in Rhino.

use super::{geometry_failed, unexpected};
use crate::{Answer, Command, Kind, Live, Message, Request, Step};
use curva_document::{Document, FacePick, Shape};
use curva_kernel::{Body, Curve, KernelError};
use std::cell::RefCell;

/// The thickness used until another is typed, in millimetres.
const STARTING_THICKNESS: f64 = 1.0;
const MEMORY: &str = "shell.thickness";

/// The running command.
#[derive(Default)]
pub struct Shell {
    /// The solid and the faces of it to open.
    open: Option<(u64, Vec<usize>)>,
    /// The last preview worked out, for the thickness it was worked out for.
    shown: RefCell<Option<(f64, Vec<Curve>)>>,
}

fn thickness_of(doc: &Document) -> f64 {
    doc.remembered(MEMORY).unwrap_or(STARTING_THICKNESS)
}

fn ask_thickness(doc: &Document) -> Step {
    Step::Ask(Request {
        prompt: Message::with(
            "prompt.shell.thickness",
            vec![thickness_of(doc).to_string()],
        ),
        allow_empty: true,
        ..Request::new("prompt.shell.thickness", Kind::Number)
    })
}

impl Shell {
    fn shelled(&self, doc: &Document, thickness: f64) -> Result<Option<(u64, Body)>, KernelError> {
        let Some((id, faces)) = &self.open else {
            return Ok(None);
        };
        let Some(body) = doc
            .state()
            .objects
            .iter()
            .find(|o| o.id == *id)
            .and_then(|o| o.shape.body())
        else {
            return Ok(None);
        };
        body.shelled(faces, thickness).map(|made| Some((*id, made)))
    }

    fn make(&self, doc: &mut Document, thickness: f64) -> Step {
        match self.shelled(doc, thickness) {
            Ok(Some((id, body))) => {
                doc.replace_shape(id, Shape::Body(body));
                Step::Done(Some(Message::new("done.shell")))
            }
            Ok(None) => unexpected(),
            Err(error) => geometry_failed(&error),
        }
    }
}

impl Command for Shell {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if self.open.is_none() {
            let picked: Vec<FacePick> = match answer {
                Some(Answer::Faces(faces)) => faces,
                _ => return Step::Ask(Request::new("prompt.shell.faces", Kind::Faces)),
            };
            // As in Rhino, the faces opened all belong to one solid: the first one picked.
            let Some(&(id, _)) = picked.first() else {
                return Step::Ask(Request::new("prompt.shell.faces", Kind::Faces));
            };
            let faces = picked
                .iter()
                .filter(|(owner, _)| *owner == id)
                .map(|(_, face)| *face)
                .collect();
            self.open = Some((id, faces));
            return ask_thickness(doc);
        }
        match answer {
            Some(Answer::Empty) => {
                let thickness = thickness_of(doc);
                self.make(doc, thickness)
            }
            Some(Answer::Number(thickness)) if thickness > 0.0 => {
                doc.remember(MEMORY, thickness);
                self.make(doc, thickness)
            }
            Some(Answer::Number(_)) => Step::Failed(Message::new("error.bad_distance")),
            _ => unexpected(),
        }
    }

    fn preview(&self, doc: &Document, live: &Live) -> Vec<Curve> {
        if self.open.is_none() {
            return Vec::new();
        }
        let thickness = live
            .number
            .filter(|n| *n > 0.0)
            .unwrap_or_else(|| thickness_of(doc));
        if let Some((held, curves)) = &*self.shown.borrow()
            && *held == thickness
        {
            return curves.clone();
        }
        let curves = match self.shelled(doc, thickness) {
            Ok(Some((_, body))) => body.edge_curves().unwrap_or_default(),
            _ => Vec::new(),
        };
        *self.shown.borrow_mut() = Some((thickness, curves.clone()));
        curves
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Body;

    /// A 10 mm cube and the number of its top face.
    fn cube(doc: &mut Document) -> (u64, usize) {
        let body = Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap();
        let mesh = body.mesh().unwrap();
        let top = (0..mesh.triangles.len())
            .find(|&t| {
                mesh.triangles[t]
                    .iter()
                    .all(|&c| (mesh.positions[c as usize][2] - 10.0).abs() < 1e-9)
            })
            .map(|t| mesh.triangle_faces[t])
            .unwrap();
        (doc.add_body(body), top)
    }

    #[test]
    fn a_cube_opened_at_the_top_keeps_walls_of_the_thickness() {
        let mut doc = Document::default();
        let (id, top) = cube(&mut doc);
        let mut processor = Processor::default();
        processor.submit(&mut doc, "shell");
        doc.select_faces(&[(id, top)]);
        processor.submit(&mut doc, "");
        assert!(!processor.preview(&doc, None, "1").is_empty());
        processor.submit(&mut doc, "1");
        assert!(!processor.is_running());
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!((volume - (1000.0 - 8.0 * 8.0 * 9.0)).abs() < 1e-3);
        processor.submit(&mut doc, "undo");
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!((volume - 1000.0).abs() < 1e-3);
    }
}
