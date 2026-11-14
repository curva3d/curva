//! Adding Curva to a Linux desktop's applications when it runs from an AppImage (spec
//! `feedback-0.4.2.md` §2). An AppImage installs nothing, so the dock has no `.desktop` file to
//! match the window to and shows a generic icon. As AppImageLauncher and Gear Lever do, Curva
//! writes its `.desktop` file, pointing at the AppImage, and its icons into `~/.local/share`,
//! and can remove them again.

use std::path::{Path, PathBuf};

/// The app id the window carries and the `.desktop` file names, so the two match.
pub(crate) const APP_ID: &str = "curva";

/// The `.desktop` file for an AppImage at `appimage`.
pub(crate) fn desktop_entry(appimage: &Path) -> String {
    let exec = appimage.display().to_string().replace('"', "\\\"");
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Curva\n\
         Comment=Design-oriented 3D modelling\n\
         Exec=\"{exec}\" %F\n\
         TryExec={exec}\n\
         Icon={APP_ID}\n\
         StartupWMClass={APP_ID}\n\
         Terminal=false\n\
         Categories=Graphics;3DGraphics;Engineering;\n"
    )
}

/// Where the desktop keeps a user's own applications and icons: `$XDG_DATA_HOME`, or
/// `~/.local/share`.
pub(crate) fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
}

/// The files adding Curva writes under `data`, as (path, contents).
pub(crate) fn files(data: &Path, appimage: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = vec![(
        data.join("applications").join(format!("{APP_ID}.desktop")),
        desktop_entry(appimage).into_bytes(),
    )];
    for (size, png) in ICONS {
        files.push((
            data.join(format!("icons/hicolor/{size}x{size}/apps/{APP_ID}.png")),
            png.to_vec(),
        ));
    }
    files.push((
        data.join(format!("icons/hicolor/scalable/apps/{APP_ID}.svg")),
        SVG.to_vec(),
    ));
    files
}

/// Whether Curva is in the applications under `data`.
pub(crate) fn added(data: &Path) -> bool {
    data.join("applications")
        .join(format!("{APP_ID}.desktop"))
        .exists()
}

/// Adds Curva to the applications under `data`, for the AppImage at `appimage`.
pub(crate) fn add(data: &Path, appimage: &Path) -> std::io::Result<()> {
    for (path, contents) in files(data, appimage) {
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder)?;
        }
        std::fs::write(path, contents)?;
    }
    Ok(())
}

/// Removes what [`add`] wrote under `data`.
pub(crate) fn remove(data: &Path) -> std::io::Result<()> {
    for (path, _) in files(data, Path::new("")) {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

/// The AppImage Curva runs from, on Linux: the AppImage runtime names it in `$APPIMAGE`.
pub(crate) fn appimage() -> Option<PathBuf> {
    if cfg!(target_os = "linux") {
        std::env::var_os("APPIMAGE").map(PathBuf::from)
    } else {
        None
    }
}

/// Curva's icons, in the sizes docks ask for. Only Linux needs them.
#[cfg(target_os = "linux")]
const ICONS: [(u32, &[u8]); 4] = [
    (
        48,
        include_bytes!("../../../packaging/icons/hicolor/48x48/apps/curva.png"),
    ),
    (
        128,
        include_bytes!("../../../packaging/icons/hicolor/128x128/apps/curva.png"),
    ),
    (
        256,
        include_bytes!("../../../packaging/icons/hicolor/256x256/apps/curva.png"),
    ),
    (
        512,
        include_bytes!("../../../packaging/icons/hicolor/512x512/apps/curva.png"),
    ),
];
#[cfg(not(target_os = "linux"))]
const ICONS: [(u32, &[u8]); 0] = [];

#[cfg(target_os = "linux")]
const SVG: &[u8] = include_bytes!("../../../packaging/icons/curva.svg");
#[cfg(not(target_os = "linux"))]
const SVG: &[u8] = &[];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_runs_the_appimage_and_matches_the_window() {
        let entry = desktop_entry(Path::new("/home/me/Apps/Curva 0.4.3.AppImage"));
        assert!(entry.contains("Exec=\"/home/me/Apps/Curva 0.4.3.AppImage\" %F\n"));
        assert!(entry.contains("StartupWMClass=curva\n"));
        assert!(entry.contains("Icon=curva\n"));
    }

    #[test]
    fn adding_and_removing_leaves_nothing_behind() {
        let data = std::env::temp_dir().join(format!("curva-desktop-{}", std::process::id()));
        let appimage = Path::new("/opt/Curva.AppImage");
        assert!(!added(&data));
        add(&data, appimage).unwrap();
        assert!(added(&data));
        for (path, _) in files(&data, appimage) {
            assert!(path.exists(), "{} was not written", path.display());
        }
        remove(&data).unwrap();
        assert!(!added(&data));
        remove(&data).unwrap();
        std::fs::remove_dir_all(&data).unwrap();
    }
}
