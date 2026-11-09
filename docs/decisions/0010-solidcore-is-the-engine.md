# 0010: solidcore is the engine; Curva Freeform follows Rhino and Serpentine3D

**Status:** Accepted by the owner in chat · 9 November 2026 · Amends 0004 and the Milestone 0 plan

**Decision:**
- **Engine:** Curva uses solidcore. The comparison of several engines planned in 0004 is dropped.
- **Product:** Curva is design and architecture software first, not a SolidWorks or SolidCraft clone.
  Curva Freeform should work like Rhino and Serpentine3D.
- **Commands:** Serpentine3D's command list is the base, rewritten in Rust, as set out in 0003.
- **Other code:** usable code for commands may be reused later. That is a separate step
  (see 0009, which is parked and undecided).

**Why:** solidcore has the range of tools Curva needs and by far the most activity: 68 contributors and
2,800 commits at 8 November 2026, against one contributor for arris and no commits in 30 days for
monstertruck.

**The risk we accept:** solidcore is under a month old and has not been tested on the 31 modelling
operations. Activity is not proof of quality. Curva keeps its kernel interface, so another engine can
still be added for operations solidcore handles badly.

**What changes in Milestone 0:** the 31 test cases are still run, on solidcore only, to show what works
and what is missing. They no longer choose between engines.
