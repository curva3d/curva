# 0016: Decisions for the second round of interface work

**Status:** Decided by the owner in chat · 12 November 2026, after trying 0.2.0.

The owner's feedback on 0.2.0 (inventory in `docs/specs/feedback-2026-11-12.md`) raised choices that
only the owner makes. These were asked and answered before any work started.

| Question | Decision |
|---|---|
| What comes first | **The command framework**, then picking and snaps, then the new look, then the gumball, then the logo. Each later piece builds on the earlier ones |
| Where the app menu goes | **A File tab**, first among the command tabs: New, Open, Save, Save As, Import, Export, Settings, About |
| Where the agent (MCP) connection goes | **Its own small panel**, bottom right, so the command bar can be compact and centred. Dockable and hidden in focus mode like the others |
| What Ctrl+Z undoes | **Model changes and the selection they changed**, always, even while the command line has focus. Camera moves are not undone by Ctrl+Z, as in Rhino; Undo View gets its own key |
| Accent colour | **Orange `#e0782f`** for selected objects and sub-objects, previews and every active control: tabs, toggles, picked items |
| Font | **Inter** (SIL Open Font Licence), as a free lookalike of the Claude app's interface, with Claude-style greys |
| Bringing commands up to Rhino | **Audit every command against Rhino first**, in a reference table kept in the repo, then move commands onto the framework tab by tab, adding the missing steps and options as each moves |
| Light or dark | **Follow the system** at start, with Light or Dark forced from Settings. The 3D background and grid change with it |
| Glass or solid (asked for when approving the specs) | **Every panel and interface element frosted glass** by default, with Frosted glass or Solid chosen in Settings; both follow light or dark. The glass must stay light on the machine: one shared low-resolution blur per frame, reused while nothing behind changes, at most 1 ms a frame more on the benchmark scene |

**Also taken from the feedback as stated (no choice needed):** selection by clicking anywhere on a surface or
solid; snaps only to what is visible; sticky snaps with guidelines as in Rhino and ArchiCAD; a Rhino-style
gumball; heights and sizes that follow the mouse and its snaps; typed dimensions in every multi-step command;
a crosshair and other pointers that show what to do; a dimension next to the cursor; ghosted 3D previews in
orange; triple the space between panels; panels resized from any edge or corner; a readable suggestion list;
Ctrl + right-drag to zoom; units in millimetres or metres; Serpentine3D's grid and command icons (MIT,
credited); and a logo with the C centred and the accent orange, in variations for the owner to choose from.
