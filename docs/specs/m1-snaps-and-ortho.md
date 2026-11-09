# Spec: Milestone 1, object snaps and ortho

**Status:** Approved by the owner in chat · 9 November 2026 ("go ahead, please do this as automated as possible", after choosing snaps and ortho as the next work)
**Scope comes from:** architecture section 10, Milestone 1.

## 1. What this is for

So points can be placed exactly on existing geometry. Until now a click could only land on the ground plane.

## 2. What the owner will be able to do

While a command asks for a point:

- See a marker and a label when the cursor is near something to snap to, and click to use that exact point.
- Turn each snap on or off in a row of checkboxes under the command line, as in Rhino's Osnap bar.
- Use **ortho** to keep the next point straight along X or Y from the last point. F8 toggles it, and holding Shift flips it for the moment.
- Use **grid snap** to round the point to whole units. F9 toggles it.
- Hold Alt to turn object snaps off for the moment.

| Snap | Lands on | On at start |
|---|---|---|
| End | Ends of curves, and corners of lines and polylines | Yes |
| Mid | Middle of each straight segment, or of a smooth curve | Yes |
| Cen | Centre of a circle | Yes |
| Int | Where two different curves meet | Yes |
| Perp | The foot of a right angle from the last point | Yes |
| Near | The closest point on a curve | No, as it would hide the others |

Rhino starts with every snap off. These start on so snaps can be judged straight away; the owner can ask to change that.

**Which wins:** an object snap beats ortho and grid snap. Among End, Mid, Cen, Int and Perp, the closest to the cursor wins. Near is used only when none of those is in reach. Grid snap applies before ortho.

## 3. Also in this piece of work

- `New` asks for confirmation when the model has objects, because it cannot be undone.
- A script, `setup/install-shortcut.ps1`, installs Curva and makes a desktop shortcut.
- `curva --orbit-benchmark --curves N` measures drawing and snapping on a scene of N circles.

## 4. Not in this piece of work

Quad, Knot, Tan, Point and SmartTrack snaps, snaps on objects in other construction planes, and the four viewports.

## 5. How it is checked

No Rhino tests (decision 0011). Automatic checks cover each snap's exact answer, which snap wins, a switched-off snap being ignored, ortho and grid rounding, and the `New` confirmation.

**The owner's check:** he tries it by hand and says whether it behaves like Rhino.
