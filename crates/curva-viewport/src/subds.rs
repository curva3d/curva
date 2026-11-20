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

/// What a SubD draws as: the mesh it shades with, and its lines. Shown smooth, the smooth surface
/// with its creases and open borders on it; shown flat, or when the smooth surface cannot be
/// worked out, the cage's faces shaded flat with all its edges.
pub(crate) fn drawing(subd: &SubD) -> (Mesh, Vec<Curve>) {
    if subd.display() == SubDDisplay::Smooth {
        let levels = levels_for(subd);
        // A closed cage without creases has no lines on its surface; finding them would take as
        // long as the surface itself.
        let plain = subd.creases().next().is_none() && subd.cage().is_closed();
        let creases = if plain {
            Ok(Vec::new())
        } else {
            subd.crease_curves(levels)
        };
        if let (Ok(surface), Ok(creases)) = (subd.surface(levels), creases) {
            let lines = creases
                .into_iter()
                .filter_map(|run| Curve::polyline(run, false).ok())
                .collect();
            return (surface, lines);
        }
    }
    (flat(subd.cage()), crate::meshes::wires(subd.cage()))
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

    /// Shown smooth, a cube's cage draws as a rounded surface inside the cage, with no lines
    /// until it has creases; its creases draw on the surface.
    #[test]
    fn a_smooth_subd_draws_its_surface_and_its_creases() {
        let (low, high) = cube().bounds().unwrap();
        let middle: Point = std::array::from_fn(|k| (low[k] + high[k]) / 2.0);
        let (surface, lines) = drawing(&cube());
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
        let (_, lines) = drawing(&creased);
        assert!(!lines.is_empty());
    }

    /// Shown flat, the cage's own faces shade flat, each with its own normal, and all twelve of
    /// a cube's edges draw.
    #[test]
    fn a_flat_subd_draws_its_cage() {
        let flat_cube = cube().with_display(SubDDisplay::Flat);
        let (shown, lines) = drawing(&flat_cube);
        assert_eq!(shown.vertices(), flat_cube.cage().vertices());
        let segments: usize = lines
            .iter()
            .map(|line| match line {
                Curve::Polyline { points, .. } => points.len() - 1,
                _ => 1,
            })
            .sum();
        assert_eq!(segments, 12);
        let (drawn, _) = crate::meshes::body_mesh(&shown);
        // Each corner's normal is straight out of one face: one axis only.
        assert!(drawn.normals.iter().all(|n| {
            let ones = n.iter().filter(|c| (c.abs() - 1.0).abs() < 1e-9).count();
            let zeros = n.iter().filter(|c| c.abs() < 1e-9).count();
            ones == 1 && zeros == 2
        }));
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
