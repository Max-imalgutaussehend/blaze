#!/usr/bin/env python3
"""Generates the static site into docs/_site from bench/results.json. No dependencies."""
import json, os, shutil, html

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "_site")
R = json.load(open(os.path.join(HERE, "..", "bench", "results.json")))
REPO = "https://github.com/Max-imalgutaussehend/blaze"
INSTALL_SH = f"curl -LsSf {REPO}/releases/latest/download/install.sh | sh"
INSTALL_CARGO = f"cargo install --git {REPO} blaze"

TOOLS = [
    ("ember", "linter", "Ruff", "An extremely fast Rust linter. Token-level rules, no compile step.", "lint"),
    ("forge", "formatter", "rustfmt", "A fast, comment-preserving Rust formatter with a content-hash cache.", "fmt"),
    ("spark", "test runner", "cargo-nextest", "A parallel, process-per-test runner that can skip tests proven unchanged.", "test"),
    ("ward", "dependency policy", "cargo-deny", "License, ban, duplicate and source checks that run in milliseconds.", "deny"),
]


def x(a, b):
    return f"{a / b:.0f}×" if a / b >= 10 else f"{a / b:.1f}×"


def fmt_t(s):
    return f"{s * 1000:.0f} ms" if s < 1 else f"{s:.2f} s"


def bars(title, sub, rows, note=""):
    top = max(v for _, v, _ in rows)
    out = f'<div class="bench"><h3>{title}</h3><div class="sub">{sub}</div>'
    for label, v, us in rows:
        out += (f'<div class="row{" us" if us else ""}"><span>{label}</span>'
                f'<div class="bar"><i style="width:{max(v / top * 100, 0.6):.2f}%"></i></div>'
                f'<span class="v">{fmt_t(v)}</span></div>')
    return out + (f'<div class="note">{note}</div>' if note else "") + "</div>"


def b_fmt():
    return bars("Formatting", f"{R['files']} real crates.io files ({R['machine']}), <code>--check</code>, lower is better", [
        ("rustfmt (8 procs)", R["fmt_rustfmt"], False),
        ("forge, cold", R["fmt_forge_cold"], True),
        ("forge, cached", R["fmt_forge_warm"], True)],
        f"cold {x(R['fmt_rustfmt'], R['fmt_forge_cold'])}, cached {x(R['fmt_rustfmt'], R['fmt_forge_warm'])} faster. "
        "Not like-for-like: forge's style is prettyplease-based, not rustfmt's, and rustfmt also follows <code>mod</code> children.")


def b_lint():
    return bars("Linting", f"one crate (regex-syntax), time to first diagnostics, lower is better", [
        ("clippy, clean build", R["clippy_cold"], False),
        ("clippy, no changes", R["clippy_warm"], False),
        ("ember", R["lint_ember_crate"], True)],
        f"{x(R['clippy_cold'], R['lint_ember_crate'])} vs a clean clippy run, {x(R['clippy_warm'], R['lint_ember_crate'])} vs a no-op one. "
        "Clippy type-checks your code and has hundreds of lints; ember has 6 syntactic ones. This compares speed, not coverage.")


def b_test():
    return bars("Testing", "regex-syntax, 147 tests, binaries already built, lower is better", [
        ("cargo test", R["test_cargo_test"], False),
        ("cargo nextest", R["test_nextest"], False),
        ("spark", R["test_spark"], True),
        ("spark --cached", R["test_spark_cached"], True)],
        f"{x(R['test_nextest'], R['test_spark'])} vs nextest; {x(R['test_nextest'], R['test_spark_cached'])} with <code>--cached</code>, which assumes deterministic tests.")


def b_deny():
    return bars("Dependency checks", "licenses + bans + sources on this workspace, lower is better", [
        ("cargo deny (offline checks)", R["deny_cargo_deny"], False),
        ("ward", R["deny_ward_warm"], True)],
        f"{x(R['deny_cargo_deny'], R['deny_ward_warm'])} faster once <code>cargo metadata</code> is cached. ward has no advisory (RustSec) check yet; the cargo-deny run above skips advisories too.")


def b_scan():
    return bars("Raw lint throughput", f"{R['full_corpus_files']:,} files / {R['full_corpus_mb']} MB of crates.io source", [
        ("ember", R["lint_ember_full_corpus"], True)],
        f"≈ {R['full_corpus_mb'] / R['lint_ember_full_corpus']:.0f} MB/s on 8 cores, no cache.")


