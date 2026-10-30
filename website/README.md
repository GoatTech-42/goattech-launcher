# goattech launcher website

Source for https://launcher.goattech.us - the public site for goattech launcher
(github.com/GoatTech-42/goattech-launcher, a fork of horizon-rust/horizon, MIT).

## Contents

- `index.html` - the whole site. Self-contained: CSS and JS are inline, no build
  step, no dependencies. Fonts (Geist, Geist Mono, Bricolage Grotesque) are
  loaded from Google Fonts (css2 API); the pixel night-sky hero is generated at runtime
  by the inline script (canvas, seeded PRNG per viewport shape).
- `assets/concepts/*.webp` - renders of the 8 app UI concept screens, shown in
  the concepts section. They are concept designs with fictional data, not
  shipped UI, and the page labels them that way - keep the label if these are
  reused. Regenerated from the concept source with the real fonts at 1280x800.

## Deploy

Hosted on Cloudflare Pages, project `goattech-launcher`, on the existing
GoatTech Cloudflare account (same convention as goattech-obsidian ->
obsidian.goattech.us). Custom domain `launcher.goattech.us` is attached to the
project, with a proxied CNAME `launcher.goattech.us` ->
`goattech-launcher.pages.dev` in the goattech.us zone.

Redeploy after edits - the .assetsignore in this folder keeps this README and other docs out of the upload, and 404.html is served with a real 404 status for unmatched paths (keep it). Needs a Cloudflare API token with Pages edit on the
account - use the one in the vault, never commit it):

    export CLOUDFLARE_API_TOKEN=...   # from vault, do not hardcode
    export CLOUDFLARE_ACCOUNT_ID=59c7a3b570c83fd2fa1520fba6793941
    npx wrangler@4 pages deploy . --project-name goattech-launcher --branch main

Or drag the folder onto the project in the Pages dashboard. Production is
https://launcher.goattech.us and https://goattech-launcher.pages.dev; every
deploy also gets a preview URL like https://<id>.goattech-launcher.pages.dev.

If this folder gets committed into the repo (e.g. as `website/`), switching the
Pages project to git integration is fine - it is a static dir, no build command,
output dir is this folder.

## Content rules (please keep)

- No download button, no installers, no release claims until they actually
  exist. The hero says there is no download on purpose.
- The status section ("where it actually is") is dated and states what works
  and what does not. Update the date and the lists when the real state changes
  (GUI runs, releases ship, forge/neoforge land, fork syncs with upstream).
- Concept screens stay labeled "concept - not shipped ui".
- No benchmarks, parity claims, or invented stats. Link only the repo, its
  LICENSE, upstream horizon, and goattech.us.
- Keep the fork credit to horizon (MIT) and the goattech.us attribution in the
  footer.

## Motion + player model (2026-10-30 update)

- The hero is one continuous night scene: twinkling stars, drifting blocky
  clouds, meteor bursts, snow-capped spruces, the goat, fireflies, and the
  player - all layered so scroll and pointer move the world with depth, and the
  moon sets as you scroll away. Keep new effects inside this scene language;
  don't bolt on unrelated ones.
- The player is the owner's actual Minecraft skin (the owner's Mojang profile),
  embedded as a data URI and assembled part-by-part from the 64x64 skin texture
  (front view, with overlay layers). He idles (breathing, arm sway, head look),
  and jumps when clicked. The same skin appears in the home concept's "you"
  card. If his skin changes, re-pull from the Mojang session server for UUID
  profile and swap the data URI + the concept render. The UUID is recorded in the lead's notes, not here.
- prefers-reduced-motion gets a calm page (no drift, bob, showers, or reveals
  beyond simple fades); touch devices skip pointer parallax, tilt and magnetic
  buttons; the player and scroll hint are hidden on small screens.
- Gallery: idle drift (pauses on hover/drag), drag-to-scroll and tilt on
  desktop.
