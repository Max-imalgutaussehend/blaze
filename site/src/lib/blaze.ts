import results from '../../../bench/results.json'

export const R = results
export const VERSION = 'v0.3.0-alpha.1'
export const REPO = 'https://github.com/Max-imalgutaussehend/blaze'
export const INSTALL_SH = `curl -LsSf ${REPO}/releases/latest/download/install.sh | sh`
export const INSTALL_CARGO = `cargo install --git ${REPO} blaze`
export const BASE = import.meta.env.BASE_URL
export const logo = (n: string) => `${BASE}logos/${n}.svg`
export const link = (path: string) => `${BASE}${path}`

export const x = (a: number, b: number) => {
  const r = a / b
  return r >= 10 ? `${Math.round(r)}×` : `${r.toFixed(1)}×`
}
export const fmtT = (s: number) => (s < 1 ? `${(s * 1000).toFixed(s < 0.01 ? 1 : 0)} ms` : `${s.toFixed(2)} s`)

export type ToolId = 'blaze' | 'ember' | 'forge' | 'spark' | 'ward'

export interface ToolInfo {
  id: ToolId
  role: string
  /** Shown on the badge of overview cards. */
  badge: string
  tagline: string
  /** One sentence on the overview cards. */
  blurb: string
  headline: { value: string; label: string }
  can: { t: string; d: string }[]
  usage: string
  config: string
  how: string[]
  limits: string
  bench: { title: string; sub: string; rows: { name: string; incumbent: number; blaze: number }[]; note: string }
}

