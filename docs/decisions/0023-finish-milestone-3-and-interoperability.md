# 0023: Finish Milestone 3 before 0.4.3; files must interoperate

**Status:** Decided by the owner in chat · 15 November 2026.

**The question:** with most of Milestone 3 built, should 0.4.3 be released now with the rest moved
on, or wait until every part of `milestone-3-in-0.4.3.md` is built and checked?

**The owner chose:**

1. **Finish Milestone 3 first, then release 0.4.3** (option A). The parts still to build when this
   was decided:
   - the Technical and Hidden views (point 12), which are the display stream's to build (#164),
     probably as visualisation styles (#160);
   - ordinate, arc length and curve length dimensions, and dimensions that follow the geometry they
     measure (point 7);
   - hatch patterns from `.pat` files, gradients and a base point (point 9);
   - printing the model or a view, PNG output and shaded details on paper (point 14);
   - Make2D's four-view drawings, clipping-plane cuts, annotations and source layers (point 11).
   (Rejected: releasing now with the gaps listed; a middle way leaving out the display work and
   associative dimensions.)
2. **Interoperability is required for `.3dm`, DXF and DWG, both ways.** A drawing made in Curva must
   open in Rhino or AutoCAD as the same drawing, and one made there must open in Curva as the same
   drawing, not just "something comes across". Every gap is listed in `drawing-output.md` §7 and is
   part of finishing Milestone 3.

**What would make us revisit:** a gap that cannot close because Rhino or AutoCAD keep something in a
form no library or published format documents; that comes back to the owner as a choice.
