//! Joining, cutting and intersecting solids.

use crate::{Body, KernelError};
use solidcore_ops::BooleanOp;
use solidcore_ops::boolean::boolean;

fn failed(what: &str, error: impl std::fmt::Debug) -> KernelError {
    KernelError(format!("{what} failed: {error:?}"))
}

/// Which way two solids are combined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Combine {
    /// Everything inside either.
    Union,
    /// The first with the second cut away.
    Difference,
    /// Only what is inside both.
    Intersection,
}

impl Body {
    /// This solid combined with `other`. `None` when nothing is left, as when solids that do
    /// not touch are intersected.
    pub fn combined(&self, other: &Self, how: Combine) -> Result<Option<Self>, KernelError> {
        let op = match how {
            Combine::Union => BooleanOp::Union,
            Combine::Difference => BooleanOp::Subtract,
            Combine::Intersection => BooleanOp::Intersect,
        };
        let made = boolean(self.engine_body()?, other.engine_body()?, op)
            .map_err(|e| failed("combining solids", e))?;
        made.map(|body| Self::made(&body)).transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::Combine;
    use crate::{Body, Similarity};

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-6 * b.abs().max(1.0)
    }

    fn cube(at: [f64; 3]) -> Body {
        Body::cuboid(at, [2.0, 2.0, 2.0]).unwrap()
    }

    #[test]
    fn overlapping_cubes_unite_cut_and_intersect() {
        let (a, b) = (cube([0.0; 3]), cube([1.0, 0.0, 0.0]));
        let volume = |how| a.combined(&b, how).unwrap().unwrap().volume().unwrap();
        assert!(close(volume(Combine::Union), 12.0));
        assert!(close(volume(Combine::Difference), 4.0));
        assert!(close(volume(Combine::Intersection), 4.0));
    }

    #[test]
    fn solids_apart_have_no_intersection() {
        let made = cube([0.0; 3])
            .combined(&cube([10.0, 0.0, 0.0]), Combine::Intersection)
            .unwrap();
        assert!(made.is_none());
    }

    #[test]
    fn turned_and_moved_solids_combine_and_the_result_moves_again() {
        let a = cube([0.0; 3]);
        let b = cube([0.0; 3])
            .mapped(&Similarity::rotation([1.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.5))
            .translated([0.5, 0.3, 0.7]);
        let united = a.combined(&b, Combine::Union).unwrap().unwrap();
        let volume = united.volume().unwrap();
        assert!(volume > 8.0 && volume < 16.0, "{volume}");
        let moved = united
            .mapped(&Similarity::rotation([0.0; 3], [1.0, 0.0, 0.0], 0.4))
            .translated([5.0, 0.0, 0.0]);
        assert!(close(moved.volume().unwrap(), volume));
        assert!(!moved.mesh().unwrap().triangles.is_empty());
        let again = moved.combined(&cube([5.0, 0.0, 0.0]), Combine::Difference);
        assert!(again.is_ok(), "{again:?}");
    }
}
