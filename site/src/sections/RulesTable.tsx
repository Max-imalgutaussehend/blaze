import { useEffect, useState } from 'react'
import { listRules, type RuleInfo } from '../lib/wasm'
import { Reveal } from './Reveal'

/** The rule list is read from the real ember (WebAssembly), so it can never drift from the code. */
export function RulesTable() {
  const [rules, setRules] = useState<RuleInfo[] | null>(null)
  useEffect(() => { listRules().then(setRules).catch(() => setRules([])) }, [])
  return (
    <section className="section" style={{ paddingTop: 48, paddingBottom: 48 }}>
      <div className="wrap">
        <Reveal>
          <div className="eyebrow">Rules</div>
          <h2 className="h2">{rules ? rules.length : 31} rules, <span className="grad">read from the binary.</span></h2>
          <p className="lede">Rules marked <b>fix</b> can be applied automatically. <b>Safe</b> fixes never depend on types; <b>unsafe</b> fixes need <code>--unsafe-fixes</code> because they can fail to compile for some types. Off-by-default rules run with <code>--select</code>.</p>
        </Reveal>
        <Reveal delay={0.1}>
          <div className="card" style={{ marginTop: 24, overflowX: 'auto' }}>
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 14 }}>
              <thead><tr style={{ textAlign: 'left', color: 'var(--smoke)' }}>
                {['Code', 'Name', 'Default', 'Fix', 'What it flags'].map((h) => <th key={h} style={{ padding: '12px 16px', fontWeight: 500, borderBottom: '1px solid #14151a' }}>{h}</th>)}
              </tr></thead>
              <tbody>
                {(rules ?? []).map((r) => (
                  <tr key={r.code} style={{ borderBottom: '1px solid #0f1013' }}>
                    <td className="mono" style={{ padding: '10px 16px', color: 'var(--rust-1)' }}>{r.code}</td>
                    <td className="mono" style={{ padding: '10px 16px' }}>{r.name}</td>
                    <td style={{ padding: '10px 16px', color: r.default ? '#59d499' : 'var(--smoke)' }}>{r.default ? 'on' : 'off'}</td>
                    <td style={{ padding: '10px 16px', color: 'var(--ash)' }}>{r.fixable ? (r.unsafe ? 'unsafe' : 'safe') : '-'}</td>
                    <td style={{ padding: '10px 16px', color: 'var(--ash)' }}>{r.summary}</td>
                  </tr>
                ))}
                {!rules && <tr><td colSpan={5} style={{ padding: 20, color: 'var(--smoke)' }}>loading rules from WebAssembly…</td></tr>}
              </tbody>
            </table>
          </div>
        </Reveal>
      </div>
    </section>
  )
}
