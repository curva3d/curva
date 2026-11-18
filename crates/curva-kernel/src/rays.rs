//! Rays through bodies, by solidcore's B-rep ray firing (`topology::brep::p2::ray_fire`, with a
//! limit on how far it looks): whether anything lies between two points, faces' trims
//! respected. For hidden lines (spec `make2d.md` §2, step 4). Each body is read into a kernel
//! of Curva's own here, from the engine's native text, because the ops layer keeps its kernel
//! to itself; its query keeps the B-rep's search structures from ray to ray.

use crate::{Body, KernelError, Point};
mod boxes;
use boxes::{BoxTree, meets};
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

/// A placement that leaves a body where it was made.
const IN_PLACE: [[f64; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

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

/// A body as made, read for rays once for all its copies: its B-rep and the query that keeps
/// its search structures.
struct Shape {
    brep: Brep,
    query: BrepQuery<'static>,
}

/// Where a copy is placed, undone: a ray at the copy is fired at its shape as made, carried back
/// by the inverse of the copy's placement (#317).
#[derive(Clone)]
struct Placed {
    /// The inverse of the placement's turn and scale, as rows.
    inverse: [[f64; 3]; 3],
    /// The placement's move.
    moved: Point,
    /// How much the placement enlarges lengths.
    scale: f64,
}

impl Placed {
    /// The inverse of `placement`, a similarity (rows 0 to 2 its axes, row 3 its move).
    fn of(placement: &[[f64; 4]; 4]) -> Option<Self> {
        let length2: f64 = (0..3).map(|k| placement[0][k] * placement[0][k]).sum();
        if length2.is_nan() || length2 <= 0.0 {
            return None;
        }
        // A similarity's rows are at right angles and alike in length, so its inverse is its
        // transpose over the length squared.
        let inverse = std::array::from_fn(|i| std::array::from_fn(|j| placement[j][i] / length2));
        Some(Self {
            inverse,
            moved: [placement[3][0], placement[3][1], placement[3][2]],
            scale: length2.sqrt(),
        })
    }

    /// A point of the scene where it is on the shape as made.
    fn point(&self, p: Point) -> Point {
        let q: Point = std::array::from_fn(|i| p[i] - self.moved[i]);
        std::array::from_fn(|j| (0..3).map(|i| q[i] * self.inverse[i][j]).sum())
    }

    /// A direction of the scene as it is on the shape as made, of length 1.
    fn direction(&self, d: Point) -> Option<Vec3> {
        let v: Point = std::array::from_fn(|j| (0..3).map(|i| d[i] * self.inverse[i][j]).sum());
        Vec3::new(v[0], v[1], v[2]).unitize().ok()
    }
}

/// A body of the scene: its shape, where that is placed, the sphere around it (centre and
/// radius), which the engine's rays must start near, and its box (low and high corners), all
/// in the scene.
#[derive(Clone)]
struct Held {
    shape: usize,
    placed: Option<Placed>,
    centre: Point,
    radius: f64,
    low: Point,
    high: Point,
}

/// Bodies ready for rays.
pub struct RayScene {
    kernel: Kernel,
    shapes: Vec<Shape>,
    breps: Vec<Held>,
    /// The bodies' boxes, grown by [`RayScene::margin`], nested.
    tree: BoxTree,
    /// The model's tolerance: how near the end of a ray a hit may be and still count as the
    /// end itself.
    tolerance: f64,
}

fn dot(a: Point, b: Point) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}

/// A body's B-rep read into `kernel` from the engine's native text.
fn read_shape(kernel: &mut Kernel, text: Vec<u8>) -> Result<Shape, KernelError> {
    let mut attributes = AttributeStore::default();
    let mut records = NotifyRecords::new();
    let mut env = TopologyEnv {
        all_attributes: &[],
        attributes: &mut attributes,
        key_of: &key_of,
        records: &mut records,
        next_key: FILE_KEYS,
    };
    let mut brep = read_brep_from_file(kernel, Some(text), true, false, &mut env)
        .map_err(|e| KernelError(format!("reading a body for rays failed: {e:?}")))?;
    brep.refresh_cached_topology();
    Ok(Shape {
        brep,
        query: BrepQuery::new(identity()),
    })
}

/// A body written in the engine's native text, for rays.
fn text_of(body: &solidcore_ops::Body) -> Result<Vec<u8>, KernelError> {
    body.to_smb(true)
        .map_err(|e| KernelError(format!("writing a body for rays failed: {e:?}")))
}

/// The bodies of a ray scene written out once, so that any number of scenes, one for each
/// thread, read them without asking the geometry engine again (#317): each body as made once,
/// however many copies of it there are; a body in place exactly as it is.
pub struct RayBodies {
    texts: Vec<Vec<u8>>,
    held: Vec<Held>,
}

impl RayBodies {
    /// The bodies, written out.
    ///
    /// # Errors
    ///
    /// When a body cannot be written or measured.
    pub fn of(bodies: &[Body]) -> Result<Self, KernelError> {
        let mut texts = Vec::new();
        let mut known: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        let mut held = Vec::new();
        for body in bodies {
            // A placement is always a similarity (copies are moved, turned, scaled evenly or
            // mirrored); in place, there is nothing to undo.
            let placement = body.placement();
            let placed = (placement != IN_PLACE)
                .then(|| Placed::of(&placement))
                .flatten();
            let shape = if placed.is_some() {
                if let Some(&shape) = known.get(&body.shape_key()) {
                    shape
                } else {
                    texts.push(text_of(&body.unplaced_engine_body()?)?);
                    known.insert(body.shape_key(), texts.len() - 1);
                    texts.len() - 1
                }
            } else {
                // In place (or placed in a way that cannot be undone): written as it is.
                texts.push(text_of(&body.engine_body()?)?);
                texts.len() - 1
            };
            let (low, high) = body.bounds()?;
            let centre: Point = std::array::from_fn(|i| (low[i] + high[i]) / 2.0);
            let half: Point = std::array::from_fn(|i| (high[i] - low[i]) / 2.0);
            held.push(Held {
                shape,
                placed,
                centre,
                // A little over, so a ray starting on the sphere starts outside the body.
                radius: dot(half, half).sqrt() * 1.01 + 1e-9,
                low,
                high,
            });
        }
        Ok(Self { texts, held })
    }
}

impl RayScene {
    /// The bodies, read into the engine for rays.
    ///
    /// # Errors
    ///
    /// When a body cannot be written or read back.
    pub fn new(bodies: &[Body]) -> Result<Self, KernelError> {
        Self::of(&RayBodies::of(bodies)?)
    }

    /// Bodies already written out, read into a kernel of this scene's own.
    ///
    /// # Errors
    ///
    /// When a body cannot be read back.
    pub fn of(bodies: &RayBodies) -> Result<Self, KernelError> {
        let mut kernel = Kernel::default();
        let shapes = bodies
            .texts
            .iter()
            .map(|text| read_shape(&mut kernel, text.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut scene = Self {
            kernel,
            shapes,
            breps: bodies.held.clone(),
            tree: BoxTree::new(&[]),
            tolerance: 1e-6,
        };
        scene.grow_boxes();
        Ok(scene)
    }

    /// Sets the model's tolerance.
    pub fn set_tolerance(&mut self, tolerance: f64) {
        self.tolerance = tolerance.max(1e-9);
        self.grow_boxes();
    }

    /// How far beyond a body's box a ray may still meet it: well over the tolerance the engine
    /// counts a hit within.
    fn margin(&self, held: &Held) -> f64 {
        self.tolerance * 100.0 + held.radius * 1e-6 + 1e-9
    }

    /// The box tree, over the boxes grown by their margins.
    fn grow_boxes(&mut self) {
        let boxes: Vec<(Point, Point)> = self
            .breps
            .iter()
            .map(|held| {
                let m = self.margin(held);
                (held.low.map(|v| v - m), held.high.map(|v| v + m))
            })
            .collect();
        self.tree = BoxTree::new(&boxes);
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
        // Only bodies whose box the ray passes through can be on its way.
        let near = self.tree.along(point, way, far);
        for index in near {
            let margin = self.margin(&self.breps[index]);
            let held = &self.breps[index];
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
            // The part of the ray this body could block, against its grown box.
            let (low, high) = (held.low.map(|v| v - margin), held.high.map(|v| v + margin));
            if !meets(low, high, point, way, top) {
                continue;
            }
            let start: Point = std::array::from_fn(|i| point[i] + way[i] * top);
            // Fired at the shape as made: for a placed copy, the ray carried back.
            let (start, back, reach, tolerance) = match &held.placed {
                None => (start, back, top - short, self.tolerance),
                Some(placed) => {
                    let Some(back) = placed.direction([back.x, back.y, back.z]) else {
                        continue;
                    };
                    let s = placed.scale;
                    (
                        placed.point(start),
                        back,
                        (top - short) / s,
                        self.tolerance / s,
                    )
                }
            };
            let shape = &mut self.shapes[held.shape];
            let Ok(cache) = shape.query.cache(&self.kernel, &shape.brep) else {
                continue;
            };
            let fired = bp2::ray_fire(
                &self.kernel,
                &mut shape.brep,
                identity.brep_id,
                cache,
                identity.keys,
                identity.owner_of,
                solidcore::Point3::new(start[0], start[1], start[2]),
                back,
                tolerance,
                Some(reach),
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

    #[test]
    fn copies_of_one_body_block_rays_where_they_stand() {
        // Three copies of one box: moved, turned a quarter about z, and doubled; each read once.
        let cube = Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap();
        let moved = cube.translated([100.0, 0.0, 0.0]);
        let turn =
            crate::Similarity::rotation([0.0; 3], [0.0, 0.0, 1.0], std::f64::consts::FRAC_PI_2);
        let turned = cube.mapped(&turn).translated([0.0, 100.0, 0.0]);
        let doubled = cube
            .mapped(&crate::Similarity::scaling([0.0; 3], 2.0))
            .translated([0.0, -100.0, 0.0]);
        let mut scene = RayScene::new(&[moved, turned, doubled]).unwrap();
        assert_eq!(scene.shapes.len(), 1);
        let up = [0.0, 0.0, 1.0];
        let far = f64::INFINITY;
        // The moved box: x 100 to 110.
        assert!(scene.blocked([105.0, 5.0, -1.0], up, far));
        assert!(!scene.blocked([95.0, 5.0, -1.0], up, far));
        assert!(!scene.blocked([105.0, 5.0, 10.0], up, far));
        assert!(scene.blocked([105.0, 0.0, 0.0], up, far));
        // The turned box: x -10 to 0, y 100 to 110.
        assert!(scene.blocked([-5.0, 105.0, -1.0], up, far));
        assert!(!scene.blocked([5.0, 105.0, -1.0], up, far));
        // The doubled box: x 0 to 20, y -100 to -80, z 0 to 20.
        assert!(scene.blocked([15.0, -85.0, -1.0], up, far));
        assert!(!scene.blocked([15.0, -85.0, 20.0], up, far));
        assert!(!scene.blocked([25.0, -85.0, -1.0], up, far));
        // Sideways through the doubled box, and stopping short of it.
        assert!(scene.blocked([-10.0, -90.0, 15.0], [1.0, 0.0, 0.0], far));
        assert!(!scene.blocked([-10.0, -90.0, 15.0], [1.0, 0.0, 0.0], 5.0));
    }
}
