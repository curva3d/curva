# Status

*Updated at the end of every work session. Newest first.*

## Current state — 8 November 2026

**Milestone:** M0, groundwork and kernel evaluation.

**Done**
- Architecture draft v0.9 written by the owner (`docs/architecture.md`).
- Agents work through the owner's GitHub account; he approves by label (decision 0005).
- Starter repository prepared: rules, decision notes, M0 kernel evaluation spec, issue list.

**In progress**
- One-time GitHub setup (`setup/KICKOFF.md`).

**Waiting for the owner**
- Review the list of test operations in `docs/specs/m0-kernel-evaluation.md` (section 3).

**Known problems / risks**
- Serpentine3D's geometry is done by OpenCascade (C++), so its geometric logic cannot be ported
  directly; only commands, prompts, options and interaction flow port. See decision 0003.
- The kernel choice is untested. M0 exists to test it before M1 starts. See decision 0004.
