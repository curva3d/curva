//! A sheet's own objects over its details, as Rhino draws a layout (#826).

use super::{SIZE, drawing, one_at_a_time};
use crate::{NO_OBJECT, StandardView, Vertex};
use std::sync::Arc;

const PAPER: epaint::Color32 = epaint::Color32::WHITE;
const DETAIL: epaint::Color32 = epaint::Color32::from_rgb(40, 40, 40);

/// A red line across the sheet, seen from above; the sheet on white paper.
fn sheet() -> crate::Drawing {
    let mut view = drawing(StandardView::Top, Vec::new());
    let end = |x: f32| Vertex {
        position: [x, 0.0, 0.0],
        colour: [1.0, 0.0, 0.0],
        width: -2.0,
        object: NO_OBJECT,
        kind: crate::LineKind::Given,
    };
    view.scene = Arc::new(vec![end(-1.0e5), end(1.0e5)]);
    view.background = PAPER;
    view.background_low = PAPER;
    view
}

/// The pixel at `x`, `y` once the sheet, a detail over its middle third, and, `over` it if asked,
/// the sheet again, are drawn in one frame; `None` with no graphics.
fn pixel_with(over: bool, [x, y]: [u32; 2]) -> Option<[u8; 3]> {
    let mut views = vec![sheet()];
    let third = epaint::vec2(SIZE[0] as f32 / 3.0, SIZE[1] as f32 / 3.0);
    let frame = epaint::Rect::from_min_size(epaint::pos2(third.x, third.y), third);
    let mut detail = drawing(StandardView::Top, Vec::new());
    detail.slot = 1;
    detail.area = frame;
    detail.background = DETAIL;
    detail.background_low = DETAIL;
    views.push(detail);
    if over {
        let mut again = sheet();
        again.over = vec![frame];
        views.push(again);
    }
    let pixels = super::super::with_pictures(|pictures| {
        // Bevy's first picture of a new scene comes ready on a later update.
        for _ in 0..4 {
            pictures.draw_frame(&mut views, SIZE);
        }
        pictures.pixels(SIZE)
    })
    .ok()?;
    let at = ((y * SIZE[0] + x) * 4) as usize;
    Some([pixels[at], pixels[at + 1], pixels[at + 2]])
}

#[test]
fn a_sheet_s_objects_show_over_its_details_and_its_paper_does_not() {
    let _turn = one_at_a_time();
    let middle = [SIZE[0] / 2, SIZE[1] / 2];
    let off_line = [SIZE[0] / 2, SIZE[1] / 2 + 5];
    let (Some(hidden), Some(shown), Some(detail)) = (
        pixel_with(false, middle),
        pixel_with(true, middle),
        pixel_with(true, off_line),
    ) else {
        return;
    };
    assert!(
        hidden[0] < 100,
        "the detail alone covers the line: {hidden:?}"
    );
    assert!(
        shown[0] > 180 && shown[1] < 100,
        "the line shows over the detail: {shown:?}"
    );
    assert!(
        detail[0] < 100,
        "the paper leaves the detail showing: {detail:?}"
    );
}
