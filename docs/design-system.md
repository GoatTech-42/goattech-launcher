# the goattech look

one design language for the launcher and the website. black and white. dark is the default and the identity: a drawn animated night sky (stars, clouds, sun, skylines), white and grey text, square controls with a chunky bottom edge, and the player's own 3d skin standing in the middle of home.

this page replaces the old horizon design system doc. the violet accent, the pixel sunset sky, the rounded cards, the partner banner and every other ad are gone.

## where it comes from

the owner plays in norisk and lunar and asked for their strengths: the square ui with its chunky edges, the wide launch button, and the player model in the middle of everything. the theme itself is goattech's own, not a clone. his standing notes:

- black and white. no color accent anywhere.
- real svg icons, never hand-drawn.
- quick, cool animations. not overdone.
- make it look a little less ai.

## colour

there is no accent hue. white is the accent: it fills the one main action on a screen and marks focus. everything else is neutral.

| token | value | use |
|---|---|---|
| `page` | `#0a0a0a` | the page background |
| `side` | `#0d0d0d` | the sidebar |
| `card` | `#131313` | cards, plates, fields |
| `card_hi` | `#1b1b1b` | a selected or raised plate, hover |
| `ink` | `#ffffff` | text, icons, the main action's face |
| `ink_soft` | `#b4b4b4` | secondary text, values |
| `muted` | `#8f8f8f` | labels, details, disabled |
| `accent` | `#ffffff` | the accent is white: it fills the one main action (black text on it) and marks focus |
| sky band | `#030303`..`#242424` | the drawn night sky's range |

values are the shipped night.rs set - the doc follows the code. states (ready, update available, error) read through brightness, a small square marker and words, never through a traffic-light palette. every text colour passes wcag aa (4.5:1) on every background it sits on; the theme tests check each pair.

## shape

- square corners. radius at most 4 px.
- every plate: a 2 px border a step above its face, the bottom edge 5-6 px thick. that heavy lip is the chunky edge - it is the look.
- the main action: white face, black text, a grey bottom lip. it sinks when pressed. one per screen.
- secondary actions: transparent face, a bordered edge, white text.
- disabled: 40 % opacity (`theme::DISABLED` = 0.4, shipped), nothing else changes.

## type

the app bundles two families (seen in the lead's 0.1.0-rc.1 mono captures): a pixel face for headings, buttons and the wordmark, and a grotesque for body text, with a mono for paths, codes and counts. [confirm the exact families and ofl files with the lead] the website uses the same voices so launcher and site read as one thing. [confirm: the site draft currently runs on system sans]

| token | px | weight | use |
|---|---|---|---|
| `caption` | 12 | 400 | footnotes, one-line states |
| `label` | 13 | 400 | row labels, nav, buttons |
| `ui` | 14 | 400 | body text |
| `title` | 20 | 700 | section heads |
| `display` | 28-44 | 700 | page titles, the selected instance's name |

two or three sizes per screen, not more. headings and buttons sit lowercase like everything else.

## icons

real svg stroke icons from one set (lucide): 24 unit grid, 2 px stroke, round caps, tinted with the text colour. one icon per feature or action, never decorative clusters. no emoji as icons, no hand-drawn glyphs. the pixel icon set from the horizon theme is retired with it.

## the sky and the player

- the background is the brand. home's default is the drawn animated night sky: stars, clouds, sun, skylines, all drawn in code, no photos, no gradient mesh. the player can set their own picture instead. the other pages sit on pure black with a sparse field of small white stars.
- home centres the player's own skin as a real 3d model: sharp at display resolution, facing the camera, idle poses, a wave, draggable to turn. nothing blurred, nothing pixelated, nothing malformed.
- the website renders the same sky and the same player. the launcher and the site should be hard to tell apart at a glance.

## motion

| token | duration | use |
|---|---|---|
| `quick` | 150 ms | hovers, presses |
| `move` | 240 ms | selection, page changes |
| `enter` | 400-600 ms | content arriving: a short fade and an 8 px rise |

eased out, never bouncy. reduced motion: nothing moves. the sky keeps still when the window is unfocused.

## copy

lowercase, short plain sentences. say what happens ("downloads 26.3 and java 21, about 600 mb"), never praise ("the best launcher"). no ai vocabulary, no exclamation marks. if a sentence could sit in a press release, rewrite it.

## the mark

the goattech mark is a flat-top peak, approved 2026-11-07: a mountain silhouette that also reads as an a. drawn on whole units of a 16 px grid so it stays sharp at 16 px and scales cleanly to 1024.

- always monochrome: white on black, black on white. no accent tint, ever.
- app icon: the white mark on a black square. [confirm the final export set with `docs/media/brand/README.md`]
- the old horizon line mark (the sliced sun) belongs to upstream. it does not appear in goattech surfaces.

## exceptions

- **sign in with microsoft** stays microsoft's own button, drawn exactly as microsoft publishes it (their brand guidelines require it). it is the only control that is not ours.
- mod loader logos keep their own colours when an instance needs them. the launcher ships the ones it may - `launcher/assets/logos/` carries neoforge.png and quilt.png - and fetches the rest.

## what's gone

- the violet accent and every coloured accent option's branding (the setting may still exist in the app; the theme's identity is mono).
- the pixel sunset sky, the dawn palette, the banded clouds.
- rounded cards, pills, gradients, shadows.
- the sculk partner banner and every other ad placement, in the launcher and on the site.
- the pixel icon set and pixel display type as the interface's voice.
