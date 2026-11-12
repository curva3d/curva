# Spec: the AI Assistant

**Status:** Proposed · 12 November 2026. Fifth in the order of work (decision 0017).
**Covers:** feedback item 34 (`docs/specs/feedback-0.3.0.md`), following Serpentine3D's assistant panel
(`serpentine3d/ai`).

## 1. The panel

The agent panel becomes **AI Assistant**. Unconnected, it offers, as Serpentine3D does:
- **ChatGPT:** sign in with a ChatGPT account.
- **Local model:** LM Studio on this machine (its local server), choosing from the models it has loaded.
- **API key:** OpenAI or Claude, with the model chosen from a list.
- **External assistant · MCP:** step-by-step instructions to connect Claude Desktop, Claude Code or
  another MCP client to Curva, with the configuration to copy.

The chosen connection shows at the top ("LM Studio · local · <model>"), with buttons for a new chat and
settings. Keys are kept in the system's credential store, never in the layout or the model.

## 2. The chat

A conversation in the panel. The assistant has the same tools as the MCP server (run commands, read the
model and the selection, take pictures of views), so it works on the model directly. Changes it makes go
through Agent access approval as now, and each request it carries out is one undo step.

## 3. Order of work

1. Panel, connection choices and MCP instructions.
2. Chat with local models and API keys (OpenAI, Claude), sharing the MCP tools.
3. ChatGPT account sign-in.

## 4. How it is checked

Tool calls from the chat reach the same handlers as MCP (tested with a fake model); keys never reach a
file Curva writes; the owner tries each connection.
