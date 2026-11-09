# 0008 — Licence and health check of supporting libraries

**Status:** Accepted · 8 November 2026 · The owner chose the recommended option for both decisions
**Answers:** architecture open decisions 4 and 5 · task M0-17

**What we checked:** each library's licence, latest release and recent activity, on 8 November 2026.
We did **not** test how well any of them works. Quality is what the kernel evaluation is for.

| Library | Used for | Licence | Latest release | Commits, last 90 days | Fits Curva's licence? |
|---|---|---|---|---|---|
| curvo 0.3.3 | NURBS curves and surfaces | MIT | Oct 2026 | 60 | Yes |
| monstertruck 0.4.1 | Geometry kernel candidate | Apache-2.0 | Sep 2026 | 27 | Yes |
| csgrs 0.20.1 | Mesh booleans | MIT | Jul 2025 | 100 or more | Yes |
| boolmesh 0.2.2 | Mesh booleans | MPL-2.0 | Oct 2026 | 38 | **Not on our permissive list** |
| openbim-ifc 0.41.0 | IFC import and export | AGPL-3.0, or paid commercial | Nov 2026 | 100 or more | **No** (decision 0002) |

All five are actively worked on and none is abandoned. csgrs is very active, but its last published
release is over a year old, so using current work would mean taking it straight from its source code.

## Decision 1: IFC library

openbim-ifc covers IFC2x3, IFC4 and IFC 4.3, which is what Curva needs, but it is under the AGPL.
That licence would force Curva's own code to be offered under the AGPL too, which decision 0002
rules out. It is also offered under a paid commercial licence.

- **Build our own IFC support for M4 (recommended).** Curva stays freely licensed, and we can limit
  it to what walls, slabs, openings, doors and windows need. It is real work, but M4 is far off.
- **Buy the commercial licence.** Fastest route to IFC, at a cost we don't yet know, and it ties an
  open-source project to one company's terms.
- **Change Curva's licence to AGPL.** Reverses decision 0002 and stops others reusing Curva freely.
  Not recommended.

Nothing depends on this until M4.

## Decision 2: mesh boolean library

boolmesh is under the MPL-2.0: changes to boolmesh's own files must be shared, but Curva's code is
unaffected. Using it needs the owner's approval, because it is not on the permissive list.

- **Use csgrs first (recommended).** MIT, so no licence question. How robust it is remains untested.
- **Allow boolmesh too and test both.** Better chance of finding the most reliable one, at the cost
  of adding a licence with a sharing condition.

## Also noted

monstertruck and curvo raise no licence issues. curvo's name is close to Curva's, as the architecture
already warns.
