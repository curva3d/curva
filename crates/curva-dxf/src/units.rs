//! Units, line weights and colours as DXF writes them.

use crate::*;

/// The DXF unit code (`$INSUNITS`) for a model unit.
pub(crate) fn unit_code(unit: Unit) -> i16 {
    match unit {
        Unit::Inches => 1,
        Unit::Feet => 2,
        Unit::Millimetres => 4,
        Unit::Metres => 6,
    }
}

/// The model unit for a DXF unit code, if Curva has it.
pub(crate) fn unit_of(code: i16) -> Option<Unit> {
    match code {
        1 => Some(Unit::Inches),
        2 => Some(Unit::Feet),
        4 => Some(Unit::Millimetres),
        6 => Some(Unit::Metres),
        _ => None,
    }
}

/// AutoCAD's line weights, in hundredths of a millimetre: the only ones it accepts.
pub(crate) const LINE_WEIGHTS: [i16; 24] = [
    0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158, 200,
    211,
];

/// A colour channel from 0 to 1 as a byte.
pub(crate) fn byte(c: f32) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    byte
}

/// The AutoCAD line weight nearest a print width in millimetres.
pub(crate) fn line_weight(millimetres: f64) -> LineWeight {
    let wanted = millimetres * 100.0;
    let nearest = LINE_WEIGHTS
        .iter()
        .copied()
        .min_by(|a, b| {
            (f64::from(*a) - wanted)
                .abs()
                .total_cmp(&(f64::from(*b) - wanted).abs())
        })
        .unwrap_or(0);
    LineWeight::Value(nearest)
}
