//! Hatch patterns, annotation styles and annotation as Rhino reads them.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A hatch pattern as Rhino 6 and later write it (`ON_HatchPattern::Write`): its id, index and
/// name, fill type, description, then its lines, each in a chunk of its own.
pub(super) fn write_hatch_pattern(w: &mut Writer, pattern: &HatchPattern, index: usize) {
    w.begin_anonymous(0);
    w.begin_anonymous(0);
    w.u32(0x01 | 0x04 | 0x08); // id, index and name follow
    w.uuid(pattern.id);
    w.count(index);
    w.wstring(&pattern.name);
    w.end();
    w.u32(u32::from(!pattern.solid));
    w.wstring(&pattern.description);
    w.begin(tcode::ANONYMOUS_CHUNK);
    let lines: &[HatchLine] = if pattern.solid { &[] } else { &pattern.lines };
    w.count(lines.len());
    for line in lines {
        w.begin_anonymous(0);
        w.f64(line.angle);
        w.f64s(&line.base);
        w.f64s(&line.offset);
        w.count(line.dashes.len());
        w.f64s(&line.dashes);
        w.end();
    }
    w.end();
    w.end();
}

/// A length as openNURBS writes one (`ON_LengthValue::Write`): a plain number, no unit.
fn write_length_value(w: &mut Writer, value: f64) {
    w.begin_anonymous(1);
    w.f64(value);
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0); // ON_UnitSystem: none
    w.u32(0);
    w.f64(1.0);
    w.wstring("");
    w.end();
    w.u32(0); // angle unit: none
    w.i32(0); // locale
    w.wstring("1");
    w.u32(0); // exact decimal
    w.end();
}

/// An annotation style as Rhino 6 and later read it (`ON_DimStyle::Read`), written to version
/// 1.1: everything up to the notes' alignment and the style's scale, with Rhino's usual values
/// for what Curva does not set; Rhino fills in the fields of later versions with its defaults.
pub(super) fn write_dim_style(w: &mut Writer, style: &DimStyle, index: usize) {
    write_style_fields(w, style, Some(index), None);
}

/// The fields a note overrides in its style, by openNURBS' `ON_DimStyle::field` numbers: its
/// text height and its horizontal alignment.
const NOTE_OVERRIDES: [usize; 2] = [9, 94];

/// How many fields an annotation style has (`ON_DimStyle::field::Count`).
const STYLE_FIELDS: usize = 115;

/// An annotation style, or (with its parent's id) the style a note sets for itself over the one
/// it names: then with no id, name or index, as Rhino requires, and the note's fields marked as
/// overridden.
fn write_style_fields(
    w: &mut Writer,
    style: &DimStyle,
    index: Option<usize>,
    parent: Option<Uuid>,
) {
    // Version 1.2 adds the font; without one, 1.1 leaves Rhino its default.
    w.begin_versioned(
        tcode::ANONYMOUS_CHUNK,
        1,
        if style.font.is_some() { 2 } else { 1 },
    );
    write_style_head(w, style, index);
    write_style_sizes(w, style, parent);
    write_style_looks(w, style);
    w.u32(0); // unit system: none (the model's)
    if let Some(font) = &style.font {
        write_font(w, font);
    }
    w.end();
}

/// A style's component attributes: its id, index and name, or none for a note's own style.
fn write_style_head(w: &mut Writer, style: &DimStyle, index: Option<usize>) {
    w.begin_versioned(tcode::MODEL_ATTRIBUTES_CHUNK, 1, 0);
    w.u8(0); // model serial number
    if let Some(index) = index {
        w.u8(1);
        w.uuid(style.id);
        w.u8(0); // component type
        w.u8(1);
        w.count(index);
        w.u8(1);
        w.wstring(&style.name);
    } else {
        w.u8(2); // no id
        w.u8(0); // component type
        w.u8(2); // no index
        w.u8(2); // no name
    }
    w.end();
}

/// A style's sizes, formats and units, and, for a note's own style, its parent and the fields
/// it overrides.
fn write_style_sizes(w: &mut Writer, style: &DimStyle, parent: Option<Uuid>) {
    let h = style.text_height;
    // Extension line extension and offset, arrow and leader arrow sizes, centre mark, text gap,
    // text height.
    w.f64s(&[
        style.extension_extension,
        style.extension_offset,
        style.arrow_size,
        style.arrow_size,
        h,
        style.text_gap,
        h,
    ]);
    w.i32(0); // text alignment (obsolete)
    w.i32(0); // angle format: decimal degrees
    w.i32(0); // length format: decimal
    w.i32(0); // angle resolution
    w.i32(style.length_resolution);
    w.i32(-2_147_483_647); // ignored text style index
    w.f64(1.0); // length factor
    w.bool(false); // alternate units
    w.f64(1.0);
    w.i32(0);
    w.i32(2);
    for _ in 0..4 {
        w.wstring(""); // prefixes and suffixes
    }
    w.f64(0.0); // dimension line extension
    w.bool(false);
    w.bool(false);
    if let Some(parent) = parent {
        w.uuid(parent);
        w.count(NOTE_OVERRIDES.len());
        w.bool(true);
        w.count(STYLE_FIELDS);
        for field in 0..STYLE_FIELDS {
            w.bool(NOTE_OVERRIDES.contains(&field));
        }
    } else {
        w.uuid(Uuid::NIL); // no parent style
        w.i32(0);
        w.bool(false); // no overridden fields
    }
}

