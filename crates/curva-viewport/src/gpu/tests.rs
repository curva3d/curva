//! Tests of drawing on the graphics card.

use super::{
    PICK_ROW, Picture, Scene, chunk_ranges, draw_picture, draw_picture_in_chunks, save_picture,
    table_bytes,
};
use crate::{MeshVertex, NO_OBJECT, Vertex};
use std::sync::Arc;

const SIZE: [u32; 2] = [64, 64];

/// The brightest red value along the middle few rows at the centre column, or `None` when
/// this machine has no graphics adapter to draw with.
fn centre_brightness(ghosted: bool) -> Option<u8> {
    // Drawn straight in clip space: a grey face across the middle at depth 0.5, and a
    // white line behind it at depth 0.8.
    let corner = |x: f32, y: f32| MeshVertex {
        position: [x, y, 0.5],
        normal: [0.0, 0.0, 1.0],
        colour: [0.3, 0.3, 0.3],
        object: NO_OBJECT,
    };
    let faces = vec![corner(-0.9, -0.9), corner(0.9, -0.9), corner(0.0, 0.9)];
    let lines = (-2..=2)
        .flat_map(|step| {
            let y = step as f32 * 0.01;
            [
                Vertex {
                    position: [-0.5, y, 0.8],
                    colour: [1.0; 3],
                    width: -1.0,
                    object: NO_OBJECT,
                },
                Vertex {
                    position: [0.5, y, 0.8],
                    colour: [1.0; 3],
                    width: -1.0,
                    object: NO_OBJECT,
                },
            ]
        })
        .collect();
    let identity = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let picture = Picture {
        lines,
        faces,
        matrix: identity,
        light: [0.0, 0.0, 1.0],
        ghosted,
        hidden: false,
        hidden_lines: false,
        sketch: false,
        ground: 0.0,
    };
    let path = std::env::temp_dir().join(format!(
        "curva-ghost-test-{ghosted}-{}.png",
        std::process::id()
    ));
    if let Err(error) = save_picture(picture, SIZE, &path) {
        eprintln!("no graphics adapter to draw with, so not checked: {error}");
        return None;
    }
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).ok()?));
    let mut reader = decoder.read_info().ok()?;
    let mut pixels = vec![0; reader.output_buffer_size()?];
    reader.next_frame(&mut pixels).ok()?;
    let _ = std::fs::remove_file(&path);
    let (width, middle) = (SIZE[0] as usize, SIZE[1] as usize / 2);
    (middle - 3..=middle + 3)
        .map(|row| pixels[(row * width + width / 2) * 4])
        .max()
}

#[test]
fn a_line_behind_a_face_shows_through_only_when_ghosted() {
    let (Some(shaded), Some(ghosted)) = (centre_brightness(false), centre_brightness(true)) else {
        return;
    };
    assert!(ghosted > shaded + 40, "shaded {shaded}, ghosted {ghosted}");
}

/// How many rows of the centre column a horizontal line of `width` lights up, or `None`
/// without a graphics adapter.
fn rows_lit(width: f32) -> Option<usize> {
    let end = |x: f32| Vertex {
        position: [x, 0.0, 0.5],
        colour: [1.0; 3],
        width,
        object: NO_OBJECT,
    };
    let identity = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let picture = Picture {
        lines: vec![end(-0.8), end(0.8)],
        faces: Vec::new(),
        matrix: identity,
        light: [0.0, 0.0, 1.0],
        ghosted: false,
        hidden: false,
        hidden_lines: false,
        sketch: false,
        ground: 0.0,
    };
    let path = std::env::temp_dir().join(format!(
        "curva-width-test-{}-{}.png",
        width.to_bits(),
        std::process::id()
    ));
    if save_picture(picture, SIZE, &path).is_err() {
        return None;
    }
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).ok()?));
    let mut reader = decoder.read_info().ok()?;
    let mut pixels = vec![0; reader.output_buffer_size()?];
    reader.next_frame(&mut pixels).ok()?;
    let _ = std::fs::remove_file(&path);
    let w = SIZE[0] as usize;
    Some(
        (0..SIZE[1] as usize)
            .filter(|row| pixels[(row * w + w / 2) * 4] > 200)
            .count(),
    )
}

