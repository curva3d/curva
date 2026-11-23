//! Meshes' wires on views Bevy draws.

use super::{SIZE, drawing, one_at_a_time, square};
use crate::{MeshVertex, NO_OBJECT, StandardView, Vertex};
use std::sync::Arc;

/// A mesh's wires, plain one-pixel lines that may take no depth bias, still show on the mesh's
/// own faces as Bevy draws them, seen slanting in perspective, as Curva's other lines do: a white wire lying on a grey face
/// shows as much of itself as with no face there.
#[test]
fn a_mesh_wire_shows_on_its_own_face() {
    let _turn = one_at_a_time();
    let white_pixels = |faces: Vec<MeshVertex>| {
        let mut view = drawing(StandardView::Perspective, faces);
        let end = |x: f32| Vertex {
            position: [x, 0.0, 0.0],
            colour: [1.0; 3],
            width: crate::WIRE_WIDTH,
            object: NO_OBJECT,
            kind: crate::LineKind::Given,
        };
        view.scene = Arc::new(vec![end(-1.0e5), end(1.0e5)]);
        let pixels = super::super::with_pictures(|pictures| {
            // Bevy's first picture of a new scene comes ready on a later update.
            for _ in 0..4 {
                pictures.draw(&mut view, SIZE);
            }
            pictures.pixels(SIZE)
        })
        .ok()?;
        Some(pixels.chunks(4).filter(|pixel| pixel[0] > 200).count())
    };
    let (Some(alone), Some(on_face)) = (
        white_pixels(Vec::new()),
        white_pixels(square([0.0, 0.0, 1.0], [0.3; 3])),
    ) else {
        return;
    };
    assert!(alone > 20, "the wire is drawn: {alone} pixels");
    assert!(
        on_face * 10 >= alone * 9,
        "the wire shows on the face: {on_face} pixels against {alone} alone"
    );
}
