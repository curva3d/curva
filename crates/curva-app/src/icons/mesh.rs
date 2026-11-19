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
        _ => return false,
    }
    true
}
