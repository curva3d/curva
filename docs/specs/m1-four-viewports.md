# Spec: Milestone 1, four viewports and standard views

**Status:** Approved by the owner in chat · 9 November 2026 (asked to "keep moving on" and to work "as automated as possible", after the plan of snaps, then views and construction planes, then more commands)
**Scope comes from:** architecture section 10, Milestone 1.

## 1. What this is for

So work can happen in 3D. Until now every click landed on the ground plane.

## 2. What the owner will be able to do

- See Top, Perspective, Front and Right viewports at once, in Rhino's arrangement, each with its own camera.
- Click in Front or Right to draw on that view's plane, so geometry can leave the ground.
- Right-drag to pan in Top, Front and Right, which are flat parallel views. Right-drag orbits Perspective, and Shift + right-drag pans it. The wheel zooms the viewport under the cursor.
- Double-click a viewport's title to maximise it, and again to go back to four.

| Command | Does |
|---|---|
| `ZoomExtents` (`ze`, `zea`) | Fits the model in every viewport |
| `4View` | Returns to four viewports in their standard arrangement |
| `Top`, `Front`, `Right`, `Perspective` | Turns the last-used viewport into that view |

## 3. Construction planes

Top and Perspective draw on the world XY plane, Front on XZ, and Right on YZ. Each viewport shows its plane's grid. Grid snap and ortho work within the viewport's plane.

## 4. Not in this piece of work

Custom construction planes (the `CPlane` command), named views, shaded display modes, and resizing viewports by dragging their borders. Typed coordinates stay world coordinates for now; in Rhino they follow the viewport's plane.

## 5. How it is checked

No Rhino tests (decision 0011). Automatic checks cover each standard view's direction, parallel projection, clicks landing on each plane, grid snap and ortho on each plane, and the view commands.

**The owner's check:** he tries it by hand and says whether it behaves like Rhino.
