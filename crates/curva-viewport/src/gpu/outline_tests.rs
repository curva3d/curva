//! Outlines drawn as wide as their view's style says, on the graphics card.

use super::{Picture, draw_picture};
use curva_document::style::ViewStyle;

const SIZE: u32 = 128;

/// How many pixels of ink lie just inside a sphere's rim, seen straight down, going right from
/// its middle, with outlines `width` millimetres wide. `None` without a graphics adapter.
fn ink_at_rim(width: f32) -> Option<usize> {
    let mut doc = curva_document::Document::default();
    doc.add_body(curva_kernel::Body::sphere([0.0; 3], 10.0).unwrap());
    let outlines = crate::Outlines::of(doc.state());
    let mut camera = crate::StandardView::Top.camera();
    camera.frame([-12.0, -12.0, -10.0], [12.0, 12.0, 10.0]);
    let mut view = ViewStyle::shaded();
    view.outlines.width = width;
    let picture = Picture {
        lines: crate::shaded_object_lines(&outlines, &[]),
        faces: crate::face_vertices(&outlines, &[]),
        matrix: camera.matrix(1.0),
        light: camera.forward().map(|v| v as f32),
        camera: crate::Camera::default(),
        style: crate::Style::of(&view),
    };
    let Ok(pixels) = draw_picture(picture, [SIZE, SIZE], true) else {
        eprintln!("no graphics adapter to draw with, so not checked");
        return None;
    };
    let size = f64::from(SIZE);
    let [cx, cy] = camera.project([0.0; 3], [size, size]).unwrap();
    let rim = camera.project([10.0, 0.0, 0.0], [size, size]).unwrap()[0];
    let row = cy as usize;
    // Pictures store colours converted for display: the near-black ink reads about 80, the lit
    // surface 180 up.
    Some(
        (cx as usize..rim.ceil() as usize)
            .filter(|&x| pixels[(row * SIZE as usize + x) * 4] <= 110)
            .count(),
    )
}

#[test]
fn outlines_draw_as_wide_as_the_style_says() {
    let (Some(thin), Some(wide)) = (ink_at_rim(0.25), ink_at_rim(1.5)) else {
        return;
    };
    assert!((1..=3).contains(&thin), "Shaded's two pixels: {thin}");
    assert!(
        (5..=7).contains(&wide),
        "1.5 mm is about six pixels: {wide}"
    );
}
