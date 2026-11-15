# Curva website

The public website: one home page in English (`index.html`) and European Portuguese (`pt/index.html`). Plain HTML and CSS with no build step. Colours, type and the logo follow the Curva brand guidelines.

- `assets/site.css`: the look. The values match the brand tokens; light and dark follow the visitor's system.
- `assets/site.js`: fetches the latest release from GitHub, to show the version and link straight to each system's download. Without it, every download link goes to the Releases page.
- `assets/fonts/`: Inter, served from the site itself (licence in `OFL.txt`), so no font service sees visitors.
- `_headers`: security and caching headers for Cloudflare Pages.

When you change one language, change the other.

## Hosting on Cloudflare Pages

The owner connects it once, in the Cloudflare dashboard:

1. Workers & Pages → Create → Pages → Connect to Git → choose `curva3d/curva`.
2. Production branch: `main`. Framework preset: None. Build command: leave empty. Build output directory: `website`.
3. Save and deploy. The site appears at `https://<project-name>.pages.dev`; pull requests get their own preview links.

After that, every merge to `main` that touches `website/` publishes the site. A custom domain can be added later under the project's Custom domains tab.

Once the site has its final address, make `canonical`, the `hreflang` links and `og:image` in both pages full addresses (`https://…`), so search engines and link previews pick them up.
