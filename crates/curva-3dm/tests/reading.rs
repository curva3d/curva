//! Reading Rhino files: openNURBS' example files (spec `docs/specs/3dm-files.md` §4).

use curva_document::Shape;

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

#[test]
fn meshes_and_point_clouds_are_listed_as_left_out() {
    let (_, report) = open("v7_my_mesh.3dm");
    assert_eq!(report.skipped, vec![("mesh".to_owned(), 1)]);
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
