# 0037: a patched copy of solidcore-ops until solidcore has a kernel per thread

**Status:** Decided by the owner in chat · 19 November 2026.

**What was decided.** solidcore runs every engine call behind one lock for the whole program, so
work split over the processor's cores still runs one call at a time. Opening big models, cutting
plans and finding hidden lines all wait on it. Curva keeps a copy of solidcore's `solidcore-ops`
crate (at commit `9342f32`, the one Curva uses) in `crates/curva-kernel/vendor/solidcore-ops`
with one change: `session::with_own_kernel`, inside which a thread's calls run on a kernel of its
own, set up as the program's. The change is kept as `curva.patch` beside the copy, and the two
changed files say so in their first line. Nothing else in the copy is changed; the rest of
solidcore is used as released.

Curva calls it through `curva_kernel::parallel::each_in_parallel`, which today builds the solids of
an opened `.3dm`, cuts each solid for a plan (Make2D) and finds each solid's lines for hidden-line
drawings, on every core at once.

The change was offered to solidcore's authors in [tangency-labs/solidcore#3358](https://github.com/tangency-labs/solidcore/issues/3358), as the owner
asked. **When solidcore has the fix, Curva goes back to the released crate and deletes the copy**,
once Curva's tests pass on it and the timings hold (steps in issue #381), so Curva carries as
little custom code as possible.

**Why.** The owner needs drawings to open quickly, since architects will open them all the time.
Measured on this machine: 02 Int Design opens in 12 s instead of 55 s, the test house plan in 89 ms
instead of 184 ms, the large plan in 21 s instead of 30 s. Waiting for an upstream release would
leave every Curva release slow until then. The copy is in Curva's own code so every release has it,
and anyone can build Curva from its public code.

**Rejected.**
- A public fork of solidcore that Curva downloads: the same code, plus a second repository to keep.
- A private copy: Curva's code is public, so nobody else could build it, and CI would need a key.
- A patch on the owner's machine only: releases would stay slow.

**Revisit if** solidcore takes the fix (then delete the copy), solidcore changes `solidcore-ops`
in a way Curva needs before it takes the fix (then the copy moves to that commit, with the same
patch), or solidcore's authors fix the lock another way (then Curva uses theirs).
