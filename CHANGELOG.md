# Changelog

## 0.2.1
- Website: one page per tool, no em dashes, capabilities instead of "replaces" wording.
- License is now Apache-2.0 only; third-party notices in THIRD_PARTY_NOTICES.md.
- New website (Vite/React/Tailwind): hero, replay race, charts, in-browser WASM playground running the real ember and forge.
- New logo family: Rust-style cog ring with rivets and per-tool glyphs.
- `blaze-lexer` split into its own crate; `blaze-wasm` crate for the playground.

## 0.2.0
- Tools renamed: **ember** (lint), **forge** (format), **spark** (test), **ward** (deny); each ships as its own binary plus `blaze` subcommands.
- ember: token-level linter on a new shared lexer (no syntax tree); `[ember] ignore` config.
- forge: comments and blank lines are preserved and verified; per-content-hash cache for clean/dirty/unsupported results.
- spark: parallel test listing, opt-in `--cached` result cache.
- ward: `cargo metadata` cached by `Cargo.lock` hash.
- Reproducible benchmarks (`bench/run.py`), multi-page docs site, install script, release workflow, GitHub Action, pre-commit hooks.

## 0.1.0
- Initial workspace.
