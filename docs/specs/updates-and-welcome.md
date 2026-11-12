# Spec: updates and the welcome

**Status:** Approved by the owner in chat · 12 November 2026. Fourth in the order of work (decision 0017).
**Covers:** feedback items 30 to 33 (`docs/specs/feedback-0.3.0.md`).

## 1. Checking for updates

- On opening (unless turned off in Settings → General), Curva asks GitHub, in the background, for the
  latest release of `curva3d/curva`. Nothing waits for the answer; no network means no message.
- If it is newer, the command bar's first line says "Curva 0.4.0 is available. Click here to update."
- Clicking it downloads the file for this system from the release, showing progress in the command bar:
  - **Windows:** offers to save a changed model, closes Curva, runs the new installer silently, and
    reopens Curva when it finishes.
  - **Linux:** replaces the running AppImage with the new one and reopens Curva.
  - **macOS:** opens the new disk image for the user to drag to Applications, as an unsigned app cannot
    be replaced in place; Curva then quits, and a small helper it leaves running waits for the new copy
    in Applications and opens it.
  In every case the model that was open, if saved, opens again.
- Before installing, the file's size and SHA-256 are checked against the release's checksums file,
  which the release workflow now publishes.

## 2. After an update

The first time a new version opens, the command bar says "Updated to Curva 0.4.0. See what's new." with a
link to that release's notes on GitHub.

## 3. The welcome

After any update line: "Curva is ready. What will you Create today?" These opening lines glow softly in
the accent for about two seconds, then settle.

## 4. How it is checked

Version comparison, choosing the right file for each system, the checksum check, and the order of the
opening messages are tested; the download and install are checked by hand on each system.
