//! `Hatch`, as in Rhino 8 (spec `annotation.md` §5; `rhino-reference.md`): closed flat curves,
//! or with `Boundary`, clicks inside the regions crossing curves make; each outline with the
//! outlines inside it (its holes) becomes one hatch. `Pattern`, `Scale` and `Rotation` are set
//! as options here, where Rhino has a dialog; Properties changes them afterwards.

use crate::commands::curves::curve_boolean::regions;
use crate::commands::{curves_of, unexpected};
use crate::{Answer, Command, Kind, Message, Request, Step};
use curva_document::Document;
use curva_document::annotation::{Annotation, Hatch as Made};
use curva_kernel::{Curve, Point, dot, sub};

const PATTERN: &str = "Pattern";
const SCALE: &str = "Scale";
const ROTATION: &str = "Rotation";
const BOUNDARY: &str = "Boundary";

/// The pattern a new hatch takes until another is chosen.
const FIRST_PATTERN: &str = "Hatch1";

/// What the command asks next.
#[derive(Default, PartialEq)]
enum Asking {
    #[default]
    Curves,
    Pattern,
    Scale,
    Rotation,
    Inside,
}

/// The running command.
pub struct Hatch {
    asking: Asking,
    pattern: String,
    scale: f64,
    rotation: f64,
    boundary: bool,
    curves: Vec<Curve>,
    picks: Vec<Point>,
}

impl Default for Hatch {
    fn default() -> Self {
        Self {
            asking: Asking::Curves,
            pattern: FIRST_PATTERN.to_owned(),
            scale: 1.0,
            rotation: 0.0,
            boundary: false,
            curves: Vec::new(),
            picks: Vec::new(),
        }
    }
}

fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(v: Point) -> Option<Point> {
    let length = dot(v, v).sqrt();
    (length > 1e-12).then(|| v.map(|c| c / length))
}

/// The plane a closed curve lies in, as its X and Y directions and a point on it; `None` when it
/// is not flat. X follows the work plane's X where it can, so patterns line up across hatches.
fn plane_of(curve: &Curve, work_x: Point) -> Option<(Point, Point, Point)> {
    let points = curve.sample().ok()?;
    let first = *points.first()?;
    // Newell's normal of the polygon.
    let mut normal = [0.0; 3];
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
        normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
        normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let n = unit(normal)?;
    let size = points
        .iter()
        .map(|p| dot(sub(*p, first), sub(*p, first)).sqrt())
        .fold(0.0, f64::max);
    let flat = points
        .iter()
        .all(|p| dot(sub(*p, first), n).abs() <= 1e-6 * size.max(1.0));
    if !flat {
        return None;
    }
    // Facing up the work plane's normal side, as drawn curves usually are.
    let n = if n[2] < 0.0 { n.map(|c| -c) } else { n };
    let x = unit(sub(work_x, n.map(|c| c * dot(work_x, n))))
        .or_else(|| unit(sub(points[1 % points.len()], first)))?;
    Some((x, cross(n, x), first))
}

/// Whether a polygon (in plane coordinates) holds a point, even–odd.
fn holds(ring: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut inside = false;
    for (i, a) in ring.iter().enumerate() {
        let b = ring[(i + 1) % ring.len()];
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0])
        {
            inside = !inside;
        }
    }
    inside
}

/// Closed curves in one plane grouped into outlines with their holes: a curve inside an even
/// number of others is an outline, and one inside an odd number is a hole of the outline just
/// around it.
fn nested(curves: &[Curve], origin: Point, x: Point, y: Point) -> Vec<Vec<Curve>> {
    let flat = |p: Point| [dot(sub(p, origin), x), dot(sub(p, origin), y)];
    let rings: Vec<Vec<[f64; 2]>> = curves
        .iter()
        .map(|c| {
            c.sample()
                .map(|ps| ps.into_iter().map(flat).collect())
                .unwrap_or_default()
        })
        .collect();
    let around = |i: usize| -> Vec<usize> {
        let Some(&p) = rings[i].first() else {
            return Vec::new();
        };
        (0..rings.len())
            .filter(|&j| j != i && !rings[j].is_empty() && holds(&rings[j], p))
            .collect()
    };
    let depth: Vec<usize> = (0..rings.len()).map(|i| around(i).len()).collect();
    let mut groups: Vec<(usize, Vec<Curve>)> = (0..rings.len())
        .filter(|&i| depth[i].is_multiple_of(2) && !rings[i].is_empty())
        .map(|i| (i, vec![curves[i].clone()]))
        .collect();
    for i in (0..rings.len()).filter(|&i| !depth[i].is_multiple_of(2)) {
        // The outline just around this hole is the one a level up.
        if let Some(outer) = around(i).into_iter().find(|&j| depth[j] + 1 == depth[i])
            && let Some(group) = groups.iter_mut().find(|(j, _)| *j == outer)
        {
            group.1.push(curves[i].clone());
        }
    }
    groups.into_iter().map(|(_, loops)| loops).collect()
}

