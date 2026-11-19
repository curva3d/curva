# Spec: one list of file formats (code health fix 6)

**Status:** Draft, for the owner's approval in chat. The owner agreed the direction on
19 November 2026 (code health review of that day, issue #391).
**Covers:** how Curva's file commands (`Open`, `Import`, `Save`, `SaveAs`, `Export`,
`ExportSelected`, `Print`) find the formats they read and write. No format gains or loses anything.
**Rules:** a tidy-up (`AGENTS.md`): every file opens, saves and exports exactly as before, and the
tests stay as they are. Pure Rust, no new outside library.

## 0. In short

| What changes | What you will notice |
|---|---|
| Each format describes itself once, in its own crate, and joins one list of formats | Nothing: the same file types in every dialog, in the same order, with the same results |
| The file commands ask the list instead of knowing every format | Adding a format later touches only that format, never the commands |
| The commands and the interface no longer depend on the format crates | A lighter commands crate; formats can be left out of a build |

**No new file formats** (FBX, USD, SketchUp export and others in `export-formats.md`) until this is
built: they would arrive as the first users of the list.

## 1. Why

Today the commands crate depends on `curva-3dm`, `curva-dxf`, `curva-skp` and `curva-meshfiles`, and
the interface on `curva-pdf`. The lists of types each dialog offers (`READ_FORMATS`,
`IMPORT_FORMATS`, `SAVE_FORMATS`, `EXPORT_FORMATS`) sit in the commands crate, and `Open`, `Import`,
`Export` and `Print` each choose a format by matching the file's extension, with one branch per
format. Each new format so far added a dependency and a branch to four commands. Six formats have
arrived since the first review asked for this.

## 2. The format description

A new module in `curva-document`, `formats`, which every format crate can reach (the document is
below them all):

- **A format** says:
  - its name for dialogs, as a translation key (`files.rhino`);
  - its file extensions (`3dm`);
  - what it can do: open, import, save, export, export the selection, print;
  - and the work itself. A file read into a new model, read into the open model, or written from
    the model (or only the chosen objects) gives back the same kind of report everywhere: what came
    in, what was left out and why, and notes. That is the report `.3dm`, DXF and SketchUp already
    give.
- **Options** a format asks when writing (a DWG's version, a STEP file's schema, a mesh format's
  density) are declared by the format as names and choices. The `Export` command asks them the way
  it asks any command option, so they stay remembered and typed in Rhino's style. Today's options
  are carried over as they are.
- **Printing** formats (PDF, and SVG with one file per layout) take the pages the viewport draws,
  as `Print` does today.

## 3. The list

- **`Formats`**, in `curva-document`: an ordered list of formats. It answers "which format is this
  file?" (by extension, ignoring case) and "which formats can be opened, imported, saved, exported,
  printed?", in the order they joined.
- **Who fills it:** a small new crate, `curva-formats`, depends on every format crate and has one
  function that returns the full list: Curva's own `.curva` first, then Rhino, DXF, DWG, SketchUp,
  STEP, STL, OBJ, PLY, PDF and SVG, in today's dialog order. Each format crate offers one function
  that adds its formats. A build can leave a format out with a cargo feature in `curva-formats`.
- **Who uses it:** the interface (`curva-app`) and the agent server (`curva-mcp`) get the list from
  `curva-formats` when they start, and hand it to the document they work on. The document keeps it
  beside the model, never saves it, and passes it on to a document `Open` or `New` makes. The file
  commands ask `doc.formats()`. No command, and nothing in `curva-commands` or `curva-app`, names a
  format crate.
- **Where each format moves:**
  - `.curva`: built into `curva-document` itself (it is the document's own file).
  - Rhino, DXF and DWG, SketchUp, STL, OBJ and PLY, PDF and SVG: each registered by its own crate.
    The code that turns their reports into Curva's and writes their files moves out of
    `curva-commands/src/commands/files/` into those crates, unchanged.
  - STEP: its writer is in the commands crate today (`files/step.rs`, using the engine's STEP
    writer through `curva-kernel`). It moves, unchanged, into a new crate `curva-step`, which
    registers it.

## 4. Steps

Each step is its own pull request, and every test passes unchanged at each one.

1. **The lists, pinned:** a test that writes down today's four lists and the print formats, in
   order, so every later step proves nothing moved. (Added before anything else changes.)
2. **The format description and the list** in `curva-document`, with `.curva` registered. The file
   commands ask the list for `.curva` and still branch for the rest.
3. **Rhino** (`curva-3dm`) registers itself; its branches leave the commands.
4. **DXF and DWG** (`curva-dxf`), then **SketchUp** (`curva-skp`), then **STL, OBJ and PLY**
   (`curva-meshfiles`, with the mesh stream told first, as it owns that crate's insides), then
   **STEP** (new `curva-step`), then **PDF and SVG** (`curva-pdf`).
5. **The dependencies go:** `curva-commands` and `curva-app` drop the format crates. A test fails
   if either depends on one again. The old list constants are removed; the pinned test from step 1
   now checks the registry's lists.

## 5. Tests

- The pinned lists (step 1) stay green throughout: same types, same order, in every dialog.
- Every existing file test (Rhino round trips, DXF, DWG, SketchUp, STL, OBJ, PLY, STEP, PDF, SVG)
  passes unchanged. Tests that call a format crate directly move into that crate with their code.
- New: a format is found by extension in any case; a format that cannot export is not offered by
  `Export`; a document from `Open` keeps the list.
- A dependency check: `curva-commands` and `curva-app` name no format crate.

## 6. Not in this spec

New formats, new options, or any change to what a format reads or writes. Moving the `.curva`
reading into its own crate. A list of formats that can be added at run time (plug-ins): the list is
fixed when Curva is built.

## 7. Choices for the owner

- **Where the list lives while Curva runs.** Recommended: with the document, as above, so every
  command reaches it without changing how the ~250 commands are called. The alternative, handing it
  to each command as it starts, means touching every command for no difference you would see.
- **STEP in its own crate** (recommended), or left in the commands crate as the one exception. Its
  own crate keeps the rule "the commands name no format" without exceptions.
