//! Icons: mesh and SubD commands (the mesh stream's own file).

use super::*;

/// Draws the icon `key` (a command name in lower case) if it is one of these; returns whether it was.
pub(super) fn mesh_1(pen: &Pen<'_>, key: &str) -> bool {
    match key {
        "mesh" => {
            // A sphere's outline, faceted: a ring of flat sides with lines across to the middle.
            let points: Vec<(f32, f32)> = (0..8)
                .map(|k| {
                    let a = std::f32::consts::TAU * k as f32 / 8.0;
                    (11.0 + 8.0 * a.cos(), 11.0 + 8.0 * a.sin())
                })
                .collect();
            for k in 0..8 {
                pen.line(points[k], points[(k + 1) % 8]);
            }
            for k in [0, 2, 4, 6] {
                pen.line(points[k], (11.0, 11.0));
            }
            pen.dot(11.0, 11.0);
        }
        "meshplane" => grid(pen, (3.0, 5.0), (19.0, 17.0), 4),
        "meshbox" => {
            // A box's front face as a grid, its top and side drawn back.
            grid(pen, (3.0, 8.0), (14.0, 19.0), 3);
            pen.line((3.0, 8.0), (8.0, 3.0));
            pen.line((14.0, 8.0), (19.0, 3.0));
            pen.line((8.0, 3.0), (19.0, 3.0));
            pen.line((14.0, 19.0), (19.0, 14.0));
            pen.line((19.0, 3.0), (19.0, 14.0));
        }
        "meshsphere" | "meshtorus" => {
            if key == "meshsphere" {
                pen.ellipse((3.0, 3.0, 16.0, 16.0));
                pen.ellipse((7.0, 3.0, 8.0, 16.0));
                pen.ellipse((3.0, 8.0, 16.0, 6.0));
            } else {
                pen.ellipse((2.0, 5.0, 18.0, 12.0));
                pen.ellipse((7.0, 9.0, 8.0, 4.0));
                pen.line((2.0, 11.0), (7.0, 11.0));
                pen.line((15.0, 11.0), (20.0, 11.0));
                pen.line((11.0, 5.0), (11.0, 9.0));
                pen.line((11.0, 13.0), (11.0, 17.0));
            }
        }
        "meshcylinder" | "meshcone" => {
            pen.ellipse((4.0, 15.0, 14.0, 5.0));
            let top = if key == "meshcylinder" {
                pen.ellipse((4.0, 2.0, 14.0, 5.0));
                [(4.0, 4.5), (11.0, 2.0), (18.0, 4.5), (11.0, 7.0)]
            } else {
                [(11.0, 3.0); 4]
            };
            for (k, bottom) in [(4.0, 17.5), (11.0, 15.0), (18.0, 17.5), (11.0, 20.0)]
                .into_iter()
                .enumerate()
            {
                pen.line(top[k], bottom);
            }
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` if it is one of the mesh checks and repairs; returns whether it was.
pub(super) fn mesh_2(pen: &Pen<'_>, key: &str) -> bool {
    match key {
        "check" => {
            grid(pen, (2.0, 2.0), (14.0, 14.0), 3);
            pen.path_in(
                &[(10.0, 15.0), (13.5, 19.0), (20.0, 10.0)],
                false,
                pen.accent(),
            );
        }
        "meshrepair" | "culldegeneratemeshfaces" => {
            grid(pen, (2.0, 2.0), (14.0, 14.0), 3);
            pen.line_in((12.0, 20.0), (20.0, 12.0), pen.accent());
            if key == "meshrepair" {
                pen.line_in((14.0, 12.0), (20.0, 18.0), pen.accent());
            }
        }
        "unifymeshnormals" | "rebuildmeshnormals" => {
            pen.line((2.0, 16.0), (20.0, 16.0));
            for x in [5.0, 11.0, 17.0] {
                pen.line_in((x, 16.0), (x, 6.0), pen.accent());
                pen.line((x - 2.0, 8.0), (x, 6.0));
                pen.line((x + 2.0, 8.0), (x, 6.0));
            }
            if key == "rebuildmeshnormals" {
                pen.dot(11.0, 19.0);
            }
        }
        "fillmeshholes" => {
            grid(pen, (2.0, 2.0), (20.0, 20.0), 3);
            pen.path_in(
                &[(8.0, 8.0), (14.0, 8.0), (14.0, 14.0), (8.0, 14.0)],
                true,
                pen.accent(),
            );
        }
        _ => return false,
    }
    true
}

/// Draws the icon `key` if it is one of the commands that change a mesh's faces; returns whether
/// it was.
pub(super) fn mesh_3(pen: &Pen<'_>, key: &str) -> bool {
    match key {
        "reducemesh" => {
            grid(pen, (2.0, 6.0), (10.0, 14.0), 4);
            pen.path_in(&[(11.0, 10.0), (13.0, 10.0)], false, pen.accent());
            grid(pen, (14.0, 6.0), (20.0, 14.0), 1);
        }
        "triangulatemesh" | "quadrangulatemesh" => {
            grid(pen, (2.0, 2.0), (20.0, 20.0), 2);
            if key == "triangulatemesh" {
                pen.line_in((2.0, 11.0), (11.0, 2.0), pen.accent());
                pen.line_in((11.0, 11.0), (20.0, 2.0), pen.accent());
                pen.line_in((2.0, 20.0), (11.0, 11.0), pen.accent());
                pen.line_in((11.0, 20.0), (20.0, 11.0), pen.accent());
            } else {
                pen.path_in(
                    &[(2.0, 2.0), (20.0, 2.0), (20.0, 20.0), (2.0, 20.0)],
                    true,
                    pen.accent(),
                );
            }
        }
        "extractmeshfaces" => {
            grid(pen, (2.0, 2.0), (14.0, 14.0), 3);
            pen.path_in(
                &[(14.0, 14.0), (20.0, 14.0), (20.0, 20.0), (14.0, 20.0)],
                true,
                pen.accent(),
            );
        }
        "meshtonurb" => {
            grid(pen, (2.0, 2.0), (10.0, 20.0), 2);
            pen.path_in(
                &[(12.0, 20.0), (14.0, 12.0), (17.0, 6.0), (20.0, 2.0)],
                false,
                pen.accent(),
            );
        }
        key if key.starts_with("meshboolean") => mesh_boolean(pen, key),
        key if key.starts_with("subd") || key == "tosubd" => return subd_primitive(pen, key),
        "quadremesh" => {
            pen.arc((2.0, 2.0, 18.0, 18.0), 0.0, 360.0);
            grid(pen, (5.0, 5.0), (17.0, 17.0), 3);
            pen.path_in(
                &[(9.0, 9.0), (13.0, 9.0), (13.0, 13.0), (9.0, 13.0)],
                true,
                pen.accent(),
            );
        }
        _ => return false,
    }
    true
}

/// The SubD primitives: their smooth outline, with dots for the cage's corner points.
fn subd_primitive(pen: &Pen<'_>, key: &str) -> bool {
    let cage: &[(f32, f32)] = match key {
        "subdbox" => {
            pen.ellipse((3.0, 6.0, 13.0, 13.0));
            pen.arc((6.0, 3.0, 13.0, 13.0), -90.0, 180.0);
            &[
                (3.0, 6.0),
                (16.0, 6.0),
                (16.0, 19.0),
                (3.0, 19.0),
                (19.0, 3.0),
            ]
        }
        "subdplane" => {
            pen.ellipse((3.0, 5.0, 16.0, 12.0));
            &[(3.0, 5.0), (19.0, 5.0), (19.0, 17.0), (3.0, 17.0)]
        }
        "subdsphere" => {
            pen.ellipse((3.0, 3.0, 16.0, 16.0));
            &[(3.0, 3.0), (19.0, 3.0), (19.0, 19.0), (3.0, 19.0)]
        }
        "subdcylinder" | "subdcone" => {
            pen.ellipse((4.0, 15.0, 14.0, 5.0));
            if key == "subdcylinder" {
                pen.ellipse((4.0, 2.0, 14.0, 5.0));
                pen.line((4.0, 4.5), (4.0, 17.5));
                pen.line((18.0, 4.5), (18.0, 17.5));
            } else {
                pen.line((11.0, 3.0), (4.0, 17.5));
                pen.line((11.0, 3.0), (18.0, 17.5));
            }
            &[(3.0, 2.0), (19.0, 2.0), (3.0, 20.0), (19.0, 20.0)]
        }
        "subdtorus" => {
            pen.ellipse((2.0, 5.0, 18.0, 12.0));
            pen.ellipse((7.0, 9.0, 8.0, 4.0));
            &[(2.0, 5.0), (20.0, 5.0), (20.0, 17.0), (2.0, 17.0)]
        }
        "subddisplaytoggle" => {
            grid(pen, (2.0, 6.0), (10.0, 14.0), 1);
            pen.path_in(&[(11.0, 10.0), (13.0, 10.0)], false, pen.accent());
            pen.ellipse((14.0, 6.0, 7.0, 8.0));
            &[]
        }
        "tosubd" => {
            grid(pen, (2.0, 6.0), (10.0, 14.0), 2);
            pen.path_in(&[(11.0, 10.0), (13.0, 10.0)], false, pen.accent());
            pen.ellipse((14.0, 6.0, 7.0, 8.0));
            &[(14.0, 6.0), (21.0, 6.0), (21.0, 14.0), (14.0, 14.0)]
        }
        _ => return false,
    };
    for &(x, y) in cage {
        pen.dot(x, y);
    }
    true
}

/// Two overlapping meshed squares, with what the boolean keeps outlined.
fn mesh_boolean(pen: &Pen<'_>, key: &str) {
    grid(pen, (2.0, 2.0), (14.0, 14.0), 2);
    grid(pen, (8.0, 8.0), (20.0, 20.0), 2);
    let overlap = [(8.0, 8.0), (14.0, 8.0), (14.0, 14.0), (8.0, 14.0)];
    let first = [
        (2.0, 2.0),
        (14.0, 2.0),
        (14.0, 8.0),
        (8.0, 8.0),
        (8.0, 14.0),
        (2.0, 14.0),
    ];
    match key {
        "meshbooleanunion" => pen.path_in(
            &[
                (2.0, 2.0),
                (14.0, 2.0),
                (14.0, 8.0),
                (20.0, 8.0),
                (20.0, 20.0),
                (8.0, 20.0),
                (8.0, 14.0),
                (2.0, 14.0),
            ],
            true,
            pen.accent(),
        ),
        "meshbooleandifference" => pen.path_in(&first, true, pen.accent()),
        "meshbooleanintersection" => pen.path_in(&overlap, true, pen.accent()),
        _ => {
            pen.path_in(&first, true, pen.accent());
            pen.path_in(&overlap, true, pen.accent());
        }
    }
}

/// A rectangle from `a` to `b` split into `n` × `n` faces.
fn grid(pen: &Pen<'_>, a: (f32, f32), b: (f32, f32), n: u32) {
    for k in 0..=n {
        let t = k as f32 / n as f32;
        let x = a.0 + (b.0 - a.0) * t;
        let y = a.1 + (b.1 - a.1) * t;
        pen.line((x, a.1), (x, b.1));
        pen.line((a.0, y), (b.0, y));
    }
}
