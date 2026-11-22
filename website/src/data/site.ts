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
				bullets: ['Curves, surfaces, solids, mesh and SubD', "Rhino's command names, gumball and snaps", 'A model with memory and constraints'],
				points: [
					{ short: 'Muscle memory', title: 'Intuitive professional 3D modelling. Keep your muscle memory.', text: 'Rhino\'s command names, command line, gumball and object snaps, so the shortcuts already in your hands keep working. Curves, surfaces, solids and booleans, at a professional level.' },
					{ short: 'A model that remembers', title: 'A 3D model with memory and constraints.', text: 'Objects remember how they were made: parameters, relationships, constraints and live booleans. Edit a curve and the loft follows. Bake to plain geometry whenever you want to sculpt freely.' },
					{ short: 'NURBS, mesh and SubD', title: 'A full 3D suite for NURBS, meshes and SubD.', text: 'Mesh primitives, booleans and repair, QuadRemesh, and SubD primitives with creases. Convert between mesh, SubD and NURBS.' },
					{ short: 'Model to sheet', title: 'From model to sheet.', text: 'Layouts with detail views, Make2D, dimensions, text and hatches. Print, or export PDF, DXF and SVG.' },
					{ short: 'Your CAD files open', title: 'Your professional CAD files open in Curva.', text: 'Rhino .3dm in and out; SketchUp, DXF and mesh files (STL, OBJ, PLY) in; STEP, DXF and PDF out. IFC, glTF and USD are planned.' },
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
					{ short: 'Smart elements', title: 'Smart building elements, easy and intuitive.', text: 'Levels, walls, slabs, openings, doors, windows, roofs and stairs that know what they are. A window knows its wall; a wall knows its level.' },
					{ short: 'Elements that respond', title: 'Elements that respond: 3D, plan and section, at every LOD.', text: 'Change a door\'s width and its 3D model, plan symbol and section all follow. At 1:50 it shows its leaf and swing; at 1:200, a simpler symbol. One set of parameters drives every view.' },
					{ short: 'Live plans and sections', title: 'Live plans, sections and elevations.', text: 'Cut once. Poché, hidden lines and line weights come from the model and stay current, on sheets ready to print.' },
					{ short: 'Quick and snappy', title: 'Drawing that is quick and snappy.', text: 'Sketch walls and rooms in plan, with clean graphics by default. The plan isn\'t a separate drawing: what you draw builds the model.' },
					{ short: 'IFC compatible', title: 'Industry-standard compatibility with IFC.', text: 'Exchange models with other BIM tools through IFC. A built-in library comes too: wall and door styles, windows, trees and people.' },
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
					{ short: 'Rules to massing', title: 'Responsive massing from rules and setbacks.', text: 'Plots, setbacks, height limits and floor-area ratios generate the volumes. Change a rule and the massing responds.' },
					{ short: 'Real context', title: 'Import the real context.', text: 'Bring in OpenStreetMap, terrain and GIS data, so every study sits on the actual site.' },
					{ short: 'Live data, both ways', title: 'Instant data from your model, and back.', text: 'Area and density tables update as the massing changes. Edit a number in a table and the model changes to match.' },
					{ short: 'Streets to cities', title: 'Generate streets, blocks and cities parametrically.', text: 'Lay out streets, divide them into blocks and grow whole districts from a few parameters. Change a street width or a block size and the city rebuilds.' },
					{ short: 'Street sections', title: 'Set your street section, watch it being generated.', text: 'Draw the section once: pavements, kerbs, lanes and trees. Curva builds the street from it along any path, and rebuilds it when you change a width.' },
				],
			},
			analysis: {
				name: 'Analysis',
				status: 'Later',
				live: false,
				line: 'Ask the model questions.',
				lede: 'Sun and shadow first, then wind and views. Run studies on the model you are designing, without exporting it anywhere.',
				bullets: ['Sun and shadow studies', 'Wind and view studies', 'Your model responds to the results'],
				points: [
					{ short: 'Sun and shadow', title: 'Sun and shadow.', text: 'Shadows through the day and the year, and hours of sun on facades and open space.' },
					{ short: 'Wind', title: 'Understand your design\'s impact on wind.', text: 'See how wind moves around and between your buildings, and where it speeds up or stalls.' },
					{ short: 'Views', title: 'Study the views.', text: 'See what can be seen from a window, a street or a square, and what your design blocks.' },
					{ short: 'Carbon', title: 'Carbon calculation straight from your model.', text: 'Embodied carbon worked out from the materials and quantities already in your model, and updated as you design.' },
					{ short: 'Analysis drives design', title: 'Your model responds to the analysis.', text: 'Results feed back into the design: parameters respond to sun, wind and views, and every study updates when the model changes.' },
				],
			},
		},
	},
};

/** The address of a page: '' is the home page. */
export function href(_lang: Lang, slug: Slug | '' = ''): string {
	return '/' + (slug ? slug + '/' : '');
}
