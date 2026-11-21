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

## Streams (owner's rules, 15 and 16 November 2026)

Work runs in three streams, each in its own sessions. A session says which stream it is in.

| Stream | Does | Picks up issues labelled | Owns |
|---|---|---|---|
| **display** | The viewport: drawing, display modes, speed on large scenes | `display` | The inside of `crates/curva-viewport` |
| **mesh** (from 16 November 2026, decision 0032) | Mesh and SubD objects: their geometry, commands, files and tests | `mesh` | The inside of `crates/curva-mesh`, and its own new files elsewhere (below) |
| **features** | Everything else: commands, NURBS geometry, documents, files, the interface | `agent-ready` issues with neither `display` nor `mesh` | Everything not owned by the other two |

- **Each stream writes only its own status file:** `status/display.md`, `status/mesh.md` or
  `status/features.md`. `STATUS.md` only points to them; do not write progress there.
- **The display stream owns the inside of `crates/curva-viewport`.** Other streams use only its
  public functions and types. If they need a viewport change (a new function, a fix, a different
  look), they open an issue labelled `display` describing what they need and why, instead of
  editing the crate. The display stream folds those issues into its plan.
- **The mesh stream owns the inside of `crates/curva-mesh`** (Curva's own mesh and SubD cage
  structure, and the only place that calls manifold-rust and subdiv-kernels). In the crates the
  features stream owns, it **adds its own new files only**: mesh and SubD commands in their own
  folders under `crates/curva-commands/src/commands/mesh/` and `.../subd/`, a mesh and SubD module
  in `curva-document`, mesh and SubD reading and writing in the file crates (`curva-3dm` and
  others). Registering them takes one-line additions to existing lists (a new shape kind, a command
  in the registry, a menu entry); those are allowed. **Any other change to existing shared code**
  (the command framework, document core types, the interface layout, an existing file reader) goes
  through an issue labelled `agent-ready` for the features stream, describing what is needed and
  why. Viewport needs go through `display` issues as above.
- **SubD to NURBS** uses solidcore only through `curva-kernel`'s public interface. If the mesh
  stream needs something new from the kernel, it asks the features stream with an issue.
- **Only the display stream runs viewport speed benchmarks** (`--orbit-benchmark` and any other
  timing of drawing). Other streams may time their own geometry (a boolean, a subdivision), but
  another session may be building on the same machine, so they rerun any result that looks off
  before reporting it, and say in the report when they did.
- **Disk space is shared:** remove a worktree and its build folder as soon as its pull request is
  merged or closed.

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

## Keeping the code tidy (owner's rule, 15 November 2026)

The code grows fast, so keep it easy to find your way around. Leave each file you touch at
least as tidy as you found it.

- **Size limits.** A source file stays under about 800 lines, not counting its tests. A function
  stays under about 80 lines. If a change would push a file or function past that, split it first,
  in a separate pull request or a separate commit before your change. Long tables of plain data are
  the only exception.
- **Fit in, don't bolt on.** Put new code in the module where it belongs. Never make a numbered
  copy such as `making2`. If nothing fits, make a new module whose name says what it does.
- **Same logic three times means a shared helper.** For commands, the helper goes in the command
  framework, so every command gets it.
- **Don't crash on outside input.** Code that reads files or user input never uses `unwrap` or
  `expect` on what it read. It returns an error the user can see. Outside tests, use them only
  where failure is impossible, with a comment saying why.
- **Tidy-up pull requests change no behaviour.** They move and split code only. Tests stay as
  they are: they may move with their code, but are never changed to pass. Issues for this work are
  labelled `tidy-up`. A pull request that only moves code may be larger than usual; say so in it.
- **Tidy-up first (owner's rule, 19 November 2026):** every stream starts each session by
  completing one open tidy-up issue in code it owns, before its next feature. If its stream has
  none open, it goes straight to features.

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
  `curva_viewport::ghost_lines`. No command chooses its own colour or line style, or draws for itself. Point previews are the same colour, drawn as markers because they have no lines.
- **The rule is tested.** A new command that changes geometry must be listed in `PREVIEW_STEPS` in
  `crates/curva-commands/src/commands/mod.rs`, or the tests fail.

- **A typed size holds** (owner's rule, 21 November 2026, #510): a distance, angle or size typed in the
  command line holds the preview, and so the geometry made, from the moment it is typed and for as long as
  it stays there; the mouse then only gives the direction, and a click places it at that size. Deleting it
  gives the mouse back. The framework does this for every command (`Processor::set_typed`); no command
  works around it.

## Writing or completing a command (owner's decision, 12 November 2026)

Every command is written on the command framework (`docs/specs/command-framework.md`), and matches Rhino.
To add a command, or bring one up to Rhino:
1. **Rhino first.** Find or write its row in `docs/commands/rhino-reference.md`, from Rhino itself: its steps
   in order, its options, and what each step takes typed.
2. **Declare it as a recipe** (`crates/curva-commands/src/recipe.rs`): its options (`OptionDef`), the steps
   it asks given the answers so far (`StepDef`), and one `make` function used for both the preview and the
   result. List it with `recipe!` in `commands/mod.rs`. See `line.rs`, `arc.rs` and `box_solid.rs`.
3. **Use only the framework's kinds.** Typed distances and angles, sizes that follow the mouse (`along`),
   options in Rhino's style and remembered, the cursor tag and the preview come with them. If the framework
   cannot express something a command needs, add it to the framework for every command; never work around it
   in one command.
4. **Tests:** one per option and per typed form its steps take. The preview, undo and reference-table checks
   run for every command without extra work.
5. **Update the row** under In Curva and Status. The reference test fails if the table and the code disagree.

## When you are stuck

- **Three genuine attempts, then stop.** If the same test still fails, write up what you tried and
  what you suspect, label the issue `needs-human` or `blocked`, and move on.
- **Same failure twice in a row means change approach** or escalate.
- **Flaky tests are quarantined, not retried:** move them to the quarantine list and open an issue.
- When a session's time or budget runs out, hand over with notes in your stream's status file.

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

Update your stream's status file (`status/display.md`, `status/mesh.md` or `status/features.md`): what changed,
what is in progress, known problems, handoff notes.