NAV = [("Tools", "#tools"), ("Benchmarks", "benchmarks/"), ("Guide", "guide/"), ("GitHub", REPO)]


def page(path, title, body, depth, desc="Extremely fast Rust tooling."):
    root = "../" * depth
    nav = "".join(
        f'<a href="{h if h.startswith("http") else root + (h if not h.startswith("#") else "index.html" + h)}">{n}</a>'
        for n, h in NAV)
    toolnav = "".join(f'<a href="{root}{t[0]}/">{t[0]}</a>' for t in TOOLS)
    doc = f'''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{html.escape(title)}</title><meta name="description" content="{html.escape(desc)}">
<link rel="stylesheet" href="{root}assets/style.css"></head><body>
<nav><div class="wrap"><a class="logo" href="{root}">bl<b>a</b>ze</a>{toolnav}{nav}</div></nav>
{body}
<footer><div class="wrap">Blaze is an independent early-stage project and is not affiliated with Astral. Dual-licensed MIT or Apache-2.0.
· <a href="{REPO}">Source</a> · <a href="{root}benchmarks/">Benchmark methodology</a></div></footer>
<script src="{root}assets/site.js"></script></body></html>'''
    p = os.path.join(OUT, path)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    open(p, "w").write(doc)


# ---------- index ----------
cards = "".join(
    f'<a class="card" href="{n}/"><h3>{n}</h3><div class="rep">{role} · replaces {rep}</div><p>{d}</p></a>'
    for n, role, rep, d, _ in TOOLS)
cards = ('<a class="card" href="guide/"><h3>blaze</h3><div class="rep">the umbrella · wraps cargo</div>'
         '<p>One binary that dispatches to every tool below and forwards everything else to cargo.</p></a>') + cards
hero = f'''<header class="hero"><div class="wrap">
<h1>Extremely fast <em>Rust tooling</em>,<br>written in Rust.</h1>
<p>Lint, format, test and audit dependencies with a single toolchain. Built around a fast lexer, parallel workers and content-hash caches.</p>
<div class="tabs"><button class="on" data-cmd="{INSTALL_SH}">macOS &amp; Linux</button><button data-cmd="{INSTALL_CARGO}">cargo</button></div>
<div class="install"><code id="cmd">{INSTALL_SH}</code><button data-copy="cmd">Copy</button></div>
<p style="margin-top:22px;font-size:.95rem">Early alpha. Numbers below are measured, caveats included.</p>
</div></header>'''
numbers = f'''<section><div class="wrap"><h2>Speed, with the fine print</h2>
<p class="lede">Every figure is produced by <a href="{REPO}/blob/main/bench/run.py"><code>bench/run.py</code></a> and rendered into this page from <code>bench/results.json</code>.</p>
<div class="grid">
<div class="card"><div class="big">{x(R['clippy_cold'], R['lint_ember_crate'])}</div><p><b>ember</b> vs a clean <code>clippy</code> run (different depth of analysis)</p></div>
<div class="card"><div class="big">{x(R['fmt_rustfmt'], R['fmt_forge_warm'])}</div><p><b>forge</b> vs <code>rustfmt</code> with a warm cache; {x(R['fmt_rustfmt'], R['fmt_forge_cold'])} cold</p></div>
<div class="card"><div class="big">{x(R['test_nextest'], R['test_spark_cached'])}</div><p><b>spark --cached</b> vs <code>nextest</code>; {x(R['test_nextest'], R['test_spark'])} without the cache</p></div>
<div class="card"><div class="big">{x(R['deny_cargo_deny'], R['deny_ward_warm'])}</div><p><b>ward</b> vs <code>cargo deny</code> for license/ban/source checks</p></div>
</div></div></section>'''
index = hero + f'<section id="tools"><div class="wrap"><h2>One toolchain</h2><p class="lede">Each tool is a standalone binary and also a subcommand of <code>blaze</code>.</p><div class="grid">{cards}</div></div></section>' + numbers + f'''
<section><div class="wrap"><h2>Try it</h2><pre><code>blaze check          # forge --check + ember + ward
ember .              # lint
forge --check .      # format check
spark --cached       # run tests, skip proven-unchanged ones
ward                 # dependency policy
blaze add serde      # anything else goes to cargo</code></pre></div></section>'''
page("index.html", "Blaze — extremely fast Rust tooling", index, 0)

