//! Section styles (spec `layouts.md` §3): how the face a clipping plane cuts through a closed
//! solid is filled. As in Rhino 8, the style comes from the object, else its layer, else the
//! clipping plane that cuts it.

use crate::Document;
use crate::clipping::SectionFill;
use serde::{Deserialize, Serialize};

/// A hatch pattern drawn over a section's fill.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SectionHatch {
    /// The pattern, by its name in the document's hatch patterns.
    pub pattern: String,
    /// How large the pattern is drawn: 1 as the pattern is defined.
    pub scale: f64,
    /// How far the pattern is turned, in radians counter-clockwise.
    pub rotation: f64,
}

impl SectionHatch {
    /// A pattern at its own size, unturned.
    #[must_use]
    pub fn new(pattern: &str) -> Self {
        Self {
            pattern: pattern.to_owned(),
            scale: 1.0,
            rotation: 0.0,
        }
    }
}

/// A section style: the fill behind a cut, and a hatch over it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SectionStyle {
    /// The fill behind the cut: none, or a colour.
    pub fill: SectionFill,
    /// A hatch pattern drawn over the fill, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hatch: Option<SectionHatch>,
}

impl SectionStyle {
    /// Whether it draws anything over a cut.
    #[must_use]
    pub fn shows(&self) -> bool {
        self.fill != SectionFill::None || self.hatch.is_some()
    }
}

impl Document {
    /// The section style an object's cut by the clipping plane `frame` takes: the object's own,
    /// else its layer's, else the plane's. `None` when none of them has one, as for an object
    /// that is gone and a frame that is not a clipping plane.
    #[must_use]
    pub fn section_style(&self, object: u64, frame: u64) -> Option<SectionStyle> {
        let object = self.state.objects.iter().find(|o| o.id == object)?;
        if let Some(own) = &object.section {
            return Some(own.clone());
        }
        let layer = self.state.layers.iter().find(|l| l.id == object.layer);
        if let Some(style) = layer.and_then(|l| l.section.as_ref()) {
            return Some(style.clone());
        }
        let plane = self.clipping_plane_of(frame)?;
        Some(SectionStyle {
            fill: plane.section,
            hatch: plane.hatch.clone(),
        })
    }

    /// Sets objects' own section style, or `None` to take their layer's again. For a clipping
    /// plane's rectangle, sets the plane's style instead (its fill and hatch); `None` there means
    /// Curva's standard fill.
    pub fn set_section_style(&mut self, ids: &[u64], style: Option<SectionStyle>) {
        for plane in &mut self.state.clipping_planes {
            if ids.contains(&plane.frame) {
                let style = style.clone().unwrap_or_default();
                plane.section = style.fill;
                plane.hatch = style.hatch;
            }
        }
        let planes: Vec<u64> = self.state.clipping_planes.iter().map(|p| p.frame).collect();
        for object in &mut self.state.objects {
            if ids.contains(&object.id) && !planes.contains(&object.id) {
                std::sync::Arc::make_mut(object).section = style.clone();
            }
        }
    }

    /// Sets a layer's section style, or `None` for none, so its objects take the clipping
    /// plane's.
    pub fn set_layer_section_style(&mut self, id: u64, style: Option<SectionStyle>) {
        if let Some(layer) = self.state.layers.iter_mut().find(|l| l.id == id) {
            layer.section = style;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipping::{ClipView, ClippingPlane};
    use curva_kernel::Curve;

    /// A model with a box-like object (a curve stands in: styles do not look at the shape) and
    /// a clipping plane, returning the object and the plane's frame.
    fn model(doc: &mut Document) -> (u64, u64) {
        let object = doc.add(Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        let frame = doc.add(
            Curve::polyline(
                vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
                true,
            )
            .unwrap(),
        );
        doc.add_clipping_plane(ClippingPlane {
            frame,
            flipped: false,
            views: vec![ClipView::Viewport(0)],
            section: SectionFill::Solid([0.2, 0.3, 0.4]),
            hatch: Some(SectionHatch::new("Grid")),
        });
        (object, frame)
    }

    #[test]
    fn the_style_comes_from_the_object_else_its_layer_else_the_plane() {
        let mut doc = Document::default();
        let (object, frame) = model(&mut doc);
        let plane = doc.section_style(object, frame).unwrap();
        assert_eq!(plane.fill, SectionFill::Solid([0.2, 0.3, 0.4]));
        assert_eq!(plane.hatch, Some(SectionHatch::new("Grid")));

        let on_layer = SectionStyle {
            fill: SectionFill::None,
            hatch: Some(SectionHatch::new("Hatch1")),
        };
        let layer = doc.layer_of(object).unwrap();
        doc.set_layer_section_style(layer, Some(on_layer.clone()));
        assert_eq!(doc.section_style(object, frame), Some(on_layer.clone()));

        let own = SectionStyle {
            fill: SectionFill::Solid([1.0, 0.0, 0.0]),
            hatch: None,
        };
        doc.set_section_style(&[object], Some(own.clone()));
        assert_eq!(doc.section_style(object, frame), Some(own));

        // Cleared, the object takes its layer's again.
        doc.set_section_style(&[object], None);
        assert_eq!(doc.section_style(object, frame), Some(on_layer));
    }

    #[test]
    fn setting_a_clipping_planes_style_changes_the_plane() {
        let mut doc = Document::default();
        let (object, frame) = model(&mut doc);
        doc.set_section_style(&[frame], Some(SectionStyle::default()));
        let plane = doc.clipping_plane_of(frame).unwrap();
        assert_eq!(plane.section, SectionFill::default());
        assert_eq!(plane.hatch, None);
        assert_eq!(
            doc.section_style(object, frame),
            Some(SectionStyle::default())
        );
        // The rectangle itself keeps no style of its own.
        assert!(doc.state().objects.iter().all(|o| o.section.is_none()));
    }

    #[test]
    fn styles_are_saved_and_read_back() {
        let mut doc = Document::default();
        let (object, frame) = model(&mut doc);
        let style = SectionStyle {
            fill: SectionFill::None,
            hatch: Some(SectionHatch {
                pattern: "Plus".to_owned(),
                scale: 2.5,
                rotation: 0.5,
            }),
        };
        doc.set_section_style(&[object], Some(style.clone()));
        let layer = doc.layer_of(object).unwrap();
        doc.set_layer_section_style(layer, Some(SectionStyle::default()));
        let dir = std::env::temp_dir().join(format!("curva-section-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("styles.curva");
        doc.save(&path).unwrap();
        let back = Document::open(&path).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(back.section_style(object, frame), Some(style));
        assert_eq!(back.layers()[0].section, Some(SectionStyle::default()));
        assert_eq!(
            back.clipping_plane_of(frame).unwrap().hatch,
            Some(SectionHatch::new("Grid"))
        );
    }
}
