//! What sheets carry besides drawings (spec `layouts.md` §4): scale bars that follow a detail's
//! scale, and tables (the sheet index and a sheet's revisions) that follow the document. Both
//! are annotation, worked out each time they are drawn, so they never go stale.
//!
//! These are Serpentine3D's drafting commands (MIT); Rhino has none of them. Their placement is
//! a [`Text`]: its point, directions and height place them and size everything, so they move,
//! turn and scale as text does.

use crate::annotation::{Text, TextAlign};
use crate::shape::{AnnotationDrawing, text_glyphs};
use crate::{State, Unit};
use curva_kernel::{Curve, Point};
use serde::{Deserialize, Serialize};

/// A scale bar for a detail view: alternate filled and empty blocks with the model lengths they
/// stand for, and the detail's scale beneath.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScaleBar {
    /// Where its lower left corner is, the way it reads, and its text height. Its words are
    /// not used.
    pub at: Text,
    /// The detail it measures, by its frame's id.
    pub detail: u64,
    /// The model length it stands for, in the model's unit; `None` picks a round length about
    /// 25 text heights long on paper.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<f64>,
    /// How many blocks it is split into; `0` picks four or five to suit the length.
    #[serde(default)]
    pub divisions: u32,
}

/// A table on a sheet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Table {
    /// Its corner, the way it reads, and its text height. A sheet index hangs down from its
    /// top left corner; a revision table grows up from its bottom left corner, as revision
    /// blocks above a title block do. Its words are not used.
    pub at: Text,
    /// What it lists.
    pub kind: TableKind,
    /// The column headings, as placed (in the user's language).
    pub headings: Vec<String>,
    /// A title above (or, for revisions, below) the headings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// How a layout without a name is listed, `{0}` standing for its number, as its tab shows
    /// it (in the user's language).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unnamed: Option<String>,
    /// A width to stretch it to, in the paper's unit, as a revision table matches the title
    /// block under it; otherwise as wide as its words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
}

/// What a [`Table`] lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableKind {
    /// Every layout: its number, its name and its `Title` layout user text.
    SheetIndex,
    /// The revisions of the layout with this id.
    Revisions(u64),
}

/// One row of a sheet's revision table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    /// Its mark, such as `A` or `P02`.
    pub mark: String,
    /// When it was made, as text.
    pub date: String,
    /// What changed.
    pub description: String,
    /// Who made it, by initials.
    pub by: String,
}

/// A rough outline for a placement `wide` by `tall` text heights (negative `tall` hangs down),
/// for picking and extents where the document is not at hand; the drawing is exact.
#[must_use]
pub fn outline(place: &Text, wide: f64, tall: f64) -> Vec<Curve> {
    let h = place.height;
    let corners = [[0.0, 0.0], [wide, 0.0], [wide, tall], [0.0, tall]];
    Curve::polyline(
        corners
            .iter()
            .map(|c| at(place, c[0] * h, c[1] * h))
            .collect(),
        true,
    )
    .into_iter()
    .collect()
}

impl ScaleBar {
    /// The bar placed at `at`. Its typed length is a model length, so it stays; `_stretch`
    /// is how much the placement grew, which its text height already carries.
    #[must_use]
    pub fn with_at(&self, at: Text, _stretch: f64) -> Self {
        Self { at, ..self.clone() }
    }
}

impl Table {
    /// The table placed at `at`, its width grown by `stretch` with it.
    #[must_use]
    pub fn with_at(&self, at: Text, stretch: f64) -> Self {
        Self {
            at,
            width: self.width.map(|w| w * stretch),
            ..self.clone()
        }
    }
}

/// A scale bar or table as plain parts, for files that have no such object: lines, closed shapes
/// to fill, and notes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SheetParts {
    /// Lines, the outlines of filled shapes among them.
    pub lines: Vec<Curve>,
    /// Closed shapes to fill, each with its holes.
    pub fills: Vec<Vec<Curve>>,
    /// Words, as notes.
    pub texts: Vec<Text>,
}

impl SheetParts {
    /// What the parts draw: the notes' letters added as lines and filled shapes.
    #[must_use]
    pub fn drawing(self) -> AnnotationDrawing {
        let mut drawing = AnnotationDrawing {
            lines: self.lines,
            fills: self.fills,
        };
        for text in &self.texts {
            let glyphs = text_glyphs(text);
            drawing.lines.extend(glyphs.iter().flatten().cloned());
            drawing.fills.extend(glyphs);
        }
        drawing
    }
}

