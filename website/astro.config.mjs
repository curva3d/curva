// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

// The UI kit's files carry a stamp of their content, so the docs never load an older copy (as Page.astro does).
const stamp = (/** @type {string} */ f) => `/assets/${f}?v=` + createHash('sha256').update(readFileSync(`public/assets/${f}`)).digest('hex').slice(0, 10);

// One site: the home page and the specialization pages are Astro pages in src/pages/ (English
// only for now), their words in src/data/site.ts; the docs are Starlight, under /docs/.
export default defineConfig({
	site: 'https://curva3d.com',
	integrations: [
		starlight({
			title: 'Curva',
			description: 'Documentation for Curva, the open-source 3D suite for architects and designers.',
			logo: {
				light: './src/assets/curva-mark-black.png',
				dark: './src/assets/curva-mark-white.png',
				alt: '',
			},
			favicon: '/assets/favicon.svg',
			head: [
				{ tag: 'link', attrs: { rel: 'stylesheet', href: stamp('kit.css') } },
				{ tag: 'link', attrs: { rel: 'icon', href: '/assets/favicon-32.png', sizes: '32x32', type: 'image/png' } },
				{ tag: 'link', attrs: { rel: 'apple-touch-icon', href: '/assets/apple-touch-icon.png' } },
				{ tag: 'script', attrs: { src: stamp('lean.js'), defer: true } },
			],
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/curva3d/curva' }],
			editLink: { baseUrl: 'https://github.com/curva3d/curva/edit/main/website/' },
			customCss: ['./src/styles/curva.css'],
			sidebar: [
				{ label: 'Welcome', link: '/docs/' },
				{
					label: 'Getting started',
					items: [
						'docs/getting-started/install',
						'docs/getting-started/interface',
						'docs/getting-started/first-model',
						'docs/getting-started/from-rhino',
					],
				},
			],
		}),
	],
});
