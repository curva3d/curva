# 0026: Contributors sign a Contributor Licence Agreement

**Status:** Decided by the owner in chat · 15 November 2026.

**What was decided.**
- **Every outside contributor signs `CLA.md` once**, by commenting a fixed sentence on their first
  pull request. A GitHub check (`.github/workflows/cla.yml`) asks for it and stays red until every
  author has signed. Signatures are kept in `signatures/cla.json` on the `cla-signatures` branch.
- **It is a licence, not a transfer of copyright.** Contributors keep their copyright and grant the
  Maintainer a broad licence (copyright and patents) to use their work under any terms, including
  paid and closed editions.
- **A promise back:** every accepted contribution always stays available under the open-source
  licence of the part of Curva it went into (decision 0025). The Maintainer may add other terms,
  never replace them with closed ones only.
- **The owner's account and bots need no signature.** Agents commit through the owner's account.
- The agreement can pass to a company or foundation set up for Curva later.

**Why.** Under decision 0025 the application is GPL. Without a CLA, once others contribute, their
code could only ever be used under the GPL, which would rule out paid features built into the
application itself or a commercially licensed edition. With it, Curva keeps those options. Doing it
now, before the first outside contributor, means no one ever has to be asked afterwards.

**Rejected.**
- *Copyright assignment* (contributors hand over ownership): puts off more contributors and isn't
  needed, since the licence already allows everything Curva plans.
- *DCO only* (contributors certify the work is theirs, no extra rights): gives no right to offer
  contributions under other terms.
- *cla-assistant.io* (a hosted service): needs the owner to sign in and set it up there; the
  workflow lives in the repository instead.

**Known risk.** The workflow uses `contributor-assistant/github-action`, whose repository is
archived. The pinned release still works; if it breaks, swap in a maintained fork or a small
workflow of our own. Signatures already recorded are kept either way.

**Before relying on it commercially:** have a lawyer review `CLA.md`, and put the owner's legal name
(or a company's) in its definition of "Maintainer".
