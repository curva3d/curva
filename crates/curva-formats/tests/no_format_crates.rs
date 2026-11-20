//! The commands and the interface reach file formats only through the model's list (spec
//! `docs/specs/file-formats.md` §4, step 5): neither depends on a format crate. Tests may, as
//! dev-dependencies, to read back what was written.

/// The crates that read or write a file format, each joining the list through `curva-formats`.
const FORMAT_CRATES: &[&str] = &[
    "curva-3dm",
    "curva-dxf",
    "curva-skp",
    "curva-meshfiles",
    "curva-step",
    "curva-pdf",
];

/// The names in a manifest's `[dependencies]` section.
fn dependencies(manifest: &str) -> Vec<String> {
    let mut inside = false;
    let mut names = Vec::new();
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == "[dependencies]";
        } else if inside
            && let Some((name, _)) = line.split_once('=')
            && !name.trim().starts_with('#')
        {
            names.push(name.trim().to_owned());
        }
    }
    names
}

#[test]
fn the_commands_and_the_interface_depend_on_no_format_crate() {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for user in ["curva-commands", "curva-app"] {
        let manifest = std::fs::read_to_string(crates.join(user).join("Cargo.toml")).unwrap();
        let named: Vec<String> = dependencies(&manifest)
            .into_iter()
            .filter(|name| FORMAT_CRATES.contains(&name.as_str()))
            .collect();
        assert!(
            named.is_empty(),
            "{user} depends on {named:?}: add formats to curva-formats instead"
        );
    }
}

#[test]
fn every_format_crate_is_on_the_list() {
    let manifest = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
    )
    .unwrap();
    let ours = dependencies(&manifest);
    for format in FORMAT_CRATES {
        assert!(
            ours.iter().any(|name| name == format),
            "{format} is not gathered"
        );
    }
}
