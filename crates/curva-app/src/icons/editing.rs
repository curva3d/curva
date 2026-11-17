//! Icons: visibility and editing.

use super::*;

/// Draws the icon `key` (a command name in lower case) if it is one of these; returns whether it was.
pub(super) fn editing(pen: &Pen<'_>, key: &str) -> bool {
    match key {
        "hide" => pen.eye(false),
        "show" => pen.eye(true),
        "isolate" => {
            pen.eye(true);
            pen.dashed(
                &[
                    (2.0, 2.0),
                    (20.0, 2.0),
                    (20.0, 20.0),
                    (2.0, 20.0),
                    (2.0, 2.0),
                ],
                0.9,
            );
        }
        "rename" => {
            pen.rect(3.0, 7.0, 16.0, 8.0);
            pen.line_in((14.0, 5.0), (14.0, 17.0), pen.accent());
        }
        "history" => {
            pen.ellipse((4.0, 4.0, 14.0, 14.0));
            pen.line((11.0, 11.0), (11.0, 6.5));
            pen.line_in((11.0, 11.0), (14.5, 13.0), pen.accent());
        }
        "delete" => {
            let red = Stroke::new(2.0 * pen.scale(), RED);
            pen.line_in((6.0, 6.0), (16.0, 16.0), red);
            pen.line_in((16.0, 6.0), (6.0, 16.0), red);
        }
        "undo" | "redo" => {
            let back = key == "undo";
            let (start, tip) = if back { (0.0, 180.0) } else { (180.0, -180.0) };
            pen.arc((4.0, 6.0, 14.0, 12.0), start, tip);
            let end = if back { (4.0, 12.0) } else { (18.0, 12.0) };
            pen.arrow_head(end, (end.0, 8.0), pen.accent());
        }
        "boundingbox" => {
            pen.dashed(
                &[
                    (3.0, 3.0),
                    (19.0, 3.0),
                    (19.0, 19.0),
                    (3.0, 19.0),
                    (3.0, 3.0),
                ],
                1.0,
            );
            pen.ellipse((6.0, 7.0, 10.0, 8.0));
            for (x, y) in [(3.0, 3.0), (19.0, 3.0), (19.0, 19.0), (3.0, 19.0)] {
                pen.dot(x, y);
            }
        }
        _ => return false,
    }
    true
}
