import { useEffect, useRef, useState } from 'react'
import { useInView } from '../lib/hooks'
import { Reveal } from './Reveal'

export type Line = { kind: 'cmd' | 'out'; html: string; wait?: number }
// Lines are taken from real runs: blaze check on this repo, spark on regex-syntax (147 tests).
export const HOME_SCRIPT: Line[] = [
  { kind: 'cmd', html: 'blaze check' },
  { kind: 'out', html: '<span class="ok">✔</span> forge   14 files, 14 cached, 0 changed <span class="t">in 8ms</span>', wait: 380 },
  { kind: 'out', html: '<span class="ok">✔</span> ember   14 files, 0 issues <span class="t">in 3ms</span>', wait: 260 },
  { kind: 'out', html: '<span class="ok">✔</span> ward    0 violations <span class="t">in 4ms</span>', wait: 260 },
  { kind: 'cmd', html: 'spark --cached', wait: 700 },
  { kind: 'out', html: '<span class="ok">✔</span> 147 tests, 147 cached, 0 failed <span class="t">in 10ms</span>', wait: 420 },
  { kind: 'cmd', html: 'blaze add serde', wait: 700 },
  { kind: 'out', html: '<span class="dim">→ forwarded to cargo</span>', wait: 300 },
]

export function Terminal({ script = HOME_SCRIPT, title = 'sample session: numbers from real runs' }: { script?: Line[]; title?: string }) {
  const [ref, seen] = useInView<HTMLDivElement>()
  const [shown, setShown] = useState<{ kind: string; html: string }[]>([])
  const [typing, setTyping] = useState('')
  const [run, setRun] = useState(0)
  const timers = useRef<number[]>([])

  useEffect(() => {
    if (!seen) return
    timers.current.forEach(clearTimeout); timers.current = []
    setShown([]); setTyping('')
    let t = 250
    const at = (ms: number, fn: () => void) => timers.current.push(window.setTimeout(fn, ms))
    script.forEach((l) => {
      t += l.wait ?? 0
      if (l.kind === 'cmd') {
        for (let i = 1; i <= l.html.length; i++) { const s = l.html.slice(0, i); at(t + i * 42, () => setTyping(s)) }
        t += l.html.length * 42 + 220
        at(t, () => { setTyping(''); setShown((p) => [...p, l]) })
      } else at(t, () => setShown((p) => [...p, l]))
    })
    return () => timers.current.forEach(clearTimeout)
  }, [seen, run, script])

  return (
    <section className="wrap" style={{ marginTop: -10, paddingBottom: 40 }}>
      <Reveal>
        <div ref={ref} className="win" style={{ maxWidth: 820, margin: '0 auto' }}>
          <div className="win-bar"><i /><i /><i /><span>{title}</span>
            <button onClick={() => setRun((r) => r + 1)} className="mono" style={{ marginLeft: 'auto', background: 'none', border: 0, color: 'var(--smoke)', cursor: 'pointer', fontSize: 12 }}>↻ replay</button>
          </div>
          <div className="term">
            {shown.map((l, i) => l.kind === 'cmd'
              ? <div key={i}><span className="p">❯ </span>{l.html}</div>
              : <div key={i} dangerouslySetInnerHTML={{ __html: l.html }} />)}
            {(typing || shown.length < script.length) && <div><span className="p">❯ </span>{typing}<span className="caret" /></div>}
          </div>
        </div>
      </Reveal>
    </section>
  )
}