# ---------- tool pages ----------
PAGES = {
 "ember": dict(
  tag="An extremely fast Rust linter.", bench=b_lint() + b_scan(),
  how=["Ember never builds a syntax tree. A hand-written lexer produces tokens and every rule is a small pattern over that token stream, so linting is bounded by memory bandwidth and scales across files with rayon.",
       "Because there is no type information, ember cannot replace clippy's type-aware lints. It targets the cheap, high-signal checks you want on every save."],
  use="""ember .                       # lint the current directory
ember src/ --ignore B001      # skip a rule for this run
ember --rules                 # list rules""",
  conf="""[ember]
ignore = ["B006"]""",
  extra="""<h2>Rules</h2><table><tr><th>Code</th><th>Name</th><th>What it flags</th></tr>
<tr><td>B001</td><td>unwrap-used</td><td><code>.unwrap()</code></td></tr>
<tr><td>B002</td><td>dbg-macro</td><td>leftover <code>dbg!</code></td></tr>
<tr><td>B003</td><td>todo-macro</td><td><code>todo!</code> / <code>unimplemented!</code></td></tr>
<tr><td>B004</td><td>len-zero</td><td><code>.len() == 0</code> instead of <code>.is_empty()</code></td></tr>
<tr><td>B005</td><td>needless-return</td><td><code>return x;</code> as the last statement of a fn</td></tr>
<tr><td>B006</td><td>bool-comparison</td><td><code>== true</code> / <code>!= false</code></td></tr></table>
<p>Suppress a finding with <code>// blaze:ignore</code> on the same line. <code>#[cfg(test)]</code> items are skipped.</p>""",
  limits="Six syntactic rules. No autofix. No type information, so no false-positive protection that clippy gets from it (for example B001 also flags <code>.unwrap()</code> on types where it cannot fail)."),
 "forge": dict(
  tag="A fast Rust formatter that keeps your comments.", bench=b_fmt(),
  how=["Forge parses with <code>syn</code>, prints with <code>prettyplease</code>, then re-attaches comments and blank lines: the original and formatted text are tokenized with the same lexer, the code token streams are aligned, and each comment is placed next to the token it belonged to.",
       "The result is verified before it is written: code tokens must equal the formatter's output and the comments must be identical and in the same order. If verification fails, the file is left untouched and reported as unsupported. Results (clean / would-change / unsupported) are cached per content hash, so repeated runs only touch edited files."],
  use="""forge .             # format in place
forge --check .     # exit 1 if anything would change""",
  conf="# forge has no options yet.",
  extra="",
  limits="Style follows prettyplease, not rustfmt: expect different line breaking, and no <code>rustfmt.toml</code> support. In our 2,000-file sample about 1.4% of files could not be handled safely (unsupported or unverifiable) and about 0.2% were not idempotent on a second pass. Trailing comments can move to the end of the line the formatter moved their token to."),
 "spark": dict(
  tag="A parallel test runner with an opt-in result cache.", bench=b_test(),
  how=["Spark builds once with <code>cargo test --no-run</code>, lists the tests of every binary in parallel, then runs each test in its own process on a shared worker pool. Failures print their captured output at the end.",
       "With <code>--cached</code>, a test that passed against a byte-identical test binary (blake3 hash) is skipped. This is sound only for deterministic tests whose inputs are inside the binary, which is why it is opt-in."],
  use="""spark                 # run everything
spark parse           # only tests whose name contains "parse"
spark --cached        # skip tests proven unchanged""",
  conf="# spark has no options yet.",
  extra="",
  limits="No doc-tests, no retries, no JUnit output, no per-test timeouts. Cached mode ignores files tests read at runtime."),
 "ward": dict(
  tag="Dependency policy checks that finish before you notice.", bench=b_deny(),
  how=["Ward reads the resolved graph from <code>cargo metadata</code> (cached by the hash of <code>Cargo.lock</code> and <code>Cargo.toml</code>) and checks it against your policy: SPDX license expressions (<code>OR</code>, <code>AND</code>, <code>WITH</code>, parentheses), banned crates, duplicate versions and non-crates.io sources."],
  use="ward",
  conf="""[deny]
allow_licenses = ["MIT", "Apache-2.0", "Unicode-3.0"]
ban = ["openssl"]
deny_duplicates = false
deny_unknown_sources = true""",
  extra="",
  limits="No advisory database (RustSec) yet, so it does not replace cargo-deny for security audits."),
}
for name, role, rep, desc, alias in TOOLS:
    P = PAGES[name]
    how = "".join(f"<p>{h}</p>" for h in P["how"])
    body = f'''<div class="wrap page prose"><h1>{name}</h1><div class="tag">{P["tag"]} Replaces {rep}.</div>
<pre><code>{html.escape(P["use"])}</code></pre>
{P["bench"]}
<h2>How it works</h2>{how}
<h2>Configuration</h2><p>In <code>blaze.toml</code> at the project root:</p><pre><code>{html.escape(P["conf"])}</code></pre>
{P["extra"]}
<h2>Limitations</h2><div class="warn">{P["limits"]}</div>
<p>Also available as <code>blaze {alias}</code>.</p></div>'''
    page(f"{name}/index.html", f"{name} — Blaze", body, 1, desc)

