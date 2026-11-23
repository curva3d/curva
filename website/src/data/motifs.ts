// The line drawings on the plates: on the home page, what each specialization is about; on a
// specialization page, each of its strong points. A few strokes each.
// Drawn in a 200 x 200 box. Each one's viewBox is cropped to what it reaches over its whole animation,
// so all of them fill their frame the same way, centred; --lw scales the line weights to match.
// site.css (.motif) places and animates them.
import type { Slug } from './site';

export const MOTIFS: Record<Slug, string> = {
	// Freeform: a curve through its control points
	freeform: `<svg class="motif" viewBox="4.8 9.8 190.4 190.4" style="--lw:0.952" aria-hidden="true"><path class="thin" d="M20 150 L60 24 L112 186 L180 50"/><path pathLength="1" class="d" d="M20 150 C60 24 112 186 180 50"/><rect class="pt p1" x="16" y="146" width="8" height="8"/><rect class="pt p2" x="56" y="20" width="8" height="8"/><rect class="pt p3" x="108" y="182" width="8" height="8"/><rect class="pt p4" x="176" y="46" width="8" height="8"/></svg>`,
	// Architecture: a plan with a door that swings
	architecture: `<svg class="motif" viewBox="21.6 23.6 156.8 156.8" style="--lw:0.784" aria-hidden="true"><path pathLength="1" class="d" d="M84 164 H30 V40 H170 V164 H116"/><path pathLength="1" class="d d2" d="M104 40 V104 H138"/><mask id="swing-home" maskUnits="userSpaceOnUse" x="0" y="0" width="200" height="200"><path class="swingm" pathLength="1" d="M116 164 A32 32 0 0 0 84 132"/></mask><path class="thin" mask="url(#swing-home)" d="M116 164 A32 32 0 0 0 84 132"/><line class="leaf" x1="84" y1="164" x2="116" y2="164"/></svg>`,
	// Urban: plots filling a block, one after another
	urban: `<svg class="motif" viewBox="-0.8 -0.8 201.6 201.6" style="--lw:1.008" aria-hidden="true"><path class="thin" d="M10 100 H190 M100 20 V180"/><rect pathLength="1" class="d" x="26" y="34" width="60" height="52"/><rect pathLength="1" class="d d2" x="114" y="34" width="38" height="52"/><rect pathLength="1" class="d d3" x="160" y="34" width="22" height="30"/><rect pathLength="1" class="d d4" x="26" y="114" width="34" height="56"/><rect pathLength="1" class="d d5" x="68" y="134" width="18" height="36"/><rect pathLength="1" class="d d6" x="114" y="114" width="68" height="56"/></svg>`,
	// Analysis: the sun travelling its arc, a pole and its shadow
	analysis: `<svg class="motif" viewBox="-14.4 -13.4 228.8 228.8" style="--lw:1.144" aria-hidden="true"><path class="thin" d="M10 150 H190"/><path pathLength="1" class="d" d="M24 150 A76 76 0 0 1 176 150"/><g class="sun"><circle cx="100" cy="74" r="9"/><path class="thin" d="M100 56 V50 M100 98 V92 M82 74 H76 M124 74 H118"/></g><path d="M100 150 V118"/><path class="shade" d="M100 150 H150"/></svg>`,
};

