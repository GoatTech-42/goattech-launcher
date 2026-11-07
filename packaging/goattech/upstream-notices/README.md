# Supplemental license texts

These are original text bytes, not generated license boilerplate.

`index.json` maps package name/version and the crate archive's recorded VCS commit to each text's SHA256 and source URL. The packager does not fetch anything. These are fixed, reviewed inputs, not a format for importing arbitrary remote indexes.

- `upstream-vcs` entries were retrieved from the owning GitHub repository at the exact commit in the published crate's `.cargo_vcs_info.json`. A matching crate version/commit is required before packaging.
- `distro-version-copyright` entries are full Debian copyright records for the exact package version, retrieved from Debian's package tracker or Ubuntu's changelog archive. They include attribution and MIT license text. They are distribution-maintainer evidence, not proof that the same text existed at the upstream VCS commit. The crate commit still guards which archive receives the record. The output index labels these as `distro-version-text-review-required`.

The pathfinder_geometry record offers MIT or Apache-2.0. Its MIT text is present; its Apache text is a reference to a Debian system path, not a bundled complete Apache license. The simd_helpers record contains the apparent upstream-year typo `2919`; the bytes are preserved, not silently corrected. These limitations remain for review.

Workspace crates use the repository's MIT license, which is already bundled at the archive root. The collector still marks their missing package-local text rather than guessing inherited-license scope.

Text collection is not legal clearance, proof of final linked inclusion, or publication approval. SPDX expressions, including AND obligations, are preserved. Distribution-specific records do not cancel any other obligation.
