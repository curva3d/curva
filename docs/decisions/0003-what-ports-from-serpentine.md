# 0003 — What we take from Serpentine3D, and what we don't

**Status:** Accepted · 8 November 2026 · Amends architecture section 9

**What we found:** Serpentine3D does its geometry through OCP, the Python link to OpenCascade
(a large C++ geometry engine). Its commands mostly *ask* OpenCascade to do the hard work: lofts,
sweeps, fillets, booleans, offsets.

**Decision:** From Serpentine3D we port:
- the command list, names and aliases
- each command's prompts, options and interaction flow
- its tests, as a source of command test cases
- its ideas for interface behaviour (gumball, picking, layers), already shaped by Rhino users

We do **not** expect to port its geometry. That comes from a Rust kernel (see 0004).

**What this means for the owner:** Freeform's commands will *feel* like Serpentine and Rhino from
early on, but how well lofts, sweeps and fillets *work* depends entirely on the Rust kernel.

**Rejected:** Using OpenCascade from Rust. It would break the pure-Rust principle and bring in a
very large C++ dependency.
