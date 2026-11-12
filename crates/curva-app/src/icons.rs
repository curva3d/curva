//! Icons for the command buttons (spec `docs/specs/ui-look-and-layout.md` §5): Serpentine3D's
//! toolbar icons, which it draws in code rather than storing as pictures, redrawn here in Rust
//! with the same shapes (MIT; credited in `ATTRIBUTION.md`). They take the theme's text colour,
//! with the accent where Serpentine3D uses its own gold. Curva's own icon language comes later.

use crate::theme;
use eframe::egui::{self, Color32, Pos2, Stroke, epaint};

/// The box every icon is drawn in, in Serpentine3D's units.
const BOX: f32 = 22.0;
/// Serpentine3D's red, for what a command takes away.
const RED: Color32 = Color32::from_rgb(0xc9, 0x5f, 0x5f);

/// Draws one icon in a 22-unit box scaled to fit `rect`.
struct Pen<'a> {
    painter: &'a egui::Painter,
    rect: egui::Rect,
    ink: Color32,
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        let scale = self.rect.width().min(self.rect.height()) / BOX;
        let corner = self.rect.center() - egui::Vec2::splat(BOX * scale / 2.0);
        corner + egui::vec2(x * scale, y * scale)
    }

    fn scale(&self) -> f32 {
        self.rect.width().min(self.rect.height()) / BOX
    }

    fn stroke(&self) -> Stroke {
        Stroke::new(1.7 * self.scale(), self.ink)
    }

    fn line(&self, a: (f32, f32), b: (f32, f32)) {
        self.line_in(a, b, self.stroke());
    }

    fn line_in(&self, a: (f32, f32), b: (f32, f32), stroke: Stroke) {
        self.painter
            .line_segment([self.at(a.0, a.1), self.at(b.0, b.1)], stroke);
    }

    fn dashed(&self, points: &[(f32, f32)], width: f32) {
        let points: Vec<Pos2> = points.iter().map(|p| self.at(p.0, p.1)).collect();
        let scale = self.scale();
        self.painter.extend(egui::Shape::dashed_line(
            &points,
            Stroke::new(width * scale, self.ink),
            3.0 * scale,
            2.0 * scale,
        ));
    }

    fn path(&self, points: &[(f32, f32)], closed: bool) {
        let points: Vec<Pos2> = points.iter().map(|p| self.at(p.0, p.1)).collect();
        if closed {
            self.painter
                .add(egui::Shape::closed_line(points, self.stroke()));
        } else {
            self.painter.add(egui::Shape::line(points, self.stroke()));
        }
    }

    fn filled(&self, points: &[(f32, f32)], fill: Color32) {
        let points: Vec<Pos2> = points.iter().map(|p| self.at(p.0, p.1)).collect();
        self.painter
            .add(egui::Shape::convex_polygon(points, fill, Stroke::NONE));
    }

    fn cubic(
        &self,
        from: (f32, f32),
        c1: (f32, f32),
        c2: (f32, f32),
        to: (f32, f32),
        stroke: Stroke,
    ) {
        self.painter
            .add(epaint::CubicBezierShape::from_points_stroke(
                [
                    self.at(from.0, from.1),
                    self.at(c1.0, c1.1),
                    self.at(c2.0, c2.1),
                    self.at(to.0, to.1),
                ],
                false,
                Color32::TRANSPARENT,
                stroke,
            ));
    }

    fn quad(&self, from: (f32, f32), c: (f32, f32), to: (f32, f32), stroke: Stroke) {
        self.painter
            .add(epaint::QuadraticBezierShape::from_points_stroke(
                [
                    self.at(from.0, from.1),
                    self.at(c.0, c.1),
                    self.at(to.0, to.1),
                ],
                false,
                Color32::TRANSPARENT,
                stroke,
            ));
    }

    /// Points round the ellipse in the box `(x, y, w, h)`, from `start` through `span` degrees,
    /// counter-clockwise from three o'clock as Qt counts them.
    fn arc_points(
        &self,
        (x, y, w, h): (f32, f32, f32, f32),
        start: f32,
        span: f32,
    ) -> Vec<(f32, f32)> {
        let steps = 32;
        (0..=steps)
            .map(|i| {
                let angle = (start + span * i as f32 / steps as f32).to_radians();
                (
                    x + w / 2.0 * (1.0 + angle.cos()),
                    y + h / 2.0 * (1.0 - angle.sin()),
                )
            })
            .collect()
    }

    fn ellipse(&self, frame: (f32, f32, f32, f32)) {
        self.path(&self.arc_points(frame, 0.0, 360.0), true);
    }

    fn arc(&self, frame: (f32, f32, f32, f32), start: f32, span: f32) {
        self.path(&self.arc_points(frame, start, span), false);
    }

    fn dot(&self, x: f32, y: f32) {
        self.painter
            .circle_filled(self.at(x, y), 1.6 * self.scale(), theme::ACCENT);
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32) {
        self.path(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)], true);
    }

    /// The accent X that Join and Explode share.
    fn gold_x(&self, arm: f32, width: f32) {
        let stroke = Stroke::new(width * self.scale(), theme::ACCENT);
        self.line_in((11.0 - arm, 11.0 - arm), (11.0 + arm, 11.0 + arm), stroke);
        self.line_in((11.0 + arm, 11.0 - arm), (11.0 - arm, 11.0 + arm), stroke);
    }
}

