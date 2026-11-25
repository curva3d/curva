//! Lines in the same place, on the graphics card: a selected line wins, then a curve over an
//! edge, as in Rhino (owner's report 18 on 0.1.6: a curve DupBorder copied from a cylinder's
//! edge drew mixed with that edge).

use super::picture::{Scene, draw_picture_in_chunks};
use super::{CHUNK_BYTES, Picture};
use crate::{LineKind, Style, Vertex};
use std::sync::Arc;

const SIZE: [u32; 2] = [64, 16];

/// A horizontal line across the picture, in `colour`, of `kind`, as object number `object`.
fn line(colour: [f32; 3], kind: LineKind, object: u32) -> [Vertex; 2] {
    let end = |x: f32| Vertex {
        position: [x, 0.0, 0.5],
        colour,
        width: -2.0,
        object,
        kind,
    };
    [end(-0.9), end(0.9)]
}

/// The colour of the middle pixel of a picture of `lines`, drawn in that order, with objects 10
/// and 11 numbered 0 and 1 and `selected` selected. `None` without a graphics adapter.
fn middle(lines: Vec<Vertex>, selected: Vec<u64>) -> Option<[u8; 3]> {
    let picture = Picture {
        lines,
        matrix: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        light: [0.0, 0.0, 1.0],
        style: Style::wireframe(),
        ..Picture::default()
    };
    let scene = Scene {
        objects: Arc::new(vec![10, 11]),
        selection: Arc::new(selected),
        ..Scene::default()
    };
    match draw_picture_in_chunks(picture, SIZE, true, CHUNK_BYTES, &scene) {
        Ok(pixels) => {
            let (w, h) = (SIZE[0] as usize, SIZE[1] as usize);
            let at = (h / 2 * w + w / 2) * 4;
            Some([pixels[at], pixels[at + 1], pixels[at + 2]])
        }
        Err(error) => {
            eprintln!("no graphics adapter to draw with, so not checked: {error}");
            None
        }
    }
}

#[test]
fn a_curve_on_an_edge_shows_over_it_and_a_selected_line_over_both() {
    const WHITE: [f32; 3] = [1.0; 3];
    const BLACK: [f32; 3] = [0.0; 3];
    // The edge drawn after the curve, which would win a tie in depth.
    let curve_then_edge = [
        line(WHITE, LineKind::Curve, 0),
        line(BLACK, LineKind::Edge, 1),
    ];
    let Some(plain) = middle(curve_then_edge.concat(), Vec::new()) else {
        return;
    };
    assert!(
        plain.iter().all(|&c| c > 200),
        "the curve shows over the edge: {plain:?}"
    );
    // Selected, the edge wins in the selection colour, orange.
    let Some(chosen) = middle(curve_then_edge.concat(), vec![11]) else {
        return;
    };
    let edge_after = [
        line(BLACK, LineKind::Edge, 1),
        line(WHITE, LineKind::Curve, 0),
    ];
    let Some(selected_first) = middle(edge_after.concat(), vec![11]) else {
        return;
    };
    assert_eq!(
        chosen, selected_first,
        "the selected edge wins whichever comes first"
    );
    assert!(
        chosen[0] > 200 && chosen[0] > chosen[1] + 40 && chosen[1] > chosen[2],
        "in the selection colour: {chosen:?}"
    );
}
