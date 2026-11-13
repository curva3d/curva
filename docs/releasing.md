# Releasing Curva

Before a release is tagged, the automatic checks must pass on every system, and these are tried by hand
in the running app (the owner asked for this after 0.3.0 shipped with curve snapping broken):

1. **Snapping:** draw a line, a polyline and an arc; start Line and snap to their ends, middles and
   centres in Wireframe, Shaded and Ghosted. Snap to a box's corners too.
2. **Options:** start Circle and choose `2Point` by clicking it and by typing it; the same for one
   option of a command with several steps (Rectangle, Offset).
3. **Typing after clicking in a viewport:** click in a viewport, then type a command; it reaches the
   command line.
4. **Previews:** start Box, orbit the view while the height previews; it holds. FilletEdge shows the
   rounded solid.
5. **Escape:** Copy one copy, press Escape; the copy stays. Escape with nothing running clears the
   selection.
6. **Panels:** move, merge, split, hide, close and restore one panel; resize one from an edge.
7. **Gumball:** move along an arrow with a snap; extrude a curve with the circle.
8. **Clipboard:** Ctrl+C, Ctrl+V on a selected object.

And, before every release (the owner's rule, decision 0019): **check the custom kernel operations
against the latest solidcore.** Move solidcore to its latest revision on a branch, run the tests, and
for each row of `docs/kernel/custom-operations.md` see whether solidcore now covers the operation
fully. Switch those to solidcore's, delete Curva's code and the row, note it in the release notes, and
update "Checked at" on the rest.

Then: bump the version in `Cargo.toml`, write `docs/releases/vX.Y.Z.md`, merge, tag `vX.Y.Z` and push the
tag; the release workflow builds the Windows installer, the Linux AppImage and the macOS disk image.
