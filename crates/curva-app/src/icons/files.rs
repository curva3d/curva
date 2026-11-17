//! Icons: files.

use super::*;

/// Draws the icon `key` (a command name in lower case) if it is one of these; returns whether it was.
pub(super) fn files(pen: &Pen<'_>, key: &str) -> bool {
    match key {
        "new" => {
            pen.path(
                &[
                    (5.0, 2.0),
                    (13.0, 2.0),
                    (17.0, 6.0),
                    (17.0, 20.0),
                    (5.0, 20.0),
                ],
                true,
            );
            pen.line_in((11.0, 9.0), (11.0, 15.0), pen.accent());
            pen.line_in((8.0, 12.0), (14.0, 12.0), pen.accent());
        }
        "open" => {
            pen.path(
                &[
                    (2.0, 6.0),
                    (8.0, 6.0),
                    (10.0, 8.0),
                    (19.0, 8.0),
                    (19.0, 18.0),
                    (2.0, 18.0),
                ],
                true,
            );
            pen.path_in(
                &[(4.0, 18.0), (7.0, 11.0), (21.0, 11.0), (19.0, 18.0)],
                false,
                pen.accent(),
            );
        }
        "save" => pen.disk(),
        "saveas" => {
            pen.disk();
            pen.line_in((17.0, 14.0), (17.0, 21.0), pen.accent());
            pen.line_in((13.5, 17.5), (20.5, 17.5), pen.accent());
        }
        _ => return false,
    }
    true
}
