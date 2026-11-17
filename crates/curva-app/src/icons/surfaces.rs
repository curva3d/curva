//! Icons: surfaces and solids.

use super::*;

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn surfaces_1(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    let fill = pen.ink.gamma_multiply(0.27);
    match key {
        "extrudecrv" => {
            pen.rect(5.0, 12.0, 12.0, 6.0);
            pen.line((11.0, 10.0), (11.0, 3.0));
            pen.line((8.0, 6.0), (11.0, 3.0));
            pen.line((14.0, 6.0), (11.0, 3.0));
        }
        "revolve" => {
            pen.line((11.0, 3.0), (11.0, 19.0));
            pen.ellipse((4.0, 12.0, 14.0, 5.0));
            pen.quad((15.0, 6.0), (18.0, 8.0), (16.0, 11.0));
            pen.line((16.0, 11.0), (14.4, 9.4));
        }
        "loft" => {
            pen.quad((4.0, 6.0), (11.0, 3.0), (18.0, 6.0));
            pen.quad((4.0, 16.0), (11.0, 13.0), (18.0, 16.0));
            pen.line((4.0, 6.0), (4.0, 16.0));
            pen.line((18.0, 6.0), (18.0, 16.0));
        }
        "planarsrf" => {
            let outline = [(6.0, 5.0), (18.0, 5.0), (16.0, 17.0), (4.0, 17.0)];
            pen.filled(&outline, fill);
            pen.path(&outline, true);
        }
        "sweep1" => {
            pen.cubic((4.0, 17.0), (8.0, 17.0), (12.0, 8.0), (18.0, 6.0));
            pen.ellipse((2.0, 14.5, 5.0, 5.0));
        }
        "offsetsrf" => {
            pen.path(
                &[(3.0, 15.0), (8.0, 10.0), (19.0, 10.0), (14.0, 15.0)],
                true,
            );
            pen.dashed_in(
                &[(3.0, 9.0), (8.0, 4.0), (19.0, 4.0), (14.0, 9.0), (3.0, 9.0)],
                pen.accent(),
            );
        }
        "pipe" => {
            pen.ellipse((2.0, 7.0, 5.0, 8.0));
            pen.arc((15.0, 7.0, 5.0, 8.0), -90.0, 180.0);
            pen.line((4.5, 7.0), (17.5, 7.0));
            pen.line((4.5, 15.0), (17.5, 15.0));
        }
        "box" => pen.cube(4.0, 4.0, 10.0),
        "filletedge" | "chamferedge" | "shell" => {
            pen.cube(4.0, 4.0, 10.0);
            match name.to_lowercase().as_str() {
                "filletedge" => pen.quad_in((14.0, 11.0), (14.0, 8.0), (17.0, 6.0), pen.accent()),
                "chamferedge" => pen.line_in((12.0, 8.0), (16.0, 4.5), pen.accent()),
                _ => pen.path_in(&[(6.5, 9.5), (8.5, 7.0), (15.5, 7.0)], false, pen.accent()),
            }
        }
        "booleanunion" => {
            if let Some(painter) = pen.painter {
                painter.circle(pen.at((8.5, 11.5)), 5.5 * pen.scale(), fill, Stroke::NONE);
                painter.circle(pen.at((13.5, 10.5)), 5.5 * pen.scale(), fill, Stroke::NONE);
            }
            pen.arc((3.0, 6.0, 11.0, 11.0), 70.0, 250.0);
            pen.arc((8.0, 5.0, 11.0, 11.0), -110.0, 250.0);
        }
        "booleandifference" => {
            pen.arc((3.0, 6.0, 11.0, 11.0), 70.0, 250.0);
            pen.arc((8.0, 5.0, 11.0, 11.0), 110.0, 140.0);
            pen.dashed(&pen.arc_points((8.0, 5.0, 11.0, 11.0), 0.0, 360.0), 1.1);
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn surfaces_2(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    let fill = pen.ink.gamma_multiply(0.27);
    let thin = |w: f32| pen.width(w);
    match key {
        "booleanintersection" => {
            for frame in [(3.0, 6.0, 11.0, 11.0), (8.0, 5.0, 11.0, 11.0)] {
                pen.dashed(&pen.arc_points(frame, 0.0, 360.0), 1.1);
            }
            let mut lens = pen.arc_points((3.0, 6.0, 11.0, 11.0), -70.0, 140.0);
            lens.extend(pen.arc_points((8.0, 5.0, 11.0, 11.0), 110.0, 140.0));
            pen.filled(&lens, theme::ACCENT.gamma_multiply(0.45));
            pen.path(&lens, true);
        }
        "plane" => {
            let quad = [(3.0, 15.0), (8.0, 6.0), (19.0, 6.0), (14.0, 15.0)];
            pen.filled(&quad, fill);
            pen.path(&quad, true);
        }
        "srfpt" => {
            let quad = [(4.0, 16.0), (7.0, 5.0), (18.0, 6.0), (16.0, 17.0)];
            pen.filled(&quad, fill);
            pen.path(&quad, true);
            for (x, y) in quad {
                pen.dot(x, y);
            }
        }
        "extrudesrf" => {
            let quad = [(3.0, 18.0), (7.0, 14.0), (19.0, 14.0), (15.0, 18.0)];
            pen.filled(&quad, fill);
            pen.path(&quad, true);
            pen.line_in((11.0, 13.0), (11.0, 3.0), pen.accent());
            pen.arrow_head((11.0, 3.0), (11.0, 13.0), pen.accent());
        }
        "paraboloid" => {
            pen.ellipse((3.0, 3.0, 16.0, 5.0));
            pen.quad((3.0, 5.5), (11.0, 30.0), (19.0, 5.5));
        }
        "extrudecrvtapered" => {
            pen.path(&[(3.0, 18.0), (19.0, 18.0), (15.0, 6.0), (7.0, 6.0)], true);
            pen.line_in((11.0, 16.0), (11.0, 2.0), pen.accent());
        }
        "extrudecrvalongcrv" => {
            pen.rect(3.0, 13.0, 6.0, 6.0);
            pen.rect(13.0, 3.0, 6.0, 6.0);
            pen.quad_in((6.0, 13.0), (6.0, 6.0), (13.0, 6.0), pen.accent());
        }
        "extrudecrvtopoint" => {
            pen.ellipse((3.0, 14.0, 16.0, 5.0));
            pen.line((3.0, 16.5), (11.0, 3.0));
            pen.line((19.0, 16.5), (11.0, 3.0));
            pen.dot(11.0, 3.0);
        }
        "railrevolve" => {
            pen.line_in((11.0, 2.0), (11.0, 20.0), thin(1.0));
            pen.ellipse((2.0, 12.0, 18.0, 7.0));
            pen.quad_in((15.0, 4.0), (19.0, 9.0), (17.0, 14.0), pen.accent());
        }
        "extractsrf" => {
            pen.cube(3.0, 7.0, 10.0);
            let face = [(12.0, 2.0), (20.0, 2.0), (20.0, 10.0), (12.0, 10.0)];
            pen.filled(&face, fill);
            pen.path_in(&face, true, pen.accent());
        }
        "mergeallfaces" => {
            pen.rect(3.0, 6.0, 16.0, 10.0);
            pen.dashed_in(&[(11.0, 6.0), (11.0, 16.0)], thin(1.2));
            pen.line_in((7.0, 11.0), (15.0, 11.0), pen.accent());
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn surfaces_3(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    let fill = pen.ink.gamma_multiply(0.27);
    let thin = |w: f32| pen.width(w);
    match key {
        "edgesrf" => {
            let outline = [(4.0, 4.0), (18.0, 6.0), (17.0, 18.0), (3.0, 16.0)];
            pen.filled(&outline, fill);
            for pair in outline.windows(2) {
                pen.line_in(pair[0], pair[1], pen.accent());
            }
            pen.line_in(outline[3], outline[0], pen.accent());
        }
        "networksrf" => {
            for y in [5.0, 11.0, 17.0] {
                pen.quad((3.0, y), (11.0, y - 3.0), (19.0, y));
            }
            for x in [4.0, 11.0, 18.0] {
                pen.line_in((x, 3.0), (x, 19.0), pen.accent());
            }
        }
        "patch" => {
            let outline = [
                (5.0, 4.0),
                (17.0, 5.0),
                (19.0, 15.0),
                (10.0, 19.0),
                (3.0, 13.0),
            ];
            pen.filled(&outline, fill);
            pen.path_in(&outline, true, pen.accent());
            pen.dot(11.0, 11.0);
        }
        "extendsrf" => {
            pen.path(&[(3.0, 6.0), (12.0, 6.0), (12.0, 16.0), (3.0, 16.0)], true);
            pen.dashed_in(
                &[(12.0, 6.0), (19.0, 6.0), (19.0, 16.0), (12.0, 16.0)],
                thin(1.3),
            );
            pen.line_in((12.0, 6.0), (12.0, 16.0), pen.accent());
        }
        "untrim" => {
            pen.rect(3.0, 3.0, 16.0, 16.0);
            pen.dashed_in(
                &[
                    (8.0, 8.0),
                    (14.0, 8.0),
                    (14.0, 14.0),
                    (8.0, 14.0),
                    (8.0, 8.0),
                ],
                pen.accent(),
            );
        }
        "rebuilduv" => {
            pen.path(&[(3.0, 5.0), (19.0, 5.0), (19.0, 17.0), (3.0, 17.0)], true);
            for x in [7.0, 11.0, 15.0] {
                pen.line_in((x, 5.0), (x, 17.0), thin(1.0));
            }
            for y in [9.0, 13.0] {
                pen.line_in((3.0, y), (19.0, y), thin(1.0));
            }
            pen.dot(11.0, 9.0);
        }
        "extractisocurve" => {
            pen.path(&[(3.0, 5.0), (19.0, 5.0), (19.0, 17.0), (3.0, 17.0)], true);
            pen.line_in((11.0, 5.0), (11.0, 17.0), pen.accent());
            pen.line_in((3.0, 11.0), (19.0, 11.0), thin(1.0));
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn surfaces_4(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    let thin = |w: f32| pen.width(w);
    match key {
        "shrinktrimmedsrf" => {
            pen.dashed_in(
                &[
                    (2.0, 2.0),
                    (20.0, 2.0),
                    (20.0, 20.0),
                    (2.0, 20.0),
                    (2.0, 2.0),
                ],
                thin(1.0),
            );
            pen.rect(6.0, 6.0, 10.0, 10.0);
            pen.ellipse((7.0, 7.0, 8.0, 8.0));
        }
        "blendsrf" => {
            pen.path(&[(2.0, 4.0), (8.0, 4.0), (8.0, 9.0), (2.0, 9.0)], true);
            pen.path(
                &[(14.0, 13.0), (20.0, 13.0), (20.0, 18.0), (14.0, 18.0)],
                true,
            );
            pen.cubic_in(
                (8.0, 6.5),
                (12.0, 6.5),
                (10.0, 15.5),
                (14.0, 15.5),
                pen.accent(),
            );
        }
        "matchsrf" => {
            pen.path(&[(2.0, 6.0), (10.0, 6.0), (10.0, 16.0), (2.0, 16.0)], true);
            pen.path(
                &[(12.0, 6.0), (20.0, 6.0), (20.0, 16.0), (12.0, 16.0)],
                true,
            );
            pen.line_in((10.0, 6.0), (10.0, 16.0), pen.accent());
            pen.line_in((12.0, 6.0), (12.0, 16.0), pen.accent());
        }
        "sweep2" => {
            pen.line((3.0, 19.0), (7.0, 3.0));
            pen.line((15.0, 19.0), (19.0, 3.0));
            pen.quad_in((4.0, 15.0), (10.0, 10.0), (16.0, 15.0), pen.accent());
            pen.quad_in((6.0, 7.0), (12.0, 2.0), (18.0, 7.0), thin(1.0));
        }
        "unrollsrf" => {
            pen.arc((2.0, 6.0, 10.0, 12.0), 90.0, 180.0);
            pen.path(
                &[(12.0, 6.0), (20.0, 6.0), (20.0, 18.0), (12.0, 18.0)],
                true,
            );
            pen.arrow((8.0, 12.0), (13.0, 12.0));
        }
        "splitsrf" => {
            pen.path(&[(3.0, 5.0), (19.0, 5.0), (19.0, 17.0), (3.0, 17.0)], true);
            pen.quad_in((10.0, 3.0), (13.0, 11.0), (10.0, 19.0), pen.accent());
        }
        _ => return false,
    }
    true
}
