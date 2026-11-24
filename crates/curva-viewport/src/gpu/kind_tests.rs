//! Each kind of line drawn as its view's style gives it, on the graphics card.

use super::{Picture, draw_picture};
use crate::{LineKind, NO_OBJECT, Style, Vertex};
use curva_document::style::{Colour, LineColour, LineWidth, ViewStyle};

const SIZE: [u32; 2] = [64, 64];

/// The centre column of a picture of one white horizontal line of `kind`, its own width
/// `width`, drawn in `style`: how many rows it lights, and the colour at its middle. `None`
/// without a graphics adapter.
fn drawn(kind: LineKind, width: f32, style: &ViewStyle) -> Option<(usize, [u8; 3])> {
    let end = |x: f32| Vertex {
        position: [x, 0.0, 0.5],
        colour: [1.0; 3],
        width,
        object: NO_OBJECT,
        kind,
    };
    let picture = Picture {
        lines: vec![end(-0.8), end(0.8)],
        matrix: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        light: [0.0, 0.0, 1.0],
        style: Style::of(style),
        ..Picture::default()
    };
    let pixels = match draw_picture(picture, SIZE, true) {
        Ok(pixels) => pixels,
        Err(error) => {
            eprintln!("no graphics adapter to draw with, so not checked: {error}");
            return None;
        }
    };
    let (w, h) = (SIZE[0] as usize, SIZE[1] as usize);
    let at = |row: usize| {
        let i = (row * w + w / 2) * 4;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    };
    let background = at(0);
    let lit = (0..h).filter(|&row| at(row) != background).count();
    Some((lit, at(h / 2)))
}

fn red() -> LineColour {
    LineColour::One(Colour([255, 0, 0]))
}

#[test]
fn an_edge_takes_its_style_s_colour_and_a_curve_keeps_its_own() {
    let mut style = ViewStyle::shaded();
    style.nurbs.edges.colour = red();
    let (Some((_, edge)), Some((_, curve))) = (
        drawn(LineKind::Edge, -2.0, &style),
        drawn(LineKind::Curve, -2.0, &style),
    ) else {
        return;
    };
    assert!(
        edge[0] > 200 && edge[1] < 60 && edge[2] < 60,
        "edge {edge:?}"
    );
    assert!(curve.iter().all(|&c| c > 200), "curve {curve:?}");
}

#[test]
fn a_kind_turned_off_draws_nothing_and_given_lines_always_draw() {
    let mut style = ViewStyle::shaded();
    style.curves.shown = false;
    style.curves.colour = red();
    let (Some((curve, _)), Some((given, colour))) = (
        drawn(LineKind::Curve, -2.0, &style),
        drawn(LineKind::Given, -2.0, &style),
    ) else {
        return;
    };
    assert_eq!(curve, 0, "curves off");
    assert_eq!(given, 2, "a preview or locked line still draws");
    assert!(
        colour.iter().all(|&c| c > 200),
        "in its own colour: {colour:?}"
    );
}

#[test]
fn a_fixed_width_and_the_line_weight_set_how_thick_lines_draw() {
    let mut style = ViewStyle::shaded();
    style.curves.colour = red();
    let Some((own, _)) = drawn(LineKind::Curve, -2.0, &style) else {
        return;
    };
    style.curves.width = LineWidth::Fixed(2.0);
    let Some((fixed, _)) = drawn(LineKind::Curve, -2.0, &style) else {
        return;
    };
    style.curves.width = LineWidth::Object(1.0);
    style.line_rules.weight = 3.0;
    let Some((heavier, _)) = drawn(LineKind::Curve, -2.0, &style) else {
        return;
    };
    assert_eq!(own, 2, "the default line, 2 pixels");
    assert!(
        (7..=9).contains(&fixed),
        "2 mm is about 7.6 pixels: {fixed}"
    );
    assert!(
        (5..=7).contains(&heavier),
        "three times 2 pixels: {heavier}"
    );
}

/// How many rows a white horizontal line across a black picture lights in its left and right
/// quarters, drawn in `style` and measured from a camera looking along it from the left, one
/// model unit to a metre. `None` without a graphics adapter.
fn lit_left_and_right(style: &ViewStyle) -> Option<(usize, usize)> {
    let end = |x: f32| Vertex {
        position: [x, 0.0, 0.5],
        colour: [1.0; 3],
        width: -6.0,
        object: NO_OBJECT,
        kind: LineKind::Curve,
    };
    // Looking along +X from (-1, 0, 0): the line's left end is 0.2 away, its right end 1.8;
    // its left quarter is about 0.5 away, its right quarter 1.5.
    let camera = crate::Camera {
        target: [0.0; 3],
        yaw: std::f64::consts::PI,
        pitch: 0.0,
        distance: 1.0,
        ..crate::Camera::default()
    };
    let picture = Picture {
        lines: vec![end(-0.8), end(0.8)],
        matrix: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        light: [0.0, 0.0, 1.0],
        style: Style::of(style).in_units(1000.0),
        camera,
        ..Picture::default()
    };
    let pixels = match draw_picture(picture, SIZE, true) {
        Ok(pixels) => pixels,
        Err(error) => {
            eprintln!("no graphics adapter to draw with, so not checked: {error}");
            return None;
        }
    };
    let (w, h) = (SIZE[0] as usize, SIZE[1] as usize);
    let at = |row: usize, column: usize| pixels[(row * w + column) * 4];
    let lit = |column: usize| {
        (0..h)
            .filter(|&row| at(row, column) > at(0, column))
            .count()
    };
    Some((lit(w / 4), lit(3 * w / 4)))
}

#[test]
fn lines_thin_and_fade_with_distance_where_the_style_says() {
    let mut style = ViewStyle::wireframe();
    style.curves.colour = LineColour::One(Colour([255, 255, 255]));
    let Some((left, right)) = lit_left_and_right(&style) else {
        return;
    };
    assert_eq!(left, right, "no rule: the same everywhere");
    style.line_rules.by_distance = true;
    style.line_rules.near = 0.5;
    style.line_rules.far = 1.3;
    style.line_rules.far_width = 0.25;
    style.line_rules.far_fade = 0.0;
    let Some((near, far)) = lit_left_and_right(&style) else {
        return;
    };
    assert_eq!(near, left, "nearer than `near`: as drawn");
    assert!(far < near, "thinner far away: {far} against {near}");
    style.line_rules.far_width = 1.0;
    style.line_rules.far_fade = 1.0;
    let Some((near, far)) = lit_left_and_right(&style) else {
        return;
    };
    assert_eq!(near, left);
    assert_eq!(far, 0, "faded away entirely beyond `far`");
}

#[test]
fn hidden_draws_model_lines_in_ink_and_leaves_previews_their_colour() {
    let hidden = ViewStyle::hidden();
    let (Some((_, edge)), Some((_, preview))) = (
        drawn(LineKind::Edge, -2.0, &hidden),
        drawn(LineKind::Given, -2.0, &hidden),
    ) else {
        return;
    };
    assert!(edge.iter().all(|&c| c < 40), "ink {edge:?}");
    assert!(preview.iter().all(|&c| c > 200), "own {preview:?}");
}
