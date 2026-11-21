//! The small drawings in the view picker's tiles (#471, the owner's choice of 21 November 2026):
//! one house, seen from the side each view looks from, so a tile shows where it looks from as
//! well as saying it. Flat line drawings on the menu, not a cube in the scene.

use curva_viewport::StandardView;
use eframe::egui;

/// Wall height, ridge height, width (X, the gable's width) and depth (Y) of the house.
const WALL: f32 = 1.1;
const RIDGE: f32 = 1.9;
const WIDTH: f32 = 2.0;
const DEPTH: f32 = 3.0;

/// The house's corners: the floor, the tops of the walls, then the ridge's two ends. Its gables
/// face Front and Back, so the ridge runs from front to back.
const CORNERS: [[f32; 3]; 10] = [
    [0.0, 0.0, 0.0],
    [WIDTH, 0.0, 0.0],
    [WIDTH, DEPTH, 0.0],
    [0.0, DEPTH, 0.0],
    [0.0, 0.0, WALL],
    [WIDTH, 0.0, WALL],
    [WIDTH, DEPTH, WALL],
    [0.0, DEPTH, WALL],
    [WIDTH / 2.0, 0.0, RIDGE],
    [WIDTH / 2.0, DEPTH, RIDGE],
];

/// Every edge of the house, by corner. Seen straight on from any side, the hidden ones fall on
/// visible ones, so all can be drawn.
const EDGES: [[usize; 2]; 15] = [
    [0, 1],
    [1, 2],
    [2, 3],
    [3, 0],
    [5, 6],
    [7, 4],
    [0, 4],
    [1, 5],
    [2, 6],
    [3, 7],
    [4, 8],
    [8, 5],
    [7, 9],
    [9, 6],
    [8, 9],
];

/// The edges seen from the front left and above, as the perspective views look: the front
/// gable, the left wall and both slopes of the roof, without the edges behind the house.
const SEEN_IN_PERSPECTIVE: [[usize; 2]; 12] = [
    [0, 1],
    [0, 3],
    [0, 4],
    [1, 5],
    [3, 7],
    [4, 7],
    [4, 8],
    [8, 5],
    [8, 9],
    [7, 9],
    [9, 6],
    [5, 6],
];

/// The front door, seen only from the front, so Front and Back differ.
const DOOR: [[f32; 3]; 4] = [
    [0.8, 0.0, 0.0],
    [0.8, 0.0, 0.7],
    [1.2, 0.0, 0.7],
    [1.2, 0.0, 0.0],
];

/// Where a point of the house lands in the drawing for `view`, before it is fitted to the tile:
/// x to the right, y down.
fn projected(view: StandardView, [x, y, z]: [f32; 3]) -> egui::Pos2 {
    match view {
        StandardView::Top => egui::pos2(x, -y),
        StandardView::Bottom => egui::pos2(x, y),
        StandardView::Front => egui::pos2(x, -z),
        StandardView::Back => egui::pos2(-x, -z),
        StandardView::Left => egui::pos2(-y, -z),
        StandardView::Right => egui::pos2(y, -z),
        StandardView::Perspective => {
            // Seen from the front left and above, turned 35° and looking 25° down.
            let (turn, tilt) = (35f32.to_radians(), 25f32.to_radians());
            let across = x * turn.cos() - y * turn.sin();
            let along = x * turn.sin() + y * turn.cos();
            egui::pos2(across, -(z * tilt.cos() + along * tilt.sin()))
        }
        StandardView::TwoPoint => {
            // The same side, from eye height and nearer, so the walls stay upright and the
            // horizontal edges run to two vanishing points.
            let turn = 35f32.to_radians();
            let across = x * turn.cos() - y * turn.sin();
            let along = x * turn.sin() + y * turn.cos() + 4.5;
            egui::pos2(across * 4.5 / along, -(z - 0.7) * 4.5 / along)
        }
    }
}

