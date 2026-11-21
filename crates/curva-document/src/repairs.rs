//! Solids read quickly and healed later (#476): a large file opens with its biggest polysurfaces
//! read without the engine's healer, which takes most of the time, and each is healed in the
//! background and put in its place when ready. Not saved, and not part of undo.

use crate::{Document, Shape};
use curva_kernel::Body;
use std::sync::Arc;

/// The work that heals one body read quickly; `None` if healing fails, and the quick reading
/// stays.
pub type Repair = Box<dyn FnOnce() -> Option<Body> + Send>;

/// The repairs waiting, each with the faces it heals ([`Body::faces_key`]). Behind a lock, so the
/// model can still be read from many threads at once, as Make2D does.
#[derive(Default)]
pub struct Repairs(std::sync::Mutex<Vec<(usize, Repair)>>);

impl Repairs {
    fn waiting(&mut self) -> &mut Vec<(usize, Repair)> {
        self.0
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl std::fmt::Debug for Repairs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.0.lock().map_or(0, |waiting| waiting.len());
        write!(f, "Repairs({count})")
    }
}

impl Document {
    /// Queues `repair`, which heals `quick` and every copy sharing its faces.
    pub fn queue_repair(&mut self, quick: &Body, repair: Repair) {
        self.repairs.waiting().push((quick.faces_key(), repair));
    }

    /// Queues repairs taken from another model, such as a copy a file was imported into.
    pub fn queue_repairs(&mut self, repairs: Vec<(usize, Repair)>) {
        self.repairs.waiting().extend(repairs);
    }

    /// The repairs waiting, taken to be run; each gives the faces it heals.
    pub fn take_repairs(&mut self) -> Vec<(usize, Repair)> {
        std::mem::take(self.repairs.waiting())
    }

    /// Whether repairs are waiting.
    #[must_use]
    pub fn has_repairs(&self) -> bool {
        self.repairs
            .0
            .lock()
            .is_ok_and(|waiting| !waiting.is_empty())
    }

    /// Puts `healed` in place of every body whose faces are `faces` (a quick reading), in the
    /// model and in block definitions, each kept where it was placed. Returns how many were
    /// replaced; none if they were edited or deleted meanwhile, which made them other bodies.
    pub fn apply_repair(&mut self, faces: usize, healed: &Body) -> usize {
        let mut replaced = 0;
        for object in &mut self.state.objects {
            if let Shape::Body(body) = &object.shape
                && body.faces_key() == faces
            {
                let mut copy = (**object).clone();
                copy.shape = Shape::Body(body.with_faces_of(healed));
                *object = Arc::new(copy);
                replaced += 1;
            }
        }
        for block in &mut self.state.blocks {
            for object in &mut block.objects {
                if let Shape::Body(body) = &object.shape
                    && body.faces_key() == faces
                {
                    object.shape = Shape::Body(body.with_faces_of(healed));
                    replaced += 1;
                }
            }
        }
        replaced
    }

    /// Runs every waiting repair now, on every core, and puts the results in place: for scripts,
    /// agents and tests, which want the healed model at once.
    pub fn repair_all_now(&mut self) {
        let repairs = self.take_repairs();
        let healed: Vec<(usize, Option<Body>)> = std::thread::scope(|s| {
            let handles: Vec<_> = repairs
                .into_iter()
                .map(|(faces, repair)| s.spawn(move || (faces, repair())))
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        for (faces, body) in healed {
            if let Some(body) = body {
                self.apply_repair(faces, &body);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quick_body_and_its_copies_take_the_healed_faces_where_they_stand() {
        let mut doc = Document::default();
        let quick = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        let first = doc.add_body(quick.clone());
        doc.add_body(quick.translated([5.0, 0.0, 0.0]));
        let other = doc.add_body(Body::cuboid([0.0; 3], [2.0, 2.0, 2.0]).unwrap());
        let healed = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        let healed_key = healed.faces_key();
        doc.queue_repair(&quick, Box::new(move || Some(healed)));
        assert!(doc.has_repairs());
        doc.repair_all_now();
        assert!(!doc.has_repairs());
        let bodies: Vec<&Body> = doc
            .state()
            .objects
            .iter()
            .filter_map(|o| o.shape.body())
            .collect();
        assert_eq!(bodies[0].faces_key(), healed_key);
        assert_eq!(bodies[1].faces_key(), healed_key);
        let (low, _) = bodies[1].bounds().unwrap();
        assert!((low[0] - 5.0).abs() < 1e-9, "the copy stays where it was");
        assert_ne!(
            bodies[2].faces_key(),
            healed_key,
            "another body is left alone"
        );
        let _ = (first, other);
    }

    #[test]
    fn an_edited_body_is_not_replaced() {
        let mut doc = Document::default();
        let quick = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        let key = quick.faces_key();
        doc.add_body(Body::cuboid([0.0; 3], [3.0, 1.0, 1.0]).unwrap());
        let healed = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        assert_eq!(doc.apply_repair(key, &healed), 0);
    }
}
