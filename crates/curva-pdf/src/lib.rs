//! Drawings to PDF (spec `drawing-output.md`): each [`Page`] becomes a PDF page of its size, its
//! lines strokes at their print widths and its shapes fills, all as vectors, written with
//! `krilla`. Paper millimetres become PDF points (1/72 inch), and the page's upward Y becomes
//! PDF's downward one.

pub mod format;
mod svg;
pub use svg::write_svg;

use curva_document::drawing::{Colour, Mark, Page};
use krilla::Document;
use krilla::color::rgb;
use krilla::geom::{Path, PathBuilder};
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, FillRule, LineCap, LineJoin, Stroke, StrokeDash};

/// PDF points in a millimetre.
const POINTS_PER_MM: f64 = 72.0 / 25.4;

/// The width a hairline prints at, in points: the thinnest line printers draw reliably.
const HAIRLINE: f32 = 0.25;

fn paint(colour: Colour) -> krilla::paint::Paint {
    let [r, g, b] = colour.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
    rgb::Color::new(r, g, b).into()
}

/// A path through points in millimetres on a page `height` millimetres tall.
fn path(contours: &[(&[[f64; 2]], bool)], height: f64) -> Option<Path> {
    let at = |p: [f64; 2]| {
        (
            (p[0] * POINTS_PER_MM) as f32,
            ((height - p[1]) * POINTS_PER_MM) as f32,
        )
    };
    let mut builder = PathBuilder::new();
    for (points, closed) in contours {
        let Some((&first, rest)) = points.split_first() else {
            continue;
        };
        let (x, y) = at(first);
        builder.move_to(x, y);
        if rest.is_empty() {
            // A dot: a line of no length, which round caps draw.
            builder.line_to(x, y);
        }
        for p in rest {
            let (x, y) = at(*p);
            builder.line_to(x, y);
        }
        if *closed {
            builder.close();
        }
    }
    builder.finish()
}

/// The PDF of the pages, in order.
///
/// # Errors
///
/// When there are no pages, a page has no size, or the PDF cannot be written.
pub fn write_pdf(pages: &[Page]) -> Result<Vec<u8>, String> {
    if pages.is_empty() {
        return Err("nothing to print".to_owned());
    }
    let mut document = Document::new();
    for page in pages {
        let [width, height] = page.size;
        let settings = PageSettings::from_wh(
            (width * POINTS_PER_MM) as f32,
            (height * POINTS_PER_MM) as f32,
        )
        .ok_or_else(|| format!("page {} has no size", page.name))?;
        let mut sheet = document.start_page_with(settings);
        let mut surface = sheet.surface();
        for mark in &page.marks {
            match mark {
                Mark::Line {
                    points,
                    closed,
                    colour,
                    width,
                    dashes,
                    ..
                } => {
                    let Some(line) = path(&[(points, *closed)], height) else {
                        continue;
                    };
                    let width = if *width > 0.0 {
                        (width * POINTS_PER_MM) as f32
                    } else {
                        HAIRLINE
                    };
                    let dash = (!dashes.is_empty()).then(|| StrokeDash {
                        array: dashes
                            .iter()
                            .map(|d| (d.abs() * POINTS_PER_MM) as f32)
                            .collect(),
                        offset: 0.0,
                    });
                    surface.set_fill(None);
                    surface.set_stroke(Some(Stroke {
                        paint: paint(*colour),
                        width,
                        line_cap: LineCap::Round,
                        line_join: LineJoin::Round,
                        dash,
                        ..Stroke::default()
                    }));
                    surface.draw_path(&line);
                }
                Mark::Fill {
                    contours, colour, ..
                } => {
                    let outlines: Vec<(&[[f64; 2]], bool)> =
                        contours.iter().map(|c| (c.as_slice(), true)).collect();
                    let Some(shape) = path(&outlines, height) else {
                        continue;
                    };
                    surface.set_stroke(None);
                    surface.set_fill(Some(Fill {
                        paint: paint(*colour),
                        opacity: NormalizedF32::ONE,
                        rule: FillRule::EvenOdd,
                    }));
                    surface.draw_path(&shape);
                }
            }
        }
        surface.finish();
        sheet.finish();
    }
    document.finish().map_err(|e| format!("{e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a4() -> Page {
        Page {
            name: "Sheet 1".to_owned(),
            size: [297.0, 210.0],
            marks: vec![
                Mark::Line {
                    points: vec![[10.0, 10.0], [110.0, 10.0]],
                    closed: false,
                    colour: [0.0; 3],
                    width: 0.5,
                    dashes: vec![2.0, 1.0],
                    source: None,
                },
                Mark::Fill {
                    contours: vec![
                        vec![[20.0, 20.0], [60.0, 20.0], [60.0, 60.0], [20.0, 60.0]],
                        vec![[30.0, 30.0], [50.0, 30.0], [50.0, 50.0], [30.0, 50.0]],
                    ],
                    colour: [1.0, 0.0, 0.0],
                    source: None,
                },
            ],
        }
    }

    #[test]
    fn pages_become_a_pdf_of_their_size() {
        let pdf = write_pdf(&[a4(), a4()]).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        let text = String::from_utf8_lossy(&pdf);
        // Two pages, A4 landscape in points.
        assert!(text.contains("/Count 2"), "not two pages");
        assert!(text.contains("841.8"), "not A4 wide");
    }

    /// The same page with every mark naming an object.
    fn with_sources(mut page: Page) -> Page {
        for (n, mark) in page.marks.iter_mut().enumerate() {
            match mark {
                Mark::Line { source, .. } | Mark::Fill { source, .. } => {
                    *source = Some(n as u64 + 7)
                }
            }
        }
        page
    }

    #[test]
    fn the_objects_marks_come_from_do_not_change_the_pdf() {
        assert_eq!(
            write_pdf(&[a4()]).unwrap(),
            write_pdf(&[with_sources(a4())]).unwrap()
        );
        assert_eq!(svg::write_svg(&a4()), svg::write_svg(&with_sources(a4())));
    }

    #[test]
    fn no_pages_is_an_error() {
        assert!(write_pdf(&[]).is_err());
    }
}
