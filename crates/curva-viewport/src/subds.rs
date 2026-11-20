//! What a SubD draws as (#324, spec `mesh-and-subd.md` §4, Stage B): its smooth surface with its
//! creases and open borders, or, shown flat, its cage's own faces and edges. The mesh stream works
//! out the smooth surface (`curva_mesh::SubD::surface`); this file chooses how fine and turns the
//! cage flat.

use curva_kernel::{Curve, Point};
use curva_mesh::{Attribute, Mesh, SubD, SubDDisplay};

/// Most quads a smooth SubD is drawn with: enough for a smooth outline, few enough that working
/// out its surface stays near a tenth of a second (about 2 microseconds a quad).
const MOST_QUADS: usize = 50_000;

/// Most subdivision levels drawn: each face becomes 256 quads, as smooth as Rhino's display.
const MOST_LEVELS: u8 = 4;

/// How many times a SubD is subdivided for drawing: as many as [`MOST_LEVELS`] while it stays
/// within [`MOST_QUADS`], and at least once.
pub(crate) fn levels_for(subd: &SubD) -> u8 {
    let faces = subd.cage().faces().count().max(1);
    (1..=MOST_LEVELS)
        .rev()
        .find(|&levels| faces << (2 * u32::from(levels)) <= MOST_QUADS)
        .unwrap_or(1)
}

/// What a SubD draws as.
pub(crate) struct Drawing {
    /// The mesh it shades with.
    pub(crate) shown: Mesh,
    /// The lines it draws in every view: its creases and open borders on the smooth surface, or
    /// shown flat, all its cage's edges.
    pub(crate) lines: Vec<Curve>,
    /// The lines drawn only where faces do not show, as Rhino draws a smooth SubD in wireframe:
    /// its other cage edges on the smooth surface. None shown flat.
    pub(crate) wires: Vec<Curve>,
}

/// What a SubD draws as: shown smooth, the smooth surface with its creases and open borders on
/// it, and its other cage edges for wireframe; shown flat, or when the smooth surface cannot be
/// worked out, the cage's faces shaded flat with all its edges.
pub(crate) fn drawing(subd: &SubD) -> Drawing {
    if subd.display() == SubDDisplay::Smooth {
        let levels = levels_for(subd);
        // A closed cage without creases has no lines on its surface in every view; finding them
        // would take as long as the surface itself.
        let plain = subd.creases().next().is_none() && subd.cage().is_closed();
        let creases = if plain {
            Ok(Vec::new())
        } else {
            subd.crease_curves(levels)
        };
        let parts = (subd.surface(levels), creases, subd.edge_curves(levels));
        if let (Ok(shown), Ok(creases), Ok(edges)) = parts {
            let wires = apart_from(edges, &creases);
            let polylines = |runs: Vec<Vec<Point>>| {
                runs.into_iter()
                    .filter_map(|run| Curve::polyline(run, false).ok())
                    .collect()
            };
            return Drawing {
                shown,
                lines: polylines(creases),
                wires: polylines(wires),
            };
        }
    }
    Drawing {
        shown: flat(subd.cage()),
        lines: crate::meshes::wires(subd.cage()),
        wires: Vec::new(),
    }
}

/// The parts of `runs` not along `drawn`, so no segment is drawn twice: each run is cut where it
/// follows a segment of `drawn`, which comes from the same refined points.
fn apart_from(runs: Vec<Vec<Point>>, drawn: &[Vec<Point>]) -> Vec<Vec<Point>> {
    let key = |p: &Point| p.map(f64::to_bits);
    let segment = |a: &Point, b: &Point| {
        let (a, b) = (key(a), key(b));
        if a <= b { (a, b) } else { (b, a) }
    };
    let taken: std::collections::HashSet<_> = drawn
        .iter()
        .flat_map(|run| run.windows(2).map(|w| segment(&w[0], &w[1])))
        .collect();
    if taken.is_empty() {
        return runs;
    }
    let mut apart = Vec::new();
    for run in runs {
        let mut current: Vec<Point> = Vec::new();
        for pair in run.windows(2) {
            if taken.contains(&segment(&pair[0], &pair[1])) {
                if current.len() >= 2 {
                    apart.push(std::mem::take(&mut current));
                }
                current.clear();
            } else {
                if current.is_empty() {
                    current.push(pair[0]);
                }
                current.push(pair[1]);
            }
        }
        if current.len() >= 2 {
            apart.push(current);
        }
    }
    apart
}

