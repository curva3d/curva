//! A command's long work, done on its own thread so the interface keeps answering (#480): the
//! command hands back [`crate::Step::Working`], the processor checks each frame whether it has
//! finished, and the command bar shows its job meanwhile.

use crate::Step;
use curva_document::Document;
use curva_document::progress::Job;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

/// What finishes the work on the model once it is done: run on the main thread, it gives the
/// command's next step.
pub type Finish = Box<dyn FnOnce(&mut Document) -> Step + Send>;

/// Work running for a command.
#[derive(Clone)]
pub struct Working {
    /// The job the command bar shows.
    pub job: Job,
    finished: Arc<Mutex<Receiver<Finish>>>,
}

impl std::fmt::Debug for Working {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Working").field("job", &self.job).finish()
    }
}

impl PartialEq for Working {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.finished, &other.finished)
    }
}

impl Working {
    /// Starts `work` on its own thread, reporting to `job`.
    pub fn spawn(job: Job, work: impl FnOnce() -> Finish + Send + 'static) -> Self {
        let (send, finished) = std::sync::mpsc::channel();
        let reporting = job.clone();
        std::thread::spawn(move || {
            let finish = reporting.reporting(work);
            let _ = send.send(finish);
        });
        Self {
            job,
            finished: Arc::new(Mutex::new(finished)),
        }
    }

    /// The finish, if the work is done; `Err` if it ended without one (it failed outright).
    pub(crate) fn try_finish(&self) -> Option<Result<Finish, ()>> {
        let finished = self.finished.lock().ok()?;
        match finished.try_recv() {
            Ok(finish) => Some(Ok(finish)),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(())),
        }
    }

    /// Waits for the work to finish, for scripts and agents, which have no frames to check on.
    pub(crate) fn wait(&self) -> Result<Finish, ()> {
        let finished = self.finished.lock().map_err(|_| ())?;
        finished.recv().map_err(|_| ())
    }
}
