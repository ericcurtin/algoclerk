## What does this change?

<!-- A sentence or two. Link the issue it closes, e.g. "Closes #123". -->

## Type

- [ ] New level or level fix (`levels/`)
- [ ] Translation (`crates/algoclerk-core/lang/`)
- [ ] Bug fix
- [ ] Feature
- [ ] Docs / tooling

## Checklist

- [ ] `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` pass
- [ ] `cargo test --workspace` passes (for levels: `cargo test -p algoclerk-levels`)
- [ ] New Rust files start with `// SPDX-License-Identifier: Apache-2.0`
- [ ] No new dependency, or it is permissively licensed and `cargo deny check` passes
- [ ] No WebAssembly, web build or scripting-engine code
- [ ] For levels: all text is original (no text, puzzles or art copied from games or problem sites), the reference solution passes `algoclerk-cli validate`, and a hint ladder is included
- [ ] User-visible changes are noted in `CHANGELOG.md`
- [ ] Every commit is signed off (`git commit -s`, see CONTRIBUTING.md)
