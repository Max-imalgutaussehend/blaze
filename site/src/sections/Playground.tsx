import { useEffect, useMemo, useRef, useState } from 'react'
import { format, lint, type Diag } from '../lib/wasm'
import { Reveal } from './Reveal'

const SAMPLE = `// Config loader. Try editing me.
use std::fs;

/// Reads the port from disk.
fn read_port(path: &str) -> u16 {
    let text = fs::read_to_string(path).unwrap();   // ember: B001
    let v: Vec<&str> = text.split(',').collect();
    if v.len() == 0 {                                // ember: B004
        todo!();                                     // ember: B003
    }
    dbg!(&v);
    let ok = v[0].len()>0;
    if ok == true { return v[0].parse().unwrap_or(80); }
    80
}

fn main(){
    println!("{}",read_port("port.txt"));   // forge keeps this comment
}
`

export function Playground({ only, id = 'playground', heading = true }: { only?: 'lint' | 'fmt'; id?: string; heading?: boolean }) {
  const [tab, setTab] = useState<'lint' | 'fmt'>(only ?? 'lint')
  const [src, setSrc] = useState(SAMPLE)
  const [diags, setDiags] = useState<Diag[]>([])
  const [fmtOut, setFmtOut] = useState<string>('')
  const [err, setErr] = useState<string | null>(null)
  const [ms, setMs] = useState<number | null>(null)
  const [ready, setReady] = useState(false)
  const timer = useRef<number>(0)

  useEffect(() => {
    window.clearTimeout(timer.current)
    timer.current = window.setTimeout(async () => {
      try {
        if (tab === 'lint') { const r = await lint(src); setDiags(r.diags); setMs(r.ms); setErr(null) }
        else { const r = await format(src); setMs(r.ms); if (r.res.ok) { setFmtOut(r.res.output); setErr(null) } else setErr(r.res.error) }
        setReady(true)
      } catch (e) { setErr(String(e)) }
    }, 120)
    return () => window.clearTimeout(timer.current)
  }, [src, tab])

  const byLine = useMemo(() => { const m = new Map<number, Diag[]>(); diags.forEach((d) => m.set(d.line, [...(m.get(d.line) ?? []), d])); return m }, [diags])

  return (
    <section id={id} className="section">
      <div className="wrap">
        {heading && <Reveal>
          <div className="eyebrow">Playground</div>
          <h2 className="h2">The real tools, <span className="grad">running in your browser.</span></h2>
          <p className="lede">This is ember and forge compiled to WebAssembly (<code>crates/blaze-wasm</code>, 875&nbsp;KB) (not a mock-up). Edit the code; nothing leaves your machine.</p>
        </Reveal>}
        <Reveal delay={0.1}>
          <div className="win" style={{ marginTop: 32 }}>
            <div className="win-bar">
              <i /><i /><i />
              <span>main.rs</span>
              {!only && <div className="tabs" style={{ marginLeft: 'auto' }}>
                <button className={tab === 'lint' ? 'on' : ''} onClick={() => setTab('lint')}>ember · lint</button>
                <button className={tab === 'fmt' ? 'on' : ''} onClick={() => setTab('fmt')}>forge · format</button>
              </div>}
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(340px, 1fr))' }}>
              <div style={{ borderRight: '1px solid #14151a' }}>
                <textarea className="editor" spellCheck={false} value={src} onChange={(e) => setSrc(e.target.value)} aria-label="Rust source" />
              </div>
              <div style={{ minHeight: 360, background: '#050607' }}>
                {tab === 'lint' ? (
                  <div style={{ padding: '14px 0' }}>
                    {!ready && <div className="diag"><span className="dim" style={{ color: 'var(--smoke)' }}>loading wasm…</span></div>}
                    {ready && diags.length === 0 && <div className="diag" style={{ border: 0 }}><b style={{ color: '#59d499' }}>✔ no issues</b></div>}
                    {[...byLine.entries()].map(([line, ds]) => ds.map((d, i) => (
                      <div key={`${line}-${i}`} className="diag"><span style={{ color: 'var(--smoke)', minWidth: 52 }}>{d.line}:{d.column}</span><b>{d.code}</b><span style={{ color: 'var(--mist)' }}>{d.message}</span></div>
                    )))}
                  </div>
                ) : err ? (
                  <div className="diag" style={{ border: 0, padding: 20 }}><b>✖</b><span style={{ color: 'var(--mist)' }}>{err}</span></div>
                ) : (
                  <pre className="editor" style={{ margin: 0, minHeight: 360, color: 'var(--mist)' }}>{fmtOut}</pre>
                )}
              </div>
            </div>
            <div className="mono" style={{ padding: '10px 20px', borderTop: '1px solid #14151a', color: 'var(--smoke)', fontSize: 12, display: 'flex', justifyContent: 'space-between' }}>
              <span>{tab === 'lint' ? `${diags.length} issue${diags.length === 1 ? '' : 's'}` : err ? 'not formatted' : 'comments & blank lines preserved'}</span>
              <span>{ms !== null ? `${ms.toFixed(2)} ms in WebAssembly` : ''}</span>
            </div>
          </div>
        </Reveal>
      </div>
    </section>
  )
}
