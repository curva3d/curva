# Curva website

The public website, with the home page and the documentation as one site, built with [Starlight](https://starlight.astro.build) (Astro). Only the website uses Node; the app stays pure Rust.

| Path | What |
|---|---|
| `src/pages/`, `src/components/` | The home page and one page per specialization (Freeform, Architecture, Urban, Analysis). Each opens with an exploded axonometric stack of glass plates and scrolls on to a section per plate. English only for now; `public/_redirects` sends the old `/pt/` addresses to English. |
| `src/data/site.ts` | Every word on those pages. |
| `public/assets/` | Their CSS and script (`site.css`, `site.js`, and `theme.js` for light and dark: no inline code, for the Content-Security-Policy), the logo marks, icons, the social image, Inter and JetBrains Mono (licences in `fonts/`). |
| `src/content/docs/docs/` | The documentation, one Markdown (`.md` / `.mdx`) file per page, served under `/docs/`. |
| `src/styles/curva.css` | Starlight in Curva's colours and type, from the brand guidelines. |
| `astro.config.mjs` | The sidebar, languages, logo and links. A new page goes into `sidebar` there. |
| `public/_headers` | Security and caching headers for Cloudflare Pages. |

Writing pages: plain language, short sentences, the user's words, as in the brand guidelines' voice. Describe the latest release, and say when something changes in the next one. A new page outside the docs also needs its Content-Security-Policy line in `public/_headers`.

## Working on it

```sh
cd website
npm install
npm run dev      # http://localhost:4321, reloads as you edit
npm run build    # the finished site in website/dist
```

## Hosting on Cloudflare

The site is served by Cloudflare Workers as static files, at **https://curva3d.com**. `wrangler.jsonc` says what to upload (`dist/`) and how: missing pages show `404.html`, and addresses work with or without a trailing slash. Its `name`, `curva`, must match the Worker's name in the dashboard.

Set up once, in the Cloudflare dashboard:

1. Workers & Pages → Create application → Import a repository → choose `curva3d/curva`.
2. Project name `curva`. Build command `npm run build`. Deploy command `npx wrangler deploy`. Under Advanced settings, root directory `website`.
3. Deploy. Then, in the Worker's Settings → Domains & Routes, add the custom domains `curva3d.com` and `www.curva3d.com`.
4. Settings → Build → Build watch paths: include `website/*`, so commits to the app don't rebuild the site.
5. Settings → Build → Branch control: untick builds for non-production branches. Pull-request builds failed on every pull request (issue #166) and nobody used their preview links.

After that, every merge to `main` that touches `website/` publishes the site. Pull requests are not built by Cloudflare, so they show no Cloudflare check.
