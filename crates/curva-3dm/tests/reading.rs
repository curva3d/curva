//! Reading Rhino files: openNURBS' example files (spec `docs/specs/3dm-files.md` §4).

use curva_document::{Linetype, Shape};

fn open(name: &str) -> (curva_document::Document, curva_3dm::import::Report) {
    let path = format!("{}/tests/files/{name}", env!("CARGO_MANIFEST_DIR"));
    let data = std::fs::read(path).unwrap();
    let model = curva_3dm::read::read(&data).unwrap();
    curva_3dm::import::open(&model)
}

#[test]
fn every_example_file_opens_with_a_report() {
    let dir = format!("{}/tests/files", env!("CARGO_MANIFEST_DIR"));
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let (doc, report) = open(&name);
        println!(
            "{name}: {} objects; {}",
            doc.state().objects.len(),
            report.summary()
        );
    }
}

#[test]
fn a_line_of_zero_length_is_listed_as_drawing_nothing() {
    let path = format!("{}/tests/files/v7_my_curves.3dm", env!("CARGO_MANIFEST_DIR"));
    let mut model = curva_3dm::read::read(&std::fs::read(path).unwrap()).unwrap();
    let count = model.objects.len();
    let at = [5.0, 6.0, 0.0];
    model.objects[0].geometry = curva_3dm::model::Geometry::Curve(curva_3dm::model::Curve::Line {
        ends: [at, at],
        domain: [0.0, 1.0],
    });
    let (doc, report) = curva_3dm::import::open(&model);
    assert_eq!(
        report.skipped,
        vec![(curva_3dm::import::ZERO_LENGTH.to_owned(), 1)]
    );
    assert_eq!(doc.state().objects.len(), count - 1);
}

#[test]
fn curves_come_in_with_their_names_layers_and_sizes() {
    let (doc, report) = open("v7_my_curves.3dm");
    assert!(report.skipped.is_empty(), "{}", report.summary());
    let names: Vec<&str> = doc
        .state()
        .objects
        .iter()
        .map(|o| o.name.as_str())
        .collect();
    assert!(names.contains(&"radius 3 circle"), "{names:?}");
    // The file is in inches, which Curva keeps (spec `feedback-0.4.2.md` §3): the circle's
    // radius is 3.
    assert_eq!(doc.state().unit, curva_document::Unit::Inches);
    let circle = doc
        .state()
        .objects
        .iter()
        .find(|o| o.name == "radius 3 circle")
        .unwrap();
    let Shape::Curve(c) = &circle.shape else {
        panic!()
    };
    let length = c.length().unwrap();
    assert!(
        (length / (std::f64::consts::TAU * 3.0) - 1.0).abs() < 1e-6,
        "{length}"
    );
    let layers: Vec<&str> = doc.layers().iter().map(|l| l.name.as_str()).collect();
    assert!(layers.contains(&"blue circles"), "{layers:?}");
}

#[test]
fn a_brep_comes_in_as_one_body() {
    let (doc, report) = open("v7_my_brep.3dm");
    assert_eq!(doc.state().objects.len(), 1, "{}", report.summary());
    let Shape::Body(body) = &doc.state().objects[0].shape else {
        panic!()
    };
    assert_eq!(body.face_count().unwrap(), 6);
}

#[test]
fn a_trimmed_surface_comes_in() {
    let (doc, report) = open("v7_my_trimmed_surface.3dm");
    assert_eq!(doc.state().objects.len(), 1, "{}", report.summary());
}

#[test]
fn rhino_5_solids_come_in() {
    let (doc, report) = open("v5_teacup.3dm");
    assert_eq!(doc.state().objects.len(), 6, "{}", report.summary());
}

#[test]
fn extrusions_come_in_as_bodies() {
    let (doc, report) = open("v5_extrusions.3dm");
    let bodies = doc
        .state()
        .objects
        .iter()
        .filter(|o| matches!(o.shape, Shape::Body(_)))
        .count();
    assert_eq!(bodies, 7, "{}", report.summary());
}

