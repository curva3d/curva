// The line drawing on each home-page plate: what the specialization is about, in a few strokes.
// Drawn in a 200 x 200 box; site.css (.motif) places and animates them.
import type { Slug } from './site';

export const MOTIFS: Record<Slug, string> = {
	// Freeform: a curve through its control points
	freeform: `<svg class="motif" viewBox="0 0 200 200" aria-hidden="true"><path class="thin" d="M20 150 L60 24 L112 186 L180 50"/><path pathLength="1" class="d" d="M20 150 C60 24 112 186 180 50"/><rect class="pt p1" x="16" y="146" width="8" height="8"/><rect class="pt p2" x="56" y="20" width="8" height="8"/><rect class="pt p3" x="108" y="182" width="8" height="8"/><rect class="pt p4" x="176" y="46" width="8" height="8"/></svg>`,
	// Architecture: a plan with a door that swings
	architecture: `<svg class="motif" viewBox="0 0 200 200" aria-hidden="true"><path pathLength="1" class="d" d="M84 164 H30 V40 H170 V164 H116"/><path pathLength="1" class="d d2" d="M104 40 V104 H138"/><path class="thin" d="M116 164 A32 32 0 0 0 84 132"/><line class="leaf" x1="84" y1="164" x2="116" y2="164"/></svg>`,
	// Urban: plots filling a block, one after another
	urban: `<svg class="motif" viewBox="0 0 200 200" aria-hidden="true"><path class="thin" d="M10 100 H190 M100 20 V180"/><rect pathLength="1" class="d" x="26" y="34" width="60" height="52"/><rect pathLength="1" class="d d2" x="114" y="34" width="38" height="52"/><rect pathLength="1" class="d d3" x="160" y="34" width="22" height="30"/><rect pathLength="1" class="d d4" x="26" y="114" width="34" height="56"/><rect pathLength="1" class="d d5" x="68" y="134" width="18" height="36"/><rect pathLength="1" class="d d6" x="114" y="114" width="68" height="56"/></svg>`,
	// Analysis: the sun travelling its arc, a pole and its shadow
	analysis: `<svg class="motif" viewBox="0 0 200 200" aria-hidden="true"><path class="thin" d="M10 150 H190"/><path pathLength="1" class="d" d="M24 150 A76 76 0 0 1 176 150"/><g class="sun"><circle cx="100" cy="74" r="9"/><path class="thin" d="M100 56 V50 M100 98 V92 M82 74 H76 M124 74 H118"/></g><path d="M100 150 V118"/><path class="shade" d="M100 150 H150"/></svg>`,
};