#[test]
fn lines_draw_at_their_own_width() {
    // The default line is 2 pixels; 1 mm is about 3.8 pixels at 96 dpi; 2 mm about 7.6.
    let (Some(default), Some(one), Some(two)) = (rows_lit(-2.0), rows_lit(1.0), rows_lit(2.0))
    else {
        return;
    };
    assert_eq!(default, 2);
    assert!((3..=5).contains(&one), "1 mm lit {one} rows");
    assert!((7..=9).contains(&two), "2 mm lit {two} rows");
}

/// A grey face crossed by lines of three widths, drawn straight in clip space.
fn mixed_picture() -> Picture {
    let corner = |x: f32, y: f32| MeshVertex {
        position: [x, y, 0.5],
        normal: [0.0, 0.0, 1.0],
        colour: [0.4, 0.4, 0.4],
        object: NO_OBJECT,
    };
    let line = |y: f32, width: f32| {
        [-0.8, 0.8].map(|x| Vertex {
            position: [x, y, 0.3],
            colour: [0.9, 0.5, 0.2],
            width,
            object: NO_OBJECT,
        })
    };
    Picture {
        lines: [line(-0.4, -1.0), line(0.0, -3.0), line(0.4, 0.5)]
            .into_iter()
            .flatten()
            .collect(),
        // Two triangles, so faces too cross a block boundary when blocks are small.
        faces: vec![
            corner(-0.9, -0.9),
            corner(0.9, -0.9),
            corner(0.0, 0.9),
            corner(0.5, 0.2),
            corner(0.95, 0.95),
            corner(0.2, 0.95),
        ],
        matrix: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        light: [0.0, 0.0, 1.0],
        ghosted: false,
        hidden: false,
        hidden_lines: false,
        sketch: false,
        ground: 0.0,
    }
}

/// Spec `display-pipeline.md` §5: optional extras never change the picture, so a card
/// without them shows the same.
#[test]
fn every_optional_extra_off_draws_the_same_picture() {
    let (Ok(bare), Ok(full)) = (
        draw_picture(mixed_picture(), SIZE, false),
        draw_picture(mixed_picture(), SIZE, true),
    ) else {
        eprintln!("no graphics adapter to draw with, so not checked");
        return;
    };
    // Something was drawn: not every pixel is the background.
    assert!(bare.chunks(4).any(|pixel| pixel != &bare[0..4]));
    assert_eq!(bare, full);
}

#[test]
fn blocks_never_split_a_segment_or_a_triangle() {
    assert!(chunk_ranges(0, 2, 10).is_empty());
    assert_eq!(chunk_ranges(7, 3, 100), vec![0..7]);
    // 10 vertices to a block holds 3 triangles (9 vertices), never a part of a fourth.
    assert_eq!(chunk_ranges(20, 3, 10), vec![0..9, 9..18, 18..20]);
    assert_eq!(chunk_ranges(10, 2, 5), vec![0..4, 4..8, 8..10]);
    // A block smaller than one group still holds a whole group.
    assert_eq!(chunk_ranges(4, 2, 1), vec![0..2, 2..4]);
    for (len, group, most) in [(1000, 2, 7), (999, 3, 64), (12, 3, 3)] {
        let ranges = chunk_ranges(len, group, most);
        assert_eq!(ranges.first().map(|r| r.start), Some(0));
        assert_eq!(ranges.last().map(|r| r.end), Some(len));
        for pair in ranges.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
            assert_eq!(pair[0].end % group, 0);
        }
    }
}

/// Spec `display-pipeline.md` step 4: a scene too big for one block of graphics memory is
/// drawn in several, and looks exactly the same.
#[test]
fn a_scene_drawn_in_many_blocks_looks_the_same() {
    // Blocks of 100 bytes: one segment or one triangle each.
    let (Ok(whole), Ok(split)) = (
        draw_picture(mixed_picture(), SIZE, true),
        draw_picture_in_chunks(mixed_picture(), SIZE, true, 100, &Scene::default()),
    ) else {
        eprintln!("no graphics adapter to draw with, so not checked");
        return;
    };
    assert!(whole.chunks(4).any(|pixel| pixel != &whole[0..4]));
    assert_eq!(whole, split);
}

