# Code health review · 19 November 2026

A follow-up to the review of 14 November (`code-health-2026-11-14.md`), approved by the owner on
19 November 2026. In five days Curva has doubled in size, with layouts, drawings, Make2D, meshes,
SubDs and four new file formats. This review asks whether the code has stayed easy to find your way
around while growing that fast.

**Short answer:** the structure is still right. The layers point one way, and fixes 1 to 5 from
14 November are in. But almost all recent work was new features and very little was tidying up:
the overdue file format registry (fix 6), five files over the size limit, about sixty functions
over it, and an interface crate with few tests. None of this is hard to fix, but it gets dearer each
day. The owner has made a new rule so it gets done: every stream starts each session with one
tidy-up.

## The numbers

Measured on `main` on 19 November 2026. Code is counted without its tests (the `#[cfg(test)]`
modules and the files under `tests/`), as the size rule counts it.

| Crate | Code (lines) | Test code (lines) | Tests |
|---|---:|---:|---:|
| `curva-commands` | 22,485 | 10,310 | 503 |
| `curva-app` | 14,980 | 1,553 | 63 |
| `curva-3dm` | 12,244 | 3,068 | 71 |
| `curva-kernel` | 10,575 | 4,260 | 250 |
| `curva-viewport` | 10,130 | 3,649 | 155 |
| `curva-document` | 8,493 | 2,502 | 119 |
| `curva-mesh` | 5,323 | 2,701 | 134 |
| `curva-dxf` | 2,479 | 1,629 | 34 |
| `curva-meshfiles` | 1,238 | 377 | 20 |
| `curva-mcp` | 594 | 279 | 24 |
| `curva-skp` | 518 | 165 | 3 |
| `curva-pdf` | 269 | 128 | 6 |
| **Total, 12 crates** | **89,328** | **30,621** | **1,382** |

On 14 November Curva was 59,900 lines in 8 crates with 840 tests; it is now 119,950 lines in 12
crates with 1,382 tests. 584 crates are built in all, outside libraries included (368 then), most
of the new ones from the mesh libraries and the Rhino, DWG and SketchUp readers. The patched copies
of two outside libraries (`quadrs`, `solidcore-ops`) are not counted.

