# Spec: no console window on Windows

**Status:** Approved by the owner in chat · 13 November 2026.
**Covers:** feedback on 0.4.0, point 5.

Curva is built as a Windows *GUI* program, so Windows opens no console beside it, whether started from
the Start menu, a file, or the installer.

What still needs a console keeps working:
- `curva --mcp` (the AI Assistant bridge, and Claude Desktop's connection) talks through standard input
  and output, which a GUI program still has when another program starts it with pipes.
- Run from a terminal, the command-line modes (`--picture`, `--orbit-benchmark`) print when their
  output is piped (`curva --picture out.png --run line | more`): attaching to the terminal's console
  needs a Windows call Curva's no-`unsafe` rule does not allow.
- An error that stops Curva starting is also written to `curva.log` in Curva's settings folder.

**Checked by:** opening the installed Curva from the Start menu (no console), running `curva --picture p.png | more`
in PowerShell (prints), and the AI Assistant's tools working through the bridge.
