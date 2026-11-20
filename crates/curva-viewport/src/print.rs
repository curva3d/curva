//! Layouts made into pages to print or export (spec `drawing-output.md` §2): the sheet's own
//! objects, and each detail's model drawn through its camera, clipped to its frame, all as
//! vectors in paper millimetres. Shaded details print as their lines for now; raster details and
//! exact hidden lines (`make2d.md`) come later.

use curva_document::drawing::{Colour, Mark, Page, clip_polygon, clip_polyline};
use curva_document::{Document, Layer, Object, Shape, State};
use curva_kernel::{Curve, Point};

use crate::StandardView;
use curva_document::fields::FieldContext;

/// How colours print (spec `drawing-output.md` §1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PrintColour {
    /// As shown, except that white and near-white, which vanish on paper, print black.
    #[default]
    Display,
    /// Everything black.
    Black,
}

impl PrintColour {
    fn ink(self, colour: Colour) -> Colour {
        let light = 0.299 * colour[0] + 0.587 * colour[1] + 0.114 * colour[2];
        match self {
            Self::Black => [0.0; 3],
            Self::Display if light > 0.85 => [0.0; 3],
            Self::Display => colour,
        }
    }
}

/// Where model points land on paper, and the box marks are clipped to.
struct Placing<'a> {
    project: &'a dyn Fn(Point) -> Option<[f64; 2]>,
    frame: Option<[[f64; 2]; 2]>,
}

impl Placing<'_> {
    fn line(&self, points: &[Point]) -> Vec<Vec<[f64; 2]>> {
        // A point behind a perspective camera breaks the line there.
        let mut runs = vec![Vec::new()];
        for p in points {
            match (self.project)(*p) {
                Some(on) => {
                    if let Some(run) = runs.last_mut() {
                        run.push(on);
                    }
                }
                None => runs.push(Vec::new()),
            }
        }
        runs.retain(|r| !r.is_empty());
        match self.frame {
            Some(frame) => runs.iter().flat_map(|r| clip_polyline(r, frame)).collect(),
            None => runs,
        }
    }

    fn outline(&self, points: &[Point]) -> Option<Vec<[f64; 2]>> {
        let on: Option<Vec<[f64; 2]>> = points.iter().map(|p| (self.project)(*p)).collect();
        let on = on?;
        let on = match self.frame {
            Some(frame) => clip_polygon(&on, frame),
            None => on,
        };
        (on.len() >= 3).then_some(on)
    }
}

/// What one object prints as, through `placing`.
fn marks_of(
    state: &State,
    object: &Object,
    layer: Option<&Layer>,
    placing: &Placing<'_>,
    fields: &FieldContext,
    colour_mode: PrintColour,
    marks: &mut Vec<Mark>,
) {
    let colour = colour_mode.ink(layer.map_or(crate::CURVE_COLOUR, |l| l.colour));
    let width = object.print_width.resolved(layer).unwrap_or(0.0);
    let dashes = object.linetype.resolved(layer).pattern().to_vec();
    let shapes: Vec<Shape> = match &object.shape {
        Shape::Instance(instance) => state
            .instance_shapes(instance)
            .into_iter()
            .map(|(shape, _)| shape)
            .collect(),
        shape => vec![shape.clone()],
    };
    let line = |points: &[Point], marks: &mut Vec<Mark>| {
        for run in placing.line(points) {
            marks.push(Mark::Line {
                points: run,
                closed: false,
                colour,
                width,
                dashes: dashes.clone(),
                source: Some(object.id),
            });
        }
    };
    for shape in shapes {
        match &shape {
            Shape::Curve(_)
            | Shape::Body(_)
            | Shape::Instance(_)
            | Shape::Mesh(_)
            | Shape::SubD(_) => {
                for points in shape_lines(&shape) {
                    line(&points, marks);
                }
            }
            // Dots are drawn on the screen and do not print yet.
            Shape::Annotation(curva_document::annotation::Annotation::Dot(_)) => {}
            Shape::Annotation(annotation) => {
                let shown = crate::shown_annotation(annotation, state, object.space, fields);
                let drawing =
                    curva_document::shape::annotation_drawing(&shown, state, object.space);
                // Letters and arrowheads print filled; their outlines are not drawn again.
                let filled: Vec<&Curve> = drawing.fills.iter().flatten().collect();
                for curve in drawing.lines.iter().filter(|c| !filled.contains(c)) {
                    if let Ok(points) = curve.sample() {
                        line(&points, marks);
                    }
                }
                for group in &drawing.fills {
                    let contours: Vec<Vec<[f64; 2]>> = group
                        .iter()
                        .filter_map(|c| c.sample().ok())
                        .filter_map(|points| placing.outline(&points))
                        .collect();
                    if !contours.is_empty() {
                        marks.push(Mark::Fill {
                            contours,
                            colour,
                            source: Some(object.id),
                        });
                    }
                }
            }
        }
    }
}

