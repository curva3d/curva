# CLAUDE.md

Read `AGENTS.md` first. It holds all the rules. Then read `STATUS.md` to see where things stand.
This file only adds notes specific to Claude Code.

- Use plan mode for anything non-trivial: write the plan (files touched, approach, tests) before editing.
- Start every task in a fresh session that reads `AGENTS.md`, `STATUS.md` and the issue.
- Work in a Git worktree per issue (`git worktree add ../curva-<issue> -b <issue>-<short-name>`).
- Update `STATUS.md` before ending a session.
