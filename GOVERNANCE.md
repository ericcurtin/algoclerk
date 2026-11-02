# Governance

AlgoClerk uses a lightweight, transparent model that can grow with the community.

## Roles

- **Users and contributors**: anyone who opens issues, discussions or pull requests. Everyone is
  covered by the [Code of Conduct](CODE_OF_CONDUCT.md).
- **Reviewers**: contributors with a track record in one area (levels, translations, platforms). They
  can approve pull requests in their area (listed in `.github/CODEOWNERS`).
- **Maintainers**: merge pull requests, cut releases, moderate, and keep the project healthy.
- **Founder / project owner**: the person who publishes the repository holds the maintainer role
  until others join and can nominate further maintainers.

## Path to maintainer

1. Contribute regularly for a few months: levels, reviews, fixes, translations, triage.
2. A maintainer nominates you in a public issue (or you ask). Two maintainers must agree and nobody
   should object within one week.
3. You get commit rights, a CODEOWNERS entry and the responsibility to uphold this document.
   Inactive maintainers (12 months) become emeritus; they can come back at any time.

## Decisions

- Day-to-day decisions: pull request review, lazy consensus (one maintainer approval; two for changes
  to `algoclerk-core`'s public behaviour).
- Larger or hard-to-reverse choices (toolkit, licence, file formats, dependencies with policy impact)
  are written down as an Architecture Decision Record in `docs/adr/` and discussed in a public issue
  for at least one week.
- Non-negotiables unless a new ADR explicitly supersedes them: Apache-2.0 and permissive dependencies,
  no WebAssembly, player programs run in the built-in interpreter, original assets and texts.

## Releases

Maintainers tag `vX.Y.Z` on `main` after CI is green; the release workflow builds all platforms.
Versions follow semantic versioning for the level file format and the CLI; `CHANGELOG.md` follows
Keep a Changelog.

## Contact

The maintainers must list a contact address for conduct and security reports here when the
repository is published:  `<contact address to be filled in by the project owner>`.
