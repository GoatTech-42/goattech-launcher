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

Redeploy after edits (needs a Cloudflare API token with Pages edit on the
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
