import { TOOLS, logo } from '../lib/blaze'
import { useSpot } from '../lib/hooks'
import { Reveal } from './Reveal'

function Card({ id, role, vs, blurb, alias, n }: { id: string; role: string; vs: string; blurb: string; alias: string; n: number }) {
  const spot = useSpot()
  return (
    <Reveal delay={n * 0.07}>
      <div className="card spot" {...spot} style={{ padding: 26, height: '100%' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <img src={logo(id)} alt={`${id} logo`} width={56} height={56} />
          <span className="badge">blaze {alias}</span>
        </div>
        <h3 style={{ margin: '22px 0 4px', fontSize: 24, fontWeight: 600, letterSpacing: '-0.02em' }}>{id}</h3>
        <div className="mono" style={{ color: 'var(--rust-1)', fontSize: 12, marginBottom: 12 }}>{role} · replaces {vs}</div>
        <p style={{ color: 'var(--ash)', margin: 0, lineHeight: 1.55, fontSize: 15 }}>{blurb}</p>
      </div>
    </Reveal>
  )
}

const PRINCIPLES = [
  { n: '01', t: 'Fast by construction', d: 'No syntax tree where none is needed: ember lints a token stream, every tool fans out across cores, and results are cached by content hash.' },
  { n: '02', t: 'Never lose your code', d: 'forge re-attaches comments and blank lines, then verifies the result token-for-token. If it cannot prove the output is safe, the file is left untouched.' },
  { n: '03', t: 'Measured, not claimed', d: 'Every number on this page comes from a script in the repo — with the caveats printed next to it, including where we are only 2× faster.' },
]

export function Tools() {
  return (
    <>
      <section id="tools" className="section">
        <div className="wrap">
          <Reveal>
            <div className="eyebrow">The toolchain</div>
            <h2 className="h2">Four tools. <span className="grad">One cog.</span></h2>
            <p className="lede">Each tool ships as its own binary and as a <code>blaze</code> subcommand. Anything else — <code>build</code>, <code>add</code>, <code>run</code> — is forwarded to cargo.</p>
          </Reveal>
          <div style={{ display: 'grid', gap: 16, gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))', marginTop: 40 }}>
            {TOOLS.map((t, i) => <Card key={t.id} {...t} n={i} />)}
          </div>
        </div>
      </section>
      <hr className="hair" />
      <section className="section" style={{ paddingTop: 96 }}>
        <div className="wrap" style={{ display: 'grid', gap: 18, gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))' }}>
          {PRINCIPLES.map((p, i) => (
            <Reveal key={p.n} delay={i * 0.08}>
              <div className="mono" style={{ color: 'var(--rust-1)', fontSize: 13 }}>{p.n}</div>
              <h3 style={{ fontSize: 26, letterSpacing: '-0.025em', margin: '10px 0 10px', fontWeight: 600 }}>{p.t}</h3>
              <p style={{ color: 'var(--ash)', lineHeight: 1.6, margin: 0 }}>{p.d}</p>
            </Reveal>
          ))}
        </div>
      </section>
    </>
  )
}
