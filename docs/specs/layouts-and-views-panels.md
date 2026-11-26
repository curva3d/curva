# Plan: the Layouts panel and the Views panel

**Status:** Draft for the owner's discussion · 21 November 2026 (features stream, #490).
**Covers:** the owner's request of 21 November 2026: "a layout panel just like in Rhino and a views
panel just like in Archicad or Revit… mind you the views or details will also be the live drawings."
**Builds on:** `layouts.md` (approved), `views-and-styles.md` §1 (approved: views are saved items
listed in a Views panel), `live-plans.md` (live plans and sections).

## 0. In short

- **Views panel:** one list of every view in the model: 3D cameras, plans, sections, elevations.
  Create, duplicate, rename, delete, lock, organise in folders. A view is a **live drawing**: it keeps
  its camera or cut, its style, its own layers and its scale, and anything showing it updates with
  the model.
- **Layouts panel:** every layout (sheet), as Rhino 8's panel, with Archicad's folders (subsets) and
  Revit's sheet numbers. Create, duplicate, copy, delete, reorder, print.
- **Details show views.** A detail on a layout no longer keeps a camera of its own: it shows a view
  from the Views panel. Drag a view from the Views panel onto a layout to place it. Change the view and
  every detail and viewport showing it changes. This is how Archicad and Revit work, and it is what
  makes "the views are the live drawings" true.

## 1. How the others do it

| | Archicad | Revit | Rhino 8 | Curva |
|---|---|---|---|---|
| Where the model's viewpoints are | **Project Map**: stories, sections, elevations, details, 3D, as a tree | **Project Browser**, Views: grouped by kind (floor plans, ceiling plans, sections, 3D) | Viewports and Named Views | Views panel, Viewpoints tab (§2.2) |
| Saved views with their settings | **View Map**: each view is a viewpoint plus layer combination, scale, pen set, model view options, in folders | Each view *is* saved, with visibility, scale, detail level, view template | Named Views (camera only) | Views panel, Views tab: camera or cut **plus** style, layers, scale |
| Copying a view | Save the current view again with other settings | **Duplicate** (model only), **Duplicate with detailing** (with its annotation), **Duplicate as dependent** (a crop of the same view that follows it) | Copy a Named View | Duplicate, Duplicate with annotation; Dependent later (§5) |
| Sheets | **Layout Book**: layouts in subsets (folders), each on a **master layout** (title block) | **Sheets** in the browser, numbered and named; a view goes on one sheet only | **Layouts panel**: list with New, Copy, Delete, Print, and drag to reorder | Layouts panel: Rhino's list, Archicad's folders, Revit's numbers |
| Putting a view on a sheet | Drag from the View Map to a layout (a **drawing**, which stays live) | Drag from the browser onto a sheet (a **viewport**) | Draw a Detail and set its camera | Drag from the Views panel (a detail that shows the view) |
| Printing many | **Publisher Sets**: saved lists of views and layouts with their output | Print, choose sheets, save the selection as a set | Print dialog with layouts ticked; Print from the Layouts panel | Print sets (see `print-panel.md`) |

What we take: Archicad's split between **viewpoints** (where you look from) and **views** (a viewpoint
with its settings); Revit's one-browser feel, its three Duplicates and its rule that a view sits on
one sheet only (it keeps sheet references unambiguous); Rhino's simple Layouts panel buttons and
Print button.

## 2. The Views panel

### 2.1 What a view is

As approved in `views-and-styles.md` §1.1: a name, a kind (Perspective, Parallel, Plan, Section,
Elevation), the camera or cut, the style, its own layers on and off, its sun, locked or not. Added
here: **scale** (for 2D views: 1:50 and so on; the scale details use by default) and a **folder**.

### 2.2 The panel

Two tabs, as Archicad's Project Map and View Map, kept small:

- **Views** (the main tab): every saved view, in folders. Default folders by kind: **3D**,
  **Plans**, **Sections**, **Elevations**, **Details**. The user can make their own (e.g.
  "Submission", "Client"). A view placed on a layout shows that layout's number beside it; one not
  yet placed shows a hollow dot, so you can see what is still missing from the set.
- **Viewpoints:** the model's cuts and cameras by themselves (each plan cut, each section line, each
  elevation), so you can see every section in the model and make views from them. Several views can
  share one viewpoint: the ground floor plan for submission and for the client share the same cut.

Rows: a small picture, the name (renamed in place by a slow double-click, as files), the scale, a
lock. Click shows the view in the active viewport; double-click opens it full size.

Buttons and right-click menu:

| Action | What it does |
|---|---|
| New view | From the active viewport as it is now; or New plan, New section, New elevation (each asks for the cut, as `live-plans.md` says) |
| Duplicate | A new view with the same camera or cut, style, layers and scale; independent from then on |
| Duplicate with annotation | Also copies the dimensions, texts and tags drawn in that view |
| Rename, Delete | Delete asks only when the view is placed on a layout, and says which |
| Lock | A locked view cannot be orbited, panned or zoomed by accident |
| Place on layout | Puts it on the open layout at its scale; or drag the row onto a layout |
| Update | For a plan or section waiting for an update (`live-plans.md` §3) |
| Folder | New folder, move to folder, drag between folders |

### 2.3 Live drawings

A plan, section or elevation view *is* the live drawing (`live-plans.md`): opening it shows the
drawing, and placing it on a layout places the same live drawing. One view, one drawing: wherever it
shows, it is the same and changes together.

## 3. The Layouts panel

Today's panel (`layouts.md` §1) already lists, renames, reorders, opens, copies and deletes
layouts. It gains:

- **Number and name** per layout, as Revit's sheets (A-101 Ground floor plan). Fields on the title
  block read them, so the title block and the sheet index stay right.
- **Folders (subsets)**, as Archicad's Layout Book: e.g. "Licensing", "Construction", "Client". A new
  layout goes in the selected folder. Numbering can follow the folder (A-1xx, A-2xx).
