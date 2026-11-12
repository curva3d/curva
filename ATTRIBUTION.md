# Attribution

Curva builds on the work of others. Anything ported, translated or reused is credited here,
with its original copyright and licence notice.

## Serpentine3D

Curva's Freeform command list, names, aliases, prompts, options and interaction flows are based on
Serpentine3D: https://github.com/chisomobanzi/Serpentine3D

The icons on Curva's command buttons (`crates/curva-app/src/icons.rs`) are Serpentine3D's toolbar icons
(`serpentine3d/ui/icons.py`), redrawn in Rust with the same shapes, in Curva's colours.

> MIT License
>
> Copyright (c) 2026 Chisomo Banzi
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

## solidcore

Curva calls solidcore (Apache-2.0) for its geometry: https://github.com/tangency-labs/solidcore
Its licence and NOTICE ship with its source at the pinned revision in `Cargo.lock`. A copy of the
NOTICE is in `licenses/solidcore-NOTICE.txt`, and goes with every release build.

## Inter

The interface's text is set in Inter 4.1, by The Inter Project Authors (https://github.com/rsms/inter),
bundled from its official release as `crates/curva-app/fonts/Inter-Regular.ttf`
under the SIL Open Font License 1.1, whose text is in `licenses/fonts/Inter-OFL.txt`.

## Fonts bundled by egui

The Curva window uses the default fonts in `epaint_default_fonts` 0.37.0, part of egui. They are
compiled into the program, so these notices must ship with any distributed copy of Curva. The full
texts are in `licenses/fonts/`:

- `OFL.txt`: SIL Open Font License 1.1
- `UFL.txt`: Ubuntu Font Licence 1.0
- `Hack-Regular.txt`, `egui-icons.txt`, `emoji-icon-font-mit-license.txt`: the other bundled fonts

These licences cover only the font files, not Curva's code.

## To add when used

- SolidCraft (MIT / Apache-2.0)
- Geometry kernel(s) chosen in Milestone 0 (see `docs/decisions/0004-*`)