/// A small model, seen whole: two circles and a box, so lines, edges on faces and faces all
/// show. Returns its outlines, the camera's matrix, the light and the ids made.
fn small_model() -> (crate::Outlines, [[f32; 4]; 4], [f32; 3], Vec<u64>) {
    let mut doc = curva_document::Document::default();
    let ids = vec![
        doc.add(curva_kernel::Curve::circle([0.0, 0.0, 0.0], 5.0).unwrap()),
        doc.add(curva_kernel::Curve::circle([12.0, 0.0, 0.0], 5.0).unwrap()),
        doc.add_body(curva_kernel::Body::cuboid([-4.0, 8.0, 0.0], [8.0, 8.0, 8.0]).unwrap()),
    ];
    let outlines = crate::Outlines::of(doc.state());
    let (low, high) = outlines.bounds().unwrap();
    let mut camera = crate::Camera::default();
    camera.frame(low, high);
    let light = camera.forward().map(|value| value as f32);
    (outlines, camera.matrix(1.0), light, ids)
}

/// Spec `display-pipeline.md` step 4: the selection drawn through the graphics card's table
/// looks exactly as when it was drawn into the vertices' colours, lines a pixel wider
/// included, in a shaded and in a wireframe view.
#[test]
fn the_selection_table_draws_what_coloured_vertices_drew() {
    let (outlines, matrix, light, ids) = small_model();
    // One circle and the box selected; the other circle not.
    let selected = vec![ids[0], ids[2]];
    let parts = crate::SceneParts::of(&outlines);
    for shaded in [true, false] {
        let picture = |lines: Vec<Vertex>, faces: Vec<MeshVertex>| Picture {
            lines,
            faces: if shaded { faces } else { Vec::new() },
            matrix,
            light,
            ghosted: false,
            hidden: false,
            hidden_lines: false,
            sketch: false,
            ground: 0.0,
        };
        let coloured = picture(
            if shaded {
                crate::shaded_object_lines(&outlines, &selected)
            } else {
                crate::object_lines(&outlines, &selected)
            },
            crate::face_vertices(&outlines, &selected),
        );
        let numbered = picture(
            if shaded {
                parts.shaded_lines.clone()
            } else {
                parts.lines.clone()
            },
            parts.faces.clone(),
        );
        let scene = Scene {
            objects: Arc::new(parts.objects.clone()),
            selection: Arc::new(selected.clone()),
            clips: Vec::new(),
            circles: Arc::default(),
        };
        let size = [96, 96];
        let (Ok(before), Ok(after)) = (
            draw_picture(coloured, size, true),
            draw_picture_in_chunks(numbered, size, true, super::CHUNK_BYTES, &scene),
        ) else {
            eprintln!("no graphics adapter to draw with, so not checked");
            return;
        };
        // The selection colour shows, so the comparison is about something.
        let orange = before
            .chunks(4)
            .filter(|p| p[0] > 180 && p[2] < 120)
            .count();
        assert!(orange > 20, "only {orange} selected pixels");
        assert_eq!(before, after, "shaded: {shaded}");
    }
}

/// #153: circles drawn by the graphics card from their centre and axes look as their segments
/// did, selected one included, in a shaded and in a wireframe view; at most a few edge pixels
/// differ, where the card's sines round otherwise than the kernel's.
#[test]
fn circles_drawn_from_their_axes_look_as_their_segments() {
    let (outlines, matrix, light, ids) = small_model();
    let selected = vec![ids[0]];
    let (whole, apart) = (
        crate::SceneParts::of(&outlines),
        crate::SceneParts::shared(&outlines),
    );
    assert_eq!(apart.circles.len(), 2);
    assert_eq!(whole.objects, apart.objects);
    for shaded in [true, false] {
        let picture = |parts: &crate::SceneParts| Picture {
            lines: if shaded {
                parts.shaded_lines.clone()
            } else {
                parts.lines.clone()
            },
            faces: if shaded {
                whole.faces.clone()
            } else {
                Vec::new()
            },
            matrix,
            light,
            ghosted: false,
            hidden: false,
            hidden_lines: false,
            sketch: false,
            ground: 0.0,
        };
        let scene = |circles: &[crate::PlacedCircle]| Scene {
            objects: Arc::new(whole.objects.clone()),
            selection: Arc::new(selected.clone()),
            clips: Vec::new(),
            circles: Arc::new(circles.to_vec()),
        };
        let size = [96, 96];
        let chunk = super::CHUNK_BYTES;
        let (Ok(before), Ok(after)) = (
            draw_picture_in_chunks(picture(&whole), size, true, chunk, &scene(&[])),
            draw_picture_in_chunks(picture(&apart), size, true, chunk, &scene(&apart.circles)),
        ) else {
            eprintln!("no graphics adapter to draw with, so not checked");
            return;
        };
        let orange = |pixels: &[u8]| {
            pixels
                .chunks(4)
                .filter(|p| p[0] > 180 && p[2] < 120)
                .count()
        };
        assert!(orange(&before) > 20, "the selected circle shows");
        assert!(orange(&after).abs_diff(orange(&before)) <= 4);
        let differing = before
            .chunks(4)
            .zip(after.chunks(4))
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            differing <= 8,
            "shaded: {shaded}, {differing} pixels differ"
        );
    }
}

