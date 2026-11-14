# Curva — Architecture (draft v0.10)

*8 November 2026, updated 11 November 2026 · starting point for the Curva project (working name; previously "Dex")*

## 1. Vision

Curva is an open-source, **design-oriented** 3D suite for architects and designers. It is not an engineering tool: no SolidWorks clone, no mechanical design.

It combines Rhino's freedom with the "memory" of Revit and Archicad:

- **Objects that remember how they were made:** parameters, relationships, constraints and live booleans, all native
- **A Rhino-style 3D model space** with layouts
- **Revit / Archicad-style 2D views** (plans, sections, elevations) and 3D views, generated live from the model and placed on sheets
- **Specializations:** one program whose interface adapts to the kind of work, starting with **Freeform**

Curva reuses open-source components wherever possible instead of reinventing them.

## 2. Principles

1. **Pure Rust.** Everything Curva builds or links is Rust: the core, kernel, interface, viewport, BIM model, file formats and scripting. **The one exception is the Blender / Cycles rendering bridge**, which may use Blender's own C++ and Python.
2. **Freeform is the base.** The Freeform specialization (Rhino-level modelling) is built first, and every other specialization builds on it.
3. **Design-oriented parametrics.** Every object can remember its recipe and its relationships, and can be "baked" into plain geometry for free sculpting.
4. **One program, many specializations.** Everything shared is shared. A specialization adds tools and layout, never its own object model.
5. **Fast and multicore by design.** Independent work runs on all cores. The interface never waits for geometry.
6. **The kernel sits behind our own interface,** so Rust kernels can be swapped or combined as they mature.
7. **Everything is a command, and MCP is native.** One command layer drives the interface, scripting, the command line and AI agents. The MCP server is built in from Milestone 1, not added later (section 5c).
8. **Objects know how to draw themselves.** A smart object has several representations: a 3D model at different levels of detail, a 2D plan symbol, and section and elevation graphics. Each view picks the right one (section 5b).
9. **Open formats first:** IFC, STEP, glTF, USD, 3DM.
10. **Built by AI agents, verified by machines.** Agents write the code. Automated tests, not people, decide whether it works (section 15).
11. **Runs on Windows, macOS and Linux** from day one, all three tested on every change (section 16).
12. **Readable over clever.** Code a person can open, follow and maintain. No black-box logic (section 17).
13. **Clean geometry.** Use exact NURBS where it matters, and build simple elements directly rather than through chains of booleans.

## 3. Specializations

### The rule: shared vs unique

| Shared by every specialization (the core) | Unique to each specialization |
|---|---|
| One document and object model | Toolsets |
| One command system and command line, with the same shortcuts everywhere | Panel layout, ribbon or toolbar style |
| Selection, object snaps, navigation, gumball | Templates and default settings |
| Properties panel, layers, materials | Specialized object types (walls, plots, …) built on core objects |
| Parametric graph, history, live booleans | |
| Views, layouts and sheets | |
| Visual scripting (later) | |

Objects from every specialization live in the same file and react to each other. A freeform canopy can cut an Archicad-style wall. Switching specialization changes the room you're working in, not the building.

This avoids FreeCAD's problem, where workbenches feel like separate programs with their own objects and conventions.

### The four specializations

| # | Specialization | Inspired by | Content |
|---|---|---|---|
| 1 | **Freeform** (the base) | Rhino | Curves, surfaces, solids, transforms, analysis, layouts. Mesh and SubD objects come at a later stage (section 10d). Interface follows Rhino's overall layout and command line |
| 2 | **Architecture** | Archicad, Vectorworks | Levels, walls, slabs, doors, windows, roofs, stairs, types; plans, sections, elevations; IFC. Interface closer to Archicad / Vectorworks |
| 3 | **Urban** | Grasshopper-style massing tools | Rule-based generation of urban settings: plots, setbacks, heights, floor-area ratios; massing exercises; city data import (OpenStreetMap, terrain, GIS); area and density tables |
| 4 | **Analysis** (much later) | — | Sun and shadow first, then wind, views and others |

## 4. Design-oriented parametric model

- **Not a linear feature tree** (SolidWorks), which is rigid and engineering-minded.
- **Objects with editable recipes, plus relationships between objects.** A loft remembers its curves, a wall knows its level, a window knows its wall. Edit an input and everything downstream updates. In Freeform this is Rhino's "record history", made native and robust.
- **A dependency graph, not a single sequence.** This is what makes recomputing on many cores possible.
- **Live booleans as real objects.** "This void cuts that mass" stays editable forever. Operands can be shown, hidden, moved or swapped.
- **Constraints where designers want them:** constraint sketches, and relationships such as "attached to", "aligned with", "offset from".
- **The bake escape hatch.** Any object can be turned into plain geometry for free editing (control points, sculpting), the way Rhino users expect. Two steps, like the Explode designed for Serpentine: first a plain solid, then faces.

## 5. Model space, views, layouts and sheets

All of this is shared core, used by every specialization.

- **One 3D model space,** as in Rhino.
- **Views are live windows into the model,** not copies:
  - **3D views:** a saved camera plus display settings
  - **2D views** (plans, sections, elevations): a cut plane and view range plus graphic settings, generated live from the model. The user draws 2D annotation on top
- **Layouts and sheets:** views placed at a scale, with title blocks, dimensions, text and hatches. Rhino-style layouts with detail views and Archicad-style sheets use the same mechanism.
- **The hard part is 2D view generation:** fast, correct plans and sections with cut fills (poché), hidden lines, and line weights by cut or projection. This deserves its own milestone. Freeform gets Rhino-style layouts and Make2D first; full live plan and section views come with Architecture.
- **Views ask objects for their representation** (next section) instead of always cutting the 3D geometry. A door draws its plan symbol in a plan, and its true 3D model in an elevation.

## 5b. Smart objects and representations

