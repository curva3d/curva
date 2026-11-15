# 0025: The application is GPL, the libraries stay MIT OR Apache-2.0

**Status:** Decided by the owner in chat · 15 November 2026. Replaces decision 0002.

**What was decided.**
- **The Curva application is licensed GPL-3.0-or-later** (`LICENSE`): anyone who distributes a
  modified Curva must publish their changes under the same licence. "Or later" lets Curva take up a
  future GPL version, as Blender, GIMP and Inkscape do.
- **With a plugin exception** (`PLUGIN-EXCEPTION.md`): plugins that talk to Curva only through the
  official Plugin API may use any licence, including closed, paid ones. This uses GPLv3's own
  mechanism for extra permissions (section 7).
- **The reusable foundations stay MIT OR Apache-2.0** (`LICENSE-MIT`, `LICENSE-APACHE`), so other
  Rust projects can use them freely and contribute back.

| Crate | Licence | Why |
|---|---|---|
| `curva-app` | GPL-3.0-or-later | The program itself |
| `curva-commands` | GPL-3.0-or-later | The commands and how they behave: Curva's core experience |
| `curva-viewport` | GPL-3.0-or-later | Curva's display |
| `curva-mcp` | GPL-3.0-or-later | Curva's link to AI agents |
| `curva-kernel` | MIT OR Apache-2.0 | The geometry interface |
| `curva-document` | MIT OR Apache-2.0 | Objects, undo and files; the file crates below need it |
| `curva-3dm`, `curva-dxf`, `curva-pdf` | MIT OR Apache-2.0 | File formats other programs can reuse |
| `evaluation/*` | MIT OR Apache-2.0 | Test tools, not part of the product |

New crates: a piece of the product (interface, commands, specializations) is GPL; a reusable
foundation (geometry, file formats, the future Plugin API) is MIT OR Apache-2.0. A permissive crate
must never depend on a GPL one.

**Why.**
- **No closed forks.** Under MIT/Apache a company could take Curva, close it and sell it. The GPL
  keeps Curva and every improvement to it open, as it has for Blender.
- **Plugins stay open to business.** Architecture relies on commercial plugins (renderers,
  toolsets). The exception keeps that possible without opening Curva itself.
- **Patents.** GPLv3 gives a patent licence from every contributor and withdraws the licence from
  anyone who sues users over patents. CAD is a field full of patents.
- **Nothing that 0002 relied on is lost.** Permissive code (solidcore, SolidCraft, Serpentine3D,
  every MIT/Apache library) can still flow into Curva, since Apache-2.0 and MIT combine with GPLv3.
  The libraries still flow out. The open-core plan is unaffected: Pro sells content and services,
  not a closed version of the code. GPLv3 also allows AGPL libraries, which 0002 had ruled out.
- **Done now** because the owner is so far the only contributor, so relicensing needs no one
  else's agreement.

**Rejected.**
- GPLv2: cannot be combined with Apache-2.0 code.
- GPL-3.0-only: would lock Curva out of a future GPL version.
- AGPL-3.0: only adds protection against closed online versions, and many companies refuse it.
- LGPL-3.0 or MPL-2.0 for the application: weaker than needed; the plugin exception covers the one
  thing they would add.
- GPL for every crate: would put off the Rust developers most likely to use and improve the
  foundations.

**Later.** If Curva ever wants to sell commercial licences of the application (dual licensing),
contributors would need to sign a contributor licence agreement first. Decide that before outside
contributors join. Have a lawyer review the exception's wording once a company plans a
serious commercial plugin.

**Would make us revisit:** a plugin maker or partner unable to work under the exception, or the
Plugin API design needing a different boundary than "solely through the Plugin API".
