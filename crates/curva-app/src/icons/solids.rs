//! Icons: solids.

use super::*;

/// Draws the icon `key` (a command name in lower case) if it is one of these; returns whether it was.
pub(super) fn solids_1(pen: &Pen<'_>, key: &str) -> bool {
    let thin = |w: f32| pen.width(w);
    match key {
        "sphere" => {
            pen.ellipse((3.0, 3.0, 16.0, 16.0));
            pen.dashed(&pen.arc_points((3.0, 8.5, 16.0, 5.0), 0.0, 180.0), 1.0);
            pen.arc((3.0, 8.5, 16.0, 5.0), 180.0, 180.0);
        }
        "cylinder" | "tube" => {
            pen.ellipse((5.0, 2.0, 12.0, 5.0));
            pen.line((5.0, 4.5), (5.0, 17.5));
            pen.line((17.0, 4.5), (17.0, 17.5));
            pen.arc((5.0, 15.0, 12.0, 5.0), 180.0, 180.0);
            if key == "tube" {
                pen.path_in(
                    &pen.arc_points((8.0, 3.0, 6.0, 3.0), 0.0, 360.0),
                    true,
                    pen.accent(),
                );
            }
        }
        "cone" => {
            pen.arc((4.0, 15.0, 14.0, 5.0), 180.0, 180.0);
            pen.dashed(&pen.arc_points((4.0, 15.0, 14.0, 5.0), 0.0, 180.0), 1.0);
            pen.line((4.0, 17.5), (11.0, 3.0));
            pen.line((18.0, 17.5), (11.0, 3.0));
            pen.dot(11.0, 3.0);
        }
        "truncatedcone" => {
            pen.ellipse((7.0, 3.0, 8.0, 3.0));
            pen.arc((3.0, 15.0, 16.0, 5.0), 180.0, 180.0);
            pen.line((7.0, 4.5), (3.0, 17.5));
            pen.line((15.0, 4.5), (19.0, 17.5));
        }
        "torus" => {
            pen.ellipse((2.0, 6.0, 18.0, 10.0));
            pen.arc((7.0, 9.0, 8.0, 4.0), 200.0, 140.0);
            pen.arc((8.0, 8.0, 6.0, 4.0), 20.0, 140.0);
        }
        "ellipsoid" => {
            pen.ellipse((1.0, 5.0, 20.0, 12.0));
            pen.arc((1.0, 9.0, 20.0, 4.0), 180.0, 180.0);
            pen.dashed(&pen.arc_points((1.0, 9.0, 20.0, 4.0), 0.0, 180.0), 1.0);
        }
        "booleansplit" => {
            pen.arc((3.0, 6.0, 11.0, 11.0), 70.0, 250.0);
            pen.dashed(&pen.arc_points((8.0, 5.0, 11.0, 11.0), 0.0, 360.0), 1.1);
            pen.line_in((11.0, 2.0), (11.0, 20.0), pen.accent());
        }
        "pyramid" => {
            pen.path(&[(3.0, 16.0), (13.0, 19.0), (19.0, 14.0)], false);
            pen.dashed_in(&[(3.0, 16.0), (9.0, 12.0), (19.0, 14.0)], thin(1.0));
            pen.line((11.0, 3.0), (3.0, 16.0));
            pen.line((11.0, 3.0), (13.0, 19.0));
            pen.line((11.0, 3.0), (19.0, 14.0));
        }
        "wirecut" => {
            pen.cube(4.0, 5.0, 11.0);
            pen.quad_in((2.0, 4.0), (12.0, 12.0), (20.0, 20.0), pen.accent());
        }
        "moveface" | "pushpull" => {
            pen.cube(4.0, 8.0, 10.0);
            let top = [(4.0, 8.0), (10.0, 4.0), (20.0, 4.0), (14.0, 8.0)];
            pen.filled(&top, pen.accent().color.gamma_multiply(0.5));
            pen.arrow((12.0, 7.0), (12.0, 1.0));
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (a command name in lower case) if it is one of these; returns whether it was.
pub(super) fn solids_2(pen: &Pen<'_>, key: &str) -> bool {
    match key {
        "cap" => {
            pen.ellipse((4.0, 3.0, 14.0, 5.0));
            pen.line((4.0, 5.5), (4.0, 16.5));
            pen.line((18.0, 5.5), (18.0, 16.5));
            pen.arc((4.0, 14.0, 14.0, 5.0), 0.0, 180.0);
            pen.filled(
                &pen.arc_points((4.0, 3.0, 14.0, 5.0), 0.0, 360.0),
                pen.accent().color.gamma_multiply(0.6),
            );
        }
        "extrudeface" => {
            pen.cube(4.0, 8.0, 10.0);
            pen.path_in(
                &[(4.0, 8.0), (10.0, 4.0), (20.0, 4.0), (14.0, 8.0)],
                true,
                pen.accent(),
            );
            pen.arrow((12.0, 6.0), (12.0, 1.0));
        }
        "filletsrf" | "chamfersrf" => {
            pen.line((3.0, 19.0), (19.0, 19.0));
            pen.line((3.0, 3.0), (3.0, 19.0));
            if key == "filletsrf" {
                pen.arc_in((3.0, 9.0, 20.0, 20.0), 180.0, 90.0, pen.accent());
            } else {
                pen.line_in((3.0, 11.0), (11.0, 19.0), pen.accent());
            }
        }
        "editboolean" | "breakboolean" => {
            pen.ellipse((2.0, 5.0, 12.0, 12.0));
            pen.ellipse((8.0, 5.0, 12.0, 12.0));
            if key == "editboolean" {
                pen.dashed_in(&[(8.0, 6.0), (8.0, 16.0)], pen.accent());
                pen.dot(14.0, 11.0);
            } else {
                pen.line_in((3.0, 19.0), (19.0, 3.0), pen.accent());
            }
        }
        _ => return false,
    }
    true
}
