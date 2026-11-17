//! `SectionStyle`: how clipping planes fill what they cut through closed solids (spec
//! `layouts.md` §3). Rhino 8 sets section styles in Properties and has no such command; this is
//! Curva's own, as the spec has it, with Serpentine's `SectionHatch` as its alias.
//!
//! Select objects, then press Enter. On a solid it sets the solid's own style; on a clipping
//! plane, the plane's, which solids with no style of their own or on their layer take. `Layers`
//! sets it on the chosen objects' layers instead. `ByLayer` clears the objects' own style (or,
//! with `Layers`, the layers'), so they take the next one in Rhino's order again.

use crate::Message;
use crate::commands::arguments::objects;
use crate::recipe::{Context, Edit, Made, OptionDef, Recipe, StepDef};
use curva_document::clipping::SectionFill;
use curva_document::section::{SectionHatch, SectionStyle as Style};

const FILL: &str = "Fill";
const HATCH: &str = "Hatch";
const PATTERN: &str = "Pattern";
const SCALE: &str = "Scale";
const ROTATION: &str = "Rotation";
const LAYERS: &str = "Layers";
const BY_LAYER: &str = "ByLayer";

/// `SectionStyle`, on the command framework.
#[derive(Default)]
pub struct SectionStyle;

impl SectionStyle {
    /// The style the options describe.
    fn style(cx: &Context<'_>) -> Style {
        Style {
            fill: if cx.toggle(FILL) {
                SectionFill::default()
            } else {
                SectionFill::None
            },
            hatch: cx.toggle(HATCH).then(|| SectionHatch {
                pattern: cx.name(PATTERN),
                scale: cx.number(SCALE),
                rotation: cx.number(ROTATION).to_radians(),
            }),
        }
    }
}

