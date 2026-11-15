//! A drawing on paper, ready to print or export (spec `drawing-output.md` §2): pages of lines and
//! filled shapes in millimetres, measured from the sheet's lower left corner. The viewport makes
//! pages from layouts and views; writers such as `curva-pdf` turn them into files.

/// A colour, red, green and blue from 0 to 1.
pub type Colour = [f32; 3];

/// One mark on paper.
#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    /// A line through points.
    Line {
        /// Its points, in millimetres.
        points: Vec<[f64; 2]>,
        /// Whether it runs back to its first point.
        closed: bool,
        /// Its colour.
        colour: Colour,
        /// Its width in millimetres; zero is a hairline, the thinnest the device draws.
        width: f64,
        /// Dash and gap lengths in millimetres, alternating; solid when empty.
        dashes: Vec<f64>,
    },
    /// A filled shape: closed outlines, inside even–odd so inner outlines are holes.
    Fill {
        /// The outlines, in millimetres.
        contours: Vec<Vec<[f64; 2]>>,
        /// Its colour.
        colour: Colour,
    },
}

/// A sheet of paper and what is on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    /// Its name, such as the layout's.
    pub name: String,
    /// Its width and height in millimetres.
    pub size: [f64; 2],
    /// Its marks, in drawing order.
    pub marks: Vec<Mark>,
}

/// The piece of a line from `a` to `b` inside the box `[low, high]`, if any (Liang and Barsky's
/// clipping).
#[must_use]
pub fn clip_segment(
    a: [f64; 2],
    b: [f64; 2],
    [low, high]: [[f64; 2]; 2],
) -> Option<([f64; 2], [f64; 2])> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let (mut from, mut to) = (0.0_f64, 1.0_f64);
    for axis in 0..2 {
        for (p, q) in [
            (-d[axis], a[axis] - low[axis]),
            (d[axis], high[axis] - a[axis]),
        ] {
            if p == 0.0 {
                if q < 0.0 {
                    return None;
                }
            } else {
                let t = q / p;
                if p < 0.0 {
                    from = from.max(t);
                } else {
                    to = to.min(t);
                }
            }
        }
    }
    (from <= to).then(|| {
        let at = |t: f64| [a[0] + d[0] * t, a[1] + d[1] * t];
        (at(from), at(to))
    })
}

/// The runs of a polyline inside a box: each a list of points, broken where it leaves the box.
#[must_use]
pub fn clip_polyline(points: &[[f64; 2]], frame: [[f64; 2]; 2]) -> Vec<Vec<[f64; 2]>> {
    let mut runs: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut current: Vec<[f64; 2]> = Vec::new();
    for pair in points.windows(2) {
        match clip_segment(pair[0], pair[1], frame) {
            Some((a, b)) => {
                if current.last() != Some(&a) {
                    if current.len() > 1 {
                        runs.push(std::mem::take(&mut current));
                    }
                    current = vec![a];
                }
                current.push(b);
                // Leaving the box ends the run.
                if b != pair[1] {
                    runs.push(std::mem::take(&mut current));
                }
            }
            None => {
                if current.len() > 1 {
                    runs.push(std::mem::take(&mut current));
                }
                current.clear();
            }
        }
    }
    if current.len() > 1 {
        runs.push(current);
    }
    runs
}

/// A closed outline cut to a box (Sutherland and Hodgman's clipping); empty when it lies outside.
#[must_use]
pub fn clip_polygon(points: &[[f64; 2]], [low, high]: [[f64; 2]; 2]) -> Vec<[f64; 2]> {
    let mut out = points.to_vec();
    // Each side of the box: which axis, the limit, and whether inside is above it.
    for (axis, limit, above) in [
        (0, low[0], true),
        (0, high[0], false),
        (1, low[1], true),
        (1, high[1], false),
    ] {
        let inside = |p: &[f64; 2]| {
            if above {
                p[axis] >= limit
            } else {
                p[axis] <= limit
            }
        };
        let input = std::mem::take(&mut out);
        for (i, p) in input.iter().enumerate() {
            let q = input[(i + 1) % input.len()];
            let cross = |a: [f64; 2], b: [f64; 2]| {
                let t = (limit - a[axis]) / (b[axis] - a[axis]);
                [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
            };
            match (inside(p), inside(&q)) {
                (true, true) => out.push(q),
                (true, false) => out.push(cross(*p, q)),
                (false, true) => {
                    out.push(cross(*p, q));
                    out.push(q);
                }
                (false, false) => {}
            }
        }
        if out.is_empty() {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOX: [[f64; 2]; 2] = [[0.0, 0.0], [10.0, 10.0]];

    #[test]
    fn a_line_is_cut_where_it_leaves_the_box() {
        assert_eq!(
            clip_segment([-5.0, 5.0], [15.0, 5.0], BOX),
            Some(([0.0, 5.0], [10.0, 5.0]))
        );
        assert_eq!(clip_segment([-5.0, -5.0], [-1.0, 20.0], BOX), None);
        assert_eq!(
            clip_segment([2.0, 2.0], [3.0, 3.0], BOX),
            Some(([2.0, 2.0], [3.0, 3.0]))
        );
    }

    #[test]
    fn a_polyline_leaving_and_coming_back_makes_two_runs() {
        let runs = clip_polyline(&[[2.0, 5.0], [15.0, 5.0], [15.0, 8.0], [2.0, 8.0]], BOX);
        assert_eq!(
            runs,
            vec![vec![[2.0, 5.0], [10.0, 5.0]], vec![[10.0, 8.0], [2.0, 8.0]]]
        );
    }

    #[test]
    fn a_polygon_is_cut_to_the_box() {
        let cut = clip_polygon(&[[5.0, 5.0], [15.0, 5.0], [15.0, 15.0], [5.0, 15.0]], BOX);
        let area: f64 = (0..cut.len())
            .map(|i| {
                let (a, b) = (cut[i], cut[(i + 1) % cut.len()]);
                a[0] * b[1] - b[0] * a[1]
            })
            .sum::<f64>()
            / 2.0;
        assert!((area.abs() - 25.0).abs() < 1e-9);
        assert!(clip_polygon(&[[20.0, 20.0], [30.0, 20.0], [30.0, 30.0]], BOX).is_empty());
    }
}