A smart object (a door, window, tree, person, furniture piece, wall style) carries **several representations of itself, all driven by the same parameters**. Change a door's width and its 3D model, plan symbol and elevation all update together.

| Representation | Used in | Examples |
|---|---|---|
| **3D, low detail** | Working viewport, large models | A tree as a light low-poly volume, a person as a simple figure |
| **3D, high detail** | Close-up views and rendering (Blender / Cycles) | A full tree model, a detailed door with handle and frame |
| **Plan symbol (2D)** | Plan views | Door leaf and swing arc; a tree as a graphic canopy symbol |
| **Section / elevation graphics (2D or 3D)** | Sections, elevations | A door drawn from its true 3D model in elevation; a tree as a drawn silhouette in section |
| **Scale variants** | Drawings at different scales | A simpler door symbol at 1:200 than at 1:50 |

**How a view chooses:** by view type (3D / plan / section / elevation), drawing scale, and a detail level the user sets per view (coarse / medium / fine). If an object has no 2D representation for a view, the view falls back to cutting or projecting its 3D model.

**Why it matters:**

- **Drawings look like architects' drawings,** not raw cuts through 3D geometry. Archicad (GDL objects), Revit (families with symbolic lines and detail levels) and Vectorworks (hybrid 2D/3D symbols) all do this. Curva makes it native and shared by every specialization.
- **Performance:** the viewport draws light representations. Heavy ones are only loaded for close-ups and rendering. Combined with instancing, a site with 2,000 trees stays fast.

**How they're built:**

- **Definition and placements:** a smart object definition (parameters plus representations) is stored once. Each placed object refers to it with its own parameter values. This is the same instancing used for blocks and types.
- **Representations can be:**
  - geometry generated from parameters (a parametric door)
  - fixed geometry (an imported tree model)
  - 2D drawings (a hand-drawn plan symbol)
  - a mix of the above
- **Logic in a sandbox:** parametric logic is written in Rhai or as a WebAssembly module, so downloaded objects can't harm the user's computer.
- **An open format:** a smart object can be saved as its own file (`.curvaobj`). The format is documented, so anyone can make and share objects.

## 5c. Native MCP

- **Built in from Milestone 1.** The MCP server is part of Curva, not a plug-in, and is switched on from the app's settings.
- **Agents can read:** the scene, objects, parameters, layers, views and sheets. They can also render any view to an image, so they can check their own work visually, the way Serpentine's assistant does.
- **Agents can act:** every command is available, with the same prompts and options as the command line.
- **Safe by design:**
  - each agent action is one undo step
  - destructive actions can require the user's approval
  - the user sees what the agent is doing as it happens
- **A built-in assistant panel** connects to Claude or another model through the same MCP layer.
- **Later:** Curva as an MCP client too, so it can pull data from other tools (city data, product catalogues, project management).

## 6. Stack

| Layer | Choice | Licence | Notes |
|---|---|---|---|
| Core: document, parametric graph, scheduler, undo, file format | Our own | — | Uses rayon to run work on all cores |
| Command framework | Our own, modelled on Serpentine3D's | — | See section 9 |
| Exact geometry (B-rep / NURBS) | **solidcore**, primary | Apache-2.0 | Young (0.0.x). We contribute fixes upstream |
| NURBS curves and surfaces library (to evaluate) | **curvo** | check | Actively developed Rust NURBS modelling library (Oct 2026). Possible source of curve and surface algorithms for Freeform, alongside solidcore |
| Second kernel / reference | **truck** | Apache-2.0 / MIT | Older Rust B-rep kernel (last release Sept 2024). A cross-check for tests, or a fallback for single operations, behind the same interface |
| Mesh booleans (later stage, section 10d) | **csgrs** (decision 0008) | MIT | **manifold-rust** to be evaluated alongside it when that stage starts |
| SubD (later stage, section 10d) | **subdiv-kernels** | MIT / Apache-2.0 / Zlib / BSD-3 | Pure-Rust Catmull–Clark with creases and the true limit surface, matching OpenSubdiv. Very young |
| Sketch constraints | Reuse **SolidCraft's** Rust constraint solver | MIT / Apache | Our own solver later only if needed |
| BIM model (Architecture) | **Our own**, built on IFC 4.3 concepts: storeys, walls, slabs, openings, types, property sets | — | No stable open-source BIM base exists in Rust. BIMCraft, Rustit and Bonsai are references only |
| IFC import / export | **openbim-ifc** crates (Rust, actively developed) as the first candidate | check | IFC2x3, IFC4 and IFC 4.3. Tested against the official schemas and buildingSMART samples |
| STEP | Reuse SolidCraft's STEP reader and writer | MIT / Apache | Main exchange format with other CAD tools |
| Rhino 3DM files | Our own Rust reader and writer, from the published openNURBS file format | — | High priority for Freeform. STEP until it's ready |
| Native file container | **redb** or a zip-style package | MIT / Apache | See section 8 |
| Viewport | **wgpu** (Vulkan / Metal / DX12) | MIT / Apache | GPU drawing with instancing, level of detail and culling |
| Interface | **egui** to start. Look at Slint or iced as well | MIT / Apache | Must support a different layout per specialization |
| Scripting | **Rhai** for user scripts. **WebAssembly plug-ins** through wasmtime for extensions | MIT / Apache | Visual (node-based) scripting later, as shared core |
| AI agents | **MCP server** on the command layer | — | Agents use the same commands as the interface |
| Photoreal rendering (**exception**) | **Blender / Cycles**: export the scene as USD or glTF and render in Cycles, run as a separate program | Apache-2.0 (Cycles) / GPL (Blender) | The only place C++ and Python are allowed. Curva talks to it through files or a socket, never by linking |

## 7. Performance and the kernel

### Multicore strategy

