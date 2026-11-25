//! Icons for the panels, shown in a panel's title row in place of the names when its tabs are too
//! many to name (owner, 25 November 2026): solid shapes with thick strokes, larger than the
//! command icons and unlike their thin line drawings, so a panel's tab never looks like a
//! command's button.

use crate::layout::Piece;
use eframe::egui::{self, Color32, Pos2, Stroke};

/// The box every panel icon is drawn in.
const BOX: f32 = 20.0;
/// The stroke's width, in the box's units: twice a command icon's.
const STROKE: f32 = 2.6;

/// Draws in a 20-unit box scaled into a rectangle.
struct Ink<'a> {
    painter: &'a egui::Painter,
    rect: egui::Rect,
    colour: Color32,
}

impl Ink<'_> {
    fn scale(&self) -> f32 {
        self.rect.width().min(self.rect.height()) / BOX
    }

    fn at(&self, (x, y): (f32, f32)) -> Pos2 {
        let scale = self.scale();
        self.rect.center() + egui::vec2((x - BOX / 2.0) * scale, (y - BOX / 2.0) * scale)
    }

    fn stroke(&self) -> Stroke {
        Stroke::new(STROKE * self.scale(), self.colour)
    }

    fn fill(&self, points: &[(f32, f32)]) {
        let points = points.iter().map(|p| self.at(*p)).collect();
        self.painter.add(egui::Shape::convex_polygon(
            points,
            self.colour,
            Stroke::NONE,
        ));
    }

    fn outline(&self, points: &[(f32, f32)]) {
        let points = points.iter().map(|p| self.at(*p)).collect();
        self.painter
            .add(egui::Shape::closed_line(points, self.stroke()));
    }

    fn line(&self, a: (f32, f32), b: (f32, f32)) {
        self.painter
            .line_segment([self.at(a), self.at(b)], self.stroke());
    }

    fn block(&self, (x0, y0): (f32, f32), (x1, y1): (f32, f32)) {
        let scale = self.scale();
        self.painter.rect_filled(
            egui::Rect::from_two_pos(self.at((x0, y0)), self.at((x1, y1))),
            1.5 * scale,
            self.colour,
        );
    }

    fn frame(&self, (x0, y0): (f32, f32), (x1, y1): (f32, f32)) {
        let scale = self.scale();
        self.painter.rect_stroke(
            egui::Rect::from_two_pos(self.at((x0, y0)), self.at((x1, y1))),
            2.0 * scale,
            self.stroke(),
            egui::StrokeKind::Middle,
        );
    }

    fn dot(&self, at: (f32, f32), radius: f32) {
        self.painter
            .circle_filled(self.at(at), radius * self.scale(), self.colour);
    }

    fn ring(&self, at: (f32, f32), radius: f32) {
        self.painter
            .circle_stroke(self.at(at), radius * self.scale(), self.stroke());
    }
}

/// Draws `piece`'s icon in `rect`, in `colour`.
pub(crate) fn panel_icon(painter: &egui::Painter, rect: egui::Rect, piece: Piece, colour: Color32) {
    let ink = Ink {
        painter,
        rect,
        colour,
    };
    match piece {
        Piece::Settings
        | Piece::Blocks
        | Piece::Layouts
        | Piece::AnnotationStyles
        | Piece::Print
        | Piece::Notes
        | Piece::Styles => document_icon(&ink, piece),
        _ => workspace_icon(&ink, piece),
    }
}

