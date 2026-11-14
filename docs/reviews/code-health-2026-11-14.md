# Code health review · 14 November 2026

The owner asked: "are we using the best strategies to keep Curva lightweight and efficient? Is the
code well organised and separate so that things can be updated like modules? I don't want to create
a software with tech debt." This reviews Curva as released in 0.4.2 (59,900 lines of Rust in 8
crates, 840 tests), before Milestone 3 adds layouts and drawings on top of it.

**Short answer:** the foundations are right: the crate layering, the rules, the single command
framework and the small download. But six things in how the model is held and drawn, and in how two
big files have grown, would turn into real tech debt as models and Milestone 3 grow. All six can be
fixed now, cheaply, before Milestone 3 builds on them.

## What is already right

- **Layers that only point one way.** kernel ← document ← commands ← mcp / viewport ← app. The
  geometry engine (solidcore) is reached only through `curva-kernel`, so it can be updated or
  swapped in one place. Nothing reaches upwards.
- **Few outside dependencies,** each with a written reason in its `Cargo.toml`; 368 crates in all,
  most of them from egui and wgpu. Pure Rust, no `unsafe` (forbidden in the workspace), no C
  libraries.
- **Strict rules enforced by CI:** every public item documented, clippy warnings as errors,
  `cargo-deny` on licences and advisories (it caught the unmaintained font library in 0.4.2).
- **One command framework** drives buttons, the command line, scripts and MCP agents alike; every
  command is a state machine with Rhino's steps, checked against `rhino-reference.md` by a test.
- **Text kept out of code:** every message is in `locales/en.txt`, ready for other languages.
- **A small program:** the Windows installer is 18.8 MB, the Linux AppImage 26 MB, the macOS image
  26.6 MB.
- **Work spread over cores** where it matters: meshing many solids at once with rayon.

## What would become tech debt

Ranked by how much it would cost later.

### 1. Solids are kept as text and re-read for every operation

`curva_kernel::Body` holds each solid as the engine's native format written out as a string, and
`Body::engine()` parses that string back into the engine's structure every time anything is asked of
the solid: meshing, picking, snapping, a boolean, a fillet, a bounding box. For a large solid that is
milliseconds of parsing per question, repeated many times per command, and the text takes more memory
than the solid itself.

**Fix:** hold the engine's solid itself, shared behind an `Arc`, and write the text only when saving.
Parsing happens once, when a file opens.

### 2. Every undo step copies the whole model, with no limit

`Document::record_change` pushes a full copy of `State` (every object, every solid's text) onto the
undo list, and nothing ever drops old steps. A 50 MB model edited 100 times holds 5 GB.

**Fix:** keep objects behind `Arc`, so an undo step shares every unchanged object with the steps
around it and only stores what changed; plus a memory budget for undo (a setting) that drops the
oldest steps.

### 3. Every frame compares the whole model, and any change redraws everything

`Curva::refresh_scene` keeps a second full copy of the model and compares the whole of it to the
document on every frame to see whether anything changed. When anything does, even selecting one
object, every object's outline is rebuilt and resampled. The mesh cache also compares each solid's
full text and checks for gone objects in a way that slows down with the square of the object count.

**Fix:** the document counts its changes (a number that goes up on every change) and lists the
objects that changed, which it already half does (`last_changed`). The viewport rebuilds only those,
and only when the number moves. Selection changes recolour without resampling.

### 4. `main.rs` holds most of the app in one 5,870-line file

The `Curva` struct in `crates/curva-app/src/main.rs` has 160 functions and handles the gumball,
panels, layers, properties, settings, the AI assistant, updates, agent access, viewports, mouse
handling and the top bar. Every change touches the same file and struct, which is where tech debt
grows fastest. Milestone 3 would add the layout tabs, detail views and annotation editing to it.

**Fix:** one module per part, each owning its own state: `gumball`, `panels/layers`,
`panels/properties`, `panels/blocks`, `settings`, `assistant`, `agent`, `top_bar`, `viewports`,
`mouse`, `files` (dialogs). `main.rs` keeps only starting the program and the frame loop. No
behaviour changes; the app's tests must pass unchanged.

### 5. Commands are filed by when they were built, not by what they are

`curva-commands/src/commands/` has 112 files side by side: most are one command per file, but
`parity.rs` to `parity7.rs` (6,680 lines) hold dozens of commands each, named after the batch they
were built in. Finding a command means searching.

**Fix:** folders by the tabs a user sees, each with one file per command or per small family:
`curves/`, `surfaces/`, `solids/`, `modify/`, `select/`, `view/`, `analyze/`, `organise/`, `files/`.
The registry in `mod.rs` stays the single list.

### 6. File formats are wired straight into the commands

`curva-commands` depends on `curva-3dm`, and Open, Import, Export and SaveAs pick the format by
matching file extensions. Milestone 3 adds PDF, SVG, DXF and possibly DWG; wired the same way, each
would add another branch to every file command and another dependency to the commands crate.

**Fix:** a small format interface in `curva-document` (`read`, `write`, extensions, options), with
each format in its own crate: `curva-3dm` now, then `curva-dxf` (DXF and DWG), `curva-pdf`
(PDF and SVG drawing output). The file commands ask a registry; adding a format touches no command.
Formats can be switched off with cargo features for a lighter build.

## Smaller points

- **Old-file support is scattered:** code for files from 10 and 11 November (`StoredBody`'s record)
  and from before layers sits inside the types. Give `.curva` files a format version and move every
  upgrade into one `migrate` module, so the types stay clean and old upgrades can be retired in one
  place.
- **`curva-placeholder` is dead:** a crate left from proving the workspace on day one. Remove it.
- **Performance is measured only for orbiting.** Add benchmarks to `docs/performance.md` for
  opening and saving, undo memory, a boolean, and (with Milestone 3) Make2D, and report lines of code
  per crate in CI, as `architecture.md` §17 asks.
- **The `.curva` model is one JSON entry** with each solid's text inside it. Fine at today's sizes;
  measure it with a large model after fix 1, and move solids into binary entries of their own in the
  zip if it is slow.

## Plan

Fixes 1 to 5, the old-file `migrate` module and removing the placeholder crate make up Milestone 3's
clean-up (point 1 of `milestone-3-in-0.4.3.md`), done before any Milestone 3 feature, each in its own
pull request with before-and-after numbers. Fix 6 is done as the first Milestone 3 format arrives.
After each milestone, this review is repeated.
