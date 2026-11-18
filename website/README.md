# goattech launcher website

source for https://launcher.goattech.us - the public site for goattech launcher
(github.com/GoatTech-42/goattech-launcher, a fork of horizon-rust/horizon, MIT).

## contents

- `index.html` - the whole site. self-contained: css and js inline, no build step, no
  dependencies. icons are inline svg (lucide-style stroke icons). the star field is drawn at
  runtime (seeded, stable between loads). [confirm the font choice: bundled bricolage like the
  app, or the system stack the current draft uses]
- `assets/concepts/*.webp` - app screens shown in the concepts section. they are concept
  renders with fictional data, not shipped ui - keep the "concept - not shipped ui" label.
  (still accurate for the mono ui; only replace them when a newer concept pass lands.)

## the look

black and white, matching the launcher's mono theme: pure black sky, sparse white stars, white and grey text,
square controls with a chunky bottom edge, the flat-top peak mark. the full token list and rules
live in [../docs/design-system.md](../docs/design-system.md). no color accent anywhere, no ads,
no tracking pixels, no discord links.

## the player

the hero player is a box player drawn in css/svg on the page - not a pulled skin. the v1
direction is a real 3d model of the player's own skin: sharp at display resolution, facing the
camera, idle poses, a wave, draggable to turn. nothing blurred or pixelated.

motion stays quick and quiet. prefers-reduced-motion gets a calm page; touch devices skip
pointer effects.

## content rules (please keep)

- no download button, no installers, no release claims until they actually exist. the button
  stays visibly disabled with "in testing now. the download goes live when v1 passes review."
  until release day. [flip at release]
- the status section ("where it is right now") is dated and states what works and what does
  not. update the date and the lists when the real state changes.
- concept screens stay labeled "concept - not shipped ui" until they are current captures.
- no benchmarks, parity claims, or invented stats. link only the repo, its LICENSE, upstream
  horizon, and goattech.us.
- keep the fork credit to horizon (MIT) and the goattech attribution in the footer.

## deploy

hosted on cloudflare pages, project `goattech-launcher`, on the existing goattech cloudflare
account (same convention as goattech-obsidian -> obsidian.goattech.us). custom domain
`launcher.goattech.us` is attached to the project, with a proxied cname
`launcher.goattech.us` -> `goattech-launcher.pages.dev` in the goattech.us zone.

redeploy after edits - the .assetsignore in this folder keeps this README and other docs out of
the upload, and 404.html is served with a real 404 status for unmatched paths (keep it; it gets
the same mono theme). needs a cloudflare api token with pages edit on the account - use the one
in the vault, never commit it:

    export CLOUDFLARE_API_TOKEN=...   # from vault, do not hardcode
    export CLOUDFLARE_ACCOUNT_ID=...   # the goattech account id - from the pages dashboard or the vault, do not hardcode
    npx wrangler@4 pages deploy . --project-name goattech-launcher --branch main

or drag the folder onto the project in the pages dashboard. production is
https://launcher.goattech.us and https://goattech-launcher.pages.dev; every deploy also gets a
preview url like https://<id>.goattech-launcher.pages.dev.

if this folder gets committed into the repo (e.g. as `website/`), switching the pages project to
git integration is fine - it is a static dir, no build command, output dir is this folder.