- **Parallel recompute across objects.** When an input changes, all dependent objects rebuild at the same time, in dependency order. SolidCraft uses the same pattern (`crates/kernel/src/par.rs`).
- **Parallel meshing for display** on all cores.
- **Background work.** Geometry is computed off the interface thread. The viewport shows the last good state until new results arrive.
- **Instancing.** Blocks and types are computed once and drawn many times.
- **Incremental rebuilds.** Only objects whose inputs changed are recomputed. Results are cached by their inputs.
- **Accepted limit:** a single heavy operation (one complex fillet) stays mostly single-core. That's true of every CAD kernel, commercial ones included.

### The kernel interface

The app talks only to a Curva geometry interface:

- **Create:** extrude, revolve, sweep (1 and 2 rails), loft, network surface, patch, primitives
- **Combine:** union, subtract, intersect, split
- **Modify:** fillet, chamfer, shell, offset, rebuild, trim, untrim, extend
- **Query:** measure, bounding box, closest point, intersections, curvature
- **Output:** display mesh, STEP / 3DM export

Each operation has a primary implementation (solidcore) and can have a second Rust implementation (truck) behind a feature flag.

**Testing:** every operation is checked against known answers (volume, area, centre of mass, bounding box), as SolidCraft does. Where both kernels support an operation, their results are compared.

## 8. File format

**Curva has its own native format, `.curva`. IFC is its building vocabulary and main BIM exchange format, not its native file.**

### Why not IFC as the native format

| Curva needs | Can IFC hold it? |
|---|---|
| Walls, slabs, openings, types, properties | Yes, this is what IFC is for |
| Freeform NURBS surfaces | Partly. Stored only as fixed shapes, with no editing history |
| Parametric history, constraint sketches | No. IFC stores results, not the recipe |
| Drawing sheets and layouts | Weak, and rarely survives between programs |
| Render materials and meshes | Basic only |
| Speed on large models | The `.ifc` text format is large and slow, with no way to save only what changed |

### What a `.curva` file holds

- **The document:** objects, their parameters, history and the dependency graph
- **Sketches and constraints**
- **Exact geometry** in solidcore's own format, cached so files open without recomputing
- **Display and render meshes** as compact binary buffers in the style of glTF
- **BIM data** using IFC 4.3 class and property names internally, so IFC export is close to lossless
- **Views, layouts and sheets**
- **Smart object definitions** used in the file, with all their representations, so a file opens anywhere without the online library
- **Materials,** with both drawing hatches and render properties
- **Metadata:** units, layers, levels, project information, file version

### Container

- **Pure Rust.** Option A is **redb**, an embedded database that saves only what changed and is crash-safe. Option B is a **zip-style package**, simpler and easy to inspect.
- Decide during Milestone 1 by measuring save and open times.
- **Versioned from day one.** Newer Curva versions always open older files.

### Exchange formats

| Format | Use |
|---|---|
| 3DM | Rhino native. High priority for Freeform |
| STEP | Exact solids with other CAD tools |
| IFC 2x3 / 4 / 4.3 | BIM exchange (Architecture) |
| USD / glTF | Scenes for Blender / Cycles |
| DXF / PDF / SVG | Drawings and sheets |
| OBJ / STL | Meshes |

**IFC5** (buildingSMART, alpha stage) is a lighter, JSON-based, layered format inspired by USD. Revisit aligning with it once it's final.

## 9. Building Freeform from Serpentine3D's commands

