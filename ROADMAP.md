# goattech launcher roadmap

what's being worked on, in plain words. no dates: things are now, next or later. this page is only about the launcher - the engine's own roadmap is upstream's and lives at [docs/engine/ROADMAP.md](docs/engine/ROADMAP.md).

## now

- **qualify the gui on real machines.** the cli and the library and binary test suites pass, and the gui has rendered on linux (offline traversal, reviewed frames). windows is not qualified: microsoft sign-in, game download, launch, input and sustained play still have to run on real hardware.
- **independent review before any release.** nothing publishes - no installers, no tags, no site launch - until an independent review of the actual running app scores it above 9/10.

## next

- the first public preview: signed installers, github releases, and real download buttons on [launcher.goattech.us](https://launcher.goattech.us).
- forge and neoforge java instances carried through a signed-in game launch (their installers already run; the launch itself is unqualified).
- mac testing, which has not run at all.

## later

- keeping pace with upstream horizon as it follows each minecraft version.
- modpacks, servers and shared worlds as first-class citizens of the launcher.
