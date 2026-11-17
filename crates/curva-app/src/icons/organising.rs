//! Icons: layers, groups, blocks and notes.

use super::*;

/// Draws the icon `key` (a command name in lower case) if it is one of these; returns whether it was.
pub(super) fn organising_1(pen: &Pen<'_>, key: &str) -> bool {
    let thin = |w: f32| pen.width(w);
    match key {
        "hideswap" => {
            pen.eye(true);
            pen.arrow_head((19.0, 3.0), (14.0, 3.0), pen.accent());
            pen.arrow_head((3.0, 19.0), (8.0, 19.0), pen.accent());
        }
        "lock" | "unlock" => {
            pen.rect(5.0, 10.0, 12.0, 9.0);
            if key == "lock" {
                pen.arc((7.0, 4.0, 8.0, 12.0), 180.0, 180.0);
            } else {
                pen.arc((11.0, 2.0, 8.0, 12.0), 180.0, 180.0);
            }
            pen.dot(11.0, 14.5);
        }
        "group" | "ungroup" => {
            pen.rect(4.0, 4.0, 6.0, 6.0);
            pen.ellipse((12.0, 12.0, 7.0, 7.0));
            let frame = [(2.0, 2.0), (20.0, 2.0), (20.0, 20.0), (2.0, 20.0)];
            if key == "group" {
                pen.path_in(&frame, true, pen.accent());
            } else {
                pen.dashed_in(
                    &[
                        (2.0, 2.0),
                        (20.0, 2.0),
                        (20.0, 20.0),
                        (2.0, 20.0),
                        (2.0, 2.0),
                    ],
                    pen.accent(),
                );
            }
        }
        "changelayer" | "matchproperties" => {
            pen.path(&[(2.0, 8.0), (11.0, 4.0), (20.0, 8.0), (11.0, 12.0)], true);
            pen.path_in(
                &[(2.0, 14.0), (11.0, 10.0), (20.0, 14.0), (11.0, 18.0)],
                true,
                pen.accent(),
            );
            if key == "matchproperties" {
                pen.dot(11.0, 8.0);
            }
        }
        "linetype" => {
            pen.dashed_in(&[(3.0, 7.0), (19.0, 7.0)], pen.accent());
            pen.line((3.0, 12.0), (19.0, 12.0));
            pen.dashed_in(&[(3.0, 17.0), (19.0, 17.0)], thin(1.0));
        }
        "objectcolour" => {
            pen.rect(4.0, 4.0, 14.0, 14.0);
            pen.filled(
                &[(7.0, 7.0), (15.0, 7.0), (15.0, 15.0), (7.0, 15.0)],
                theme::ACCENT,
            );
        }
        "printwidth" => {
            pen.line_in((3.0, 6.0), (19.0, 6.0), thin(1.0));
            pen.line((3.0, 11.0), (19.0, 11.0));
            pen.line_in((3.0, 17.0), (19.0, 17.0), thin(3.5));
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (a command name in lower case) if it is one of these; returns whether it was.
pub(super) fn organising_2(pen: &Pen<'_>, key: &str) -> bool {
    let fill = pen.ink.gamma_multiply(0.27);
    match key {
        "setusertext"
        | "getusertext"
        | "setdocumentusertext"
        | "getdocumentusertext"
        | "setlayoutusertext" => {
            pen.rect(4.0, 4.0, 14.0, 14.0);
            for y in [8.0, 11.0, 14.0] {
                pen.line_in((7.0, y), (15.0, y), pen.accent());
            }
        }
        // A notepad: a page with its binding along the top and lines of writing.
        "notes" => {
            pen.rect(4.0, 3.0, 14.0, 17.0);
            pen.line((4.0, 6.0), (18.0, 6.0));
            for y in [10.0, 13.0, 16.0] {
                pen.line_in((7.0, y), (15.0, y), pen.accent());
            }
        }
        "purge" => {
            pen.path(&[(6.0, 7.0), (16.0, 7.0), (15.0, 19.0), (7.0, 19.0)], true);
            pen.line((4.0, 6.0), (18.0, 6.0));
            pen.line((9.0, 4.0), (13.0, 4.0));
        }
        "audit" => {
            pen.magnifier();
        }
        "lockswap" | "unlockselected" => {
            pen.rect(5.0, 10.0, 12.0, 9.0);
            pen.arc((7.0, 4.0, 8.0, 12.0), 180.0, 180.0);
            pen.line_in((3.0, 3.0), (19.0, 19.0), pen.accent());
        }
        "bringtofront" | "sendtoback" | "bringforward" | "sendbackward" => {
            let front = key.starts_with("bring");
            let (back, top) = if front {
                ((4.0, 4.0), (8.0, 8.0))
            } else {
                ((8.0, 8.0), (4.0, 4.0))
            };
            pen.rect(back.0, back.1, 10.0, 10.0);
            let face = [
                (top.0, top.1),
                (top.0 + 10.0, top.1),
                (top.0 + 10.0, top.1 + 10.0),
                (top.0, top.1 + 10.0),
            ];
            pen.filled(&face, fill);
            pen.path_in(&face, true, pen.accent());
        }
        "textobject" => {
            pen.path(&[(4.0, 18.0), (9.0, 4.0), (14.0, 18.0)], false);
            pen.line((6.0, 13.0), (12.0, 13.0));
            pen.line_in((16.0, 10.0), (16.0, 18.0), pen.accent());
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (a command name in lower case) if it is one of these; returns whether it was.
pub(super) fn organising_3(pen: &Pen<'_>, key: &str) -> bool {
    match key {
        "block"
        | "insert"
        | "blockmanager"
        | "blockedit"
        | "explodeblock"
        | "replaceblock"
        | "selblockinstancenamed" => {
            pen.rect(3.0, 6.0, 9.0, 9.0);
            pen.rect(10.0, 9.0, 9.0, 9.0);
            match key {
                "insert" => pen.arrow((16.0, 2.0), (16.0, 8.0)),
                "blockedit" => pen.line_in((13.0, 13.0), (19.0, 7.0), pen.accent()),
                "explodeblock" => {
                    for (a, b) in [((11.0, 4.0), (13.0, 1.0)), ((20.0, 8.0), (21.0, 5.0))] {
                        pen.line_in(a, b, pen.accent());
                    }
                }
                "replaceblock" => pen.arrow((4.0, 20.0), (12.0, 20.0)),
                "selblockinstancenamed" => pen.dot(14.5, 13.5),
                "blockmanager" => {
                    for y in [3.0, 19.0] {
                        pen.line_in((3.0, y), (19.0, y), pen.accent());
                    }
                }
                _ => pen.line_in((3.0, 6.0), (12.0, 15.0), pen.accent()),
            }
        }
        _ => return false,
    }
    true
}