- **New layout** with a chosen **title block template** (Archicad's master layout): paper size,
  orientation, title block, margins in one choice. `New from template…` lists the model's own and
  Curva's standard ones.
- **Duplicate** (the layout with its title block and its annotation, but empty details, since a
  view may sit on one layout only), **Duplicate with views** (also new copies of its views, so the
  copy can be changed on its own). **Copy** to another model later, with Paste.
- **Delete** asks once, and says which views it held (the views stay in the Views panel).
- **Print** from the panel: the selected layouts go to the print panel, as in Rhino.
- **Thumbnails** as today, drawn on another thread and kept until the layout changes, so the panel
  never slows the viewport (the lesson from #474).

## 4. Details show views

- A detail holds **which view it shows**, plus its frame on the paper. Scale, style and layers come
  from the view; a detail may override the scale only (as Archicad lets a drawing's scale differ).
- **Placing:** drag a view onto a layout, or run Detail and pick a view at the prompt (the default,
  "New view from these settings", keeps Rhino's way: draw the frame, then set it up, and a view is
  made for it behind the scenes and listed in the Views panel).
- **Entering a detail** (double-click) and orbiting or panning it changes its view, as today, unless
  the view or the detail is locked. Every other place showing that view follows.
- **One view, one layout**, as Revit: placing a view already placed elsewhere offers Duplicate
  instead. So a sheet's drawing references (Section 3 on A-201) always point at one place.
- **Files from Rhino:** each detail of a `.3dm` layout becomes a view (in a folder named after its
  layout) and a detail showing it, keeping its camera, projection, scale, display and lock (#478).
  Named Views become views in a "Named views" folder.
- **Older Curva files:** each detail's own camera becomes a view in the same way when the file
  opens; nothing is lost.

## 5. Steps, each a small pull request

1. Views as saved items in the document (`SavedView` grows into the view of §2.1, with old files
   read as today) and the Views panel listing, showing, renaming, deleting and duplicating them.
2. Details show views: the document change, the old-file reading, dragging a view onto a layout,
   and `.3dm` details and named views brought in as views (#478 rides on this step).
3. Layouts panel: numbers, folders, Duplicate and Duplicate with views, Print button.
4. Title block templates for New layout.
5. Folders and the Viewpoints tab in the Views panel.
6. Later, with the architecture specialisation: dependent views (one large plan split over several
   sheets) and drawing references (section marks that know their sheet).

## 6. Decisions for the owner

1. **Details show views (§4)**, the Archicad and Revit way, instead of each detail keeping its own
   camera as Rhino does. *Recommended*: it is what makes views the live drawings.
2. **One view on one layout only**, as Revit. *Recommended*; the alternative (Archicad lets a view sit
   on several layouts) makes section and detail references ambiguous.
3. **Two tabs in the Views panel** (Views and Viewpoints) or **one list** with the viewpoints as
   folders. *Recommended*: two tabs, Views first; most work happens in Views.
4. **Panel placement:** Views and Layouts as two panels side by side in the right-hand dock, or one
   panel with two tabs. *Recommended*: two panels, as each is used on its own a lot.

### Owner's answers (26 November 2026)

1. **Details show views:** yes, as recommended.
2. **A view can sit on several layouts**, as in Archicad. To keep references clear, each section,
   elevation or detail marker points to **one chosen placement** of its view: the first placement
   by default, changeable in the marker's properties. The Views panel shows every layout a view is
   on.
3. **Cameras are saved positions**, as Rhino's named views, not placed objects. The Views panel is
   **one tree grouped by the user's own folders** (as Archicad's View Map), named for what is
   delivered ("Planning set 1:100", "Working 1:50"), not by storey. It replaces the separate
   Viewpoints tab:
   - **Clone folders:** one set-up view (layers, scale, style) and a choice of levels; the folder
     makes one view per level and adds or removes them as levels change. A 60-storey building
     then needs one set-up view per set, not 60.
   - **Group by:** folders (the default), level, or kind, switching how the same views are shown.
   - **Search** at the top, narrowing to matching views and opening their folders.
   - The **Layouts panel** works the same way: the user's folders (drawing sets), sheet numbers and
     search. A clone folder offers **Place each on its own layout**, one sheet per level from a
     chosen title block, kept in step as levels are added (confirmed by the owner).
4. **Two panels**, as recommended. Besides the normal way (a detail on the layout, then choose its
   view), **drag a view** from the Views panel onto an open layout, or onto a layout's row in the
   Layouts panel, to place it; dropped on a row, it lands in the layout's first free space at the
   view's scale.

## 7. Sources

- [Archicad: Navigator basics](https://community.graphisoft.com/t5/Getting-started/Archicad-Basic-Concepts-The-Navigator/ta-p/303987),
  [Archicad 28 Navigator](https://help.graphisoft.com/AC/28/INT/_AC28_Help/030_Interaction/030_Interaction-3.htm),
  [The Layout Book](https://helpcenter.graphisoft.com/?p=55439)
- [Revit: Project Browser](https://help.autodesk.com/cloudhelp/2025/ENU/Revit-GetStarted/files/GUID-C8D3E5A6-02A5-43A9-AFFC-D49DD27398B1.htm),
  [Revit: dependent views](https://help.autodesk.com/cloudhelp/2023/ENU/Revit-DocumentPresent/files/GUID-8D44336D-00A3-4218-8442-5ED235D8D326.htm)
- [Rhino 8: Layouts](https://docs.mcneel.com/rhino/8/help/en-us/commands/layouts.htm),
  [Rhino layout management](https://www.rhino3d.com/features/drafting/layout-management/)
