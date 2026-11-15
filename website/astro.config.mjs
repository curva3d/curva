// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// One site: the home page lives in public/ (index.html, pt/index.html) and the docs under /docs/.
export default defineConfig({
	site: 'https://curva3d.com',
	integrations: [
		starlight({
			title: 'Curva',
			description: 'Documentation for Curva, the open-source 3D suite for architects and designers.',
			logo: {
				light: './src/assets/curva-lockup-light.svg',
				dark: './src/assets/curva-lockup-dark.svg',
				replacesTitle: true,
			},
			favicon: '/assets/favicon.svg',
			head: [{ tag: 'link', attrs: { rel: 'icon', href: '/assets/favicon-32.png', sizes: '32x32', type: 'image/png' } }],
			// English now; Portuguese pages go in src/content/docs/pt/ as they are translated.
			// Until then the Portuguese side shows the English page with a note.
			defaultLocale: 'root',
			locales: {
				root: { label: 'English', lang: 'en' },
				pt: { label: 'Português', lang: 'pt-PT' },
			},
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/curva3d/curva' }],
			editLink: { baseUrl: 'https://github.com/curva3d/curva/edit/main/website/' },
			customCss: ['./src/styles/curva.css'],
			sidebar: [
				{ label: 'Welcome', translations: { pt: 'Bem-vindo' }, link: '/docs/' },
				{
					label: 'Getting started',
					translations: { pt: 'Primeiros passos' },
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
