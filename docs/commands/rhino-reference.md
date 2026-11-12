# Rhino reference for Curva's commands

The reference every command is checked against (spec `docs/specs/command-framework.md` §5): for each command,
Rhino's steps in order, Rhino's options, which of them Curva has, and its status.

**How to read and keep it:**
- One row per command. Options are written in backticks, exactly as Rhino names them.
- **Status** is `done` (every Rhino step and option is in Curva), `partly`, or `missing` (not in Curva yet).
- A test (`the_rhino_reference_matches_the_commands` in `crates/curva-commands/src/commands/mod.rs`) reads this
  table. For every command written on the framework, each option under **In Curva** must exist, and a `done` row
  must have every option under **Rhino's options**. Change the table and the code together.
- Rhino's behaviour is the source. Write a row from Rhino itself (or its documentation), never from Curva's code.

**Audit in progress:** the three commands moved first onto the framework are below. The rest are added by the
audit (step 3 of the spec), tab by tab.

| Command | Rhino's steps | Rhino's options | In Curva | Status |
|---|---|---|---|---|
| Line | Start of line; End of line (typed distance, angle, or both) | `BothSides` `Normal` `Angled` `Vertical` `FourPoint` `Bisector` `Perpendicular` `Tangent` `Extension` | `BothSides` | partly |
| Arc | Center of arc; Start of arc (radius); End point or angle (`Length`). With `StartPoint`: start, end, point on arc (`Direction`) | `Deformable` `StartPoint` `Tangent` `ExtendByArc` `ExtendByLine` | `StartPoint` | partly |
| Box | First corner of base; Other corner of base or length; Height, Enter uses the width | `Diagonal` `3Point` `Vertical` `Center` `BothSides` | `Center` `BothSides` | partly |