impl Recipe for SectionStyle {
    const NAME: &'static str = "SectionStyle";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(FILL, true),
        OptionDef::toggle(HATCH, false),
        OptionDef::name(PATTERN, "Hatch1"),
        OptionDef::distance(SCALE, 1.0, false),
        OptionDef::number(ROTATION, 0.0),
        OptionDef::toggle(LAYERS, false),
        OptionDef::finish(BY_LAYER),
    ];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::objects("prompt.sectionstyle.objects").offering(&[]),
            StepDef::point("prompt.sectionstyle.apply").optional(),
        ]
    }

    fn choices(&self, cx: &Context<'_>, option: &str) -> Option<Vec<String>> {
        (option == PATTERN).then(|| {
            cx.doc
                .state()
                .patterns()
                .into_iter()
                .map(|p| p.name)
                .collect()
        })
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let chosen = objects(cx, 0).to_vec();
        if chosen.is_empty() || cx.answered < 2 && cx.mode != Some(BY_LAYER) {
            return Ok(Made::default());
        }
        let style = (cx.mode != Some(BY_LAYER)).then(|| Self::style(cx));
        let edit = if cx.toggle(LAYERS) {
            let mut layers: Vec<u64> = chosen
                .iter()
                .filter_map(|&id| cx.doc.layer_of(id))
                .collect();
            layers.sort_unstable();
            layers.dedup();
            Edit::LayerSectionStyle(layers, style)
        } else {
            Edit::SectionStyle(chosen.clone(), style)
        };
        Ok(Made {
            message: Some(Message::with(
                "done.sectionstyle",
                vec![chosen.len().to_string()],
            )),
            edits: vec![edit],
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_document::clipping::{ClipView, ClippingPlane, SectionFill};
    use curva_document::section::{SectionHatch, SectionStyle};
    use curva_kernel::Curve;

    /// An object and a clipping plane's rectangle, selected in turn by the tests.
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
            section: SectionFill::None,
            hatch: None,
        });
        (object, frame)
    }

    fn style_of(doc: &Document, object: u64) -> Option<SectionStyle> {
        doc.state()
            .objects
            .iter()
            .find(|o| o.id == object)
            .and_then(|o| o.section.clone())
    }

    #[test]
    fn a_solid_fill_by_default() {
        let mut doc = Document::default();
        let (object, frame) = model(&mut doc);
        doc.select(&[object]);
        let processor = run(&mut doc, &["SectionStyle", ""]);
        assert!(!processor.is_running());
        assert_eq!(
            style_of(&doc, object),
            Some(SectionStyle {
                fill: SectionFill::default(),
                hatch: None
            })
        );
        assert!(processor.log().iter().any(|l| l.contains('1')));
        assert_eq!(doc.section_style(object, frame).unwrap().hatch, None);
    }

    #[test]
    fn a_hatch_with_its_pattern_scale_and_rotation_and_no_fill() {
        let mut doc = Document::default();
        let (object, _) = model(&mut doc);
        doc.select(&[object]);
        run(
            &mut doc,
            &[
                "SectionStyle",
                "Fill",
                "Hatch",
                "Pattern",
                "grid",
                "Scale",
                "2.5",
                "Rotation",
                "30",
                "",
            ],
        );
        let style = style_of(&doc, object).unwrap();
        assert_eq!(style.fill, SectionFill::None);
        let hatch = style.hatch.unwrap();
        // Typed in other capitals, the pattern takes the name it has.
        assert_eq!(hatch.pattern, "Grid");
        assert!((hatch.scale - 2.5).abs() < 1e-12);
        assert!((hatch.rotation - 30f64.to_radians()).abs() < 1e-12);
    }

    #[test]
    fn a_pattern_that_does_not_exist_is_asked_again() {
        let mut doc = Document::default();
        let (object, _) = model(&mut doc);
        doc.select(&[object]);
        let processor = run(&mut doc, &["SectionStyle", "Hatch", "Pattern", "Bricks"]);
        assert!(
            processor
                .prompt()
                .is_some_and(|p| p.contains("There is no \"Bricks\""))
        );
        let mut processor = processor;
        processor.submit(&mut doc, "Plus");
        processor.submit(&mut doc, "");
        assert_eq!(
            style_of(&doc, object).unwrap().hatch.unwrap().pattern,
            "Plus"
        );
    }

    #[test]
    fn options_are_remembered_for_the_next_run() {
        let mut doc = Document::default();
        let (object, _) = model(&mut doc);
        doc.select(&[object]);
        run(&mut doc, &["SectionStyle", "Hatch", "Pattern", "Dash", ""]);
        let second = doc.add(Curve::line([0.0; 3], [0.0, 1.0, 0.0]).unwrap());
        doc.select(&[second]);
        run(&mut doc, &["SectionStyle", ""]);
        assert_eq!(
            style_of(&doc, second).unwrap().hatch.unwrap().pattern,
            "Dash"
        );
    }

    #[test]
    fn on_a_clipping_plane_it_sets_the_planes_style() {
        let mut doc = Document::default();
        let (object, frame) = model(&mut doc);
        doc.select(&[frame]);
        run(&mut doc, &["SectionStyle", "Hatch", ""]);
        let plane = doc.clipping_plane_of(frame).unwrap();
        assert_eq!(plane.section, SectionFill::default());
        assert_eq!(plane.hatch, Some(SectionHatch::new("Hatch1")));
        assert_eq!(style_of(&doc, frame), None);
        // The object, with no style of its own or on its layer, takes the plane's.
        assert_eq!(
            doc.section_style(object, frame).unwrap().hatch,
            Some(SectionHatch::new("Hatch1"))
        );
    }

    #[test]
    fn layers_sets_the_layers_style_and_by_layer_clears_it() {
        let mut doc = Document::default();
        let (object, frame) = model(&mut doc);
        doc.select(&[object]);
        run(&mut doc, &["SectionStyle", "Layers", ""]);
        assert_eq!(style_of(&doc, object), None);
        let layer = doc.layer_of(object).unwrap();
        let on_layer = |doc: &Document| {
            doc.layers()
                .iter()
                .find(|l| l.id == layer)
                .and_then(|l| l.section.clone())
        };
        assert_eq!(on_layer(&doc), Some(SectionStyle::default()));
        assert_eq!(
            doc.section_style(object, frame),
            Some(SectionStyle::default())
        );
        doc.select(&[object]);
        run(&mut doc, &["SectionStyle", "ByLayer"]);
        assert_eq!(on_layer(&doc), None);
    }

    #[test]
    fn by_layer_clears_an_objects_own_style() {
        let mut doc = Document::default();
        let (object, _) = model(&mut doc);
        doc.select(&[object]);
        run(&mut doc, &["SectionStyle", "Layers", ""]);
        // Layers is remembered: turn it off again, then set and clear the object's own.
        doc.select(&[object]);
        run(&mut doc, &["SectionStyle", "Layers", ""]);
        assert!(style_of(&doc, object).is_some());
        doc.select(&[object]);
        run(&mut doc, &["SectionStyle", "ByLayer"]);
        assert_eq!(style_of(&doc, object), None);
    }

    #[test]
    fn undo_takes_a_style_back() {
        let mut doc = Document::default();
        let (object, _) = model(&mut doc);
        doc.select(&[object]);
        run(&mut doc, &["SectionStyle", ""]);
        assert!(style_of(&doc, object).is_some());
        doc.undo();
        assert_eq!(style_of(&doc, object), None);
    }
}
