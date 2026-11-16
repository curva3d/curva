# 0031: IFC compliant from the start, without IFC as the editing format

**Status:** Direction agreed by the owner in chat · 16 November 2026. The library is chosen at the
start of M4, after a head-to-head test. Updates decision 0008 (IFC library).

**What was decided.**
- **Curva keeps its own model for editing** (architecture §6, §8), as Archicad and Revit do. IFC is
  how Curva exchanges buildings, not how it holds them while you edit. Bonsai (Blender), whose model
  *is* IFC, was considered: it works, but IFC is verbose and not made for live editing, and Curva's
  recipes, NURBS history, views and styles don't fit it naturally.
- **Every architectural object knows its IFC identity from the moment it is created:** its IFC class
  (IfcWall, IfcSlab, IfcDoor…), its type, its place in the spatial structure (site, building,
  storey), and its standard property sets and quantities. Nothing is mapped "later".
- **That vocabulary comes from a library**, generated from the official IFC 4.3 schema (class
  names, property sets, allowed values), never typed by hand.
- **The library reads and writes the files.** Curva's writer hands it clean geometry: walls as
  extrusions, curved shapes as exact surfaces from solidcore. Meshes never stand in for Curva's own
  geometry.
- **Checked from the first wall onwards**, in CI: every test building exported and validated against
  IDS rules, checked against the buildingSMART sample files, and round-tripped (export, re-import,
  compare).
- **Curva's own work** is how objects *behave* while you edit them: joins, openings following their
  wall, layers, recipes. No IFC library provides that.
- **First target:** the Reference View (the coordination exchange consultants use). Others later.

**Importing IFC.** Reading an IFC file never turns it into meshes: a library's meshes are only for
showing a model. To bring elements in as editable Curva objects, Curva reads the IFC entities
themselves and maps them: an extruded wall becomes a Curva wall with its profile and height, exact
curved geometry (IfcAdvancedBrep) becomes solidcore geometry, and geometry already stored as faces
stays a mesh.

**The candidates, to test head to head at the start of M4** (read the buildingSMART samples, write
a test building, run IDS checks):

| Library | Strengths | Licence | Notes |
|---|---|---|---|
| **OpenBIM.rs** (`openbim-ifc`, `openbim-ifcx`) | The most complete in Rust: entity graph, STEP and ifcXML, schema tables, validation, authoring, geometry; lossless round trip; IFC5 crate | AGPL-3.0 or commercial (decision 0008; the crate page no longer states it, so re-check) | Rejected in 0008 when Curva was MIT/Apache. Now that the application is GPL-3.0 (decision 0025), GPLv3 allows combining with AGPLv3 code. The AGPL's network clause would matter only for a future online Curva service. A legal choice for the owner, to confirm properly |
| **IFClite** (`ifc-lite-*`, LTplus) | Reads IFC2x3, IFC4/4.3 and IFC5; exact boolean kernel checked against IfcOpenShell (99.9%+); writes edits back; IDS checks; bSDD lookup | MPL-2.0: needs the owner's approval (not on the permissive list) | Output is meshes, for display only; geometry editing and creating files are mostly in its JavaScript packages; pins a nightly Rust toolchain |
| **Curva's own, on a STEP base** | Fully permissive and under Curva's control; can grow from the STEP reader and writer Curva already reuses | MIT / Apache | Most work. The Rust STEP toolkits (ruststep, step-p21) call themselves experimental |

Likely outcome as things stand: OpenBIM.rs for reading and writing if its licence is acceptable,
with IFClite as a cross-check; otherwise IFClite for reading and checking, and Curva's own writer.

**Also noted.** `rvt-rs` (Apache-2.0) reads Revit files without Revit and writes IFC4; a possible
way to open consultants' Revit files later. `ifc_rs` (MIT, stalled since December 2024) and
`bimifc-parser` (MPL, a parser only) were looked at and set aside. IfcOpenShell, web-ifc and xBIM
are not Rust.

**Revisit at the start of M4**, with the test results.
