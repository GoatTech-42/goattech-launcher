
## Renderer correction (account-skin-4337, 2026-11-03)

Scope: sharp display-resolution 3D player model on the Play stage, auto idle/wave
poses, drag-to-turn, seamless corner player heads, Discord entries removed
(sidebar, crash dialog, CLI), all 7 language tables updated.

Commits: c30afab7d (stage + heads + Discord removal + i18n), 6878933eb
(display-res rasterizer render_posed_hi, back/bottom faces, per-face shading),
b2ac13293 (hi-res stage origin scale fix), 3b1590c32 (sub-texel turn test),
cd6977c92 (revert ranged shot probe; keep single-shot dump).

Tests: lib 366 pass (stage-test4.log), bin 53/53 pass (stage-test5.log, includes
sub-texel turn test and shading tones test). GUI build BUILD_EXIT=0
(stage-gui5.log).

Actual running-app frames (headless Xvfb, test home, software GL):
- hi2-idle.png: front idle at display resolution (full frame + 3x/5x crops sent earlier)
- seq-c88.png idle sway, seq-c95.png wave with right arm raised beside head
- drag-diag.png ~110 deg side view after incremental drag
- drag-back.png ~180 deg back view after 260 px drag
- seq-wave.gif: wave cycle frames c40-c95
All inspected angles show connected joints (neck, shoulders, hips), solid
back/bottom faces, no detached or hollow parts. Turn runs t60-t300 in the first
multi-run script did not rotate (single 300 px jump did not register as drag);
incremental 20 px moves rotate reliably at 0.012 rad/px - captured via
drag-diag/drag-back instead.

Known caveat: ranged probe experiment (GOATTECH_SHOT_DIR) caused a swapchain
acquire panic and was fully reverted in cd6977c92; shipped probe is the
original single-shot GOATTECH_SHOT dump used by all captures above.
