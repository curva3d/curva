# Spec: the command framework

**Status:** Approved by the owner in chat · 12 November 2026. First in the order of work (decision 0016).
**Covers:** feedback items 4, 6, 7, 19, 20, 22 and 23 (`docs/specs/feedback-2026-11-12.md`).

## Why

Each command today handles its own steps, typed input, options and preview. That is why some take typed
dimensions and others do not, why Arc has one way of drawing where Rhino has five, and why previews differ.
Curva will have hundreds of commands, written mostly by AI agents. Everything a command needs that is the
same across commands must be written once, in the framework, so a new command only states what is its own.

## 1. A command is a list of steps and options

A command declares, in a few lines:
- **Its steps**, built from a small set of reusable kinds:

  | Step | Asks for | Typed alternative |
  |---|---|---|
  | `point` | a position, optionally measured from an earlier one (§2) | coordinates, a distance, an angle |
  | `distance` | a length from a base point | a number, or a point |
  | `along` | a length along a direction: heights and extrusions (§3) | a number |
  | `angle` | an angle about a centre | a number in degrees |
  | `number` | a count or factor | a number |
  | `objects`, `edges`, `faces` | picked things, with a filter of which kinds | names |
  | `choice` | one of a few words | the word, or its start |

- **Its options**, each a toggle, number or choice with a default. The framework shows them in the prompt
  the Rhino way, `Extrusion distance <10> ( Solid=Yes  BothSides=No ):`, takes them typed or clicked,
  shortened to any unambiguous start, and remembers each between runs as `command.option`. A command never
  formats or parses its own options.
- **What it makes** from the answers: one function, used for both the live preview and the result, so the
  preview is always exactly what Enter will give.

Undo, the selection after the command, the repeat on Enter, Escape, and the layer the result goes on are
the framework's, the same for every command.

## 2. Typed dimensions in every step (item 6)

Every `point` step that is measured from an earlier point accepts, typed into the command line:

| Typed | Means | Then |
|---|---|---|
| `25` | 25 from the base point | the mouse chooses the direction, with snaps and ortho; click or Enter to accept |
| `<30` | at 30° on the construction plane | the mouse chooses the distance along that line |
| `25<30` | both | placed at once |
| `@10,5` or `@10,5,2` | relative to the base point | placed at once |
| `10,5` or `10,5,2` | absolute, on the construction plane (`w10,5,2` in world coordinates, as now) | placed at once |

While a distance or angle is locked, the preview and the cursor tag show it held, and the mouse only
supplies what is still free. This is Rhino's behaviour, done once in the framework, so it works in Line,
Arc, Rectangle, Box, Move, Copy, Rotate, Scale, ExtrudeCrv and every other command with such a step.

## 3. Sizes that follow the mouse and its snaps (item 4)

An `along` step measures along a direction, such as the extrusion direction, or Box's height off its base.
The mouse position, after snapping, is projected onto that direction, and the distance follows it. So
the top of an extrusion lines up with a snapped corner or edge of another object, as in Rhino. A typed
number still wins. ExtrudeCrv, Box, Pipe's radius, OffsetSrf, Revolve's angle (`angle` works the same
way about its axis) and every other sizing step use it.

## 4. Feedback while a command runs (items 19, 20, 22, 23)

- **Ghosted previews in orange.** Every step that can show a result does: curves as orange lines, surfaces
  and solids as see-through orange faces with their edges, like Serpentine3D's. Fillets, chamfers, shells,
  booleans and extrusions show the solid they will make, not lines. The objects being changed are drawn
  faded meanwhile.
- **A dimension next to the cursor.** Each `point`, `distance`, `along` and `angle` step names what it
  measures (distance from the base, radius, length, height, angle), and the value shows in a small tag
  beside the cursor, in the model's units: `25.736 mm` or `30°`.
- **Pointers that say what to do.** Each kind of step sets the pointer: a crosshair to pick a point, a
  pointer with a small box to pick objects, edges or faces, and the normal arrow otherwise. The interface
  uses the same pointer set for its own handles: resize arrows on panel edges, a grab hand on grips and the
  gumball (`ui-look-and-layout.md` §3).

## 5. Rhino as the reference (item 7)

`docs/commands/rhino-reference.md` holds a row for every Curva command, and for Rhino commands on the
roadmap, with Rhino's prompts in order, each option and its default, and the typed input each step
takes. Curva's status is marked against each: done, partly, or missing. It is written first, by
auditing every command (decision 0016), and kept up to date. Examples of what the audit adds:
- **Arc:** Rhino starts with `Center of arc ( Deformable StartPoint Tangent ExtendByArc ExtendByLine )`.
  StartPoint gives the three-point and start-end-direction ways, and the last step takes `Angle` or `Length`.
- **BooleanDifference, BooleanUnion, BooleanIntersection:** `DeleteInput=Yes`, to keep the originals.
- **Circle:** `3Point`, `Tangent`, `AroundCurve`, `Diameter` and the rest; **Rectangle:** `3Point`,
  `Vertical`, `Center`, `Rounded`.

A test reads the reference and fails when a command marked done is missing a step or option that the
table lists, so the table and the code cannot drift apart.

## 6. How an agent adds or completes a command

Written into `AGENTS.md` once this is approved:
1. Find or write the command's row in `docs/commands/rhino-reference.md` from Rhino's own behaviour.
2. Declare the command: its steps, its options and its making function, using only the framework's kinds.
   A need the framework does not meet is added to the framework, never worked around in one command.
3. Tests: one per option and per typed form that the command's steps allow. The preview and undo checks
   that apply to every command run without extra work.
4. Mark the row done.

## 7. Order of work

1. The framework, with three commands moved onto it to prove it: Line, Arc (all of Rhino's ways) and Box
   (heights following the mouse).
2. Ghosted solid previews and the cursor tag, which need drawing support.
3. The Rhino audit, `docs/commands/rhino-reference.md`, for every command.
4. Commands moved tab by tab (decision 0016): Curves, Operations, Surfaces, Solids, then the rest, each
   gaining the Rhino steps and options its row lists.

## 8. Not in this piece of work

New commands that are not yet in Curva (they follow later, through the same workflow), the gumball
(`gumball-rhino.md`), and snapping changes (`picking-snaps-undo.md`).

## 9. How it is checked

- Automatic: each typed form of §2 in a measured `point` step, on Line, Arc and Box. An `along` step
  follows a snapped point, and a typed number overrides it. Options print in Rhino's style, can be
  shortened, and are remembered. The reference test of §5. The existing preview test, extended so every
  command's preview equals its result.
- **The owner's check:** tries Line, Arc and Box against Rhino, then each tab as it moves.