impl Hatch {
    fn ask(&self) -> Step {
        let request = match self.asking {
            Asking::Curves if self.boundary => Request::new("prompt.hatch.boundary", Kind::Objects),
            Asking::Curves => Request::new("prompt.hatch.curves", Kind::Objects),
            Asking::Pattern => Request::new("prompt.hatch.pattern", Kind::Path),
            Asking::Scale => Request::new("prompt.hatch.scale", Kind::Number),
            Asking::Rotation => Request::new("prompt.hatch.rotation", Kind::Number),
            Asking::Inside => Request {
                allow_empty: !self.picks.is_empty(),
                ..Request::new("prompt.hatch.inside", Kind::Point)
            },
        };
        Step::Ask(Request {
            options: vec![PATTERN, SCALE, ROTATION, BOUNDARY],
            option_labels: vec![
                format!("{PATTERN}={}", self.pattern),
                format!("{SCALE}={}", self.scale),
                format!("{ROTATION}={}", self.rotation.to_degrees()),
                format!("{BOUNDARY}={}", if self.boundary { "Yes" } else { "No" }),
            ],
            ..request
        })
    }

    /// The step after an option's value: back to where the command was.
    fn resume(&mut self) -> Step {
        self.asking = if self.curves.is_empty() {
            Asking::Curves
        } else {
            Asking::Inside
        };
        self.ask()
    }

    /// Adds a hatch for each group of outlines; none when the curves are not flat.
    fn make(&self, doc: &mut Document, groups: Vec<Vec<Curve>>) -> Step {
        let [work_x, _] = doc.work_plane();
        let mut made = 0;
        for loops in groups {
            let Some(first) = loops.first() else {
                continue;
            };
            let Some((x, y, on)) = plane_of(first, work_x) else {
                continue;
            };
            // The pattern starts from the world origin, brought onto the plane, so hatches
            // side by side line up, as in Rhino.
            let n = cross(x, y);
            let base: Point = std::array::from_fn(|i| n[i] * dot(on, n));
            doc.add_annotation(Annotation::Hatch(Made {
                loops,
                pattern: self.pattern.clone(),
                scale: self.scale,
                rotation: self.rotation,
                base,
                x,
                y,
            }));
            made += 1;
        }
        if made == 0 {
            return Step::Failed(Message::new("error.hatch_nothing"));
        }
        Step::Done(Some(Message::with("done.hatch", vec![made.to_string()])))
    }

    fn chosen(&mut self, doc: &mut Document, ids: &[u64]) -> Step {
        let curves: Vec<Curve> = curves_of(doc, ids)
            .into_iter()
            .map(|(_, c)| c)
            .filter(|c| c.is_closed().unwrap_or(false))
            .collect();
        if curves.is_empty() {
            return Step::Failed(Message::new("error.hatch_nothing"));
        }
        if self.boundary {
            self.curves = curves;
            self.asking = Asking::Inside;
            return self.ask();
        }
        let [work_x, _] = doc.work_plane();
        // Curves are grouped plane by plane: each with the others flat in its plane.
        let mut groups = Vec::new();
        let mut left = curves;
        while let Some(first) = left.first().cloned() {
            let Some((x, y, on)) = plane_of(&first, work_x) else {
                left.remove(0);
                continue;
            };
            let n = cross(x, y);
            let (same, other): (Vec<Curve>, Vec<Curve>) = left.into_iter().partition(|c| {
                c.sample().is_ok_and(|ps| {
                    let size = ps
                        .iter()
                        .map(|p| dot(sub(*p, on), sub(*p, on)).sqrt())
                        .fold(1.0, f64::max);
                    ps.iter().all(|p| dot(sub(*p, on), n).abs() <= 1e-6 * size)
                })
            });
            groups.extend(nested(&same, on, x, y));
            left = other;
        }
        self.make(doc, groups)
    }
}

