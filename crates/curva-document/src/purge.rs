//! Purge, as in Rhino 8 (#771): taking away the document's tables nothing uses. Each kind can be
//! left alone, as Rhino's options let it be.

use crate::annotation::Annotation;
use crate::{Document, Linetype, Object, Shape, State};
use std::collections::{BTreeMap, BTreeSet};

/// Which kinds Purge takes away, as Rhino's `Purge` options name them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct PurgeWhat {
    /// Annotation styles no annotation uses (but the current one).
    pub annotation_styles: bool,
    /// Block definitions no instance uses.
    pub blocks: bool,
    /// The document's own hatch patterns no hatch or section style uses.
    pub hatch_patterns: bool,
    /// Layers nothing is on (but the current one and the first).
    pub layers: bool,
    /// The document's own linetypes no object or layer uses.
    pub linetypes: bool,
}

impl PurgeWhat {
    /// Every kind, as Rhino's options all start.
    pub const ALL: Self = Self {
        annotation_styles: true,
        blocks: true,
        hatch_patterns: true,
        layers: true,
        linetypes: true,
    };
}

/// How many of each kind Purge took away.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Purged {
    /// Annotation styles.
    pub annotation_styles: usize,
    /// Block definitions.
    pub blocks: usize,
    /// Hatch patterns.
    pub hatch_patterns: usize,
    /// Layers.
    pub layers: usize,
    /// Linetypes.
    pub linetypes: usize,
}

impl Purged {
    /// How many things went, of every kind.
    #[must_use]
    pub fn total(&self) -> usize {
        self.annotation_styles + self.blocks + self.hatch_patterns + self.layers + self.linetypes
    }
}

impl Document {
    /// Removes layers nothing is on (but the current one and the first), and block definitions no
    /// instance uses; returns how many of each went.
    pub fn purge(&mut self) -> (usize, usize) {
        let purged = self.purge_with(PurgeWhat::ALL);
        (purged.layers, purged.blocks)
    }

    /// Removes what nothing uses, of the kinds `what` names.
    pub fn purge_with(&mut self, what: PurgeWhat) -> Purged {
        self.state.purge_with(what)
    }
}

impl State {
    /// Removes what nothing uses, of the kinds `what` names; on a copy, it counts what Purge
    /// would take away.
    pub fn purge_with(&mut self, what: PurgeWhat) -> Purged {
        let mut purged = Purged::default();
        if what.blocks {
            purged.blocks = self.purge_blocks();
        }
        if what.layers {
            purged.layers = self.purge_layers();
        }
        if what.annotation_styles {
            purged.annotation_styles = self.purge_annotation_styles();
        }
        if what.hatch_patterns {
            purged.hatch_patterns = self.purge_hatch_patterns();
        }
        if what.linetypes {
            purged.linetypes = self.purge_linetypes();
        }
        purged
    }

    /// Every object, at the top of the model and inside block definitions.
    fn every_object(&self) -> impl Iterator<Item = &Object> {
        let top = self.objects.iter().map(AsRef::as_ref);
        top.chain(self.blocks.iter().flat_map(|d| d.objects.iter()))
    }

    fn purge_blocks(&mut self) -> usize {
        let mut gone = 0;
        loop {
            let used: BTreeSet<u64> = self
                .every_object()
                .filter_map(|o| match &o.shape {
                    Shape::Instance(i) => Some(i.definition),
                    _ => None,
                })
                .collect();
            let before = self.blocks.len();
            self.blocks.retain(|d| used.contains(&d.id));
            if self.blocks.len() == before {
                return gone;
            }
            gone += before - self.blocks.len();
        }
    }

    fn purge_layers(&mut self) -> usize {
        let current = self.current_layer;
        let used: BTreeSet<u64> = self.objects.iter().map(|o| o.layer).collect();
        let first = self.layers.first().map(|l| l.id);
        let before = self.layers.len();
        self.layers
            .retain(|l| used.contains(&l.id) || l.id == current || Some(l.id) == first);
        before - self.layers.len()
    }

    fn purge_annotation_styles(&mut self) -> usize {
        let mut used: BTreeSet<u64> = self
            .every_object()
            .filter_map(|o| match &o.shape {
                Shape::Annotation(a) => Some(a.style()),
                _ => None,
            })
            .collect();
        used.insert(self.current_annotation_style);
        let first = self.annotation_styles.first().map(|s| s.id);
        let before = self.annotation_styles.len();
        self.annotation_styles
            .retain(|s| used.contains(&s.id) || Some(s.id) == first);
        before - self.annotation_styles.len()
    }

    fn purge_hatch_patterns(&mut self) -> usize {
        let mut used: BTreeSet<String> = BTreeSet::new();
        let section_hatches = self
            .every_object()
            .filter_map(|o| o.section.as_ref())
            .chain(self.layers.iter().filter_map(|l| l.section.as_ref()))
            .filter_map(|s| s.hatch.as_ref());
        for hatch in
            section_hatches.chain(self.clipping_planes.iter().filter_map(|c| c.hatch.as_ref()))
        {
            used.insert(hatch.pattern.to_lowercase());
        }
        for object in self.every_object() {
            if let Shape::Annotation(Annotation::Hatch(h)) = &object.shape {
                used.insert(h.pattern.to_lowercase());
            }
        }
        let before = self.hatch_patterns.len();
        self.hatch_patterns
            .retain(|p| used.contains(&p.name.to_lowercase()));
        before - self.hatch_patterns.len()
    }

    fn purge_linetypes(&mut self) -> usize {
        let custom = |l: Linetype| match l {
            Linetype::Custom(i) => Some(i),
            _ => None,
        };
        let used: BTreeSet<u16> = self
            .every_object()
            .filter_map(|o| custom(o.linetype))
            .chain(self.layers.iter().filter_map(|l| custom(l.linetype)))
            .collect();
        let before = self.linetypes.len();
        // The kept linetypes in order, and where each old place went.
        let mut moved: BTreeMap<u16, u16> = BTreeMap::new();
        let mut kept = Vec::new();
        for (i, linetype) in self.linetypes.drain(..).enumerate() {
            let Ok(i) = u16::try_from(i) else { break };
            if used.contains(&i) {
                if let Ok(to) = u16::try_from(kept.len()) {
                    moved.insert(i, to);
                }
                kept.push(linetype);
            }
        }
        self.linetypes = kept;
        let renumber = |l: &mut Linetype| {
            if let Linetype::Custom(i) = l {
                *i = moved.get(i).copied().unwrap_or(*i);
            }
        };
        for layer in &mut self.layers {
            renumber(&mut layer.linetype);
        }
        for object in &mut self.objects {
            if let Some(i) = custom(object.linetype)
                && moved.get(&i) != Some(&i)
            {
                renumber(&mut std::sync::Arc::make_mut(object).linetype);
            }
        }
        for definition in &mut self.blocks {
            for object in &mut definition.objects {
                renumber(&mut object.linetype);
            }
        }
        before - self.linetypes.len()
    }
}
