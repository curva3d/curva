//! The Print panel's page preview, built once on another thread and kept until the model or the
//! print choices change (#474): drawing a page of a large model takes long enough that building
//! it every frame slowed the whole app while the panel was open.

use curva_document::Document;
use curva_document::drawing::Page;
use curva_document::fields::FieldContext;
use curva_viewport::print::PrintColour;
use std::sync::mpsc::{Receiver, TryRecvError};

/// What a preview was built from. Equal keys give the same page.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PreviewKey {
    /// The layout shown.
    layout: u64,
    /// Whether everything prints black.
    black: bool,
    /// The number of changes recorded for undo, which grows with every edit.
    changes: usize,
    /// Where each object is held: an edited object is a new one, so this changes with any edit,
    /// undo or redo, even one that leaves the count of changes as it was.
    objects: u64,
    /// The layouts, with their details, and the layers, which decide what prints and how; small
    /// enough to compare whole.
    layouts: Vec<curva_document::layouts::Layout>,
    layers: Vec<curva_document::Layer>,
}

impl PreviewKey {
    /// The key for `layout` of `doc` as it is now.
    pub(crate) fn of(doc: &Document, layout: u64, black: bool) -> Self {
        use std::hash::{Hash, Hasher};
        let state = doc.state();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for object in &state.objects {
            std::sync::Arc::as_ptr(object).hash(&mut hasher);
        }
        Self {
            layout,
            black,
            changes: doc.undo_mark(),
            objects: hasher.finish(),
            layouts: state.layouts.clone(),
            layers: state.layers.clone(),
        }
    }
}

/// The preview held between frames.
#[derive(Default)]
pub(crate) struct Preview {
    /// The last page built and what it was built from.
    built: Option<(PreviewKey, Option<Page>)>,
    /// A page being built on another thread, and what it is built from.
    building: Option<(PreviewKey, Receiver<Option<Page>>)>,
}

impl Preview {
    /// The page for `key`, or the last one built while a newer one is on its way, and whether one
    /// is on its way. Starts building when nothing built or building matches `key`.
    pub(crate) fn page(
        &mut self,
        doc: &Document,
        key: &PreviewKey,
        fields: FieldContext,
    ) -> (Option<&Page>, bool) {
        self.collect();
        let fresh = self.built.as_ref().is_some_and(|(k, _)| k == key);
        let on_its_way = self.building.as_ref().is_some_and(|(k, _)| k == key);
        if !fresh && !on_its_way {
            self.start(doc, key.clone(), fields);
        }
        let page = self.built.as_ref().and_then(|(_, page)| page.as_ref());
        (page, !fresh)
    }

    /// Takes a finished page from the building thread.
    fn collect(&mut self) {
        let Some((_, receiver)) = &self.building else {
            return;
        };
        let page = match receiver.try_recv() {
            Ok(page) => page,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => None,
        };
        if let Some((key, _)) = self.building.take() {
            self.built = Some((key, page));
        }
    }

    /// Builds the page for `key` on another thread, from a copy of the model; a page still being
    /// built for an older key is let go.
    fn start(&mut self, doc: &Document, key: PreviewKey, fields: FieldContext) {
        let mut copy = Document::default();
        copy.restore(doc.state().clone());
        let (layout, black) = (key.layout, key.black);
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let colour = if black {
                PrintColour::Black
            } else {
                PrintColour::Display
            };
            let page = curva_viewport::print::layout_page(&copy, layout, &fields, colour);
            let _ = send.send(page);
        });
        self.building = Some((key, receive));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_kernel::Curve;

    fn model_with_a_layout() -> (Document, u64) {
        let mut doc = Document::default();
        let layout = doc.add_layout_sized(
            "Sheet".to_owned(),
            [297.0, 210.0],
            curva_document::layouts::PaperUnit::Millimetres,
        );
        let before = doc.state().clone();
        doc.add(Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap());
        doc.record_change(before);
        (doc, layout)
    }

    fn wait_for(preview: &mut Preview, doc: &Document, key: &PreviewKey) {
        for _ in 0..2000 {
            if !preview.page(doc, key, FieldContext::default()).1 {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("the preview never finished");
    }

    #[test]
    fn the_preview_is_built_once_and_kept_while_nothing_changes() {
        let (doc, layout) = model_with_a_layout();
        let key = PreviewKey::of(&doc, layout, false);
        let mut preview = Preview::default();
        wait_for(&mut preview, &doc, &key);
        assert!(preview.built.is_some());
        // Asked again with nothing changed: no new build starts.
        let (page, waiting) = preview.page(&doc, &key, FieldContext::default());
        assert!(page.is_some());
        assert!(!waiting);
        assert!(preview.building.is_none());
        assert_eq!(PreviewKey::of(&doc, layout, false), key);
    }

    #[test]
    fn an_edit_or_a_print_choice_asks_for_a_new_preview() {
        let (mut doc, layout) = model_with_a_layout();
        let key = PreviewKey::of(&doc, layout, false);
        assert_ne!(PreviewKey::of(&doc, layout, true), key);
        let before = doc.state().clone();
        doc.add(Curve::line([0.0; 3], [0.0, 5.0, 0.0]).unwrap());
        doc.record_change(before);
        assert_ne!(PreviewKey::of(&doc, layout, false), key);
        doc.undo();
        let undone = PreviewKey::of(&doc, layout, false);
        doc.redo();
        assert_ne!(PreviewKey::of(&doc, layout, false), undone);
    }
}