export const TOOLS: ToolInfo[] = [
  {
    id: 'blaze', role: 'cargo front-end', badge: 'build · run · add',
    tagline: 'Everyday cargo commands, without the wait.',
    blurb: 'build, check and run skip cargo when nothing changed. add and remove edit Cargo.toml directly.',
    headline: { value: x(R.build_cargo_noop, R.build_blaze_noop), label: 'faster than cargo build when nothing changed' },
    can: [
      { t: 'build, check, run', d: 'Skips cargo entirely when none of the files it tracks changed. Otherwise it runs cargo and remembers the inputs.' },
      { t: 'run starts your binary directly', d: 'No second cargo process: an up-to-date binary is executed straight away.' },
      { t: 'add and remove', d: 'Edit Cargo.toml in place, keep comments and sort order, and validate features against the crates.io index.' },
      { t: 'Cached index lookups', d: 'Versions come from the sparse index with an ETag cache, so repeat lookups are a tiny 304.' },
      { t: 'Everything else is cargo', d: 'new, init, test flags, publish and every other command are forwarded unchanged.' },
      { t: 'The whole toolchain', d: 'blaze ember, forge, spark and ward run the other tools. blaze ci runs the checks for CI and pre-commit.' },
    ],
    usage: 'blaze build                    # skips cargo when nothing changed\nblaze run -- --flag value      # build if needed, then exec the binary\nblaze check                    # type-check, same fast path\nblaze add serde -F derive      # edit Cargo.toml, resolve from the index\nblaze remove serde\nblaze ci                       # forge --check + ember + ward',
    config: '# No configuration. Set BLAZE_NO_FAST=1 to always go through cargo.',
    how: ['After a successful cargo run, blaze records every file that influenced it: rustc dep-info for your crates (this includes include_str! files), manifests, Cargo.lock, build-script rerun-if-changed inputs and environment variables. The next call just stats that list.',
      'If nothing changed and the outputs still exist, cargo is never started. Registry dependencies are treated as immutable for a given Cargo.lock, which is tracked. Any other change falls through to a normal cargo build.'],
    limits: 'Blaze does not replace cargo\'s resolver or compiler orchestration: real builds are cargo builds. The fast path only covers plain invocations (--release, --bin, -p, -q, --locked, --offline); other flags go to cargo. add leaves Cargo.lock to the next build and forwards -p, --git and --path to cargo add.',
    bench: {
      title: 'Commands with nothing to do', sub: 'This workspace (about 170 crates), already built. Lower is better, seconds.',
      rows: [
        { name: 'cargo build', incumbent: R.build_cargo_noop, blaze: 0 }, { name: 'blaze build', incumbent: 0, blaze: R.build_blaze_noop },
        { name: 'cargo check', incumbent: R.check_cargo_noop, blaze: 0 }, { name: 'blaze check', incumbent: 0, blaze: R.check_blaze_noop },
        { name: 'cargo run', incumbent: R.run_cargo_noop, blaze: 0 }, { name: 'blaze run', incumbent: 0, blaze: R.run_blaze_noop },
        { name: 'cargo add x3', incumbent: R.add_cargo_3, blaze: 0 }, { name: 'blaze add x3', incumbent: 0, blaze: R.add_blaze_3 },
      ],
      note: 'Cargo\'s no-op cost grows with the dependency graph; blaze only stats the recorded files. The add timings need network and a warm index cache for blaze.',
    },
  },
  {
    id: 'ember', role: 'linter', badge: 'blaze lint',
    tagline: 'Lint a whole crate in milliseconds.',
    blurb: 'Finds unwrap, dbg!, todo!, len() == 0 and more, straight from the token stream.',
    headline: { value: x(R.clippy_cold, R.lint_ember_crate), label: 'faster than a clean clippy run (speed only, ember is far smaller)' },
    can: [
      { t: 'Six rules out of the box', d: 'unwrap, dbg!, todo!/unimplemented!, len() == 0, needless return and boolean comparisons.' },
      { t: 'No compile step', d: 'Works on a token stream, so it runs on code that does not build yet.' },
      { t: 'Parallel scanning', d: `${R.full_corpus_files.toLocaleString()} files (${R.full_corpus_mb} MB) in ${R.lint_ember_full_corpus.toFixed(1)} s, no cache.` },
      { t: 'Skips unchanged files', d: 'A stat check lets repeat runs skip files that were clean last time.' },
      { t: 'Inline suppression', d: 'Add // blaze:ignore to a line. Items under #[cfg(test)] are skipped.' },
      { t: 'Runs in the browser', d: 'The playground on this page runs the same code compiled to WebAssembly.' },
    ],
    usage: 'ember .                       # lint the current directory\nember src/ --ignore B001      # skip a rule for this run\nember --rules                 # list all rules',
    config: '[ember]\nignore = ["B006"]',
    how: ['Ember never builds a syntax tree. A hand-written lexer produces tokens, and each rule is a small pattern over that stream. Files are scanned in parallel, so throughput is limited by memory bandwidth, not parsing.',
      'The trade-off is that rules cannot use type information. Ember covers the cheap, high-signal checks you want on every save.'],
    limits: 'Six syntactic rules, no autofix, no type information. For example B001 also flags .unwrap() on types where it cannot fail. It compares speed, not coverage, with a type-aware linter like clippy.',
    bench: {
      title: 'Time to first diagnostics', sub: 'One crate (regex-syntax). Lower is better, seconds.',
      rows: [{ name: 'clippy (clean build)', incumbent: R.clippy_cold, blaze: 0 }, { name: 'clippy (no changes)', incumbent: R.clippy_warm, blaze: 0 }, { name: 'ember', incumbent: 0, blaze: R.lint_ember_crate }],
      note: 'Clippy type-checks your code and has hundreds of lints. Ember has six. This chart compares speed only.',
    },
  },
  {
    id: 'forge', role: 'formatter', badge: 'blaze fmt',
    tagline: 'Format Rust and keep every comment.',
    blurb: 'Re-attaches comments and blank lines, verifies the result, and skips files by stat.',
    headline: { value: x(R.fmt_rustfmt, R.fmt_forge_cold), label: `faster than rustfmt cold, ${x(R.fmt_rustfmt, R.fmt_forge_warm)} on a repeat run` },
    can: [
      { t: 'Keeps comments and blank lines', d: 'They are re-attached after formatting and verified token for token.' },
      { t: 'Refuses to guess', d: 'If it cannot prove the output is safe, the file is left untouched and reported.' },
      { t: 'Skips unchanged files by stat', d: 'Repeat runs check modification time and size, with a content-hash fallback.' },
      { t: 'Parallel, even inside big files', d: 'Large files are split at top-level items and parsed on all cores, largest first.' },
      { t: 'CI mode', d: 'forge --check exits 1 when anything would change.' },
      { t: 'Runs in the browser', d: 'Try it in the playground below, compiled to WebAssembly.' },
    ],
    usage: 'forge .             # format in place\nforge --check .     # exit 1 if anything would change',
    config: '# forge has no options yet.',
    how: ['Forge parses with syn and prints with prettyplease, then tokenizes the original and the formatted text with the same lexer, aligns the code tokens and places each comment next to the token it belonged to.',
      'Before writing, it checks that the code tokens equal the formatter output and the comments are identical and in order. Files over 192 KB are split at top-level item boundaries and formatted in parallel; the result is identical to formatting the whole file.'],
    limits: `The style follows prettyplease, not rustfmt, and rustfmt.toml is not read. Arguments inside macro calls are printed token by token, which can look odd (for example "| x |" in closures). In our ${R.files}-file sample about 1.4% of files could not be handled safely and about 0.2% changed again on a second pass.`,
    bench: {
      title: `Format ${R.files} files (--check)`, sub: 'Lower is better, seconds.',
      rows: [{ name: 'rustfmt (8 procs)', incumbent: R.fmt_rustfmt, blaze: 0 }, { name: 'forge (cold)', incumbent: 0, blaze: R.fmt_forge_cold }, { name: 'forge (repeat run)', incumbent: 0, blaze: R.fmt_forge_warm }],
      note: 'Not like-for-like: the style differs from rustfmt, and rustfmt also follows mod children.',
    },
  },
  {
    id: 'spark', role: 'test runner', badge: 'blaze test',
    tagline: 'Run every test in its own process, in parallel.',
    blurb: 'One worker pool across all test binaries, plus a shared mode and an opt-in result cache.',
    headline: { value: x(R.test_nextest, R.test_spark_cached), label: `faster than nextest with --cached, ${x(R.test_nextest, R.test_spark)} without` },
    can: [
      { t: 'Process per test', d: 'Each test is isolated, so shared state cannot leak between tests.' },
      { t: 'Shared mode', d: '--shared runs each binary once with libtest threads, like cargo test but without the cargo overhead.' },
      { t: 'One worker pool', d: 'Tests from every binary are scheduled together across all cores.' },
      { t: 'Result cache (opt-in)', d: '--cached skips tests that passed against a byte-identical binary.' },
      { t: 'Name filter', d: 'spark parse runs only tests whose name contains "parse".' },
      { t: 'Works with cargo test', d: 'Uses the standard test binaries, no changes to your tests.' },
    ],
    usage: 'spark                 # one process per test\nspark --shared        # one process per binary\nspark parse           # only tests whose name contains "parse"\nspark --cached        # skip tests proven unchanged',
    config: '# spark has no options yet.',
    how: ['Spark builds once with cargo test --no-run, lists the tests of every binary in parallel, then runs each test in its own process on a shared worker pool. Failures print their captured output at the end.',
      'With --cached, a test that passed against a byte-identical test binary (blake3 hash) is skipped. That is only sound for deterministic tests whose inputs live inside the binary, which is why it is opt-in.'],
    limits: 'No doc-tests, retries, JUnit output or per-test timeouts yet. Cached mode ignores files that tests read at runtime. Shared mode gives up per-test isolation.',
    bench: {
      title: 'Run 147 tests', sub: 'regex-syntax, binaries already built. Lower is better, seconds.',
      rows: [{ name: 'cargo test', incumbent: R.test_cargo_test, blaze: 0 }, { name: 'cargo nextest', incumbent: R.test_nextest, blaze: 0 }, { name: 'spark', incumbent: 0, blaze: R.test_spark }, { name: 'spark --shared', incumbent: 0, blaze: R.test_spark_shared }, { name: 'spark --cached', incumbent: 0, blaze: R.test_spark_cached }],
      note: 'Most of the remaining time is the tests themselves (debug build). Cached mode describes repeat runs of deterministic tests.',
    },
  },
  {
    id: 'ward', role: 'dependency policy', badge: 'blaze deny',
    tagline: 'Dependency policy in about ten milliseconds.',
    blurb: 'Licenses, bans, duplicate versions and sources, read straight from Cargo.lock.',
    headline: { value: x(R.deny_cargo_deny, R.deny_ward_cold), label: 'faster than cargo deny, with no cache' },
    can: [
      { t: 'License expressions', d: 'Understands SPDX OR, AND, WITH and parentheses against your allow-list.' },
      { t: 'Ban list', d: 'Fail the build when a banned crate shows up anywhere in the graph.' },
      { t: 'Duplicate versions', d: 'Report crates that appear in more than one version.' },
      { t: 'Source allow-list', d: 'Flag anything that does not come from crates.io, such as git dependencies.' },
      { t: 'No cargo process', d: 'Reads Cargo.lock and the extracted registry manifests directly, and falls back to cargo metadata only when needed.' },
      { t: 'CI friendly', d: 'Exit code 1 on violations, policy in blaze.toml.' },
    ],
    usage: 'ward',
    config: '[deny]\nallow_licenses = ["MIT", "Apache-2.0", "Unicode-3.0"]\nban = ["openssl"]\ndeny_duplicates = false\ndeny_unknown_sources = true',
    how: ['Ward reads Cargo.lock for the resolved graph and the licenses from the already-extracted crate manifests in the cargo registry, then checks both against your policy. If a crate is not extracted or comes from git, it falls back to cargo metadata (cached by the hash of Cargo.lock).'],
    limits: 'No advisory database (RustSec) yet, so it does not cover security audits. Your own workspace crates and path dependencies are not license-checked.',
    bench: {
      title: 'License, ban and source checks', sub: 'This workspace. Lower is better, seconds.',
      rows: [{ name: 'cargo deny', incumbent: R.deny_cargo_deny, blaze: 0 }, { name: 'ward', incumbent: 0, blaze: R.deny_ward_cold }],
      note: 'Both skip advisories. Ward is measured without any cache directory.',
    },
  },
]
export const tool = (id: string) => TOOLS.find((t) => t.id === id)!
