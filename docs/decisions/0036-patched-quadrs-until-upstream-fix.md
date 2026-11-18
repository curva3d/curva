# 0036: a patched copy of quadrs until its author releases a fix

**Status:** Decided by the owner in chat · 18 November 2026. Adds to decision 0033 (QuadRemesh on
quadrs), which stands.

**What was decided.** quadrs 1.0.0 does not give the same quads twice for the same mesh and seed,
which Curva needs (the same file must remesh the same way on every machine and every day). Curva
keeps a copy of quadrs 1.0.0 in `crates/curva-mesh/vendor/quadrs` with a small fix: four places
that used hash maps, whose order changes on every run, use ordered maps instead. Nothing else in the
copy is changed. The fix was offered to quadrs' author
([vidstige/quadrs#1](https://github.com/vidstige/quadrs/issues/1)). **When a quadrs release has the
fix, Curva goes back to the released crate and deletes its copy**, so Curva carries as little custom
code as possible. The steps are in issue #349, and the copy is checked at every mesh library upgrade
with the custom operations (`docs/mesh/custom-operations.md`).

**Why.** Quickest way to repeatable quads while keeping quadrs, and the fix is small enough for its
author to take as it is.

**Rejected.**
- Using quadrs as it is and changing the spec to allow different quads on each run: files would not
  remesh the same way twice.
- Curva's own QuadRemesh now: two to three more weeks; it stays the fallback named in 0033.

**Revisit if** the author does not answer and quadrs stays unchanged for a long time (then the copy
simply stays), or quadrs falls short of Rhino on the test models (decision 0033's own fallback).