/// The lines of the house seen from `view`, fitted inside `rect`.
#[must_use]
pub(crate) fn house_lines(view: StandardView, rect: egui::Rect) -> Vec<[egui::Pos2; 2]> {
    let corner = |i: usize| projected(view, CORNERS[i]);
    let mut lines: Vec<[egui::Pos2; 2]> = match view {
        StandardView::Perspective => SEEN_IN_PERSPECTIVE
            .iter()
            .map(|&[a, b]| [corner(a), corner(b)])
            .collect(),
        // From eye height the far slope of the roof is out of sight: its last two edges go.
        StandardView::TwoPoint => SEEN_IN_PERSPECTIVE[..10]
            .iter()
            .map(|&[a, b]| [corner(a), corner(b)])
            .collect(),
        // From below only the floor shows.
        StandardView::Bottom => EDGES[..4]
            .iter()
            .map(|&[a, b]| [corner(a), corner(b)])
            .collect(),
        _ => EDGES.iter().map(|&[a, b]| [corner(a), corner(b)]).collect(),
    };
    if view == StandardView::Front {
        let door: Vec<egui::Pos2> = DOOR.iter().map(|&p| projected(view, p)).collect();
        lines.extend(door.windows(2).map(|pair| [pair[0], pair[1]]));
    }
    fitted(lines, rect)
}

/// `lines` scaled evenly and centred to fill `rect`.
fn fitted(lines: Vec<[egui::Pos2; 2]>, rect: egui::Rect) -> Vec<[egui::Pos2; 2]> {
    let around = egui::Rect::from_points(&lines.iter().flatten().copied().collect::<Vec<_>>());
    let scale =
        (rect.width() / around.width().max(1e-3)).min(rect.height() / around.height().max(1e-3));
    let place = |p: egui::Pos2| rect.center() + (p - around.center()) * scale;
    lines
        .into_iter()
        .map(|[a, b]| [place(a), place(b)])
        .collect()
}

/// Draws the house seen from `view` inside `rect` in `colour`; from below, dashed, as a floor
/// seen through.
pub(crate) fn paint_house(
    painter: &egui::Painter,
    view: StandardView,
    rect: egui::Rect,
    colour: egui::Color32,
) {
    let stroke = egui::Stroke::new(1.2, colour);
    for [a, b] in house_lines(view, rect) {
        if view == StandardView::Bottom {
            painter.extend(egui::Shape::dashed_line(&[a, b], stroke, 2.5, 2.0));
        } else {
            painter.line_segment([a, b], stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(34.0, 24.0))
    }

    /// Every view's drawing fills its tile without leaving it, and the views each look
    /// different: Front has its door, so it is not Back, and Top is not Bottom.
    #[test]
    fn each_view_draws_its_own_house_inside_the_tile() {
        let mut drawings = Vec::new();
        for view in StandardView::ALL {
            let lines = house_lines(view, tile());
            assert!(!lines.is_empty(), "{view:?}");
            let inside = tile().expand(0.01);
            assert!(
                lines.iter().flatten().all(|p| inside.contains(*p)),
                "{view:?}"
            );
            let around =
                egui::Rect::from_points(&lines.iter().flatten().copied().collect::<Vec<_>>());
            assert!(
                (around.width() - 34.0).abs() < 0.01 || (around.height() - 24.0).abs() < 0.01,
                "{view:?} {around:?}"
            );
            drawings.push(lines);
        }
        for (i, a) in drawings.iter().enumerate() {
            for b in &drawings[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    /// The perspective drawings show only the edges in sight, none from the corner hidden
    /// behind the house and no line across the foot of a gable, which is one flat face; from eye
    /// height the far slope of the roof is out of sight too.
    #[test]
    fn the_perspective_house_leaves_out_its_hidden_edges() {
        assert_eq!(house_lines(StandardView::Perspective, tile()).len(), 12);
        assert_eq!(house_lines(StandardView::TwoPoint, tile()).len(), 10);
        let all = SEEN_IN_PERSPECTIVE.iter().chain(&EDGES);
        assert!(
            SEEN_IN_PERSPECTIVE
                .iter()
                .flatten()
                .all(|&corner| corner != 2)
        );
        for gable_foot in [[4, 5], [5, 4], [6, 7], [7, 6]] {
            assert!(!all.clone().any(|edge| *edge == gable_foot));
        }
    }
}
