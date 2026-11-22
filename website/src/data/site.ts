// The words on the home page and the four specialization pages. English only for now; to add a
// language, add it to Lang, give it a block in WORDS and pages under src/pages/<lang>/.

export type Lang = 'en';
export const SLUGS = ['freeform', 'architecture', 'urban', 'analysis'] as const;
export type Slug = (typeof SLUGS)[number];

export interface Point { short: string; title: string; text: string }
export interface Spec { name: string; status: string; live: boolean; line: string; lede: string; bullets: string[]; points: Point[] }

export interface Words {
	htmlLang: string;
	title: string;
	description: string;
	ogDescription: string;
	skip: string;
	home: string;
	motto: string;
	intro: string;
	hintHome: string;
	hintSpec: string; // {n} plates
	back: string;
	notYet: string;
	plate: string;
	shot: string;
	docs: string;
	download: string;
	downloadFor: string; // {os}
	latest: string;
	free: string;
	system: string;
	explore: string; // {name}
	others: string;
	theme: string;
	dark: string;
	light: string;
	foot: string;
	releases: string;
	notes: { mac: string; win: string; linux: string };
	specs: Record<Slug, Spec>;
}

export const WORDS: Record<Lang, Words> = {
	en: {
		htmlLang: 'en',
		title: 'Curva · Open-source 3D for architects and designers',
		description:
			'Curva is an open-source 3D suite for architects and designers: freeform modelling with objects that remember how they were made, and plans, sections and sheets generated live from the model.',
		ogDescription: 'Every project starts with a curve. Open-source 3D for architects and designers.',
		skip: 'Skip to content',
		home: 'Curva, home',
		motto: 'Every project starts with a curve.',
		intro: 'Four specializations, stacked on one model. Freeform is the base; the others build on it.',
		hintHome: 'Exploded axonometric · Move up to pull it apart · Click a plate to read on',
		hintSpec: '{n} plates · Move up to pull them apart · Click one to read on',
		back: '← Back to the stack',
		notYet: 'Not in Curva yet',
		plate: 'Plate',
		shot: 'Screenshot to come',
		docs: 'Docs',
		download: 'Download',
		downloadFor: 'Download for {os}',
		latest: 'Latest release on GitHub',
		free: 'Free',
		system: 'System',
		explore: 'Explore {name} →',
		others: 'The specializations',
		theme: 'Switch between light and dark',
		dark: 'Dark',
		light: 'Light',
		foot: 'Free and open source, under the GPL-3.0 licence.',
		releases: 'All releases',
		notes: { mac: 'Apple silicon', win: '64-bit installer', linux: '64-bit AppImage' },
		specs: {
			freeform: {
				name: 'Freeform',
				status: 'Out now',
				live: true,
				line: 'Model anything.',
				lede: 'Rhino-level modelling as the base of everything. Curves, surfaces, solids, meshes and SubD, with the commands and habits you already have.',
				bullets: ['Curves, surfaces, solids, mesh and SubD', "Rhino's command names, gumball and snaps", 'History that stays: edit the curve, the loft follows'],
				points: [
					{ short: 'Model like you do', title: 'Model like you already do', text: "Curves, surfaces, solids and booleans, with Rhino's command names, command line, gumball and object snaps. If you know Rhino, you know where things are." },
					{ short: 'History that stays', title: 'History that stays', text: 'A loft remembers its curves; a solid remembers its profile. Edit the input and the result follows. Bake any object to plain geometry when you want to sculpt freely.' },
					{ short: 'Mesh and SubD', title: 'Mesh and SubD', text: 'Mesh primitives, booleans and repair, QuadRemesh, and SubD primitives with creases. Convert between mesh, SubD and NURBS.' },
					{ short: 'Model to sheet', title: 'From model to sheet', text: 'Layouts with detail views, Make2D, dimensions, text and hatches. Print, or export PDF, DXF and SVG.' },
					{ short: 'Your files open', title: 'Your files open', text: 'Reads and writes Rhino .3dm, imports SketchUp, exports STEP. IFC, glTF and USD are planned.' },
				],
			},
			architecture: {
				name: 'Architecture',
				status: 'Coming next',
				live: false,
				line: 'Buildings that draw themselves.',
				lede: 'Walls, slabs, doors and windows that know what they are. Draw in plan and the model follows; change the model and every plan, section and elevation updates.',
				bullets: ['Walls, slabs, doors, windows, roofs, stairs', 'Smart objects with their own plan symbols', 'Live plans, sections and elevations'],
				points: [
					{ short: 'Elements with types', title: 'Building elements with types', text: 'Levels, walls, slabs, openings, doors, windows, roofs and stairs. A window knows its wall; a wall knows its level.' },
					{ short: 'Objects draw themselves', title: 'Objects that draw themselves', text: 'A door shows its leaf and swing in plan, its true model in elevation, and a simpler symbol at 1:200. One set of parameters drives them all.' },
					{ short: 'Live plans and sections', title: 'Live plans, sections and elevations', text: 'Cut once. Poché, hidden lines and line weights come from the model and stay current, on sheets ready to print.' },
					{ short: 'Quick plan drawing', title: 'Plan drawing that feels quick', text: 'Sketch walls and rooms in 2D with clean graphics by default. The plan is not a separate drawing: it is the model.' },
					{ short: 'IFC and a library', title: 'IFC and a built-in library', text: 'Exchange with other BIM tools through IFC. Wall and door styles, windows, trees and people come included.' },
				],
			},
			urban: {
				name: 'Urban',
				status: 'Planned',
				live: false,
				line: 'Test a city block before lunch.',
				lede: 'Massing from rules, on real sites. Set plots, setbacks, heights and density, and see the numbers change as you move things.',
				bullets: ['Massing from plots, setbacks and heights', 'OpenStreetMap, terrain and GIS import', 'Area and density tables, live'],
				points: [
					{ short: 'Massing from rules', title: 'Massing from rules', text: 'Plots, setbacks, height limits and floor-area ratios generate the volumes. Change a rule and the block rebuilds.' },
					{ short: 'Real context', title: 'Real context', text: 'Bring in OpenStreetMap, terrain and GIS data, so studies sit on the actual site.' },
					{ short: 'Numbers beside the model', title: 'Numbers beside the model', text: 'Area and density tables update with the massing: gross floor area, coverage, floor counts.' },
					{ short: 'Visual scripting', title: 'Visual scripting', text: 'Build your own rules by wiring nodes, shared with every specialization.' },
				],
			},
			analysis: {
				name: 'Analysis',
				status: 'Later',
				live: false,
				line: 'Ask the model questions.',
				lede: 'Sun and shadow first, then wind and views. Run studies on the model you are designing, without exporting it anywhere.',
				bullets: ['Sun and shadow studies', 'Wind and view studies later', 'Runs on the live model'],
				points: [
					{ short: 'Sun and shadow', title: 'Sun and shadow', text: 'Shadows through the day and the year, and hours of sun on facades and open space.' },
					{ short: 'Wind and views, later', title: 'Wind and views, later', text: 'Further studies follow once sun and shadow are solid.' },
					{ short: 'On the live model', title: 'On the live model', text: 'Because analysis reads the same objects you design with, a study updates when the design does.' },
				],
			},
		},
	},
};

/** The address of a page: '' is the home page. */
export function href(_lang: Lang, slug: Slug | '' = ''): string {
	return '/' + (slug ? slug + '/' : '');
}
