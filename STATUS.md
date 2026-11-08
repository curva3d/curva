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
- The kernel test list grew from 25 to 31 operations, chosen by the owner after comparing it with
  Serpentine3D's commands (#19). The owner merged that change himself, but without the
  `owner-approved` label, because the label had not been created yet.
- The owner confirmed the 31-operation list is final and approved; #8 is released.
- Dev Drive and Rust toolchain set up (#2). The repository is at `V:\curva`.
- Rust workspace skeleton (#3).

**In progress**
- Nothing.

**Waiting for the owner**
- The Rhino licence question (#7).

**Handoff notes**
- `V:` is a file-based Dev Drive (`C:\DevDrives\curva.vhdx`), reconnected at startup by the
  scheduled task "Attach Curva Dev Drive". Reconnecting after a restart has not been tested yet.
- The old copy in `C:\Projects\curva` is out of date and no longer used.
- The owner merges pull requests himself, about once per finished task; group small changes.

**Known problems / risks**
- Serpentine3D's geometry is done by OpenCascade (C++), so its geometric logic cannot be ported
  directly; only commands, prompts, options and interaction flow port. See decision 0003.
- The kernel choice is untested. M0 exists to test it before M1 starts. See decision 0004.
