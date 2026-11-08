# Status

*Updated at the end of every work session. Newest first.*

## Current state — 8 November 2026

**Milestone:** M0, groundwork and kernel evaluation.

**Done**
- Architecture draft v0.9 written by the owner (`docs/architecture.md`).
- Agents work through the owner's GitHub account; he approves by label (decision 0005).
- Starter repository prepared: rules, decision notes, M0 kernel evaluation spec, issue list.
- One-time GitHub setup finished: https://github.com/curva3d/curva, with `main` protected and the
  17 M0 tasks in https://github.com/curva3d/curva/milestone/1.

**In progress**
- Nothing. Agents wait for the owner's go-ahead before starting on the tasks.

**Waiting for the owner**
- Review the list of test operations in `docs/specs/m0-kernel-evaluation.md` (section 3).
- Dev Drive and Rust setup (#2) and the Rhino licence question (#7).

**Handoff notes**
- The working copy is in `C:\Projects\curva` until it moves to the Dev Drive (#2).

**Known problems / risks**
- Serpentine3D's geometry is done by OpenCascade (C++), so its geometric logic cannot be ported
  directly; only commands, prompts, options and interaction flow port. See decision 0003.
- The kernel choice is untested. M0 exists to test it before M1 starts. See decision 0004.