**Serpentine3D** (https://github.com/chisomobanzi/Serpentine3D, MIT, © 2026 Chisomo Banzi) already implements about 248 Rhino-style commands with Rhino's names and aliases. Curva uses it as the blueprint for Freeform:

- **What we port:** the command list, names and aliases, each command's prompts, options and interaction flow, and the geometric logic. All of it is rewritten in Rust on solidcore.
- **Licence:** MIT allows this. Curva keeps Serpentine's copyright and licence notice in its `ATTRIBUTION.md` for anything translated from it.
- **Its command framework maps well to Rust.** In Serpentine a command is a sequence of input requests: "pick a point", "pick objects", "enter a number", "choose an option". The command line, viewport clicks and AI agents all answer those requests the same way. In Curva each command becomes a Rust state machine with the same requests, so the interface, scripts and MCP all drive it identically.
- **Acceptance testing:** the user, an experienced Rhino user, checks each command against how Rhino behaves. Rhino's behaviour is the target; Serpentine is the starting point.

### Serpentine3D command inventory (248 commands), by Curva phase

**Phase F1 — Core modelling**

- **Curves (12):** line, polyline, curve, interpcrv, circle, arc, ellipse, rectangle, closecrv, point, divide, tweencurves
- **Transform (16):** move, copy, rotate, scale, scalenu, mirror, arraypolar, arraypath, array, orient, orient3pt, rotate3d, setpt, projecttocplane, scale1d, scale2d
- **Edit (28):** delete, join, offset, fillet, filletcorners, explode, split, trim, rebuild, insertknot, removeknot, removecontrolpoint, flip, dir, hide, show, selall, selnone, undo, redo, rename, layer, material, recordhistory, plugins, boundingbox, smooth, chamfer
- **Select (16):** selcrv, selhatch, selsrf, selpt, selsolid, selpointcloud, sellayer, selname, sellast, invert, isolate, unisolate, selfilter, selfiltertoggle, seldup, selprev
- **File (6):** save, open, import, export, setdefaultapp, new
- **View (60):** standard views, zoom commands, display modes, clipping planes, viewports, cplane, grid, snaps, osnap, ortho, gumball, measurements (distance, area, volume, length), curvature and zebra analysis, named views, view capture, picture frame, tolerance, units

**Phase F2 — Surfaces and solids**

- **Surfaces (8):** extrude, revolve, loft, planarsrf, sweep1, offsetsrf, shell, sweep2
- **Surfaces 2 (17):** patch, blendcrv, project, pull, helix, textobject, unrollsrf, pipe, edgesrf, dupborder, dupedge, untrim, extractisocurve, dupfaceborder, extractsrf, extendsrf, blendsrf
- **Solids (5):** box, sphere, cylinder, cone, torus
- **Solid editing (9):** filletedge, chamferedge, cap, intersect, contour, section, booleansplit, mergeallcoplanarfaces, pushpull
- **Booleans (4):** booleanunion, booleandifference, booleanintersection, curveboolean

**Phase F3 — Deformation, analysis and organisation**

- **Deform (8):** twist, taper, bend, flow, extend, matchcrv, curvaturegraph, draftanalysis
- **Organise (26):** group, ungroup, lock, lockother, unlockall, block, insert, blocklist, count, pointcloud, meshtobrep, breptomesh, purge, what, matchprops, linetype, printwidth, setusertext, getusertext, sectionhatch, bringtofront, sendtoback, bringforward, sendbackward, changelayer, audit

**Phase F4 — Layouts and drafting (becomes shared core)**

- **Drafting (28):** layout, detail, detailscale, detailmode, detaillock, detailborder, detaildelete, make2d, printsheets, exportpdf, text, dim, leader, hatch, hatchpatterns, dimradius, dimdiameter, dimangle, titleblock, scalebar, detailsection, exportdxf, exportsvg, dimstyle, annotedit, sheetindex, revision, dot

**Not ported as-is:** camera, turntable and turntableui (replaced by Curva's own viewport and the Blender bridge); help and ai (rebuilt for Curva's own help and MCP).

## 10. Roadmap

**Build order.** First, a demo that proves Curva is possible: NURBS modelling and an interface on par with Rhino (M1 and M2). Then layouts and drafting (M3). Then a first try at the Architecture specialization (M4). Mesh and SubD come only after that (section 10d).

| Milestone | Content |
|---|---|
| **M1 — Freeform foundation** | Core, viewport, command framework, `.curva` file, phase F1 commands, history on curves |
| **M2 — Freeform surfaces and solids** | Phases F2 and F3: surfaces, solids, booleans (live), deformation, analysis, blocks. 3DM import / export |
| **M3 — Layouts and drafting** | Phase F4: layouts, detail views, Make2D, dimensions, hatches, PDF / DXF / SVG. Becomes the shared views-and-sheets core. Ships as 0.4.3 (decision 0020) |
| **M4 — Architecture** | Specialization 2: **smart object representations** (section 5b; moved from M3 by decision 0020), levels, walls, slabs, openings, doors, windows, types; live plan and section views; IFC. The Serpentine BIM tool specs carry over. Ships the **built-in object library**: wall and door styles, windows, simple trees and people, each with drawing representations |
| **After M4 — Mesh and SubD** | Mesh and SubD objects (section 10d). Not part of the demo |
| **M5 — Urban** | Specialization 3, with visual scripting as shared core |
| **M6 — Analysis** | Specialization 4, starting with sun and shadow |
| **Rendering** | Blender / Cycles bridge, whenever it's useful from M2 on |
| **Far future — Curva online library and Curva Pro** | Section 10b |

**Designed in from M1, even though it's built later:** the object model must already allow a definition with several representations and placed instances that refer to it. That way M4 doesn't need a rewrite.

### Milestone 1 — "Freeform foundation" in detail

**Goal:** a fast, solid Rhino-style base: the core, the command framework and the first command set, all in Rust on solidcore.

**Scope:**

- Document, layers, undo / redo, selection, parametric dependency graph with multicore recompute
- wgpu viewport: four Rhino-style viewports, display modes (wireframe, shaded, ghosted), construction planes, grid, object snaps, ortho, gumball
- Rhino-style interface: command line with autocomplete and aliases, properties panel, layers panel
- Command framework (section 9), driven equally by the command line, viewport clicks, Rhai scripts and MCP
- Native MCP server: read the scene, run commands, render a view to an image
- Phase F1 commands: curves, transforms, editing, selection, views, file
- History: curves built from other curves update when their inputs change
- `.curva` save and open. Both container options are measured

**Test:** a scene of several thousand curves and simple solids.

**Success criteria (targets to confirm):**

- Every F1 command behaves like Rhino, judged by the user
- Orbiting stays smooth (aim for 60 fps) and picking and snapping feel instant
- History updates and recompute use all cores without freezing the interface
- `.curva` files save and reopen quickly with the same result
- Any solidcore failure is logged with a reproducible case and reported upstream
- The whole build contains no C++ or Python

## 10b. Far future: online library and Curva Pro

An **open-core model**: Curva itself stays free and open source, and paid online services fund its development.

| Curva (free, open source) | Curva Pro (subscription) |
|---|---|
| The full application, every specialization | Everything in Curva |
| A built-in library of the basic, necessary objects: wall styles, door and window styles, simple trees and people, each with drawing representations | A large online library of smart objects: furniture, vegetation, people, fixtures, façade systems, urban elements |
| Making, saving and sharing your own `.curvaobj` objects | Online services, for example cloud sync of libraries across machines, team libraries, and later possibly cloud rendering or collaboration |

**Principles for this, decided early:**

- **Never paywall basics.** Everything needed to model and draw a real project stays free. Pro adds breadth and convenience.
- **The object format stays open** (`.curvaobj`), so the community and manufacturers can publish objects too. A marketplace for third-party objects is a possible later step.
- **Files never depend on the subscription.** A `.curva` file carries the definitions of the objects it uses, so it opens for anyone, with or without Pro.
- **Code and content are licensed separately.** The code is MIT OR Apache-2.0. Free library objects use an open content licence (for example CC-BY). Pro library content is licensed to subscribers.
- **Before going commercial, clear the name** (section 10c).

## 10c. Name

**Curva** (Italian, Spanish and Portuguese for "curve"). It's the working name, chosen 8 November 2026.

**Why:** short and memorable in the style of Linea and Plasticity, easy to say in any language, and it names the freeform base the whole suite grows from.

**Why not "Dex":**

- Samsung DeX is a well-known product with almost the same name.
- `.dex` is Android's executable file format.
- The name is crowded: an open-source login system, the crypto term "DEX", and many apps.

**First check (8 November 2026):**

| Item | Result |
|---|---|
| 3D / CAD / design software named Curva | None found |
| Rust package `curva` | Free |
| Domains | curva.com, curva.app, curva.org, getcurva.com, usecurva.com, curva.xyz registered. curvacad.com, curva3d.com, curvadesign.com looked free |

**Watch-outs:**

- **Linearity Curve,** a 2D vector design app, uses "Curve", trademarked with the company name in front.
- **The Rust NURBS library `curvo`** has a near-identical name in the same niche. If Curva uses it, documentation must keep the two clearly apart.
- **"Curva" is a plain word** in Italian, Spanish and Portuguese, so it may be considered descriptive for curve-modelling software in those markets.

**Before anything commercial (Curva Pro):**

- Run a trademark search for classes 9 and 42 in TMview (EU and national registers), USPTO and WIPO, ideally with an IP lawyer.
- If plain "Curva" is too weak or conflicted, use a compound such as **Curva3D** or **CurvaCAD**.
- Register the domains and GitHub organisation early.

## 10d. Mesh and SubD (later stage)

This comes after the NURBS demo and the first try at the Architecture specialization. Display meshes for the viewport are not affected: they are needed from M1. This section is about mesh and SubD **objects** the user models with.

- **SubD smoothing:** **subdiv-kernels**. Catmull–Clark subdivision with creases and the true limit surface, matching OpenSubdiv (the industry standard), tested against OpenSubdiv on every change. Permissive licence. Version 0.1.0, so expect changes.
- **Mesh booleans:** **csgrs** first, as decision 0008 chose. Evaluate **manifold-rust** alongside it: a pure-Rust port of Manifold, one of the most reliable mesh boolean engines, Apache-2.0, and it also handles messy non-manifold imported meshes.
- **Remeshing and simplification (optional):** **baby_shark**, for cleaning up heavy imported meshes. Its booleans work through voxels and are approximate, so they are not used for modelling. Check its licence first.
- **Editable mesh / SubD cage:** Curva's own polygon (n-gon) mesh structure. No existing pure-Rust library fits well (the `halfedge` crate assumes triangle meshes), and this structure has to carry Curva's object memory anyway. It feeds the libraries above.
- **SubD to NURBS** conversion, as Rhino does, is a separate question for after that, tied to the NURBS kernel.

Re-check all of these when the stage starts: they are young and the landscape will have moved.

## 11. Prior art to learn from

| Project | What to take from it |
|---|---|
| **Serpentine3D** (Python, MIT) | Blueprint for Freeform: command list, aliases, interaction flows and geometric logic, ported to Rust with attribution (section 9) |
| **Rhino** | The behaviour target for Freeform |
| **SolidCraft** (Rust, MIT/Apache) | Parametric timeline, constraint solver, STEP, parallel kernel pool, wgpu viewport, command and MCP layer |
| **solidcore** and **truck** (Rust) | The kernels |
| **Archicad, Vectorworks, Revit** | Behaviour targets for Architecture: views, sheets, element relationships. Their smart objects (GDL objects, hybrid symbols, families with detail levels) are the reference for section 5b |
| **Enscape and similar asset libraries** | Reference for light viewport proxies with heavy render models, and for an online object library |
| **VisualARQ** and **RSTool** (docs only) | How architectural objects behave on a Rhino-style base: parameters stored on the object, plain-solid conversion, openings held by walls |
| **BIMCraft**, **Rustit** (Rust) | References only for the BIM model. Judged too early as bases |
| **openbim-ifc** (Rust) | Candidate IFC library |
| **Bonsai**, **FreeCAD** | References for open BIM workflows, and for what to avoid (sequential recompute, workbenches as separate worlds) |

SolidCraft and solidcore are about a month old (started October 2026). Re-check their maturity regularly.

## 12. Risks and mitigations

| Risk | Mitigation |
|---|---|
| **Freeform first puts the most demand on the youngest part: the kernel.** Robust NURBS intersections, trims and curved fillets are solidcore's weakest areas today | Start with operations solidcore already handles. Log every failure with a reproducible case. Make kernel fixes a standing priority and contribute them upstream. Use truck as a second implementation where it helps |
| Rust libraries are younger than their C++ equivalents (mesh booleans, 3DM, IFC) | Choose actively maintained crates. Wrap each behind our own interface. Write our own where none exists (3DM) |
| solidcore or SolidCraft stall | Kernel interface. Permissive licences allow forking |
| Scope is enormous (Rhino + Archicad + urban + analysis) | One milestone at a time, each usable on its own. Specializations share the core, so each one adds tools, not a new program |
| Specializations drift into separate worlds | The shared-vs-unique rule in section 3, checked at every milestone |
| The native format locks users in, or old files break | 3DM, STEP, IFC and USD export. Format versioning, with tests that open every older version |
| Downloaded smart objects contain code | Object logic runs only in the Rhai / WebAssembly sandbox, with no file or network access |
| An AI agent makes unwanted changes through MCP | One undo step per agent action, approval for destructive actions, visible activity |
| AI-written code nobody fully reviews | Strong tests, Rust's compiler checks, known-answer tests, CI as the gate, and the user's hands-on acceptance of every command |

## 13. Open decisions

1. **Build on SolidCraft's code, or start fresh** while reusing parts of it? Test it hands-on first.
2. **`.curva` container:** redb vs zip-style package. Measure both in Milestone 1.
3. **Interface toolkit:** egui vs Slint vs iced. It must handle a different layout per specialization.
4. **IFC library:** openbim-ifc, or our own. Check its licence and IFC 4.3 coverage.
5. **Mesh boolean library:** answered by decision 0008 (csgrs first). Re-check with manifold-rust when mesh work starts (section 10d).
6. **Licence:** suggest MIT OR Apache-2.0, to match the Rust ecosystem and allow reusing SolidCraft and Serpentine3D code.
7. **Rhino licensing for automated reference generation:** check Rhino's licence terms for scripted, unattended use on the Windows machine (section 15).
8. **A Mac for hands-on checks:** CI covers macOS builds and tests, but a real Mac (for example a Mac mini) is eventually needed for feel and display checks. An Apple Developer account is needed to sign and notarise the macOS app.

## 14. Link to the Serpentine work

- **Freeform:** Serpentine3D's commands are the blueprint (section 9).
- **Architecture:** the BIM tool specs written for Serpentine carry straight over to M4:
  - commands that pick points on the workplane
  - walls, slabs, doors, windows and the Level Manager
  - Sketch Mode
  - two-step Explode

## 15. Development by AI agents: automated testing

**Goal:** agents write the code, and machines check it. The user doesn't test command by command. He reviews summaries and spot-checks.

### The test layers

| Layer | What it checks | How |
|---|---|---|
| **Unit and known-answer tests** | Each geometry operation gives the right result | Volume, area, centre of mass, bounding box, control points against hand-computed answers |
| **Property-based tests** | Rules that must always hold, on thousands of random inputs | `proptest`. Examples: moving an object never changes its volume; a closed solid stays closed after a fillet; undo restores the exact previous state |
| **Command scripts** | Every command, end to end, without the interface | Each command has scripted cases (inputs, options, expected result) run through the same command layer as the UI and MCP |
| **Rhino comparison** | Curva behaves like Rhino | See below |
| **Visual snapshots** | Viewports, plans and sheets look right | Render to an image without a window and compare with a stored reference image. Differences beyond a tolerance fail the test |
| **Interface tests** | Panels, the command line and dialogs work | Automated UI tests (for example `egui_kittest`) that click and type |
| **File tests** | Files open, save and convert correctly | Round trips (`.curva` → save → open = identical), every older format version opens, 3DM / STEP / IFC exchange |
| **Fuzzing** | File readers and the command line never crash on bad input | `cargo-fuzz` on importers and parsers |
| **Performance benchmarks** | Speed never quietly gets worse | Benchmarks for recompute, meshing, open/save and viewport. CI fails on regressions beyond a set threshold |

### Rhino as the automatic reference

The Windows 11 machine with Rhino and Rhino MCP becomes the **reference machine**:

1. Each command case is written once as a neutral script: inputs, options, expected kind of output.
2. A harness runs the script in **Rhino** through Rhino MCP and records the result: geometry measurements, control points, topology counts and an image.
3. The results are stored in the repository as **reference files** (goldens).
4. CI runs the same scripts in **Curva** on every change, on all three systems, and compares against the stored Rhino results. No Rhino is needed in CI.
5. References are regenerated on the Windows machine only when cases are added or Rhino's behaviour is deliberately re-checked.

### What the user sees

- **A comparison gallery,** generated after each batch of work: Curva and Rhino side by side for every command case, with failures and differences highlighted. Reviewing it takes minutes, not days.
- **A weekly status report:** what was added, test counts, failures, performance trends, open kernel issues.
- **Spot checks:** the user tries the parts that matter most to him by hand. Anything that feels wrong becomes a new automated test case.

### Rules for agents

- **Nothing merges unless CI passes** on Linux, Windows and macOS.
- **Every new command or fix comes with its tests.** A bug fix starts with a test that reproduces the bug.
- **Reference files and images change only on purpose,** with the reason stated in the pull request.
- **A separate reviewer agent** checks each pull request against the readability rules (section 17) before merge.

## 16. Platforms and development machines

### Windows, macOS and Linux

- **The stack is cross-platform by design:** Rust, wgpu (Vulkan on Linux and Windows, DirectX 12 on Windows, Metal on macOS) and egui all run natively on all three.
- **CI builds and tests on all three** on every change, using GitHub's Linux, Windows and macOS machines.
- **CI machines have no graphics card,** so rendering tests use software rendering: Mesa's software Vulkan driver on Linux and WARP on Windows. GPU-specific checks run on the development machines.
- **Releases:** an AppImage and Flatpak for Linux, an installer for Windows, and a signed and notarised app for macOS (needs an Apple Developer account).
- **No platform-specific code in the core.** Anything platform-specific (file dialogs, 3D mouse, installers) sits in a small, clearly marked layer.

### Which machine does what

| Machine | Role |
|---|---|
| **Windows 11 PC with Rhino**, primary | Main development machine **and** Rhino reference machine. Claude Code runs natively on Windows, not in a VM, with the Rust MSVC toolchain. Agents can run Rhino comparisons locally through Rhino MCP while they work, and the real GPU is available for viewport work (DirectX 12 and Vulkan) |
| **Linux PC (Fedora Atomic)** | Hands-on Linux checks, and a second machine for an agent working in parallel. The Rust toolchain installs in the home folder (rustup) or a toolbox / distrobox container |
| **macOS** | GitHub's macOS machines for builds and tests. A real Mac later for hands-on checks |
| **GitHub and cloud agents** | The repository and CI. Cloud agents can also work on issues in parallel |

**Windows setup notes:**

- **Develop natively on Windows** (Rust, Git and Claude Code installed directly). The Ubuntu VM in VMware isn't needed for Curva.
- **WSL2 is optional.** It needs Windows' own hypervisor, which was switched off earlier so VMware could pass virtualization through to the Ubuntu VM. Turning it back on breaks that VM's Cowork shell. Linux testing is covered by CI and the Linux PC, so WSL2 isn't required.
- **Use a Dev Drive** (Windows 11's ReFS developer volume) for the repository and build folders, and exclude it from antivirus real-time scanning. Rust builds create many small files, and both make a large difference to compile speed.

## 17. Readable, maintainable code

AI agents can produce huge amounts of code quickly. SolidCraft reached over a million lines in a month. Curva aims for the opposite: **less code, clearly organised, that a person can read.**

### Structure

- **Small crates with one job each,** arranged in layers. A higher layer can use a lower one, never the reverse. The layer rules are checked automatically in CI.
  - kernel interface
  - geometry
  - document
  - commands
  - interface
  - specializations
- **One command per file,** written from the same template: prompts, options, steps, tests. Reading one command teaches you all of them.
- **Short files and functions,** with limits enforced by linting. When a file grows past the limit, it's split.
- **A map of the code:** `ARCHITECTURE.md` explains every crate in plain language, and `AGENTS.md` / `CLAUDE.md` give agents the rules.

### Clarity rules

- **Names say what things are.** No abbreviations beyond standard ones (NURBS, UV, CPlane).
- **Every public item is documented** (`#![deny(missing_docs)]`). Comments explain why, not just what.
- **Every geometric algorithm cites its source,** for example a section of *The NURBS Book* or a paper, and complex ones get a diagram in the docs.
- **No `unsafe` code, no hidden global state, no copied or generated blobs** in the source tree.
- **Formatting and linting are enforced:** `rustfmt`, and `clippy` with warnings treated as errors.
- **Dependencies are added deliberately,** with a reason, and checked for licence and maintenance (`cargo-deny`).

### Keeping it clean over time

- **Architecture decision records:** every significant decision gets a one-page note in `docs/decisions/` saying what was decided, why, and what was rejected.
- **Pull requests explain themselves:** what changed, why, how it was tested.
- **Regular clean-up passes:** after each milestone, an agent removes dead code, merges duplicates and updates the docs, with all tests still passing.
- **Measure size, not just features:** CI reports lines of code per crate, so unexplained growth gets noticed.

## 18. Agent workflow and guardrails

How a project this size runs smoothly with AI agents, with the user making decisions rather than doing checks.

### The work queue

- **GitHub Issues are the backlog.** Each issue is one small, well-defined task, sized for a single agent session.
- **Every issue has acceptance criteria:** what "done" means, which tests must exist and pass, and which Rhino reference cases apply.
- **Labels drive the flow:**
  - `agent-ready`: an agent may pick it up
  - `in-progress`: claimed, with the claiming pull request linked
  - `needs-human`: waiting for a decision from the user
  - `blocked`: waiting on something else, with the reason stated
- **Milestones group issues** to match the roadmap (M1, M2, …).

### Spec first, then code

- **Larger features start with a one-page spec** in `docs/specs/`: what it does, how it should feel (with Rhino references), edge cases and test cases. The user approves the spec. Agents then implement against it.
- **Plan before editing.** For anything non-trivial, the agent writes a short plan (files touched, approach, tests) before changing code. Claude Code's plan mode is designed for this.
- **Small pull requests.** One concern per pull request, ideally a few hundred lines. Big changes are split.

### Guardrails against runaway loops

- **Retry limit:** if the same test keeps failing after **3 genuine attempts** at a fix, the agent stops. It writes up what it tried and what it suspects, labels the issue `needs-human` or `blocked`, and moves on to other work. No endless retry cycles.
- **Time and budget limits per task:** each task has a session time and usage budget. When it runs out, the agent hands over with notes instead of continuing.
- **Flaky tests are quarantined, not retried.** A test that passes and fails without code changes is moved to a quarantine list with its own issue, so it can't block everyone or tempt agents into retry loops.
- **No progress means stop.** If two consecutive attempts produce the same failure output, the agent must change approach or escalate.

### Guardrails against cheating tests

AI agents under pressure sometimes "fix" a failing test by weakening the test instead of the code. Forbidden, and checked automatically:

- **Tests, reference files and Rhino goldens are protected:**
  - Changes that weaken them need the user's say-so, in conversation (decision 0007).
  - CI flags any pull request that deletes tests, loosens tolerances or marks tests as ignored.
- **No special-casing:** code must never detect test inputs to return expected answers. The reviewer agent checks for this explicitly.
- **Mutation testing** (`cargo-mutants`), run nightly, deliberately breaks the code in small ways and checks that the tests notice. Tests that don't notice anything are weak, and get flagged for improvement.

### Guardrails on the machines

- **Claude Code permissions:** an allowlist of safe commands (build, test, git on branches). Dangerous ones are denied: force-push, deleting branches, touching files outside the repository, installing system software.
- **Claude Code hooks** run automatically:
  - after an edit: formatting and a fast lint
  - before finishing: the relevant tests
  - before risky commands: a block
  This way agents can't skip the basics.
- **Protected `main` branch:** changes arrive only through pull requests that pass CI on all three systems and the reviewer agent.
- **No secrets in the repository,** and tests never use the network.
- **Pinned toolchain** (`rust-toolchain.toml`) and lockfile, so every machine and agent builds the same way.

### Parallel agents without collisions

- **One agent, one issue, one branch, one Git worktree** (a separate checkout), so agents working at the same time never edit the same files in the same folder.
- **Claim before starting:** an agent opens a draft pull request linked to the issue before writing code. Others skip claimed issues.
- **Shared interfaces change in their own pull requests,** merged first, before the work that depends on them.

### Memory between sessions

Agents start each session without memory of the last one. The repository carries the memory:

- **`AGENTS.md` / `CLAUDE.md`:** the rules (short, kept current). Per-crate files hold local rules where needed.
- **`ARCHITECTURE.md`:** the map of the code.
- **`docs/decisions/`:** why things are the way they are.
- **`STATUS.md`:** current state, what's in progress, known problems and handoff notes. Updated at the end of every session.
- **Fresh context per task:** each task starts a new session that reads these files, rather than one endless session that drifts.

### Automation that runs without anyone

| When | What runs |
|---|---|
| Every pull request | Build, lint and all tests on Linux, Windows and macOS. Layer checks. Reviewer agent |
| Nightly | Fuzzing, mutation testing, full benchmarks, the visual comparison gallery, dependency and security audit (`cargo-deny`, `cargo-audit`) |
| Weekly | Status report to the user. A clean-up agent pass (dead code, duplicates, docs) |
| When Rhino cases change | Regenerate Rhino references on the Windows machine |
| On release | Build and sign installers for all three systems, changelog generated from pull requests |

Claude Code can run headless (`claude -p`) in scripts and scheduled jobs, and its GitHub integration lets agents respond to issues and pull requests directly.

### What the user decides

Agents escalate only these, in a short `needs-human` issue with options and a recommendation:

- product and interface decisions (how a tool should feel)
- approving specs
- architecture decisions, recorded as decision notes
- changes to tests and Rhino references
- new dependencies with licence implications
- releases

Everything else proceeds automatically.

### Cost awareness

- **Match the model to the task:** the most capable model for architecture, the geometry kernel and hard bugs; faster, cheaper models for routine work (docs, simple commands, clean-up).
- **Track usage per milestone** in the weekly report, so costs stay visible.

## 19. The project owner and how agents work with him

### Who he is

- **A practising building architect,** not a software developer.
- **An expert user** of architecture and design software: extremely experienced in Rhino, and familiar with Archicad, Revit, VisualARQ and similar tools.
- **Very limited knowledge of programming and software systems.** He doesn't read code, and he shouldn't need to.

### What that means for agents

- **He's the authority on how Curva should feel and behave.** Tools, workflows, drawing conventions and interface: agents defer to his judgement and to Rhino / Archicad behaviour he describes.
- **Agents own the technical decisions,** within this architecture document. They record significant ones in `docs/decisions/` in plain language, so he can follow the reasoning without technical background.
- **Plain language, always.** No unexplained jargon. When a technical term is unavoidable, explain it in one short sentence the first time. Explain trade-offs in terms of what a user would notice: speed, reliability, what a tool can or can't do.
- **Show, don't describe.** Pull requests and reports that change anything visible include screenshots or short screen recordings, plus Curva-versus-Rhino comparisons where relevant.
- **Decisions come as choices.** When his input is needed: a short summary, two or three options with their consequences, and a clear recommendation. He shouldn't have to research anything to answer.
- **Never ask him to read code** or to judge a technical detail he can't evaluate. Turn it into a question about behaviour or outcome instead.
- **If he must run something himself** (installing software, approving a login, changing a setting), give exact step-by-step instructions, with every command ready to copy and paste as-is, and say what he should see when it worked.
- **Be honest about problems and limits.** Say clearly when something failed, is fragile or isn't possible yet. Never overstate progress.
- **Keep reports short:**
  - what changed
  - what to look at
  - what needs his decision
  - anything that went wrong

### What he provides

- **Command and tool concepts,** written as specs: how a command should work, its options, how it should feel. These are the source of truth for behaviour.
- **Approval** of specs, interface direction, anything that weakens a test or reference, and releases.
- **Spot checks** of the comparison gallery and hands-on use, with findings turned into test cases by agents.
- **Real project files and workflows** to test against (heavy Rhino models, typical drawings).

## 20. Other things to plan for early

These are cheap to get right at the start and expensive to fix later.

### Precision and large coordinates

- **Architecture and urban models often sit far from the origin,** for example at real map coordinates. Plain floating-point numbers then lose precision, and the viewport jitters. Design for this from M1:
  - double precision in the model
  - a local origin per document or site
  - drawing relative to the camera in the viewport
- **Units and tolerances per document** (millimetres, metres, inches, feet), as in Rhino, with drawing scales kept separate from model units.

### Never lose work

- **Autosave and crash recovery** from M1: periodic background saves, and recovery of the last session after a crash.
- **Undo that never breaks:** every command, including agent actions over MCP, is undoable. This is tested automatically (section 15).
- **Crash reports are opt-in only:** sent only with the user's permission, with no model data unless the user attaches it.

### Languages

- **Prepare for translation from day one:** no interface text hard-coded in the source, with all wording kept in translation files. English first; Portuguese and other languages can follow without code changes.
- **Locale-aware numbers and units** (decimal comma versus point).

### Documentation for users

- **The user manual is generated from the command specs** and kept in the repository, so it never falls behind the software: every command with its options, a short explanation and an image.
- **"Coming from Rhino" and "Coming from Archicad" guides** mapping familiar commands and concepts to Curva's.

### Legal hygiene

- **Clean-room behaviour, not copying.** Matching Rhino's command names and behaviour is fine. Copying Rhino's (or any proprietary product's) code, documentation text, icons or interface artwork is not.
- **Reading and writing 3DM** is based on the published openNURBS file format.
- **Contributions:** use a Developer Certificate of Origin ("Signed-off-by" on commits) so the origin of all code is clear.
- **Attribution:** keep `ATTRIBUTION.md` current for everything ported or reused (Serpentine3D, SolidCraft, solidcore, etc.).

### Releases

- **Three channels:** nightly (automatic, for testing), beta, and stable.
- **Automatic updates,** with the user's consent and the option to stay on a version.
- **Version numbers that mean something:** a file saved in a newer version warns clearly when opened in an older one.

### Accessibility and comfort

- **Scalable interface,** high-contrast and dark themes, keyboard access for everything, and customisable shortcuts.
- **3D mouse support** (SpaceMouse) on all three systems.

### Keep everything in the repository

- **The project must not depend on any one chat, session or person's memory.** Specs, decisions, status, rules and tests all live in the repository. Any new agent, any new contributor, or the owner years later can pick it up from there.
- **Back up the repository** beyond GitHub, for example with a periodic mirror.