impl Command for Hatch {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => self.ask(),
            Some(Answer::Option(PATTERN)) => {
                self.asking = Asking::Pattern;
                self.ask()
            }
            Some(Answer::Option(SCALE)) => {
                self.asking = Asking::Scale;
                self.ask()
            }
            Some(Answer::Option(ROTATION)) => {
                self.asking = Asking::Rotation;
                self.ask()
            }
            Some(Answer::Option(BOUNDARY)) => {
                self.boundary = !self.boundary;
                self.ask()
            }
            Some(Answer::Path(name)) if self.asking == Asking::Pattern => {
                let found = doc
                    .state()
                    .patterns()
                    .into_iter()
                    .find(|p| p.name.eq_ignore_ascii_case(name.trim()));
                match found {
                    Some(pattern) => {
                        self.pattern = pattern.name;
                        self.resume()
                    }
                    None => Step::Ask(Request {
                        options: vec![PATTERN, SCALE, ROTATION, BOUNDARY],
                        ..Request::new("prompt.hatch.no_pattern", Kind::Path)
                    }),
                }
            }
            Some(Answer::Number(scale)) if self.asking == Asking::Scale => {
                if scale > 0.0 {
                    self.scale = scale;
                }
                self.resume()
            }
            Some(Answer::Number(degrees)) if self.asking == Asking::Rotation => {
                self.rotation = degrees.to_radians();
                self.resume()
            }
            Some(Answer::Objects(ids)) if self.asking == Asking::Curves => self.chosen(doc, &ids),
            Some(Answer::Point(p)) if self.asking == Asking::Inside => {
                self.picks.push(p);
                self.ask()
            }
            Some(Answer::Empty) if self.asking == Asking::Inside && !self.picks.is_empty() => {
                let Some((x, y, _)) = self
                    .curves
                    .first()
                    .and_then(|c| plane_of(c, doc.work_plane()[0]))
                else {
                    return Step::Failed(Message::new("error.hatch_nothing"));
                };
                match regions(&self.curves, &self.picks, [x, y], true) {
                    Ok(found) => self.make(doc, found),
                    Err(message) => Step::Failed(message),
                }
            }
            Some(_) => unexpected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Processor;
    use crate::commands::run;
    use curva_document::annotation::{Annotation, Hatch};
    use curva_document::{Document, Shape};

    fn hatches(doc: &Document) -> Vec<Hatch> {
        doc.state()
            .objects
            .iter()
            .filter_map(|o| match &o.shape {
                Shape::Annotation(Annotation::Hatch(h)) => Some(h.clone()),
                _ => None,
            })
            .collect()
    }

    fn choose_all(doc: &mut Document, processor: &mut Processor) {
        let ids: Vec<u64> = doc.state().objects.iter().map(|o| o.id).collect();
        doc.select(&ids);
        processor.submit(doc, "");
    }

    #[test]
    fn a_curve_inside_another_is_a_hole() {
        let mut doc = Document::default();
        run(&mut doc, &["Rectangle", "0,0", "40,30"]);
        run(&mut doc, &["Circle", "20,15", "5"]);
        run(&mut doc, &["Rectangle", "100,0", "110,10"]);
        let mut processor = run(&mut doc, &["Hatch", "Pattern", "ANSI31", "Scale", "2"]);
        choose_all(&mut doc, &mut processor);
        let made = hatches(&doc);
        assert_eq!(made.len(), 2);
        let with_hole = made.iter().find(|h| h.loops.len() == 2).unwrap();
        assert_eq!(with_hole.pattern, "ANSI31");
        assert!((with_hole.scale - 2.0).abs() < 1e-12);
        // Its lines (after its two outlines) stay out of the hole.
        let drawing = curva_document::shape::annotation_drawing(
            &Annotation::Hatch(with_hole.clone()),
            doc.state(),
            None,
        );
        assert!(drawing.lines.len() > 10);
        for line in &drawing.lines[2..] {
            let points = line.sample().unwrap();
            let (a, b) = (points[0], points[points.len() - 1]);
            let middle = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            let gap = (middle[0] - 20.0).hypot(middle[1] - 15.0);
            assert!(gap > 4.9, "a line through the hole at {middle:?}");
        }
    }

    #[test]
    fn a_solid_hatch_fills_its_outline() {
        let mut doc = Document::default();
        run(&mut doc, &["Rectangle", "0,0", "10,10"]);
        let mut processor = run(&mut doc, &["Hatch", "Pattern", "solid"]);
        choose_all(&mut doc, &mut processor);
        let made = hatches(&doc);
        assert_eq!(made[0].pattern, "Solid");
        let drawing = curva_document::shape::annotation_drawing(
            &Annotation::Hatch(made[0].clone()),
            doc.state(),
            None,
        );
        assert_eq!(drawing.fills.len(), 1);
    }

    #[test]
    fn boundary_hatches_the_region_clicked_inside() {
        let mut doc = Document::default();
        run(&mut doc, &["Rectangle", "0,0", "20,20"]);
        run(&mut doc, &["Rectangle", "10,10", "30,30"]);
        let mut processor = run(&mut doc, &["Hatch", "Boundary"]);
        choose_all(&mut doc, &mut processor);
        processor.submit(&mut doc, "15,15");
        processor.submit(&mut doc, "");
        let made = hatches(&doc);
        assert_eq!(made.len(), 1);
        // The overlap only: a square 10 across.
        let (low, high) = Shape::Annotation(Annotation::Hatch(made[0].clone()))
            .bounds()
            .unwrap();
        assert!((low[0] - 10.0).abs() < 1e-6 && (high[0] - 20.0).abs() < 1e-6);
    }

    #[test]
    fn an_unknown_pattern_is_asked_again() {
        let mut doc = Document::default();
        let processor = run(&mut doc, &["Hatch", "Pattern", "Nonesuch"]);
        assert_eq!(
            processor.current_request().map(|r| r.prompt.key),
            Some("prompt.hatch.no_pattern")
        );
    }
}
