//! The Properties panel's measurement of a selected solid or surface, worked out on its own thread
//! (#475): measuring a large polysurface, such as a whole house, took long enough on the main
//! thread to freeze Curva when it was selected, with only one core busy.

use curva_document::{Object, Shape};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};

/// What a body is, which measure is shown, and its value: a solid's volume or a surface's area.
pub(crate) type Reading = (&'static str, &'static str, f64);

/// The measurement held between frames.
#[derive(Default)]
pub(crate) struct Measure {
    /// The object last measured, as it was, and its reading (`None` if it has no body).
    held: Option<(Arc<Object>, Option<Reading>)>,
    /// The object being measured on another thread.
    working: Option<(Arc<Object>, Receiver<Option<Reading>>)>,
}

impl Measure {
    /// The reading for `object`, or `None` while it is being measured. An edited object is a new
    /// one, so comparing where it is held is enough to know whether the reading is still right.
    pub(crate) fn reading(&mut self, object: &Arc<Object>) -> Option<Reading> {
        self.collect();
        if let Some((held, reading)) = &self.held
            && Arc::ptr_eq(held, object)
        {
            return *reading;
        }
        let working = self
            .working
            .as_ref()
            .is_some_and(|(at, _)| Arc::ptr_eq(at, object));
        if !working {
            let (send, receive) = std::sync::mpsc::channel();
            let measured = Arc::clone(object);
            std::thread::spawn(move || {
                let _ = send.send(reading_of(&measured));
            });
            self.working = Some((Arc::clone(object), receive));
        }
        None
    }

    /// Takes a finished reading from the measuring thread.
    fn collect(&mut self) {
        let Some((_, receiver)) = &self.working else {
            return;
        };
        let reading = match receiver.try_recv() {
            Ok(reading) => reading,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => None,
        };
        if let Some((object, _)) = self.working.take() {
            self.held = Some((object, reading));
        }
    }
}

/// A body's reading: its volume if it is a closed solid, otherwise its area.
fn reading_of(object: &Object) -> Option<Reading> {
    let Shape::Body(body) = &object.shape else {
        return None;
    };
    Some(match body.volume().ok().filter(|v| *v > 0.0) {
        Some(volume) => ("ui.kind.solid", "ui.properties.volume", volume),
        None => (
            "ui.kind.surface",
            "ui.properties.area",
            body.area().unwrap_or(0.0),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_document::Document;
    use curva_kernel::Body;

    fn wait(measure: &mut Measure, object: &Arc<Object>) -> Reading {
        for _ in 0..2000 {
            if let Some(reading) = measure.reading(object) {
                return reading;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("the measurement never finished");
    }

    #[test]
    fn a_solid_is_measured_off_the_main_thread_once_per_change() {
        let mut doc = Document::default();
        doc.add_body(Body::cuboid([0.0; 3], [2.0, 3.0, 4.0]).unwrap());
        let object = Arc::clone(&doc.state().objects[0]);
        let mut measure = Measure::default();
        // The first ask starts the work and does not wait for it.
        assert!(measure.reading(&object).is_none());
        let (kind, _, volume) = wait(&mut measure, &object);
        assert_eq!(kind, "ui.kind.solid");
        assert!((volume - 24.0).abs() < 1e-6, "{volume}");
        // Asked again for the same object: answered at once, nothing new started.
        assert!(measure.reading(&object).is_some());
        assert!(measure.working.is_none());
        // A changed object is measured again.
        let mut changed = (*object).clone();
        changed.shape = Shape::Body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        let changed = Arc::new(changed);
        assert!(measure.reading(&changed).is_none());
        assert!((wait(&mut measure, &changed).2 - 1.0).abs() < 1e-6);
    }
}
