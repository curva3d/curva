# 0035: Dimensions are tied to the objects they snapped to

**Status:** Decided by the owner in chat · 17 November 2026.

**What was decided.** A dimension remembers, for each of its points, the object it snapped to and
the place on it (an end, a corner, a middle, a centre; for a solid, which edge). When the object
moves or changes, the dimension follows, as in Rhino (spec `annotation.md` §3). The snaps, which
the display stream keeps, report the object and place; the features stream keeps the ties in the
document and moves the dimensions (#294).

**Why.** It is the reliable way: the dimension follows the very object the user snapped to, even
where two objects share a point, such as two walls meeting at a corner.

**Rejected.** Finding the object afterwards by matching the dimension's points to objects' points
(option B): it needs no change to snapping, but where two objects share a point it can pick the
wrong one, and the dimension would then follow the other object. Leaving dimensions fixed for now
(option C).

**Revisit if** snapping can no longer say which object it landed on, for example if a new display
design picks points on the graphics card without knowing the objects.