/// The icons of the panels always there: operations, layers, properties, the command line, the
/// assistant, display and status.
fn workspace_icon(ink: &Ink<'_>, piece: Piece) {
    match piece {
        // Four blocks: the buttons of the operations.
        Piece::CommandTabs => {
            ink.block((2.0, 2.0), (9.0, 9.0));
            ink.block((11.0, 2.0), (18.0, 9.0));
            ink.block((2.0, 11.0), (9.0, 18.0));
            ink.block((11.0, 11.0), (18.0, 18.0));
        }
        // Two sheets stacked, the top one solid.
        Piece::Layers => {
            ink.outline(&[(10.0, 9.0), (19.0, 13.5), (10.0, 18.0), (1.0, 13.5)]);
            ink.fill(&[(10.0, 2.0), (19.0, 6.5), (10.0, 11.0), (1.0, 6.5)]);
        }
        // Three sliders.
        Piece::Properties => {
            for (y, knob) in [(4.0, 13.0), (10.0, 6.0), (16.0, 11.0)] {
                ink.line((2.0, y), (18.0, y));
                ink.dot((knob, y), 2.8);
            }
        }
        // A prompt: a chevron and a line.
        Piece::Command => {
            ink.painter.add(egui::Shape::line(
                vec![ink.at((3.0, 5.0)), ink.at((9.0, 10.0)), ink.at((3.0, 15.0))],
                ink.stroke(),
            ));
            ink.block((11.0, 14.0), (18.0, 17.0));
        }
        // A four-pointed spark.
        Piece::Agent => ink.fill(&[
            (10.0, 1.0),
            (12.5, 7.5),
            (19.0, 10.0),
            (12.5, 12.5),
            (10.0, 19.0),
            (7.5, 12.5),
            (1.0, 10.0),
            (7.5, 7.5),
        ]),
        // A screen on its stand.
        Piece::Display => {
            ink.frame((2.0, 3.0), (18.0, 13.0));
            ink.block((8.0, 15.0), (12.0, 16.5));
            ink.block((5.0, 16.5), (15.0, 18.0));
        }
        // A target: where the pointer is.
        Piece::Status => {
            ink.ring((10.0, 10.0), 6.0);
            ink.dot((10.0, 10.0), 2.4);
            ink.line((10.0, 0.5), (10.0, 4.0));
            ink.line((10.0, 16.0), (10.0, 19.5));
            ink.line((0.5, 10.0), (4.0, 10.0));
            ink.line((16.0, 10.0), (19.5, 10.0));
        }
        _ => {}
    }
}

/// The icons of the panels a command opens: settings, blocks, layouts, annotation styles, print,
/// notes and styles.
fn document_icon(ink: &Ink<'_>, piece: Piece) {
    match piece {
        // A cog: a ring with teeth.
        Piece::Settings => {
            ink.ring((10.0, 10.0), 5.5);
            for k in 0..8 {
                let angle = k as f32 * std::f32::consts::TAU / 8.0;
                let (s, c) = angle.sin_cos();
                ink.line(
                    (10.0 + 6.0 * c, 10.0 + 6.0 * s),
                    (10.0 + 9.0 * c, 10.0 + 9.0 * s),
                );
            }
        }
        // A cube, its top solid.
        Piece::Blocks => {
            ink.fill(&[(10.0, 1.5), (18.0, 5.5), (10.0, 9.5), (2.0, 5.5)]);
            ink.outline(&[
                (2.0, 5.5),
                (10.0, 9.5),
                (18.0, 5.5),
                (18.0, 14.5),
                (10.0, 18.5),
                (2.0, 14.5),
            ]);
            ink.line((10.0, 9.5), (10.0, 18.5));
        }
        // A sheet with a drawing frame on it.
        Piece::Layouts => {
            ink.frame((2.0, 4.0), (18.0, 16.0));
            ink.block((5.0, 7.0), (12.0, 13.0));
            ink.block((13.5, 11.0), (15.5, 13.0));
        }
        // A dimension: a line between two ticks, with a solid A above.
        Piece::AnnotationStyles => {
            ink.fill(&[(10.0, 1.5), (14.5, 11.0), (5.5, 11.0)]);
            ink.line((2.0, 15.5), (18.0, 15.5));
            ink.line((2.0, 12.5), (2.0, 18.5));
            ink.line((18.0, 12.5), (18.0, 18.5));
        }
        // A printer: its body solid, the sheet coming out.
        Piece::Print => {
            ink.frame((6.0, 2.0), (14.0, 7.0));
            ink.block((2.0, 7.0), (18.0, 14.0));
            ink.frame((6.0, 12.0), (14.0, 18.5));
        }
        // A note with lines.
        Piece::Notes => {
            ink.frame((3.0, 2.0), (17.0, 18.0));
            for y in [7.0, 10.5, 14.0] {
                ink.line((6.0, y), (14.0, y));
            }
        }
        // Three swatches.
        Piece::Styles => {
            ink.dot((6.0, 7.0), 4.0);
            ink.ring((14.0, 7.0), 3.5);
            ink.dot((10.0, 14.5), 4.0);
        }
        _ => {}
    }
}
