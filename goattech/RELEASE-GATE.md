# GoatTech Launcher publication gate (standing, from Luke, 2026-10-30)

"Goattech launcher has to be the best and reviewed as over a 9/10 by an
independent unbiased review agent before any version publishes."

The quote above is the requirement, in Luke's words. Everything below it
is the review method built on it, not his words; the concrete criterion
list and its weights are not user-approved yet and must be fixed with
him before any review runs.

## What is gated

ANY version publication: a GitHub Release (which the launcher's update
channel reads), a website download, a tag meant for users, or any other
distribution of a build. Source commits and pushes to the project branch
are NOT publication: they continue under the existing finite per-commit
review protocol. This gate adds to that protocol; it does not replace it.

## The bar

- Strictly OVER 9/10 - 9.0 itself does not pass.
- Scored on the actual working app: every claimed screen, flow and
  interaction pixel-verified in a real run. Source, diffs, check and test
  output alone cannot earn the score; a build whose GUI is unverified
  cannot receive or pass a product review.
- Fixed, transparent criteria, published before the review runs, with the
  review's evidence (captures, runs, flows exercised) attached to its
  verdict. The bar is not adjusted to make a release fit.

## The reviewer

Independent and unbiased: not the implementer, not the per-commit source
reviewer, and no self-rating. The review agent gets the fixed criteria and
the built app, and returns a scored verdict with its evidence.

## Current status

GUI qualification is blocked (no verified presentation route), so no
product score is possible today and NO version may publish. The order is:
working-app qualification -> independent review scoring over 9/10 ->
Luke's publication decision. The launcher updater only ever sees a
release after this gate has passed.
