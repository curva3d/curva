//! Work on many bodies spread over the processor's cores, each thread's geometry engine calls on
//! an engine of its own (solidcore-ops' `with_own_kernel`), so they run side by side instead of
//! one at a time (#317).

use crate::KernelError;

/// `work` for each of `0..count`, on every core, the answers in order. Fewer than two items, or
/// one core, run here.
///
/// # Errors
///
/// The first error by item, when any item fails.
pub fn each_in_parallel<T: Send>(
    count: usize,
    work: impl Fn(usize) -> Result<T, KernelError> + Sync,
) -> Result<Vec<T>, KernelError> {
    let threads = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(count);
    if threads < 2 {
        return (0..count).map(work).collect();
    }
    let work = &work;
    let shares: Vec<Vec<(usize, Result<T, KernelError>)>> = std::thread::scope(|scope| {
        let running: Vec<_> = (0..threads)
            .map(|first| {
                scope.spawn(move || {
                    solidcore_ops::session::with_own_kernel(|| {
                        (first..count)
                            .step_by(threads)
                            .map(|i| (i, work(i)))
                            .collect()
                    })
                })
            })
            .collect();
        running
            .into_iter()
            .map(|r| r.join().unwrap_or_else(|p| std::panic::resume_unwind(p)))
            .collect()
    });
    let mut answers: Vec<Option<Result<T, KernelError>>> = (0..count).map(|_| None).collect();
    for (i, answer) in shares.into_iter().flatten() {
        answers[i] = Some(answer);
    }
    answers
        .into_iter()
        .map(|a| a.unwrap_or_else(|| Err(KernelError("an item was not worked on".to_owned()))))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Body;

    #[test]
    fn work_on_every_core_answers_in_order_as_one_at_a_time() {
        let boxes: Vec<Body> = (0..40)
            .map(|i| {
                let side = 1.0 + f64::from(i);
                Body::cuboid([0.0; 3], [side, 2.0, 3.0]).unwrap()
            })
            .collect();
        let edges = |i: usize| boxes[i].exact_edges().map(|e| e.len());
        let volumes = |i: usize| boxes[i].volume();
        let alone: Vec<f64> = (0..boxes.len()).map(|i| volumes(i).unwrap()).collect();
        assert_eq!(each_in_parallel(boxes.len(), volumes).unwrap(), alone);
        assert_eq!(each_in_parallel(boxes.len(), edges).unwrap(), vec![12; 40]);
        let failing = each_in_parallel(5, |i| {
            if i >= 3 {
                Err(KernelError(format!("item {i}")))
            } else {
                Ok(i)
            }
        });
        assert_eq!(failing.unwrap_err().0, "item 3");
    }
}
