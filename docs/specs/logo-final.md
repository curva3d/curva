# Spec: the final logo

**Status:** Approved by the owner in chat · 15 November 2026. Replaces the Drafting icon of 13 November.
**Brand guidelines:** the Curva design system on claude.ai (logo construction, colours, type, highlight rules). The owner has the link.

## 1. What changes

The icon becomes an **extruded C**: a C drawn as an arc in plan and pulled into a solid, lit from the top left, in greyscale. The light version (white tile, ink C) is the app icon. There's no orange in the logo any more, and no construction lines or control points.

Small sizes are drawn for their size, not shrunk: 48 px has less depth, 32 px is drawn on the 32 px grid with a 1 px side, and 16–24 px shows the face alone on the 16 px grid.

## 2. Files

Copy every file from this package over the repository, keeping the paths:

- `packaging/icons/curva.svg`: the 256 master (light).
- `packaging/icons/curva-256.png`, `packaging/icons/curva-1024.png`: from the master.
- `packaging/icons/curva.ico`: 16, 24, 32, 48, 64, 128 and 256 px, PNG entries. 16 and 24 come from the 16 px drawing, 32 from the 32 px drawing, 48 from the 48 px drawing, and 64 up from the master.
- `packaging/icons/hicolor/<size>/apps/curva.png`: 16, 24, 32, 48, 64, 128, 256 and 512 px, chosen the same way.
- `docs/design/brand/`: every logo file (icon light and dark at each size, monochrome icon, mark, one-colour mark, lockup, wordmark). These are the sources; don't edit them by hand.

The names and paths are the ones the build already uses (`build.rs`, `main.rs`, `desktop.rs`, `curva.iss`, `release.yml`), so no code changes. The macOS `.icns` is still made from `curva-1024.png` by the release workflow.

## 3. Also update

- `README.md`: the `<img>` alt text becomes: *Curva's icon: a C extruded into a solid, lit from the top left.*
- `STATUS.md`: replace the Drafting icon paragraph with one saying the extruded-C icon was chosen on 15 November 2026, and listing where it lives (as above).
- Move `docs/design/curva-logo-drafting*.svg`, `docs/design/logo-*variations.html` and `docs/specs/logo-*.md` (other than this spec) into `docs/design/archive/` and `docs/specs/archive/`, so nobody builds from them by mistake.
- Add a decision note in `docs/decisions/`, in plain language per `AGENTS.md`: what was chosen, why (simpler, recognisable, reads at 16 px, says "2D drives 3D"), and what was dropped (the Drafting construction and the orange points).

## 4. Check

- The Windows program, installer, title bar and taskbar show the new icon.
- The AppImage and the macOS app show it.
- At 16 and 32 px it's sharp, not blurred.
