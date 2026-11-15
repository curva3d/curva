# 0025: An RTX 3060-class graphics card is recommended, with a fallback for weaker machines

**Status:** Decided by the owner in chat · 15 November 2026.

**What was decided.**
- Curva's **recommended** graphics card is an NVIDIA GeForce RTX 3060, or an AMD or Intel card of the
  same class (for example AMD Radeon RX 6600 / 6600 XT or RX 7600, Intel Arc A750). Curva is designed,
  tuned and measured for that class.
- Below it Curva **still runs**: every feature that needs a strong card has a fallback that draws the
  same thing more slowly or at lower quality, chosen automatically. Nothing a user makes looks different
  on paper because of their graphics card.
- This replaces the earlier rule that everything runs well on 2020 Intel integrated graphics
  (`display-pipeline.md` §5 and decision A in its §9; `views-and-styles.md`). Those machines are now the
  fallback case, not the design target.

**What changes in practice.**
- Speed targets are set on the RTX 3060 itself, at 1920×1080, without the tenfold margin that stood in
  for integrated graphics.
- Strong-card features may be used by default when present (compute shaders, larger limits, full-quality
  shadows and ambient occlusion), each with a fallback below the recommended class.
- For 2D views and sheets the Vello evaluation (#185) recommends full GPU Vello, with vello_cpu as the
  fallback; Vello Hybrid is no longer needed for the weak-card case.
- Steps 14 and 15 of the display plan stay shelved (decision 0024): this decision changes what Curva may
  use, not what is worth building.
- CI has no graphics card and keeps testing with software graphics, which is where the fallbacks are
  checked.

**AMD and Intel cards.** Curva draws through wgpu (Vulkan or DirectX 12 on Windows and Linux, Metal on
macOS), which AMD and Intel support fully, so a card of the same class should perform about the same.
This is an expectation, not a measurement: only an RTX 3060 is available to measure on. Numbers from
anyone with an AMD or Intel card of that class are welcome in `docs/performance.md`. Driver differences
do show up (Windows' DirectX 12 shader compiler refused a shader on 15 November 2026 that Vulkan
accepted), so shaders keep being checked on DirectX 12 as well as Vulkan.

**Why.** Architectural and urban models are large, and the owner wants Curva to be fast and good-looking
on the machines architects buy for this work. Designing for 2020 integrated graphics made every feature
harder and held back the ones that need a strong card, such as full GPU Vello for drawings. Keeping a
fallback means laptops and older machines still work.

**Who it leaves behind.** Most laptops with built-in graphics, and base Apple M-series Macs, are below
the recommended class. They run Curva on the fallbacks: slower with large models, and some effects at
lower quality.

**Rejected.** A hard requirement with no fallback: simplest to build, but it would exclude most laptops
and many Macs. Keeping the 2020 Intel baseline: safest for laptops, but it holds back features and makes
each one harder to build.

**Revisit if** users below the recommended class report Curva unusable, or measurements on an AMD or
Intel card of the class differ markedly from the RTX 3060.
