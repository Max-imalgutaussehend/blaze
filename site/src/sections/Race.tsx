import { useEffect, useRef, useState } from 'react'
import { R, fmtT, x } from '../lib/blaze'
import { Reveal } from './Reveal'

type Lane = { name: string; them: string; us: string; tThem: number; tUs: number; note: string }
const COLD: Lane[] = [
  { name: 'Format', them: 'rustfmt', us: 'forge', tThem: R.fmt_rustfmt, tUs: R.fmt_forge_cold, note: `${R.files} files, cold` },
  { name: 'Lint', them: 'clippy (clean build)', us: 'ember', tThem: R.clippy_cold, tUs: R.lint_ember_crate, note: 'one crate · different depth' },
  { name: 'Test', them: 'nextest', us: 'spark', tThem: R.test_nextest, tUs: R.test_spark, note: '147 tests' },
]
const CACHED: Lane[] = [
  { name: 'Format', them: 'rustfmt', us: 'forge (cached)', tThem: R.fmt_rustfmt, tUs: R.fmt_forge_warm, note: `${R.files} files` },
  { name: 'Lint', them: 'clippy (no changes)', us: 'ember', tThem: R.clippy_warm, tUs: R.lint_ember_crate, note: 'one crate · different depth' },
  { name: 'Test', them: 'nextest', us: 'spark --cached', tThem: R.test_nextest, tUs: R.test_spark_cached, note: '147 tests · deterministic' },
  { name: 'Deps', them: 'cargo deny', us: 'ward (cached)', tThem: R.deny_cargo_deny, tUs: R.deny_ward_warm, note: 'licenses · bans · sources' },
]

export function Race() {
  const [mode, setMode] = useState<'cold' | 'cached'>('cold')
  const lanes = mode === 'cold' ? COLD : CACHED
  const [t, setT] = useState(0) // seconds elapsed in replay
  const [running, setRunning] = useState(false)
  const raf = useRef(0)
  const total = Math.max(...lanes.map((l) => l.tThem))

  const start = () => {
    cancelAnimationFrame(raf.current)
    const t0 = performance.now()
    setRunning(true); setT(0)
    const tick = (now: number) => {
      const s = (now - t0) / 1000
      if (s >= total) { setT(total); setRunning(false); return }
      setT(s); raf.current = requestAnimationFrame(tick)
    }
    raf.current = requestAnimationFrame(tick)
  }
  useEffect(() => { setT(0); setRunning(false); cancelAnimationFrame(raf.current) }, [mode])
  useEffect(() => () => cancelAnimationFrame(raf.current), [])
  const finished = !running && t >= total

  return (
    <section id="race" className="section">
      <div className="wrap">
        <Reveal>
          <div className="eyebrow">Speed</div>
          <h2 className="h2">Press play. <span className="grad">Watch it finish.</span></h2>
          <p className="lede">A real-time replay of the timings from <code>bench/results.json</code>: same clock, same machine, best of three. This is a replay, not a live benchmark.</p>
        </Reveal>
        <Reveal delay={0.1}>
          <div className="card" style={{ marginTop: 36, padding: 28 }}>
            <div style={{ display: 'flex', gap: 14, alignItems: 'center', flexWrap: 'wrap', marginBottom: 18 }}>
              <div className="tabs">
                <button className={mode === 'cold' ? 'on' : ''} onClick={() => setMode('cold')}>Cold</button>
                <button className={mode === 'cached' ? 'on' : ''} onClick={() => setMode('cached')}>Cached (repeat run)</button>
              </div>
              <button className="btn btn-primary" onClick={start} disabled={running}>{running ? 'Running…' : finished ? '↻ Run again' : '▶ Run'}</button>
              <span className="mono" style={{ marginLeft: 'auto', color: 'var(--ash)', fontSize: 13 }}>t = {(t * 1000).toFixed(0).padStart(4, '0')} ms</span>
            </div>
            {lanes.map((l) => (
              <div key={l.name} style={{ margin: '22px 0' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline' }}>
                  <b style={{ fontSize: 15 }}>{l.name}</b><span style={{ color: 'var(--smoke)', fontSize: 12.5 }}>{l.note}</span>
                </div>
                {[{ label: l.them, dur: l.tThem, cls: 'them' }, { label: l.us, dur: l.tUs, cls: 'us' }].map((b) => {
                  const p = Math.min(1, t / b.dur)
                  const done = t >= b.dur
                  return (
                    <div key={b.label} className={`lane ${b.cls} ${done ? 'done' : ''}`}>
                      <span style={{ color: b.cls === 'us' ? '#fff' : 'var(--ash)' }}>{b.label}</span>
                      <div className="track"><div className={`fill ${b.cls}`} style={{ width: `${t === 0 ? 0 : p * 100}%` }} /></div>
                      <span className="time">{done ? `${fmtT(b.dur)} ✓` : t === 0 ? '·' : fmtT(t)}</span>
                    </div>
                  )
                })}
                {finished && <div className="mono" style={{ color: 'var(--rust-1)', fontSize: 13, textAlign: 'right' }}>{x(l.tThem, l.tUs)} faster</div>}
              </div>
            ))}
            <div style={{ color: 'var(--smoke)', fontSize: 12.5, borderTop: '1px solid #14151a', paddingTop: 14 }}>
              ember is a much smaller linter than clippy and does not type-check; forge's style is not rustfmt's; cached modes describe repeat runs. See <a href="#benchmarks" style={{ color: 'var(--ash)' }}>the fine print</a>.
            </div>
          </div>
        </Reveal>
      </div>
    </section>
  )
}
