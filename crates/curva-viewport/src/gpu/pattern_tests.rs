//! Lines dashed, dotted and see-through as their view's style gives them, on the graphics card.

use super::{Picture, draw_picture};
use crate::{LineKind, NO_OBJECT, Style, Vertex};
use curva_document::style::{Colour, LineColour, Pattern, ViewStyle};

const SIZE: [u32; 2] = [96, 32];

/// The middle row of a picture of one white horizontal curve, 2 pixels wide, drawn in `style`:
/// each pixel's red. `None` without a graphics adapter.
fn middle_row(style: Style) -> Option<Vec<u8>> {
    let end = |x: f32| Vertex {
        position: [x, 0.0, 0.5],
        colour: [1.0; 3],
        width: -2.0,
        object: NO_OBJECT,
        kind: LineKind::Curve,
    };
    let picture = Picture {
        lines: vec![end(-0.9), end(0.9)],
        matrix: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        light: [0.0, 0.0, 1.0],
        style,
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
    // Of the two rows the line lights, the lower one.
    let row = h / 2;
    Some((0..w).map(|x| pixels[(row * w + x) * 4]).collect())
}

/// A white curve's style, with curves drawn in `pattern`.
fn curves(pattern: Pattern) -> Style {
    let mut view = ViewStyle::wireframe();
    view.curves.colour = LineColour::One(Colour([255, 255, 255]));
    view.curves.pattern = pattern;
    Style::of(&view)
}

/// How many pixels of the line's middle 80 % are lit, and how many runs of lit pixels it has.
fn lit_and_runs(row: &[u8]) -> (usize, usize) {
    let background = row[0];
    let middle = &row[row.len() / 10..row.len() * 9 / 10];
    let lit: Vec<bool> = middle.iter().map(|&red| red > background + 40).collect();
    let runs = lit
        .iter()
        .enumerate()
        .filter(|&(i, &on)| on && (i == 0 || !lit[i - 1]))
        .count();
    (lit.iter().filter(|&&on| on).count(), runs)
}

#[test]
fn curves_are_solid_dashed_or_dotted_as_the_style_says() {
    let (Some(solid), Some(dashed), Some(dotted)) = (
        middle_row(curves(Pattern::Solid)),
        middle_row(curves(Pattern::Dashed)),
        middle_row(curves(Pattern::Dotted)),
    ) else {
        return;
    };
    let (solid_lit, solid_runs) = lit_and_runs(&solid);
    let (dashed_lit, dashed_runs) = lit_and_runs(&dashed);
    let (dotted_lit, dotted_runs) = lit_and_runs(&dotted);
    assert_eq!(solid_runs, 1, "one unbroken line");
    // 76 pixels: 12-pixel dashes and gaps make about six dashes, 6-pixel dots about twelve.
    assert!((5..=8).contains(&dashed_runs), "dashes: {dashed_runs}");
    assert!((11..=14).contains(&dotted_runs), "dots: {dotted_runs}");
    assert!(
        dotted_lit < dashed_lit && dashed_lit < solid_lit,
        "{dotted_lit} < {dashed_lit} < {solid_lit}"
    );
}

#[test]
fn a_see_through_kind_of_line_lets_the_background_through() {
    let mut style = curves(Pattern::Solid);
    let Some(solid) = middle_row(style) else {
        return;
    };
    style.lines.0[LineKind::Curve as usize].opacity = 0.5;
    let Some(half) = middle_row(style) else {
        return;
    };
    let (background, full, part) = (solid[0], solid[48], half[48]);
    assert!(
        background + 40 < part && part + 40 < full,
        "between the background {background} and the line {full}: {part}"
    );
}
