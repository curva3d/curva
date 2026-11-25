//! Silhouettes of curved faces in views whose faces hide nothing, on the graphics card.

use super::picture::{Scene, draw_picture_in_chunks};
use super::{CHUNK_BYTES, Picture};
use std::sync::Arc;

const SIZE: usize = 128;

/// A sphere of radius 10 seen straight down in Wireframe, with or without its turns: the
/// picture's pixels, where its centre and rim are on screen, and its seam's row. `None` without
/// a graphics adapter.
fn sphere_from_above(turns: bool) -> Option<(Vec<u8>, [f64; 2], f64)> {
    let mut doc = curva_document::Document::default();
    doc.add_body(curva_kernel::Body::sphere([0.0; 3], 10.0).unwrap());
    let outlines = crate::Outlines::of(doc.state());
    let parts = crate::SceneParts::shared(&outlines);
    assert!(!parts.turns.is_empty());
    let mut camera = crate::StandardView::Top.camera();
    camera.frame([-12.0, -12.0, -10.0], [12.0, 12.0, 10.0]);
    let size = [SIZE as f64; 2];
    let centre = camera.project([0.0; 3], size)?;
    let rim = camera.project([10.0, 0.0, 0.0], size)?[0] - centre[0];
    let measured = camera.measured_from(parts.origin);
    let picture = Picture {
        lines: parts.lines,
        matrix: measured.matrix(1.0),
        light: measured.forward().map(|v| v as f32),
        style: crate::Style::wireframe(),
        ..Picture::default()
    };
    let scene = Scene {
        objects: Arc::new(parts.objects),
        turns: Arc::new(if turns { parts.turns } else { Vec::new() }),
        ..Scene::default()
    };
    let square = [SIZE as u32; 2];
    match draw_picture_in_chunks(picture, square, true, CHUNK_BYTES, &scene) {
        Ok(pixels) => Some((pixels, centre, rim)),
        Err(error) => {
            eprintln!("no graphics adapter to draw with, so not checked: {error}");
            None
        }
    }
}

/// Whether pixel `x`, `y` of `pixels` differs from the picture's corner, the background.
fn inked(pixels: &[u8], x: usize, y: usize) -> bool {
    let at = (y * SIZE + x) * 4;
    (0..3).any(|k| pixels[at + k].abs_diff(pixels[k]) > 40)
}

/// Rhino 8 draws a curved surface's silhouette in Wireframe: a sphere seen from above shows its
/// rim, though it has no edge there, and nothing inside it besides its seam.
#[test]
fn a_sphere_in_wireframe_shows_its_rim_and_nothing_inside() {
    let (Some((with, centre, rim)), Some((without, ..))) =
        (sphere_from_above(true), sphere_from_above(false))
    else {
        return;
    };
    let [cx, cy] = centre;
    // Towards the left, up and down (the seam runs right from the middle), a pixel or two of
    // ink near the rim with its turns, and none without.
    for (dx, dy) in [(-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        let near_rim = |pixels: &[u8]| {
            (-2..=2).any(|off| {
                let at = rim + f64::from(off);
                inked(pixels, (cx + dx * at) as usize, (cy + dy * at) as usize)
            })
        };
        assert!(near_rim(&with), "no rim towards {dx},{dy}");
        assert!(!near_rim(&without), "rim without turns towards {dx},{dy}");
    }
    // Inside, well clear of the rim and the seam's row, no ink: only the turns facing both
    // ways at once draw.
    for y in 0..SIZE {
        for x in 0..SIZE {
            let from = (x as f64 + 0.5 - cx).hypot(y as f64 + 0.5 - cy);
            let off_seam = (y as f64 + 0.5 - cy).abs() > 3.0;
            if from < rim - 4.0 && off_seam {
                assert!(!inked(&with, x, y), "ink inside at {x},{y}");
            }
        }
    }
}
