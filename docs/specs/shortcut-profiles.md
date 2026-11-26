# Spec draft: shortcut and mouse profiles

**Status:** Draft for the owner's discussion · 21 November 2026 (features stream, #489).
**Covers:** the owner's request of 21 November 2026: a Settings page for shortcut profiles, with
templates following Rhino, SketchUp, Archicad, Revit, Vectorworks, AutoCAD, Blender, Fusion 360 and
SolidWorks (the owner's list of 26 November 2026),
covering keyboard shortcuts and how the mouse works (selecting, navigating).

## 0. In short

- A **profile** is a full set of keyboard shortcuts **and** mouse behaviour: how to orbit, pan and
  zoom, how window selection works, which keys add, remove or pick parts of objects, how to copy
  while dragging.
- Curva ships **templates** for each program listed, plus **Curva** (its own default, which is
  Rhino's). The user picks one in Settings › Shortcuts, then changes any key; their changes are saved
  as their own profile, which they can export and import as a file.
- Every command keeps its typed name in every profile; only keys and mouse change.
- Rhino's must be exact first (#485 and #486 are part of it), since Rhino is the standard.

## 1. What a profile holds

| Part | Examples |
|---|---|
| Navigation | Orbit, pan, zoom: which mouse button and keys; zoom to the cursor (#469); zoom extents |
| Selection | Click, window and crossing (by direction or by key), add, remove, toggle, pick a part of an object (edge, face, point), select all, deselect |
| Dragging | Move by dragging, copy while dragging, constrain to an axis |
| Keys for commands | Single letters (SketchUp's L for Line), modified keys (Ctrl+C), and Rhino's command aliases |
| Snaps and input | Toggle ortho, snaps, temporary snap override, typing a distance during a drag |

## 2. The programs' standard behaviour (to be measured in each program before building)

This is the research summary; each template is checked against the program's own default shortcut
list before it ships, as Rhino commands are measured in Rhino 8.

| | Orbit | Pan | Zoom | Window selection | Add / remove | Pick a part | Copy while dragging |
|---|---|---|---|---|---|---|---|
| **Rhino** | Right drag (perspective) | Shift + right drag; right drag in parallel views | Wheel, Ctrl + right drag | Left to right inside, right to left crossing | Shift adds, Ctrl removes | **Ctrl+Shift** click or window | **Alt** while dragging or with the gumball |
| **SketchUp** | Middle drag (Orbit, O) | Shift + middle drag (Pan, H) | Wheel (Zoom, Z) | Left to right inside, right to left crossing | Ctrl adds, Shift toggles, Ctrl+Shift removes | Double-click enters a group or component | **Ctrl** with Move (M) |
| **Revit** | Shift + middle drag | Middle drag | Wheel | Left to right inside, right to left crossing | Ctrl adds, Shift removes | Tab cycles through what is under the cursor | **Ctrl** while dragging |
| **Archicad** | Shift + middle drag (in 3D) | Middle drag | Wheel | Marquee or Shift + click | Shift toggles | Arrow tool on a node; Tab cycles | **Ctrl** (Windows) / **Alt** (macOS) during Drag |
| **Vectorworks** | Middle drag with the Flyover tool, or Ctrl+Shift with the wheel | Middle drag | Wheel | Left drag a box (Alt for crossing, a preference) | Shift toggles | Double-click enters a group; Reshape tool for points | **Ctrl** (Windows) / **Alt** (macOS) while dragging |
| **AutoCAD** | Shift + middle drag | Middle drag | Wheel | Left to right window, right to left crossing | Click adds, Shift removes | Ctrl + click picks a face, edge or sub-object of a solid | Ctrl while dragging with grips |
| **Fusion 360** | Shift + middle drag | Middle drag | Wheel | Left to right inside, right to left crossing | Ctrl or Shift adds, Ctrl removes | Faces, edges and bodies picked directly; selection filters | **Ctrl** with Move (Create Copy) |
| **SolidWorks** | Middle drag | Ctrl + middle drag | Wheel, Shift + middle drag | Left to right box, right to left crossing | Ctrl toggles | Faces, edges and vertices picked directly; selection filter (F5) | **Ctrl** while dragging |
| **Blender** | Middle drag | Shift + middle drag | Wheel, Ctrl + middle drag | B box select, C circle; left drag box in 2.8+ | Shift toggles, Ctrl removes | 1, 2, 3 for vertex, edge, face in Edit mode (Tab) | **Shift+D** duplicates and then moves |

Also worth including: **Fusion 360** (engineers and furniture makers), **3ds Max** (visualisation),
**BricsCAD** (same as AutoCAD; could share AutoCAD's template), **Allplan** and **ArchiCAD for
macOS** differences (one template with platform keys handled for every program, as Ctrl and Cmd
swap on macOS).

## 3. The Settings page

- **Settings › Shortcuts:** a profile picker at the top (Curva, Rhino, SketchUp, Revit, Archicad,
  Vectorworks, AutoCAD, Blender, Fusion 360, SolidWorks, then the user's own), with **Duplicate**, **Rename**,
  **Export**, **Import** and **Reset to template**.
- Below it, three tabs: **Mouse** (navigation, selection, dragging, drawn as a picture of a mouse
  with labels), **Keys** (searchable list of every command with its key, click to change, conflicts
  shown), **Selection rules** (window direction, add and remove keys, part picking).
- First run asks once: "Which program do you know best?" and picks that profile (it can be changed
  later). The welcome screen can show the five keys that matter most in the chosen profile.

## 4. Steps

1. Rhino's behaviour exact in the Curva profile: Ctrl+Shift part picking by click and window (#485),
   Alt-drag copies (#486), zoom to the cursor (#469).
2. Profiles in the settings file: keys and mouse rules read from a profile instead of being fixed in
   code; Curva's profile is today's behaviour.
3. Settings › Shortcuts page: picker, Keys tab, Mouse tab, export and import.
4. Templates, one pull request each, each checked against the program's own published default list:
   SketchUp, Revit, Archicad, Vectorworks, AutoCAD, Blender, Fusion 360, SolidWorks.
5. First-run question.

## 5. Decisions for the owner

1. **Mouse included in profiles** (orbit, pan, selection rules), not only keys. *Recommended*:
   yes; muscle memory is mostly in the mouse.
2. **Which extra programs:** Fusion 360 and 3ds Max are worth a template; BricsCAD shares AutoCAD's.
   *Recommended*: add Fusion 360; 3ds Max later if asked.
3. **First-run question** ("Which program do you know best?"). *Recommended*: yes.
4. **Where a template can't match** (e.g. Blender's modes, Revit's Tab cycling): the closest Curva
   behaviour, listed in the template's notes. *Recommended*.

### Owner's answers (26 November 2026)

1. **Profiles hold keys and mouse behaviour.**
2. **Templates to ship now:** Rhino (the Curva default), SketchUp, Archicad, Revit, Vectorworks,
   AutoCAD, Blender, Fusion 360 and SolidWorks, merging any that are essentially the same. Cinema 4D
   and 3ds Max are dropped.
   - No two of the nine share their keys: each program has its own shortcut list, so each keeps its
     own template.
   - Several share their **mouse** rules (orbit Shift + middle drag, pan middle drag, zoom wheel:
     Revit, AutoCAD and Fusion 360 by default). So the mouse part is stored as a named **mouse
     preset** that templates refer to, and a shared rule is written once. Fusion 360 also offers
     SolidWorks-style navigation, which reuses SolidWorks' preset.
   - BricsCAD, if ever wanted, reuses AutoCAD's template whole.
3. **First-run question:** yes, as recommended.
4. **Where a template can't match:** the closest Curva behaviour, listed in the template's notes,
   as recommended.
