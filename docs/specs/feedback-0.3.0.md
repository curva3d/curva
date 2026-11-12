# The owner's feedback on 0.3.0 · 12 November 2026

Every point the owner raised after trying 0.3.0, numbered, with the spec that covers it. Choices made in
answer to questions are in decision 0017.

## Fixes (`fixes-0.3.0.md`)

1. Snaps in Ghosted work exactly as in Wireframe: everything snaps, nothing is hidden.
2. Escape with no command running clears the selection.
3. Curva opens filling the screen it opens on.
4. Snaps on curves and polylines stopped working (only polysurfaces snap). Fix, and add tests that snap
   on curves and polylines through the whole app path, so this cannot slip into a release again.
5. Escape during a command ends it but keeps what it already made (Copy keeps the copies placed).
6. Rotating the view while Box or an extrusion previews makes the preview go wrong.
7. Curves, polylines and the edges of polysurfaces draw twice as thick.
8. An orange line appears across the top panel when the pointer is over its middle (screenshot 1).
9. Options of multi-step commands (Circle's `2Point`, `3Point`) cannot be clicked or typed. Fix the
   framework so this works everywhere.
10. A typed value drives the command but the dimension beside the pointer does not show it; it must.
11. Dragging a panel's edge or corner also selects things in the 3D view behind it.
12. ChamferEdge and FilletEdge previews are hidden behind the untouched solid.
13. Ctrl+C, Ctrl+V (and the other shortcuts every program has: Ctrl+X, Ctrl+A) work on objects.
14. Objects being made show their width and depth beside them, as Serpentine3D does (screenshot 4).

## One panel system for everything (`ui-unified-panels.md`)

15. Tabs inside a panel can be reordered by dragging; the same in the top bar.
16. Settings and About open as ordinary panels, with the same look, placed and docked like any other.
17. A cross at a panel's top-right closes it, after asking to confirm and saying where to bring it
    back (Settings). Closing a panel with several tabs closes them all.
18. Settings lists every panel with a toggle to show it, and has Reset interface. Settings is
    organised in tabs.
19. Panels that pop up do so in the centre of Curva's window.
20. Every panel, the top bar included, uses the same fonts, sizes and colours.
21. The hide V always points towards the edge the panel will hide to.
22. The Perspective (display mode) button is its own panel, below the top-left panel by default, with a
    V pointing down to show it opens a list.
23. The software's name and version show at the bottom right of the window.
24. The top bar is called Curva and is always there, fixed (only focus mode hides it), floating with
    the same gap as other panels. Centred: the file name and Curva. Below, at the left: File and
    Settings. Below the name, centred: the specializations Freeform, Architecture, Publisher, Analysis,
    in bold, as a dial that loops, the selected one in the centre.
25. Frosted glass is 25% more see-through.

## Gumball (`gumball-rhino.md`, revised)

26. Remove the scale square; the circle extrudes, and stretches where it cannot extrude.
27. Every gumball action snaps to the model: move, extrude, stretch.

## Icons (`ui-look-and-layout.md` §5, extended)

28. Curve and InterpCrv have the same icon; give them different ones.
29. Draw icons for every existing command, in the same style: from Serpentine3D where it has one,
    otherwise Rhino's idea for that command in Curva's style and colours (not Rhino's colour coding).

## Updates and welcome (`updates-and-welcome.md`)

30. Check for an update on GitHub each time Curva opens; say so in the command bar with "click here to
    update", which updates Curva on every system.
31. After an update, the command bar says so and links to the release notes.
32. On opening: "Curva is ready. What will you shape today?", after the update message if there is one.
33. The opening messages glow briefly.

## AI Assistant (`ai-assistant.md`)

34. The MCP panel becomes the **AI Assistant**: models from cloud to local, as Serpentine3D offers
    (screenshot 2), and instructions for connecting an external assistant through MCP.
