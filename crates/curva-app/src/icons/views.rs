//! Icons: views.

use super::*;

/// Draws the icon `key` (a command name in lower case) if it is one of these; returns whether it was.
pub(super) fn views_1(pen: &Pen<'_>, key: &str) -> bool {
    match key {
        "zoom" => pen.magnifier(),
        "zoomextents" => {
            pen.magnifier();
            for (a, b, c) in [
                ((5.0, 7.0), (5.0, 5.0), (7.0, 5.0)),
                ((12.0, 5.0), (12.0, 5.0), (12.0, 7.0)),
            ] {
                pen.path_in(&[a, b, c], false, pen.accent());
            }
        }
        "zoomselected" => {
            pen.magnifier();
            if let Some(painter) = pen.painter {
                painter.circle_filled(pen.at((8.5, 8.5)), 2.4 * pen.scale(), theme::ACCENT);
            }
        }
        "4view" => {
            for (x, y) in [(3.0, 3.0), (12.0, 3.0), (3.0, 12.0), (12.0, 12.0)] {
                pen.rect(x, y, 7.0, 7.0);
            }
        }
        "top" | "front" | "right" => {
            pen.rect(3.0, 3.0, 16.0, 16.0);
            match key {
                "top" => pen.dot(11.0, 11.0),
                "front" => pen.line_in((3.0, 14.0), (19.0, 14.0), pen.accent()),
                _ => pen.line_in((14.0, 3.0), (14.0, 19.0), pen.accent()),
            }
        }
        "perspective" => {
            pen.path(&[(6.0, 6.0), (16.0, 6.0), (20.0, 18.0), (2.0, 18.0)], true);
            pen.line_in(
                (11.0, 6.0),
                (11.0, 18.0),
                Stroke::new(1.0 * pen.scale(), theme::ACCENT),
            );
            pen.line_in(
                (4.5, 12.0),
                (17.5, 12.0),
                Stroke::new(1.0 * pen.scale(), theme::ACCENT),
            );
        }
        "cplane" => {
            pen.path(&[(2.0, 15.0), (8.0, 9.0), (20.0, 9.0), (14.0, 15.0)], true);
            pen.line_in(
                (11.0, 12.0),
                (17.0, 12.0),
                Stroke::new(1.5 * pen.scale(), Color32::from_rgb(220, 90, 80)),
            );
            pen.line_in(
                (11.0, 12.0),
                (11.0, 3.0),
                Stroke::new(1.5 * pen.scale(), Color32::from_rgb(90, 130, 230)),
            );
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (a command name in lower case) if it is one of these; returns whether it was.
pub(super) fn views_2(pen: &Pen<'_>, key: &str) -> bool {
    let thin = |w: f32| pen.width(w);
    match key {
        // NamedCPlane: the construction plane, with the saved mark NamedView has.
        "namedcplane" => {
            pen.path(&[(2.0, 15.0), (8.0, 9.0), (20.0, 9.0), (14.0, 15.0)], true);
            pen.line_in(
                (11.0, 12.0),
                (17.0, 12.0),
                Stroke::new(1.5 * pen.scale(), Color32::from_rgb(220, 90, 80)),
            );
            pen.line_in(
                (11.0, 12.0),
                (11.0, 3.0),
                Stroke::new(1.5 * pen.scale(), Color32::from_rgb(90, 130, 230)),
            );
            pen.dot(17.0, 4.0);
        }
        // Import, Export, ExportSelected: a folder, a Rhino file going into it or out of it.
        "import" | "export" | "exportselected" => {
            pen.path(
                &[
                    (2.0, 8.0),
                    (7.0, 8.0),
                    (9.0, 10.0),
                    (17.0, 10.0),
                    (17.0, 20.0),
                    (2.0, 20.0),
                ],
                true,
            );
            if key == "import" {
                pen.line_in((14.0, 2.0), (14.0, 13.0), pen.accent());
                pen.arrow_head((14.0, 14.0), (14.0, 9.0), pen.accent());
            } else {
                pen.line_in((14.0, 14.0), (14.0, 3.0), pen.accent());
                pen.arrow_head((14.0, 2.0), (14.0, 7.0), pen.accent());
            }
            if key == "exportselected" {
                pen.dot(6.0, 15.0);
            }
        }
        "namedview" | "undoview" | "redoview" => {
            pen.camera();
            match key {
                "undoview" => pen.arrow_head((3.0, 4.0), (8.0, 4.0), pen.accent()),
                "redoview" => pen.arrow_head((19.0, 4.0), (14.0, 4.0), pen.accent()),
                _ => pen.dot(9.0, 12.5),
            }
        }
        "setdisplaymode" => {
            pen.rect(3.0, 3.0, 16.0, 16.0);
            pen.filled(
                &[(3.0, 3.0), (19.0, 3.0), (3.0, 19.0)],
                theme::ACCENT.gamma_multiply(0.6),
            );
        }
        // Two styles, one over the other, the front one half lit: the Style panel.
        "styles" => {
            pen.rect(3.0, 7.0, 12.0, 12.0);
            pen.rect(7.0, 3.0, 12.0, 12.0);
            pen.filled(
                &[(7.0, 3.0), (19.0, 3.0), (7.0, 15.0)],
                theme::ACCENT.gamma_multiply(0.6),
            );
        }
        "plan" => {
            pen.rect(4.0, 4.0, 14.0, 14.0);
            pen.line_in((4.0, 11.0), (18.0, 11.0), thin(1.0));
            pen.line_in((11.0, 4.0), (11.0, 18.0), thin(1.0));
            pen.arrow((11.0, 1.0), (11.0, 8.0));
        }
        _ => return false,
    }
    true
}
