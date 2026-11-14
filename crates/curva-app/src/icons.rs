//! Icons for the command buttons (specs `docs/specs/ui-look-and-layout.md` §5 and
//! `docs/specs/gumball-and-icons-0.3.0.md` §2): line drawings in the theme's text colour, with the
//! accent for points and for what a command adds. Where Serpentine3D has a toolbar icon (it draws
//! them in code), the icon is redrawn from it here (MIT; credited in `ATTRIBUTION.md`); the others
//! take Rhino's idea for that command, drawn in the same style and colours, never Rhino's colour
//! coding.

use crate::theme;
use eframe::egui::{self, Color32, Pos2, Stroke, epaint};

/// The box every icon is drawn in, in Serpentine3D's units.
const BOX: f32 = 22.0;
/// Serpentine3D's red, for what a command takes away.
const RED: Color32 = Color32::from_rgb(0xc9, 0x5f, 0x5f);

type P = (f32, f32);

/// Draws one icon in a 22-unit box scaled to fit a rectangle; with no painter it draws nothing,
/// which tells whether an icon exists.
struct Pen<'a> {
    painter: Option<&'a egui::Painter>,
    rect: egui::Rect,
    ink: Color32,
}

impl Pen<'_> {
    fn scale(&self) -> f32 {
        self.rect.width().min(self.rect.height()) / BOX
    }

    fn at(&self, p: P) -> Pos2 {
        let scale = self.scale();
        let corner = self.rect.center() - egui::Vec2::splat(BOX * scale / 2.0);
        corner + egui::vec2(p.0 * scale, p.1 * scale)
    }

    fn width(&self, w: f32) -> Stroke {
        Stroke::new(w * self.scale(), self.ink)
    }

    fn stroke(&self) -> Stroke {
        self.width(1.7)
    }

    fn accent(&self) -> Stroke {
        Stroke::new(1.7 * self.scale(), theme::ACCENT)
    }

    fn add(&self, shape: egui::Shape) {
        if let Some(painter) = self.painter {
            painter.add(shape);
        }
    }

    fn line(&self, a: P, b: P) {
        self.line_in(a, b, self.stroke());
    }

    fn line_in(&self, a: P, b: P, stroke: Stroke) {
        self.add(egui::Shape::line_segment([self.at(a), self.at(b)], stroke));
    }

    fn path_in(&self, points: &[P], closed: bool, stroke: Stroke) {
        let points: Vec<Pos2> = points.iter().map(|p| self.at(*p)).collect();
        self.add(if closed {
            egui::Shape::closed_line(points, stroke)
        } else {
            egui::Shape::line(points, stroke)
        });
    }

    fn path(&self, points: &[P], closed: bool) {
        self.path_in(points, closed, self.stroke());
    }

    fn dashed(&self, points: &[P], width: f32) {
        self.dashed_in(points, Stroke::new(width * self.scale(), self.ink));
    }

    fn dashed_in(&self, points: &[P], stroke: Stroke) {
        let points: Vec<Pos2> = points.iter().map(|p| self.at(*p)).collect();
        let scale = self.scale();
        if let Some(painter) = self.painter {
            painter.extend(egui::Shape::dashed_line(
                &points,
                stroke,
                3.0 * scale,
                2.0 * scale,
            ));
        }
    }

    fn filled(&self, points: &[P], fill: Color32) {
        let points: Vec<Pos2> = points.iter().map(|p| self.at(*p)).collect();
        self.add(egui::Shape::convex_polygon(points, fill, Stroke::NONE));
    }

    fn cubic_in(&self, from: P, c1: P, c2: P, to: P, stroke: Stroke) {
        self.add(egui::Shape::CubicBezier(
            epaint::CubicBezierShape::from_points_stroke(
                [self.at(from), self.at(c1), self.at(c2), self.at(to)],
                false,
                Color32::TRANSPARENT,
                stroke,
            ),
        ));
    }

    fn cubic(&self, from: P, c1: P, c2: P, to: P) {
        self.cubic_in(from, c1, c2, to, self.stroke());
    }

    fn quad_in(&self, from: P, c: P, to: P, stroke: Stroke) {
        self.add(egui::Shape::QuadraticBezier(
            epaint::QuadraticBezierShape::from_points_stroke(
                [self.at(from), self.at(c), self.at(to)],
                false,
                Color32::TRANSPARENT,
                stroke,
            ),
        ));
    }

    fn quad(&self, from: P, c: P, to: P) {
        self.quad_in(from, c, to, self.stroke());
    }

    /// Points round the ellipse in the box `(x, y, w, h)`, from `start` through `span` degrees,
    /// counter-clockwise from three o'clock as Qt counts them.
    fn arc_points(&self, (x, y, w, h): (f32, f32, f32, f32), start: f32, span: f32) -> Vec<P> {
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

    fn arc_in(&self, frame: (f32, f32, f32, f32), start: f32, span: f32, stroke: Stroke) {
        self.path_in(&self.arc_points(frame, start, span), false, stroke);
    }

    fn dot(&self, x: f32, y: f32) {
        if let Some(painter) = self.painter {
            painter.circle_filled(self.at((x, y)), 1.6 * self.scale(), theme::ACCENT);
        }
    }

    fn ring(&self, x: f32, y: f32) {
        if let Some(painter) = self.painter {
            painter.circle_stroke(self.at((x, y)), 1.8 * self.scale(), self.width(1.1));
        }
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32) {
        self.path(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)], true);
    }

    fn arrow_head(&self, tip: P, from: P, stroke: Stroke) {
        let (dx, dy) = (tip.0 - from.0, tip.1 - from.1);
        let len = dx.hypot(dy).max(1e-3);
        let (ux, uy) = (dx / len, dy / len);
        let back = (tip.0 - ux * 3.0, tip.1 - uy * 3.0);
        self.line_in(tip, (back.0 - uy * 2.2, back.1 + ux * 2.2), stroke);
        self.line_in(tip, (back.0 + uy * 2.2, back.1 - ux * 2.2), stroke);
    }

    fn arrow(&self, from: P, to: P) {
        self.line(from, to);
        self.arrow_head(to, from, self.stroke());
    }

    /// The accent X that Join and Explode share.
    fn gold_x(&self, arm: f32, width: f32) {
        let stroke = Stroke::new(width * self.scale(), theme::ACCENT);
        self.line_in((11.0 - arm, 11.0 - arm), (11.0 + arm, 11.0 + arm), stroke);
        self.line_in((11.0 + arm, 11.0 - arm), (11.0 - arm, 11.0 + arm), stroke);
    }

    /// The selection pointer, at the top left, for the Sel commands.
    fn pointer(&self) {
        let arrow = [
            (3.0, 2.0),
            (3.0, 12.0),
            (5.6, 9.6),
            (7.4, 13.4),
            (9.0, 12.7),
            (7.2, 9.0),
            (10.4, 9.0),
        ];
        self.filled(&arrow[..4], theme::current().text.gamma_multiply(0.35));
        self.path(&arrow, true);
    }

    /// A box drawn in a simple oblique view.
    fn cube(&self, x: f32, y: f32, s: f32) {
        let d = s * 0.4;
        self.rect(x, y + d, s, s);
        self.line((x, y + d), (x + d, y));
        self.line((x + s, y + d), (x + s + d, y));
        self.line((x + s, y + d + s), (x + s + d, y + s));
        self.line((x + d, y), (x + s + d, y));
        self.line((x + s + d, y), (x + s + d, y + s));
    }

    /// An eye, open or shut, for Show and Hide.
    fn eye(&self, open: bool) {
        self.quad((3.0, 11.0), (11.0, 3.0), (19.0, 11.0));
        if open {
            self.quad((3.0, 11.0), (11.0, 19.0), (19.0, 11.0));
            if let Some(painter) = self.painter {
                painter.circle_filled(self.at((11.0, 11.0)), 2.6 * self.scale(), theme::ACCENT);
            }
        } else {
            for (a, b) in [
                ((6.0, 9.0), (5.0, 12.0)),
                ((11.0, 8.0), (11.0, 11.5)),
                ((16.0, 9.0), (17.0, 12.0)),
            ] {
                self.line(a, b);
            }
        }
    }

    /// A magnifying glass, for the Zoom commands.
    fn magnifier(&self) {
        self.ellipse((3.0, 3.0, 11.0, 11.0));
        self.line_in((12.5, 12.5), (19.0, 19.0), self.width(2.4));
    }

    /// A camera, for the named and stepped views.
    fn camera(&self) {
        self.rect(3.0, 8.0, 12.0, 9.0);
        self.path(
            &[(15.0, 10.5), (19.0, 8.5), (19.0, 16.5), (15.0, 14.5)],
            true,
        );
    }

    /// A floppy disk, for Save.
    fn disk(&self) {
        self.path(
            &[
                (4.0, 3.0),
                (16.0, 3.0),
                (19.0, 6.0),
                (19.0, 19.0),
                (4.0, 19.0),
            ],
            true,
        );
        self.rect(7.0, 3.0, 8.0, 5.0);
        self.rect(7.0, 12.0, 9.0, 7.0);
    }
}