/// Meshes and SubDs come in as objects since the document holds them (#266); point clouds are
/// still left out.
#[test]
fn meshes_and_subds_come_in_and_point_clouds_are_listed_as_left_out() {
    let (doc, report) = open("v7_my_mesh.3dm");
    assert!(report.skipped.is_empty(), "{}", report.summary());
    let meshes: Vec<_> = doc
        .state()
        .objects
        .iter()
        .filter_map(|o| o.shape.mesh())
        .collect();
    assert_eq!(meshes.len(), 1);
    assert!(meshes[0].faces().count() > 0);
    let (doc, report) = open("v8_subds.3dm");
    assert!(report.skipped.is_empty(), "{}", report.summary());
    let subds = doc
        .state()
        .objects
        .iter()
        .filter(|o| o.shape.subd().is_some())
        .count();
    assert_eq!(subds, 3);
    let (doc, report) = open("v7_my_points.3dm");
    assert_eq!(doc.state().objects.len(), 2);
    assert_eq!(report.skipped, vec![("point cloud".to_owned(), 1)]);
}

/// Damaged files are refused or read in part, never a crash (spec §4): each example file cut
/// short at many points, and with bytes changed through it.
#[test]
fn damaged_files_never_crash_the_reader() {
    let dir = format!("{}/tests/files", env!("CARGO_MANIFEST_DIR"));
    for entry in std::fs::read_dir(dir).unwrap() {
        let data = std::fs::read(entry.unwrap().path()).unwrap();
        // Fewer cases on the large files, which take long to read in a debug build.
        let large = data.len() >= 200_000;
        let step = (data.len() / if large { 23 } else { 97 }).max(1);
        for cut in (0..data.len()).step_by(step) {
            let _ = curva_3dm::read::read(&data[..cut]);
        }
        // A simple generator, the same every run, so a failure can be repeated.
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        // Bringing a model into Curva goes through the kernel, which is slow on large damaged
        // models: the small files, every fortieth case.
        let import = !large;
        for case in 0..if large { 30 } else { 200 } {
            let mut damaged = data.clone();
            for _ in 0..4 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let at = 32 + (seed as usize) % (damaged.len() - 32);
                damaged[at] = (seed >> 32) as u8;
            }
            if let Ok(model) = curva_3dm::read::read(&damaged)
                && import
                && case % 40 == 0
            {
                // What does read must also come into Curva without a crash.
                let _ = curva_3dm::import::open(&model);
            }
        }
    }
}

