# Feedback on 0.4.2 · 14 November 2026

**Status:** Draft for the owner's approval.

The owner's points after 0.4.2, numbered, with what was decided and the spec for each. The owner's
answers are in decision 0021. Everything here ships in 0.4.3 with Milestone 3.

| # | The owner's point | Where it is answered |
|---|---|---|
| 1 | "the import and export commands are missing a native os folder window to actually pick the file, write the name etc." | §1 |
| 2 | "the icon in linux doesn't show up in the dock" | §2 |
| 3 | "allow curva files to mm, m, feet and inches" | §3 |
| 4 | Analyse what is built: lightweight, efficient, organised in modules, no tech debt | `docs/reviews/code-health-2026-11-14.md`; fixes in Milestone 3's clean-up |
| 5 | For Milestone 3, "always check if someone else hasn't built the function before building a custom solution, especially for import export" | "Prior work checked" in each Milestone 3 spec; DWG added (decision 0021) |
| 6 | Top bar at half its height: Curva at the left edge, File and Settings after it, the specialization dial in the centre, the file name over its path on the right | §4 |
| 7 | "align the mcp panel to the ui panel above it in the default layout" | §5 |
| 8 | Layers panel: a way to size each column (first written as rows, then corrected to columns) | §6 |
| 9 | "Commands ui panel should be called Operations"; its Operations tab becomes Modify | §7 |

## 1. Native file dialogs

Open, Import, Save (for an untitled model), SaveAs, Export and ExportSelected show the system's own
file dialog: Explorer's on Windows, Finder's on macOS, the desktop's through the XDG portal on Linux.

- **Open and Import:** the open dialog, filtered to the formats Curva reads ("All supported" first,
  then each format), starting in the folder last used.
- **Save, SaveAs, Export, ExportSelected:** the save dialog, with a name field filled with the model's
  name, a type list of the formats Curva writes, and the system's own overwrite question.
- **The program keeps drawing** while the dialog is open (it runs off the frame loop), and Escape or
  Cancel ends the command with nothing changed.
- **Typed paths and scripts:** as in Rhino, the command with a dash in front (`-Open`, `-Import`,
  `-SaveAs`, `-Export`, `-ExportSelected`) asks for the path on the command line with no dialog;
  MCP agents and `--run` scripts always use these. A path typed straight after `Open` at the command
  line still works.
- **Prior work checked:** `rfd` 0.18 (MIT, updated 7 November 2026, about 19.5 million recent
  downloads) is the standard Rust crate for native dialogs on all three systems, using the XDG portal
  on Linux through `ashpd` (MIT). Used as it is; no `unsafe` in Curva's own code.

## 2. The icon on Linux

On Linux the dock and task switcher match a window to an installed `.desktop` file by its app id;
Wayland ignores the icon a program sets on its window, and an AppImage does not install its
`.desktop` file. So today the dock shows a generic icon.

- The window gets the app id `curva` (`ViewportBuilder::with_app_id`), and `curva.desktop` gets
  `StartupWMClass=curva`, so the two match on Wayland and X11.
- The AppImage carries PNG icons at 16 to 512 pixels in `usr/share/icons/hicolor` plus `.DirIcon`,
  as well as the SVG, since some docks don't read SVG.
- **Adding Curva to the applications menu:** the first time Curva runs from an AppImage, the welcome
  screen offers "Add Curva to your applications", which writes the `.desktop` file (pointing at the
  AppImage) and the icons into `~/.local/share`. A setting removes them again. This is what
  AppImageLauncher and Gear Lever do; Curva does it itself so nothing else is needed.
- **Prior work checked:** the AppImage desktop-integration convention (`.desktop` + hicolor icons
  in `~/.local/share`), as AppImageLauncher, Gear Lever and appimaged do it. No library needed.
- **Check:** CI checks the AppImage holds the icons and the matching `StartupWMClass`; the dock itself
  needs a hand check on a Linux desktop (GNOME and KDE), which the Windows machine can't do, so the
  owner is asked to try it before release.

## 3. Units: millimetres, metres, feet and inches

A model's unit can be millimetres, metres, feet or inches (`Unit` gains `Feet` and `Inches`), chosen
for a new model and changed in Settings, where Curva already asks whether to scale the model or keep
its numbers, as Rhino does.

- **How lengths show** (decision 0021, point 3): in feet, architectural feet-and-inches
  (`5'-3 1/2"`); in inches, inches and fractions (`63 1/2"`). The fraction's precision is a setting
  (1/2 to 1/64), and a setting switches either unit to decimals (`5.29'`, `63.5"`).
- **Typing** accepts every form in any unit: `5'3"`, `5'-3 1/2"`, `5ft`, `63in`, `63.5`, `1.6m`,
  `20mm`, `3/8"`. A typed unit is converted to the model's.
- **Defaults that follow the unit,** as in Rhino's templates: absolute tolerance (0.01 mm, 0.001 m,
  0.001 in, 0.0001 ft), grid spacing and the ISO or ANSI drafting set (decision 0020, point 4).
- **Files:** `.curva` files store the unit; `.3dm` files map to Rhino's units both ways (Rhino's
  feet and inches already exist in the format).

## 4. The top bar at half height

The top bar becomes one row, 36 points tall (half of today's 72):

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ ◆ Curva   File   Settings          ‹  Freeform  ›           Tower study     │
│                                                       V:\projects\tower.curva│
└─────────────────────────────────────────────────────────────────────────────┘
```

- **Left:** Curva's mark and name at the left edge, then File and Settings.
- **Centre:** the specialization dial, centred on the window.
- **Right:** the file name on top, its folder path below in smaller, quieter text, both right-aligned.
  A long path is shortened in the middle (`V:\projects\…\tower.curva`); hovering shows it in full.
  An untitled model shows "Untitled" and no path.
- Panels below move up to use the space; focus mode slides the bar away as it does now.

## 5. The agent (MCP) panel lines up with the panel above it

In the standard layout, the agent panel in the bottom-right corner takes the width of the Layers and
Properties column above it (260 points instead of 220), so their left and right edges line up, with
the usual gap between them. Layouts people have saved keep their own arrangement; Reset layout
gives the new one.

## 6. Layers panel columns

The Layers panel becomes a table with a header row: **Name, On, Lock, Colour, Linetype, Print
width**.

- **Sizing columns:** drag the divider between two headers to make the column on its left wider or
  narrower, column by column; the pointer shows a resize cursor over a divider. Double-clicking a
  divider fits that column to its widest content. Each column has a minimum width so nothing
  disappears.
- Widths are kept with the panel layout (`layout.json`), so they survive restarts, and Reset layout
  puts them back.
- The Name column takes any space left over when the panel is widened.

## 7. "Operations" for the commands panel; "Modify" for its tab

- The panel of command buttons, today titled **Commands**, is titled **Operations**.
- Its tab today called Operations (booleans, Trim, Split, Join, Move, Copy, Rotate, Scale, Mirror,
  arrays, deformations, Group, Block…) is renamed **Modify** (decision 0021, point 2).
- Only the names in `locales/en.txt` and the matching keys change; saved layouts keep working.