Since 14 November, 195 pull requests have been merged. 14 of them were tidy-ups: fixes 1 to 5, the
removal of the placeholder crate, and eight splits of large files and long functions. None of them
had the `tidy-up` label, and the two `tidy-up` issues still open (#178, #303) date from the first
review.

## What is right

- **Layers that only point one way.** kernel ← document ← commands ← mcp, viewport ← app, with
  the file formats (`curva-3dm`, `curva-dxf`, `curva-skp`, `curva-meshfiles`, `curva-pdf`) beside
  the commands. The new crates fitted in without anything reaching upwards: `curva-mesh` sits under
  the document, beside the kernel. solidcore is still reached only through `curva-kernel`.
- **Fixes 1 to 5 from 14 November are done** (#105 to #108, #112). Solids are kept parsed, undo
  shares unchanged objects, only what changed is redrawn, the app is one module per part, and
  commands are in folders by tab.
- **New crates are small and do one job:** a format each for SketchUp, PDF, and STL, OBJ and PLY,
  and a crate for meshes and SubDs.
- **Commands are well tested:** 503 tests for `curva-commands`, and the preview, undo and Rhino
  reference checks run for every command.
- **Patched outside code is kept apart,** with a written reason and a plan to delete it (decisions
  0036 and 0037).

## What needs fixing

### 1. Almost no tidying up while the code doubled

14 of 195 merged pull requests were tidy-ups, and all of them came in the two days after the first
review. Since then features have come in at full speed and nothing has kept the code in order: the
size limits in `AGENTS.md` are a rule that nothing checks, and no stream has a reason to stop for
them.

**Fix:** the owner's new rule (`AGENTS.md`, "Keeping the code tidy"): every stream starts each
session by finishing one open tidy-up issue in code it owns, before its next feature. And a CI
check on file and function sizes (#400), so new code cannot break the limits unnoticed.

### 2. The file format registry (fix 6) is overdue

The first review asked for it "as the first Milestone 3 format arrives". Since then DXF and DWG,
PDF, SketchUp, and STL, OBJ and PLY have all arrived, wired the old way: `curva-commands` depends
on `curva-3dm`, `curva-dxf`, `curva-skp` and `curva-meshfiles`, `curva-app` on `curva-pdf`, and the
file commands choose a format by matching extensions. Each new format makes the commands crate
heavier and every file command longer.

**Fix:** a format interface and registry in `curva-document`, which each format crate fills in
(#391). Spec first, and no new file formats until it is built.

### 3. Five files over 800 lines

| File | Lines (without tests) |
|---|---:|
| `curva-dxf/src/lib.rs` | 1,355 |
| `curva-commands/src/commands/mod.rs` | 931 |
| `curva-3dm/src/read/mod.rs` | 825 |
| `curva-3dm/src/model.rs` | 823 |
| `curva-kernel/src/hidden.rs` | 817 |

The owner's figures named four: `commands/mod.rs` crossed the line since. `curva-dxf/src/lib.rs`
holds a whole format in one file, reading and writing together.

**Fix:** #392, #393, #394 and #401.

### 4. About sixty functions over 80 lines

My count finds 59 functions over 80 lines outside tests; checking by hand, it misses some whose
first line is split (`paint_details`, 104 lines), so the owner's 64 is likely nearer. The longest:

| Function | Lines |
|---|---:|
| `curva-app` `viewports` | 200 |
| `curva-3dm` reading a mesh (`read/mesh.rs`) | 199 |
| `curva-document` text fields' `call` | 190 |
| `curva-commands` title block `placed` | 171 |
| `curva-3dm` `write_mesh`, `write_subd` | 159 each |
| `curva-app` `command_line` | 125 |
| `curva-dxf` `curve_of` | 125 |
| `curva-commands` `What`'s `make` | 121 |

Long functions are where mistakes hide: one change has to be read against two hundred lines.

**Fix:** #392, #395, #396, #397 and #398 for the longest; the rest as each file is next touched,
and the CI check stops new ones.

### 5. The interface has few tests

`curva-app` is the second-largest crate (14,980 lines) with 63 tests. `curva-commands`, a little
larger, has 503. Much of the interface needs a window to test, but working out a layout, a label or
which view to draw does not, and today that logic sits inside the long drawing functions.

**Fix:** as #396 splits the long `curva-app` functions, the logic that needs no window comes out
into plain functions with their own tests.

## Issues

All labelled `tidy-up`. Each moves and splits code only, with no change in behaviour and tests
unchanged.

| Issue | What | Who |
|---|---|---|
| #391 | File format registry in `curva-document`; the format crates register themselves. Spec first. | features |
| #392 | Split `curva-dxf/src/lib.rs` into export, import, curves and units; `curve_of` by entity kind | features |
| #393 | `curva-3dm` `read/mod.rs` and `model.rs` under 800 lines | features |
| #394 | `curva-kernel` `hidden.rs` under 800 lines | features |
| #395 | `.3dm` mesh and SubD reading and writing under 80 lines a function | mesh |
| #396 | `curva-app`'s long functions under 80 lines, with tests for logic that needs no window | features |
| #397 | `curva-document` `fields.rs` `call` under 80 lines | features |
| #398 | `title_block.rs` `placed` and `analyze/what.rs` `make` under 80 lines | features |
| #400 | CI check: new files over 800 lines or functions over 80 fail, today's listed as known | features |
| #401 | The shared helpers at the end of `commands/mod.rs` into `support.rs` | features |

Still open from the first review: #178 and #303 (the interface's long panels and functions), which
#396 continues.

## Plan

From today every stream starts each session with one tidy-up issue in code it owns (`AGENTS.md`).
The features stream writes the spec for #391 first, for the owner to approve, then builds the CI
check (#400), so that the limits are checked from then on. This review is repeated after the next
milestone, or sooner if the count of long files and functions goes up.
