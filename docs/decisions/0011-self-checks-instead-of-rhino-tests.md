# 0011: Curva checks itself; no automated Rhino tests

**Status:** Accepted by the owner in chat · 9 November 2026 · Amends 0004 and the Milestone 0 spec

**Decision:** Agents stop running tests through Rhino. Milestone 0 ends here and development of Curva
starts. Curva's results are verified by its own checks:
- exact answers worked out by hand, where a shape has one
- consistency checks, for example the parts of a cut adding back up to the whole
- validity checks on every result
- pictures of the results

Rhino stays the target for how commands should behave. The owner judges that by hand.

**Why:** automated work through Rhino was unreliable. Importing a STEP file stalled twice, and a script met
a document with 320 unexplained objects. The owner chose to spend the effort on Curva itself.

**What we know about solidcore (`evaluation/reports/solidcore.tsv`):** 28 of the 31 cases were run, and 24
showed no problem. The owner expects solidcore to keep improving, so these stay as known gaps:
- The open loft raises a warning from solidcore's own check. It looks fine in a picture.
- An offset surface could not be meshed for display by the route used.
- A split by a curve that stops short of the surface's edges cut nothing.
- Joined and capped walls closed up but had no volume. This may be how the test assembled them.
- Sweep2 and network surface have no ready-made operation. The freeform fillet case was not run.

**The risk we accept:** nothing has been compared with an independent modeller. A self-consistent result
can still differ from what Rhino would make, and the owner's hands-on judgement is what catches that.
