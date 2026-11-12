# 0017: Choices made on the feedback to 0.3.0

**Status:** Decided by the owner in chat · 12 November 2026, after trying 0.3.0.

The owner's feedback (`docs/specs/feedback-0.3.0.md`) left these open; the owner chose:

1. **Specializations.** The top bar's dial shows Freeform, Architecture, Publisher and Analysis. Only
   Freeform has commands; the others show a short note that they come in a later milestone.
2. **Command buttons.** The command tabs (Curves, Surfaces, Solids, Operations) and their buttons
   become an ordinary panel, docked under the top bar by default. The top bar holds only the file name,
   Curva, File, Settings and the specialization dial. The command panel shows the tabs of the selected
   specialization.
3. **Updating.** "Click here to update" downloads, installs and reopens Curva on every system: on
   Windows through the new installer; on Linux by replacing the AppImage; on macOS by opening the new
   disk image for the user to drag in (an unsigned app cannot replace itself), after which a helper
   opens the new copy.
4. **AI Assistant.** A full chat in the panel that drives Curva's commands, with a local model through
   LM Studio, an OpenAI or Claude API key, and a ChatGPT account, besides the external MCP connection
   with instructions.
5. **Welcome line.** "Curva is ready. What will you Create today?"
6. **Gumball circle.** It extrudes curves and open surfaces, and stretches closed solids along its axis.
   Shift-drag stretches anything instead of extruding; Shift+Ctrl scales evenly.

Decided by Claude, as the owner asked:

7. **Settings tabs:** General (theme, panels glass or solid, units, grid, check for updates),
   Interface (every panel with a toggle, Reset interface), AI Assistant (providers, keys, model), and
   About last (the owner's addition); File → About opens Settings at that tab.
8. **Rhino parity (the owner's addition).** This version also brings every Rhino NURBS modelling command
   Curva is missing, each with an icon (`rhino-parity.md`).
