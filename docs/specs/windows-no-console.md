# Spec: no console window on Windows

**Status:** Draft, for the owner's approval · 13 November 2026.
**Covers:** feedback on 0.4.0, point 5.

Curva is built as a Windows *GUI* program, so Windows opens no console beside it, whether started from
the Start menu, a file, or the installer.

What still needs a console keeps working:
- `curva --mcp` (the AI Assistant bridge, and Claude Desktop's connection) talks through standard input
  and output, which a GUI program still has when another program starts it with pipes.
- Run from a terminal (`curva --help`, `--version`, errors at start), Curva attaches to that terminal's
  console and prints there.
- Messages that went to the console (logs) also go to `curva.log` in Curva's settings folder.

**Checked by:** opening the installed Curva from the Start menu (no console), running `curva --version`
in PowerShell (prints), and the AI Assistant's tools working through the bridge.
