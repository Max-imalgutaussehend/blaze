# Blaze

Fast, unified Rust tooling in the spirit of Astral's uv/ruff: one binary, parallel by default, content-hash caching.

| Command | Replaces | State |
|---|---|---|
| `blaze lint` | clippy | 6 syntactic rules (`blaze rules`); no type-aware lints |
| `blaze fmt [--check]` | rustfmt | syn + prettyplease; **skips files containing `//` or `/* */` comments** (they would be deleted) |
| `blaze test [filter]` | cargo-nextest | one process per test, shared thread pool |
| `blaze deny` | cargo-deny | licenses (SPDX expressions), bans, duplicates, sources; **no advisory DB yet** |
| `blaze <anything else>` | cargo | forwarded to cargo (no resolver/build rewrite) |

`blaze check` = `fmt --check` + `lint` + `deny`. Suppress a lint with `// blaze:ignore` on the line.
Policy lives in `blaze.toml` under `[deny]`.

## Honest benchmark
2000 random comment-free files from `~/.cargo/registry` (Apple silicon, release build):

| | time |
|---|---|
| `rustfmt --check` (xargs -P 8) | 39.6 s |
| `blaze fmt --check` (cold cache) | 1.1 s |

Caveats: rustfmt also follows `mod` children and implements the real rustfmt style; Blaze's output
uses prettyplease's style (e.g. no blank lines between items). Not a like-for-like comparison.
Full 27.6k-file corpus (522 MB): `fmt --check` 64 s cold, `lint` 57 s.

## Roadmap (what is *not* done)
Comment-preserving formatter, rustfmt-compatible style, type-aware lints, RustSec advisories,
a real dependency resolver/build orchestrator replacing cargo.

## Build
`cargo build --release` → `target/release/blaze`