/// How many paper units one millimetre is, for the layout an annotation is on.
fn paper_units_per_millimetre(state: &State, space: Option<u64>) -> f64 {
    let unit = space
        .and_then(|id| state.layouts.iter().find(|l| l.id == id))
        .map_or(1.0, |l| l.unit.millimetres());
    1.0 / unit
}

/// A point `u` along and `v` up from a placement's point.
fn at(place: &Text, u: f64, v: f64) -> Point {
    std::array::from_fn(|i| place.origin[i] + place.x[i] * u + place.y[i] * v)
}

/// Text of the placement's look at `u`, `v`, of height `height`.
fn words(place: &Text, content: &str, u: f64, v: f64, height: f64, align: TextAlign) -> Text {
    Text {
        content: content.to_owned(),
        origin: at(place, u, v),
        height,
        align,
        ..place.clone()
    }
}

/// How wide a line of text of the placement's font is.
fn width_of(place: &Text, content: &str, height: f64) -> f64 {
    let text = words(place, content, 0.0, 0.0, height, TextAlign::Left);
    let start = text.origin;
    text_glyphs(&text)
        .iter()
        .flatten()
        .filter_map(|c| c.sample().ok())
        .flatten()
        .map(|p| (0..3).map(|i| (p[i] - start[i]) * place.x[i]).sum::<f64>())
        .fold(0.0_f64, f64::max)
}

/// Adds a note to the parts.
fn add_text(drawing: &mut SheetParts, text: &Text) {
    drawing.texts.push(text.clone());
}

/// Adds a line between two placement coordinates.
fn add_line(drawing: &mut SheetParts, place: &Text, a: [f64; 2], b: [f64; 2]) {
    if let Ok(line) = Curve::line(at(place, a[0], a[1]), at(place, b[0], b[1])) {
        drawing.lines.push(line);
    }
}

