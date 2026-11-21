//! Healing the solids a file was read with quickly (#476), on every core in the background, with
//! the command bar showing how far along it is; each healed solid takes its place as it comes.

use curva_document::progress::Job;
use curva_document::{Document, Repair};
use curva_kernel::Body;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

/// Repairs running in the background.
pub(crate) struct Repairing {
    /// The job the command bar shows; dropping it ends the listing.
    job: Job,
    /// Each healed solid as it comes, with the faces it replaces.
    healed: Receiver<(usize, Option<Body>)>,
}

impl Repairing {
    /// Starts `repairs` on every core but one, leaving one for the interface.
    pub(crate) fn start(repairs: Vec<(usize, Repair)>) -> Self {
        let job = Job::start("job.repair", Vec::new());
        job.step("job.step.repair", Vec::new(), repairs.len());
        let queue = Arc::new(Mutex::new(repairs));
        let (send, healed) = std::sync::mpsc::channel();
        let workers = std::thread::available_parallelism()
            .map_or(1, usize::from)
            .saturating_sub(1)
            .max(1);
        for _ in 0..workers {
            let (queue, send, job) = (Arc::clone(&queue), send.clone(), job.clone());
            std::thread::spawn(move || {
                loop {
                    if job.stopping() {
                        return;
                    }
                    let next = queue.lock().ok().and_then(|mut q| q.pop());
                    let Some((faces, repair)) = next else {
                        return;
                    };
                    let body = repair();
                    job.one_done();
                    if send.send((faces, body)).is_err() {
                        return;
                    }
                }
            });
        }
        Self { job, healed }
    }

    /// Puts every solid healed since the last call in its place; returns whether repairs are
    /// still going.
    pub(crate) fn collect(&mut self, doc: &mut Document) -> bool {
        loop {
            match self.healed.try_recv() {
                Ok((faces, Some(body))) => {
                    doc.apply_repair(faces, &body);
                }
                Ok((_, None)) => {}
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => return false,
            }
        }
    }

    /// Stops the repairs still waiting; the solids keep their quick readings.
    pub(crate) fn stop(&self) {
        self.job.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repairs_run_in_the_background_and_take_their_places() {
        let mut doc = Document::default();
        let quick = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        doc.add_body(quick.clone());
        let healed = Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap();
        let healed_key = healed.faces_key();
        doc.queue_repair(&quick, Box::new(move || Some(healed)));
        let mut repairing = Repairing::start(doc.take_repairs());
        for _ in 0..2000 {
            if !repairing.collect(&mut doc) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let body = doc.state().objects[0].shape.body().unwrap();
        assert_eq!(body.faces_key(), healed_key);
    }
}