#[test]
fn rhinos_hatches_come_in_with_their_patterns() {
    use curva_document::annotation::Annotation;
    let (doc, report) = open("v7_rhino_logo_annotations.3dm");
    let hatches: Vec<_> = doc
        .state()
        .objects
        .iter()
        .filter_map(|o| match &o.shape {
            Shape::Annotation(Annotation::Hatch(h)) => Some(h.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(hatches.len(), 7, "{}", report.summary());
    for h in &hatches {
        assert!(!h.loops.is_empty());
        assert!(h.scale > 0.0);
        let known = doc
            .state()
            .patterns()
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(&h.pattern));
        assert!(known, "{} is a pattern Curva has", h.pattern);
        // Their outlines lie in their plane.
        let z = [
            h.x[1] * h.y[2] - h.x[2] * h.y[1],
            h.x[2] * h.y[0] - h.x[0] * h.y[2],
            h.x[0] * h.y[1] - h.x[1] * h.y[0],
        ];
        for p in h.loops[0].sample().unwrap() {
            let off: f64 = (0..3).map(|i| (p[i] - h.base[i]) * z[i]).sum();
            assert!(off.abs() < 1e-6, "{off}");
        }
    }
}

#[test]
fn rhinos_notes_come_in_with_their_words_height_and_alignment() {
    use curva_document::annotation::{Annotation, LINE_SPACING, TextAlign};
    // Made in Rhino 8: "Hall" 3.5 high, left and bottom at (10, 20); "Ground floor / Plan"
    // 5 high, centred at the bottom, turned 30 degrees at (50, 0); "Kitchen" in the Default
    // style (1 high, left and top) at (0, -30).
    let (doc, report) = open("v8_texts.3dm");
    let notes: Vec<_> = doc
        .state()
        .objects
        .iter()
        .filter_map(|o| match &o.shape {
            Shape::Annotation(Annotation::Text(t)) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(notes.len(), 3, "{}", report.summary());
    let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9);
    let find = |words: &str| notes.iter().find(|n| n.content == words).unwrap();
    let hall = find("Hall");
    assert!((hall.height - 3.5).abs() < 1e-12);
    assert_eq!(hall.align, TextAlign::Left);
    assert!(near(hall.origin, [10.0, 20.0, 0.0]), "{:?}", hall.origin);
    let ground = find("Ground floor\nPlan");
    assert!((ground.height - 5.0).abs() < 1e-12);
    assert_eq!(ground.align, TextAlign::Centre);
    let (s, c) = (30f64.to_radians().sin(), 30f64.to_radians().cos());
    assert!(near(ground.x, [c, s, 0.0]));
    // Aligned at the bottom of its second line, so its first line sits one line higher.
    let up = 5.0 * LINE_SPACING;
    assert!(
        near(ground.origin, [50.0 - s * up, c * up, 0.0]),
        "{:?}",
        ground.origin
    );
    let kitchen = find("Kitchen");
    assert!((kitchen.height - 1.0).abs() < 1e-12);
    // Aligned at its top, so its baseline is one text height lower.
    assert!(
        near(kitchen.origin, [0.0, -31.0, 0.0]),
        "{:?}",
        kitchen.origin
    );
}

#[test]
fn rhinos_linear_dimensions_come_in_measuring_the_same() {
    use curva_document::annotation::Annotation;
    // Made in Rhino 8: an aligned dimension from (0, 50) to (100, 50), its line through
    // (50, 60).
    let (doc, _) = open("v8_texts.3dm");
    let dims: Vec<_> = doc
        .state()
        .objects
        .iter()
        .filter_map(|o| match &o.shape {
            Shape::Annotation(Annotation::Dimension(d)) => Some(d.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(dims.len(), 1);
    let d = &dims[0];
    assert!((d.value() - 100.0).abs() < 1e-9, "{}", d.value());
    let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9);
    assert!(near(d.first, [0.0, 50.0, 0.0]) && near(d.second, [100.0, 50.0, 0.0]));
    assert!((d.line[1] - 60.0).abs() < 1e-9, "{:?}", d.line);
    assert_eq!(d.text, None);
}

#[test]
fn rhinos_layouts_come_in_with_their_details_and_sheet_drawing() {
    // Made in Rhino 8: an A3 layout "Plan" with a 1:50 top detail from (20, 20) to (220, 170)
    // looking at (5000, 0), and a rectangle drawn on the sheet; a 10 m line in the model.
    let (doc, report) = open("v8_layout.3dm");
    let state = doc.state();
    assert_eq!(state.layouts.len(), 1, "{}", report.summary());
    let sheet = &state.layouts[0];
    assert_eq!(sheet.name, "Plan");
    assert_eq!(sheet.size, [420.0, 297.0]);
    assert_eq!(sheet.details.len(), 1);
    let detail = &sheet.details[0];
    assert_eq!(detail.projection, "Top");
    assert!((detail.scale.unwrap() - 0.02).abs() < 1e-12);
    assert!((detail.target[0] - 5000.0).abs() < 1e-6);
    let frame = state.objects.iter().find(|o| o.id == detail.frame).unwrap();
    assert_eq!(frame.space, Some(sheet.id));
    let (low, high) = frame.shape.bounds().unwrap();
    assert!((low[0] - 20.0).abs() < 1e-6 && (high[1] - 170.0).abs() < 1e-6);
    let on_sheet: Vec<_> = state
        .objects
        .iter()
        .filter(|o| o.space == Some(sheet.id) && o.id != detail.frame)
        .collect();
    assert_eq!(on_sheet.len(), 1, "the rectangle");
    let (low, high) = on_sheet[0].shape.bounds().unwrap();
    assert!((low[0] - 250.0).abs() < 1e-6 && (high[0] - 400.0).abs() < 1e-6);
    let in_model: Vec<_> = state.objects.iter().filter(|o| o.space.is_none()).collect();
    assert_eq!(in_model.len(), 1, "the line");
    assert!((in_model[0].curve().unwrap().length().unwrap() - 10_000.0).abs() < 1e-6);
}

#[test]
fn rhinos_layout_user_text_comes_in_with_its_fields() {
    // Made in Rhino 8: an A1 layout with user text "Sheet title" and "Drawn by", and two notes
    // reading them, one by key alone and one naming the page by its viewport's id.
    let (doc, report) = open("v8_layout_user_text.3dm");
    let state = doc.state();
    assert_eq!(state.layouts.len(), 1, "{}", report.summary());
    let sheet = &state.layouts[0];
    assert_eq!(
        sheet.user_text.get("Sheet title").map(String::as_str),
        Some("Ground floor plan")
    );
    assert_eq!(
        sheet.user_text.get("Drawn by").map(String::as_str),
        Some("LP")
    );
    let mut shown: Vec<String> = state
        .objects
        .iter()
        .filter_map(|o| match &o.shape {
            curva_document::Shape::Annotation(curva_document::annotation::Annotation::Text(t)) => {
                Some(curva_document::fields::resolve(
                    &t.content,
                    state,
                    o.space,
                    &curva_document::fields::FieldContext::default(),
                ))
            }
            _ => None,
        })
        .collect();
    shown.sort();
    assert_eq!(shown, ["Ground floor plan", "LP"]);
}

#[test]
fn rhinos_linetypes_come_in_on_layers_and_objects() {
    // Made in Rhino 8: layers with Dashed, Hidden and a custom "Wall dash" linetype; lines by
    // layer, one with Border and one with "Wall dash" of their own.
    let (doc, report) = open("v8_linetypes.3dm");
    let state = doc.state();
    let layer = |n: &str| state.layers.iter().find(|l| l.name == n).unwrap().linetype;
    assert_eq!(layer("Dashed layer"), Linetype::Dashed);
    assert_eq!(layer("Hidden layer"), Linetype::Hidden);
    let wall = state
        .linetype_named("Wall dash")
        .expect("the document's own");
    assert_eq!(layer("Wall layer"), wall);
    assert_eq!(state.linetype_pattern(wall), [8.0, 2.0, 1.0, 2.0]);
    let object = |n: &str| state.objects.iter().find(|o| o.name == n).unwrap().linetype;
    assert_eq!(object("by layer"), Linetype::ByLayer);
    assert_eq!(object("border by object"), Linetype::Border);
    assert_eq!(object("wall dash by object"), wall);
    assert!(report.skipped.is_empty(), "{}", report.summary());
}

#[test]
fn rhinos_annotation_style_comes_in_with_its_looks() {
    // Made in Rhino 8, in millimetres: a style "Sections" in Times New Roman, text 5, arrows 3,
    // extension line offset 2 and extension 2.5, text gap 1.2, three decimals; a 4 m aligned
    // dimension and a note in it.
    let (doc, report) = open("v8_annotation_style.3dm");
    let state = doc.state();
    let style = state
        .annotation_styles
        .iter()
        .find(|s| s.name == "Sections")
        .unwrap_or_else(|| panic!("the style: {}", report.summary()));
    assert_eq!(style.font.as_deref(), Some("Times New Roman"));
    assert!((style.text_height - 5.0).abs() < 1e-9);
    assert_eq!(style.arrow_size, Some(3.0));
    assert_eq!(style.extension_offset, Some(2.0));
    assert_eq!(style.extension_extension, Some(2.5));
    assert_eq!(style.text_gap, Some(1.2));
    assert_eq!(style.linear_resolution, Some(3));
    let annotation: Vec<u64> = state
        .objects
        .iter()
        .filter_map(|o| match &o.shape {
            Shape::Annotation(a) => Some(a.style()),
            _ => None,
        })
        .collect();
    assert_eq!(
        annotation,
        [style.id, style.id],
        "the dimension and the note"
    );
}

#[test]
fn rhino_5_annotation_styles_come_in_as_rhino_reads_them() {
    // Rhino 8 saved as a Rhino 5 file, in millimetres: a style "Wide" in Courier New, text 5,
    // arrows 3, extension line extension 2 and offset 1.5, text gap 0.8, three decimals; a note,
    // an aligned dimension and a leader in it, and a dimension in "Default". Rhino 8 reads the
    // file back with the same style on the dimensions and the leader, and the note on
    // "Default" (keeping its own height).
    let (doc, report) = open("v5_styles.3dm");
    assert!(report.skipped.is_empty(), "{}", report.summary());
    let state = doc.state();
    let style = |name: &str| {
        state
            .annotation_styles
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("the style {name}: {}", report.summary()))
    };
    let wide = style("Wide");
    assert_eq!(wide.font.as_deref(), Some("Courier New"));
    assert!((wide.text_height - 5.0).abs() < 1e-9);
    assert_eq!(wide.arrow_size, Some(3.0));
    assert_eq!(wide.extension_extension, Some(2.0));
    assert_eq!(wide.extension_offset, Some(1.5));
    assert_eq!(wide.text_gap, Some(0.8));
    assert_eq!(wide.linear_resolution, Some(3));
    let default = style("Default");
    assert_eq!(default.font.as_deref(), Some("Arial"));
    let styles: Vec<u64> = state
        .objects
        .iter()
        .filter_map(|o| match &o.shape {
            Shape::Annotation(a) => Some(a.style()),
            _ => None,
        })
        .collect();
    assert_eq!(styles, [default.id, wide.id, default.id, wide.id]);
}

#[test]
fn a_layer_rhino_turned_off_in_one_detail_comes_in_off_in_it() {
    // Made in Rhino 8: layout "Plan" with details "Left" (20 to 170 mm) and "Right" (200 to
    // 350 mm), and a layer "Furniture" turned off in the left one only.
    let (doc, report) = open("v8_detail_layers.3dm");
    let state = doc.state();
    let furniture = state
        .layers
        .iter()
        .find(|l| l.name == "Furniture")
        .unwrap()
        .id;
    let plan = state
        .layouts
        .iter()
        .find(|l| l.name == "Plan")
        .unwrap_or_else(|| panic!("the layout: {}", report.summary()));
    assert_eq!(plan.details.len(), 2);
    let left_of = |frame: u64| {
        state
            .objects
            .iter()
            .find(|o| o.id == frame)
            .and_then(|o| o.shape.bounds())
            .map(|(low, _)| low[0])
            .unwrap()
    };
    for detail in &plan.details {
        let hidden = detail.hidden_layers.contains(&furniture);
        assert_eq!(
            hidden,
            left_of(detail.frame) < 100.0,
            "off in the left detail only"
        );
    }
}

#[test]
fn rhinos_clipping_plane_comes_in_cutting_its_views() {
    // Made in Rhino 8: a 2000 by 1500 clipping plane at x = 500 facing +x (keeping what is on
    // the +x side), cutting the Front view and a detail on layout "Plan".
    use curva_document::clipping::ClipView;
    let (doc, report) = open("v8_clipping_plane.3dm");
    let state = doc.state();
    assert_eq!(state.clipping_planes.len(), 1, "{}", report.summary());
    let clip = &state.clipping_planes[0];
    let keep = doc
        .planes_cutting(ClipView::Viewport(2))
        .first()
        .map(|cut| cut.keep)
        .expect("it cuts Front, Curva's third viewport");
    assert!((keep[0] - 1.0).abs() < 1e-9, "keeps the +x side: {keep:?}");
    let detail = state
        .layouts
        .iter()
        .find(|l| l.name == "Plan")
        .and_then(|l| l.details.first())
        .unwrap()
        .frame;
    assert!(clip.views.contains(&ClipView::Detail(detail)));
    assert_eq!(clip.views.len(), 2);
    let frame = state.objects.iter().find(|o| o.id == clip.frame).unwrap();
    let (low, high) = frame.shape.bounds().unwrap();
    assert!((low[0] - 500.0).abs() < 1e-6 && (high[0] - 500.0).abs() < 1e-6);
    assert!((high[1] - low[1] - 2000.0).abs() < 1e-6 || (high[2] - low[2] - 2000.0).abs() < 1e-6);
}

#[test]
fn rhino_5_annotation_comes_in_as_rhino_reads_it() {
    use curva_document::annotation::{Annotation, DimensionKind, TextAlign};
    // Rhino 8 saved the same notes, dimensions and leader as a Rhino 5 file and a Rhino 8 one.
    // What Rhino 8 reads back from the Rhino 5 file is what Curva should: the aligned dimension
    // then measures along its plane (20, not 25), and the leader ends at its words, no landing.
    let annotations = |name: &str| {
        let (doc, report) = open(name);
        assert!(report.skipped.is_empty(), "{name}: {}", report.summary());
        doc.state()
            .objects
            .iter()
            .filter_map(|o| match &o.shape {
                Shape::Annotation(a) => Some(a.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let old = annotations("v5_annotation.3dm");
    let new = annotations("v8_annotation_for_v5.3dm");
    assert_eq!(old.len(), 8);
    let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-9);
    // Notes: the same words, place, height and alignment as from the Rhino 8 file.
    for i in 0..2 {
        let (Annotation::Text(a), Annotation::Text(b)) = (&old[i], &new[i]) else {
            panic!("{:?}", old[i]);
        };
        assert_eq!(a.content, b.content);
        assert!(near(a.origin, b.origin), "{a:?}");
        assert!((a.height - b.height).abs() < 1e-9);
        assert_eq!(a.align, b.align);
    }
    let Annotation::Text(centred) = &old[1] else {
        unreachable!()
    };
    assert_eq!(centred.align, TextAlign::Centre);
    let Annotation::Dimension(linear) = &old[2] else {
        panic!("{:?}", old[2]);
    };
    assert_eq!(linear.kind, DimensionKind::Horizontal);
    assert!(near(linear.second, [30.0, 10.0, 0.0]) && near(linear.line, [15.0, -8.0, 0.0]));
    assert!((linear.value() - 30.0).abs() < 1e-9);
    let Annotation::Dimension(aligned) = &old[3] else {
        panic!("{:?}", old[3]);
    };
    assert!((aligned.value() - 20.0).abs() < 1e-9, "{aligned:?}");
    let Annotation::Radial(radius) = &old[4] else {
        panic!("{:?}", old[4]);
    };
    assert!(!radius.diameter && near(radius.on_curve, [105.0, 0.0, 0.0]));
    assert!(
        near(radius.text_at, [116.0, 8.0, 0.0]),
        "Rhino's dimension line point"
    );
    let Annotation::Radial(diameter) = &old[5] else {
        panic!("{:?}", old[5]);
    };
    assert!(diameter.diameter);
    // The same angle, on the Rhino 5 file's own style (each document numbers its styles).
    let Annotation::Angle(old_angle) = &old[6] else {
        panic!("{:?}", old[6]);
    };
    assert_ne!(old_angle.style, 0, "the file's style, not Curva's default");
    assert_eq!(old[6], {
        let Annotation::Angle(mut b) = new[6].clone() else {
            panic!("{:?}", new[6]);
        };
        b.style = old_angle.style;
        Annotation::Angle(b)
    });
    let Annotation::Leader(leader) = &old[7] else {
        panic!("{:?}", old[7]);
    };
    assert_eq!(leader.content, "Oak floor");
    assert!(!leader.landing);
    assert_eq!(leader.points.len(), 4);
}

#[test]
fn ordinate_dimensions_rhino_wrote_come_in_where_rhino_draws_them() {
    use curva_document::annotation::{Annotation, OrdinateAxis};
    // Rhino 8: from base (10, 10), the point (40, 25) with its leader up to (45, 60) and kink
    // offsets 2 and 3, measuring X; and with its leader left to (0, 20), direction left unset.
    // Rhino gives X 30 with kinks at (40, 55) and (45, 58), and Y 15.
    let (doc, report) = open("v8_ordinate.3dm");
    assert!(report.skipped.is_empty(), "{}", report.summary());
    let got: Vec<_> = doc
        .state()
        .objects
        .iter()
        .filter_map(|o| match &o.shape {
            Shape::Annotation(Annotation::Ordinate(d)) => Some(d.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].axis, OrdinateAxis::X);
    assert!((got[0].value() - 30.0).abs() < 1e-9);
    assert_eq!(
        got[0].kink_points(0.0),
        [[40.0, 55.0, 0.0], [45.0, 58.0, 0.0]]
    );
    assert_eq!(got[1].axis, OrdinateAxis::Y);
    assert!((got[1].value() - 15.0).abs() < 1e-9);
}

/// Damaged files never crash Curva: every example file cut short at many points, and with
/// bytes overwritten at many places, is either read (with what could be read) or refused with
/// an error. A crash would end the test.
#[test]
fn damaged_files_are_refused_or_read_but_never_crash() {
    let dir = format!("{}/tests/files", env!("CARGO_MANIFEST_DIR"));
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    names.sort();
    for path in names {
        let data = std::fs::read(&path).unwrap();
        // Bringing a damaged model into Curva is slow in a debug build: the smaller files only.
        // A longer run over every file at 40 places found nothing more.
        if data.len() > 100_000 {
            continue;
        }
        let open = |bytes: &[u8]| {
            if let Ok(model) = curva_3dm::read::read(bytes) {
                let _ = curva_3dm::import::open(&model);
            }
        };
        // Cut short: at 8 places through the file.
        for k in 1..8 {
            open(&data[..data.len() * k / 8]);
        }
        // Overwritten: 8 runs of eight bytes, each filled with 0xFF, then with 0x00.
        for k in 1..8 {
            let at = data.len() * k / 8;
            for fill in [0xFF, 0x00] {
                let mut broken = data.clone();
                for b in broken.iter_mut().skip(at).take(8) {
                    *b = fill;
                }
                open(&broken);
            }
        }
    }
}
