# 0015: How a `.curva` file is stored

**Status:** Proposed · waiting for the owner's choice · 12 November 2026

**The question:** the architecture (section 8, Container) asks for two ways of storing a `.curva`
file to be measured in Milestone 1 before one is chosen: **redb**, a small embedded database, and a
**zip-style package**. Today a `.curva` file is one JSON text, so that was measured too.

**How it was measured:** the Milestone 1 test scene, 5,000 circles and 500 solid cylinders, saved
and opened three times in each format on the owner's PC, keeping the fastest run. The measuring
program was kept outside Curva, so neither library was added to Curva for it. Opening here means
reading the file; meshing the solids for display afterwards takes about 3.5 seconds whatever the
format.

| Format | Save | Open | Size | A copy cut short by 10%, as after a crash |
|---|---|---|---|---|
| JSON text (today) | 26 ms | 17 ms | 12.0 MB | refused |
| zip package (the same JSON, compressed) | 26 ms | 22 ms | 0.9 MB | refused |
| redb database (one row per object) | 149 ms | 33 ms | 32.1 MB | refused |

**What it shows:**
- All three are fast enough: under a sixth of a second to save or read this scene.
- The zip package is **13 times smaller** than today's file, at the same speed.
- redb is the slowest and the biggest here. Its strength, saving only what changed, only pays off for
  much larger models than these.
- A file cut short is refused by all three, so none of them protects against a crash on its own.
  That protection is now in Curva itself: since step 4, a save is written beside the file and then
  renamed, so the old file stays whole until the new one is complete.

**Recommendation: the zip package.** It is as fast as today's file and far smaller. It is a standard
format that any zip tool can open, which helps if a file ever needs rescuing. It also leaves room to
add more parts beside the model later: a preview picture, the smart object definitions of
Milestone 3, or materials. Older JSON files would keep opening. The `zip` library is MIT-licensed
and pure Rust.

**What was rejected, and why:** redb, for now. It was slower and larger on this scene, and its
advantage only shows once models get much larger. JSON text alone works, but makes files 13 times
larger for no gain.

**What would make us revisit it:** models so large that saving the whole file each time becomes
slow, which is when saving only what changed (redb's strength) starts to matter.

**For the owner:** choose the zip package (recommended), stay with JSON text for now, or choose redb.
Adding the `zip` library is a new dependency, so it needs your approval as well.
