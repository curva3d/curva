# 0032: Mesh and SubD: a slim set of libraries, and a third stream starting now

**Status:** Decided by the owner in chat · 16 November 2026. Quad remeshing row replaced by 0033
(QuadRemesh now, on quadrs). Updates decision 0008 (mesh boolean
library) and architecture §6 and §10d.

**What was decided.**

The fewest libraries that do the job, one per job:

| Job | Use | Licence | Instead of |
|---|---|---|---|
| Mesh booleans and repair | **manifold-rust**: a complete pure-Rust port of Manifold (Emmett Lalish), with a second, robust engine for messy imported meshes, and repair tools | Apache-2.0 | csgrs (older BSP method), boolmesh (MPL, one author) |
| SubD smoothing and the exact limit surface | **subdiv-kernels**: Catmull–Clark with creases, limit evaluation and face-varying channels, tested against OpenSubdiv | Apache-2.0 / MIT / Zlib / BSD-3 | simpler subdivision in other mesh libraries |
| Editable mesh and SubD cage | **Curva's own small structure** in `crates/curva-mesh`. Mostly a data structure; it has to carry Curva's object memory anyway | Curva's | halfedge, unless a test shows it saves real work |
| SubD to NURBS | subdiv-kernels' regular patches handed to solidcore as exact surfaces (through `curva-kernel`); only the patches around irregular points approximated, as Rhino does. Listed as a custom operation | — | — |
| Quad remeshing (as Rhino's QuadRemesh) | **Nothing for now.** Add only when needed; quadrs (MIT, experimental, based on Instant Meshes) is the candidate then | — | — |
| Drawing | Bevy, from triangles (decisions 0028 to 0030) | — | — |

**Rules for every outside library, so updates stay cheap:**
1. **One library per job,** never two doing the same thing.
2. **Each sits behind one Curva interface, in one place** (for mesh and SubD: `crates/curva-mesh`).
   An upgrade, or a swap if a library dies, touches only that place.
3. **Pinned versions, upgraded on a schedule:** once per Curva release, not whenever a library
   publishes, as with Bevy (decision 0030).
4. **Tests catch any change in results** before an upgrade merges, and anything Curva adds on top
   is listed (as `docs/kernel/custom-operations.md` and `docs/bevy/custom-parts.md` do).

**Starting now, as a third stream.** Mesh and SubD were planned for after M4. The owner chose to
start them now, in their own sessions: the **mesh** stream (`AGENTS.md`, Streams). It owns
`crates/curva-mesh` and its own new mesh and SubD files in shared crates, picks up issues labelled
`mesh`, and writes only `status/mesh.md`. Its first piece of work is a spec for the owner's
approval, `docs/specs/mesh-and-subd.md`; no code before that.

**Why.** The owner asked for fewer engines to keep updated. Libraries that overlapped were dropped
(halfedge, csgrs, boolmesh, mesh-repair, quadrs for now), leaving two engines (solidcore, Bevy)
plus one library each for mesh operations, SubD and, at M4, IFC (decision 0031). Robust mesh
booleans and exact SubD surfaces take years to get right; updating a library is a day of agent
work.

**Rejected.** Keeping csgrs (its BSP method tends to crack on real models). Adopting halfedge as
the mesh structure now (broad, but one maintainer and 2-manifold only; Curva's structure is small).
Adding a quad remesher before anyone needs it.

**Licence check.** manifold-rust brings dashu and clipper2-rust; run `cargo-deny` when it is first
added and ask the owner about anything not on the allowed list.

**Revisit if** a library stops being maintained, or a test shows halfedge (or another) would save
real work over Curva's own structure.
