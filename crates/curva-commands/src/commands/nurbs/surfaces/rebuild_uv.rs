//! The `RebuildUv` command.

use crate::Message;
use crate::commands::arguments::*;
use crate::commands::bodies_of;
use crate::recipe::{Context, Made, OptionDef, Recipe, StepDef};
use curva_document::Shape;

const DIRECTION: &str = "Direction";
const POINT_COUNT: &str = "PointCount";
const DELETE_INPUT: &str = "DeleteInput";

/// `RebuildUV`, as in Rhino 8 (#740): rebuilds surfaces one way, U or V, as a cubic on uniform
/// knots with `PointCount` points (10 at first), the other way kept as it was. A way that closes
/// on itself, as a sphere's way round, comes out periodic. The rebuilt surfaces are added and the
/// old ones kept, unless `DeleteInput`.
#[derive(Default)]
pub struct RebuildUv;

impl Recipe for RebuildUv {
    const NAME: &'static str = "rebuilduv";
    const OPTIONS: &'static [OptionDef] = &[
        OptionDef::name(DIRECTION, "U"),
        OptionDef::whole(POINT_COUNT, 10, 4, 1000),
        OptionDef::toggle(DELETE_INPUT, false),
    ];

    fn choices(&self, _cx: &Context<'_>, option: &str) -> Option<Vec<String>> {
        (option == DIRECTION).then(|| vec!["U".to_owned(), "V".to_owned()])
    }

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![
            StepDef::objects("prompt.rebuilduv.objects")
                .bodies()
                .offering(&[DIRECTION, POINT_COUNT, DELETE_INPUT]),
        ]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let count = cx.number(POINT_COUNT).round().max(4.0) as usize;
        let along_v = cx.name(DIRECTION).eq_ignore_ascii_case("V");
        let mut made = Made::default();
        for (id, body) in bodies_of(cx.doc, objects(cx, 0)) {
            let rebuilt = body
                .rebuilt_one_way(along_v, count)
                .map_err(|e| failed(&e))?;
            if cx.toggle(DELETE_INPUT) {
                made.replace.push((id, Shape::Body(rebuilt)));
            } else {
                made.add.push(Shape::Body(rebuilt));
            }
        }
        let rebuilt = made.add.len() + made.replace.len();
        made.message = Some(Message::with("done.rebuilduv", vec![rebuilt.to_string()]));
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::run;
    use curva_document::Document;
    use curva_kernel::Body;

    #[test]
    fn a_sphere_is_rebuilt_round_and_kept_as_in_rhino() {
        let mut doc = Document::default();
        let sphere = doc.add_body(Body::sphere([0.0; 3], 5.0).unwrap());
        doc.select(&[sphere]);
        run(&mut doc, &["rebuilduv"]);
        // Rhino 8: a new surface beside the sphere, area 314.06.
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 2);
        let rebuilt = objects[1].shape.body().unwrap();
        let area = rebuilt.area().unwrap();
        assert!((area - 314.06).abs() < 0.01, "{area}");
    }

    #[test]
    fn direction_v_and_delete_input_rebuild_the_other_way_in_place() {
        let mut doc = Document::default();
        let sphere = doc.add_body(Body::sphere([0.0; 3], 5.0).unwrap());
        let mut processor = run(&mut doc, &["rebuilduv"]);
        for typed in ["Direction", "V", "PointCount", "6", "DeleteInput"] {
            processor.submit(&mut doc, typed);
        }
        doc.select(&[sphere]);
        processor.submit(&mut doc, "");
        let objects = &doc.state().objects;
        assert_eq!(objects.len(), 1);
        // Six points pole to pole, the way round untouched, rational as in Rhino. Rhino 8 gives
        // 311.17 here (and 311.22 with 10 points, never reaching the sphere); Curva 306.4, then
        // 314.03 with 10 points.
        let area = objects[0].shape.body().unwrap().area().unwrap();
        assert!((area - 306.41).abs() < 0.05, "{area}");
    }
}
