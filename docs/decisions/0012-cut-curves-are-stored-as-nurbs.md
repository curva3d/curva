# 0012: Cut curves are stored as NURBS

**Status:** Accepted by the owner in chat · 9 November 2026

**Decision:** Curva stores a smooth curve that has been cut as real NURBS data: degree, knots, control points and weights. `Curve::Nurbs` is the curve kind for this, and `Curve::to_nurbs` turns a smooth curve into it exactly.

**Why:** a smooth curve was saved as the points it passes through, and a piece of it cannot be described that way. NURBS holds any piece exactly and is what Rhino uses. Surfaces in Milestone 2 need the same data.

**Considered and not chosen:**
- Keeping the whole curve and remembering which part remains. Every later operation would have to handle a part of a curve, and repeated cuts become awkward.
- Fitting a new curve through the piece. The shape would change slightly with every cut.

**Consequence:** `.curva` files gain a new kind of curve. Files saved earlier still open unchanged.
