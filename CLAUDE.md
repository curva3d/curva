# CLAUDE.md

Read `AGENTS.md` first. It holds all the rules. Then read `STATUS.md` and the three status pages it points to (`status/display.md`, `status/mesh.md`, `status/features.md`) to see where things stand.
This file only adds notes specific to Claude Code.

- Use plan mode for anything non-trivial: write the plan (files touched, approach, tests) before editing.
- Start every task in a fresh session that reads `AGENTS.md`, all three status pages and the issue.
- Work in a Git worktree per issue (`git worktree add ../curva-<issue> -b <issue>-<short-name>`), and remove it once its pull request is merged or closed.
- Update your stream's status page (`status/display.md`, `status/mesh.md` or `status/features.md`) before ending a session.
