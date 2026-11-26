//! Each kind of object's faces drawn as the view's style says, by Bevy.

use super::{drawing, middle, one_at_a_time, pilot};
use crate::{FaceKind, PlacedShape, StandardView, Style};
use curva_document::style::{Colour, ColourFrom, ViewStyle};
use std::sync::Arc;

/// A slab filling the view seen from above, as a placed shape of `kind`, in grey.
fn slab(kind: FaceKind) -> PlacedShape {
    let slab = curva_kernel::Body::cuboid([-5.0e4, -5.0e4, -1.0], [1.0e5, 1.0e5, 2.0]).unwrap();
    PlacedShape {
        object: 0,
        colour: [0.6; 3],
        mesh: slab.shared_mesh().unwrap(),
        placement: slab.placement(),
        colours: None,
        kind,
    }
}

/// The middle pixel of a slab of `kind` seen from above in `style`, once it has settled.
fn seen(kind: FaceKind, style: &ViewStyle) -> Option<[u8; 4]> {
    let (mut pilot, device, queue) = pilot()?;
    let mut view = drawing(StandardView::Top, Vec::new());
    view.shapes = Arc::new(vec![slab(kind)]);
    view.style = Style::of(style);
    for _ in 0..3 {
        middle(&mut pilot, &device, &queue, &view);
    }
    Some(middle(&mut pilot, &device, &queue, &view))
}

const BACKGROUND: [u8; 4] = [200, 120, 40, 255];

#[test]
fn meshes_follow_nurbs_until_set_apart() {
    let _turn = one_at_a_time();
    let mut style = ViewStyle::shaded();
    style.nurbs.faces.colour = ColourFrom::One(Colour([0, 0, 255]));
    let Some(following) = seen(FaceKind::Mesh, &style) else {
        return;
    };
    assert!(
        following[2] > following[0] + 40,
        "blue as NURBS: {following:?}"
    );
    style.meshes.same_as_nurbs = false;
    style.meshes.faces.colour = ColourFrom::One(Colour([255, 0, 0]));
    let Some(apart) = seen(FaceKind::Mesh, &style) else {
        return;
    };
    assert!(apart[0] > apart[2] + 40, "red of its own: {apart:?}");
    let Some(nurbs) = seen(FaceKind::Nurbs, &style) else {
        return;
    };
    assert!(nurbs[2] > nurbs[0] + 40, "NURBS stay blue: {nurbs:?}");
}

#[test]
fn a_kind_s_faces_hidden_show_what_is_behind_while_others_show() {
    let _turn = one_at_a_time();
    let mut style = ViewStyle::shaded();
    style.subd.same_as_nurbs = false;
    style.subd.faces.shown = false;
    let (Some(subd), Some(nurbs)) = (seen(FaceKind::Subd, &style), seen(FaceKind::Nurbs, &style))
    else {
        return;
    };
    assert_eq!(subd, BACKGROUND, "SubD faces hidden");
    assert_ne!(nurbs, BACKGROUND, "NURBS faces still show");
}

#[test]
fn a_kind_s_opacity_lets_the_background_through_and_locked_faces_keep_their_colour() {
    let _turn = one_at_a_time();
    let mut style = ViewStyle::shaded();
    style.nurbs.faces.colour = ColourFrom::One(Colour([0, 0, 255]));
    let Some(solid) = seen(FaceKind::Nurbs, &style) else {
        return;
    };
    style.nurbs.faces.opacity = 0.4;
    let Some(faint) = seen(FaceKind::Nurbs, &style) else {
        return;
    };
    assert!(
        faint[0] > solid[0] + 40,
        "the orange background shows through: {solid:?} then {faint:?}"
    );
    style.nurbs.faces.opacity = 1.0;
    let Some(given) = seen(FaceKind::Given, &style) else {
        return;
    };
    assert!(
        given[0] > 60 && given[2] < given[0] + 40,
        "a locked object keeps its own grey, not the style's blue: {given:?}"
    );
}

/// Going from a sheet back to the model changes a view's faces and its style in the same frame:
/// the parts gone are removed first, and the new style must not reach for them (#811, which
/// crashed Curva on the second click on Model).
#[test]
fn faces_and_style_changing_together_do_not_reach_removed_parts() {
    let _turn = one_at_a_time();
    let Some((mut pilot, device, queue)) = pilot() else {
        return;
    };
    let mut view = drawing(StandardView::Top, Vec::new());
    view.shapes = Arc::new(vec![slab(FaceKind::Nurbs)]);
    view.style = Style::of(&ViewStyle::shaded());
    for _ in 0..2 {
        middle(&mut pilot, &device, &queue, &view);
    }
    // Another object's slab in place of the first, and another look, at once.
    let mut other = slab(FaceKind::Nurbs);
    other.object = 1;
    view.shapes = Arc::new(vec![other]);
    view.objects = Arc::new(vec![7, 8]);
    view.style = Style::of(&ViewStyle::ghosted());
    let pixel = middle(&mut pilot, &device, &queue, &view);
    assert_ne!(pixel, BACKGROUND, "the new slab shows");
}
