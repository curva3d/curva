//! Whole circles drawn by the graphics card from their centre and axes (#153): one entry of
//! 56 bytes each, where their segments took [`CIRCLE_SEGMENTS`] entries of 64. The benchmark's
//! 40 000 circles came to 328 MB of lines as segments.

use curva_kernel::{Curve, Point};

use crate::{Outlines, line_width, narrow, piece_colour};

/// How many segments a circle is drawn with: as many as the kernel samples it with, so it
/// shows where it is picked and snapped to.
pub const CIRCLE_SEGMENTS: u32 = 128;

/// A whole circle as the graphics card draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedCircle {
    /// Its centre in model space.
    pub centre: [f32; 3],
    /// Its first axis, as long as its radius: where it starts.
    pub x: [f32; 3],
    /// Its second axis, as long as its radius, a quarter turn on from `x`.
    pub y: [f32; 3],
    /// Its colour when not selected.
    pub colour: [f32; 3],
    /// How thick it draws, as [`crate::Vertex::width`].
    pub width: f32,
    /// The object's number in [`crate::SceneParts::objects`].
    pub object: u32,
}

/// A circle's centre and its two axes as long as its radius; `None` for any other curve.
pub(crate) fn axes_of(curve: &Curve) -> Option<[Point; 3]> {
    match curve {
        Curve::Circle {
            centre,
            radius,
            x,
            y,
        } => Some([*centre, x.map(|v| v * radius), y.map(|v| v * radius)]),
        _ => None,
    }
}

/// The centre and axes of entry `index` of `outlines` when it is drawn as a [`PlacedCircle`]:
/// a whole circle, undashed (dashes are laid along its length) and not an edge on faces (whose
/// colour changes with the view).
pub(crate) fn drawn_apart(outlines: &Outlines, index: usize) -> Option<[Point; 3]> {
    let dashed = !outlines
        .dashes
        .get(index)
        .copied()
        .unwrap_or(&[])
        .is_empty();
    let on_faces = outlines.on_faces.get(index).copied().unwrap_or(false);
    if dashed || on_faces {
        return None;
    }
    outlines.circles.get(index).copied().flatten()
}

/// The circles of `outlines` drawn apart, numbered by `numbers`, unselected.
pub(crate) fn circles_of(
    outlines: &Outlines,
    numbers: &std::collections::HashMap<u64, u32>,
) -> Vec<PlacedCircle> {
    outlines
        .objects
        .iter()
        .enumerate()
        .filter_map(|(index, (id, _))| {
            let [centre, x, y] = drawn_apart(outlines, index)?;
            Some(PlacedCircle {
                centre: narrow(centre),
                x: narrow(x),
                y: narrow(y),
                colour: piece_colour(outlines, index, false, false),
                width: line_width(outlines.widths.get(index).copied().flatten(), false),
                object: numbers.get(id).copied().unwrap_or(crate::NO_OBJECT),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SceneParts;

    fn circle(centre: Point, radius: f64) -> Curve {
        Curve::circle(centre, radius).unwrap()
    }

    /// The kernel samples a circle with as many segments as the graphics card draws.
    #[test]
    fn the_card_draws_the_segments_the_kernel_samples() {
        let points = circle([0.0; 3], 1.0).sample().unwrap_or_default();
        assert_eq!(points.len(), CIRCLE_SEGMENTS as usize + 1);
    }

    /// A shared scene draws a circle as one entry, out of its lines, while a line stays a line;
    /// the plain scene keeps every circle's segments.
    #[test]
    fn a_shared_scene_holds_a_circle_as_one_entry() {
        let mut doc = curva_document::Document::default();
        let ring = doc.add(circle([1.0, 2.0, 3.0], 2.0));
        doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let outlines = Outlines::of(doc.state());
        let shared = SceneParts::shared(&outlines);
        assert_eq!(shared.circles.len(), 1);
        let placed = shared.circles[0];
        assert_eq!(placed.centre, [1.0, 2.0, 3.0]);
        assert_eq!((placed.x, placed.y), ([2.0, 0.0, 0.0], [0.0, 2.0, 0.0]));
        assert_eq!(shared.objects[placed.object as usize], ring);
        assert_eq!(shared.lines.len(), 2, "only the line's one segment");
        assert_eq!(shared.shaded_lines.len(), 2);
        let plain = SceneParts::of(&outlines);
        assert!(plain.circles.is_empty());
        assert_eq!(plain.lines.len(), 2 * (CIRCLE_SEGMENTS as usize + 1));
    }
}
