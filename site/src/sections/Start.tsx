import { useState } from 'react'
import { INSTALL_CARGO, INSTALL_SH, REPO, logo } from '../lib/blaze'
import { Reveal } from './Reveal'

const SNIPPETS: Record<string, { label: string; code: string }> = {
  use: { label: 'Use', code: `blaze check          # forge --check + ember + ward\nember .              # lint\nforge --check .      # format check\nspark --cached       # tests; skip proven-unchanged ones\nward                 # dependency policy\nblaze add serde      # anything else goes to cargo` },
  config: { label: 'blaze.toml', code: `[ember]\nignore = ["B006"]\n\n[deny]\nallow_licenses = ["MIT", "Apache-2.0", "Unicode-3.0"]\nban = ["openssl"]\ndeny_unknown_sources = true` },
  ci: { label: 'GitHub Actions', code: `- uses: Max-imalgutaussehend/blaze@v0.2.0\n- uses: actions/cache@v4\n  with: { path: .blaze_cache, key: blaze-\${{ hashFiles('**/*.rs') }} }\n- run: blaze check` },
  hook: { label: 'pre-commit', code: `repos:\n  - repo: ${REPO}\n    rev: v0.2.0\n    hooks:\n      - id: forge\n      - id: ember` },
}

export function Start() {
  const [k, setK] = useState('use')
  return (
    <section id="start" className="section">
      <div className="wrap" style={{ display: 'grid', gap: 40, gridTemplateColumns: 'repeat(auto-fit, minmax(340px, 1fr))', alignItems: 'start' }}>
        <Reveal>
          <div className="eyebrow">Get started</div>
          <h2 className="h2">Install in <span className="grad">ten seconds.</span></h2>
          <p className="lede">Prebuilt binaries for macOS (arm64, x86_64) and Linux (x86_64). The installer puts <code>blaze</code>, <code>ember</code>, <code>forge</code>, <code>spark</code> and <code>ward</code> in <code>~/.local/bin</code>.</p>
          <pre className="card" style={{ padding: 16, fontSize: 13, overflowX: 'auto', marginTop: 22 }}><code><span style={{ color: 'var(--rust-1)' }}>$ </span>{INSTALL_SH}{'\n'}<span style={{ color: 'var(--smoke)' }}># or{'\n'}</span><span style={{ color: 'var(--rust-1)' }}>$ </span>{INSTALL_CARGO}</code></pre>
        </Reveal>
        <Reveal delay={0.1}>
          <div className="win">
            <div className="win-bar"><i /><i /><i /><div className="tabs" style={{ marginLeft: 12, flexWrap: 'wrap' }}>
              {Object.entries(SNIPPETS).map(([id, s]) => <button key={id} className={k === id ? 'on' : ''} onClick={() => setK(id)}>{s.label}</button>)}
            </div></div>
            <pre className="term" style={{ margin: 0, minHeight: 230, overflowX: 'auto' }}>{SNIPPETS[k].code}</pre>
          </div>
        </Reveal>
      </div>
    </section>
  )
}

export function Footer() {
  const cols = [
    { h: 'Tools', l: [['ember', '#tools'], ['forge', '#tools'], ['spark', '#tools'], ['ward', '#tools']] },
    { h: 'Docs', l: [['Get started', '#start'], ['Benchmarks', '#benchmarks'], ['Playground', '#playground'], ['Changelog', `${REPO}/blob/main/CHANGELOG.md`]] },
    { h: 'Project', l: [['GitHub', REPO], ['Releases', `${REPO}/releases`], ['Issues', `${REPO}/issues`], ['bench/run.py', `${REPO}/blob/main/bench/run.py`]] },
  ]
  return (
    <footer style={{ borderTop: '1px solid #14151a', padding: '64px 0 40px', background: 'linear-gradient(#040506, #07080a)' }}>
      <div className="wrap">
        <div style={{ display: 'grid', gap: 32, gridTemplateColumns: 'minmax(220px,1.4fr) repeat(3, minmax(120px, 1fr))' }}>
          <div>
            <a className="brand" href="#top" style={{ marginBottom: 14 }}><img src={logo('blaze')} alt="" />blaze</a>
            <p style={{ color: 'var(--smoke)', fontSize: 14, maxWidth: 280, lineHeight: 1.55 }}>Extremely fast Rust tooling. An independent early-stage project, not affiliated with Astral or the Rust Project.</p>
          </div>
          {cols.map((c) => (
            <div key={c.h}>
              <div className="eyebrow" style={{ marginBottom: 14 }}>{c.h}</div>
              {c.l.map(([t, h]) => <a key={t} href={h} style={{ display: 'block', color: 'var(--ash)', fontSize: 14, padding: '4px 0', textDecoration: 'none' }}>{t}</a>)}
            </div>
          ))}
        </div>
        <div className="mono" style={{ marginTop: 48, fontSize: 12, color: 'var(--smoke)' }}>
          © 2026 Blaze contributors &nbsp;|&nbsp; MIT OR Apache-2.0 &nbsp;|&nbsp; charts by bklit ui &nbsp;|&nbsp; design after Raycast style (Refero)
        </div>
      </div>
    </footer>
  )
}