/// A number as short as it can be shown: `0.5`, `10`, `2.25`.
fn tidy(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

/// The round length (1, 2 or 5 times a power of ten) nearest below `wanted`, and how many
/// blocks suit it.
fn round_length(wanted: f64) -> (f64, u32) {
    let power = 10_f64.powf(wanted.log10().floor());
    let lead = wanted / power;
    if lead >= 5.0 {
        (5.0 * power, 5)
    } else if lead >= 2.0 {
        (2.0 * power, 4)
    } else {
        (power, 5)
    }
}

/// What a scale bar draws.
#[must_use]
pub fn scale_bar_drawing(bar: &ScaleBar, state: &State, space: Option<u64>) -> AnnotationDrawing {
    scale_bar_parts(bar, state, space).drawing()
}

/// A scale bar as plain parts: its blocks' outlines, the filled ones, and its numbers as notes.
#[must_use]
pub fn scale_bar_parts(bar: &ScaleBar, state: &State, space: Option<u64>) -> SheetParts {
    let mut drawing = SheetParts::default();
    let place = &bar.at;
    let h = place.height;
    let detail = state
        .layouts
        .iter()
        .flat_map(|l| &l.details)
        .find(|d| d.frame == bar.detail);
    let Some(scale) = detail.and_then(|d| d.scale) else {
        add_text(
            &mut drawing,
            &words(place, "####", 0.0, 0.0, h, TextAlign::Left),
        );
        return drawing;
    };
    // Paper length (in the paper's unit) of one model unit.
    let per_unit = state.unit.millimetres() * scale * paper_units_per_millimetre(state, space);
    if per_unit <= 0.0 || !per_unit.is_finite() {
        return drawing;
    }
    let (length, picked) = match bar.length {
        Some(length) if length > 0.0 => (length, 4),
        _ => round_length(25.0 * h / per_unit),
    };
    let blocks = if bar.divisions == 0 {
        picked
    } else {
        bar.divisions
    };
    let paper = length * per_unit;
    let thick = 0.8 * h;
    // Millimetre models of buildings read better in metres once the bar reaches one.
    let (shown_unit, divide) = match state.unit {
        Unit::Millimetres if length >= 1000.0 => ("m", 1000.0),
        unit => (unit.symbol(), 1.0),
    };
    for block in 0..blocks {
        let u0 = paper * f64::from(block) / f64::from(blocks);
        let u1 = paper * f64::from(block + 1) / f64::from(blocks);
        let corners = [[u0, 0.0], [u1, 0.0], [u1, thick], [u0, thick]];
        if let Ok(outline) = Curve::polyline(
            corners.iter().map(|c| at(place, c[0], c[1])).collect(),
            true,
        ) {
            if block % 2 == 0 {
                drawing.fills.push(vec![outline.clone()]);
            }
            drawing.lines.push(outline);
        }
    }
    let label_height = 0.8 * h;
    for tick in 0..=blocks {
        let value = length * f64::from(tick) / f64::from(blocks) / divide;
        let mut label = tidy(value);
        if tick == blocks {
            label = format!("{label} {shown_unit}");
        }
        let u = paper * f64::from(tick) / f64::from(blocks);
        let align = if tick == blocks && tick > 0 {
            TextAlign::Left
        } else {
            TextAlign::Centre
        };
        let shift = if align == TextAlign::Left {
            -width_of(place, &tidy(value), label_height) / 2.0
        } else {
            0.0
        };
        add_text(
            &mut drawing,
            &words(
                place,
                &label,
                u + shift,
                thick + 0.5 * h,
                label_height,
                align,
            ),
        );
    }
    let format = if space
        .and_then(|id| state.layouts.iter().find(|l| l.id == id))
        .is_some_and(|l| l.unit == crate::layouts::PaperUnit::Inches)
    {
        "#=1"
    } else {
        "1:#"
    };
    add_text(
        &mut drawing,
        &words(
            place,
            &crate::fields::scale_in_format(scale, format),
            paper / 2.0,
            -1.5 * h,
            h,
            TextAlign::Centre,
        ),
    );
    drawing
}

/// The rows a table lists now, without its headings.
#[must_use]
pub fn table_rows(table: &Table, state: &State) -> Vec<Vec<String>> {
    match table.kind {
        TableKind::SheetIndex => state
            .layouts
            .iter()
            .enumerate()
            .map(|(index, layout)| {
                vec![
                    (index + 1).to_string(),
                    if layout.name.is_empty() {
                        table.unnamed.as_deref().map_or_else(
                            || (index + 1).to_string(),
                            |pattern| pattern.replace("{0}", &(index + 1).to_string()),
                        )
                    } else {
                        layout.name.clone()
                    },
                    layout.user_text.get("Title").cloned().unwrap_or_default(),
                ]
            })
            .collect(),
        TableKind::Revisions(id) => state
            .layouts
            .iter()
            .find(|l| l.id == id)
            .map(|l| {
                l.revisions
                    .iter()
                    .map(|r| {
                        vec![
                            r.mark.clone(),
                            r.date.clone(),
                            r.description.clone(),
                            r.by.clone(),
                        ]
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// What a table draws: a grid of cells with their words, the headings in their own row.
#[must_use]
pub fn table_drawing(table: &Table, state: &State) -> AnnotationDrawing {
    table_parts(table, state).drawing()
}

/// A table as plain parts: its grid lines and its words as notes.
#[must_use]
pub fn table_parts(table: &Table, state: &State) -> SheetParts {
    let mut drawing = SheetParts::default();
    let place = &table.at;
    let h = place.height;
    let pad = 0.6 * h;
    let row_height = 2.0 * h;
    let rows = table_rows(table, state);
    let columns = table
        .headings
        .len()
        .max(rows.iter().map(Vec::len).max().unwrap_or(0));
    if columns == 0 {
        return drawing;
    }
    // Each column as wide as its widest words, and at least four text heights.
    let mut widths: Vec<f64> = (0..columns)
        .map(|c| {
            table
                .headings
                .get(c)
                .into_iter()
                .chain(rows.iter().filter_map(|r| r.get(c)))
                .map(|w| width_of(place, w, h))
                .fold(4.0 * h, f64::max)
                + 2.0 * pad
        })
        .collect();
    let natural: f64 = widths.iter().sum();
    if let Some(wanted) = table.width.filter(|w| *w > natural) {
        // The widest column takes the extra, as a description column would.
        let widest = (0..columns)
            .max_by(|a, b| widths[*a].total_cmp(&widths[*b]))
            .unwrap_or(0);
        widths[widest] += wanted - natural;
    }
    let total: f64 = widths.iter().sum();
    // Rows in drawing order from the anchor: index 0 touches it.
    let mut lines: Vec<Vec<String>> = Vec::new();
    // A revision table grows upwards, so its newest row is furthest from the title block.
    let grows_up = matches!(table.kind, TableKind::Revisions(_));
    let title_row = table.title.as_ref().map(|title| {
        lines.push(vec![title.clone()]);
        0
    });
    lines.push(table.headings.clone());
    lines.extend(rows);
    let count = lines.len();
    #[allow(clippy::cast_precision_loss)]
    let height = count as f64 * row_height;
    // The v of a row's bottom edge.
    #[allow(clippy::cast_precision_loss)]
    let bottom = |row: usize| {
        if grows_up {
            row as f64 * row_height
        } else {
            -((row + 1) as f64) * row_height
        }
    };
    let (low, high) = if grows_up {
        (0.0, height)
    } else {
        (-height, 0.0)
    };
    for edge in 0..=count {
        #[allow(clippy::cast_precision_loss)]
        let v = if grows_up {
            edge as f64 * row_height
        } else {
            -(edge as f64) * row_height
        };
        add_line(&mut drawing, place, [0.0, v], [total, v]);
    }
    // The outer sides run the full height; lines between columns stop at the title's row,
    // which spans the table (it is always the row touching the anchor).
    add_line(&mut drawing, place, [0.0, low], [0.0, high]);
    add_line(&mut drawing, place, [total, low], [total, high]);
    let (inner_low, inner_high) = match (title_row.is_some(), grows_up) {
        (false, _) => (low, high),
        (true, true) => (low + row_height, high),
        (true, false) => (low, high - row_height),
    };
    let mut u = 0.0;
    for width in &widths[..columns - 1] {
        u += width;
        add_line(&mut drawing, place, [u, inner_low], [u, inner_high]);
    }
    for (row, cells) in lines.iter().enumerate() {
        let baseline = bottom(row) + (row_height - h) / 2.0;
        if Some(row) == title_row {
            add_text(
                &mut drawing,
                &words(
                    place,
                    &cells[0],
                    total / 2.0,
                    baseline,
                    h,
                    TextAlign::Centre,
                ),
            );
            continue;
        }
        let mut left = 0.0;
        for (column, cell) in cells.iter().enumerate().take(columns) {
            if !cell.is_empty() {
                add_text(
                    &mut drawing,
                    &words(place, cell, left + pad, baseline, h, TextAlign::Left),
                );
            }
            left += widths[column];
        }
    }
    drawing
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;
    use crate::layouts::paper_size;

    fn place(height: f64) -> Text {
        Text {
            content: String::new(),
            origin: [10.0, 20.0, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            height,
            font: None,
            align: TextAlign::Left,
            style: 0,
        }
    }

    /// The extent along x of everything drawn.
    fn span(drawing: &AnnotationDrawing) -> (f64, f64) {
        drawing
            .lines
            .iter()
            .filter_map(|c| c.sample().ok())
            .flatten()
            .fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                (lo.min(p[0]), hi.max(p[0]))
            })
    }

    #[test]
    fn round_lengths_are_one_two_or_five_of_a_power_of_ten() {
        assert_eq!(round_length(3.2), (2.0, 4));
        assert_eq!(round_length(0.07), (0.05, 5));
        assert_eq!(round_length(1250.0), (1000.0, 5));
    }

    #[test]
    fn a_scale_bar_follows_its_details_scale() {
        let mut doc = Document::default();
        let sheet = doc.add_layout("Plan".to_owned(), paper_size("A3").unwrap(), true);
        let frame = 7;
        doc.add_detail(
            sheet,
            crate::layouts::Detail {
                frame,
                projection: "Top".to_owned(),
                target: [0.0; 3],
                distance: 100.0,
                scale: Some(1.0 / 50.0),
                display: "Wireframe".to_owned(),
                locked: false,
                hidden_layers: Vec::new(),
            },
        );
        let bar = ScaleBar {
            at: place(2.5),
            detail: frame,
            length: None,
            divisions: 0,
        };
        // The bar's blocks are 2 mm tall (0.8 text heights) from its point; labels sit above
        // them and the scale below.
        let bar_end = |drawing: &AnnotationDrawing| {
            drawing
                .lines
                .iter()
                .filter_map(|c| c.sample().ok())
                .filter(|p| p.iter().all(|q| (19.99..=22.01).contains(&q[1])))
                .flatten()
                .map(|q| q[0])
                .fold(f64::MIN, f64::max)
        };
        // 25 text heights is 62.5 mm on paper, 3.125 m at 1:50: a 2 m bar, 40 mm long.
        let drawing = scale_bar_drawing(&bar, doc.state(), Some(sheet));
        let (lo, _) = span(&drawing);
        assert!((lo - 10.0).abs() < 1.0, "starts at its point: {lo}");
        assert!(drawing.fills.len() >= 2 + 3, "two filled blocks, and words");
        let end = bar_end(&drawing);
        assert!((end - 50.0).abs() < 1e-6, "2 m at 1:50 is 40 mm: {end}");
        doc.change_detail(frame, |d| d.scale = Some(1.0 / 100.0));
        let drawing = scale_bar_drawing(&bar, doc.state(), Some(sheet));
        let end = bar_end(&drawing);
        // 62.5 mm is 6.25 m at 1:100: a 5 m bar, 50 mm long.
        assert!((end - 60.0).abs() < 1e-6, "5 m at 1:100 is 50 mm: {end}");
        let fixed = ScaleBar {
            length: Some(10_000.0),
            divisions: 2,
            ..bar
        };
        let end = bar_end(&scale_bar_drawing(&fixed, doc.state(), Some(sheet)));
        assert!(
            (end - 110.0).abs() < 1e-6,
            "a typed 10 m at 1:100 is 100 mm: {end}"
        );
    }

    #[test]
    fn a_scale_bar_without_a_scaled_detail_shows_hashes() {
        let doc = Document::default();
        let bar = ScaleBar {
            at: place(2.5),
            detail: 99,
            length: None,
            divisions: 0,
        };
        let drawing = scale_bar_drawing(&bar, doc.state(), None);
        assert!(!drawing.fills.is_empty(), "#### is drawn");
        assert!(
            drawing
                .lines
                .iter()
                .all(|c| !matches!(c, Curve::Line { .. })),
            "no bar"
        );
    }

    #[test]
    fn the_sheet_index_follows_the_layouts() {
        let mut doc = Document::default();
        let paper = paper_size("A3").unwrap();
        let first = doc.add_layout("Plans".to_owned(), paper, true);
        doc.add_layout("Sections".to_owned(), paper, true);
        doc.set_layout_text(first, "Title", "Ground floor");
        let table = Table {
            at: place(2.5),
            kind: TableKind::SheetIndex,
            headings: vec!["No.".into(), "Sheet".into(), "Title".into()],
            title: Some("Sheet index".into()),
            width: None,
            unnamed: None,
        };
        assert_eq!(
            table_rows(&table, doc.state()),
            vec![
                vec!["1".to_owned(), "Plans".into(), "Ground floor".into()],
                vec!["2".to_owned(), "Sections".into(), String::new()],
            ]
        );
        let before = table_drawing(&table, doc.state());
        doc.rename_layout(first, "General arrangement".to_owned());
        assert_eq!(table_rows(&table, doc.state())[0][1], "General arrangement");
        let after = table_drawing(&table, doc.state());
        assert!(
            span(&after).1 > span(&before).1,
            "wider for the longer name"
        );
    }

    #[test]
    fn a_revision_table_stretches_to_its_width_and_grows_upwards() {
        let mut doc = Document::default();
        let sheet = doc.add_layout("Plans".to_owned(), paper_size("A3").unwrap(), true);
        let table = Table {
            at: place(2.5),
            kind: TableKind::Revisions(sheet),
            headings: vec![
                "Rev".into(),
                "Date".into(),
                "Description".into(),
                "By".into(),
            ],
            title: None,
            width: Some(180.0),
            unnamed: None,
        };
        let empty = table_drawing(&table, doc.state());
        let (lo, hi) = span(&empty);
        assert!(
            (lo - 10.0).abs() < 1e-6 && (hi - 190.0).abs() < 1e-6,
            "{lo} {hi}"
        );
        doc.add_revision(
            sheet,
            Revision {
                mark: "A".into(),
                date: "2026-11-15".into(),
                description: "Issued for planning".into(),
                by: "LV".into(),
            },
        );
        let top = |d: &AnnotationDrawing| {
            d.lines
                .iter()
                .filter_map(|c| c.sample().ok())
                .flatten()
                .map(|p| p[1])
                .fold(f64::MIN, f64::max)
        };
        let one = table_drawing(&table, doc.state());
        assert!((top(&one) - (20.0 + 10.0)).abs() < 1e-6, "two rows of 5 mm");
        assert!(top(&empty) < top(&one));
    }
}
