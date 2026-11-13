# Milestone 2 in 0.4.2 · 13 November 2026

The owner: "update 0.4.2 will contain everything in Milestone 2 plus the new logo." Milestone 2 is
"Freeform surfaces and solids" (`docs/architecture.md` §10: phases F2 and F3, live booleans,
deformation, analysis, blocks, 3DM import and export). This lists every part of it still to build,
numbered, with the spec that covers it. The owner's answers are in decision 0019.

## Files (`3dm-files.md`)

1. Open and import Rhino `.3dm` files; save and export them. Pure Rust, written for Curva.

## Blocks (`blocks.md`)

2. Block definitions and placed instances, embedded in the file and linked to other files, with a
   Blocks panel: Block, Insert, BlockManager, ExplodeBlock, BlockEdit, linked files that update.

## Live booleans (`live-booleans.md`)

3. BooleanUnion, BooleanDifference, BooleanIntersection and BooleanSplit keep their input solids,
   hidden, and the result follows when they change.

## The engine's gaps (`engine-gaps.md`)

4. The 34 commands marked `blocked` in `docs/commands/rhino-parity.md`, built in Curva's own kernel
   on the engine's lower-level pieces where solidcore cannot do them yet.
5. Every such operation is flagged as custom, listed in `docs/kernel/custom-operations.md`, and
   checked against the latest solidcore before every release: whatever solidcore fully covers by then
   is switched to solidcore's and Curva's own is removed.

## Organising, analysis and the rest of F2/F3 (`m2-organise-analysis.md`)

6. Organising commands from phase F3 still missing: Count, ChangeLayer, MatchProperties, Linetype,
   PrintWidth (the command), SetUserText, GetUserText, Purge, Audit, LockSwap, UnlockSelected,
   BringToFront, SendToBack, BringForward, SendBackward.
7. Phase F2 commands still missing: DupFaceBorder, PushPull, TextObject.
8. Analysis: CurvatureGraph, Zebra, DraftAnalysis (with the gaps in 4 that are analysis).
9. The commands still outside the command framework moved onto it: Trim, Lock, Unlock, Group,
   Ungroup, Plan, SelColor.

## The logo

10. The Drafting mark with its architectural construction as Curva's icon (already built:
    curva3d/curva#101).

## Not in 0.4.2

Mesh and SubD (PointCloud, MeshToBrep, BrepToMesh) come after Milestone 4 (`architecture.md` §10d);
SectionHatch and the other drafting tools are Milestone 3.