/// The lines a shape other than an annotation prints as, in model coordinates.
fn shape_lines(shape: &Shape) -> Vec<Vec<Point>> {
    let sampled = |curves: Vec<Curve>| curves.iter().filter_map(|c| c.sample().ok()).collect();
    match shape {
        Shape::Curve(curve) => curve.sample().into_iter().collect(),
        Shape::Body(body) => sampled(body.edge_curves().unwrap_or_default()),
        // A mesh prints its borders: all its wires would blacken a scan's print (#267).
        Shape::Mesh(mesh) => {
            let at = mesh.vertices();
            mesh.naked_edges()
                .into_iter()
                .filter_map(|[a, b]| Some(vec![*at.get(a as usize)?, *at.get(b as usize)?]))
                .collect()
        }
        // A SubD prints the lines it draws: its creases and borders, or shown flat its cage's
        // edges (#324).
        Shape::SubD(subd) => sampled(crate::subds::drawing(subd).lines),
        Shape::Instance(_) | Shape::Annotation(_) => Vec::new(),
    }
}

/// Whether an object prints: not hidden, on a shown layer (with its parents on), and not on a
/// layer `hidden` here, or under one.
fn prints<'a>(state: &'a State, object: &Object, hidden: &[u64]) -> Option<Option<&'a Layer>> {
    let layer = state.layers.iter().find(|l| l.id == object.layer);
    let hidden_here = state
        .layer_chain(object.layer)
        .iter()
        .any(|l| hidden.contains(&l.id));
    let shown = !object.hidden && state.layer_shown(object.layer) && !hidden_here;
    shown.then_some(layer)
}

