# Rules for agents

These rules apply to every AI agent and every human contributor. They come from
`docs/architecture.md` (sections 15 to 19). If this file and the architecture disagree,
the architecture wins; open a `needs-human` issue about the conflict.

## The project owner

The owner is a practising architect and an expert Rhino user. He is **not** a programmer and
does not read code.

- He decides how Curva should **feel and behave**. You decide the technical details, within
  `docs/architecture.md`, and record significant ones in `docs/decisions/` in plain language.
- **Plain language, always.** Explain any unavoidable technical term in one short sentence.
  Explain trade-offs in terms a user would notice: speed, reliability, what a tool can do.
- **Decisions come as choices:** a short summary, two or three options with consequences, and a
  clear recommendation. He should never have to research anything to answer.
- **Never ask him to read code** or judge a technical detail. Turn it into a question about
  behaviour or outcome.
- **If he must do something himself**, give exact numbered steps with every command ready to
  copy and paste, and say what he should see when it worked.
- **Show, don't describe:** screenshots, recordings, Curva-versus-Rhino comparisons.
- **Be honest.** Say clearly when something failed or is fragile. Never overstate progress.
- **Reports are short:** what changed, what to look at, what needs his decision, what went wrong.

## Where work comes from

- **GitHub Issues are the backlog.** Only pick up issues labelled `agent-ready`.
- **Claim before starting:** open a draft pull request linked to the issue, then move the label to
  `in-progress`. Skip issues that are already claimed.
- **One agent, one issue, one branch, one worktree.**
- **Larger features need an approved spec** in `docs/specs/` before code is written.
- **Small pull requests:** one concern each, ideally a few hundred lines.

## Code rules (architecture section 17)

- **Pure Rust.** No C++ or Python in anything Curva builds or links. The only exception is the
  Blender / Cycles rendering bridge, which runs as a separate program.
- Small crates with one job each. Higher layers use lower ones, never the reverse:
  kernel interface → geometry → document → commands → interface → specializations.
- One command per file, from the same template.
- Every public item documented (`#![deny(missing_docs)]`). Comments explain *why*.
- Every geometric algorithm cites its source (e.g. *The NURBS Book*, section and page).
- No `unsafe`, no hidden global state, no generated or copied blobs in the source tree.
- `rustfmt` and `clippy -D warnings` must pass.
- New dependencies need a reason in the pull request and must pass `cargo-deny`
  (licence and maintenance). Dependencies with licence implications need the owner's approval.
- No interface text hard-coded in source: all wording goes in translation files.
- No secrets in the repository. Tests never use the network.

## Tests (architecture section 15)

- **Nothing merges unless CI passes** on Linux, Windows and macOS.
- Every new command or fix comes with its tests. A bug fix starts with a test that reproduces it.
- **Never weaken a test to make it pass.** Do not delete tests, loosen tolerances, mark tests as
  ignored or special-case test inputs.
- **Agents merge their own pull requests once all checks pass.** The owner does not review pull
  requests or click merge, and there is no approval label (decision 0007).
- **If an existing test, reference file or Rhino golden has to change** without weakening it, say
  why in the pull request and mention it in your report. If the change would weaken it, stop and
  ask the owner in chat before merging.
- **Specs are approved by the owner in chat.** Note his approval in the pull request.
- Keep the `Co-authored-by: Claude` line on commits, so the history shows who wrote what.

## Command rules (owner's rule, 9 November 2026)

- **Every command that changes geometry shows a live preview before it commits.** That includes
  drawing commands and transforms, and following the cursor and typed values. The only exception
  is a command with no result to show, such as Delete, whose targets are already highlighted.
- **All previews look the same.** A command returns the curves to preview from
  `Command::preview`, and the viewport draws every one of them the same way, with
  `curva_viewport::ghost_lines`. No command chooses its own colour or line style, or draws for itself.
- **The rule is tested.** A new command that changes geometry must be listed in `PREVIEW_STEPS` in
  `crates/curva-commands/src/commands/mod.rs`, or the tests fail.

## When you are stuck

- **Three genuine attempts, then stop.** If the same test still fails, write up what you tried and
  what you suspect, label the issue `needs-human` or `blocked`, and move on.
- **Same failure twice in a row means change approach** or escalate.
- **Flaky tests are quarantined, not retried:** move them to the quarantine list and open an issue.
- When a session's time or budget runs out, hand over with notes in `STATUS.md`.

## Keep going (owner's rule, 9 November 2026)

- **Do not ask the owner whether to continue.** When a piece of work is merged and the next one is
  already planned, say what happened in a line or two and start it.
- **Stop only for a real decision** (see "What only the owner decides"), or when blocked. Ask then,
  with a recommendation.
- Owner feedback can arrive at any time. Fix it first if it is small, then carry on.

## What only the owner decides

Product and interface behaviour, spec approval, architecture decisions, anything that weakens a
test or reference, new dependencies with licence implications, releases. Everything else proceeds.

## Ending a session

Update `STATUS.md`: what changed, what is in progress, known problems, handoff notes.
