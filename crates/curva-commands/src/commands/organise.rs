//! The organising commands of phase F3 and the last of F2 (spec `docs/specs/m2-organise-analysis.md`):
//! `Count`, `ChangeLayer`, `MatchProperties`, `Linetype`, `PrintWidth`, `SetUserText`,
//! `GetUserText`, `Purge`, `Audit`, `LockSwap`, `UnlockSelected`, the draw-order commands,
//! `DupFaceBorder`, `TextObject` and `DraftAnalysis`.

use super::{ask_objects, bodies_of, unexpected};
use crate::recipe::{Context, Derived, Made, Recipe, StepDef, Value};
use crate::{Answer, Command, Kind, Message, Request, Step, ViewRequest};
use curva_document::{Document, Linetype, PrintWidth, Shape};
use curva_kernel::{Body, Point};

fn chosen(doc: &Document, answer: &Option<Answer>) -> Vec<u64> {
    match answer {
        Some(Answer::Objects(list)) => list.clone(),
        _ => doc.selection(),
    }
}

/// `Count`: how many objects are selected, by kind.
#[derive(Default)]
pub struct Count;

impl Command for Count {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let ids = chosen(doc, &answer);
        if ids.is_empty() {
            return ask_objects("prompt.count.objects");
        }
        let (mut curves, mut surfaces, mut solids, mut blocks) = (0, 0, 0, 0);
        for object in doc.state().objects.iter().filter(|o| ids.contains(&o.id)) {
            match &object.shape {
                Shape::Curve(_) => curves += 1,
                Shape::Body(b) if b.is_solid() => solids += 1,
                Shape::Body(_) => surfaces += 1,
                Shape::Instance(_) => blocks += 1,
            }
        }
        Step::Done(Some(Message::with(
            "done.count",
            [ids.len(), curves, surfaces, solids, blocks]
                .iter()
                .map(ToString::to_string)
                .collect(),
        )))
    }
}

/// `ChangeLayer`: moves objects to a layer, by its name.
#[derive(Default)]
pub struct ChangeLayer {
    objects: Vec<u64>,
}

impl Command for ChangeLayer {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Path(name)) if !self.objects.is_empty() => {
                let Some(layer) = doc
                    .layers()
                    .iter()
                    .find(|l| l.name.eq_ignore_ascii_case(name.trim()))
                    .map(|l| l.id)
                else {
                    return Step::Failed(Message::with("error.no_layer", vec![name]));
                };
                doc.move_to_layer(&self.objects, layer);
                Step::Done(Some(Message::with(
                    "done.changelayer",
                    vec![self.objects.len().to_string()],
                )))
            }
            other => {
                self.objects = chosen(doc, &other);
                if self.objects.is_empty() {
                    return ask_objects("prompt.changelayer.objects");
                }
                Step::Ask(Request::new("prompt.changelayer.name", Kind::Path))
            }
        }
    }
}

/// `MatchProperties`: copies layer, print width, linetype and name from one object to others:
/// the objects to change, then the one to match.
#[derive(Default)]
pub struct MatchProperties {
    targets: Vec<u64>,
}

impl Command for MatchProperties {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        if self.targets.is_empty() {
            self.targets = chosen(doc, &answer);
            if self.targets.is_empty() {
                return ask_objects("prompt.matchproperties.targets");
            }
            return ask_objects("prompt.matchproperties.source");
        }
        match answer {
            Some(Answer::Objects(list)) if !list.is_empty() => {
                let count = doc.match_properties(list[0], &self.targets);
                Step::Done(Some(Message::with(
                    "done.matchproperties",
                    vec![count.to_string()],
                )))
            }
            _ => ask_objects("prompt.matchproperties.source"),
        }
    }
}

/// `Linetype`: sets objects' linetype by name (ByLayer, Continuous, Dashed, Dots, DashDot,
/// Hidden, Center, Border).
#[derive(Default)]
pub struct LinetypeCommand {
    objects: Vec<u64>,
}