/// The rest of a style: tolerances, mask, colours, arrows, leaders, the notes' alignment and
/// the style's scale.
fn write_style_looks(w: &mut Writer, style: &DimStyle) {
    let h = style.text_height;
    w.i32(0); // tolerance format
    w.i32(4);
    w.f64s(&[0.0, 0.0, 1.0, 3.0 * h]);
    w.bool(false); // no text mask
    w.i32(0);
    w.color([255, 255, 255, 0]);
    w.f64(1.0); // dimension scale
    w.i32(0);
    w.uuid(Uuid::NIL);
    for _ in 0..2 {
        for _ in 0..4 {
            w.u8(0); // colour sources: by layer
        }
        for _ in 0..4 {
            w.color([0, 0, 0, 0]);
        }
    }
    w.u8(0); // plot weight sources
    w.u8(0);
    w.f64s(&[0.0, 0.0]);
    w.f64(1.0); // fixed extension length
    w.bool(false);
    w.f64(0.0); // text rotation
    w.i32(4);
    w.f64(1.0);
    w.bool(false);
    w.bool(false);
    w.i32(0);
    w.i32(0);
    w.f64(0.7); // stack height
    w.i32(0);
    w.f64s(&[0.0, 0.0, 0.0]); // roundings
    for _ in 0..4 {
        w.i32(0); // zero suppression
    }
    w.bool(false);
    w.i32(1); // arrows: solid triangles
    w.i32(1);
    w.i32(1);
    for _ in 0..3 {
        w.uuid(Uuid::NIL);
    }
    // Version 1.1: leaders, then the notes' alignment in Rhino 5's numbering.
    w.i32(1);
    w.u32(style.v_align);
    w.u32(style.v_align);
    w.i32(0);
    w.i32(1);
    w.f64(0.0);
    w.bool(true);
    w.f64(h);
    w.u32(style.h_align);
    w.u32(style.h_align);
    w.bool(true); // draw forward
    w.bool(false);
    // The style's scale, one to one (`ON_ScaleValue::Write`).
    w.begin_anonymous(1);
    w.f64(1.0);
    w.f64(1.0);
    w.i32(0);
    w.u32(0);
    w.u32(0);
    w.wstring("1:1");
    write_length_value(w, 1.0);
    write_length_value(w, 1.0);
    w.u32(1); // ratio format
    w.end();
}

/// A font as openNURBS writes one (`ON_Font::Write`, version 1.0, the least Rhino reads): its
/// characteristics (normal weight, upright, medium stretch, as `FontCharacteristicsAsUnsigned`
/// makes 429), its Windows name as a UTF-8 string chunk, and no PostScript name. Rhino finds
/// the installed font by the name.
fn write_font(w: &mut Writer, family: &str) {
    w.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, 0);
    w.u32(429);
    w.begin(tcode::UTF8_STRING_CHUNK);
    w.u8(1); // uncompressed
    for byte in family.as_bytes() {
        w.u8(*byte);
    }
    w.end();
    w.wstring("");
    w.end();
}

/// The part every annotation shares, as Rhino 6 and later write it
/// (`ON_Annotation::Internal_WriteAnnotation`, content version 4): its words, style, plane,
/// kind (openNURBS' annotation type), no overrides, the horizontal direction and text scaling.
fn write_annotation_core(
    w: &mut Writer,
    rich: &str,
    rotation: f64,
    style: Uuid,
    plane: &Plane,
    kind: u32,
) {
    write_annotation_core_over(w, rich, rotation, style, plane, kind, None);
}

