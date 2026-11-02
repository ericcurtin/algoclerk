# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - unreleased

First public version.

### Added

- Core crate: instruction set, text assembler, deterministic sandboxed interpreter with step traces
  and rewind, judge with size and speed scoring and staple / gold-seal medals, deterministic test
  generators, level validator, solution sharing as text, local progress, translation catalogues.
- Equivalent-code reveal: translation of any valid program into C, C++ and Rust with control-flow
  recovery, optional per-level function signatures, syntax highlighting and compile-and-compare tests.
- 48 levels in 7 wings (Mail Room, Counting House, Number Cruncher, The Shelves, Sort & Search,
  Ledger Room, Executive Floor) with hint ladders, reference solutions and independent oracles.
- Game (egui): animated office, drag-and-drop and tap-to-edit program list with live two-way text
  sync, run / pause / step / rewind / speed, victory screen with code view, sandbox, light / dark /
  high-contrast themes, colour-blind palette, keyboard shortcuts, touch-sized controls, safe areas.
- Headless CLI (`algoclerk-cli`): list, show, run, judge, validate, tune, codegen, export, import.
- Headless software renderer for screenshots (`algoclerk --screenshot`).
- Packaging for Linux, macOS, Windows, Android and iOS; CI and release workflows.

[Unreleased]: https://github.com/algoclerk/algoclerk/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/algoclerk/algoclerk/releases/tag/v0.1.0