impl Command for LinetypeCommand {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Path(name)) if !self.objects.is_empty() => {
                let Some(linetype) = Linetype::named(name.trim()) else {
                    return Step::Failed(Message::with("error.no_linetype", vec![name]));
                };
                doc.set_linetype(&self.objects, linetype);
                Step::Done(Some(Message::with(
                    "done.linetype",
                    vec![linetype.name().to_owned()],
                )))
            }
            other => {
                self.objects = chosen(doc, &other);
                if self.objects.is_empty() {
                    return ask_objects("prompt.linetype.objects");
                }
                Step::Ask(Request::new("prompt.linetype.name", Kind::Path))
            }
        }
    }
}

/// `PrintWidth`: sets objects' print width: millimetres, `ByLayer` or `Default`.
#[derive(Default)]
pub struct PrintWidthCommand {
    objects: Vec<u64>,
}

impl Command for PrintWidthCommand {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Path(text)) if !self.objects.is_empty() => {
                let text = text.trim().trim_end_matches("mm").trim().to_owned();
                let width = if text.eq_ignore_ascii_case("bylayer") {
                    PrintWidth::ByLayer
                } else if text.eq_ignore_ascii_case("default") {
                    PrintWidth::Default
                } else if let Ok(mm) = text.parse::<f64>() {
                    PrintWidth::Millimetres(mm)
                } else {
                    return Step::Failed(Message::with("error.print_width", vec![text]));
                };
                doc.set_print_width(&self.objects, width);
                Step::Done(Some(Message::with(
                    "done.printwidth",
                    vec![self.objects.len().to_string()],
                )))
            }
            other => {
                self.objects = chosen(doc, &other);
                if self.objects.is_empty() {
                    return ask_objects("prompt.printwidth.objects");
                }
                Step::Ask(Request::new("prompt.printwidth.value", Kind::Path))
            }
        }
    }
}

/// `SetUserText`: keeps a key and its text on objects (an empty text removes the key).
#[derive(Default)]
pub struct SetUserText {
    objects: Vec<u64>,
    key: Option<String>,
}

impl Command for SetUserText {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Path(text)) if self.key.is_none() && !self.objects.is_empty() => {
                self.key = Some(text.trim().to_owned());
                Step::Ask(Request {
                    allow_empty: true,
                    ..Request::new("prompt.setusertext.value", Kind::Path)
                })
            }
            Some(Answer::Path(_) | Answer::Empty) if self.key.is_some() => {
                let value = match answer {
                    Some(Answer::Path(v)) => v,
                    _ => String::new(),
                };
                let key = self.key.clone().unwrap_or_default();
                doc.set_user_text(&self.objects, &key, &value);
                Step::Done(Some(Message::with(
                    "done.setusertext",
                    vec![key, self.objects.len().to_string()],
                )))
            }
            other => {
                self.objects = chosen(doc, &other);
                if self.objects.is_empty() {
                    return ask_objects("prompt.setusertext.objects");
                }
                Step::Ask(Request::new("prompt.setusertext.key", Kind::Path))
            }
        }
    }
}

/// `GetUserText`: shows an object's keys and texts.
#[derive(Default)]
pub struct GetUserText;

impl Command for GetUserText {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let ids = chosen(doc, &answer);
        let Some(object) = ids
            .first()
            .and_then(|id| doc.state().objects.iter().find(|o| o.id == *id))
        else {
            return ask_objects("prompt.getusertext.object");
        };
        if object.user_text.is_empty() {
            return Step::Done(Some(Message::new("done.getusertext_none")));
        }
        let shown: Vec<String> = object
            .user_text
            .iter()
            .map(|(k, v)| format!("{k} = {v}"))
            .collect();
        Step::Done(Some(Message::with(
            "done.getusertext",
            vec![shown.join("; ")],
        )))
    }
}

