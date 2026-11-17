//! Each object into the file's model: its geometry (on a sheet, as a clipping plane, or as it
//! is) and its attributes (layer, print width, linetype, colour, groups, section style).

#[allow(clippy::wildcard_imports)]
use super::*;

/// What the objects look up as they go out: the tables already written, by Curva's ids.
pub(super) struct Tables<'a> {
    pub(super) state: &'a State,
    /// Each detail's viewport id, by its frame.
    pub(super) detail_ids: BTreeMap<u64, Uuid>,
    /// Each layout's page id.
    pub(super) page_ids: BTreeMap<u64, Uuid>,
    /// Each block definition's id.
    pub(super) block_ids: BTreeMap<u64, Uuid>,
    /// Each annotation style's id.
    pub(super) style_ids: StyleIds,
    /// Each hatch pattern's index, by its name in small letters.
    pub(super) pattern_index: BTreeMap<String, i32>,
    /// Each layer's index in the file's layer table.
    pub(super) layer_index: BTreeMap<u64, usize>,
}

/// The model being written, with what changes as objects go in.
pub(super) struct Out {
    pub(super) model: Model,
    pub(super) report: Report,
    pub(super) ids: Ids,
    /// Each chosen object's id, settled first; taken once when it is written.
    pub(super) object_ids: BTreeMap<u64, Uuid>,
    /// Each Curva group's index in the file's group table.
    pub(super) groups: BTreeMap<u64, usize>,
}

impl Out {
    /// One object into the model (`in_block` for a block definition's), returning its id, or
    /// `None` when it cannot be written, which the report counts.
    pub(super) fn add(&mut self, t: &Tables<'_>, o: &Object, in_block: bool) -> Option<Uuid> {
        let sheet = o
            .space
            .and_then(|id| t.state.layouts.iter().find(|l| l.id == id));
        let geometry = match self.geometry(t, o, sheet) {
            Ok(g) => g,
            Err(why) => {
                count(&mut self.report.skipped, &format!("not written ({why})"));
                return None;
            }
        };
        // A detail is written under its viewport's id, as Rhino names details: layers turned off
        // in one name it by that.
        // Other objects keep the id settled for them, taken once (a scale bar's or table's
        // parts share its number).
        let id = match &geometry {
            Geometry::Detail(d) if d.id != Uuid::NIL => d.id,
            _ if in_block => self.ids.next(),
            _ => self
                .object_ids
                .remove(&o.id)
                .unwrap_or_else(|| self.ids.next()),
        };
        let attributes = self.attributes(t, o, id, in_block, sheet);
        self.model.objects.push(model::Object {
            geometry,
            attributes,
        });
        if !in_block {
            count(&mut self.report.brought, kind_of(&o.shape));
        }
        Some(id)
    }

    /// An object's geometry: on a layout, drawn in millimetres on its page, a detail's frame as
    /// the detail; a clipping plane's rectangle as the plane; others as they are.
    fn geometry(
        &self,
        t: &Tables<'_>,
        o: &Object,
        sheet: Option<&curva_document::layouts::Layout>,
    ) -> Result<Geometry, String> {
        match sheet {
            Some(layout) => sheet_geometry(
                o,
                layout,
                &t.block_ids,
                &t.pattern_index,
                &t.style_ids,
                t.state,
                &t.detail_ids,
            ),
            None => match t.state.clipping_planes.iter().find(|c| c.frame == o.id) {
                Some(clip) => clipping_plane(o, clip, &self.model.model_views, &t.detail_ids),
                None => geometry_of(
                    &o.shape,
                    &t.block_ids,
                    &t.pattern_index,
                    &t.style_ids,
                    t.state,
                ),
            },
        }
    }

    /// An object's attributes under the id `id`.
    fn attributes(
        &mut self,
        t: &Tables<'_>,
        o: &Object,
        id: Uuid,
        in_block: bool,
        sheet: Option<&curva_document::layouts::Layout>,
    ) -> Attributes {
        let groups = &mut self.groups;
        let group = o.group.map(|g| {
            let n = groups.len();
            *groups.entry(g).or_insert(n)
        });
        let (print_width, print_width_source) = match o.print_width {
            PrintWidth::ByLayer => (0.0, Source::Layer),
            PrintWidth::Default => (0.0, Source::Object),
            PrintWidth::Millimetres(w) => (w, Source::Object),
        };
        let (linetype, linetype_source) = if o.linetype.is_by_layer() {
            (-1, Source::Layer)
        } else {
            (
                linetype_in(&mut self.model, &mut self.ids, t.state, o.linetype),
                Source::Object,
            )
        };
        let (section_source, section) = section_of(t, o);
        Attributes {
            section_source,
            section,
            id,
            layer: t.layer_index.get(&o.layer).copied().unwrap_or(0),
            name: o.name.clone(),
            print_width,
            print_width_source,
            visible: !o.hidden,
            mode: if in_block {
                3
            } else if o.hidden {
                1
            } else if o.locked {
                2
            } else {
                0
            },
            groups: group.into_iter().collect(),
            page_space: sheet.is_some(),
            viewport: sheet.map_or(Uuid::NIL, |l| t.page_ids[&l.id]),
            user_text: o
                .user_text
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            linetype,
            linetype_source,
            color: o.colour.map_or([0, 0, 0], |c| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
            }),
            color_source: if o.colour.is_some() {
                Source::Object
            } else {
                Source::Layer
            },
        }
    }

    /// The finished model: its group table, and a note when live booleans or History were
    /// written as plain geometry.
    pub(super) fn finish(mut self, state: &State) -> (Model, Report) {
        let groups = self.groups.len();
        self.model.groups = (0..groups)
            .map(|_| model::Group {
                name: String::new(),
                id: self.ids.next(),
            })
            .collect();
        if state
            .objects
            .iter()
            .any(|o| o.live.is_some() || o.recipe.is_some())
        {
            self.report
                .notes
                .push("live booleans and History are saved as plain geometry".to_owned());
        }
        (self.model, self.report)
    }
}

/// An object's section style as Rhino writes it: a clipping plane's style is its object's own,
/// with the source left at its layer, as Rhino 8 writes it; another object's own style says it
/// comes from the object.
fn section_of(t: &Tables<'_>, o: &Object) -> (u8, Option<crate::sections::SectionStyle>) {
    let plane = t.state.clipping_planes.iter().find(|c| c.frame == o.id);
    let (source, section) = match (plane, &o.section) {
        (Some(c), _) => (
            crate::sections::FROM_LAYER,
            Some(curva_document::section::SectionStyle {
                fill: c.section,
                hatch: c.hatch.clone(),
            }),
        ),
        (None, Some(own)) => (crate::sections::FROM_OBJECT, Some(own.clone())),
        (None, None) => (crate::sections::FROM_LAYER, None),
    };
    (
        source,
        section.map(|s| crate::sections::to_rhino(&s, &t.pattern_index)),
    )
}
