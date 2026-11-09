# Spec: Milestone 1, Split, Trim, Offset and Fillet

**Status:** Approved by the owner in chat · 9 November 2026 (the plan these commands were part of, with "keep going")
**Scope comes from:** architecture section 10, Milestone 1, phase F1 commands.

## 1. Commands

| Command | Alias | Asks for | Does |
|---|---|---|---|
| `Split` | | Curves, then cutters | Cuts curves where they cross the cutters, keeping every piece |
| `Trim` | `tr` | Cutters, then clicks | Removes the clicked part of a curve, until Enter |
| `Offset` | | Curves, then a side, or a number for the distance | Copies curves at the distance, on the side clicked. The distance is remembered |
| `Fillet` | | First line, second line, or a number for the radius | Rounds the corner, keeping the sides clicked. Radius 0 makes a sharp corner. The radius is remembered |

## 2. Limits of this piece of work

- Offset of smooth curves fits a curve through offset points, staying within 0.01 mm in the tested case. Offset refuses joined curves, which can be exploded first, and sharp mitres only for polylines.
- Fillet works between lines only so far, not arcs, smooth curves or polyline segments.
- Trim and Split need curves to really cross or touch; they do not extend cutters.

## 3. How it is checked

No Rhino tests (decision 0011). Automatic checks cover exact results for each, pieces adding back up to the whole, pieces staying on the original curve, refused inputs leaving the model unchanged, and undo.

**The owner's check:** he tries each command by hand and says whether it behaves like Rhino.