/// `Purge`: removes layers nothing is on and block definitions nothing uses.
#[derive(Default)]
pub struct Purge;

impl Command for Purge {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let (layers, blocks) = doc.purge();
        Step::Done(Some(Message::with(
            "done.purge",
            vec![layers.to_string(), blocks.to_string()],
        )))
    }
}

/// `Audit`: checks every object and reports broken geometry, selecting it.
#[derive(Default)]
pub struct Audit;

impl Command for Audit {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let found = doc.audit();
        if found.is_empty() {
            return Step::Done(Some(Message::new("done.audit_clean")));
        }
        let ids: Vec<u64> = found.iter().map(|(id, _)| *id).collect();
        doc.select(&ids);
        let first = found
            .first()
            .map(|(_, why)| why.clone())
            .unwrap_or_default();
        Step::Done(Some(Message::with(
            "done.audit",
            vec![found.len().to_string(), first],
        )))
    }
}

/// `LockSwap`: locks what is unlocked and unlocks what is locked.
#[derive(Default)]
pub struct LockSwap;

impl Command for LockSwap {
    fn step(&mut self, doc: &mut Document, _answer: Option<Answer>) -> Step {
        let count = doc.lock_swap();
        Step::Done(Some(Message::with(
            "done.lockswap",
            vec![count.to_string()],
        )))
    }
}

/// `UnlockSelected`: unlocks the locked objects picked (they can be picked while this asks).
#[derive(Default)]
pub struct UnlockSelected;

impl Command for UnlockSelected {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            Some(Answer::Objects(list)) => {
                let count = doc.unlock(&list);
                Step::Done(Some(Message::with("done.unlock", vec![count.to_string()])))
            }
            None => {
                // Every locked object, as Rhino offers them to pick from.
                let locked: Vec<u64> = doc
                    .state()
                    .objects
                    .iter()
                    .filter(|o| o.locked)
                    .map(|o| o.id)
                    .collect();
                if locked.is_empty() {
                    return Step::Done(Some(Message::with("done.unlock", vec!["0".to_owned()])));
                }
                let count = doc.unlock(&locked);
                Step::Done(Some(Message::with("done.unlock", vec![count.to_string()])))
            }
            _ => unexpected(),
        }
    }
}

/// `BringToFront`, `SendToBack`, `BringForward` and `SendBackward`: the order curves draw in
/// where they overlap.
#[derive(Default)]
pub struct DrawOrder<const FRONT: bool, const ALL_THE_WAY: bool>;

impl<const FRONT: bool, const ALL_THE_WAY: bool> Command for DrawOrder<FRONT, ALL_THE_WAY> {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        let ids = chosen(doc, &answer);
        if ids.is_empty() {
            return ask_objects("prompt.draworder.objects");
        }
        doc.reorder(&ids, FRONT, ALL_THE_WAY);
        Step::Done(Some(Message::with(
            "done.draworder",
            vec![ids.len().to_string()],
        )))
    }
}

/// `DupFaceBorder`: copies the outlines of picked faces as curves.
#[derive(Default)]
pub struct DupFaceBorder;

impl Recipe for DupFaceBorder {
    const NAME: &'static str = "dupfaceborder";

    fn steps(&self, _cx: &Context<'_>) -> Vec<StepDef> {
        vec![StepDef::faces("prompt.dupfaceborder.faces")]
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let Some(Value::Faces(picked)) = cx.values.first() else {
            return Ok(Made::default());
        };
        let mut made = Made::default();
        for (id, face) in picked {
            let Some((_, body)) = bodies_of(cx.doc, &[*id]).into_iter().next() else {
                continue;
            };
            let fail =
                |e: curva_kernel::KernelError| Message::with("error.geometry", vec![e.to_string()]);
            let lone = body.faces_only(&[*face]).map_err(fail)?;
            let curves = lone
                .naked_edges()
                .map_err(fail)?
                .into_iter()
                .map(|e| lone.edge_curve(e))
                .collect::<Result<Vec<_>, _>>()
                .map_err(fail)?;
            for curve in curva_kernel::join_all(&curves).map_err(fail)? {
                made.derived.push(Derived {
                    shape: Shape::Curve(curve),
                    from: *id,
                    history: None,
                });
            }
        }
        made.message = Some(Message::with(
            "done.dupfaceborder",
            vec![made.derived.len().to_string()],
        ));
        Ok(made)
    }
}

