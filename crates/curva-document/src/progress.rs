//! Long jobs and how far along they are, so Curva never looks frozen (#480): opening a file,
//! importing one, a heavy command. A job is started where the work starts; the work reports its
//! step and its count as it goes, from any thread; the interface lists the running jobs in the
//! command bar and can ask one to stop.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

/// One long job. Cloning it gives another handle to the same job; it ends when the last handle
/// is dropped.
#[derive(Clone, Debug)]
pub struct Job {
    state: Arc<JobState>,
}

#[derive(Debug)]
struct JobState {
    /// The job's name as the interface says it: its translation key and arguments.
    name: (&'static str, Vec<String>),
    /// The step it is on, the same way; `None` before the first.
    step: Mutex<Option<(&'static str, Vec<String>)>>,
    /// How many of the step's items are done, and how many there are (0 when not counted).
    done: AtomicUsize,
    total: AtomicUsize,
    /// Whether the user asked it to stop.
    stop: AtomicBool,
}

/// What the interface shows of a job.
#[derive(Clone, Debug, PartialEq)]
pub struct JobView {
    /// The job's name: its translation key and arguments.
    pub name: (&'static str, Vec<String>),
    /// The step it is on, if it has said.
    pub step: Option<(&'static str, Vec<String>)>,
    /// Items of the step done.
    pub done: usize,
    /// Items in the step in all; 0 when the step is not counted.
    pub total: usize,
}

impl JobView {
    /// How far along the step is, from 0 to 1, if it is counted.
    #[must_use]
    pub fn fraction(&self) -> Option<f32> {
        #[allow(clippy::cast_precision_loss)]
        (self.total > 0).then(|| (self.done.min(self.total) as f32) / self.total as f32)
    }
}

/// Every job started and not yet finished, oldest first.
static RUNNING: Mutex<Vec<Weak<JobState>>> = Mutex::new(Vec::new());

thread_local! {
    /// The job the work on this thread reports to, if any.
    static CURRENT: std::cell::RefCell<Option<Job>> = const { std::cell::RefCell::new(None) };
}

impl Job {
    /// Starts a job named by translation `key` and `args`; it is listed until dropped.
    #[must_use]
    pub fn start(key: &'static str, args: Vec<String>) -> Self {
        let state = Arc::new(JobState {
            name: (key, args),
            step: Mutex::new(None),
            done: AtomicUsize::new(0),
            total: AtomicUsize::new(0),
            stop: AtomicBool::new(false),
        });
        if let Ok(mut running) = RUNNING.lock() {
            running.retain(|job| job.strong_count() > 0);
            running.push(Arc::downgrade(&state));
        }
        Self { state }
    }

    /// Moves to a new step, counting `total` items (0 when not counted), none done yet.
    pub fn step(&self, key: &'static str, args: Vec<String>, total: usize) {
        if let Ok(mut step) = self.state.step.lock() {
            *step = Some((key, args));
        }
        self.state.done.store(0, Ordering::Relaxed);
        self.state.total.store(total, Ordering::Relaxed);
    }

    /// Counts one more item of the step done; safe from many threads at once.
    pub fn one_done(&self) {
        self.state.done.fetch_add(1, Ordering::Relaxed);
    }

    /// Asks the job to stop. The work stops at its next check, and its result is dropped.
    pub fn stop(&self) {
        self.state.stop.store(true, Ordering::Relaxed);
    }

    /// Whether the user asked the job to stop.
    #[must_use]
    pub fn stopping(&self) -> bool {
        self.state.stop.load(Ordering::Relaxed)
    }

    /// What the interface shows of it now.
    #[must_use]
    pub fn view(&self) -> JobView {
        view_of(&self.state)
    }

    /// Runs `work` with this job as the one this thread's work reports to (see [`current`]).
    pub fn reporting<T>(&self, work: impl FnOnce() -> T) -> T {
        let before = CURRENT.with(|c| c.replace(Some(self.clone())));
        let out = work();
        CURRENT.with(|c| *c.borrow_mut() = before);
        out
    }
}

fn view_of(state: &JobState) -> JobView {
    JobView {
        name: state.name.clone(),
        step: state.step.lock().ok().and_then(|s| s.clone()),
        done: state.done.load(Ordering::Relaxed),
        total: state.total.load(Ordering::Relaxed),
    }
}

/// The job this thread's work reports to, if it runs inside [`Job::reporting`]. Work that is
/// spread over other threads takes a clone of it with it.
#[must_use]
pub fn current() -> Option<Job> {
    CURRENT.with(|c| c.borrow().clone())
}

/// Every running job as the interface shows it, oldest first.
#[must_use]
pub fn running() -> Vec<JobView> {
    RUNNING.lock().map_or_else(
        |_| Vec::new(),
        |running| {
            running
                .iter()
                .filter_map(Weak::upgrade)
                .map(|state| view_of(&state))
                .collect()
        },
    )
}

/// Asks every running job to stop, as Escape does.
pub fn stop_all() {
    if let Ok(running) = RUNNING.lock() {
        for state in running.iter().filter_map(Weak::upgrade) {
            state.stop.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_reports_its_steps_and_counts_until_it_ends() {
        let job = Job::start("job.test_listed", vec!["model.3dm".to_owned()]);
        let mine = |jobs: Vec<JobView>| jobs.into_iter().find(|j| j.name.0 == "job.test_listed");
        let listed = mine(running()).unwrap();
        assert_eq!(listed.step, None);
        assert_eq!(listed.fraction(), None);
        job.step("job.step.solids", Vec::new(), 4);
        std::thread::scope(|s| {
            for _ in 0..2 {
                let job = job.clone();
                s.spawn(move || job.one_done());
            }
        });
        let listed = mine(running()).unwrap();
        assert_eq!(listed.step.as_ref().unwrap().0, "job.step.solids");
        assert_eq!(listed.fraction(), Some(0.5));
        drop(job);
        assert!(mine(running()).is_none(), "gone once dropped");
    }

    #[test]
    fn work_reports_to_the_job_it_runs_for_and_can_be_stopped() {
        assert!(current().is_none());
        let job = Job::start("job.test_current", Vec::new());
        let seen = job.reporting(|| current().map(|j| j.view().name.0));
        assert_eq!(seen, Some("job.test_current"));
        assert!(current().is_none(), "only inside");
        assert!(!job.stopping());
        job.stop();
        assert!(job.stopping());
    }
}
