//! Icons: transforms.

use super::*;

/// Draws the icon `key` (a command name in lower case) if it is one of these; returns whether it was.
pub(super) fn transforms_1(pen: &Pen<'_>, key: &str) -> bool {
    let fill = pen.ink.gamma_multiply(0.27);
    match key {
        "move" => {
            pen.line((11.0, 4.0), (11.0, 18.0));
            pen.line((4.0, 11.0), (18.0, 11.0));
            for (a, b, c) in [
                ((11.0, 4.0), (9.0, 6.0), (13.0, 6.0)),
                ((11.0, 18.0), (9.0, 16.0), (13.0, 16.0)),
                ((4.0, 11.0), (6.0, 9.0), (6.0, 13.0)),
                ((18.0, 11.0), (16.0, 9.0), (16.0, 13.0)),
            ] {
                pen.line(a, b);
                pen.line(a, c);
            }
        }
        "copy" => {
            pen.rect(7.0, 7.0, 11.0, 11.0);
            pen.dashed(
                &[
                    (4.0, 4.0),
                    (15.0, 4.0),
                    (15.0, 15.0),
                    (4.0, 15.0),
                    (4.0, 4.0),
                ],
                1.4,
            );
        }
        "rotate" => {
            pen.arc((4.5, 4.5, 13.0, 13.0), 40.0, 250.0);
            pen.line((15.5, 3.5), (15.8, 7.4));
            pen.line((15.8, 7.4), (12.0, 6.4));
            pen.dot(11.0, 11.0);
        }
        "rotate3d" => {
            pen.arc((4.5, 6.0, 13.0, 9.0), 30.0, 280.0);
            pen.arrow_head((16.4, 7.2), (13.0, 6.2), pen.stroke());
            pen.line_in((4.0, 19.0), (18.0, 3.0), pen.accent());
        }
        "scale" => {
            pen.rect(4.0, 12.0, 6.0, 6.0);
            pen.rect(8.0, 4.0, 10.0, 10.0);
            pen.line((6.0, 16.0), (15.0, 7.0));
        }
        "scale1d" | "scale2d" | "scalenu" => {
            pen.rect(4.0, 7.0, 9.0, 9.0);
            pen.line_in((13.0, 11.5), (19.0, 11.5), pen.accent());
            pen.arrow_head((19.0, 11.5), (13.0, 11.5), pen.accent());
            if key != "scale1d" {
                pen.line_in((8.5, 7.0), (8.5, 2.0), pen.accent());
                pen.arrow_head((8.5, 2.0), (8.5, 7.0), pen.accent());
            }
            if key == "scalenu" {
                pen.line_in((4.0, 16.0), (1.5, 19.5), pen.accent());
                pen.arrow_head((1.5, 19.5), (4.0, 16.0), pen.accent());
            }
        }
        "mirror" => {
            pen.dashed(&[(11.0, 3.0), (11.0, 19.0)], 1.2);
            let left = [(8.0, 6.0), (8.0, 16.0), (3.0, 16.0)];
            pen.filled(&left, fill);
            pen.path(&left, true);
            pen.path(&[(14.0, 6.0), (14.0, 16.0), (19.0, 16.0)], true);
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (a command name in lower case) if it is one of these; returns whether it was.
pub(super) fn transforms_2(pen: &Pen<'_>, key: &str) -> bool {
    match key {
        "array" => {
            for row in 0..3 {
                for col in 0..3 {
                    let (x, y) = (3.0 + col as f32 * 6.0, 3.0 + row as f32 * 6.0);
                    if row == 0 && col == 0 {
                        pen.filled(
                            &[(x, y), (x + 4.0, y), (x + 4.0, y + 4.0), (x, y + 4.0)],
                            theme::ACCENT,
                        );
                    } else {
                        pen.rect(x, y, 4.0, 4.0);
                    }
                }
            }
        }
        "arraypolar" => {
            pen.dashed(&pen.arc_points((3.0, 3.0, 16.0, 16.0), 0.0, 360.0), 0.9);
            for i in 0..6 {
                let angle = (i as f32 * 60.0).to_radians();
                let (x, y) = (11.0 + 8.0 * angle.cos(), 11.0 - 8.0 * angle.sin());
                if i == 0 {
                    pen.dot(x, y);
                } else {
                    pen.ring(x, y);
                }
            }
            pen.dot(11.0, 11.0);
        }
        "arraycrv" => {
            pen.cubic((2.0, 17.0), (7.0, 3.0), (14.0, 19.0), (20.0, 5.0));
            for (x, y) in [(4.4, 12.3), (10.8, 11.0), (17.5, 9.4)] {
                pen.rect(x - 1.8, y - 1.8, 3.6, 3.6);
            }
        }
        "orient" | "orient3pt" => {
            pen.rect(3.0, 13.0, 5.0, 5.0);
            pen.rect(12.0, 3.0, 8.0, 8.0);
            pen.dashed(&[(8.0, 13.0), (12.0, 11.0)], 1.0);
            if key == "orient3pt" {
                for (x, y) in [(12.0, 11.0), (20.0, 11.0), (12.0, 3.0)] {
                    pen.dot(x, y);
                }
            } else {
                pen.dot(12.0, 11.0);
                pen.dot(20.0, 3.0);
            }
        }
        "arraylinear" => {
            for i in 0..4 {
                let x = 2.0 + i as f32 * 5.0;
                if i == 0 {
                    pen.filled(
                        &[(x, 9.0), (x + 3.5, 9.0), (x + 3.5, 12.5), (x, 12.5)],
                        theme::ACCENT,
                    );
                } else {
                    pen.rect(x, 9.0, 3.5, 3.5);
                }
            }
        }
        "align" => {
            pen.line_in((3.0, 19.0), (19.0, 19.0), pen.accent());
            pen.rect(4.0, 11.0, 5.0, 8.0);
            pen.rect(12.0, 6.0, 6.0, 13.0);
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (a command name in lower case) if it is one of these; returns whether it was.
pub(super) fn transforms_3(pen: &Pen<'_>, key: &str) -> bool {
    let thin = |w: f32| pen.width(w);
    match key {
        "distribute" => {
            for x in [3.0, 9.5, 16.0] {
                pen.rect(x, 7.0, 3.0, 8.0);
            }
            pen.line_in((3.0, 18.0), (19.0, 18.0), pen.accent());
        }
        "shear" => {
            pen.dashed_in(
                &[(4.0, 18.0), (4.0, 6.0), (14.0, 6.0), (14.0, 18.0)],
                thin(1.0),
            );
            pen.path(&[(4.0, 18.0), (9.0, 6.0), (19.0, 6.0), (14.0, 18.0)], true);
        }
        "twist" => {
            pen.cubic((6.0, 19.0), (6.0, 11.0), (16.0, 11.0), (16.0, 3.0));
            pen.cubic((16.0, 19.0), (16.0, 11.0), (6.0, 11.0), (6.0, 3.0));
            pen.line_in((11.0, 2.0), (11.0, 20.0), pen.accent());
        }
        "bend" => {
            pen.arc((3.0, 5.0, 16.0, 28.0), 180.0, 90.0);
            pen.dashed_in(&[(3.0, 19.0), (3.0, 3.0)], thin(1.0));
            pen.dot(11.0, 5.0);
        }
        "taper" => {
            pen.path(&[(4.0, 19.0), (18.0, 19.0), (14.0, 3.0), (8.0, 3.0)], true);
            pen.line_in((11.0, 2.0), (11.0, 20.0), pen.accent());
        }
        "flow" => {
            pen.line((3.0, 18.0), (12.0, 18.0));
            pen.rect(5.0, 14.0, 4.0, 3.0);
            pen.quad_in((10.0, 12.0), (14.0, 2.0), (20.0, 6.0), pen.accent());
            pen.arrow((9.0, 13.0), (13.0, 8.0));
        }
        "cageedit" => {
            pen.cube(4.0, 6.0, 11.0);
            pen.dot(4.0, 6.0);
            pen.dot(15.0, 17.0);
            pen.line_in((15.0, 17.0), (20.0, 20.0), pen.accent());
        }
        _ => return false,
    }
    true
}
