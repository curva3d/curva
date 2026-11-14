//! Layouts: paper sheets beside the model, as in Rhino 8 (spec `layouts.md` §1). A layout has a
//! name and a paper size; objects drawn on it carry its id as their space.

use crate::{Document, Object};
use serde::{Deserialize, Serialize};

/// The unit a sheet is measured in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperUnit {
    /// Millimetres, for ISO sheets.
    Millimetres,
    /// Inches, for ANSI and Arch sheets.
    Inches,
}

impl PaperUnit {
    /// Its symbol.
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Millimetres => "mm",
            Self::Inches => "in",
        }
    }
}

/// A paper sheet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Identifies it for as long as the document lasts; objects on it carry it as their space.
    pub id: u64,
    /// Its name, shown on its tab.
    pub name: String,
    /// The sheet's width and height in [`Self::unit`], as it lies (landscape is wider).
    pub size: [f64; 2],
    /// The unit the sheet is measured and drawn in.
    pub unit: PaperUnit,
}

impl Layout {
    /// The name of the standard size it is, either way round, if it is one.
    #[must_use]
    pub fn paper_name(&self) -> Option<&'static str> {
        PAPER_SIZES.iter().find_map(|paper| {
            let [w, h] = paper.size;
            let same = |a: f64, b: f64| (a - b).abs() < 0.01;
            (paper.unit == self.unit
                && ((same(w, self.size[0]) && same(h, self.size[1]))
                    || (same(h, self.size[0]) && same(w, self.size[1]))))
            .then_some(paper.name)
        })
    }
}

/// A standard sheet, portrait.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaperSize {
    /// Its name, as printers know it.
    pub name: &'static str,
    /// Width and height, portrait, in [`Self::unit`].
    pub size: [f64; 2],
    /// Its unit.
    pub unit: PaperUnit,
}

const fn iso(name: &'static str, w: f64, h: f64) -> PaperSize {
    PaperSize {
        name,
        size: [w, h],
        unit: PaperUnit::Millimetres,
    }
}

const fn inch(name: &'static str, w: f64, h: f64) -> PaperSize {
    PaperSize {
        name,
        size: [w, h],
        unit: PaperUnit::Inches,
    }
}

/// The standard sheets: ISO A0 to A4, ANSI A to E and Arch A to E (spec `layouts.md` §1).
pub const PAPER_SIZES: [PaperSize; 15] = [
    iso("A4", 210.0, 297.0),
    iso("A3", 297.0, 420.0),
    iso("A2", 420.0, 594.0),
    iso("A1", 594.0, 841.0),
    iso("A0", 841.0, 1189.0),
    inch("ANSI A", 8.5, 11.0),
    inch("ANSI B", 11.0, 17.0),
    inch("ANSI C", 17.0, 22.0),
    inch("ANSI D", 22.0, 34.0),
    inch("ANSI E", 34.0, 44.0),
    inch("Arch A", 9.0, 12.0),
    inch("Arch B", 12.0, 18.0),
    inch("Arch C", 18.0, 24.0),
    inch("Arch D", 24.0, 36.0),
    inch("Arch E", 36.0, 48.0),
];

/// The standard sheet named `name`, ignoring case.
#[must_use]
pub fn paper_size(name: &str) -> Option<PaperSize> {
    PAPER_SIZES
        .iter()
        .copied()
        .find(|paper| paper.name.eq_ignore_ascii_case(name))
}

impl Document {
    /// The layouts, in their order.
    #[must_use]
    pub fn layouts(&self) -> &[Layout] {
        &self.state.layouts
    }

    /// The layout with this id.
    #[must_use]
    pub fn layout(&self, id: u64) -> Option<&Layout> {
        self.state.layouts.iter().find(|layout| layout.id == id)
    }

    /// Adds a layout of `paper`, landscape or portrait, at the end; returns its id.
    pub fn add_layout(&mut self, name: String, paper: PaperSize, landscape: bool) -> u64 {
        let [w, h] = paper.size;
        let size = if landscape {
            [h.max(w), h.min(w)]
        } else {
            [w.min(h), w.max(h)]
        };
        self.add_layout_sized(name, size, paper.unit)
    }

    /// Adds a layout of any size; returns its id.
    pub fn add_layout_sized(&mut self, name: String, size: [f64; 2], unit: PaperUnit) -> u64 {
        let id = self.state.next_layout;
        self.state.next_layout += 1;
        self.state.layouts.push(Layout {
            id,
            name,
            size,
            unit,
        });
        id
    }

