//! Icons: curves.

use super::*;

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn curves_1(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    match key {
        "line" => {
            pen.line((4.0, 18.0), (18.0, 4.0));
            pen.dot(4.0, 18.0);
            pen.dot(18.0, 4.0);
        }
        "polyline" => pen.path(&[(3.0, 18.0), (8.0, 6.0), (13.0, 14.0), (19.0, 4.0)], false),
        // Curve: the control polygon stands off the curve, its points away from it.
        "curve" => {
            pen.dashed(&[(3.0, 18.0), (6.0, 4.0), (16.0, 4.0), (19.0, 18.0)], 1.0);
            pen.cubic((3.0, 18.0), (6.0, 4.0), (16.0, 4.0), (19.0, 18.0));
            for (x, y) in [(3.0, 18.0), (6.0, 4.0), (16.0, 4.0), (19.0, 18.0)] {
                pen.dot(x, y);
            }
        }
        // InterpCrv: the curve passes through every point.
        "interpcrv" => {
            pen.cubic((3.0, 16.0), (7.0, 2.0), (13.0, 22.0), (19.0, 6.0));
            for (x, y) in [(3.0, 16.0), (8.4, 10.6), (13.6, 11.4), (19.0, 6.0)] {
                pen.dot(x, y);
            }
        }
        "circle" => pen.ellipse((4.0, 4.0, 14.0, 14.0)),
        "arc" => {
            pen.arc((3.0, 6.0, 16.0, 22.0), 30.0, 120.0);
            pen.dot(4.6, 14.5);
            pen.dot(17.4, 14.5);
        }
        "ellipse" => {
            pen.ellipse((2.0, 6.0, 18.0, 10.0));
            pen.dot(11.0, 11.0);
        }
        "rectangle" => pen.rect(4.0, 6.0, 14.0, 10.0),
        // A sheet, landscape, with a detail on it.
        "layout" => {
            pen.rect(2.0, 5.0, 18.0, 12.0);
            pen.rect(4.0, 7.0, 9.0, 8.0);
            pen.line((15.0, 14.0), (18.0, 14.0));
        }
        // A plane seen at an angle, with its arrow.
        "clippingplane" => {
            pen.path(&[(3.0, 15.0), (9.0, 6.0), (19.0, 6.0), (13.0, 15.0)], true);
            pen.line((11.0, 10.5), (11.0, 3.0));
            pen.line((11.0, 3.0), (9.0, 5.5));
            pen.line((11.0, 3.0), (13.0, 5.5));
        }
        // A block cut level through its middle, the cut face on top hatched.
        "sectionstyle" => {
            pen.path(
                &[(3.0, 11.0), (15.0, 11.0), (15.0, 19.0), (3.0, 19.0)],
                true,
            );
            pen.path(&[(3.0, 11.0), (8.0, 5.0), (20.0, 5.0), (15.0, 11.0)], true);
            pen.path(&[(15.0, 19.0), (20.0, 13.0), (20.0, 5.0)], false);
            pen.line((7.0, 11.0), (12.0, 5.0));
            pen.line((11.0, 11.0), (16.0, 5.0));
        }
        // The plane with a tick: it cuts this view.
        "enableclippingplane" => {
            pen.path(&[(2.0, 13.0), (7.0, 6.0), (15.0, 6.0), (10.0, 13.0)], true);
            pen.path(&[(13.0, 17.0), (15.5, 19.5), (20.0, 14.0)], false);
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn curves_2(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    match key {
        // The plane crossed out: it no longer cuts this view.
        "disableclippingplane" => {
            pen.path(&[(2.0, 13.0), (7.0, 6.0), (15.0, 6.0), (10.0, 13.0)], true);
            pen.line((14.0, 14.0), (20.0, 20.0));
            pen.line((20.0, 14.0), (14.0, 20.0));
        }
        // A horizontal dimension: extension lines, the line between them with its arrows.
        "dim" => {
            pen.line((3.0, 8.0), (3.0, 18.0));
            pen.line((19.0, 8.0), (19.0, 18.0));
            pen.line((3.0, 11.0), (19.0, 11.0));
            pen.path(&[(6.0, 9.5), (3.0, 11.0), (6.0, 12.5)], false);
            pen.path(&[(16.0, 9.5), (19.0, 11.0), (16.0, 12.5)], false);
        }
        // A dimension along a slanted edge.
        "dimaligned" => {
            pen.line((3.0, 17.0), (15.0, 5.0));
            pen.line((6.0, 20.0), (18.0, 8.0));
            pen.line((4.5, 18.5), (16.5, 6.5));
        }
        // A dimension at a set angle, with the angle's arc.
        "dimrotated" => {
            pen.line((3.0, 19.0), (19.0, 19.0));
            pen.line((3.0, 19.0), (15.0, 7.0));
            pen.path(&[(9.0, 19.0), (8.5, 16.5), (7.2, 14.8)], false);
        }
        // A base corner, and two points with leaders jogging up to their values.
        "dimordinate" => {
            pen.line((3.0, 20.0), (20.0, 20.0));
            pen.line((3.0, 20.0), (3.0, 3.0));
            pen.dot(8.0, 16.0);
            pen.path(&[(8.0, 14.0), (8.0, 9.0), (10.0, 7.0), (10.0, 3.0)], false);
            pen.dot(16.0, 16.0);
            pen.path(
                &[(16.0, 14.0), (16.0, 9.0), (18.0, 7.0), (18.0, 3.0)],
                false,
            );
        }
        // A curve with a leader from it to its length's note.
        "dimcurvelength" => {
            pen.cubic((3.0, 19.0), (6.0, 9.0), (11.0, 21.0), (14.0, 12.0));
            pen.line((9.0, 15.0), (14.0, 6.0));
            pen.line((14.0, 6.0), (21.0, 6.0));
        }
        // A closed outline with a leader from inside it to its area's note.
        "dimarea" => {
            pen.path(&[(3.0, 10.0), (12.0, 9.0), (13.0, 20.0), (4.0, 20.0)], true);
            pen.line((8.0, 15.0), (15.0, 6.0));
            pen.line((15.0, 6.0), (21.0, 6.0));
        }
        // A box with a leader from it to its volume's note.
        "dimvolume" => {
            pen.cube(3.0, 10.0, 9.0);
            pen.line((9.0, 14.0), (15.0, 6.0));
            pen.line((15.0, 6.0), (21.0, 6.0));
        }
        // A circle with a leader from its centre out past the edge, ending in a landing.
        "dimradius" => {
            pen.ellipse((2.0, 6.0, 14.0, 14.0));
            pen.dot(9.0, 13.0);
            pen.line((9.0, 13.0), (17.0, 5.0));
            pen.line((17.0, 5.0), (21.0, 5.0));
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn curves_3(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    match key {
        // A circle with a line right across it, out to a landing.
        "dimdiameter" => {
            pen.ellipse((2.0, 6.0, 14.0, 14.0));
            pen.line((4.0, 18.0), (17.0, 5.0));
            pen.line((17.0, 5.0), (21.0, 5.0));
        }
        // Two sides from a corner with the dimension arc between them.
        "dimangle" => {
            pen.line((3.0, 19.0), (20.0, 19.0));
            pen.line((3.0, 19.0), (15.0, 4.0));
            pen.arc((-9.0, 7.0, 24.0, 24.0), 0.0, 51.0);
        }
        // An arc, and a wider dimension arc about the same centre with the arc symbol over it.
        "dimarclength" => {
            pen.arc((3.0, 9.0, 16.0, 16.0), 20.0, 140.0);
            pen.arc((0.0, 5.0, 22.0, 22.0), 20.0, 140.0);
            pen.arc((8.0, 1.0, 6.0, 4.0), 0.0, 180.0);
        }
        // A label box with a word in it, at a point.
        "dot" => {
            pen.path(&[(4.0, 6.0), (18.0, 6.0), (18.0, 15.0), (4.0, 15.0)], true);
            pen.line((8.0, 10.5), (14.0, 10.5));
            pen.dot(11.0, 19.0);
        }
        // A square filled with slanted lines.
        "hatch" => {
            pen.path(&[(3.0, 3.0), (19.0, 3.0), (19.0, 19.0), (3.0, 19.0)], true);
            pen.line((3.0, 11.0), (11.0, 3.0));
            pen.line((3.0, 19.0), (19.0, 3.0));
            pen.line((11.0, 19.0), (19.0, 11.0));
        }
        // A hatched square with its pattern's base point marked.
        "sethatchbasepoint" => {
            pen.path(&[(3.0, 3.0), (19.0, 3.0), (19.0, 19.0), (3.0, 19.0)], true);
            pen.line((3.0, 11.0), (11.0, 3.0));
            pen.line((11.0, 19.0), (19.0, 11.0));
            pen.ellipse((11.0, 11.0, 4.0, 4.0));
            pen.dot(13.0, 13.0);
        }
        // Three swatches of patterns, as a pattern list shows them.
        "hatchpatterns" => {
            pen.path(&[(3.0, 3.0), (10.0, 3.0), (10.0, 10.0), (3.0, 10.0)], true);
            pen.line((3.0, 10.0), (10.0, 3.0));
            pen.path(
                &[(12.0, 3.0), (19.0, 3.0), (19.0, 10.0), (12.0, 10.0)],
                true,
            );
            pen.line((15.5, 3.0), (15.5, 10.0));
            pen.line((12.0, 6.5), (19.0, 6.5));
            pen.path(
                &[(3.0, 12.0), (19.0, 12.0), (19.0, 19.0), (3.0, 19.0)],
                true,
            );
            pen.line((3.0, 15.5), (19.0, 15.5));
        }
        // An arrow up to a bend, then on to lines of a note.
        "leader" => {
            pen.path(&[(3.0, 19.0), (9.0, 8.0), (12.0, 8.0)], false);
            pen.path(&[(3.0, 15.5), (3.0, 19.0), (6.0, 17.5)], false);
            pen.line((14.0, 6.0), (21.0, 6.0));
            pen.line((14.0, 10.0), (19.0, 10.0));
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn curves_4(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    match key {
        // A dimension with its text: the look a style sets.
        "annotationstyle" => {
            pen.line((3.0, 14.0), (19.0, 14.0));
            pen.line((3.0, 10.0), (3.0, 18.0));
            pen.line((19.0, 10.0), (19.0, 18.0));
            pen.line((7.0, 4.0), (15.0, 4.0));
            pen.line((11.0, 4.0), (11.0, 10.0));
        }
        // The style marked as current.
        "setcurrentannotationstyle" => {
            pen.line((3.0, 14.0), (15.0, 14.0));
            pen.line((3.0, 10.0), (3.0, 18.0));
            pen.line((15.0, 10.0), (15.0, 18.0));
            pen.path(&[(14.0, 5.0), (16.5, 7.5), (20.0, 2.0)], false);
        }
        // Two styles, one a copy of the other.
        "dupannotationstyle" => {
            pen.line((2.0, 9.0), (12.0, 9.0));
            pen.line((2.0, 6.0), (2.0, 12.0));
            pen.line((12.0, 6.0), (12.0, 12.0));
            pen.line((9.0, 16.0), (19.0, 16.0));
            pen.line((9.0, 13.0), (9.0, 19.0));
            pen.line((19.0, 13.0), (19.0, 19.0));
        }
        // A capital T over a line of text.
        "text" => {
            pen.line((5.0, 4.0), (17.0, 4.0));
            pen.line((11.0, 4.0), (11.0, 14.0));
            pen.line((4.0, 18.0), (18.0, 18.0));
        }
        // A frame with a view of the model inside.
        "detail" => {
            pen.rect(3.0, 4.0, 16.0, 14.0);
            pen.path(
                &[(6.0, 15.0), (10.0, 8.0), (13.0, 12.0), (16.0, 7.0)],
                false,
            );
        }
        // A box in outline with its hidden edges dashed.
        "make2d" => {
            pen.path(&[(3.0, 8.0), (14.0, 8.0), (14.0, 19.0), (3.0, 19.0)], true);
            pen.path(&[(3.0, 8.0), (8.0, 3.0), (19.0, 3.0), (14.0, 8.0)], false);
            pen.path(&[(19.0, 3.0), (19.0, 14.0), (14.0, 19.0)], false);
            pen.line((8.0, 3.0), (8.0, 6.0));
            pen.line((8.0, 9.0), (8.0, 12.0));
            pen.line((8.0, 14.0), (11.0, 14.0));
        }
        // A sheet's border with the title block in its lower right corner.
        "titleblock" => {
            pen.rect(2.0, 3.0, 18.0, 16.0);
            pen.rect(10.0, 13.0, 10.0, 6.0);
            pen.line_in((10.0, 16.0), (20.0, 16.0), pen.accent());
        }
        // A bar of alternate filled and empty blocks over a ruler's ticks.
        "scalebar" => {
            pen.rect(2.0, 9.0, 18.0, 4.0);
            pen.filled(&[(2.0, 9.0), (6.5, 9.0), (6.5, 13.0), (2.0, 13.0)], pen.ink);
            pen.filled(
                &[(11.0, 9.0), (15.5, 9.0), (15.5, 13.0), (11.0, 13.0)],
                pen.ink,
            );
            for x in [2.0, 11.0, 20.0] {
                pen.line((x, 5.0), (x, 8.0));
            }
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn curves_5(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    match key {
        // A table with numbered rows.
        "sheetindex" => {
            pen.rect(3.0, 3.0, 16.0, 16.0);
            pen.line((3.0, 7.0), (19.0, 7.0));
            pen.line((8.0, 7.0), (8.0, 19.0));
            for y in [11.0, 15.0] {
                pen.line_in((3.0, y), (19.0, y), pen.accent());
            }
        }
        // A revision triangle with a row of the table under it.
        "revision" => {
            pen.path(&[(11.0, 3.0), (18.0, 14.0), (4.0, 14.0)], true);
            pen.line_in((3.0, 18.0), (19.0, 18.0), pen.accent());
        }
        // A printer with a sheet coming out.
        "print" => {
            pen.rect(6.0, 2.0, 10.0, 6.0);
            pen.rect(2.0, 8.0, 18.0, 8.0);
            pen.rect(6.0, 13.0, 10.0, 7.0);
            pen.line((8.0, 16.5), (14.0, 16.5));
        }
        // A sheet with its corner turned, and an arrow out of it.
        "exportpdf" => {
            pen.path(
                &[
                    (4.0, 2.0),
                    (13.0, 2.0),
                    (17.0, 6.0),
                    (17.0, 20.0),
                    (4.0, 20.0),
                ],
                true,
            );
            pen.path(&[(13.0, 2.0), (13.0, 6.0), (17.0, 6.0)], false);
            pen.line((8.0, 13.0), (21.0, 13.0));
            pen.path(&[(18.0, 10.0), (21.0, 13.0), (18.0, 16.0)], false);
        }
        // A list of sheets.
        "layouts" => {
            pen.rect(3.0, 3.0, 7.0, 5.0);
            pen.rect(3.0, 10.0, 7.0, 5.0);
            pen.line((12.0, 5.5), (19.0, 5.5));
            pen.line((12.0, 12.5), (19.0, 12.5));
            pen.line((12.0, 18.5), (19.0, 18.5));
        }
        // Two sheets, one over the other.
        "copylayout" => {
            pen.rect(6.0, 8.0, 14.0, 10.0);
            pen.path(
                &[
                    (4.0, 14.0),
                    (2.0, 14.0),
                    (2.0, 4.0),
                    (16.0, 4.0),
                    (16.0, 6.0),
                ],
                false,
            );
        }
        "point" => {
            pen.line((11.0, 5.0), (11.0, 17.0));
            pen.line((5.0, 11.0), (17.0, 11.0));
            if let Some(painter) = pen.painter {
                painter.circle_filled(pen.at((11.0, 11.0)), 2.4 * pen.scale(), theme::ACCENT);
            }
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn curves_6(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    let red = Stroke::new(1.7 * pen.scale(), RED);
    match key {
        "divide" => {
            pen.line((3.0, 15.0), (19.0, 7.0));
            for i in 0..5 {
                let t = i as f32 / 4.0;
                pen.dot(3.0 + 16.0 * t, 15.0 - 8.0 * t);
            }
        }
        "rebuild" => {
            pen.cubic((3.0, 17.0), (7.0, 3.0), (15.0, 3.0), (19.0, 17.0));
            pen.dashed(&[(3.0, 17.0), (11.0, 3.0), (19.0, 17.0)], 1.0);
            for (x, y) in [(3.0, 17.0), (11.0, 3.0), (19.0, 17.0)] {
                pen.dot(x, y);
            }
        }
        "insertknot" | "removeknot" => {
            pen.cubic((3.0, 15.0), (8.0, 5.0), (14.0, 5.0), (19.0, 15.0));
            let stroke = if name.eq_ignore_ascii_case("insertknot") {
                pen.accent()
            } else {
                red
            };
            pen.line_in((8.0, 18.5), (14.0, 18.5), stroke);
            if name.eq_ignore_ascii_case("insertknot") {
                pen.line_in((11.0, 15.5), (11.0, 21.0), stroke);
            }
            pen.dot(11.0, 7.6);
        }
        "removecontrolpoint" => {
            pen.dashed(&[(3.0, 17.0), (8.0, 5.0), (14.0, 5.0), (19.0, 17.0)], 1.0);
            for (x, y) in [(3.0, 17.0), (8.0, 5.0), (19.0, 17.0)] {
                pen.dot(x, y);
            }
            pen.line_in((12.0, 3.0), (16.0, 7.0), red);
            pen.line_in((16.0, 3.0), (12.0, 7.0), red);
        }
        "pointson" | "pointsoff" => {
            pen.cubic((3.0, 17.0), (8.0, 8.0), (14.0, 8.0), (19.0, 15.0));
            pen.dashed(&[(3.0, 17.0), (8.0, 6.0), (15.0, 6.0), (19.0, 15.0)], 1.0);
            for (x, y) in [(3.0, 17.0), (8.0, 6.0), (15.0, 6.0), (19.0, 15.0)] {
                if name.eq_ignore_ascii_case("pointson") {
                    pen.dot(x, y);
                } else {
                    pen.ring(x, y);
                }
            }
        }
        "setpt" => {
            pen.dashed(&[(2.0, 12.0), (20.0, 12.0)], 1.0);
            for (x, from) in [(5.0, 5.0), (11.0, 18.0), (17.0, 7.0)] {
                pen.dashed(&[(x, from), (x, 12.0)], 0.9);
                pen.dot(x, 12.0);
            }
        }
        "smooth" => {
            pen.path(
                &[
                    (3.0, 14.0),
                    (6.0, 8.0),
                    (8.0, 15.0),
                    (11.0, 7.0),
                    (13.0, 14.0),
                ],
                false,
            );
            pen.quad_in((13.0, 14.0), (16.0, 6.0), (19.0, 11.0), pen.accent());
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn curves_7(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    let thin = |w: f32| pen.width(w);
    match key {
        "flip" => {
            pen.arrow((4.0, 8.0), (18.0, 8.0));
            pen.line((18.0, 15.0), (4.0, 15.0));
            pen.arrow_head((4.0, 15.0), (18.0, 15.0), pen.accent());
        }
        "dir" => {
            pen.cubic((3.0, 17.0), (8.0, 3.0), (14.0, 19.0), (19.0, 5.0));
            pen.arrow_head((19.0, 5.0), (17.0, 9.0), pen.accent());
            pen.arrow_head((11.0, 11.0), (9.0, 13.0), pen.accent());
        }
        "closecrv" => {
            pen.arc((3.0, 3.0, 16.0, 16.0), 60.0, 290.0);
            pen.line_in((15.0, 4.1), (18.3, 9.0), pen.accent());
        }
        "offset" => {
            pen.quad((4.0, 17.0), (11.0, 9.0), (18.0, 12.0));
            pen.dashed_in(
                &[
                    (4.0, 12.0),
                    (8.0, 7.5),
                    (12.0, 6.0),
                    (15.0, 6.5),
                    (18.0, 7.0),
                ],
                thin(1.3),
            );
        }
        "fillet" => {
            pen.line((4.0, 18.0), (4.0, 9.0));
            pen.line((9.0, 4.0), (18.0, 4.0));
            pen.quad_in((4.0, 9.0), (4.0, 4.0), (9.0, 4.0), pen.accent());
        }
        "filletcorners" => {
            pen.path(&[(3.0, 17.0), (3.0, 9.0)], false);
            pen.path(&[(8.0, 4.0), (13.0, 4.0)], false);
            pen.path(&[(18.0, 9.0), (18.0, 17.0)], false);
            pen.quad_in((3.0, 9.0), (3.0, 4.0), (8.0, 4.0), pen.accent());
            pen.quad_in((13.0, 4.0), (18.0, 4.0), (18.0, 9.0), pen.accent());
        }
        "chamfer" => {
            pen.line((4.0, 18.0), (4.0, 9.0));
            pen.line((9.0, 4.0), (18.0, 4.0));
            pen.line_in((4.0, 9.0), (9.0, 4.0), pen.accent());
        }
        "tweencurves" => {
            pen.quad((3.0, 5.0), (11.0, 1.0), (19.0, 5.0));
            pen.quad((3.0, 18.0), (11.0, 14.0), (19.0, 18.0));
            pen.quad_in((3.0, 11.5), (11.0, 7.5), (19.0, 11.5), pen.accent());
        }
        "projecttocplane" => {
            pen.cubic((4.0, 6.0), (8.0, 2.0), (14.0, 9.0), (18.0, 4.0));
            pen.path(
                &[(2.0, 18.0), (6.0, 14.0), (20.0, 14.0), (16.0, 18.0)],
                true,
            );
            for x in [6.0, 12.0] {
                pen.dashed(&[(x, 6.0), (x, 15.5)], 0.9);
                pen.arrow_head((x, 15.5), (x, 10.0), pen.accent());
            }
        }
        "join" => {
            for (dx, dy) in SPOKES {
                pen.line((11.0 + 7.0 * dx, 11.0 + 7.0 * dy), (11.0, 11.0));
            }
            pen.gold_x(2.0, 3.4);
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn curves_8(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    let red = Stroke::new(1.7 * pen.scale(), RED);
    match key {
        "explode" => {
            for (dx, dy) in SPOKES {
                pen.line(
                    (11.0 + 4.2 * dx, 11.0 + 4.2 * dy),
                    (11.0 + 8.0 * dx, 11.0 + 8.0 * dy),
                );
            }
            pen.gold_x(1.6, 2.2);
        }
        "trim" => {
            pen.quad((3.0, 16.0), (11.0, 12.0), (19.0, 14.0));
            pen.line((13.0, 4.0), (9.0, 19.0));
            pen.line_in((15.0, 8.0), (19.0, 7.0), red);
        }
        "split" => {
            pen.line((3.0, 15.0), (9.0, 13.0));
            pen.line((13.0, 12.0), (19.0, 10.0));
            pen.line_in(
                (11.0, 4.0),
                (11.0, 19.0),
                Stroke::new(1.5 * pen.scale(), theme::ACCENT),
            );
        }
        "polygon" => {
            let points: Vec<(f32, f32)> = (0..6)
                .map(|i| {
                    let a = (i as f32 * 60.0 + 30.0).to_radians();
                    (11.0 + 8.0 * a.cos(), 11.0 - 8.0 * a.sin())
                })
                .collect();
            pen.path(&points, true);
            pen.dot(11.0, 11.0);
        }
        "helix" => {
            for i in 0..3 {
                let y = 4.0 + i as f32 * 5.0;
                pen.cubic((4.0, y), (4.0, y + 3.0), (18.0, y + 2.0), (18.0, y + 5.0));
            }
            pen.dashed(&[(11.0, 2.0), (11.0, 20.0)], 0.9);
        }
        "spiral" => {
            let points: Vec<(f32, f32)> = (0..=48)
                .map(|i| {
                    let t = i as f32 / 48.0;
                    let a = t * 3.0 * std::f32::consts::TAU;
                    let r = 1.0 + 8.0 * t;
                    (11.0 + r * a.cos(), 11.0 - r * a.sin())
                })
                .collect();
            pen.path(&points, false);
        }
        "points" => {
            for (x, y) in [(5.0, 15.0), (9.0, 7.0), (14.0, 13.0), (17.0, 5.0)] {
                pen.line((x - 2.0, y), (x + 2.0, y));
                pen.line((x, y - 2.0), (x, y + 2.0));
                pen.dot(x, y);
            }
        }
        "lines" => {
            for (a, b) in [((3.0, 17.0), (10.0, 5.0)), ((9.0, 18.0), (19.0, 9.0))] {
                pen.line(a, b);
                pen.dot(a.0, a.1);
                pen.dot(b.0, b.1);
            }
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn curves_9(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    let thin = |w: f32| pen.width(w);
    match key {
        "slot" => {
            pen.line((7.0, 7.0), (15.0, 7.0));
            pen.line((7.0, 15.0), (15.0, 15.0));
            pen.arc((11.0, 7.0, 8.0, 8.0), -90.0, 180.0);
            pen.arc((3.0, 7.0, 8.0, 8.0), 90.0, 180.0);
            pen.dot(7.0, 11.0);
            pen.dot(15.0, 11.0);
        }
        "dupedge" => {
            pen.cube(4.0, 6.0, 9.0);
            pen.line_in((15.5, 4.0), (15.5, 13.0), pen.accent());
            pen.line_in((19.0, 2.0), (19.0, 11.0), pen.accent());
        }
        "parabola" => {
            pen.quad((3.0, 3.0), (11.0, 30.0), (19.0, 3.0));
            pen.dot(11.0, 16.5);
            pen.dot(11.0, 11.0);
        }
        "changedegree" => {
            pen.cubic((3.0, 17.0), (7.0, 3.0), (15.0, 3.0), (19.0, 17.0));
            pen.line_in((14.0, 15.0), (18.0, 15.0), pen.accent());
            pen.line_in((14.0, 18.5), (18.0, 18.5), pen.accent());
        }
        "handlecurve" => {
            pen.cubic((3.0, 17.0), (6.0, 5.0), (14.0, 5.0), (19.0, 15.0));
            pen.line_in((2.0, 9.0), (10.0, 5.0), thin(1.2));
            pen.dot(6.0, 7.0);
            pen.ring(2.0, 9.0);
            pen.ring(10.0, 5.0);
        }
        "sketch" => {
            pen.path(
                &[
                    (3.0, 15.0),
                    (5.0, 11.0),
                    (7.0, 13.0),
                    (9.0, 8.0),
                    (12.0, 11.0),
                    (14.0, 6.0),
                    (17.0, 9.0),
                ],
                false,
            );
            pen.line_in((15.0, 15.0), (19.0, 11.0), pen.accent());
            pen.line_in((19.0, 11.0), (20.0, 12.0), pen.accent());
        }
        "conic" => {
            pen.dashed_in(&[(3.0, 18.0), (11.0, 3.0), (19.0, 18.0)], thin(1.2));
            pen.quad((3.0, 18.0), (11.0, 3.0), (19.0, 18.0));
            pen.dot(11.0, 3.0);
        }
        "hyperbola" => {
            pen.quad((18.0, 3.0), (6.0, 11.0), (18.0, 19.0));
            pen.line_in((4.0, 11.0), (20.0, 11.0), thin(1.0));
            pen.dot(9.0, 11.0);
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn curves_10(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    let fill = pen.ink.gamma_multiply(0.27);
    let thin = |w: f32| pen.width(w);
    match key {
        "blendcrv" => {
            pen.line((2.0, 16.0), (8.0, 16.0));
            pen.line((14.0, 6.0), (20.0, 6.0));
            pen.cubic_in(
                (8.0, 16.0),
                (12.0, 16.0),
                (10.0, 6.0),
                (14.0, 6.0),
                pen.accent(),
            );
        }
        "offsetmultiple" => {
            pen.quad((3.0, 18.0), (11.0, 10.0), (19.0, 14.0));
            pen.line((3.0, 8.0), (10.0, 4.0));
            pen.dashed_in(&[(3.0, 13.0), (11.0, 5.0), (19.0, 9.0)], thin(1.2));
            pen.dashed_in(&[(5.0, 11.0), (12.0, 7.0)], thin(1.2));
        }
        "dupborder" => {
            let outline = [(5.0, 5.0), (17.0, 4.0), (18.0, 17.0), (4.0, 18.0)];
            pen.filled(&outline, fill);
            pen.path_in(&outline, true, pen.accent());
        }
        "extend" => {
            pen.line((3.0, 15.0), (11.0, 11.0));
            pen.dashed_in(&[(11.0, 11.0), (17.0, 8.0)], thin(1.4));
            pen.line_in((18.0, 3.0), (18.0, 19.0), pen.accent());
        }
        "fair" => {
            pen.path_in(
                &[
                    (3.0, 13.0),
                    (6.0, 9.0),
                    (8.0, 14.0),
                    (11.0, 8.0),
                    (14.0, 14.0),
                    (19.0, 10.0),
                ],
                false,
                thin(1.0),
            );
            pen.quad_in((3.0, 13.0), (11.0, 7.0), (19.0, 10.0), pen.accent());
        }
        "curveboolean" => {
            pen.rect(3.0, 3.0, 10.0, 10.0);
            pen.rect(9.0, 9.0, 10.0, 10.0);
            pen.filled(
                &[(9.0, 9.0), (13.0, 9.0), (13.0, 13.0), (9.0, 13.0)],
                pen.accent().color,
            );
        }
        "match" => {
            pen.cubic((2.0, 17.0), (6.0, 6.0), (10.0, 12.0), (12.0, 9.0));
            pen.line_in((12.0, 9.0), (20.0, 9.0), pen.accent());
            pen.dot(12.0, 9.0);
        }
        "simplifycrv" => {
            pen.line((3.0, 17.0), (9.0, 17.0));
            pen.arc((7.0, 7.0, 12.0, 20.0), 180.0, 90.0);
            pen.dot(9.0, 17.0);
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (`name` in lower case) if it is one of these; returns whether it was.
pub(super) fn curves_11(pen: &Pen<'_>, name: &str, key: &str) -> bool {
    let thin = |w: f32| pen.width(w);
    match key {
        "reparameterize" => {
            pen.line((3.0, 14.0), (19.0, 14.0));
            for x in [3.0, 8.0, 14.0, 19.0] {
                pen.line_in((x, 11.0), (x, 17.0), thin(1.2));
            }
            pen.line_in((3.0, 6.0), (19.0, 6.0), pen.accent());
        }
        "makeuniform" => {
            pen.quad((3.0, 15.0), (11.0, 3.0), (19.0, 15.0));
            for x in [3.0, 7.0, 11.0, 15.0, 19.0] {
                pen.line_in((x, 18.0), (x, 20.0), pen.accent());
            }
        }
        "weight" => {
            pen.dashed_in(&[(3.0, 18.0), (11.0, 3.0), (19.0, 18.0)], thin(1.0));
            pen.quad((3.0, 18.0), (11.0, 3.0), (19.0, 18.0));
            pen.filled(
                &[(9.0, 1.5), (13.0, 1.5), (14.0, 6.0), (8.0, 6.0)],
                pen.accent().color,
            );
        }
        "section" | "contour" => {
            pen.cube(4.0, 6.0, 10.0);
            if name.eq_ignore_ascii_case("section") {
                pen.line_in((11.0, 2.0), (11.0, 20.0), pen.accent());
            } else {
                for y in [8.0, 12.0, 16.0] {
                    pen.line_in((2.0, y), (20.0, y), pen.accent());
                }
            }
        }
        "intersect" => {
            pen.ellipse((2.0, 5.0, 12.0, 12.0));
            pen.ellipse((8.0, 5.0, 12.0, 12.0));
            pen.line_in((11.0, 6.0), (11.0, 16.0), pen.accent());
        }
        "pull" | "project" => {
            pen.line((4.0, 4.0), (18.0, 4.0));
            pen.arrow((11.0, 6.0), (11.0, 13.0));
            pen.quad((2.0, 18.0), (11.0, 13.0), (20.0, 18.0));
        }
        "interpcrvonsrf" | "offsetcrvonsrf" => {
            pen.path(
                &[(2.0, 15.0), (16.0, 15.0), (20.0, 20.0), (6.0, 20.0)],
                true,
            );
            pen.quad_in((5.0, 17.5), (11.0, 14.0), (17.0, 18.5), pen.accent());
            if name.eq_ignore_ascii_case("offsetcrvonsrf") {
                pen.quad_in((6.0, 19.0), (12.0, 15.5), (18.0, 19.5), thin(1.0));
            }
            pen.dot(11.0, 6.0);
        }
        "silhouette" => {
            pen.ellipse((4.0, 4.0, 14.0, 14.0));
            pen.arc_in((4.0, 4.0, 14.0, 14.0), 0.0, 360.0, pen.accent());
            pen.ellipse((8.0, 8.0, 6.0, 3.0));
        }
        "dupfaceborder" => {
            pen.cube(4.0, 6.0, 11.0);
            pen.path_in(
                &[(4.0, 6.0), (10.0, 2.0), (21.0, 2.0), (15.0, 6.0)],
                true,
                pen.accent(),
            );
        }
        _ => return false,
    }
    true
}
