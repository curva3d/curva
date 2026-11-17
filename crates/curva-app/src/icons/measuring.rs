//! Icons: measuring.

use super::*;

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn measuring_1(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    match key {
        "distance" => {
            pen.line((3.0, 6.0), (3.0, 16.0));
            pen.line((19.0, 6.0), (19.0, 16.0));
            pen.line_in((3.0, 11.0), (19.0, 11.0), pen.accent());
            pen.arrow_head((3.0, 11.0), (19.0, 11.0), pen.accent());
            pen.arrow_head((19.0, 11.0), (3.0, 11.0), pen.accent());
        }
        "length" => {
            pen.cubic((3.0, 15.0), (8.0, 5.0), (14.0, 5.0), (19.0, 15.0));
            pen.line_in((3.0, 19.0), (19.0, 19.0), pen.accent());
            pen.line_in((3.0, 17.0), (3.0, 21.0), pen.accent());
            pen.line_in((19.0, 17.0), (19.0, 21.0), pen.accent());
        }
        "area" => {
            let quad = [(4.0, 5.0), (18.0, 4.0), (16.0, 18.0), (5.0, 17.0)];
            pen.path(&quad, true);
            for i in 0..4 {
                let o = i as f32 * 4.0;
                pen.line_in(
                    (5.0 + o, 17.0),
                    (9.0 + o, 5.0),
                    Stroke::new(1.0 * pen.scale(), theme::ACCENT),
                );
            }
        }
        "volume" => {
            pen.filled(
                &[(4.0, 8.0), (14.0, 8.0), (14.0, 18.0), (4.0, 18.0)],
                theme::ACCENT.gamma_multiply(0.45),
            );
            pen.cube(4.0, 4.0, 10.0);
        }
        "angle" => {
            pen.line((4.0, 18.0), (19.0, 18.0));
            pen.line((4.0, 18.0), (14.0, 5.0));
            pen.arc_in((-4.0, 10.0, 16.0, 16.0), 0.0, 52.0, pen.accent());
        }
        "evaluatept" => {
            pen.line((11.0, 3.0), (11.0, 19.0));
            pen.line((3.0, 11.0), (19.0, 11.0));
            pen.ellipse((7.0, 7.0, 8.0, 8.0));
            pen.dot(11.0, 11.0);
        }
        "radius" => {
            pen.ellipse((3.0, 3.0, 16.0, 16.0));
            pen.line_in((11.0, 11.0), (16.7, 5.3), pen.accent());
            pen.dot(11.0, 11.0);
        }
        "what" => {
            pen.ellipse((3.0, 3.0, 16.0, 16.0));
            pen.line_in((11.0, 10.0), (11.0, 16.0), pen.accent());
            pen.dot(11.0, 6.8);
        }
        "areacentroid" | "volumecentroid" => {
            if name.eq_ignore_ascii_case("volumecentroid") {
                pen.cube(4.0, 4.0, 10.0);
            } else {
                pen.path(&[(3.0, 15.0), (8.0, 6.0), (19.0, 6.0), (14.0, 15.0)], true);
            }
            pen.line_in((9.0, 12.0), (13.0, 12.0), pen.accent());
            pen.line_in((11.0, 10.0), (11.0, 14.0), pen.accent());
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn measuring_2(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    match key {
        "showedges" => {
            pen.cube(4.0, 4.0, 10.0);
            pen.line_in((4.0, 18.0), (14.0, 18.0), pen.accent());
            pen.line_in((14.0, 18.0), (18.0, 14.0), pen.accent());
        }
        "curvaturegraph" => {
            pen.quad((3.0, 17.0), (11.0, 5.0), (19.0, 17.0));
            for (x, y, tx, ty) in [
                (6.0, 13.0, 4.0, 9.0),
                (9.0, 10.5, 8.0, 5.0),
                (11.0, 10.0, 11.0, 3.0),
                (13.0, 10.5, 14.0, 5.0),
                (16.0, 13.0, 18.0, 9.0),
            ] {
                pen.line_in((x, y), (tx, ty), pen.accent());
            }
        }
        "zebra" => {
            pen.ellipse((3.0, 3.0, 16.0, 16.0));
            for (a, b) in [
                ((5.0, 7.0), (17.0, 7.0)),
                ((3.5, 11.0), (18.5, 11.0)),
                ((5.0, 15.0), (17.0, 15.0)),
            ] {
                pen.line_in(a, b, pen.accent());
            }
        }
        "count" => {
            for (x, y) in [
                (5.0, 6.0),
                (11.0, 6.0),
                (17.0, 6.0),
                (5.0, 12.0),
                (11.0, 12.0),
            ] {
                pen.dot(x, y);
            }
            pen.line_in((3.0, 18.0), (19.0, 18.0), pen.accent());
        }
        "draftanalysis" => {
            pen.path(&[(4.0, 19.0), (18.0, 19.0), (15.0, 4.0), (7.0, 4.0)], true);
            pen.line_in((15.0, 4.0), (18.0, 19.0), pen.accent());
        }
        _ => return false,
    }
    true
}
