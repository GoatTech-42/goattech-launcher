# Inherited Horizon workflows

These files are retained unchanged as upstream references, outside GitHub's active
`.github/workflows` directory. They include automatic release trains and named
self-hosted runners belonging to upstream. They are not the GoatTech CI policy.

GoatTech's active checks run on GitHub-hosted Linux and Windows with read-only
repository permissions. They run source tests, not game-play qualification. They
have no release, deployment, artifact upload, cache upload or signing step.

Do not copy a workflow back into the active directory without reviewing its
triggers, runners, permissions, credentials and effects. Enabling repository
Actions is safe only after the active workflow set has been checked. Publication
still needs the independent working-app review required by the project.