    /// Copies a layout and everything on it, after it, named `name`; returns the copy's id.
    pub fn copy_layout(&mut self, id: u64, name: String) -> Option<u64> {
        let index = self.state.layouts.iter().position(|l| l.id == id)?;
        let copy = self.state.next_layout;
        self.state.next_layout += 1;
        let mut layout = self.state.layouts[index].clone();
        layout.id = copy;
        layout.name = name;
        self.state.layouts.insert(index + 1, layout);
        let on_it: Vec<Object> = self
            .state
            .objects
            .iter()
            .filter(|o| o.space == Some(id))
            .map(|o| Object::clone(o))
            .collect();
        for mut object in on_it {
            object.id = self.state.next_id;
            self.state.next_id += 1;
            object.space = Some(copy);
            self.state.objects.push(std::sync::Arc::new(object));
        }
        Some(copy)
    }

    /// Renames a layout.
    pub fn rename_layout(&mut self, id: u64, name: String) {
        if let Some(layout) = self.state.layouts.iter_mut().find(|l| l.id == id) {
            layout.name = name;
        }
    }

    /// Changes a layout's sheet.
    pub fn resize_layout(&mut self, id: u64, size: [f64; 2], unit: PaperUnit) {
        if let Some(layout) = self.state.layouts.iter_mut().find(|l| l.id == id) {
            layout.size = size;
            layout.unit = unit;
        }
    }

    /// Moves a layout to `to` in the order.
    pub fn move_layout(&mut self, id: u64, to: usize) {
        if let Some(from) = self.state.layouts.iter().position(|l| l.id == id) {
            let layout = self.state.layouts.remove(from);
            let to = to.min(self.state.layouts.len());
            self.state.layouts.insert(to, layout);
        }
    }

    /// Deletes a layout and everything on it. If it was being worked on, the model is again.
    pub fn delete_layout(&mut self, id: u64) {
        self.state.layouts.retain(|l| l.id != id);
        self.state.objects.retain(|o| o.space != Some(id));
        if self.space == Some(id) {
            self.space = None;
        }
        self.drop_missing_selection();
    }

    /// Where new objects go and what can be picked: `None` for the model, or a layout's id.
    #[must_use]
    pub fn space(&self) -> Option<u64> {
        self.space
    }

    /// Works on the model, or on a layout's sheet. Not saved, and not part of undo.
    pub fn set_space(&mut self, space: Option<u64>) {
        let space = space.filter(|id| self.layout(*id).is_some());
        if space != self.space {
            self.space = space;
            self.select(&[]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_kernel::Curve;

    fn line() -> Curve {
        Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap()
    }

    #[test]
    fn a_layout_is_a_sheet_of_a_standard_size_either_way_round() {
        let mut doc = Document::default();
        let a3 = paper_size("a3").unwrap();
        let id = doc.add_layout("Plans".to_owned(), a3, true);
        let layout = doc.layout(id).unwrap();
        assert_eq!(layout.size, [420.0, 297.0]);
        assert_eq!(layout.paper_name(), Some("A3"));
        let tall = doc.add_layout("Detail".to_owned(), paper_size("ANSI B").unwrap(), false);
        assert_eq!(doc.layout(tall).unwrap().size, [11.0, 17.0]);
        assert_eq!(doc.layout(tall).unwrap().unit, PaperUnit::Inches);
    }

    #[test]
    fn objects_go_into_the_space_being_worked_on_and_only_those_can_be_picked() {
        let mut doc = Document::default();
        let model = doc.add(line());
        let sheet = doc.add_layout("Sheet".to_owned(), paper_size("A4").unwrap(), false);
        doc.set_space(Some(sheet));
        let on_paper = doc.add(line());
        assert_eq!(doc.state().objects[1].space, Some(sheet));
        assert_eq!(doc.selectable(), vec![on_paper]);
        doc.set_space(None);
        assert_eq!(doc.selectable(), vec![model]);
    }

    #[test]
    fn copying_a_layout_copies_what_is_on_it_and_deleting_removes_it() {
        let mut doc = Document::default();
        let sheet = doc.add_layout("Sheet".to_owned(), paper_size("A4").unwrap(), false);
        doc.set_space(Some(sheet));
        doc.add(line());
        doc.set_space(None);
        let copy = doc.copy_layout(sheet, "Sheet copy".to_owned()).unwrap();
        assert_eq!(doc.layouts().len(), 2);
        assert_eq!(doc.layouts()[1].id, copy);
        let count = |doc: &Document, id| {
            doc.state()
                .objects
                .iter()
                .filter(|o| o.space == Some(id))
                .count()
        };
        assert_eq!((count(&doc, sheet), count(&doc, copy)), (1, 1));
        doc.move_layout(copy, 0);
        assert_eq!(doc.layouts()[0].id, copy);
        doc.delete_layout(sheet);
        assert_eq!(doc.layouts().len(), 1);
        assert_eq!(count(&doc, sheet), 0);
    }
}
