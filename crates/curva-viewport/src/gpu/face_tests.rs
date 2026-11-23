//! Each kind of object's faces drawn as its view's style gives it, by Curva's own drawer.

use super::{Picture, draw_picture};
use crate::{FaceKind, MeshVertex, NO_OBJECT, Style};
use curva_document::style::{Colour, ColourFrom, ViewStyle};

const SIZE: [u32; 2] = [64, 64];

/// The middle pixel of a grey triangle of `kind` filling the picture's middle, drawn in
/// `style`, and the background's. `None` without a graphics adapter.
fn drawn(kind: FaceKind, style: &ViewStyle) -> Option<([u8; 3], [u8; 3])> {
    drawn_in(kind, [0.6; 3], style)
}

/// As [`drawn`], the triangle in its own `colour`.
fn drawn_in(kind: FaceKind, colour: [f32; 3], style: &ViewStyle) -> Option<([u8; 3], [u8; 3])> {
    let corner = |x: f32, y: f32| MeshVertex {
        position: [x, y, 0.5],
        normal: [0.0, 0.0, 1.0],
        colour,
        object: NO_OBJECT,
        kind,
    };
    let picture = Picture {
        faces: vec![corner(-0.9, -0.9), corner(0.9, -0.9), corner(0.0, 0.9)],
        matrix: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        light: [0.0, 0.0, 1.0],
        style: Style::of(style),
        ..Picture::default()
    };
    let pixels = match draw_picture(picture, SIZE, true) {
        Ok(pixels) => pixels,
        Err(error) => {
            eprintln!("no graphics adapter to draw with, so not checked: {error}");
            return None;
        }
    };
    let w = SIZE[0] as usize;
    let at = |x: usize, y: usize| {
        let i = (y * w + x) * 4;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    };
    Some((at(w / 2, SIZE[1] as usize / 2), at(0, 0)))
}

fn one(r: u8, g: u8, b: u8) -> ColourFrom {
    ColourFrom::One(Colour([r, g, b]))
}

#[test]
fn meshes_follow_nurbs_until_set_apart() {
    let mut style = ViewStyle::shaded();
    style.nurbs.faces.colour = one(0, 0, 255);
    let Some((following, _)) = drawn(FaceKind::Mesh, &style) else {
        return;
    };
    assert!(
        following[2] > following[0] + 40,
        "blue as NURBS: {following:?}"
    );
    style.meshes.same_as_nurbs = false;
    style.meshes.faces.colour = one(255, 0, 0);
    let (Some((apart, _)), Some((nurbs, _))) = (
        drawn(FaceKind::Mesh, &style),
        drawn(FaceKind::Nurbs, &style),
    ) else {
        return;
    };
    assert!(apart[0] > apart[2] + 40, "red of its own: {apart:?}");
    assert!(nurbs[2] > nurbs[0] + 40, "NURBS stay blue: {nurbs:?}");
}

#[test]
fn a_kind_s_faces_hidden_draw_nothing_while_others_show() {
    let mut style = ViewStyle::shaded();
    style.subd.same_as_nurbs = false;
    style.subd.faces.shown = false;
    let (Some((subd, background)), Some((nurbs, _))) = (
        drawn(FaceKind::Subd, &style),
        drawn(FaceKind::Nurbs, &style),
    ) else {
        return;
    };
    assert_eq!(subd, background, "SubD faces hidden");
    assert_ne!(nurbs, background, "NURBS faces still show");
}

#[test]
fn only_meshes_shown_still_draw_their_faces() {
    let mut style = ViewStyle::shaded();
    style.nurbs.faces.shown = false;
    style.meshes.same_as_nurbs = false;
    let Some((mesh, background)) = drawn(FaceKind::Mesh, &style) else {
        return;
    };
    assert_ne!(mesh, background, "the mesh's faces show without NURBS ones");
}

#[test]
fn a_see_through_kind_lets_the_background_through() {
    let mut style = ViewStyle::shaded();
    style.nurbs.faces.colour = one(255, 255, 255);
    let Some((solid, background)) = drawn(FaceKind::Nurbs, &style) else {
        return;
    };
    style.nurbs.faces.opacity = 0.3;
    let Some((faint, _)) = drawn(FaceKind::Nurbs, &style) else {
        return;
    };
    let brightness = |c: [u8; 3]| c.iter().map(|&v| u32::from(v)).sum::<u32>();
    assert!(
        brightness(faint) < brightness(solid) && brightness(faint) > brightness(background),
        "between the face and the background: {background:?} < {faint:?} < {solid:?}"
    );
}

#[test]
fn flat_faces_take_the_style_s_colour_unlit_and_locked_ones_keep_their_own() {
    let mut style = ViewStyle::hidden();
    style.nurbs.faces.colour = one(200, 40, 40);
    let red = Colour([200, 40, 40]).to_f32();
    let (Some((flat, _)), Some((as_given, _)), Some((locked, _))) = (
        drawn(FaceKind::Nurbs, &style),
        drawn_in(FaceKind::Given, red, &style),
        drawn(FaceKind::Given, &style),
    ) else {
        return;
    };
    assert!(flat[0] > flat[1] + 60, "in the style's red: {flat:?}");
    assert_eq!(flat, as_given, "unlit: as a locked face given that red");
    assert!(
        locked[0] == locked[1] && locked[1] == locked[2] && locked != flat,
        "a locked object keeps its own grey, as in Rhino: {locked:?}"
    );
}
