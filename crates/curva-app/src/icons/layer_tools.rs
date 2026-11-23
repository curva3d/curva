//! Icons: the Layers panel's toolbar, after Rhino's, in Curva's own style: thin lines, the part
//! that says what a button does in the accent.

use super::*;

/// A point given as fractions of the toolbar icons' 14-unit drawing, which sits in the middle of
/// the 22-unit box.
fn f(x: f32, y: f32) -> P {
    (4.0 + 14.0 * x, 4.0 + 14.0 * y)
}

/// A layer drawn as a sheet, from `(x0, y0)` to `(x1, y1)` in fractions.
fn sheet(pen: &Pen<'_>, x0: f32, y0: f32, x1: f32, y1: f32) {
    let (a, b) = (f(x0, y0), f(x1, y1));
    pen.rect(a.0, a.1, b.0 - a.0, b.1 - a.1);
}

/// A plus in the accent, centred at `(x, y)` in fractions.
fn plus(pen: &Pen<'_>, x: f32, y: f32) {
    pen.line_in(f(x - 0.2, y), f(x + 0.2, y), pen.accent());
    pen.line_in(f(x, y - 0.2), f(x, y + 0.2), pen.accent());
}

/// An object, as a small ring, at `(x, y)` in fractions.
fn object(pen: &Pen<'_>, x: f32, y: f32) {
    let (cx, cy) = f(x, y);
    pen.ellipse((cx - 1.4, cy - 1.4, 2.8, 2.8));
}

/// Draws the icon `key` if it is one of the Layers toolbar's; returns whether it was.
pub(super) fn layer_tools(pen: &Pen<'_>, key: &str) -> bool {
    match key {
        "layer.new" => {
            sheet(pen, 0.0, 0.15, 0.7, 0.75);
            plus(pen, 0.78, 0.78);
        }
        "layer.new_sublayer" => {
            sheet(pen, 0.0, 0.0, 0.6, 0.35);
            sheet(pen, 0.25, 0.5, 0.75, 0.85);
            plus(pen, 0.85, 0.82);
        }
        "layer.delete" => {
            pen.line_in(f(0.15, 0.15), f(0.85, 0.85), pen.accent());
            pen.line_in(f(0.85, 0.15), f(0.15, 0.85), pen.accent());
        }
        "layer.change_object_layer" => {
            sheet(pen, 0.45, 0.2, 1.0, 0.8);
            object(pen, 0.12, 0.5);
            accent_arrow(pen);
        }
        "layer.copy_to_layer" => {
            sheet(pen, 0.45, 0.2, 1.0, 0.8);
            object(pen, 0.12, 0.4);
            object(pen, 0.12, 0.62);
            accent_arrow(pen);
        }
        "layer.move_up" => {
            pen.path_in(
                &[f(0.5, 0.2), f(0.85, 0.75), f(0.15, 0.75)],
                true,
                pen.accent(),
            );
        }
        "layer.move_down" => {
            pen.path_in(
                &[f(0.5, 0.8), f(0.85, 0.25), f(0.15, 0.25)],
                true,
                pen.accent(),
            );
        }
        "layer.up_one_parent" => {
            pen.path_in(
                &[f(0.2, 0.5), f(0.75, 0.15), f(0.75, 0.85)],
                true,
                pen.accent(),
            );
        }
        "layer.filters" => {
            let funnel = [
                f(0.05, 0.15),
                f(0.95, 0.15),
                f(0.6, 0.55),
                f(0.6, 0.9),
                f(0.4, 0.8),
                f(0.4, 0.55),
            ];
            pen.path(&funnel, true);
        }
        "layer.tools" => {
            for y in [0.25, 0.5, 0.75] {
                pen.line(f(0.1, y), f(0.9, y));
            }
        }
        _ => return false,
    }
    true
}

/// The accent arrow from an object to a layer.
fn accent_arrow(pen: &Pen<'_>) {
    let (from, to) = (f(0.25, 0.5), f(0.62, 0.5));
    pen.line_in(from, to, pen.accent());
    pen.arrow_head(to, from, pen.accent());
}