/// `TextObject`: text as curves (the default), flat surfaces or solids: the text, where it
/// starts, its height; `Font` names a system font (Curva's own type otherwise).
#[derive(Default)]
pub struct TextObject {
    text: Option<String>,
    at: Option<Point>,
    output: u8,
    font: Option<String>,
    choosing_font: bool,
}

impl TextObject {
    fn ask_text(&self) -> Step {
        Step::Ask(Request {
            options: vec!["Curves", "Surfaces", "Solids", "Font"],
            ..Request::new(
                if self.choosing_font {
                    "prompt.textobject.font"
                } else {
                    "prompt.textobject.text"
                },
                Kind::Path,
            )
        })
    }
}

impl Command for TextObject {
    fn step(&mut self, doc: &mut Document, answer: Option<Answer>) -> Step {
        match answer {
            None => self.ask_text(),
            Some(Answer::Option(word)) => {
                match word {
                    "Curves" => self.output = 0,
                    "Surfaces" => self.output = 1,
                    "Solids" => self.output = 2,
                    _ => self.choosing_font = true,
                }
                self.ask_text()
            }
            Some(Answer::Path(text)) if self.choosing_font => {
                self.font = Some(text.trim().to_owned());
                self.choosing_font = false;
                self.ask_text()
            }
            Some(Answer::Path(text)) if self.text.is_none() => {
                self.text = Some(text);
                Step::Ask(Request::new("prompt.textobject.point", Kind::Point))
            }
            Some(Answer::Point(p)) if self.at.is_none() => {
                self.at = Some(p);
                Step::Ask(Request {
                    allow_empty: true,
                    ..Request::new("prompt.textobject.height", Kind::Number)
                })
            }
            Some(Answer::Number(_) | Answer::Empty) => {
                let height = match answer {
                    Some(Answer::Number(h)) if h > 0.0 => h,
                    _ => 10.0,
                };
                let font = curva_kernel::font_bytes(self.font.as_deref());
                let at = self.at.unwrap_or([0.0; 3]);
                let glyphs = match curva_kernel::text_outlines(
                    self.text.as_deref().unwrap_or_default(),
                    &font,
                    height,
                    at,
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                ) {
                    Ok(glyphs) => glyphs,
                    Err(e) => {
                        return Step::Failed(Message::with("error.geometry", vec![e.to_string()]));
                    }
                };
                let mut added = Vec::new();
                for glyph in glyphs {
                    match self.output {
                        0 => {
                            for curve in glyph {
                                added.push(doc.add(curve));
                            }
                        }
                        kind => {
                            // Each outline with the holes inside it, filled.
                            let made = if kind == 1 {
                                Body::planar(&glyph)
                            } else {
                                Body::extruded(&glyph, [0.0, 0.0, 1.0], height / 5.0, true)
                            };
                            if let Ok(body) = made {
                                added.push(doc.add_body(body));
                            }
                        }
                    }
                }
                doc.select(&added);
                Step::Done(Some(Message::with(
                    "done.textobject",
                    vec![added.len().to_string()],
                )))
            }
            _ => unexpected(),
        }
    }
}

/// `DraftAnalysis`: colours the shaded faces of the last-used viewport by their angle to the
/// construction plane's normal: green where they have draft, red where they undercut, yellow
/// between. Run again to stop.
#[derive(Default)]
pub struct DraftAnalysis;

impl Command for DraftAnalysis {
    fn step(&mut self, _doc: &mut Document, _answer: Option<Answer>) -> Step {
        Step::View(ViewRequest::DraftAnalysis)
    }
}

