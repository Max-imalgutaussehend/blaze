import results from '../../../bench/results.json'

export const R = results
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

export type ToolId = 'ember' | 'forge' | 'spark' | 'ward'

export interface ToolInfo {
  id: ToolId
  role: string
  alias: string
  tagline: string
  /** One sentence on the overview cards. */
  blurb: string
  can: { t: string; d: string }[]
  usage: string
  config: string
  how: string[]
  limits: string
  bench: { title: string; sub: string; rows: { name: string; incumbent: number; blaze: number }[]; note: string }
}

export const TOOLS: ToolInfo[] = [
  {
    id: 'ember', role: 'linter', alias: 'lint',
    tagline: 'Lint a whole crate in milliseconds.',
    blurb: 'Finds unwrap, dbg!, todo!, len() == 0 and more, straight from the token stream.',
    can: [
      { t: 'Six rules out of the box', d: 'unwrap, dbg!, todo!/unimplemented!, len() == 0, needless return and boolean comparisons.' },
      { t: 'No compile step', d: 'Works on a token stream, so it runs on code that does not build yet.' },
      { t: 'Parallel scanning', d: `${R.full_corpus_files.toLocaleString()} files (${R.full_corpus_mb} MB) in ${R.lint_ember_full_corpus.toFixed(1)} s, no cache.` },
      { t: 'Inline suppression', d: 'Add // blaze:ignore to a line. Items under #[cfg(test)] are skipped.' },
      { t: 'Configurable', d: 'Ignore rules per project in blaze.toml or per run with --ignore.' },
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
    id: 'forge', role: 'formatter', alias: 'fmt',
    tagline: 'Format Rust and keep every comment.',
    blurb: 'Re-attaches comments and blank lines, verifies the result, and caches by content hash.',
    can: [
      { t: 'Keeps comments and blank lines', d: 'They are re-attached after formatting and verified token for token.' },
      { t: 'Refuses to guess', d: 'If it cannot prove the output is safe, the file is left untouched and reported.' },
      { t: 'Content-hash cache', d: 'Clean, would-change and unsupported results are remembered per file content.' },
      { t: 'Parallel, big files first', d: 'Workers pick the largest files first so one huge file never trails the run.' },
      { t: 'CI mode', d: 'forge --check exits 1 when anything would change.' },
      { t: 'Runs in the browser', d: 'Try it in the playground below, compiled to WebAssembly.' },
    ],
    usage: 'forge .             # format in place\nforge --check .     # exit 1 if anything would change',
    config: '# forge has no options yet.',
    how: ['Forge parses with syn and prints with prettyplease, then tokenizes the original and the formatted text with the same lexer, aligns the code tokens and places each comment next to the token it belonged to.',
      'Before writing, it checks that the code tokens equal the formatter output and the comments are identical and in order. Results are cached per content hash, so repeat runs only touch edited files.'],
    limits: `The style follows prettyplease, not rustfmt, and rustfmt.toml is not read. In our ${R.files}-file sample about 1.4% of files could not be handled safely and about 0.2% changed again on a second pass.`,
    bench: {
      title: `Format ${R.files} files (--check)`, sub: 'Lower is better, seconds.',
      rows: [{ name: 'rustfmt (8 procs)', incumbent: R.fmt_rustfmt, blaze: 0 }, { name: 'forge (cold)', incumbent: 0, blaze: R.fmt_forge_cold }, { name: 'forge (cached)', incumbent: 0, blaze: R.fmt_forge_warm }],
      note: 'Not like-for-like: the style differs from rustfmt, and rustfmt also follows mod children.',
    },
  },
  {
    id: 'spark', role: 'test runner', alias: 'test',
    tagline: 'Run every test in its own process, in parallel.',
    blurb: 'One worker pool across all test binaries, with an opt-in cache of passed tests.',
    can: [
      { t: 'Process per test', d: 'Each test is isolated, so shared state cannot leak between tests.' },
      { t: 'One worker pool', d: 'Tests from every binary are scheduled together across all cores.' },
      { t: 'Parallel discovery', d: 'Test lists of all binaries are collected at the same time.' },
      { t: 'Result cache (opt-in)', d: '--cached skips tests that passed against a byte-identical binary.' },
      { t: 'Name filter', d: 'spark parse runs only tests whose name contains "parse".' },
      { t: 'Works with cargo test', d: 'Uses the standard test binaries, no changes to your tests.' },
    ],
    usage: 'spark                 # run everything\nspark parse           # only tests whose name contains "parse"\nspark --cached        # skip tests proven unchanged',
    config: '# spark has no options yet.',
    how: ['Spark builds once with cargo test --no-run, lists the tests of every binary in parallel, then runs each test in its own process on a shared worker pool. Failures print their captured output at the end.',
      'With --cached, a test that passed against a byte-identical test binary (blake3 hash) is skipped. That is only sound for deterministic tests whose inputs live inside the binary, which is why it is opt-in.'],
    limits: 'No doc-tests, retries, JUnit output or per-test timeouts yet. Cached mode ignores files that tests read at runtime.',
    bench: {
      title: 'Run 147 tests', sub: 'regex-syntax, binaries already built. Lower is better, seconds.',
      rows: [{ name: 'cargo test', incumbent: R.test_cargo_test, blaze: 0 }, { name: 'cargo nextest', incumbent: R.test_nextest, blaze: 0 }, { name: 'spark', incumbent: 0, blaze: R.test_spark }, { name: 'spark --cached', incumbent: 0, blaze: R.test_spark_cached }],
      note: 'Cached mode describes repeat runs of deterministic tests.',
    },
  },
  {
    id: 'ward', role: 'dependency policy', alias: 'deny',
    tagline: 'Dependency policy, checked from a cached graph.',
    blurb: 'Licenses, bans, duplicate versions and sources, with the graph cached by Cargo.lock.',
    can: [
      { t: 'License expressions', d: 'Understands SPDX OR, AND, WITH and parentheses against your allow-list.' },
      { t: 'Ban list', d: 'Fail the build when a banned crate shows up anywhere in the graph.' },
      { t: 'Duplicate versions', d: 'Report crates that appear in more than one version.' },
      { t: 'Source allow-list', d: 'Flag anything that does not come from crates.io, such as git dependencies.' },
      { t: 'Cached graph', d: 'cargo metadata is cached by the hash of Cargo.lock and Cargo.toml.' },
      { t: 'CI friendly', d: 'Exit code 1 on violations, policy in blaze.toml.' },
    ],
    usage: 'ward',
    config: '[deny]\nallow_licenses = ["MIT", "Apache-2.0", "Unicode-3.0"]\nban = ["openssl"]\ndeny_duplicates = false\ndeny_unknown_sources = true',
    how: ['Ward reads the resolved dependency graph from cargo metadata, caches it keyed by the hash of Cargo.lock and Cargo.toml, and checks it against your policy.'],
    limits: 'No advisory database (RustSec) yet, so it does not cover security audits.',
    bench: {
      title: 'License, ban and source checks', sub: 'This workspace. Lower is better, seconds.',
      rows: [{ name: 'cargo deny', incumbent: R.deny_cargo_deny, blaze: 0 }, { name: 'ward (cached graph)', incumbent: 0, blaze: R.deny_ward_warm }],
      note: 'Both skip advisories. The ward number needs a warm graph cache.',
    },
  },
]
export const tool = (id: string) => TOOLS.find((t) => t.id === id)!
