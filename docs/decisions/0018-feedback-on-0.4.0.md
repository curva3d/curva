# 0018: Choices made on the feedback to 0.4.0

**Status:** Decided by the owner in chat · 13 November 2026, after 0.4.0.

The owner's feedback (`docs/specs/feedback-0.4.0.md`) left these open; the owner chose:

1. **Line weights now.** Print widths in millimetres on layers and objects are built in this version,
   not only specified.
2. **Always shown.** Every view draws each line at its weight, not only in a print preview mode, as
   long as this costs no performance. It costs none: lines become one draw of thin quads with a width
   each (`line-weights.md` §2), as cheap as the 1-pixel lines of 0.3.0.
3. **Logo.** The Drafting variant. Its guides become the real construction of the C, only those that
   build it are kept, and a simplified version is made for small sizes. New variations of the Drafting
   version are made to choose from.

Decided by Claude, as the owner asked to keep going:

4. **The next version is 0.4.1** (the owner: each release adds 0.0.1), released when the specs here are built and checked in the app.
5. **Order of work:** the console (smallest), line drawing and weights, the approximations, the 31
   commands, the logo.