#[cfg(test)]
mod tests {
    use super::super::run;
    use curva_document::{Document, Linetype, PrintWidth};
    use curva_kernel::{Body, Curve};

    fn line(doc: &mut Document, y: f64) -> u64 {
        doc.add(Curve::line([0.0, y, 0.0], [1.0, y, 0.0]).unwrap())
    }

    #[test]
    fn count_change_layer_and_match_properties() {
        let mut doc = Document::default();
        let a = line(&mut doc, 0.0);
        let b = doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        doc.select(&[a, b]);
        let processor = run(&mut doc, &["count"]);
        assert!(processor.log().last().unwrap().contains('2'));
        let walls = doc.add_layer("Walls".to_owned());
        doc.select(&[a]);
        run(&mut doc, &["changelayer", "walls"]);
        assert_eq!(doc.layer_of(a), Some(walls));
        doc.set_linetype(&[a], Linetype::Dashed);
        doc.set_print_width(&[a], PrintWidth::Millimetres(0.5));
        let c = line(&mut doc, 2.0);
        doc.select(&[c]);
        let mut processor = crate::Processor::default();
        processor.submit(&mut doc, "matchproperties");
        doc.select(&[a]);
        processor.submit(&mut doc, "");
        let matched = doc.state().objects.iter().find(|o| o.id == c).unwrap();
        assert_eq!(
            (matched.layer, matched.linetype, matched.print_width),
            (walls, Linetype::Dashed, PrintWidth::Millimetres(0.5))
        );
    }

    #[test]
    fn linetype_print_width_and_user_text() {
        let mut doc = Document::default();
        let a = line(&mut doc, 0.0);
        doc.select(&[a]);
        run(&mut doc, &["linetype", "hidden"]);
        assert_eq!(doc.state().objects[0].linetype, Linetype::Hidden);
        doc.select(&[a]);
        run(&mut doc, &["printwidth", "0.35"]);
        assert_eq!(doc.print_width_of(a), Some(0.35));
        doc.select(&[a]);
        run(&mut doc, &["setusertext", "Material", "Oak"]);
        doc.select(&[a]);
        let processor = run(&mut doc, &["getusertext"]);
        assert!(processor.log().last().unwrap().contains("Material = Oak"));
    }

    #[test]
    fn purge_audit_lockswap_and_draw_order() {
        let mut doc = Document::default();
        doc.add_layer("Empty".to_owned());
        let a = line(&mut doc, 0.0);
        let b = line(&mut doc, 1.0);
        run(&mut doc, &["purge"]);
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
        run(&mut doc, &["unlockselected"]);
        assert!(doc.state().objects.iter().all(|o| !o.locked));
        doc.select(&[a]);
        run(&mut doc, &["bringtofront"]);
        assert_eq!(doc.state().objects.last().unwrap().id, a);
        doc.select(&[a]);
        run(&mut doc, &["sendtoback"]);
        assert_eq!(doc.state().objects[0].id, a);
    }

    #[test]
    fn face_borders_and_text() {
        let mut doc = Document::default();
        let cube = doc.add_body(Body::cuboid([0.0; 3], [2.0, 3.0, 1.0]).unwrap());
        doc.select_faces(&[(cube, 0)]);
        run(&mut doc, &["dupfaceborder", ""]);
        let border = doc.state().objects.last().unwrap().curve().unwrap().clone();
        assert!(border.is_closed().unwrap());
        let mut doc = Document::default();
        run(&mut doc, &["textobject", "Hi", "0,0", "10"]);
        assert!(doc.state().objects.len() >= 2);
        let mut doc = Document::default();
        run(&mut doc, &["textobject", "Solids", "O", "0,0", "10"]);
        assert!(
            doc.state()
                .objects
                .iter()
                .all(|o| o.shape.body().is_some_and(curva_kernel::Body::is_solid))
        );
    }
}
