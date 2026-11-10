# Spec: Milestone 1, built-in assistant panel

**Status:** Postponed by the owner in chat · 10 November 2026, in favour of surfaces. Kept as a draft for later.
**Scope comes from:** architecture section 5c ("A built-in assistant panel connects to Claude or another model through the same MCP layer").

## 1. What the owner will be able to do

- Open an **Assistant** panel on the right of the window and type a request in plain words, such as "draw a 6 by 4 m rectangle with 300 mm filleted corners on layer Walls".
- Watch the assistant work in the viewports as it goes. It uses the same commands as the command line, so each of its steps shows in the command history.
- Each request the assistant carries out is **one undo step**, so Ctrl+Z takes back everything it did for that request.
- `Delete`, `New`, `Open` and `Save` still wait for **Allow** or **Decline**, as they do for outside agents today.
- The assistant can look at a picture of the viewport to check its own work, as Serpentine3D's does.
- A **Stop** button ends a request part-way. What it has done so far stays, and can be undone in one step.

## 2. How it connects

- It talks to Claude over the internet, using the user's own Anthropic API key. The key is typed once in the panel's settings and kept in the operating system's password store (Windows Credential Manager, macOS Keychain), never in Curva files or the repository.
- It uses the same tools the MCP server already offers (`run_commands`, `list_objects`, `list_layers`, `select_objects`, `render_view`), so the panel and outside agents behave the same.
- The model it uses is chosen in the panel's settings; the default is the latest Claude model.
- With no key entered, the panel says how to get one and does nothing else. Curva works fully without it.

## 3. New supporting libraries

Talking to the internet needs a small web library (`ureq` with `rustls`, both MIT or Apache) and one for the password store (`keyring`, MIT or Apache). These pass the licence checks; they are listed here because the rules ask for the owner's approval of new dependencies with licence implications.

## 4. Not in this piece of work

Other model providers, voice, attaching files or images to a request, and remembering conversations between sessions.

## 5. How it is checked

Tests never use the network (`AGENTS.md`). Automatic checks use a stand-in for Claude that replays recorded answers, and cover: one undo step per request, Stop, the approval prompts, a missing or wrong key, and the key never being written to a file.

**The owner's check:** enter a key, ask for a few drawings, and say whether it feels useful.