# ---------- benchmarks ----------
body = f'''<div class="wrap page prose"><h1>Benchmarks</h1><div class="tag">Reproducible, and honest about what they do not show.</div>
{b_fmt()}{b_lint()}{b_scan()}{b_test()}{b_deny()}
<h2>Method</h2><ul>
<li>Machine: {R['machine']}. Each number is the best of 3 wall-clock runs; cold runs delete <code>.blaze_cache</code> first.</li>
<li>Formatting corpus: {R['files']} files sampled with a fixed seed from the {R['full_corpus_files']:,} <code>.rs</code> files ({R['full_corpus_mb']} MB) in a local crates.io registry cache. rustfmt runs as <code>xargs -n 40 -P 8 rustfmt --edition 2021 --check</code>.</li>
<li>Testing: <code>regex-syntax 0.8.10</code>, test binaries pre-built; every tool pays for its own build check.</li>
<li>Reproduce: <code>cargo build --release &amp;&amp; python3 bench/run.py &lt;corpus&gt; &lt;crate&gt;</code>.</li></ul>
<h2>What these numbers do not say</h2><div class="warn">
Ember is not a clippy replacement; it is a much smaller linter. Forge's output differs from rustfmt's. Cached numbers describe repeat runs, which is the common case in editors, pre-commit hooks and CI with a restored cache, but not the first run. Timings on a laptop vary with background load (we saw real-time antivirus scanning change results by a large factor), so expect different absolute values.</div></div>'''
page("benchmarks/index.html", "Benchmarks — Blaze", body, 1)

# ---------- guide ----------
body = f'''<div class="wrap page prose"><h1>Guide</h1><div class="tag">From zero to a green <code>blaze check</code>.</div>
<h2>Install</h2><pre><code>{INSTALL_SH}
# or
{INSTALL_CARGO}</code></pre>
<p>This installs <code>blaze</code>, <code>ember</code>, <code>forge</code>, <code>spark</code> and <code>ward</code>.</p>
<h2>Everyday use</h2><pre><code>blaze new demo &amp;&amp; cd demo   # forwarded to cargo
blaze add serde                # forwarded to cargo
blaze check                    # forge --check, ember, ward
blaze spark --cached           # test (alias: blaze test)</code></pre>
<h2>Configuration</h2><p>One file, <code>blaze.toml</code>, with a table per tool:</p>
<pre><code>[ember]
ignore = ["B001"]

[deny]
allow_licenses = ["MIT", "Apache-2.0"]
deny_unknown_sources = true</code></pre>
<h2>Pre-commit</h2><pre><code>repos:
  - repo: {REPO}
    rev: v0.2.0
    hooks:
      - id: forge
      - id: ember</code></pre>
<h2>GitHub Actions</h2><pre><code>- uses: Max-imalgutaussehend/blaze@v0.2.0
- run: blaze check</code></pre>
<h2>Caching</h2><p>Tools store state in <code>.blaze_cache/</code> (self-ignoring). Cache this directory in CI to get the cached numbers. Cache entries are keyed by file content hash and tool version.</p>
<h2>What is forwarded to cargo</h2><p>Any subcommand blaze does not know (<code>build</code>, <code>add</code>, <code>run</code>, ...) is passed to cargo unchanged. Blaze does not replace cargo's resolver or build orchestration.</p></div>'''
page("guide/index.html", "Guide — Blaze", body, 1)

shutil.copytree(os.path.join(HERE, "assets"), os.path.join(OUT, "assets"), dirs_exist_ok=True)
open(os.path.join(OUT, ".nojekyll"), "w").write("")
print("site ->", OUT)