#[test]
fn the_selection_table_marks_only_the_selected_objects() {
    let objects = [10, 20, 30, 40];
    let table = table_bytes(1, &objects, &[20, 40, 99]);
    assert_eq!(table.len(), PICK_ROW as usize);
    assert_eq!(&table[..5], &[0, 1, 0, 1, 0]);
    assert_eq!(table.iter().map(|&b| usize::from(b)).sum::<usize>(), 2);
    // Nothing selected: all zeros; more rows: still one byte per object, in order.
    assert!(table_bytes(1, &objects, &[]).iter().all(|&b| b == 0));
    let big: Vec<u64> = (0..5000).collect();
    let two_rows = table_bytes(2, &big, &[4999]);
    assert_eq!(two_rows.len(), 2 * PICK_ROW as usize);
    assert_eq!(two_rows[4999], 1);
}

/// #182: a sphere, which has no edges to draw, gets an outline at its rim from the depth
/// picture, and nothing inside it: the outline test never mistakes the curved surface itself
/// for a step back.
#[test]
fn a_sphere_is_outlined_at_its_rim_and_clean_inside() {
    let mut doc = curva_document::Document::default();
    doc.add_body(curva_kernel::Body::sphere([0.0; 3], 10.0).unwrap());
    let outlines = crate::Outlines::of(doc.state());
    // Seen straight down, so the rim on screen is exactly the sphere's radius.
    let mut camera = crate::StandardView::Top.camera();
    camera.frame([-12.0, -12.0, -10.0], [12.0, 12.0, 10.0]);
    let size = [128u32, 128];
    let picture = Picture {
        lines: crate::shaded_object_lines(&outlines, &[]),
        faces: crate::face_vertices(&outlines, &[]),
        matrix: camera.matrix(1.0),
        light: camera.forward().map(|v| v as f32),
        ghosted: false,
        hidden: false,
        hidden_lines: false,
        sketch: false,
        ground: 0.0,
    };
    let Ok(pixels) = draw_picture(picture, size, true) else {
        eprintln!("no graphics adapter to draw with, so not checked");
        return;
    };
    let red = |x: usize, y: usize| pixels[(y * 128 + x) * 4];
    let screen = |p: [f64; 3]| camera.project(p, [128.0, 128.0]).unwrap();
    let [cx, cy] = screen([0.0; 3]);
    let rim = screen([10.0, 0.0, 0.0])[0] - cx;
    // Within the last few pixels inside the rim, on every side: the outline's ink. (The
    // meshed sphere lies a fraction of a pixel inside the true one. Pictures store colours
    // converted for display, so the near-black ink reads about 80, the lit surface 180 up.)
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        let darkest = (1..=4)
            .map(|inside| {
                let at = rim - f64::from(inside);
                red((cx + dx * at) as usize, (cy + dy * at) as usize)
            })
            .min()
            .unwrap_or(255);
        assert!(darkest <= 90, "rim towards {dx},{dy}: {darkest}");
    }
    // Inside, well clear of the rim, no ink at all: the curved surface is never taken for a
    // step back.
    for y in 0..128 {
        for x in 0..128 {
            let from = (x as f64 + 0.5 - cx).hypot(y as f64 + 0.5 - cy);
            if from < rim - 5.0 {
                assert!(red(x, y) > 120, "ink inside at {x},{y}: {}", red(x, y));
            }
        }
    }
}

