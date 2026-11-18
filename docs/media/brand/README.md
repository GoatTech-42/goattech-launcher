# the goattech mark and icons

**the mark.** a flat-top peak, approved 2026-11-07: a mountain silhouette that also reads as an
a. drawn on whole units of a 16-unit grid, sharp at 16 px and clean at 1024. always monochrome -
white on black, black on white, never tinted. `docs/design-system.md` places it in the design
system.

the old horizon line mark (the sliced sun) and its violet belong to upstream. they do not appear
in goattech surfaces. `candidates/` holds the rejected horizon-era alternatives and is an
archive candidate, not a source of new art.

## masters (edit these)

[pending: the peak mark's masters land here when the app icon rollout happens. the mark itself
is approved; the export set below is the target shape, carried over from how the old set was
organised - confirm each against the real pipeline when it runs]

| file | what |
|---|---|
| `icon.svg` | the app icon: white peak on black, full square (windows, linux, favicons) |
| `icon-macos.svg` | the same on apple's 1024 grid (824 px tile, 185 px corners, transparent margin) |
| `avatar.svg` | full-bleed square for services that round or crop the image |
| `mark.svg`, `mark-dark.svg` | the mark alone, for light and for dark backgrounds |
| `social/card.html` | the github social preview, as a page |

## generated (don't edit)

`cargo run -p xtask -- icons` (needs `rsvg-convert` from librsvg) renders the masters into the
packaging icons, the website favicons and the preview pngs. [confirm the output table against
`packaging/` when the peak set is generated - paths named `horizon.*` belong to upstream's
exports and get goattech names in the same pass]

## the readme banner

the repo README carries no banner today. if one is added: a mono capture of the launcher or the
site, no version line, no counts - nothing that goes stale.

## uploading

the goattech accounts are the owner's. github avatar and social preview upload through the
repo and account settings pages with the generated pngs. there is no goattech discord server;
nothing brand-related lives there.
