# Spec: Milestone 1, MCP server (first part)

**Status:** Approved by the owner in chat · 9 November 2026 (the plan this was part of; he asked for development to continue without being asked)
**Scope comes from:** architecture sections 5c and 10, Milestone 1.

## 1. What this does

`curva --mcp` starts Curva as an MCP server on standard input and output, working on its own model with no window. An AI agent can:

| Tool | Does |
|---|---|
| `run_commands` | Runs lines on the command line, exactly as a person types them. The whole call is one undo step |
| `list_objects`, `list_layers` | Reads the model |
| `select_objects` | Sets the selection, for commands that use it |
| `render_view` | Returns a picture of the model, so the agent can check its work |

## 2. Not in this piece of work

From architecture section 5c, still to come:

- The built-in assistant panel.
- Saving is through the `Save` command, with a path.

## 3. How it is checked

No Rhino tests (decision 0011). Automatic checks cover the protocol messages, each tool, one undo step per call, bad input, and pictures, with a stand-in for the graphics card.

**The owner's check:** connect an agent and confirm that it can draw and see the result.

## 4. The open window

`curva --mcp --connect` connects an agent to the Curva window that is open, so the user sees what it does as it happens.

- It only works while the **Agent access** checkbox under the command line is ticked. That starts off each time Curva opens.
- The window listens on this computer only, and a client must present a random secret from a file in the user's temporary folder.
- `Delete`, `New`, `Open` and `Save` wait for the user to click Allow or Decline.
