import results from '../../../bench/results.json'

export const R = results
export const VERSION = 'v1.0.0'
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
      { t: 'build, check, run', d: 'Skips cargo entirely when none of the files it tracks changed. Works with features, profiles, packages and examples.' },
      { t: 'run starts your binary directly', d: 'No second cargo process: an up-to-date binary or example is executed straight away.' },
      { t: 'add and remove', d: 'Edit Cargo.toml in place, keep comments and sort order, validate features against the crates.io index, and understand workspaces with -p.' },
      { t: 'Cached index lookups', d: 'Versions come from the sparse index with an ETag cache, so repeat lookups are a tiny 304.' },
      { t: 'Shared target directory', d: 'Opt in, and every project reuses dependencies another project already compiled. A second project builds cold in under a second.' },
      { t: 'Everything else is cargo', d: 'new, init, publish and every other command are forwarded unchanged. blaze ci runs the checks for CI and pre-commit.' },
    ],
    usage: 'blaze build                    # skips cargo when nothing changed\nblaze run -- --flag value      # build if needed, then exec the binary\nblaze run --example demo\nblaze check                    # type-check, same fast path\nblaze add serde -F derive      # edit Cargo.toml, resolve from the index\nblaze add -p app itoa          # a workspace member\nblaze remove serde\nblaze ci                       # forge --check + ember + ward',
    config: '# blaze.toml\n[build]\nshared_target = true   # or BLAZE_SHARED_TARGET=1\n\n# Set BLAZE_NO_FAST=1 to always go through cargo.',
    how: ['After a successful cargo run, blaze records every file that influenced it: rustc dep-info for your crates (this includes include_str! files), manifests, Cargo.lock, build-script rerun-if-changed inputs and environment variables. The next call just stats that list. Every flag is part of the cache key.',
      'If nothing changed and the outputs still exist, cargo is never started. Registry dependencies are treated as immutable for a given Cargo.lock, which is tracked. Any other change falls through to a normal cargo build.',
      'With a shared target directory (CARGO_TARGET_DIR pointed at one cache), cargo itself reuses compiled dependencies across projects with the same versions, features and toolchain.'],
    limits: 'Blaze does not replace cargo\'s resolver or compiler orchestration: the first build of a project is a cargo build, and so is any build after a change. Flags that change cargo\'s output or write extra files (--message-format, --timings, --out-dir, --manifest-path, -Z) go to cargo. add leaves Cargo.lock to the next build. The shared target serializes concurrent builds, because cargo locks the directory.',
    bench: {
      title: 'Commands with nothing to do, and a shared cold build', sub: 'This workspace (about 170 crates) for the no-op rows. Lower is better, seconds.',
      rows: [
        { name: 'cargo build', incumbent: R.build_cargo_noop, blaze: 0 }, { name: 'blaze build', incumbent: 0, blaze: R.build_blaze_noop },
        { name: 'cargo check', incumbent: R.check_cargo_noop, blaze: 0 }, { name: 'blaze check', incumbent: 0, blaze: R.check_blaze_noop },
        { name: 'cargo run', incumbent: R.run_cargo_noop, blaze: 0 }, { name: 'blaze run', incumbent: 0, blaze: R.run_blaze_noop },
        { name: 'cargo add x3', incumbent: R.add_cargo_3, blaze: 0 }, { name: 'blaze add x3', incumbent: 0, blaze: R.add_blaze_3 },
        { name: 'cargo, 2nd project cold', incumbent: R.coldbuild_cargo_second, blaze: 0 }, { name: 'blaze shared, 2nd cold', incumbent: 0, blaze: R.coldbuild_blaze_shared_second },
      ],
      note: 'Cargo\'s no-op cost grows with the dependency graph; blaze only stats the recorded files. The cold-build rows use two projects with the same six dependencies; the first project of a shared target is exactly as slow as cargo. The add timings need network and a warm index cache for blaze.',
    },
  },
  {
    id: 'ember', role: 'linter', badge: 'blaze lint',
    tagline: 'Lint a whole crate in milliseconds, and fix it.',
    blurb: '31 token-level rules with safe automatic fixes. No compile step.',
    headline: { value: x(R.clippy_cold, R.lint_ember_crate), label: 'faster than a clean clippy run (speed only, clippy checks far more)' },
    can: [
      { t: '31 rules', d: 'unwrap, dbg!, todo!, len() == 0, needless return, mem::replace, identity ops, redundant patterns, ptr-arg and more. Run ember --rules for the list.' },
      { t: 'Automatic fixes', d: '--fix applies fixes that do not depend on types. --unsafe-fixes adds the ones that can fail to compile for some types.' },
      { t: 'No compile step', d: 'Works on a token stream, so it runs on code that does not build yet.' },
      { t: 'Parallel, and skips clean files', d: `${R.full_corpus_files.toLocaleString()} files (${R.full_corpus_mb} MB) in ${R.lint_ember_full_corpus.toFixed(1)} s. Repeat runs skip files that were clean by stat.` },
      { t: 'Select, ignore, suppress', d: '--select and --ignore take codes or names, blaze.toml sets defaults, // blaze:ignore B001 silences one line. Test and example files are relaxed.' },
      { t: 'JSON and deep mode', d: '--format json for tools. --deep also runs cargo clippy for the type-aware lints ember cannot do.' },
    ],
    usage: 'ember .                       # lint with the default rules\nember --fix .                 # apply fixes that never depend on types\nember --select ALL .          # every rule, including opt-in ones\nember --ignore B001,len-zero .\nember --format json .         # one JSON object per finding\nember --deep .                # ember, then cargo clippy\nember --rules                 # list all rules',
    config: '[ember]\nselect = ["ALL"]        # codes, names, or ALL; default is the on-by-default set\nignore = ["B006", "unwrap-used"]',
    how: ['Ember never builds a syntax tree. A hand-written lexer produces tokens, and each rule is a small pattern over that stream. Files are scanned in parallel, so throughput is limited by memory bandwidth, not parsing.',
      'Each rule only fires on shapes that are unambiguous without types. Fixes are local token rewrites applied together only if the bracket structure of the file stays intact. Fixes that depend on the receiver type (for example .is_empty() on an iterator) are classed as unsafe. We apply all fixes to real crates and recompile them as part of testing.'],
    limits: 'Ember is a syntactic linter by design: no type information, so it cannot replace clippy\'s type-aware lints. Use --deep to run clippy alongside it. Some rules can report on custom types that happen to have a same-named method.',
    bench: {
      title: 'Time to first diagnostics', sub: 'One crate (regex-syntax). Lower is better, seconds.',
      rows: [{ name: 'clippy (clean build)', incumbent: R.clippy_cold, blaze: 0 }, { name: 'clippy (no changes)', incumbent: R.clippy_warm, blaze: 0 }, { name: 'ember', incumbent: 0, blaze: R.lint_ember_crate }],
      note: 'Clippy type-checks your code and has hundreds of lints. Ember has 31 syntactic ones. This chart compares speed only.',
    },
  },
  {
    id: 'forge', role: 'formatter', badge: 'blaze fmt',
    tagline: 'Format Rust and keep every comment.',
    blurb: 'Keeps comments and blank lines, formats macro arguments, and is idempotent.',
    headline: { value: x(R.fmt_rustfmt, R.fmt_forge_cold), label: `faster than rustfmt cold, ${x(R.fmt_rustfmt, R.fmt_forge_warm)} on a repeat run` },
    can: [
      { t: 'Keeps comments and blank lines', d: 'They are re-attached after formatting and verified token for token. Comments before a closing brace stay inside the block.' },
      { t: 'Formats macro arguments', d: 'println!, format!, assert_eq!, vec! and friends are formatted like calls instead of token by token.' },
      { t: 'Idempotent', d: 'Formatting a formatted file changes nothing. Where the underlying printer is unstable, forge formats to a fixed point.' },
      { t: 'Refuses to guess', d: 'If it cannot prove the output is safe, the file is left untouched and reported (about 1 in 150 files).' },
      { t: 'Fast on repeat runs and big files', d: 'Skips unchanged files by stat, and splits huge files at top-level items so all cores work on them.' },
      { t: 'CI mode and WebAssembly', d: 'forge --check exits 1 when anything would change. The playground runs the same code in your browser.' },
    ],
    usage: 'forge .             # format in place\nforge --check .     # exit 1 if anything would change',
    config: '# forge has one fixed, documented style and no options.',
    how: ['Forge parses with syn and prints with prettyplease, then tokenizes the original and the formatted text with the same lexer, aligns the code tokens and places each comment next to the token it belonged to.',
      'Before writing, it checks that the code tokens equal the formatter output and the comments are identical and in order. Macro calls with comma-separated expression arguments are temporarily turned into ordinary calls so they get real formatting. Files over 192 KB are split at top-level item boundaries and formatted in parallel; the result is identical to formatting the whole file.'],
    limits: 'The style is forge\'s own and does not match rustfmt, so adopting forge in an existing rustfmt codebase produces a large one-time diff. rustfmt.toml is not read. Macros whose arguments are not plain expressions (matches!, macro_rules! bodies) are still printed token by token.',
    bench: {
      title: `Format ${R.files} files (--check)`, sub: 'Lower is better, seconds.',
      rows: [{ name: 'rustfmt (8 procs)', incumbent: R.fmt_rustfmt, blaze: 0 }, { name: 'forge (cold)', incumbent: 0, blaze: R.fmt_forge_cold }, { name: 'forge (repeat run)', incumbent: 0, blaze: R.fmt_forge_warm }],
      note: 'Not like-for-like: the style differs from rustfmt, and rustfmt also follows mod children.',
    },
  },
  {
    id: 'spark', role: 'test runner', badge: 'blaze test',
    tagline: 'Run every test in its own process, in parallel.',
    blurb: 'Isolated or shared mode, retries, timeouts, JUnit output and an opt-in result cache.',
    headline: { value: x(R.test_nextest, R.test_spark_cached), label: `faster than nextest with --cached, ${x(R.test_nextest, R.test_spark)} without` },
    can: [
      { t: 'Process per test', d: 'Each test is isolated, so shared state cannot leak between tests. One worker pool schedules every binary.' },
      { t: 'Shared mode', d: '--shared runs each binary once with libtest threads, like cargo test but without the cargo overhead.' },
      { t: 'Retries and flaky tests', d: '--retries N re-runs failures. A pass on a retry is reported as flaky instead of failed.' },
      { t: 'Timeouts', d: '--timeout SECONDS kills a test that hangs and reports it as timed out.' },
      { t: 'JUnit and doc-tests', d: '--junit FILE writes a report for CI. --doc also runs documentation tests through cargo.' },
      { t: 'Result cache (opt-in)', d: '--cached skips tests that passed against a byte-identical binary.' },
    ],
    usage: 'spark                       # one process per test\nspark --shared              # one process per binary\nspark parse                 # only tests whose name contains "parse"\nspark --retries 2 --timeout 30\nspark --junit report.xml --doc\nspark --cached              # skip tests proven unchanged',
    config: '# spark has no options in blaze.toml; behavior is chosen per run.',
    how: ['Spark builds once with cargo test --no-run, lists the tests of every binary in parallel, then runs each test in its own process on a shared worker pool. Failures print their captured output at the end.',
      'With --cached, a test that passed against a byte-identical test binary (blake3 hash) is skipped. That is only sound for deterministic tests whose inputs live inside the binary, which is why it is opt-in. Retries and timeouts run in isolation; doc-tests are delegated to cargo, which compiles each one.'],
    limits: 'Timeouts need one process per test, so they do not combine with --shared. Cached mode ignores files that tests read at runtime. Doc-tests are run by cargo, not by spark\'s scheduler.',
    bench: {
      title: 'Run 147 tests', sub: 'regex-syntax, binaries already built. Lower is better, seconds.',
      rows: [{ name: 'cargo test', incumbent: R.test_cargo_test, blaze: 0 }, { name: 'cargo nextest', incumbent: R.test_nextest, blaze: 0 }, { name: 'spark', incumbent: 0, blaze: R.test_spark }, { name: 'spark --shared', incumbent: 0, blaze: R.test_spark_shared }, { name: 'spark --cached', incumbent: 0, blaze: R.test_spark_cached }],
      note: 'Most of the remaining time is the tests themselves (debug build). Cached mode describes repeat runs of deterministic tests.',
    },
  },
  {
    id: 'ward', role: 'dependency policy', badge: 'blaze deny',
    tagline: 'Licenses, bans and security advisories in milliseconds.',
    blurb: 'Reads Cargo.lock directly and checks the RustSec advisory database from a local cache.',
    headline: { value: x(R.advisories_cargo_deny, R.advisories_ward), label: 'faster than cargo deny for advisories, and licenses are checked too' },
    can: [
      { t: 'RustSec advisories', d: 'Vulnerabilities fail the run; unmaintained and unsound crates warn, or fail with a setting. Ignore ids you have reviewed.' },
      { t: 'License expressions', d: 'Understands SPDX OR, AND, WITH and parentheses against your allow-list.' },
      { t: 'Bans, duplicates, sources', d: 'Ban crates, report duplicate versions, and flag anything that does not come from crates.io.' },
      { t: 'No cargo process', d: 'Reads Cargo.lock and the extracted registry manifests directly, and falls back to cargo metadata only when needed.' },
      { t: 'Cached, refreshed daily', d: 'The advisory database is downloaded once, indexed per crate, and refreshed with a conditional request at most once a day. --offline never touches the network.' },
      { t: 'CI friendly', d: 'Exit code 1 on violations, policy in blaze.toml. If the database is unavailable the check degrades with a note, or fails with --strict.' },
    ],
    usage: 'ward                  # licenses, bans, sources, advisories\nward --offline        # use the cached advisory database only\nward --update         # refresh the advisory database now\nward --no-advisories  # skip the advisory check\nward --strict         # fail if advisories cannot be checked',
    config: '[deny]\nallow_licenses = ["MIT", "Apache-2.0", "Unicode-3.0"]\nban = ["openssl"]\ndeny_duplicates = false\ndeny_unknown_sources = true\nignore_advisories = ["RUSTSEC-2020-0071"]\ndeny_unmaintained = false\ndeny_unsound = false',
    how: ['Ward reads Cargo.lock for the resolved graph and the licenses from the already-extracted crate manifests in the cargo registry, then checks both against your policy. If a crate is not extracted or comes from git, it falls back to cargo metadata (cached by the hash of Cargo.lock).',
      'For advisories it downloads the RustSec database tarball once, splits it into one small file per crate, and reads only the files of the crates in your lockfile. A crate is affected unless its version matches a patched or unaffected requirement.'],
    limits: 'Your own workspace crates and path dependencies are not license-checked. The first advisory check needs network access to download the database (a few seconds); later runs use the cache.',
    bench: {
      title: 'Advisories, and licenses without a cache', sub: 'Lower is better, seconds.',
      rows: [{ name: 'cargo deny advisories', incumbent: R.advisories_cargo_deny, blaze: 0 }, { name: 'ward (advisories)', incumbent: 0, blaze: R.advisories_ward }, { name: 'cargo deny licenses', incumbent: R.deny_cargo_deny, blaze: 0 }, { name: 'ward (no cache)', incumbent: 0, blaze: R.deny_ward_cold }],
      note: 'Both tools report the same advisory for the test project (time 0.1.43). Databases are warm for both. The license rows use this workspace; both skip advisories there.',
    },
  },
]
export const tool = (id: string) => TOOLS.find((t) => t.id === id)!
