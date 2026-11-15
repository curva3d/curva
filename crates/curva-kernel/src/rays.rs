//! Rays through bodies, by solidcore's B-rep ray firing (`topology::brep::p2::ray_fire`, with a
//! limit on how far it looks): whether anything lies between two points, faces' trims
//! respected. For hidden lines (spec `make2d.md` §2, step 4). Each body is read into a kernel
//! of Curva's own here, from the engine's native text, because the ops layer keeps its kernel
//! to itself; its query keeps the B-rep's search structures from ray to ray.

use crate::{Body, KernelError, Point};
use solidcore::geom::attribute::AttributeStore;
use solidcore::geom::object::ObjectRef;
use solidcore::intersect::point_class::PointClassType;
use solidcore::kernel::Kernel;
use solidcore::query::brep::{BrepIdentity, BrepQuery};
use solidcore::topology::api_files::read_brep_from_file;
use solidcore::topology::brep::p2 as bp2;
use solidcore::topology::brep::p10::TopologyEnv;
use solidcore::topology::brep_cache::CacheObject;
use solidcore::topology::loopuse::notify_ref as key_of;
use solidcore::topology::topology::NotifyRecords;
use solidcore::{Brep, BrepId, Vec3};

/// Keys the engine gives entities read from a file, clear of any it makes itself.
const FILE_KEYS: u64 = 1 << 50;

/// The identity of an entity of a B-rep read from a file, under the reader's naming.
fn keys(object: CacheObject) -> ObjectRef {
    bp2::caller_cache_key(&key_of, object)
}

/// Read B-reps have no intermediate geometry with owners.
fn no_owner(_: ObjectRef) -> Option<ObjectRef> {
    None
}

/// The identities rays and their search structures use, as solidcore's own ray tests do.
fn identity() -> BrepIdentity<'static> {
    BrepIdentity {
        brep_id: BrepId::from_index(0),
        keys: &keys,
        owner_of: &no_owner,
    }
}

/// A body read for rays: its B-rep, the query that keeps its search structures, and the sphere
/// around it (centre and radius), which the engine's rays must start near.
struct Held {
    brep: Brep,
    query: BrepQuery<'static>,
    centre: Point,
    radius: f64,
}

/// Bodies ready for rays.
pub struct RayScene {
    kernel: Kernel,
    breps: Vec<Held>,
    /// The model's tolerance: how near the end of a ray a hit may be and still count as the
    /// end itself.
    tolerance: f64,
}

fn dot(a: Point, b: Point) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}

impl RayScene {
    /// The bodies, read into the engine for rays.
    ///
    /// # Errors
    ///
    /// When a body cannot be written or read back.
    pub fn new(bodies: &[Body]) -> Result<Self, KernelError> {
        let mut kernel = Kernel::default();
        let mut breps = Vec::new();
        for body in bodies {
            let text = body
                .engine_body()?
                .to_smb(true)
                .map_err(|e| KernelError(format!("writing a body for rays failed: {e:?}")))?;
            let mut attributes = AttributeStore::default();
            let mut records = NotifyRecords::new();
            let mut env = TopologyEnv {
                all_attributes: &[],
                attributes: &mut attributes,
                key_of: &key_of,
                records: &mut records,
                next_key: FILE_KEYS,
            };
            let mut brep = read_brep_from_file(&mut kernel, Some(text), true, false, &mut env)
                .map_err(|e| KernelError(format!("reading a body for rays failed: {e:?}")))?;
            brep.refresh_cached_topology();
            let (low, high) = body.bounds()?;
            let centre: Point = std::array::from_fn(|i| (low[i] + high[i]) / 2.0);
            let half: Point = std::array::from_fn(|i| (high[i] - low[i]) / 2.0);
            breps.push(Held {
                brep,
                query: BrepQuery::new(identity()),
                centre,
                // A little over, so a ray starting on the sphere starts outside the body.
                radius: dot(half, half).sqrt() * 1.01 + 1e-9,
            });
        }
        Ok(Self {
            kernel,
            breps,
            tolerance: 1e-6,
        })
    }

    /// Sets the model's tolerance.
    pub fn set_tolerance(&mut self, tolerance: f64) {
        self.tolerance = tolerance.max(1e-9);
    }

