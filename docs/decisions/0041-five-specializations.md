# 0041: Urban and Publisher are both full specializations

**Status:** Decided by the owner in chat · 23 November 2026, while reviewing the website.

Decision 0017 put **Publisher** in the app's specialization switch, while the roadmap and the
website said **Urban**. The owner chose to keep both, so Curva has five specializations:

1. **Freeform** (the base), out now.
2. **Architecture**, coming next.
3. **Urban**: massing from rules, real context, streets and blocks from street sections.
4. **Publisher**: everything that leaves Curva: drawing sets, title blocks, revisions, boards and
   exports, live from the model, on the views-and-sheets core from M3.
5. **Analysis**, later.

`docs/architecture.md` (section 3 and the milestones: M6 Publisher, M7 Analysis), the README and the
website follow this order. **Still to do:** the app's switch shows Freeform, Architecture, Publisher
and Analysis; Urban has to be added there.

**Why:** Urban and Publisher are different kinds of work (city-scale massing versus drawing sets),
and the owner wants both. **Rejected:** renaming one into the other. **Revisit if** the two turn out
to share most of their tools.

**Website order (owner, 23 November 2026):** the website and README show Freeform, Architecture,
Urban, Analysis, Publisher, with Freeform on top of the stack. The build order in
`docs/architecture.md` is unchanged (M6 Publisher before M7 Analysis), since Publisher builds on the
views-and-sheets core from M3.
