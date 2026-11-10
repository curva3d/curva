//! Views saved under a name. They are kept in the file but are not part of undo, so undoing a
//! change to the model never loses a saved view.

use crate::Document;
use curva_kernel::Point;
use serde::{Deserialize, Serialize};

/// A viewport's camera, saved under a name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedView {
    /// What the user called it.
    pub name: String,
    /// The standard view it was looking from, such as `Top`, which decides its plane.
    pub kind: String,
    /// The point looked at.
    pub target: Point,
    /// Turn around the vertical, in radians.
    pub yaw: f64,
    /// Height above the horizon, in radians.
    pub pitch: f64,
    /// Distance from the target.
    pub distance: f64,
    /// Forward, right and up for a view along a fixed direction, if it has one.
    pub axes: Option<[Point; 3]>,
    /// Whether it has no perspective.
    pub parallel: bool,
}

impl Document {
    /// The saved views, oldest first.
    #[must_use]
    pub fn views(&self) -> &[SavedView] {
        &self.views
    }

    /// The saved view with this name, ignoring case.
    #[must_use]
    pub fn saved_view(&self, name: &str) -> Option<&SavedView> {
        self.views
            .iter()
            .find(|view| view.name.eq_ignore_ascii_case(name))
    }

    /// Saves a view, replacing one with the same name.
    pub fn save_view(&mut self, view: SavedView) {
        match self
            .views
            .iter_mut()
            .find(|held| held.name.eq_ignore_ascii_case(&view.name))
        {
            Some(held) => *held = view,
            None => self.views.push(view),
        }
    }

    /// Forgets a saved view. Returns false if there was none with that name.
    pub fn delete_view(&mut self, name: &str) -> bool {
        let before = self.views.len();
        self.views
            .retain(|view| !view.name.eq_ignore_ascii_case(name));
        self.views.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(name: &str, distance: f64) -> SavedView {
        SavedView {
            name: name.to_owned(),
            kind: "Perspective".to_owned(),
            target: [1.0, 2.0, 3.0],
            yaw: 0.5,
            pitch: 0.25,
            distance,
            axes: None,
            parallel: false,
        }
    }

    #[test]
    fn a_name_is_saved_once_and_found_whatever_its_case() {
        let mut doc = Document::default();
        doc.save_view(view("Entrance", 10.0));
        doc.save_view(view("entrance", 20.0));
        assert_eq!(doc.views().len(), 1);
        assert_eq!(doc.saved_view("ENTRANCE").unwrap().distance, 20.0);
        assert!(doc.delete_view("Entrance"));
        assert!(!doc.delete_view("Entrance"));
    }

    #[test]
    fn views_survive_undo_and_are_kept_in_the_file() {
        let folder = std::env::temp_dir().join(format!("curva-views-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("views.curva");
        let mut doc = Document::default();
        let before = doc.state().clone();
        doc.add(curva_kernel::Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        doc.record_change(before);
        doc.save_view(view("Corner", 5.0));
        doc.undo();
        assert_eq!(doc.views().len(), 1);
        doc.save(&path).unwrap();
        let opened = Document::open(&path).unwrap();
        assert_eq!(opened.views(), doc.views());
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
