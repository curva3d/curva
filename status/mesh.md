# Status: mesh stream

*Written only by the mesh stream, which owns the inside of `crates/curva-mesh` and its own new
mesh and SubD files elsewhere (see `AGENTS.md`). Updated at the end of every work session. Newest
first.*

## 16 November 2026 (second session, after the move to E:)

**Spec approved by the owner** (17 November 2026, `docs/specs/mesh-and-subd.md`, #246). The Boost
licence is allowed for all of Curva (decision 0034, in `deny.toml` and `about.toml`).
First draft, as sent for approval:
Three stages: A, Rhino's meshes and SubDs come in (objects, display, `.3dm`, OBJ, STL, PLY);
B, SubD modelling; C, mesh tools. Owner's decisions asked: the Boost licence (BSL-1.0) for
clipper2-rust, which manifold-rust needs (`cargo-deny` rejects it today; everything else passes);
the order of the stages; the plan as a whole.

**Owner's changes (16 November 2026):** QuadRemesh is important and comes in Stage B, on quadrs
(decision 0033, 17 November; no well-maintained pure-Rust alternative was found). Stage order
(owner, 17 November): A meshes and mesh tools, B SubDs come in, C SubD modelling with QuadRemesh.


**Next:** Stage A (spec §10), issues filed 17 November: step 1, the mesh structure in
`crates/curva-mesh` (#268), then #269 to #275. Asked of the other streams: the two shape kinds
(#266, features) and drawing meshes (#267, display). Flat or smooth SubD display is a command like any other,
`SubDDisplayToggle`, with Tab as its shortcut.

**Found on the way**
- subdiv-kernels' weights are single precision; step 2 measures whether that is enough for
  ToNURBS (spec §3.2).
- subdiv-kernels' optional Bevy feature targets Bevy 0.19 (Curva is on 0.20.1): never enable it.
- Adding the two shape kinds touches about 30 exhaustive checks in features-owned code, so it is a
  features issue, not a one-line registration (spec §6).

## 16 November 2026

**Started.** The owner started the mesh stream (decision 0032): mesh and SubD objects, built on
manifold-rust (mesh booleans and repair) and subdiv-kernels (SubD), with Curva's own small mesh and
SubD cage structure. Nothing built yet.

**Next:** the spec, `docs/specs/mesh-and-subd.md`, for the owner's approval (issue labelled `mesh`).
No code before it is approved.
