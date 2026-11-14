# 0021: Feedback on 0.4.2

**Status:** Decided by the owner in chat · 14 November 2026. Replaces points 7 (DXF library) and 8
(no DWG) of decision 0020.

The owner's points are numbered in `docs/specs/feedback-0.4.2.md`. The owner chose:

1. **Top bar:** one row at half the height, Curva and its menus on the left, the specialization dial
   in the centre, the file name over its path on the right.
2. **Modify:** the Commands panel becomes Operations, and its Operations tab becomes Modify.
   (Rejected: Edit, Transform.)
3. **Feet and inches show as feet-and-inches,** with fractions, and a setting for decimals.
   (Rejected: decimals only.)
4. **Layers panel columns** are sized one by one by dragging the dividers in a header row. (The owner
   first wrote rows, then corrected it to columns.)
5. **DWG and DXF in 0.4.3, through `acadrust`.** The owner approved MPL-2.0 as a licence exception
   for `acadrust` alone in `deny.toml`: Curva uses it unmodified, so Curva stays MIT OR Apache-2.0,
   and any change to `acadrust`'s own files would be shared under MPL-2.0. (Rejected: DXF only through
   the MIT `dxf` crate.)
6. **Clean-up before Milestone 3:** the six fixes in `docs/reviews/code-health-2026-11-14.md` are made
   before any Milestone 3 feature, each in its own pull request with before-and-after numbers; the
   format interface (fix 6) arrives with the first new format. (Rejected: only the speed fixes now,
   or only a written plan.)
7. **Look for existing work first:** before building any function, above all an import or export,
   search for an existing library or implementation, and record what was checked in the spec's
   "Prior work checked" list.

Decided by Claude, as the owner asked to keep going:

8. **Native dialogs through `rfd`** (MIT); dash-prefixed commands (`-Open`, `-Export`…) skip the
   dialog, as in Rhino, for scripts and agents.
9. **Curva adds itself to the Linux applications menu** when run from an AppImage, if the user
   agrees on the welcome screen.
10. **Hidden lines stay Curva's own:** `ogeom-hlr` (the only Rust hidden-line crate found) is
    LGPL-2.1-only and brings its own geometry kernel, so it cannot be linked into an MIT OR
    Apache-2.0 program without relinking duties; its design is studied instead.
11. **PDF through `krilla`** (MIT OR Apache-2.0, from the Typst project, built on `pdf-writer` and
    `subsetter`), which handles fonts and images as well; SVG written by Curva. This replaces the
    library choice in decision 0020, point 7; printing through the system's print command stays.
12. **Order of work in 0.4.3:** the clean-up fixes 1 to 5; then this feedback (§1 to §7 of the spec),
    built on the reorganised app; then Milestone 3 in the order of decision 0020, point 9.

**Would make us revisit:** `acadrust` going unmaintained (then `cadkit-dwg`, MIT OR Apache-2.0,
reads DWG, and `dxf` writes DXF); a permissively licensed hidden-line crate appearing.
