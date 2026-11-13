# Spec: the rest of phases F2 and F3

**Status:** Draft, for the owner's approval · 13 November 2026.
**Covers:** `milestone-2-in-0.4.2.md`, points 6 to 9.

All with Rhino's steps and options, on the command framework, with tests, icons, reference rows and
places in the Commands panel.

## 1. Organising (F3)

| Command | What it does |
|---|---|
| Count | Counts the objects selected, by kind |
| ChangeLayer | Moves objects to a layer chosen from a list |
| MatchProperties | Copies layer, colour, print width, linetype and name from one object to others |
| Linetype | Objects and layers get a linetype (Continuous, Dashed, Dotted, DashDot, Hidden, Center, Border), drawn in every view; set in Layers and Properties like print width |
| PrintWidth | Sets objects' print width from the command line (what Properties sets) |
| SetUserText, GetUserText | Key–value text on objects, kept in files and `.3dm` |
| Purge | Removes unused layers, block definitions and linetypes |
| Audit | Checks every object and reports invalid geometry |
| LockSwap, UnlockSelected | Rhino's other lock commands |
| BringToFront, SendToBack, BringForward, SendBackward | Draw order of curves where they overlap |

## 2. Surfaces and solids (F2)

| Command | What it does |
|---|---|
| DupFaceBorder | Copies the outline of picked faces as curves |
| PushPull | Pulls a face of a solid in or out by a distance (ExtrudeFace or MoveFace, as Serpentine3D does) |
| TextObject | Text as curves, surfaces or solids in a font from the system, read with a pure-Rust font reader |

## 3. Analysis

CurvatureGraph and Zebra are in `engine-gaps.md`. **DraftAnalysis** colours faces by their angle to a
direction, with the angles chosen as Rhino does.

## 4. Onto the command framework

Trim, Lock, Unlock, Group, Ungroup, Plan and SelColor are rewritten as recipes, so they get previews,
clickable options and History like every other command; their notes in `rhino-parity.md` go.
