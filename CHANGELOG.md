# Changelog

## 0.2.0
- Tools renamed: **ember** (lint), **forge** (format), **spark** (test), **ward** (deny); each ships as its own binary plus `blaze` subcommands.
- ember: token-level linter on a new shared lexer (no syntax tree); `[ember] ignore` config.
- forge: comments and blank lines are preserved and verified; per-content-hash cache for clean/dirty/unsupported results.
- spark: parallel test listing, opt-in `--cached` result cache.
- ward: `cargo metadata` cached by `Cargo.lock` hash.
- Reproducible benchmarks (`bench/run.py`), multi-page docs site, install script, release workflow, GitHub Action, pre-commit hooks.

## 0.1.0
- Initial workspace.