    /// Whether any body lies on the way from `point` along `way` (of length 1) for `far`
    /// (`f64::INFINITY` for a parallel view's eye), leaving out the face or edge `point` itself
    /// lies on.
    pub fn blocked(&mut self, point: Point, way: Point, far: f64) -> bool {
        self.blocked_short(point, way, far, 0.0)
    }

    /// As [`Self::blocked`], stopping at least `short` before `point`: for a point on a
    /// silhouette, where the ray runs along the surface it leaves and only gets clear of it
    /// some way off.
    pub fn blocked_short(&mut self, point: Point, way: Point, far: f64, short: f64) -> bool {
        let Ok(direction) = Vec3::new(way[0], way[1], way[2]).unitize() else {
            return false;
        };
        let way = [direction.x, direction.y, direction.z];
        let identity = identity();
        let back = -direction;
        for held in &mut self.breps {
            // The engine fires two bounding radii from its start. The ray starts on the eye's
            // side, where the way leaves this body's sphere (or at the eye, when nearer), and is
            // fired back towards `point`, stopping just short of it: so it never starts on a
            // face, and the face or edge `point` lies on is not reached.
            let short = (self.tolerance * 100.0).max(held.radius * 1e-6).max(short);
            let rel: Point = std::array::from_fn(|i| point[i] - held.centre[i]);
            let b = dot(rel, way);
            let disc = b * b - (dot(rel, rel) - held.radius * held.radius);
            if disc < 0.0 {
                continue;
            }
            let leave = -b + disc.sqrt();
            let top = leave.min(far);
            if top <= short * 2.0 {
                continue;
            }
            let Ok(cache) = held.query.cache(&self.kernel, &held.brep) else {
                continue;
            };
            let start = solidcore::Point3::new(
                point[0] + way[0] * top,
                point[1] + way[1] * top,
                point[2] + way[2] * top,
            );
            let fired = bp2::ray_fire(
                &self.kernel,
                &mut held.brep,
                identity.brep_id,
                cache,
                identity.keys,
                identity.owner_of,
                start,
                back,
                self.tolerance,
                Some(top - short),
            );
            // An error is a hit at the very start, which only an eye on a face gives.
            if fired.is_ok_and(|hit| hit.intersection.point_class() != PointClassType::Unknown) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ray_through_a_box_is_blocked_and_one_beside_it_is_not() {
        let cube = Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap();
        let mut scene = RayScene::new(&[cube]).unwrap();
        let up = [0.0, 0.0, 1.0];
        let far = f64::INFINITY;
        // Up from a point under the box.
        assert!(scene.blocked([5.0, 5.0, -1.0], up, far));
        // Beside it.
        assert!(!scene.blocked([20.0, 5.0, -1.0], up, far));
        // Up from the top face itself: nothing in the way.
        assert!(!scene.blocked([5.0, 5.0, 10.0], up, far));
        // Up from the middle of a bottom edge: the top face is in the way.
        assert!(scene.blocked([5.0, 0.0, 0.0], up, far));
        // Up from the middle of a top edge: clear.
        assert!(!scene.blocked([5.0, 0.0, 10.0], up, far));
        // Up from far below: the box is in the way, however far.
        assert!(scene.blocked([5.0, 5.0, -500.0], up, far));
        // But not when the eye is nearer than the box.
        assert!(!scene.blocked([5.0, 5.0, -500.0], up, 100.0));
        // From a corner of the box seen from the side, along its top face: clear.
        assert!(!scene.blocked([0.0, 0.0, 10.0], [0.0, -1.0, 0.0], far));
    }

    #[test]
    fn a_ray_past_a_hole_goes_through() {
        // A ring: a box with a square hole, by a boolean.
        let outer = Body::cuboid([0.0; 3], [30.0, 30.0, 10.0]).unwrap();
        let inner = Body::cuboid([10.0, 10.0, -1.0], [10.0, 10.0, 12.0]).unwrap();
        let ring = outer
            .combined(&inner, crate::Combine::Difference)
            .unwrap()
            .unwrap();
        let mut scene = RayScene::new(&[ring]).unwrap();
        let up = [0.0, 0.0, 1.0];
        assert!(
            !scene.blocked([15.0, 15.0, -5.0], up, f64::INFINITY),
            "through the hole"
        );
        assert!(
            scene.blocked([5.0, 5.0, -5.0], up, f64::INFINITY),
            "through the ring"
        );
    }
}
