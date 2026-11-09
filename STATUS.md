# Status

*Updated at the end of every work session. Newest first.*

## Current state — 8 November 2026

**Milestone:** M0, groundwork and kernel evaluation.

**Done**
- Architecture draft v0.9 written by the owner (`docs/architecture.md`).
- Agents work through the owner's GitHub account and merge their own work once checks pass (decisions 0005 and 0007).
- Starter repository prepared: rules, decision notes, M0 kernel evaluation spec, issue list.
- One-time GitHub setup finished: https://github.com/curva3d/curva, with `main` protected and the
  17 M0 tasks in https://github.com/curva3d/curva/milestone/1.
- The kernel test list grew from 25 to 31 operations, chosen by the owner after comparing it with
  Serpentine3D's commands (#19).
- The owner confirmed the 31-operation list is final and approved; #8 is released.
- Dev Drive and Rust toolchain set up (#2). The repository is at `V:\curva`.
- Rust workspace skeleton (#3).
- Automated checks on Linux, Windows and macOS, required on `main` (#4).
- Claude Code hooks: format and lint after edits, tests before finishing, risky commands blocked (#5).
- The 31 kernel test cases as neutral files, and a harness to run them (#8), in `evaluation/`.
- A check that flags weakened tests on every pull request (#6). It is deliberately not a required
  check, so a change the owner has agreed to can still be merged.

**In progress**
- Writing the Milestone 1 spec for the owner to approve.

**Waiting for the owner**
- Nothing yet. The engine is solidcore, with Serpentine3D's commands as the base (decision 0010). SolidCraft
  reuse is parked (0009).

**Testing**
- Agents do not run tests through Rhino (decision 0011). Curva is verified by its own checks.
- solidcore ran 28 of the 31 cases; results and known gaps are in 0011 and `evaluation/reports/solidcore.tsv`.

**Handoff notes**
- `V:` is a file-based Dev Drive (`C:\DevDrives\curva.vhdx`), reconnected at startup by the
  scheduled task "Attach Curva Dev Drive". Reconnecting after a restart has not been tested yet.
- The old copy in `C:\Projects\curva` is out of date and no longer used.
- Agents merge their own pull requests once checks pass; the owner does not click merge.

**Known problems / risks**
- Serpentine3D's geometry is done by OpenCascade (C++), so its geometric logic cannot be ported
  directly; only commands, prompts, options and interaction flow port. See decision 0003.
- The kernel choice is untested. M0 exists to test it before M1 starts. See decision 0004.