/// The page a layout prints as: its details' views of the model, then its own objects over
/// them; `None` when there is no such layout.
#[must_use]
pub fn layout_page(
    doc: &Document,
    layout: u64,
    fields: &FieldContext,
    colour_mode: PrintColour,
) -> Option<Page> {
    let sheet = doc.layout(layout)?;
    let state = doc.state();
    let to_mm = sheet.unit.millimetres();
    let model_mm = state.unit.millimetres();
    let mut marks = Vec::new();
    for (detail, [low, high]) in doc.details_on(layout) {
        let view = StandardView::from_key(&detail.projection).unwrap_or(StandardView::Top);
        let mut camera = view.camera();
        match detail.scale {
            Some(scale) => {
                let model_height = (high[1] - low[1]) * to_mm / scale / model_mm;
                camera.show_exactly(detail.target, model_height / 2.0);
            }
            None => camera.show_exactly(detail.target, detail.distance / 2.0),
        }
        // The frame as a screen of its own size in millimetres, downward Y.
        let size = [(high[0] - low[0]) * to_mm, (high[1] - low[1]) * to_mm];
        if size[0] <= 0.0 || size[1] <= 0.0 {
            continue;
        }
        let corner = [low[0] * to_mm, high[1] * to_mm];
        let project = move |p: Point| {
            camera
                .project(p, size)
                .map(|[x, y]| [corner[0] + x, corner[1] - y])
        };
        let placing = Placing {
            project: &project,
            frame: Some([
                [low[0] * to_mm, low[1] * to_mm],
                [high[0] * to_mm, high[1] * to_mm],
            ]),
        };
        for object in state.objects.iter().filter(|o| o.space.is_none()) {
            if let Some(layer) = prints(state, object, &detail.hidden_layers) {
                marks_of(
                    state,
                    object,
                    layer,
                    &placing,
                    fields,
                    colour_mode,
                    &mut marks,
                );
            }
        }
    }
    let flat = move |p: Point| Some([p[0] * to_mm, p[1] * to_mm]);
    let placing = Placing {
        project: &flat,
        frame: None,
    };
    for object in state.objects.iter().filter(|o| o.space == Some(layout)) {
        if let Some(layer) = prints(state, object, &[]) {
            marks_of(
                state,
                object,
                layer,
                &placing,
                fields,
                colour_mode,
                &mut marks,
            );
        }
    }
    Some(Page {
        name: sheet.name.clone(),
        size: [sheet.size[0] * to_mm, sheet.size[1] * to_mm],
        marks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_document::drawing::Mark;

    fn lines(page: &Page) -> Vec<&Vec<[f64; 2]>> {
        page.marks
            .iter()
            .filter_map(|m| match m {
                Mark::Line { points, .. } => Some(points),
                Mark::Fill { .. } => None,
            })
            .collect()
    }

    #[test]
    fn a_layout_prints_its_sheet_objects_at_paper_size() {
        let mut doc = Document::default();
        let paper = curva_document::layouts::paper_size("A4").unwrap();
        let sheet = doc.add_layout("Plan".to_owned(), paper, true);
        doc.set_space(Some(sheet));
        doc.add(Curve::line([10.0, 10.0, 0.0], [110.0, 10.0, 0.0]).unwrap());
        // A model line does not print where no detail shows it.
        doc.set_space(None);
        doc.add(Curve::line([0.0; 3], [5.0, 0.0, 0.0]).unwrap());
        let page =
            layout_page(&doc, sheet, &FieldContext::default(), PrintColour::Display).unwrap();
        assert_eq!(page.name, "Plan");
        assert!((page.size[0] - 297.0).abs() < 1e-9 && (page.size[1] - 210.0).abs() < 1e-9);
        let found = lines(&page);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].first(), Some(&[10.0, 10.0]));
        // White on screen prints black.
        let Mark::Line { colour, .. } = &page.marks[0] else {
            panic!();
        };
        assert_eq!(*colour, [0.0; 3]);
    }

    #[test]
    fn a_detail_prints_the_model_at_its_scale_clipped_to_its_frame() {
        let mut doc = Document::default();
        // A model line 10 m long, in millimetres, through the origin.
        doc.add(Curve::line([-5000.0, 0.0, 0.0], [5000.0, 0.0, 0.0]).unwrap());
        let paper = curva_document::layouts::paper_size("A4").unwrap();
        let sheet = doc.add_layout("Plan".to_owned(), paper, true);
        doc.set_space(Some(sheet));
        let frame = doc.add(
            Curve::polyline(
                vec![
                    [50.0, 50.0, 0.0],
                    [150.0, 50.0, 0.0],
                    [150.0, 150.0, 0.0],
                    [50.0, 150.0, 0.0],
                ],
                true,
            )
            .unwrap(),
        );
        doc.add_detail(
            sheet,
            curva_document::layouts::Detail {
                frame,
                projection: "Top".to_owned(),
                target: [0.0; 3],
                distance: 1.0,
                scale: Some(0.01),
                display: "Wireframe".to_owned(),
                locked: false,
                hidden_layers: Vec::new(),
            },
        );
        let page =
            layout_page(&doc, sheet, &FieldContext::default(), PrintColour::Display).unwrap();
        // At 1:100 the 10 m line is 100 mm on paper, exactly the frame's width: it runs across
        // the frame's middle, from edge to edge.
        let found = lines(&page);
        let across = found
            .iter()
            .find(|run| run.len() == 2 && (run[0][1] - 100.0).abs() < 1e-6)
            .expect("the model line across the detail");
        let xs = [
            across[0][0].min(across[1][0]),
            across[0][0].max(across[1][0]),
        ];
        assert!(
            (xs[0] - 50.0).abs() < 1e-6 && (xs[1] - 150.0).abs() < 1e-6,
            "{xs:?}"
        );
    }

    #[test]
    fn text_prints_filled() {
        let mut doc = Document::default();
        let paper = curva_document::layouts::paper_size("A4").unwrap();
        let sheet = doc.add_layout("Plan".to_owned(), paper, true);
        doc.set_space(Some(sheet));
        doc.add_annotation(curva_document::annotation::Annotation::Text(
            curva_document::annotation::Text {
                content: "AB".to_owned(),
                origin: [20.0, 20.0, 0.0],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 1.0, 0.0],
                height: 5.0,
                font: None,
                align: curva_document::annotation::TextAlign::Left,
                style: 0,
            },
        ));
        let page =
            layout_page(&doc, sheet, &FieldContext::default(), PrintColour::Display).unwrap();
        assert!(page.marks.iter().all(|m| matches!(m, Mark::Fill { .. })));
        assert_eq!(page.marks.len(), 2);
    }
}