/// Spec `display-on-bevy.md` §2, the *Far* check (#225): a small model drawn 500 km from the
/// origin, seen the same way, gives the same picture as at the origin. Single precision steps by
/// 32 mm out there, so positions measured from the origin would come out jagged; a window scene
/// measures them from a point near the model instead ([`crate::SceneParts::origin`]).
#[test]
fn far_from_the_origin_draws_as_at_the_origin() {
    let picture_at = |offset: [f64; 3]| {
        let at = |p: [f64; 3]| [p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]];
        let mut doc = curva_document::Document::default();
        doc.add(curva_kernel::Curve::circle(at([0.0, 0.0, 0.0]), 5.0).unwrap());
        doc.add(curva_kernel::Curve::line(at([-6.0, -6.0, 0.0]), at([6.0, 4.5, 0.0])).unwrap());
        let outlines = crate::Outlines::of(doc.state());
        let parts = crate::SceneParts::shared(&outlines);
        let mut camera = crate::Camera::default();
        camera.frame(at([-6.0, -6.0, -1.0]), at([6.0, 6.0, 1.0]));
        let camera = camera.measured_from(parts.origin);
        let picture = Picture {
            lines: parts.lines.clone(),
            faces: Vec::new(),
            matrix: camera.matrix(1.0),
            light: camera.forward().map(|v| v as f32),
            ghosted: false,
            hidden: false,
            hidden_lines: false,
            sketch: false,
            ground: 0.0,
        };
        let scene = Scene {
            objects: Arc::new(parts.objects.clone()),
            circles: Arc::new(parts.circles.clone()),
            ..Scene::default()
        };
        draw_picture_in_chunks(picture, [96, 96], true, super::CHUNK_BYTES, &scene)
    };
    let (Ok(near), Ok(far)) = (picture_at([0.0; 3]), picture_at([5.0e8, 5.0e8, 0.0])) else {
        eprintln!("no graphics adapter to draw with, so not checked");
        return;
    };
    let lit = near.chunks(4).filter(|p| p[0] > 100).count();
    assert!(lit > 100, "the model shows: {lit} pixels");
    let differing = near
        .chunks(4)
        .zip(far.chunks(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(differing <= 8, "{differing} pixels differ");
}

/// The Hidden display mode (#164), as Rhino 8 draws it: white paper, a face flat in the same
/// white hiding the line behind it, and the line in front in black ink though it was given in
/// red.
#[test]
fn the_hidden_mode_draws_black_ink_on_white_paper() {
    let corner = |x: f32, y: f32| MeshVertex {
        position: [x, y, 0.5],
        normal: [0.0, 0.0, 1.0],
        colour: [0.3, 0.5, 0.7],
        object: NO_OBJECT,
    };
    let segment = |y: f32, depth: f32, colour: [f32; 3]| {
        [-0.6, 0.6].map(|x| Vertex {
            position: [x, y, depth],
            colour,
            width: -2.0,
            object: NO_OBJECT,
        })
    };
    // In front of the face across the middle, and behind it lower down.
    let mut lines = segment(0.0, 0.2, [1.0, 0.0, 0.0]).to_vec();
    lines.extend(segment(-0.4, 0.8, [1.0, 1.0, 1.0]));
    let picture = Picture {
        lines,
        faces: vec![corner(-0.9, -0.9), corner(0.9, -0.9), corner(0.0, 0.9)],
        matrix: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        light: [0.0, 0.0, 1.0],
        ghosted: false,
        hidden: true,
        hidden_lines: false,
        sketch: false,
        ground: 0.0,
    };
    let pixels = match draw_picture(picture, SIZE, true) {
        Ok(pixels) => pixels,
        Err(error) => {
            eprintln!("no graphics adapter to draw with, so not checked: {error}");
            return;
        }
    };
    let width = SIZE[0] as usize;
    // Rows from the top: clip-space y = 0 is the middle row, y = -0.4 is 0.7 of the way down.
    let at = |x: usize, y: usize| {
        let i = (y * width + x) * 4;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    };
    let (middle, low) = (SIZE[1] as usize / 2, SIZE[1] as usize * 7 / 10);
    assert!(at(2, 2).iter().all(|&c| c > 245), "paper: {:?}", at(2, 2));
    assert!(
        at(32, middle).iter().all(|&c| c < 40),
        "ink: {:?}",
        at(32, middle)
    );
    // On the face, away from the lines: white too, so faces and paper meet with no step.
    assert!(
        at(32, middle + 6).iter().all(|&c| c > 245),
        "face: {:?}",
        at(32, middle + 6)
    );
    // The line behind the face is hidden by it.
    assert!(
        at(32, low).iter().all(|&c| c > 245),
        "hidden line: {:?}",
        at(32, low)
    );
}

/// The Technical display mode (#164), as Rhino 8 draws it: as Hidden, but a line behind a face
/// is drawn dotted, black dots with paper between them.
#[test]
fn the_technical_mode_dots_the_lines_faces_hide() {
    let corner = |x: f32, y: f32| MeshVertex {
        position: [x, y, 0.5],
        normal: [0.0, 0.0, 1.0],
        colour: [0.3, 0.5, 0.7],
        object: NO_OBJECT,
    };
    // A line behind the face, across its lower part.
    let lines = [-0.6_f32, 0.6].map(|x| Vertex {
        position: [x, -0.4, 0.8],
        colour: [1.0, 1.0, 1.0],
        width: -2.0,
        object: NO_OBJECT,
    });
    let picture = Picture {
        lines: lines.to_vec(),
        faces: vec![corner(-0.9, -0.9), corner(0.9, -0.9), corner(0.0, 0.9)],
        matrix: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        light: [0.0, 0.0, 1.0],
        hidden: true,
        hidden_lines: true,
        ..Picture::default()
    };
    let pixels = match draw_picture(picture, SIZE, true) {
        Ok(pixels) => pixels,
        Err(error) => {
            eprintln!("no graphics adapter to draw with, so not checked: {error}");
            return;
        }
    };
    let width = SIZE[0] as usize;
    // The line's row, 0.7 of the way down; along the middle stretch of the line.
    let row = SIZE[1] as usize * 7 / 10;
    let dark = |x: usize| pixels[(row * width + x) * 4] < 80;
    let along: Vec<bool> = (20..44).map(dark).collect();
    assert!(along.iter().any(|&d| d), "no dots: {along:?}");
    assert!(along.iter().any(|&d| !d), "not dotted: {along:?}");
}

/// The Clay display mode (#551): a coloured face comes out white, lit, and a line given in
/// red comes out in black ink, as in SketchUp's default look.
#[test]
fn the_clay_mode_draws_white_faces_and_black_lines() {
    let corner = |x: f32, y: f32| MeshVertex {
        position: [x, y, 0.5],
        normal: [0.0, 0.0, 1.0],
        colour: [0.2, 0.5, 0.9],
        object: NO_OBJECT,
    };
    let lines = [-0.6_f32, 0.6].map(|x| Vertex {
        position: [x, 0.0, 0.2],
        colour: [1.0, 0.0, 0.0],
        width: -2.0,
        object: NO_OBJECT,
    });
    let picture = Picture {
        lines: lines.to_vec(),
        faces: vec![corner(-0.9, -0.9), corner(0.9, -0.9), corner(0.0, 0.9)],
        matrix: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        light: [0.0, 0.0, 1.0],
        sketch: true,
        ..Picture::default()
    };
    let pixels = match draw_picture(picture, SIZE, true) {
        Ok(pixels) => pixels,
        Err(error) => {
            eprintln!("no graphics adapter to draw with, so not checked: {error}");
            return;
        }
    };
    let width = SIZE[0] as usize;
    let at = |x: usize, y: usize| {
        let i = (y * width + x) * 4;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    };
    let middle = SIZE[1] as usize / 2;
    let face = at(32, middle + 6);
    assert!(
        face[0] > 150 && face[0] == face[1] && face[1] == face[2],
        "face: {face:?}"
    );
    assert!(
        at(32, middle).iter().all(|&c| c < 40),
        "ink: {:?}",
        at(32, middle)
    );
}
