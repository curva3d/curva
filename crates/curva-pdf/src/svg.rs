//! Drawings to SVG (spec `drawing-output.md` §3): a page as one SVG file of its paper size in
//! millimetres, lines as stroked paths at their print widths and dash patterns, shapes as filled
//! paths (even–odd, so inner outlines are holes). The page's upward Y becomes SVG's downward one.
//! Written directly, as the spec decided: the format is a few kinds of element.

use curva_document::drawing::{Colour, Mark, Page};
use std::fmt::Write as _;

/// The width a hairline is drawn at, in millimetres, as the PDF draws it (a quarter point).
const HAIRLINE: f64 = 0.25 * 25.4 / 72.0;

/// A number with no more digits than a thousandth of a millimetre needs.
fn number(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

fn colour(c: Colour) -> String {
    let [r, g, b] = c.map(|v| {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let byte = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        byte
    });
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Text for XML: the five characters XML reserves, escaped.
fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Path data through points on a page `height` millimetres tall.
fn path_data(contours: &[(&[[f64; 2]], bool)], height: f64) -> String {
    let mut data = String::new();
    for (points, closed) in contours {
        let Some((first, rest)) = points.split_first() else {
            continue;
        };
        let _ = write!(data, "M{} {}", number(first[0]), number(height - first[1]));
        if rest.is_empty() {
            // A dot: a line of no length, which round caps draw.
            let _ = write!(data, "h0");
        }
        for p in rest {
            let _ = write!(data, "L{} {}", number(p[0]), number(height - p[1]));
        }
        if *closed {
            data.push('Z');
        }
    }
    data
}

/// The SVG of a page.
#[must_use]
pub fn write_svg(page: &Page) -> String {
    let [width, height] = page.size;
    let mut svg = String::new();
    let _ = writeln!(svg, r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}mm" height="{h}mm" viewBox="0 0 {w} {h}">"#,
        w = number(width),
        h = number(height)
    );
    let _ = writeln!(svg, "<title>{}</title>", escaped(&page.name));
    let _ = writeln!(
        svg,
        r#"<g fill="none" stroke-linecap="round" stroke-linejoin="round">"#
    );
    for mark in &page.marks {
        match mark {
            Mark::Line {
                points,
                closed,
                colour: c,
                width,
                dashes,
                ..
            } => {
                let data = path_data(&[(points, *closed)], height);
                if data.is_empty() {
                    continue;
                }
                let width = if *width > 0.0 { *width } else { HAIRLINE };
                let dash = if dashes.is_empty() {
                    String::new()
                } else {
                    let list: Vec<String> = dashes.iter().map(|d| number(d.abs())).collect();
                    format!(r#" stroke-dasharray="{}""#, list.join(" "))
                };
                let _ = writeln!(
                    svg,
                    r#"<path d="{data}" stroke="{}" stroke-width="{}"{dash}/>"#,
                    colour(*c),
                    number(width)
                );
            }
            Mark::Fill {
                contours,
                colour: c,
                ..
            } => {
                let outlines: Vec<(&[[f64; 2]], bool)> =
                    contours.iter().map(|c| (c.as_slice(), true)).collect();
                let data = path_data(&outlines, height);
                if data.is_empty() {
                    continue;
                }
                let _ = writeln!(
                    svg,
                    r#"<path d="{data}" fill="{}" fill-rule="evenodd" stroke="none"/>"#,
                    colour(*c)
                );
            }
        }
    }
    let _ = writeln!(svg, "</g>");
    let _ = writeln!(svg, "</svg>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> Page {
        Page {
            name: "Plans & <sections>".to_owned(),
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
                Mark::Line {
                    points: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
                    closed: true,
                    colour: [1.0, 0.0, 0.0],
                    width: 0.0,
                    dashes: Vec::new(),
                    source: None,
                },
                Mark::Fill {
                    contours: vec![
                        vec![[20.0, 20.0], [60.0, 20.0], [60.0, 60.0], [20.0, 60.0]],
                        vec![[30.0, 30.0], [50.0, 30.0], [50.0, 50.0], [30.0, 50.0]],
                    ],
                    colour: [0.0, 0.5, 1.0],
                    source: None,
                },
            ],
        }
    }

    #[test]
    fn a_page_is_an_svg_of_its_paper_size_in_millimetres() {
        let svg = write_svg(&page());
        assert!(svg.contains(r#"width="297mm" height="210mm" viewBox="0 0 297 210""#));
        assert!(svg.contains("<title>Plans &amp; &lt;sections&gt;</title>"));
        assert_eq!(svg.matches("<path ").count(), 3);
    }

    #[test]
    fn lines_keep_their_width_dashes_and_place_with_y_down() {
        let svg = write_svg(&page());
        // 10 mm up from the bottom of a 210 mm sheet is 200 mm down from its top.
        assert!(svg.contains(
            r##"<path d="M10 200L110 200" stroke="#000000" stroke-width="0.5" stroke-dasharray="2 1"/>"##
        ));
        assert!(svg.contains(r##"d="M0 210L1 210L1 209Z" stroke="#ff0000" stroke-width="0.088""##));
    }

    #[test]
    fn fills_have_their_holes() {
        let svg = write_svg(&page());
        assert!(svg.contains(
            r##"d="M20 190L60 190L60 150L20 150ZM30 180L50 180L50 160L30 160Z" fill="#0080ff" fill-rule="evenodd""##
        ));
    }
}
