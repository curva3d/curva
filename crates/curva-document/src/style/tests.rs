use super::file::FIELDS;
use super::value::Value;
use super::*;

fn all_built_in() -> Vec<ViewStyle> {
    BUILT_IN
        .iter()
        .map(|name| ViewStyle::built_in(name).expect("a built-in style"))
        .collect()
}

#[test]
fn every_built_in_style_is_found_by_its_name() {
    for name in BUILT_IN {
        assert_eq!(
            ViewStyle::built_in(name).map(|s| s.name),
            Some(name.to_owned())
        );
    }
    assert_eq!(ViewStyle::built_in("Rendered"), None);
    assert_eq!(ViewStyle::default(), ViewStyle::shaded());
}

#[test]
fn every_built_in_style_reads_back_unchanged_from_its_file() {
    for style in all_built_in() {
        let read = ViewStyle::from_text(&style.to_text());
        assert_eq!(read.problems, Vec::new(), "{}", style.name);
        assert_eq!(read.style, style, "{}", style.name);
    }
}

#[test]
fn every_setting_has_its_own_key_and_reads_its_own_text() {
    let mut keys: Vec<_> = FIELDS.iter().map(|field| field.key).collect();
    let count = keys.len();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), count, "keys are unique");
    for style in all_built_in() {
        for field in FIELDS {
            let mut copy = style.clone();
            assert!(field.read(&mut copy, &field.write(&style)), "{}", field.key);
            assert_eq!(copy, style, "{}", field.key);
        }
    }
}

#[test]
fn a_file_changes_only_the_settings_it_gives() {
    let text = "name = Pale\n\n[NURBS.Faces]\nOpacity = 0.25\ncolour = #ff8000\n\n[outlines]\nshown = no\n";
    let read = ViewStyle::from_text(text);
    assert_eq!(read.problems, Vec::new());
    let mut expected = ViewStyle::shaded();
    expected.name = "Pale".into();
    expected.nurbs.faces.opacity = 0.25;
    expected.nurbs.faces.colour = ColourFrom::One(Colour([255, 128, 0]));
    expected.outlines.shown = false;
    assert_eq!(read.style, expected);
}

#[test]
fn settings_this_version_does_not_know_are_kept_and_written_back() {
    let mut text = ViewStyle::massing().to_text();
    text.push_str("\n[toon]\nbands = 3\n");
    let read = ViewStyle::from_text(&text);
    assert_eq!(read.problems, Vec::new());
    assert_eq!(
        read.style.unknown,
        vec![("toon.bands".to_owned(), "3".to_owned())]
    );
    let again = ViewStyle::from_text(&read.style.to_text());
    assert_eq!(again.style, read.style);
}

#[test]
fn a_bad_value_keeps_its_default_and_names_its_line() {
    let text = "[nurbs.faces]\nopacity = 1.5\ncolour = purple\n[outlines]\nwidth = 0.5\nthis line has no equals sign\n";
    let read = ViewStyle::from_text(text);
    let lines: Vec<_> = read.problems.iter().map(|p| p.line).collect();
    assert_eq!(lines, vec![2, 3, 6]);
    assert_eq!(read.problems[0].text, "opacity = 1.5");
    let shaded = ViewStyle::shaded();
    assert_eq!(
        read.style.nurbs.faces, shaded.nurbs.faces,
        "bad values change nothing"
    );
    assert_eq!(read.style.outlines.width, 0.5, "good ones still apply");
}

#[test]
fn anything_at_all_reads_without_failing() {
    for text in [
        "",
        "\u{0}\u{1}=",
        "[",
        "[]\n=",
        "= =",
        "name =",
        "[a.b.c]\nd.e = f",
    ] {
        let read = ViewStyle::from_text(text);
        assert!(read.style.unknown.len() <= 1, "{text:?}");
    }
}

#[test]
fn meshes_and_subd_follow_nurbs_until_set_apart() {
    let mut style = ViewStyle::shaded();
    style.nurbs.faces.opacity = 0.3;
    style.meshes.faces.opacity = 0.9;
    style.subd.outline = OutlineChoice::Off;
    assert_eq!(style.faces(FacedKind::Mesh).opacity, 0.3);
    assert_eq!(style.outline(FacedKind::Subd), OutlineChoice::AsStyle);
    style.meshes.same_as_nurbs = false;
    style.subd.same_as_nurbs = false;
    assert_eq!(style.faces(FacedKind::Mesh).opacity, 0.9);
    assert_eq!(style.outline(FacedKind::Subd), OutlineChoice::Off);
    assert_eq!(style.faces(FacedKind::Nurbs).opacity, 0.3);
}

#[test]
fn line_widths_read_in_their_three_forms() {
    assert_eq!(LineWidth::read("object"), Some(LineWidth::Object(1.0)));
    assert_eq!(LineWidth::read("Object x1.5"), Some(LineWidth::Object(1.5)));
    assert_eq!(LineWidth::read("0.35 mm"), Some(LineWidth::Fixed(0.35)));
    assert_eq!(LineWidth::read("0.35"), Some(LineWidth::Fixed(0.35)));
    for bad in ["-1 mm", "object 2", "thick", "999 mm", "object x-1"] {
        assert_eq!(LineWidth::read(bad), None, "{bad}");
    }
    for width in [
        LineWidth::Object(1.0),
        LineWidth::Object(0.5),
        LineWidth::Fixed(0.18),
    ] {
        assert_eq!(LineWidth::read(&width.write()), Some(width));
    }
}

#[test]
fn colours_are_six_hex_digits() {
    assert_eq!(Colour::read("#14141a"), Some(Colour::EDGE));
    assert_eq!(Colour::EDGE.write(), "#14141a");
    for bad in ["#12345", "123456", "#gggggg", "#1234567"] {
        assert_eq!(Colour::read(bad), None, "{bad}");
    }
    assert_eq!(Option::<Colour>::read("auto"), Some(None));
}

#[test]
fn the_built_in_styles_differ_as_their_display_modes_did() {
    let [wireframe, shaded, ghosted, hidden, technical, massing] = [
        "Wireframe",
        "Shaded",
        "Ghosted",
        "Hidden",
        "Technical",
        "Massing",
    ]
    .map(|name| ViewStyle::built_in(name).expect("built in"));
    assert!(!wireframe.nurbs.faces.shown && shaded.solid() && ghosted.see_through());
    assert_eq!(hidden.engine, Engine::Vector);
    assert!(technical.hidden_lines.shown && !hidden.hidden_lines.shown);
    assert_eq!(massing.lighting.occlusion, OcclusionLight::Sun);
    assert!(massing.shadows.shown && massing.outlines.width > shaded.outlines.width);
}
