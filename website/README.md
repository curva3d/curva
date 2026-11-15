# Curva website

The public website, with the home page and the documentation as one site, built with [Starlight](https://starlight.astro.build) (Astro). Only the website uses Node; the app stays pure Rust.

| Path | What |
|---|---|
| `public/index.html`, `public/pt/index.html` | The home page, in English and European Portuguese. Plain HTML, copied as it is. |
| `public/assets/` | The home page's CSS and script, the logos, icons and Inter (licence in `fonts/OFL.txt`). |
| `src/content/docs/docs/` | The documentation, one Markdown (`.md` / `.mdx`) file per page, served under `/docs/`. |
| `src/content/docs/pt/docs/` | Portuguese translations, added page by page. Until a page is translated, the Portuguese side shows the English one with a note. |
| `src/styles/curva.css` | Starlight in Curva's colours and type, from the brand guidelines. |
| `astro.config.mjs` | The sidebar, languages, logo and links. A new page goes into `sidebar` there. |
| `public/_headers` | Security and caching headers for Cloudflare Pages. |

Writing pages: plain language, short sentences, the user's words, as in the brand guidelines' voice. Describe the latest release, and say when something changes in the next one. When you change the home page in one language, change the other.

## Working on it

```sh
cd website
npm install
npm run dev      # http://localhost:4321, reloads as you edit
npm run build    # the finished site in website/dist
```

## Hosting on Cloudflare Pages

The owner connects it once, in the Cloudflare dashboard:

1. Workers & Pages → Create → Pages → Connect to Git → choose `curva3d/curva`.
2. Production branch: `main`. Framework preset: **Astro**. Root directory: `website`. Build command: `npm run build`. Build output directory: `dist`.
3. Under Environment variables, add `NODE_VERSION` = `22`.
4. Save and deploy. The site appears at `https://<project-name>.pages.dev`, and every pull request gets its own preview link.

After that, every merge to `main` publishes the site. A custom domain can be added later under the project's Custom domains tab. Then set `site` in `astro.config.mjs` to the address (it turns on the sitemap), and make `canonical`, the `hreflang` links and `og:image` in the home pages full addresses (`https://…`).
