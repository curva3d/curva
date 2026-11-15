# 0027: Vello draws 2D views and sheets, with vello_cpu as the fallback

**Status:** Decided by the owner in chat · 15 November 2026, after the Vello spike (#185).

**What was decided.**
- Live 2D views (plans, sections) and sheets are drawn with **Vello**, fully on the graphics card,
  from the same vector page (`curva_document::drawing::Page`) that the PDF and SVG exports write, so
  what is on screen is what prints.
- On machines below the recommended graphics card (decision 0026), and wherever there is no graphics
  card, the same page is drawn with **vello_cpu** on the processor.
- **Vello Hybrid** is not used: it suited the earlier weak-card baseline, which decision 0026 replaced.
- 3D views keep Curva's own pipeline.

**What the spike found** (branch `185-vello-spike`, not merged; RTX 3060, A3 at 150 dpi,
2520 × 1782 pixels, each run twice on a quiet machine). The test house's plan, cut at 1.2 m, with cut
walls filled black, line weights in paper millimetres, lines below the cut thinner and lighter and one
hatch, built from Curva's own geometry (solids trimmed at the cut, their cut outlines, Make2D's hidden
lines):

| Drawn with | Matches the PDF | Redraw while panning and zooming | On software graphics |
|---|---|---|---|
| Vello, on the graphics card | yes | 1.4 ms | 830 ms, and stopped silently in some runs |
| Vello Hybrid 0.2 | yes | 3.6 ms | 31 ms |
| vello_cpu | yes | 7.8 ms | not needed |

"Matches" means no pixel differed from the PDF, rasterised with hayro, by more than a quarter of the
colour range. hayro itself draws with vello_cpu, so that comparison is partly circular; the
graphics-card renderers are the independent check. Vello 0.12 uses wgpu 30, as Curva does, and ran
within Curva's device limits with no extra features; Vello Hybrid 0.2 uses wgpu 29 and could not share
Curva's device. All are Apache-2.0 or MIT; `cargo-deny` is run when the dependency is added.

**Found on the way.** Building the plan's geometry took about 3 seconds (cutting solids and hidden
lines), Curva's own work, which live plans need faster whatever draws them. `Body::section` gives
wrong outlines for moved solids (#193), which drew the test's tree trunks as streaks in every renderer
and in the PDF.

**Before adoption, to check:** Vello sets aside working memory in advance; a very large sheet can
overflow it and lose parts of the drawing for that frame. The first production step tests a large plan
and makes Curva detect the overflow and enlarge the memory.

**Rejected.** Vello Hybrid (needed only for the old weak-card baseline, and not yet on wgpu 30);
drawing plans with Curva's own line pipeline (would have to match the PDF by hand).

**Revisit if** Vello's memory overflow cannot be handled reliably, or vello_cpu is too slow for real
sheets on weaker machines.