// One per strong point, in the same order as the points in site.ts.
export const POINT_MOTIFS: Record<Slug, string[]> = {
	freeform: [
		// Intuitive professional 3D modelling. Keep your muscle memory
		`<svg class="motif" viewBox="47.2 46.2 108.6 108.6" style="--lw:0.543" aria-hidden="true"><rect class="pt p1" x="56" y="136" width="10" height="10"/><path pathLength="1" class="d" d="M61 141 H150 M138 133 L150 141 L138 149"/><path pathLength="1" class="d d2" d="M61 141 V52 M53 64 L61 52 L69 64"/><path pathLength="1" class="d d3" d="M61 141 L122 92 M108 94 L122 92 L119 106"/><path class="thin" d="M61 86 A55 55 0 0 1 116 141"/></svg>`,
		// A 3D model with memory and constraints
		`<svg class="motif" viewBox="14.9 16.9 170.2 170.2" style="--lw:0.851" aria-hidden="true"><path pathLength="1" class="d" d="M24 60 C70 20 120 90 176 44"/><path pathLength="1" class="d d2" d="M24 150 C70 176 126 120 176 156"/><path class="thin" d="M24 60 V150 M62 47 V160 M100 55 V148 M138 62 V140 M176 44 V156"/><rect class="pt p2" x="96" y="51" width="8" height="8"/><rect class="pt p3" x="96" y="144" width="8" height="8"/></svg>`,
		// A full 3D suite for NURBS, meshes and SubD
		`<svg class="motif" viewBox="28.3 28.3 143.4 143.4" style="--lw:0.717" aria-hidden="true"><path class="thin" d="M36 36 H164 V164 H36 Z M36 100 H164 M100 36 V164"/><path pathLength="1" class="d" d="M100 46 C140 46 154 60 154 100 C154 140 140 154 100 154 C60 154 46 140 46 100 C46 60 60 46 100 46 Z"/><path pathLength="1" class="d d2" d="M46 100 C70 112 130 112 154 100 M100 46 C88 70 88 130 100 154"/></svg>`,
		// From model to sheet
		`<svg class="motif" viewBox="21.6 21.6 156.8 156.8" style="--lw:0.784" aria-hidden="true"><path pathLength="1" class="d" d="M30 34 H170 V166 H30 Z"/><path pathLength="1" class="d d2" d="M44 48 H112 V112 H44 Z"/><path pathLength="1" class="d d3" d="M120 140 H170 M120 140 V166"/><path class="thin" d="M44 128 H112 M44 122 V134 M112 122 V134"/><path pathLength="1" class="d d4" d="M58 98 L78 62 L98 98 Z"/></svg>`,
		// Your professional CAD files open in Curva
		`<svg class="motif" viewBox="10.2 8.2 183.7 183.7" style="--lw:0.918" aria-hidden="true"><path pathLength="1" class="d" d="M56 30 H122 L148 56 V170 H56 Z"/><path pathLength="1" class="d d2" d="M122 30 V56 H148"/><path pathLength="1" class="d d3" d="M20 100 H92 M80 90 L92 100 L80 110"/><path pathLength="1" class="d d4" d="M112 132 H184 M172 122 L184 132 L172 142"/></svg>`,
	],
	architecture: [
		// Smart building elements, easy and intuitive
		`<svg class="motif" viewBox="-0.7 13.9 173.3 173.3" style="--lw:0.867" aria-hidden="true"><circle class="pin pin1" cx="20" cy="95" r="3.4"/><g class="wall" transform="matrix(0.8667 0.5 0 1 20 95)"><path vector-effect="non-scaling-stroke" d="M0 0 V-60 L11.54 -71.77"/><g class="grow"><path vector-effect="non-scaling-stroke" d="M0 0 H150 M0 -60 H150"/></g><g class="grow"><path vector-effect="non-scaling-stroke" d="M11.54 -71.77 H161.54"/></g><g class="cap"><path vector-effect="non-scaling-stroke" d="M0 0 V-60 L11.54 -71.77 V-11.77 Z"/></g></g><g class="curA" transform="matrix(0.8667 0.5 0 1 20 95)"><g class="cap"><g transform="matrix(1.1538 -0.5769 0 1 0 0)"><path vector-effect="non-scaling-stroke" d="M0 -8 V8 M-8 0 H8"/></g></g></g><g class="cursor"><path d="M20 87 V103 M12 95 H28"/></g><circle class="pin pin2" cx="150" cy="170" r="3.4"/><g class="drop"><path class="solid" d="M75 127 V85 L101 100 V142 Z"/><path d="M95 117 V123"/></g></svg>`,
		// Elements that respond: 3D, plan and section, at every LOD
		`<svg class="motif" viewBox="1.4 0.9 189.3 189.3" style="--lw:0.946" aria-hidden="true"><path pathLength="1" class="d" d="M22 180 H76 M108 180 H132"/><mask id="swing" maskUnits="userSpaceOnUse" x="0" y="0" width="200" height="200"><path class="swingm" pathLength="1" d="M108 180 A32 32 0 0 0 76 148"/></mask><path class="thin" mask="url(#swing)" d="M108 180 A32 32 0 0 0 76 148"/><line class="leaf leafb" x1="76" y1="180" x2="108" y2="180"/><path pathLength="1" class="d d2" d="M96 99.9 L72 86 V16 L162 68 V138 L126 117.2 M72 16 L80 11 L170 63 L162 68 M170 63 V133 L162 138"/><path pathLength="1" class="d d3" d="M96 99.9 V47.9 L126 65.2 V117.2"/><g class="leaf3"><path class="solid" d="M96 99.9 V47.9 L126 65.2 V117.2 Z"/><path d="M120 88 V95"/></g></svg>`,
		// Live plans, sections and elevations
		`<svg class="motif" viewBox="3.7 3.7 192.6 192.6" style="--lw:0.963" aria-hidden="true"><path pathLength="1" class="d" d="M40 40 H160 V160 H40 Z M100 40 V110 H160"/><g class="sweep"><path class="thin" d="M20 70 H180"/><path d="M20 70 V58 M14 62 L20 54 L26 62 M180 70 V58 M174 62 L180 54 L186 62"/></g></svg>`,
		// Drawing that is quick and snappy
		`<svg class="motif" viewBox="21.6 23.6 156.8 156.8" style="--lw:0.784" aria-hidden="true"><path pathLength="1" class="d" d="M84 164 H30 V40 H170 V164 H116"/><path pathLength="1" class="d d2" d="M104 40 V104 H138"/><path pathLength="1" class="d d3" d="M30 104 H70"/><rect class="pt p3" x="134" y="100" width="8" height="8"/></svg>`,
		// Industry-standard compatibility with IFC
		`<svg class="motif" viewBox="13.1 24.1 165.8 165.8" style="--lw:0.829" aria-hidden="true"><circle pathLength="1" class="d" cx="62" cy="62" r="28"/><path class="thin" d="M62 34 V90 M34 62 H90 M42 42 L82 82 M82 42 L42 82"/><path pathLength="1" class="d d2" d="M122 122 L152 137 L122 152 L92 137 Z M92 137 V160 L122 175 V152 M152 137 V160 L122 175"/><path pathLength="1" class="d d3" d="M128 40 H170 V84 H128 Z M149 40 V84"/><path pathLength="1" class="d d4" d="M22 180 H70 M26 180 V174 H66 V180 M34 174 V134 M58 174 V134 M28 134 H64 V127 H28 Z M24 127 H68"/><path class="thin" d="M40 170 V138 M46 170 V138 M52 170 V138"/></svg>`,
	],
	urban: [
		// Responsive massing from rules and setbacks
		`<svg class="motif" viewBox="16.0 0.0 185.9 185.9" style="--lw:0.930" aria-hidden="true"><path class="thin" d="M26 40 H174 V164 H26 Z"/><path pathLength="1" class="d" d="M48 62 H152 V142 H48 Z"/><path pathLength="1" class="d d2" d="M26 28 H48 M26 22 V34 M48 22 V34"/><path pathLength="1" class="d d3" d="M186 142 V62 M180 72 L186 62 L192 72"/></svg>`,
		// Import the real context
		`<svg class="motif" viewBox="-0.8 -0.8 201.6 201.6" style="--lw:1.008" aria-hidden="true"><path pathLength="1" class="d" d="M10 60 C60 70 90 40 190 56"/><path pathLength="1" class="d d2" d="M10 140 C70 120 120 150 190 128"/><path pathLength="1" class="d d3" d="M56 10 C64 70 50 130 66 190"/><path pathLength="1" class="d d4" d="M140 10 C130 80 150 120 136 190"/><path class="thin" d="M74 78 H124 V112 H74 Z"/><circle class="pt p4" cx="99" cy="95" r="5"/></svg>`,
		// Instant data from your model, and back
		`<svg class="motif" viewBox="14.6 15.6 174.7 174.7" style="--lw:0.874" aria-hidden="true"><path class="thin" d="M24 166 H180"/><path class="bar b1" d="M34 166 V120 H56 V166"/><path class="bar b2" d="M68 166 V84 H90 V166"/><path class="bar b3" d="M102 166 V104 H124 V166"/><path class="bar b4" d="M136 166 V52 H158 V166"/><path class="thin" d="M24 40 H80 M24 54 H64"/></svg>`,
		// Generate streets, blocks and cities parametrically
		`<svg class="motif" viewBox="-0.8 -0.8 201.6 201.6" style="--lw:1.008" aria-hidden="true"><path pathLength="1" class="d" d="M10 66 H190 M10 136 H190"/><path pathLength="1" class="d d2" d="M66 10 V190 M136 10 V190"/><path pathLength="1" class="d d3" d="M78 22 H124 V54 H78 Z M148 78 H180 V124 H148 Z"/><path pathLength="1" class="d d4" d="M22 78 H54 V124 H22 Z M78 148 H124 V180 H78 Z"/><path pathLength="1" class="d d5" d="M22 22 H54 V54 H22 Z M148 148 H180 V180 H148 Z"/><path class="thin" d="M101 78 V124 M78 101 H124"/></svg>`,
		// Set your street section, watch it being generated
		`<svg class="motif" viewBox="-0.8 7.2 201.6 201.6" style="--lw:1.008" aria-hidden="true"><path pathLength="1" class="d" d="M10 64 H44 V70 H156 V64 H190"/><path pathLength="1" class="d" d="M26 64 V48 M174 64 V48"/><circle pathLength="1" class="d" cx="26" cy="40" r="8"/><circle pathLength="1" class="d" cx="174" cy="40" r="8"/><path class="thin" d="M10 82 H190 M10 78 V86 M44 78 V86 M100 78 V86 M156 78 V86 M190 78 V86"/><path pathLength="1" class="d d2" d="M10 108 H190 M10 184 H190"/><path pathLength="1" class="d d3" d="M10 122 H190 M10 170 H190"/><path class="thin" d="M10 146 H190"/><circle class="pt p2" cx="40" cy="115" r="4"/><circle class="pt p3" cx="100" cy="115" r="4"/><circle class="pt p4" cx="160" cy="115" r="4"/><circle class="pt p2" cx="70" cy="177" r="4"/><circle class="pt p3" cx="130" cy="177" r="4"/></svg>`,
	],
	analysis: [
		// Sun and shadow
		`<svg class="motif" viewBox="-14.4 -13.4 228.8 228.8" style="--lw:1.144" aria-hidden="true"><path class="thin" d="M10 150 H190"/><path pathLength="1" class="d" d="M24 150 A76 76 0 0 1 176 150"/><g class="sun"><circle cx="100" cy="74" r="9"/><path class="thin" d="M100 56 V50 M100 98 V92 M82 74 H76 M124 74 H118"/></g><path d="M100 150 V118"/><path class="shade" d="M100 150 H150"/></svg>`,
		// Understand your design's impact on wind
		`<svg class="motif" viewBox="-0.8 2.2 201.6 201.6" style="--lw:1.008" aria-hidden="true"><path pathLength="1" class="d" d="M84 78 H122 V128 H84 Z"/><path class="flow" d="M10 60 C60 60 70 44 104 44 C140 44 150 60 190 60"/><path class="flow f2" d="M10 104 H70"/><path class="flow f3" d="M10 148 C60 148 70 162 104 162 C140 162 150 148 190 148"/><path class="flow f2" d="M136 104 H190"/></svg>`,
		// Study the views
		`<svg class="motif" viewBox="12.5 2.4 195.3 195.3" style="--lw:0.977" aria-hidden="true"><circle pathLength="1" class="d" cx="30" cy="100" r="7"/><g class="scan"><path class="thin" d="M38 96 L190 40 M38 104 L190 160"/></g><path pathLength="1" class="d d2" d="M110 80 H140 V118 H110 Z"/><path class="thin" d="M140 82 L190 62 M140 116 L190 136"/><path pathLength="1" class="d d3" d="M164 60 V140"/></svg>`,
		// Carbon calculation straight from your model
		`<svg class="motif" viewBox="14.6 22.6 174.7 174.7" style="--lw:0.874" aria-hidden="true"><path pathLength="1" class="d" d="M24 150 V80 L64 100 V170 Z M64 100 L94 85 V155 L64 170 M24 80 L54 65 L94 85"/><path class="thin" d="M24 103 L64 123 L94 108 M24 126 L64 146 L94 131 M100 118 H114"/><text x="120" y="62">CO₂</text><rect class="hbar" x="120" y="74" width="60" height="14"/><rect class="hbar hb2" x="120" y="96" width="38" height="14"/><rect class="hbar hb3" x="120" y="118" width="50" height="14"/><rect class="hbar hb4" x="120" y="140" width="24" height="14"/></svg>`,
		// Your model responds to the analysis
		`<svg class="motif" viewBox="1.3 2.7 196.5 196.5" style="--lw:0.983" aria-hidden="true"><path pathLength="1" class="d" d="M100 44 L158 72 L100 100 L42 72 Z M42 72 V130 L100 158 V100 M158 72 V130 L100 158"/><path pathLength="1" class="d d2" d="M22 150 A84 84 0 0 1 60 34 M48 30 L61 34 L56 47"/><path pathLength="1" class="d d3" d="M178 54 A84 84 0 0 1 140 168 M152 172 L139 168 L144 155"/></svg>`,
	],
};

/** The same drawing, for the large frame in a plate's section further down the page: its mask ids
 *  get their own names so they don't clash with the plate's copy. */
export function fig(svg: string): string {
	return svg.replace(/id="([\w-]+)"/g, 'id="$1-fig"').replace(/url\(#([\w-]+)\)/g, 'url(#$1-fig)');
}
