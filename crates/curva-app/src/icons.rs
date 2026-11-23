//! Icons for the command buttons (specs `docs/specs/ui-look-and-layout.md` §5 and
//! `docs/specs/gumball-and-icons-0.3.0.md` §2): line drawings in the theme's text colour, with the
//! accent for points and for what a command adds. Where Serpentine3D has a toolbar icon (it draws
//! them in code), the icon is redrawn from it here (MIT; credited in `ATTRIBUTION.md`); the others
//! take Rhino's idea for that command, drawn in the same style and colours, never Rhino's colour
//! coding.

use crate::theme;
use eframe::egui::{self, Color32, Pos2, Stroke, epaint};

mod curves;
mod editing;
mod files;
pub(crate) mod houses;
mod layer_tools;
mod measuring;
mod mesh;
mod organising;
mod selection;
mod solids;
mod surfaces;
mod transforms;
mod views;

/// The box every icon is drawn in, in Serpentine3D's units.
const BOX: f32 = 22.0;
/// Serpentine3D's red, for what a command takes away.
const RED: Color32 = Color32::from_rgb(0xc9, 0x5f, 0x5f);

type P = (f32, f32);

/// Draws one icon in a 22-unit box scaled to fit a rectangle; with no painter it draws nothing,
/// which tells whether an icon exists, or, given `seen`, only measures what it would draw.
struct Pen<'a> {
    painter: Option<&'a egui::Painter>,
    rect: egui::Rect,
    ink: Color32,
    /// Grows to hold everything drawn, when measuring.
    seen: Option<&'a std::cell::RefCell<egui::Rect>>,
    /// Moves the drawing so its ink sits in the middle of `rect` (the owner's rule: icons are
    /// always centred, across and down).
    shift: egui::Vec2,
}

impl Pen<'_> {
    fn scale(&self) -> f32 {
        self.rect.width().min(self.rect.height()) / BOX
    }

    fn at(&self, p: P) -> Pos2 {
        let scale = self.scale();
        let corner = self.rect.center() - egui::Vec2::splat(BOX * scale / 2.0);
        corner + egui::vec2(p.0 * scale, p.1 * scale) + self.shift
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
        match (self.painter, self.seen) {
            (Some(painter), _) => {
                painter.add(shape);
            }
            (None, Some(seen)) => {
                let bounds = shape.visual_bounding_rect();
                if bounds.is_finite() {
                    let mut seen = seen.borrow_mut();
                    *seen = seen.union(bounds);
                }
            }
            (None, None) => {}
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
        for dash in egui::Shape::dashed_line(&points, stroke, 3.0 * scale, 2.0 * scale) {
            self.add(dash);
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
        self.add(egui::Shape::circle_filled(
            self.at((x, y)),
            1.6 * self.scale(),
            theme::ACCENT,
        ));
    }

    fn ring(&self, x: f32, y: f32) {
        self.add(egui::Shape::circle_stroke(
            self.at((x, y)),
            1.8 * self.scale(),
            self.width(1.1),
        ));
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
            self.add(egui::Shape::circle_filled(
                self.at((11.0, 11.0)),
                2.6 * self.scale(),
                theme::ACCENT,
            ));
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
            seen: None,
            shift: egui::Vec2::ZERO,
        },
        name,
    )
}

/// Draws the icon for the command `name` in `rect`, if there is one; returns whether there was.
/// The drawing is measured first and moved so its ink is centred in `rect`, whatever part of the
/// box the icon was drawn in.
pub fn paint(painter: &egui::Painter, rect: egui::Rect, name: &str) -> bool {
    let shift = ink_shift(rect, name);
    draw(
        &Pen {
            painter: Some(painter),
            rect,
            ink: theme::current().text,
            seen: None,
            shift,
        },
        name,
    )
}

/// How far the icon `name` drawn in `rect` must move for its ink to sit in the middle of `rect`.
fn ink_shift(rect: egui::Rect, name: &str) -> egui::Vec2 {
    let seen = std::cell::RefCell::new(egui::Rect::NOTHING);
    let pen = Pen {
        painter: None,
        rect,
        ink: Color32::WHITE,
        seen: Some(&seen),
        shift: egui::Vec2::ZERO,
    };
    draw(&pen, name);
    let bounds = seen.into_inner();
    if bounds.is_finite() && bounds.is_positive() {
        rect.center() - bounds.center()
    } else {
        egui::Vec2::ZERO
    }
}

/// Draws the icon for `name`, trying each group in turn; returns whether there was one.
fn draw(pen: &Pen<'_>, name: &str) -> bool {
    let key = name.to_lowercase();
    let groups: [fn(&Pen<'_>, &str) -> bool; 35] = [
        layer_tools::layer_tools,
        curves::curves_1,
        curves::curves_2,
        curves::curves_3,
        curves::curves_4,
        curves::curves_5,
        curves::curves_6,
        curves::curves_7,
        curves::curves_8,
        curves::curves_9,
        curves::curves_10,
        curves::curves_11,
        surfaces::surfaces_1,
        surfaces::surfaces_2,
        surfaces::surfaces_3,
        surfaces::surfaces_4,
        solids::solids_1,
        solids::solids_2,
        mesh::mesh_1,
        mesh::mesh_2,
        mesh::mesh_3,
        transforms::transforms_1,
        transforms::transforms_2,
        transforms::transforms_3,
        selection::selection_1,
        selection::selection_2,
        editing::editing,
        organising::organising_1,
        organising::organising_2,
        organising::organising_3,
        measuring::measuring_1,
        measuring::measuring_2,
        files::files,
        views::views_1,
        views::views_2,
    ];
    groups.iter().any(|draw| draw(pen, &key))
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

    /// The owner's rule: every icon's ink sits in the middle of its button, across and down
    /// (SimplifyCrv was drawn off to one side).
    #[test]
    fn every_icon_is_centred_once_moved() {
        let rect =
            eframe::egui::Rect::from_min_size(eframe::egui::pos2(10.0, 20.0), [44.0; 2].into());
        for def in &curva_commands::commands::COMMANDS {
            let shift = super::ink_shift(rect, def.name);
            let seen = std::cell::RefCell::new(eframe::egui::Rect::NOTHING);
            let pen = super::Pen {
                painter: None,
                rect,
                ink: super::Color32::WHITE,
                seen: Some(&seen),
                shift,
            };
            super::draw(&pen, def.name);
            let bounds = seen.into_inner();
            let off = bounds.center() - rect.center();
            assert!(off.length() < 0.01, "{} is {off:?} off centre", def.name);
        }
    }
}