/// [`write_annotation_core`] with the style an annotation sets for itself over the one it names.
fn write_annotation_core_over(
    w: &mut Writer,
    rich: &str,
    rotation: f64,
    style: Uuid,
    plane: &Plane,
    kind: u32,
    own: Option<&DimStyle>,
) {
    w.begin_anonymous(4);
    w.begin_anonymous(0);
    w.wstring(&rich.replace('\n', "\r\n"));
    write_plane(
        w,
        &Plane {
            origin: [0.0; 3],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            z: [0.0, 0.0, 1.0],
        },
    );
    w.f64(0.0); // no wrapping width
    w.f64(rotation);
    w.u32(0);
    w.u32(0);
    w.f64(1.0); // obsolete height
    w.bool(false); // not wrapped
    w.end();
    w.uuid(style);
    write_plane(w, plane);
    w.u32(kind);
    w.begin_anonymous(1);
    match own {
        Some(own) => {
            w.bool(true);
            class_object(w, class::DIM_STYLE, |w| {
                write_style_fields(w, own, None, Some(style));
            });
        }
        None => w.bool(false), // no overrides
    }
    w.end();
    w.f64s(&[1.0, 0.0]); // horizontal direction
    w.bool(true); // text scales with the model
    w.end();
}

/// A note (`ON_Text::Write`).
pub(super) fn write_text(w: &mut Writer, t: &Text) {
    w.begin_anonymous(0);
    write_annotation_core_over(w, &t.rich, t.rotation, t.style, &t.plane, 9, t.own.as_ref());
    w.end();
}

/// A linear dimension (`ON_DimLinear::Write` around `ON_Dimension::Internal_WriteDimension`,
/// content version 1): the annotation, its typed text, its text at the usual place, arrows as
/// they are, measured in the model, then its second point and dimension line point.
pub(super) fn write_linear_dimension(w: &mut Writer, d: &LinearDimension) {
    w.begin_anonymous(0);
    w.begin_anonymous(1);
    write_annotation_core(w, "", 0.0, d.style, &d.plane, if d.rotated { 5 } else { 1 });
    w.wstring(&d.text);
    w.f64(0.0); // obsolete text rotation
    w.bool(true); // text at its usual place
    w.f64s(&[0.0, 0.0]);
    w.bool(false); // arrows not flipped
    w.bool(false);
    w.u32(0); // arrow fit: automatic
    w.uuid(Uuid::NIL); // not measured through a detail
    w.f64(1.0); // distance scale
    w.u32(0); // text fit: automatic
    w.end();
    w.f64s(&d.second);
    w.f64s(&d.line);
    w.end();
}

/// The part dimensions share after the annotation (`ON_Dimension::Internal_WriteDimension`,
/// content version 1): typed text at its usual place, arrows as they are, measured in the model.
fn write_dimension_core(w: &mut Writer, text: &str) {
    w.wstring(text);
    w.f64(0.0); // obsolete text rotation
    w.bool(true); // text at its usual place
    w.f64s(&[0.0, 0.0]);
    w.bool(false); // arrows not flipped
    w.bool(false);
    w.u32(0); // arrow fit: automatic
    w.uuid(Uuid::NIL); // not measured through a detail
    w.f64(1.0); // distance scale
    w.u32(0); // text fit: automatic
}

/// A radius or diameter dimension (`ON_DimRadial::Write`).
pub(super) fn write_radial_dimension(w: &mut Writer, d: &RadialDimension) {
    w.begin_anonymous(0);
    w.begin_anonymous(1);
    write_annotation_core(
        w,
        "",
        0.0,
        d.style,
        &d.plane,
        if d.diameter { 3 } else { 4 },
    );
    write_dimension_core(w, &d.text);
    w.end();
    w.f64s(&d.radius_point);
    w.f64s(&d.text_point);
    w.end();
}

/// An angle dimension (`ON_DimAngular::Write`).
pub(super) fn write_angular_dimension(w: &mut Writer, d: &AngularDimension) {
    w.begin_anonymous(0);
    w.begin_anonymous(1);
    write_annotation_core(w, "", 0.0, d.style, &d.plane, 2);
    write_dimension_core(w, &d.text);
    w.end();
    w.f64s(&d.sides[0]);
    w.f64s(&d.sides[1]);
    w.f64s(&d.offsets);
    w.f64s(&d.arc_point);
    w.end();
}

/// An ordinate dimension (`ON_DimOrdinate::Write`, content version 0; openNURBS' annotation
/// type 6).
pub(super) fn write_ordinate_dimension(w: &mut Writer, d: &OrdinateDimension) {
    w.begin_anonymous(0);
    w.begin_anonymous(1);
    write_annotation_core(w, "", 0.0, d.style, &d.plane, 6);
    write_dimension_core(w, &d.text);
    w.end();
    w.u32(d.direction);
    w.f64s(&d.point);
    w.f64s(&d.leader);
    w.f64s(&d.kinks);
    w.end();
}

/// A leader (`ON_Leader::Write`, version 1): the annotation with its words, then its points.
pub(super) fn write_leader(w: &mut Writer, l: &Leader) {
    w.begin_anonymous(1);
    write_annotation_core(w, &l.rich, 0.0, l.style, &l.plane, 10);
    w.count(l.points.len());
    for p in &l.points {
        w.f64s(p);
    }
    w.end();
}