/// The four directions Join and Explode's lines run in.
const SPOKES: [(f32, f32); 4] = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)];

/// Whether there is an icon for the command `name`.
#[must_use]
pub fn has(name: &str) -> bool {
    paint_into(None, name)
}

/// Draws the icon for the command `name` in `rect`, if there is one; returns whether there was.
pub fn paint(painter: &egui::Painter, rect: egui::Rect, name: &str) -> bool {
    paint_into(Some((painter, rect)), name)
}

#[allow(clippy::too_many_lines)] // One arm per icon, as in Serpentine3D.
fn paint_into(target: Option<(&egui::Painter, egui::Rect)>, name: &str) -> bool {
    let known = matches!(
        name.to_lowercase().as_str(),
        "line"
            | "polyline"
            | "curve"
            | "interpcrv"
            | "circle"
            | "arc"
            | "rectangle"
            | "extrudecrv"
            | "revolve"
            | "loft"
            | "planarsrf"
            | "sweep1"
            | "box"
            | "move"
            | "copy"
            | "rotate"
            | "scale"
            | "mirror"
            | "booleanunion"
            | "booleandifference"
            | "booleanintersection"
            | "join"
            | "delete"
            | "trim"
            | "split"
            | "offset"
            | "fillet"
            | "explode"
            | "pointson"
    );
    let Some((painter, rect)) = target else {
        return known;
    };
    if !known {
        return false;
    }
    let pen = Pen {
        painter,
        rect,
        ink: theme::current().text,
    };
    let fill = pen.ink.gamma_multiply(0.27);
    let thin = |width: f32| Stroke::new(width * pen.scale(), pen.ink);
    match name.to_lowercase().as_str() {
        "line" => {
            pen.line((4.0, 18.0), (18.0, 4.0));
            pen.dot(4.0, 18.0);
            pen.dot(18.0, 4.0);
        }
        "polyline" => pen.path(&[(3.0, 18.0), (8.0, 6.0), (13.0, 14.0), (19.0, 4.0)], false),
        "curve" | "interpcrv" => {
            pen.cubic(
                (3.0, 18.0),
                (10.0, 20.0),
                (8.0, 4.0),
                (19.0, 5.0),
                pen.stroke(),
            );
            pen.dot(3.0, 18.0);
            pen.dot(19.0, 5.0);
        }
        "circle" => pen.ellipse((4.0, 4.0, 14.0, 14.0)),
        "arc" => {
            pen.arc((3.0, 6.0, 16.0, 22.0), 30.0, 120.0);
            pen.dot(4.6, 14.5);
            pen.dot(17.4, 14.5);
        }
        "rectangle" => pen.rect(4.0, 6.0, 14.0, 10.0),
        "extrudecrv" => {
            pen.rect(5.0, 12.0, 12.0, 6.0);
            pen.line((11.0, 10.0), (11.0, 3.0));
            pen.line((8.0, 6.0), (11.0, 3.0));
            pen.line((14.0, 6.0), (11.0, 3.0));
        }
        "revolve" => {
            pen.line((11.0, 3.0), (11.0, 19.0));
            pen.ellipse((4.0, 12.0, 14.0, 5.0));
            pen.quad((15.0, 6.0), (18.0, 8.0), (16.0, 11.0), pen.stroke());
            pen.line((16.0, 11.0), (14.4, 9.4));
        }
        "loft" => {
            pen.quad((4.0, 6.0), (11.0, 3.0), (18.0, 6.0), pen.stroke());
            pen.quad((4.0, 16.0), (11.0, 13.0), (18.0, 16.0), pen.stroke());
            pen.line((4.0, 6.0), (4.0, 16.0));
            pen.line((18.0, 6.0), (18.0, 16.0));
        }
        "planarsrf" => {
            let outline = [(6.0, 5.0), (18.0, 5.0), (16.0, 17.0), (4.0, 17.0)];
            pen.filled(&outline, fill);
            pen.path(&outline, true);
        }
        "sweep1" => {
            pen.cubic(
                (4.0, 17.0),
                (8.0, 17.0),
                (12.0, 8.0),
                (18.0, 6.0),
                pen.stroke(),
            );
            pen.ellipse((2.0, 14.5, 5.0, 5.0));
        }
        "box" => {
            pen.rect(4.0, 8.0, 10.0, 10.0);
            pen.line((4.0, 8.0), (8.0, 4.0));
            pen.line((14.0, 8.0), (18.0, 4.0));
            pen.line((14.0, 18.0), (18.0, 14.0));
            pen.line((8.0, 4.0), (18.0, 4.0));
            pen.line((18.0, 4.0), (18.0, 14.0));
        }
        "move" => {
            pen.line((11.0, 4.0), (11.0, 18.0));
            pen.line((4.0, 11.0), (18.0, 11.0));
            for (a, b, c) in [
                ((11.0, 4.0), (9.0, 6.0), (13.0, 6.0)),
                ((11.0, 18.0), (9.0, 16.0), (13.0, 16.0)),
                ((4.0, 11.0), (6.0, 9.0), (6.0, 13.0)),
                ((18.0, 11.0), (16.0, 9.0), (16.0, 13.0)),
            ] {
                pen.line(a, b);
                pen.line(a, c);
            }
        }
        "copy" => {
            pen.rect(7.0, 7.0, 11.0, 11.0);
            pen.dashed(
                &[
                    (4.0, 4.0),
                    (15.0, 4.0),
                    (15.0, 15.0),
                    (4.0, 15.0),
                    (4.0, 4.0),
                ],
                1.4,
            );
        }
        "rotate" => {
            pen.arc((4.5, 4.5, 13.0, 13.0), 40.0, 250.0);
            pen.line((15.5, 3.5), (15.8, 7.4));
            pen.line((15.8, 7.4), (12.0, 6.4));
            pen.dot(11.0, 11.0);
        }
        "scale" => {
            pen.rect(4.0, 12.0, 6.0, 6.0);
            pen.rect(8.0, 4.0, 10.0, 10.0);
            pen.line((6.0, 16.0), (15.0, 7.0));
        }
        "mirror" => {
            pen.dashed(&[(11.0, 3.0), (11.0, 19.0)], 1.2);
            let left = [(8.0, 6.0), (8.0, 16.0), (3.0, 16.0)];
            pen.filled(&left, fill);
            pen.path(&left, true);
            pen.path(&[(14.0, 6.0), (14.0, 16.0), (19.0, 16.0)], true);
        }
        "booleanunion" => {
            pen.painter
                .circle(pen.at(8.5, 11.5), 5.5 * pen.scale(), fill, Stroke::NONE);
            pen.painter
                .circle(pen.at(13.5, 10.5), 5.5 * pen.scale(), fill, Stroke::NONE);
            // The outline of the two together: each circle's arc outside the other.
            pen.arc((3.0, 6.0, 11.0, 11.0), 70.0, 250.0);
            pen.arc((8.0, 5.0, 11.0, 11.0), -110.0, 250.0);
        }
        "booleandifference" => {
            pen.arc((3.0, 6.0, 11.0, 11.0), 70.0, 250.0);
            pen.arc((8.0, 5.0, 11.0, 11.0), 110.0, 140.0);
            let dotted = pen.arc_points((8.0, 5.0, 11.0, 11.0), 0.0, 360.0);
            pen.dashed(&dotted, 1.1);
        }
        "booleanintersection" => {
            for frame in [(3.0, 6.0, 11.0, 11.0), (8.0, 5.0, 11.0, 11.0)] {
                pen.dashed(&pen.arc_points(frame, 0.0, 360.0), 1.1);
            }
            let mut lens = pen.arc_points((3.0, 6.0, 11.0, 11.0), -70.0, 140.0);
            lens.extend(pen.arc_points((8.0, 5.0, 11.0, 11.0), 110.0, 140.0));
            pen.filled(&lens, theme::ACCENT.gamma_multiply(0.45));
            pen.path(&lens, true);
        }
        "join" => {
            for (dx, dy) in SPOKES {
                pen.line((11.0 + 7.0 * dx, 11.0 + 7.0 * dy), (11.0, 11.0));
            }
            pen.gold_x(2.0, 3.4);
        }
        "explode" => {
            for (dx, dy) in SPOKES {
                pen.line(
                    (11.0 + 4.2 * dx, 11.0 + 4.2 * dy),
                    (11.0 + 8.0 * dx, 11.0 + 8.0 * dy),
                );
            }
            pen.gold_x(1.6, 2.2);
        }
        "delete" => {
            let red = Stroke::new(2.0 * pen.scale(), RED);
            pen.line_in((6.0, 6.0), (16.0, 16.0), red);
            pen.line_in((16.0, 6.0), (6.0, 16.0), red);
        }
        "trim" => {
            pen.quad((3.0, 16.0), (11.0, 12.0), (19.0, 14.0), pen.stroke());
            pen.line((13.0, 4.0), (9.0, 19.0));
            pen.line_in(
                (15.0, 8.0),
                (19.0, 7.0),
                Stroke::new(1.6 * pen.scale(), RED),
            );
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
        "offset" => {
            pen.quad((4.0, 17.0), (11.0, 9.0), (18.0, 12.0), pen.stroke());
            pen.quad((4.0, 12.0), (11.0, 4.0), (18.0, 7.0), thin(1.0));
        }
        "fillet" => {
            pen.line((4.0, 18.0), (4.0, 9.0));
            pen.line((9.0, 4.0), (18.0, 4.0));
            pen.quad(
                (4.0, 9.0),
                (4.0, 4.0),
                (9.0, 4.0),
                Stroke::new(1.7 * pen.scale(), theme::ACCENT),
            );
        }
        "pointson" => {
            pen.cubic(
                (3.0, 17.0),
                (8.0, 8.0),
                (14.0, 8.0),
                (19.0, 15.0),
                pen.stroke(),
            );
            pen.dashed(&[(3.0, 17.0), (8.0, 6.0), (15.0, 6.0), (19.0, 15.0)], 1.0);
            for (x, y) in [(3.0, 17.0), (8.0, 6.0), (15.0, 6.0), (19.0, 15.0)] {
                pen.dot(x, y);
            }
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_icons_cover_the_common_commands_and_others_show_names() {
        for name in [
            "Line",
            "Circle",
            "Box",
            "Move",
            "BooleanUnion",
            "ExtrudeCrv",
            "Join",
        ] {
            assert!(super::has(name), "{name}");
        }
        assert!(!super::has("Shell"));
    }
}
