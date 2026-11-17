# Status: mesh stream

*Written only by the mesh stream, which owns the inside of `crates/curva-mesh` and its own new
mesh and SubD files elsewhere (see `AGENTS.md`). Updated at the end of every work session. Newest
first.*

## 16 and 17 November 2026 (second session, after the move to E:)

**Spec approved by the owner** (17 November 2026, `docs/specs/mesh-and-subd.md`, #246), with
these decisions:
- Stage order: A meshes and mesh tools (with STL, OBJ, PLY and `.3dm` meshes), B SubDs come in,
  C SubD modelling with QuadRemesh.
- QuadRemesh now, on quadrs (decision 0033). No well-maintained pure-Rust quad remesher exists;
  the maintained ones are C++.
- Flat or smooth SubD display is a command like any other, `SubDDisplayToggle`, with Tab as its
  shortcut.
- The Boost licence (BSL-1.0) is allowed for all of Curva (decision 0034, in `deny.toml` and
  `about.toml`); manifold-rust needs it through clipper2-rust.

**Merged:** the spec (PR #265) and Stage A step 1, the mesh structure in `crates/curva-mesh`
(#268, PR #276).

**In review:** STL and OBJ read and written, new crate `crates/curva-meshfiles` (#269 part 1,
PR #293). Checked both ways against Rhino 8. Part 2, hooking them into Open, Import and Export,
waits for #266.

**Next:** mesh booleans on manifold-rust (#272), whose library part does not need #266, then the
rest of Stage A, #270 to #275 (spec §10). Asked of the other streams: the two shape kinds in the
document (#266, features; needed before any command or file hook-up) and drawing meshes (#267,
display).

**Found on the way**
- subdiv-kernels' weights are single precision; step 2 measures whether that is enough for
  ToNURBS (spec §3.2).
- subdiv-kernels' optional Bevy feature targets Bevy 0.19 (Curva is on 0.20.1): never enable it.
- Adding the two shape kinds touches about 30 exhaustive checks in features-owned code, so it is a
  features issue, not a one-line registration (spec §6).
- The release build's licence list (`about.toml`) lacked MIT-0, which `deny.toml` has allowed
  since Bevy came in, so release builds on main failed. Fixed in PR #265.
- Rhino's OBJ export writes no object names, so several parts come back as one mesh, and its STL
  puts all parts in one solid. Rhino's own STL import splits parts that do not touch; Curva could
  offer the same once there is an import options dialog.

## 16 November 2026

**Started.** The owner started the mesh stream (decision 0032): mesh and SubD objects, built on
manifold-rust (mesh booleans and repair) and subdiv-kernels (SubD), with Curva's own small mesh and
SubD cage structure. Nothing built yet.

**Next:** the spec, `docs/specs/mesh-and-subd.md`, for the owner's approval (issue labelled `mesh`).
No code before it is approved.
