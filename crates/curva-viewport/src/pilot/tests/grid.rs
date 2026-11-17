//! The grid on views Bevy draws (#344).

use super::{BACKGROUND, SIZE, drawing, one_at_a_time};
use crate::palette::{Palette, grid_lines_in};
use crate::{MeshVertex, StandardView};
use std::sync::Arc;

/// The grid shows on a view Bevy draws, through the window's own path, where Bevy drew nothing,
/// and not through a face lying on it (#344: since Bevy drew every view, the grid had gone).
#[test]
fn the_grid_shows_where_bevy_drew_nothing() {
    let _turn = one_at_a_time();
    // A face on the grid's plane over the left half of the view, seen from above.
    let corner = |x: f32, y: f32| MeshVertex {
        position: [x, y, 0.0],
        normal: [0.0, 0.0, 1.0],
        colour: [0.6; 3],
        object: 0,
    };
    let far = 1.0e5;
    let half = vec![
        corner(-far, -far),
        corner(0.0, -far),
        corner(0.0, far),
        corner(-far, -far),
        corner(0.0, far),
        corner(-far, far),
    ];
    let mut view = drawing(StandardView::Top, half);
    view.grid = Arc::new(grid_lines_in(
        &StandardView::Top.plane(),
        &Palette::LIGHT,
        [0.0; 3],
    ));
    let Ok(pixels) = super::super::with_pictures(|pictures| {
        // Bevy's first picture of a new scene comes ready on a later update.
        for _ in 0..4 {
            pictures.draw(&mut view, SIZE);
        }
        pictures.pixels(SIZE)
    }) else {
        return;
    };
    let pixel = |x: u32, y: u32| {
        let at = ((y * SIZE[0] + x) * 4) as usize;
        [pixels[at], pixels[at + 1], pixels[at + 2]]
    };
    let (middle, margin) = (SIZE[0] / 2, 4);
    let mut face = Vec::new();
    let mut grid = 0;
    for y in 0..SIZE[1] {
        face.extend((0..middle - margin).map(|x| pixel(x, y)));
        grid += (middle + margin..SIZE[0])
            .filter(|&x| pixel(x, y) != [BACKGROUND.r(), BACKGROUND.g(), BACKGROUND.b()])
            .count();
    }
    assert!(grid > 40, "the grid shows beside the face: {grid} pixels");
    face.sort_unstable();
    face.dedup();
    assert_eq!(
        face.len(),
        1,
        "the face is one colour, the grid hidden: {face:?}"
    );
    assert_ne!(face[0], [200, 120, 40], "the face shows");
}
