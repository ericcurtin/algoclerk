# AlgoClerk progress log

Running log of milestones, decisions and verification status. Updated at every commit.

## Milestones

- [x] M1: workspace, licence, core crate (ISA, assembler, VM, levels, judge, codegen, i18n, progress)
- [ ] M2: levels (40+), validator tests, oracles
- [ ] M3: codegen compile-tests
- [ ] M4: CLI polish, GUI toolkit ADR and app
- [ ] M5: community docs, CI, packaging
- [ ] M6: cross-compilation checks, final report

## Decisions

- Rust 1.95 (installed). `CARGO_TARGET_DIR=/home/ecurtin/git/algoclerk-run/target` for all cargo commands.
- Levels are TOML (`toml` crate, MIT/Apache-2.0), one file per level under `levels/<pack>/`.
- Own SplitMix64 PRNG so generated tests are stable across platforms and crate versions.

## Verification matrix

| Platform | Status |
| --- | --- |
| Linux | in progress |
| Windows | not yet verified |
| macOS | not yet verified |
| Android | not yet verified |
| iOS | not yet verified |
