# 0013: Mixed curves are compound curves

**Status:** Accepted by the owner in chat · 9 November 2026 ("lets use mixed", after asking how Serpentine3D does it)

**Decision:** A curve made of different kinds of part is a `Curve::Compound`: an ordered list of parts meeting end to end, each keeping its own kind. It is the same idea as a Rhino polycurve.

**How Serpentine3D does it:** its `Join` builds an OpenCASCADE wire, an ordered list of edges that each keep their own geometry, and `Explode` turns them back into separate curves. Curva has no OpenCASCADE, so this is the same structure written in Rust.

**Joining:** straight curves still give a polyline and smooth curves a single NURBS curve. Only a real mix gives a compound curve.

**Consequence:** `.curva` files gain a new kind of curve. Files saved earlier open unchanged. The test `a_line_and_a_smooth_curve_do_not_join_yet` recorded the old limit and is replaced by a test that they now join.
