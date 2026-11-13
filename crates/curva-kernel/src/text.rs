//! Text as curves (Rhino's TextObject): each glyph's outline read from a font with `ttf-parser`,
//! as lines and Bézier pieces joined into closed curves.

use crate::{Curve, KernelError, Point, Spline};

/// Curva's own type, Inter, used when no other font is asked for or found.
const INTER: &[u8] = include_bytes!("../../curva-app/fonts/Inter-Regular.ttf");

/// A font file's bytes: a system font by name if one is found, otherwise Inter.
#[must_use]
pub fn font_bytes(name: Option<&str>) -> Vec<u8> {
    let Some(name) = name.filter(|n| !n.trim().is_empty()) else {
        return INTER.to_vec();
    };
    let folders = [
        "C:\\Windows\\Fonts",
        "/usr/share/fonts/truetype",
        "/usr/share/fonts",
        "/Library/Fonts",
        "/System/Library/Fonts",
    ];
    let wanted = name.to_lowercase().replace(' ', "");
    for folder in folders {
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_lowercase().replace(' ', ""))
                .unwrap_or_default();
            let font = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("ttf") || e.eq_ignore_ascii_case("otf"));
            if font
                && stem == wanted
                && let Ok(bytes) = std::fs::read(&path)
            {
                return bytes;
            }
        }
    }
    INTER.to_vec()
}

/// One glyph contour being traced.
struct Tracer {
    scale: f64,
    shift: f64,
    origin: Point,
    x: Point,
    y: Point,
    start: Option<Point>,
    at: Point,
    pieces: Vec<Curve>,
    contours: Vec<Curve>,
}

impl Tracer {
    fn place(&self, fx: f32, fy: f32) -> Point {
        let (u, v) = (
            (f64::from(fx) + self.shift) * self.scale,
            f64::from(fy) * self.scale,
        );
        std::array::from_fn(|k| self.origin[k] + self.x[k] * u + self.y[k] * v)
    }

    fn bezier(&mut self, points: Vec<Point>) {
        let degree = points.len() - 1;
        let mut knots = vec![0.0; degree + 1];
        knots.extend(std::iter::repeat_n(1.0, degree + 1));
        let end = *points.last().unwrap_or(&self.at);
        if crate::gap(self.at, end) > 1e-9 {
            let spline = Spline {
                degree,
                knots,
                points: points.iter().map(|p| [p[0], p[1], p[2], 1.0]).collect(),
            };
            if let Ok(curve) = spline.to_curve() {
                self.pieces.push(curve);
            }
        }
        self.at = end;
    }
}

impl ttf_parser::OutlineBuilder for Tracer {
    fn move_to(&mut self, x: f32, y: f32) {
        self.at = self.place(x, y);
        self.start = Some(self.at);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let to = self.place(x, y);
        if crate::gap(self.at, to) > 1e-9
            && let Ok(line) = Curve::line(self.at, to)
        {
            self.pieces.push(line);
        }
        self.at = to;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let points = vec![self.at, self.place(x1, y1), self.place(x, y)];
        self.bezier(points);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let points = vec![
            self.at,
            self.place(x1, y1),
            self.place(x2, y2),
            self.place(x, y),
        ];
        self.bezier(points);
    }

    fn close(&mut self) {
        if let Some(start) = self.start.take() {
            self.line_to_point(start);
        }
        let pieces = std::mem::take(&mut self.pieces);
        if !pieces.is_empty()
            && let Ok(contour) = Curve::compound(pieces)
        {
            self.contours.push(contour);
        }
    }
}

impl Tracer {
    fn line_to_point(&mut self, to: Point) {
        if crate::gap(self.at, to) > 1e-9
            && let Ok(line) = Curve::line(self.at, to)
        {
            self.pieces.push(line);
        }
        self.at = to;
    }
}

/// The outlines of `text` set in `font` with capital height about `height`, starting at `origin`
/// along `x`, with `y` up the letters. Each glyph gives its closed contours; the outer ones and
/// their holes come together, in order.
pub fn text_outlines(
    text: &str,
    font: &[u8],
    height: f64,
    origin: Point,
    x: Point,
    y: Point,
) -> Result<Vec<Vec<Curve>>, KernelError> {
    let face = ttf_parser::Face::parse(font, 0)
        .map_err(|e| KernelError(format!("the font could not be read: {e}")))?;
    let capital = face
        .capital_height()
        .map_or(f64::from(face.units_per_em()) * 0.7, f64::from);
    let scale = height / capital.max(1.0);
    let mut advance = 0.0;
    let mut glyphs = Vec::new();
    for ch in text.chars() {
        let Some(glyph) = face.glyph_index(ch) else {
            advance += f64::from(face.units_per_em()) * 0.5;
            continue;
        };
        let mut tracer = Tracer {
            scale,
            shift: advance,
            origin,
            x,
            y,
            start: None,
            at: origin,
            pieces: Vec::new(),
            contours: Vec::new(),
        };
        let _ = face.outline_glyph(glyph, &mut tracer);
        if !tracer.contours.is_empty() {
            glyphs.push(tracer.contours);
        }
        advance += f64::from(face.glyph_hor_advance(glyph).unwrap_or(0));
    }
    Ok(glyphs)
}

#[cfg(test)]
mod tests {
    use super::{font_bytes, text_outlines};

    #[test]
    fn letters_become_closed_curves_of_the_height_asked() {
        let glyphs = text_outlines(
            "Ao",
            &font_bytes(None),
            10.0,
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        )
        .unwrap();
        assert_eq!(glyphs.len(), 2);
        // A has an outer contour and its hole; o has two.
        assert!(
            glyphs.iter().all(|g| g.len() == 2),
            "{:?}",
            glyphs.iter().map(Vec::len).collect::<Vec<_>>()
        );
        let top = glyphs[0]
            .iter()
            .flat_map(|c| c.sample().unwrap())
            .map(|p| p[1])
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((top - 10.0).abs() < 0.5, "{top}");
        assert!(glyphs.iter().flatten().all(|c| c.is_closed().unwrap()));
    }
}
