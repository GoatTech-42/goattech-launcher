# Synthetic window-state probes

This extends the existing window survey without changing app source. The two
Home source pins remain the survey's existing base/branch pins. Re-pin both
explicitly when reviewing a later app stack. The patch does not dispatch CI.

## Reachability audit

| State | Existing survey | Added route |
|---|---|---|
| Account popup | Closed Home/sidebar only | Synthetic offline names plus calibrated input sequence |
| Version popup | Closed Home only | Synthetic installed version plus calibrated input sequence |
| Consent | Incidental first-run frame, then uncertain click/Escape | Dedicated fresh unanswered profile; captures before any input |
| Screenshot viewer | No screenshots fixture or viewer steps | Synthetic PNG and screenshots tab plus calibrated thumbnail click |
| Export panel | Closed, below Home hero | Calibrated scroll and Export click |
| Launch options | Closed, below Home hero | Calibrated scroll and Launch click |
| Offline Skin | No kept offline names | Synthetic offline names, Settings page, calibrated scroll and Skin click |
| Import toast | No drop fixture or import action | Not added: needs a separately reviewed OS drop driver or app preview seam |
| Update Downloading/Ready | Disabled self-update; no preview seam | Blocked and excluded. Never enable self-update to capture it |

## Running a probe

Choose `state` in manual dispatch. Only the existing base-home and branch-home
jobs run for a probe; `survey` keeps the original page matrix. All homes remain
fresh, synthetic and offline. No Microsoft accounts, credentials, runnable game,
external images or real user data are added. Screenshot, trash, clipboard and
desktop paths remain inside scratch storage.

The optional `actions` input is a JSON array of at most 16 steps. Inputs are
passed as data, not evaluated as PowerShell. Each step has a short `n` name and
one of `c: [x,y]`, `s: [x,y,wheelDelta]`, or `k: key`. `w` is a 1-5 second wait.
Click/scroll coordinates are OUTER-window pixels after resizing to a measured
1028x780 client, not logical coordinates. Key values are restricted to Tab,
Shift-Tab, Enter, Escape, arrows and Page Up/Down. No arbitrary SendKeys strings.

Leave actions empty for the calibration run. Inspect `probe-fixture` and its
measured client size and DPI, then supply the coordinates of the intended
controls from those actual pixels. There are intentionally no guessed presets:
clicking an unverified coordinate can press Play, sign-in, or another action.
For a long panel, scroll first and retain that frame, then calibrate its action.
For consent details or screenshot delete confirmation, provide another bounded
input step after inspecting the initial open state.

Each input step retains a post-action frame. Final frames capture the retained
state at 1028x780 and 2560x1440. Opening or persistence is NOT asserted by the
harness. Inspect pixels to establish the popup, panel or viewer actually opened,
remained open after resize and contains the intended fixture. Source pins can
have different geometry; calibrate both sides before relying on a shared input
sequence. If they differ, run separate probes or use a route valid on both.

## Limits

No native Windows run has been performed by this patch's author. YAML parsing
and source-level checks are not proof of PowerShell execution or visible state.
The initial calibration frames are the next validation step. Existing frame
size/export ceilings and time bounds are unchanged. Extra frames may exceed the
existing encoded export budget; skipped frames are explicitly logged. Keep
probes short and split long interactions across runs rather than raising limits.