/// The cage with each face's own normal at its corners, so it shades flat, face by face.
fn flat(cage: &Mesh) -> Mesh {
    let points = cage.vertices();
    let normals: Vec<Point> = cage
        .faces()
        .flat_map(|face| {
            let normal = newell(face.iter().filter_map(|&v| points.get(v as usize).copied()));
            std::iter::repeat_n(normal, face.len())
        })
        .collect();
    cage.clone()
        .with_normals(Attribute::PerCorner(normals))
        .unwrap_or_else(|_| cage.clone())
}

/// A polygon's normal of length 1 by Newell's method, which allows for corners slightly off one
/// plane; straight up for a polygon with no area.
fn newell(corners: impl Iterator<Item = Point>) -> Point {
    let corners: Vec<Point> = corners.collect();
    let mut n = [0.0; 3];
    for (k, a) in corners.iter().enumerate() {
        let b = corners[(k + 1) % corners.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let length = n.iter().map(|c| c * c).sum::<f64>().sqrt();
    if length < 1e-300 {
        [0.0, 0.0, 1.0]
    } else {
        n.map(|c| c / length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_mesh::Frame;

    fn cube() -> SubD {
        SubD::subd_box(&Frame::world(), [100.0, 100.0, 100.0], [1, 1, 1])
    }

    #[test]
    fn a_subd_is_drawn_as_finely_as_its_size_allows() {
        assert_eq!(levels_for(&cube()), MOST_LEVELS);
        let many = SubD::subd_plane(&Frame::world(), [100.0, 100.0], [49, 49]);
        // 2,401 faces: two levels make 38,416 quads, three would make 153,664.
        assert_eq!(levels_for(&many), 2);
        let huge = SubD::subd_plane(&Frame::world(), [100.0, 100.0], [200, 200]);
        assert_eq!(levels_for(&huge), 1);
    }

    /// Shown smooth, a cube's cage draws as a rounded surface inside the cage, with no lines in
    /// every view until it has creases; its creases draw on the surface.
    #[test]
    fn a_smooth_subd_draws_its_surface_and_its_creases() {
        let (low, high) = cube().bounds().unwrap();
        let middle: Point = std::array::from_fn(|k| (low[k] + high[k]) / 2.0);
        let Drawing {
            shown: surface,
            lines,
            ..
        } = drawing(&cube());
        assert!(lines.is_empty());
        assert!(surface.is_closed());
        // Rounded: the surface keeps well inside the cage's corners.
        let farthest = surface
            .vertices()
            .iter()
            .map(|p| {
                (0..3)
                    .map(|k| (p[k] - middle[k]).powi(2))
                    .sum::<f64>()
                    .sqrt()
            })
            .fold(0.0, f64::max);
        assert!(farthest < 50.0 * 3f64.sqrt() * 0.9, "{farthest}");
        let creased = cube().with_creases(&[[0, 1]]);
        assert!(!drawing(&creased).lines.is_empty());
    }

    /// Shown flat, the cage's own faces shade flat, each with its own normal, and all twelve of
    /// a cube's edges draw.
    #[test]
    fn a_flat_subd_draws_its_cage() {
        let flat_cube = cube().with_display(SubDDisplay::Flat);
        let Drawing {
            shown,
            lines,
            wires,
        } = drawing(&flat_cube);
        assert!(wires.is_empty());
        assert_eq!(shown.vertices(), flat_cube.cage().vertices());
        assert_eq!(segments(&lines).len(), 12);
        let (drawn, _) = crate::meshes::body_mesh(&shown);
        // Each corner's normal is straight out of one face: one axis only.
        assert!(drawn.normals.iter().all(|n| {
            let ones = n.iter().filter(|c| (c.abs() - 1.0).abs() < 1e-9).count();
            let zeros = n.iter().filter(|c| c.abs() < 1e-9).count();
            ones == 1 && zeros == 2
        }));
    }

    /// Each straight segment of `lines`, its ends as exact bits, the lower end first.
    fn segments(lines: &[Curve]) -> Vec<([u64; 3], [u64; 3])> {
        let mut all = Vec::new();
        for line in lines {
            let points = line.sample().unwrap();
            for pair in points.windows(2) {
                let (a, b) = (pair[0].map(f64::to_bits), pair[1].map(f64::to_bits));
                all.push(if a <= b { (a, b) } else { (b, a) });
            }
        }
        all
    }

    /// Shown smooth, a SubD's other cage edges lie on its surface for wireframe: a cube's twelve,
    /// each a run between two corners. Creased, the crease draws in every view and is left out
    /// of the wires, so no segment draws twice.
    #[test]
    fn a_smooth_subds_cage_edges_draw_once_on_its_surface() {
        let smooth = drawing(&cube());
        assert_eq!(smooth.wires.len(), 12);
        let surface = smooth.shown.vertices();
        for wire in &smooth.wires {
            for p in wire.sample().unwrap() {
                assert!(
                    surface
                        .iter()
                        .any(|q| (0..3).all(|k| (p[k] - q[k]).abs() < 1e-6))
                );
            }
        }
        let creased = drawing(&cube().with_creases(&[[0, 1]]));
        let crease = segments(&creased.lines);
        let wires = segments(&creased.wires);
        assert!(!crease.is_empty());
        assert!(wires.iter().all(|segment| !crease.contains(segment)));
        // Together they are every edge segment the plain cube has.
        assert_eq!(crease.len() + wires.len(), segments(&smooth.wires).len());
        let mut once = wires.clone();
        once.sort_unstable();
        once.dedup();
        assert_eq!(once.len(), wires.len());
    }

    /// A SubD in the model shades as one shared shape and is picked by a click inside it, as a
    /// solid when its cage is closed, shown smooth or flat (#324).
    #[test]
    fn a_subd_in_the_model_shades_and_is_picked() {
        use crate::{Outlines, SceneParts, StandardView};
        const SIZE: [f64; 2] = [800.0, 600.0];
        let mut doc = curva_document::Document::default();
        let id = doc.add_shape(curva_document::Shape::SubD(cube()));
        let outlines = Outlines::of(doc.state());
        assert_eq!(SceneParts::shared(&outlines).shapes.len(), 1);
        // Its cage edges draw in wireframe only, as a mesh's wires do.
        assert_eq!(outlines.objects.len(), 12);
        assert!(outlines.seams.iter().all(|seam| *seam));
        assert!(outlines.on_faces.iter().all(|on| *on));
        let (low, high) = cube().bounds().unwrap();
        let mut camera = StandardView::Top.camera();
        camera.frame(low, high);
        let middle = std::array::from_fn(|k| (low[k] + high[k]) / 2.0);
        let at = camera.project(middle, SIZE).unwrap();
        assert_eq!(outlines.pick(&camera, at, SIZE, 6.0), Some(id));
        let flat = doc.add_shape(curva_document::Shape::SubD(
            cube()
                .with_display(SubDDisplay::Flat)
                .translated([300.0, 0.0, 0.0]),
        ));
        let outlines = Outlines::of(doc.state());
        assert!(outlines.objects.iter().any(|(object, _)| *object == flat));
        assert!(
            outlines
                .kinds
                .iter()
                .all(|kind| *kind == crate::PickKind::Solid)
        );
    }
}
