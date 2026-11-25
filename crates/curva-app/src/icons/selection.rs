//! Icons: selection.

use super::*;

/// Draws the icon `key` (a command name in lower case) if it is one of these; returns whether it was.
pub(super) fn selection_1(pen: &Pen<'_>, key: &str) -> bool {
    match key {
        "selall" | "selnone" | "selcrv" | "selsrf" | "selsolid" | "selpt" | "selname"
        | "sellayer" | "seldup" | "sellast" | "selprev" | "invert" => {
            pen.pointer();
            match key {
                "selall" => pen.filled(
                    &[(11.0, 11.0), (20.0, 11.0), (20.0, 20.0), (11.0, 20.0)],
                    theme::ACCENT,
                ),
                "selnone" => pen.dashed(
                    &[
                        (11.0, 11.0),
                        (20.0, 11.0),
                        (20.0, 20.0),
                        (11.0, 20.0),
                        (11.0, 11.0),
                    ],
                    1.0,
                ),
                "selcrv" => pen.cubic_in(
                    (11.0, 20.0),
                    (13.0, 11.0),
                    (17.0, 20.0),
                    (20.0, 11.0),
                    pen.accent(),
                ),
                "selsrf" => pen.path_in(
                    &[(11.0, 19.0), (13.0, 12.0), (20.0, 12.0), (18.0, 19.0)],
                    true,
                    pen.accent(),
                ),
                "selsolid" => pen.cube(11.5, 11.0, 6.0),
                "selpt" => pen.dot(16.0, 16.0),
                "selname" => {
                    pen.path_in(
                        &[
                            (11.0, 13.0),
                            (17.0, 13.0),
                            (20.0, 16.0),
                            (17.0, 19.0),
                            (11.0, 19.0),
                        ],
                        true,
                        pen.accent(),
                    );
                }
                "sellayer" => {
                    for y in [12.0, 15.0, 18.0] {
                        pen.path_in(
                            &[(11.0, y), (15.5, y + 2.0), (20.0, y)],
                            false,
                            pen.accent(),
                        );
                    }
                }
                "seldup" => {
                    pen.rect(11.0, 11.0, 6.0, 6.0);
                    pen.path_in(
                        &[(13.0, 13.0), (19.0, 13.0), (19.0, 19.0), (13.0, 19.0)],
                        true,
                        pen.accent(),
                    );
                }
                "sellast" => {
                    pen.arc((11.0, 11.0, 9.0, 9.0), 90.0, 300.0);
                    pen.line_in((15.5, 15.5), (15.5, 12.5), pen.accent());
                }
                "selprev" => {
                    pen.arc_in((11.0, 11.0, 9.0, 9.0), 0.0, 220.0, pen.accent());
                    pen.arrow_head((11.6, 17.6), (11.2, 14.5), pen.accent());
                }
                _ => {
                    pen.rect(11.0, 11.0, 9.0, 9.0);
                    pen.filled(&[(11.0, 11.0), (20.0, 11.0), (11.0, 20.0)], theme::ACCENT);
                }
            }
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` (a command name in lower case) if it is one of these; returns whether it was.
pub(super) fn selection_2(pen: &Pen<'_>, key: &str) -> bool {
    match key {
        "selfilter" | "selfiltertoggle" => {
            let funnel = [
                (3.0, 4.0),
                (19.0, 4.0),
                (13.0, 11.0),
                (13.0, 18.0),
                (9.0, 16.0),
                (9.0, 11.0),
            ];
            pen.path(&funnel, true);
            if key == "selfiltertoggle" {
                pen.line_in((3.0, 19.0), (19.0, 3.0), pen.accent());
            }
        }
        "selline" | "selpolyline" | "selcircle" | "selarc" | "selclosedcrv" | "selopencrv" => {
            pen.pointer();
            let accent = pen.accent();
            match key {
                "selline" => pen.line_in((11.0, 20.0), (20.0, 11.0), accent),
                "selpolyline" => pen.path_in(
                    &[(11.0, 20.0), (13.0, 13.0), (17.0, 18.0), (20.0, 11.0)],
                    false,
                    accent,
                ),
                "selcircle" => pen.path_in(
                    &pen.arc_points((11.0, 11.0, 9.0, 9.0), 0.0, 360.0),
                    true,
                    accent,
                ),
                "selarc" => pen.path_in(
                    &pen.arc_points((11.0, 11.0, 9.0, 9.0), 0.0, 180.0),
                    false,
                    accent,
                ),
                "selclosedcrv" => pen.path_in(
                    &[(11.0, 18.0), (13.0, 11.0), (20.0, 13.0), (18.0, 20.0)],
                    true,
                    accent,
                ),
                _ => pen.cubic_in(
                    (11.0, 20.0),
                    (12.0, 11.0),
                    (19.0, 21.0),
                    (20.0, 11.0),
                    accent,
                ),
            }
        }
        "selcolor" => {
            pen.pointer();
            pen.filled(
                &[(13.0, 13.0), (19.0, 13.0), (19.0, 19.0), (13.0, 19.0)],
                pen.accent().color,
            );
        }
        _ => return false,
    }
    true
}

/// Surfaces by their faces: two faces joined, a closed one, an open one (#679).
pub(super) fn selection_3(pen: &Pen<'_>, key: &str) -> bool {
    if !matches!(key, "selpolysrf" | "selclosedsrf" | "selopensrf") {
        return false;
    }
    pen.pointer();
    let accent = pen.accent();
    match key {
        "selpolysrf" => {
            pen.path_in(
                &[(11.0, 19.0), (12.5, 12.0), (16.5, 12.0), (15.0, 19.0)],
                true,
                accent,
            );
            pen.path_in(
                &[(16.5, 12.0), (20.0, 14.0), (19.0, 20.0), (15.0, 19.0)],
                true,
                accent,
            );
        }
        "selclosedsrf" => pen.path_in(
            &pen.arc_points((11.0, 11.0, 9.0, 9.0), 0.0, 360.0),
            true,
            accent,
        ),
        _ => pen.path_in(
            &[(11.0, 19.0), (13.0, 12.0), (20.0, 12.0), (18.0, 19.0)],
            false,
            accent,
        ),
    }
    true
}
