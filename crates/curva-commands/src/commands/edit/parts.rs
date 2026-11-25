//! Transforms of parts picked with Ctrl+Shift, and of control points: when a command's objects
//! were given by such picks, it moves those parts rather than whole objects, as Rhino transforms
//! sub-objects (the faces meeting them follow).

use crate::Message;
use crate::commands::{Change, changed};
use crate::recipe::{Context, Made};
use curva_kernel::Point;

/// Where `change` takes the point `p`.
fn moved(change: &Change, p: Point) -> Point {
    match change {
        Change::Move(by) => std::array::from_fn(|i| p[i] + by[i]),
        Change::Map(map) => map.point(p),
    }
}

/// Whether the objects `ids` are those of the picked parts, so the parts are what changes.
pub(crate) fn of_parts(cx: &Context<'_>, ids: &[u64]) -> bool {
    let mut sorted = ids.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    cx.doc.has_picked_parts() && cx.doc.picked_part_owners() == sorted
}

/// The objects `ids` changed by `change`, as [`changed`] does; or, when they are the objects of
/// the picked parts and no copy is asked for, those parts moved with their objects reshaped.
pub(crate) fn changed_or_parts(
    cx: &Context<'_>,
    ids: &[u64],
    change: Change,
    copy: bool,
    history: impl Fn(u64, &Change) -> Option<curva_document::Recipe>,
) -> Result<Made, Message> {
    if copy || !of_parts(cx, ids) {
        return Ok(changed(cx, ids, &[change], copy, history));
    }
    let replace = cx
        .doc
        .picked_parts_reshaped(|p| moved(&change, p))
        .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
    Ok(Made {
        replace,
        ..Made::default()
    })
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Body;

    /// A unit box with its top face picked with Ctrl+Shift.
    fn box_with_top_picked() -> Document {
        let mut doc = Document::default();
        let id = doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        let body = doc.state().objects[0].shape.body().unwrap().clone();
        let corners = body.corner_points().unwrap();
        let top = (0..6)
            .find(|f| {
                body.corners_of_faces(&[*f])
                    .unwrap()
                    .iter()
                    .all(|c| corners[*c][2] > 0.5)
            })
            .unwrap();
        doc.select_faces(&[(id, top)]);
        doc
    }

    fn volume(doc: &Document) -> f64 {
        doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap()
    }

    #[test]
    fn move_takes_a_picked_face_and_the_solid_follows() {
        let mut doc = box_with_top_picked();
        run(&mut doc, &["move", "0,0,0", "0,0,1"]);
        assert_eq!(doc.state().objects.len(), 1);
        assert!((volume(&doc) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn scale_takes_a_picked_face_about_its_origin() {
        let mut doc = box_with_top_picked();
        run(&mut doc, &["scale", "0.5,0.5,1", "2"]);
        // The top grows to 2 by 2 about its middle: a frustum of height 1 between areas 1 and 4.
        let expected = (1.0 + 4.0 + 2.0) / 3.0;
        assert!((volume(&doc) - expected).abs() < 1e-6, "{}", volume(&doc));
    }

    #[test]
    fn scale1d_stretches_a_picked_face_from_its_origin() {
        let mut doc = box_with_top_picked();
        run(&mut doc, &["scale1d", "0,0,0", "2", "0,0,1"]);
        assert!((volume(&doc) - 2.0).abs() < 1e-6, "{}", volume(&doc));
    }

    #[test]
    fn rotate_takes_a_picked_face() {
        let mut doc = box_with_top_picked();
        run(&mut doc, &["rotate", "0.5,0.5,1", "45"]);
        assert!(doc.state().objects[0].shape.body().unwrap().is_solid());
    }
}
