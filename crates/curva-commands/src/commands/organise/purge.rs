//! The `Purge` command.

use crate::Message;
use crate::recipe::{Context, Edit, Made, OptionDef, Recipe, StepDef};
use curva_document::PurgeWhat;

const ANNOTATION_STYLES: &str = "AnnotationStyles";
const BLOCK_DEFINITIONS: &str = "BlockDefinitions";
const HATCH_PATTERNS: &str = "HatchPatterns";
const LAYERS: &str = "Layers";
const LINETYPES: &str = "Linetypes";

/// `Purge`, as in Rhino 8 (#771): takes away what nothing uses, of the kinds its options leave on
/// (all of them at first), when Enter is pressed. Curva keeps no bitmaps, environments, materials
/// or textures, and its groups are no table of their own, so those options are not offered.
#[derive(Default)]
pub struct Purge;

impl Recipe for Purge {
    const NAME: &'static str = "purge";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::toggle(ANNOTATION_STYLES, true),
        OptionDef::toggle(BLOCK_DEFINITIONS, true),
        OptionDef::toggle(HATCH_PATTERNS, true),
        OptionDef::toggle(LAYERS, true),
        OptionDef::toggle(LINETYPES, true),
    ];

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::point("prompt.purge.what").optional().offering(&[
            ANNOTATION_STYLES,
            BLOCK_DEFINITIONS,
            HATCH_PATTERNS,
            LAYERS,
            LINETYPES,
        ])]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let what = PurgeWhat {
            annotation_styles: cx.toggle(ANNOTATION_STYLES),
            blocks: cx.toggle(BLOCK_DEFINITIONS),
            hatch_patterns: cx.toggle(HATCH_PATTERNS),
            layers: cx.toggle(LAYERS),
            linetypes: cx.toggle(LINETYPES),
        };
        // What it will take away, counted on a copy, for the message.
        let mut copy = cx.doc.state().clone();
        let purged = copy.purge_with(what);
        Ok(Made {
            edits: vec![Edit::Purge(what)],
            message: Some(Message::with(
                "done.purge",
                vec![
                    purged.total().to_string(),
                    purged.layers.to_string(),
                    purged.blocks.to_string(),
                    purged.annotation_styles.to_string(),
                    purged.hatch_patterns.to_string(),
                    purged.linetypes.to_string(),
                ],
            )),
            ..Made::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Curve;

    fn line(doc: &mut Document, y: f64) -> u64 {
        doc.add(Curve::line([0.0, y, 0.0], [1.0, y, 0.0]).unwrap())
    }

    #[test]
    fn purge_leaves_alone_what_its_options_turn_off_and_keeps_used_linetypes_straight() {
        let mut doc = Document::default();
        doc.add_layer("Empty".to_owned());
        let unused = doc.add_linetype("Unused", &[1.0, 1.0]);
        let used = doc.add_linetype("Used", &[2.0, 1.0]);
        let a = line(&mut doc, 0.0);
        doc.set_linetype(&[a], used);
        assert_ne!(unused, used);
        // Layers off: the empty layer stays; the unused linetype goes.
        run(&mut doc, &["purge", "Layers", ""]);
        assert_eq!(doc.layers().len(), 2);
        assert_eq!(doc.state().linetypes.len(), 1);
        let kept = doc.state().objects[0].linetype;
        assert_eq!(doc.state().linetype_name(kept), "Used");
    }

    #[test]
    fn purge_audit_lockswap_and_draw_order() {
        let mut doc = Document::default();
        doc.add_layer("Empty".to_owned());
        let a = line(&mut doc, 0.0);
        let b = line(&mut doc, 1.0);
        run(&mut doc, &["purge", ""]);
        assert_eq!(doc.layers().len(), 1);
        let processor = run(&mut doc, &["audit"]);
        assert!(processor.log().last().unwrap().contains("No problems"));
        doc.lock(&[a]);
        run(&mut doc, &["lockswap"]);
        assert!(
            !doc.state()
                .objects
                .iter()
                .find(|o| o.id == a)
                .unwrap()
                .locked
        );
        assert!(
            doc.state()
                .objects
                .iter()
                .find(|o| o.id == b)
                .unwrap()
                .locked
        );
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "unlockselected");
        doc.select(&[b]);
        processor.submit(&mut doc, "");
        assert!(doc.state().objects.iter().all(|o| !o.locked));
        doc.select(&[a]);
        run(&mut doc, &["bringtofront"]);
        assert_eq!(doc.state().objects.last().unwrap().id, a);
        doc.select(&[a]);
        run(&mut doc, &["sendtoback"]);
        assert_eq!(doc.state().objects[0].id, a);
    }
}
