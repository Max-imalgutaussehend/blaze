# Blaze

Extremely fast Rust tooling: one toolchain, parallel by default, content-hash caches.
Docs and benchmarks: https://max-imalgutaussehend.github.io/blaze/

| Tool | Role | Compared with |
|---|---|---|
| `ember` | linter (token-level, 6 rules) | clippy |
| `forge` | formatter (comment-preserving, verified) | rustfmt |
| `spark` | test runner (process per test, optional result cache) | cargo-nextest |
| `ward` | dependency policy (licenses, bans, duplicates, sources) | cargo-deny |
| `blaze` | umbrella; unknown subcommands go to cargo | cargo |

```sh
curl -LsSf https://github.com/Max-imalgutaussehend/blaze/releases/latest/download/install.sh | sh
blaze check
```

## Measured results
See the [benchmarks page](https://max-imalgutaussehend.github.io/blaze/benchmarks/). Reproduce with
`cargo build --release && python3 bench/run.py <corpus> <crate>`. Headline caveats:
ember is far smaller than clippy, forge's style is not rustfmt's, and cache-based speedups
describe repeat runs.

## Not done
Type-aware lints, autofix, rustfmt-compatible style, RustSec advisories, a resolver/build
orchestrator replacing cargo, doc-tests in spark.

License: MIT or Apache-2.0.