/// The four directions Join and Explode's lines run in.
const SPOKES: [P; 4] = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)];

/// Whether there is an icon for the command `name`.
#[must_use]
pub fn has(name: &str) -> bool {
    draw(
        &Pen {
            painter: None,
            rect: egui::Rect::from_min_size(Pos2::ZERO, egui::Vec2::splat(BOX)),
            ink: Color32::WHITE,
        },
        name,
    )
}

/// Draws the icon for the command `name` in `rect`, if there is one; returns whether there was.
pub fn paint(painter: &egui::Painter, rect: egui::Rect, name: &str) -> bool {
    draw(
        &Pen {
            painter: Some(painter),
            rect,
            ink: theme::current().text,
        },
        name,
    )
}

#[allow(clippy::too_many_lines)] // One arm per icon, as in Serpentine3D.
fn draw(pen: &Pen<'_>, name: &str) -> bool {
    let fill = pen.ink.gamma_multiply(0.27);
    let thin = |w: f32| pen.width(w);
    let red = Stroke::new(1.7 * pen.scale(), RED);
    match name.to_lowercase().as_str() {
        // Curves
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
        // The plane with a tick: it cuts this view.
        "enableclippingplane" => {
            pen.path(&[(2.0, 13.0), (7.0, 6.0), (15.0, 6.0), (10.0, 13.0)], true);
            pen.path(&[(13.0, 17.0), (15.5, 19.5), (20.0, 14.0)], false);
        }
        // The plane crossed out: it no longer cuts this view.
        "disableclippingplane" => {
            pen.path(&[(2.0, 13.0), (7.0, 6.0), (15.0, 6.0), (10.0, 13.0)], true);
            pen.line((14.0, 14.0), (20.0, 20.0));
            pen.line((20.0, 14.0), (14.0, 20.0));
        }
        // A frame with a view of the model inside.
        "detail" => {
            pen.rect(3.0, 4.0, 16.0, 14.0);
            pen.path(
                &[(6.0, 15.0), (10.0, 8.0), (13.0, 12.0), (16.0, 7.0)],
                false,
            );
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
        // Surfaces and solids
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
        "booleanintersection" => {
            for frame in [(3.0, 6.0, 11.0, 11.0), (8.0, 5.0, 11.0, 11.0)] {
                pen.dashed(&pen.arc_points(frame, 0.0, 360.0), 1.1);
            }
            let mut lens = pen.arc_points((3.0, 6.0, 11.0, 11.0), -70.0, 140.0);
            lens.extend(pen.arc_points((8.0, 5.0, 11.0, 11.0), 110.0, 140.0));
            pen.filled(&lens, theme::ACCENT.gamma_multiply(0.45));
            pen.path(&lens, true);
        }
        // Transforms
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
        "rotate3d" => {
            pen.arc((4.5, 6.0, 13.0, 9.0), 30.0, 280.0);
            pen.arrow_head((16.4, 7.2), (13.0, 6.2), pen.stroke());
            pen.line_in((4.0, 19.0), (18.0, 3.0), pen.accent());
        }
        "scale" => {
            pen.rect(4.0, 12.0, 6.0, 6.0);
            pen.rect(8.0, 4.0, 10.0, 10.0);
            pen.line((6.0, 16.0), (15.0, 7.0));
        }
        "scale1d" | "scale2d" | "scalenu" => {
            pen.rect(4.0, 7.0, 9.0, 9.0);
            pen.line_in((13.0, 11.5), (19.0, 11.5), pen.accent());
            pen.arrow_head((19.0, 11.5), (13.0, 11.5), pen.accent());
            if !name.eq_ignore_ascii_case("scale1d") {
                pen.line_in((8.5, 7.0), (8.5, 2.0), pen.accent());
                pen.arrow_head((8.5, 2.0), (8.5, 7.0), pen.accent());
            }
            if name.eq_ignore_ascii_case("scalenu") {
                pen.line_in((4.0, 16.0), (1.5, 19.5), pen.accent());
                pen.arrow_head((1.5, 19.5), (4.0, 16.0), pen.accent());
            }
        }
        "mirror" => {
            pen.dashed(&[(11.0, 3.0), (11.0, 19.0)], 1.2);
            let left = [(8.0, 6.0), (8.0, 16.0), (3.0, 16.0)];
            pen.filled(&left, fill);
            pen.path(&left, true);
            pen.path(&[(14.0, 6.0), (14.0, 16.0), (19.0, 16.0)], true);
        }
        "array" => {
            for row in 0..3 {
                for col in 0..3 {
                    let (x, y) = (3.0 + col as f32 * 6.0, 3.0 + row as f32 * 6.0);
                    if row == 0 && col == 0 {
                        pen.filled(
                            &[(x, y), (x + 4.0, y), (x + 4.0, y + 4.0), (x, y + 4.0)],
                            theme::ACCENT,
                        );
                    } else {
                        pen.rect(x, y, 4.0, 4.0);
                    }
                }
            }
        }
        "arraypolar" => {
            pen.dashed(&pen.arc_points((3.0, 3.0, 16.0, 16.0), 0.0, 360.0), 0.9);
            for i in 0..6 {
                let angle = (i as f32 * 60.0).to_radians();
                let (x, y) = (11.0 + 8.0 * angle.cos(), 11.0 - 8.0 * angle.sin());
                if i == 0 {
                    pen.dot(x, y);
                } else {
                    pen.ring(x, y);
                }
            }
            pen.dot(11.0, 11.0);
        }
        "arraycrv" => {
            pen.cubic((2.0, 17.0), (7.0, 3.0), (14.0, 19.0), (20.0, 5.0));
            for (x, y) in [(4.4, 12.3), (10.8, 11.0), (17.5, 9.4)] {
                pen.rect(x - 1.8, y - 1.8, 3.6, 3.6);
            }
        }
        "orient" | "orient3pt" => {
            pen.rect(3.0, 13.0, 5.0, 5.0);
            pen.rect(12.0, 3.0, 8.0, 8.0);
            pen.dashed(&[(8.0, 13.0), (12.0, 11.0)], 1.0);
            if name.eq_ignore_ascii_case("orient3pt") {
                for (x, y) in [(12.0, 11.0), (20.0, 11.0), (12.0, 3.0)] {
                    pen.dot(x, y);
                }
            } else {
                pen.dot(12.0, 11.0);
                pen.dot(20.0, 3.0);
            }
        }
        // Selection
        "selall" | "selnone" | "selcrv" | "selsrf" | "selsolid" | "selpt" | "selname"
        | "sellayer" | "seldup" | "sellast" | "selprev" | "invert" => {
            pen.pointer();
            match name.to_lowercase().as_str() {
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
            if name.eq_ignore_ascii_case("selfiltertoggle") {
                pen.line_in((3.0, 19.0), (19.0, 3.0), pen.accent());
            }
        }
        // Visibility and editing
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
            let back = name.eq_ignore_ascii_case("undo");
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
        // Measuring
        "distance" => {
            pen.line((3.0, 6.0), (3.0, 16.0));
            pen.line((19.0, 6.0), (19.0, 16.0));
            pen.line_in((3.0, 11.0), (19.0, 11.0), pen.accent());
            pen.arrow_head((3.0, 11.0), (19.0, 11.0), pen.accent());
            pen.arrow_head((19.0, 11.0), (3.0, 11.0), pen.accent());
        }
        "length" => {
            pen.cubic((3.0, 15.0), (8.0, 5.0), (14.0, 5.0), (19.0, 15.0));
            pen.line_in((3.0, 19.0), (19.0, 19.0), pen.accent());
            pen.line_in((3.0, 17.0), (3.0, 21.0), pen.accent());
            pen.line_in((19.0, 17.0), (19.0, 21.0), pen.accent());
        }
        "area" => {
            let quad = [(4.0, 5.0), (18.0, 4.0), (16.0, 18.0), (5.0, 17.0)];
            pen.path(&quad, true);
            for i in 0..4 {
                let o = i as f32 * 4.0;
                pen.line_in(
                    (5.0 + o, 17.0),
                    (9.0 + o, 5.0),
                    Stroke::new(1.0 * pen.scale(), theme::ACCENT),
                );
            }
        }
        "volume" => {
            pen.filled(
                &[(4.0, 8.0), (14.0, 8.0), (14.0, 18.0), (4.0, 18.0)],
                theme::ACCENT.gamma_multiply(0.45),
            );
            pen.cube(4.0, 4.0, 10.0);
        }
        // Files
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
        // Views
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
            match name.to_lowercase().as_str() {
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
            if name.eq_ignore_ascii_case("import") {
                pen.line_in((14.0, 2.0), (14.0, 13.0), pen.accent());
                pen.arrow_head((14.0, 14.0), (14.0, 9.0), pen.accent());
            } else {
                pen.line_in((14.0, 14.0), (14.0, 3.0), pen.accent());
                pen.arrow_head((14.0, 2.0), (14.0, 7.0), pen.accent());
            }
            if name.eq_ignore_ascii_case("exportselected") {
                pen.dot(6.0, 15.0);
            }
        }
        "namedview" | "undoview" | "redoview" => {
            pen.camera();
            match name.to_lowercase().as_str() {
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
        // Added for Rhino parity
        "sphere" => {
            pen.ellipse((3.0, 3.0, 16.0, 16.0));
            pen.dashed(&pen.arc_points((3.0, 8.5, 16.0, 5.0), 0.0, 180.0), 1.0);
            pen.arc((3.0, 8.5, 16.0, 5.0), 180.0, 180.0);
        }
        "cylinder" | "tube" => {
            pen.ellipse((5.0, 2.0, 12.0, 5.0));
            pen.line((5.0, 4.5), (5.0, 17.5));
            pen.line((17.0, 4.5), (17.0, 17.5));
            pen.arc((5.0, 15.0, 12.0, 5.0), 180.0, 180.0);
            if name.eq_ignore_ascii_case("tube") {
                pen.path_in(
                    &pen.arc_points((8.0, 3.0, 6.0, 3.0), 0.0, 360.0),
                    true,
                    pen.accent(),
                );
            }
        }
        "cone" => {
            pen.arc((4.0, 15.0, 14.0, 5.0), 180.0, 180.0);
            pen.dashed(&pen.arc_points((4.0, 15.0, 14.0, 5.0), 0.0, 180.0), 1.0);
            pen.line((4.0, 17.5), (11.0, 3.0));
            pen.line((18.0, 17.5), (11.0, 3.0));
            pen.dot(11.0, 3.0);
        }
        "truncatedcone" => {
            pen.ellipse((7.0, 3.0, 8.0, 3.0));
            pen.arc((3.0, 15.0, 16.0, 5.0), 180.0, 180.0);
            pen.line((7.0, 4.5), (3.0, 17.5));
            pen.line((15.0, 4.5), (19.0, 17.5));
        }
        "torus" => {
            pen.ellipse((2.0, 6.0, 18.0, 10.0));
            pen.arc((7.0, 9.0, 8.0, 4.0), 200.0, 140.0);
            pen.arc((8.0, 8.0, 6.0, 4.0), 20.0, 140.0);
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
        "arraylinear" => {
            for i in 0..4 {
                let x = 2.0 + i as f32 * 5.0;
                if i == 0 {
                    pen.filled(
                        &[(x, 9.0), (x + 3.5, 9.0), (x + 3.5, 12.5), (x, 12.5)],
                        theme::ACCENT,
                    );
                } else {
                    pen.rect(x, 9.0, 3.5, 3.5);
                }
            }
        }
        "angle" => {
            pen.line((4.0, 18.0), (19.0, 18.0));
            pen.line((4.0, 18.0), (14.0, 5.0));
            pen.arc_in((-4.0, 10.0, 16.0, 16.0), 0.0, 52.0, pen.accent());
        }
        "evaluatept" => {
            pen.line((11.0, 3.0), (11.0, 19.0));
            pen.line((3.0, 11.0), (19.0, 11.0));
            pen.ellipse((7.0, 7.0, 8.0, 8.0));
            pen.dot(11.0, 11.0);
        }
        "radius" => {
            pen.ellipse((3.0, 3.0, 16.0, 16.0));
            pen.line_in((11.0, 11.0), (16.7, 5.3), pen.accent());
            pen.dot(11.0, 11.0);
        }
        "what" => {
            pen.ellipse((3.0, 3.0, 16.0, 16.0));
            pen.line_in((11.0, 10.0), (11.0, 16.0), pen.accent());
            pen.dot(11.0, 6.8);
        }
        "selline" | "selpolyline" | "selcircle" | "selarc" | "selclosedcrv" | "selopencrv" => {
            pen.pointer();
            let accent = pen.accent();
            match name.to_lowercase().as_str() {
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
        "hideswap" => {
            pen.eye(true);
            pen.arrow_head((19.0, 3.0), (14.0, 3.0), pen.accent());
            pen.arrow_head((3.0, 19.0), (8.0, 19.0), pen.accent());
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
        "slot" => {
            pen.line((7.0, 7.0), (15.0, 7.0));
            pen.line((7.0, 15.0), (15.0, 15.0));
            pen.arc((11.0, 7.0, 8.0, 8.0), -90.0, 180.0);
            pen.arc((3.0, 7.0, 8.0, 8.0), 90.0, 180.0);
            pen.dot(7.0, 11.0);
            pen.dot(15.0, 11.0);
        }
        "ellipsoid" => {
            pen.ellipse((1.0, 5.0, 20.0, 12.0));
            pen.arc((1.0, 9.0, 20.0, 4.0), 180.0, 180.0);
            pen.dashed(&pen.arc_points((1.0, 9.0, 20.0, 4.0), 0.0, 180.0), 1.0);
        }
        "booleansplit" => {
            pen.arc((3.0, 6.0, 11.0, 11.0), 70.0, 250.0);
            pen.dashed(&pen.arc_points((8.0, 5.0, 11.0, 11.0), 0.0, 360.0), 1.1);
            pen.line_in((11.0, 2.0), (11.0, 20.0), pen.accent());
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
        "paraboloid" => {
            pen.ellipse((3.0, 3.0, 16.0, 5.0));
            pen.quad((3.0, 5.5), (11.0, 30.0), (19.0, 5.5));
        }
        "align" => {
            pen.line_in((3.0, 19.0), (19.0, 19.0), pen.accent());
            pen.rect(4.0, 11.0, 5.0, 8.0);
            pen.rect(12.0, 6.0, 6.0, 13.0);
        }
        "changedegree" => {
            pen.cubic((3.0, 17.0), (7.0, 3.0), (15.0, 3.0), (19.0, 17.0));
            pen.line_in((14.0, 15.0), (18.0, 15.0), pen.accent());
            pen.line_in((14.0, 18.5), (18.0, 18.5), pen.accent());
        }
        "areacentroid" | "volumecentroid" => {
            if name.eq_ignore_ascii_case("volumecentroid") {
                pen.cube(4.0, 4.0, 10.0);
            } else {
                pen.path(&[(3.0, 15.0), (8.0, 6.0), (19.0, 6.0), (14.0, 15.0)], true);
            }
            pen.line_in((9.0, 12.0), (13.0, 12.0), pen.accent());
            pen.line_in((11.0, 10.0), (11.0, 14.0), pen.accent());
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
        "pyramid" => {
            pen.path(&[(3.0, 16.0), (13.0, 19.0), (19.0, 14.0)], false);
            pen.dashed_in(&[(3.0, 16.0), (9.0, 12.0), (19.0, 14.0)], thin(1.0));
            pen.line((11.0, 3.0), (3.0, 16.0));
            pen.line((11.0, 3.0), (13.0, 19.0));
            pen.line((11.0, 3.0), (19.0, 14.0));
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
        "wirecut" => {
            pen.cube(4.0, 5.0, 11.0);
            pen.quad_in((2.0, 4.0), (12.0, 12.0), (20.0, 20.0), pen.accent());
        }
        "distribute" => {
            for x in [3.0, 9.5, 16.0] {
                pen.rect(x, 7.0, 3.0, 8.0);
            }
            pen.line_in((3.0, 18.0), (19.0, 18.0), pen.accent());
        }
        "selcolor" => {
            pen.pointer();
            pen.filled(
                &[(13.0, 13.0), (19.0, 13.0), (19.0, 19.0), (13.0, 19.0)],
                pen.accent().color,
            );
        }
        "lock" | "unlock" => {
            pen.rect(5.0, 10.0, 12.0, 9.0);
            if name.eq_ignore_ascii_case("lock") {
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
            if name.eq_ignore_ascii_case("group") {
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
        "showedges" => {
            pen.cube(4.0, 4.0, 10.0);
            pen.line_in((4.0, 18.0), (14.0, 18.0), pen.accent());
            pen.line_in((14.0, 18.0), (18.0, 14.0), pen.accent());
        }
        "plan" => {
            pen.rect(4.0, 4.0, 14.0, 14.0);
            pen.line_in((4.0, 11.0), (18.0, 11.0), thin(1.0));
            pen.line_in((11.0, 4.0), (11.0, 18.0), thin(1.0));
            pen.arrow((11.0, 1.0), (11.0, 8.0));
        }
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
        "moveface" | "pushpull" => {
            pen.cube(4.0, 8.0, 10.0);
            let top = [(4.0, 8.0), (10.0, 4.0), (20.0, 4.0), (14.0, 8.0)];
            pen.filled(&top, pen.accent().color.gamma_multiply(0.5));
            pen.arrow((12.0, 7.0), (12.0, 1.0));
        }
        "shear" => {
            pen.dashed_in(
                &[(4.0, 18.0), (4.0, 6.0), (14.0, 6.0), (14.0, 18.0)],
                thin(1.0),
            );
            pen.path(&[(4.0, 18.0), (9.0, 6.0), (19.0, 6.0), (14.0, 18.0)], true);
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
        "curvaturegraph" => {
            pen.quad((3.0, 17.0), (11.0, 5.0), (19.0, 17.0));
            for (x, y, tx, ty) in [
                (6.0, 13.0, 4.0, 9.0),
                (9.0, 10.5, 8.0, 5.0),
                (11.0, 10.0, 11.0, 3.0),
                (13.0, 10.5, 14.0, 5.0),
                (16.0, 13.0, 18.0, 9.0),
            ] {
                pen.line_in((x, y), (tx, ty), pen.accent());
            }
        }
        "cap" => {
            pen.ellipse((4.0, 3.0, 14.0, 5.0));
            pen.line((4.0, 5.5), (4.0, 16.5));
            pen.line((18.0, 5.5), (18.0, 16.5));
            pen.arc((4.0, 14.0, 14.0, 5.0), 0.0, 180.0);
            pen.filled(
                &pen.arc_points((4.0, 3.0, 14.0, 5.0), 0.0, 360.0),
                pen.accent().color.gamma_multiply(0.6),
            );
        }
        "extractisocurve" => {
            pen.path(&[(3.0, 5.0), (19.0, 5.0), (19.0, 17.0), (3.0, 17.0)], true);
            pen.line_in((11.0, 5.0), (11.0, 17.0), pen.accent());
            pen.line_in((3.0, 11.0), (19.0, 11.0), thin(1.0));
        }
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
        "twist" => {
            pen.cubic((6.0, 19.0), (6.0, 11.0), (16.0, 11.0), (16.0, 3.0));
            pen.cubic((16.0, 19.0), (16.0, 11.0), (6.0, 11.0), (6.0, 3.0));
            pen.line_in((11.0, 2.0), (11.0, 20.0), pen.accent());
        }
        "bend" => {
            pen.arc((3.0, 5.0, 16.0, 28.0), 180.0, 90.0);
            pen.dashed_in(&[(3.0, 19.0), (3.0, 3.0)], thin(1.0));
            pen.dot(11.0, 5.0);
        }
        "taper" => {
            pen.path(&[(4.0, 19.0), (18.0, 19.0), (14.0, 3.0), (8.0, 3.0)], true);
            pen.line_in((11.0, 2.0), (11.0, 20.0), pen.accent());
        }
        "flow" => {
            pen.line((3.0, 18.0), (12.0, 18.0));
            pen.rect(5.0, 14.0, 4.0, 3.0);
            pen.quad_in((10.0, 12.0), (14.0, 2.0), (20.0, 6.0), pen.accent());
            pen.arrow((9.0, 13.0), (13.0, 8.0));
        }
        "cageedit" => {
            pen.cube(4.0, 6.0, 11.0);
            pen.dot(4.0, 6.0);
            pen.dot(15.0, 17.0);
            pen.line_in((15.0, 17.0), (20.0, 20.0), pen.accent());
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
        "unrollsrf" => {
            pen.arc((2.0, 6.0, 10.0, 12.0), 90.0, 180.0);
            pen.path(
                &[(12.0, 6.0), (20.0, 6.0), (20.0, 18.0), (12.0, 18.0)],
                true,
            );
            pen.arrow((8.0, 12.0), (13.0, 12.0));
        }
        "extrudeface" => {
            pen.cube(4.0, 8.0, 10.0);
            pen.path_in(
                &[(4.0, 8.0), (10.0, 4.0), (20.0, 4.0), (14.0, 8.0)],
                true,
                pen.accent(),
            );
            pen.arrow((12.0, 6.0), (12.0, 1.0));
        }
        "filletsrf" | "chamfersrf" => {
            pen.line((3.0, 19.0), (19.0, 19.0));
            pen.line((3.0, 3.0), (3.0, 19.0));
            if name.eq_ignore_ascii_case("filletsrf") {
                pen.arc_in((3.0, 9.0, 20.0, 20.0), 180.0, 90.0, pen.accent());
            } else {
                pen.line_in((3.0, 11.0), (11.0, 19.0), pen.accent());
            }
        }
        "splitsrf" => {
            pen.path(&[(3.0, 5.0), (19.0, 5.0), (19.0, 17.0), (3.0, 17.0)], true);
            pen.quad_in((10.0, 3.0), (13.0, 11.0), (10.0, 19.0), pen.accent());
        }
        "zebra" => {
            pen.ellipse((3.0, 3.0, 16.0, 16.0));
            for (a, b) in [
                ((5.0, 7.0), (17.0, 7.0)),
                ((3.5, 11.0), (18.5, 11.0)),
                ((5.0, 15.0), (17.0, 15.0)),
            ] {
                pen.line_in(a, b, pen.accent());
            }
        }
        "count" => {
            for (x, y) in [
                (5.0, 6.0),
                (11.0, 6.0),
                (17.0, 6.0),
                (5.0, 12.0),
                (11.0, 12.0),
            ] {
                pen.dot(x, y);
            }
            pen.line_in((3.0, 18.0), (19.0, 18.0), pen.accent());
        }
        "changelayer" | "matchproperties" => {
            pen.path(&[(2.0, 8.0), (11.0, 4.0), (20.0, 8.0), (11.0, 12.0)], true);
            pen.path_in(
                &[(2.0, 14.0), (11.0, 10.0), (20.0, 14.0), (11.0, 18.0)],
                true,
                pen.accent(),
            );
            if name.eq_ignore_ascii_case("matchproperties") {
                pen.dot(11.0, 8.0);
            }
        }
        "linetype" => {
            pen.dashed_in(&[(3.0, 7.0), (19.0, 7.0)], pen.accent());
            pen.line((3.0, 12.0), (19.0, 12.0));
            pen.dashed_in(&[(3.0, 17.0), (19.0, 17.0)], thin(1.0));
        }
        "printwidth" => {
            pen.line_in((3.0, 6.0), (19.0, 6.0), thin(1.0));
            pen.line((3.0, 11.0), (19.0, 11.0));
            pen.line_in((3.0, 17.0), (19.0, 17.0), thin(3.5));
        }
        "setusertext" | "getusertext" => {
            pen.rect(4.0, 4.0, 14.0, 14.0);
            for y in [8.0, 11.0, 14.0] {
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
            let front = name.to_lowercase().starts_with("bring");
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
        "dupfaceborder" => {
            pen.cube(4.0, 6.0, 11.0);
            pen.path_in(
                &[(4.0, 6.0), (10.0, 2.0), (21.0, 2.0), (15.0, 6.0)],
                true,
                pen.accent(),
            );
        }
        "textobject" => {
            pen.path(&[(4.0, 18.0), (9.0, 4.0), (14.0, 18.0)], false);
            pen.line((6.0, 13.0), (12.0, 13.0));
            pen.line_in((16.0, 10.0), (16.0, 18.0), pen.accent());
        }
        "draftanalysis" => {
            pen.path(&[(4.0, 19.0), (18.0, 19.0), (15.0, 4.0), (7.0, 4.0)], true);
            pen.line_in((15.0, 4.0), (18.0, 19.0), pen.accent());
        }
        "block"
        | "insert"
        | "blockmanager"
        | "blockedit"
        | "explodeblock"
        | "replaceblock"
        | "selblockinstancenamed" => {
            let lower = name.to_lowercase();
            pen.rect(3.0, 6.0, 9.0, 9.0);
            pen.rect(10.0, 9.0, 9.0, 9.0);
            match lower.as_str() {
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
        "editboolean" | "breakboolean" => {
            pen.ellipse((2.0, 5.0, 12.0, 12.0));
            pen.ellipse((8.0, 5.0, 12.0, 12.0));
            if name.eq_ignore_ascii_case("editboolean") {
                pen.dashed_in(&[(8.0, 6.0), (8.0, 16.0)], pen.accent());
                pen.dot(14.0, 11.0);
            } else {
                pen.line_in((3.0, 19.0), (19.0, 3.0), pen.accent());
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
        assert!(!super::has("NoSuchCommand"));
    }

    #[test]
    fn every_command_has_an_icon() {
        for def in &curva_commands::commands::COMMANDS {
            assert!(super::has(def.name), "{} has no icon", def.name);
        }
    }
}
