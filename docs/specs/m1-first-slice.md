# Spec: Milestone 1, first slice: a small working Curva

**Status:** Draft, waiting for the owner's approval in chat
**Decisions behind it:** 0003, 0010 and 0011. Scope comes from architecture sections 9 and 10.

## 1. What this is for

The first version of Curva that the owner can open and use, so he can judge how it feels early. It is
deliberately small. Later slices add commands to the same program.

## 2. What the owner will be able to do

- Open Curva: one window with a large 3D viewport, a command line, and a small status area.
- Type a command by its Rhino name or alias, with autocomplete, and press Enter or Space to run it.
- Answer each prompt either by clicking in the viewport or by typing, for example `10,5,0`.
- Press Enter on an empty command line to repeat the last command, as in Rhino.
- Orbit, pan and zoom with the mouse.
- Click objects to select them, and press Delete to remove them.
- Save and reopen the work as a simple `.curva` file. The final file format is decided later.

## 3. First commands

| Group | Commands |
|---|---|
| Curves | Line, Polyline, Circle, InterpCrv |
| Transform | Move, Copy |
| Edit | Delete, Undo, Redo |
| File | Save, Open, New |

Names, aliases, prompts and options follow Serpentine3D's, with its licence notice kept in
`ATTRIBUTION.md`. Where Serpentine and Rhino differ, Rhino wins.

## 4. Not in this slice

One viewport instead of four, layers panel, object snaps, ortho, gumball, construction planes, history,
surfaces and solids, scripting, and the MCP server. All of these stay in Milestone 1 or later.

## 5. How it is built

Small crates, with higher layers using lower ones only:

| Crate | Job |
|---|---|
| `curva-kernel` | Curva's interface to the geometry engine. Nothing else touches solidcore directly |
| `curva-document` | Objects, selection, undo and redo, saving and opening |
| `curva-commands` | The command framework, and one file per command |
| `curva-viewport` | Drawing with wgpu |
| `curva-app` | The window and command line, with egui |

Every command is a sequence of input requests ("pick a point", "enter a number"), so the command line,
clicks and, later, scripts and agents all drive it the same way. All wording lives in a translation file.

New libraries: solidcore (Apache-2.0), wgpu and egui (MIT / Apache-2.0), serde for files, and whatever
those need. All must pass the licence check.

## 6. How it is checked

No Rhino tests (decision 0011). Every command has automatic checks that run it without a window:

- exact answers: line length, circle length and centre, Move and Copy positions
- InterpCrv passes through every input point
- Undo returns the document to exactly its earlier state, and Redo repeats the change
- saving and reopening gives the same objects
- a picture of the viewport from each run, for a visual check

**The owner's check:** he tries each command by hand and says whether it behaves like Rhino.

## 7. Done when

- All commands in section 3 work from both the command line and viewport clicks.
- Automatic checks pass on Linux, Windows and macOS.
- The owner has a simple way to start Curva, and has tried it.
- Orbiting a scene of a few thousand curves stays smooth.

## 8. Known risks

- solidcore has no ready-made curve commands, so `curva-kernel` calls its lower-level parts. Those may
  change between solidcore releases, so the version stays pinned.
- Adding solidcore means longer builds: about 4 minutes on this machine for the part used so far.
- Pictures from automatic runs need a graphics device, which the Linux and macOS check machines may lack.
  If so, pictures are made on this machine only.
