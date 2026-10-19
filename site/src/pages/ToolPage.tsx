import { BarChart } from '@/components/charts/bar-chart'
import { Bar } from '@/components/charts/bar'
import { Grid } from '@/components/charts/grid'
import { BarYAxis } from '@/components/charts/bar-y-axis'
import { ChartTooltip } from '@/components/charts/tooltip'
import { useState } from 'react'
import { Nav } from '../sections/Nav'
import { Footer } from '../sections/Start'
import { Playground } from '../sections/Playground'
import { Terminal, type Line } from '../sections/Terminal'
import { Reveal } from '../sections/Reveal'
import { ToolCard } from '../sections/Tools'
import { RulesTable } from '../sections/RulesTable'
import { TOOLS, fmtT, logo, tool, type ToolId } from '../lib/blaze'
import { useSpot } from '../lib/hooks'

const SCRIPTS: Partial<Record<ToolId, { title: string; lines: Line[] }>> = {
  blaze: {
    title: 'a project that is already built',
    lines: [
      { kind: 'cmd', html: 'blaze build' },
      { kind: 'out', html: '<span class="t">    blaze: up to date, cargo not started (3.5ms)</span>', wait: 300 },
      { kind: 'cmd', html: 'blaze run -- --version', wait: 600 },
      { kind: 'out', html: '<span class="t">    blaze: up to date, cargo not started (0.4ms)</span>', wait: 250 },
      { kind: 'out', html: 'forge 0.3.0-alpha.1', wait: 100 },
      { kind: 'cmd', html: 'blaze add serde -F derive', wait: 600 },
      { kind: 'out', html: '      Adding serde v1.0.229 to dependencies with features: derive', wait: 300 },
    ],
  },
  spark: {
    title: 'regex-syntax: 147 tests',
    lines: [
      { kind: 'cmd', html: 'spark' },
      { kind: 'out', html: '<span class="ok">PASS</span> <span class="t"> 2.1ms</span>  regex_syntax::hir::tests::empty', wait: 300 },
      { kind: 'out', html: '<span class="ok">PASS</span> <span class="t"> 1.8ms</span>  regex_syntax::ast::parse::tests::parse_ok', wait: 120 },
      { kind: 'out', html: '<span class="dim">... 145 more, all running in parallel</span>', wait: 150 },
      { kind: 'out', html: 'spark: 147 run, 0 cached, 0 failed <span class="t">in 0.35s</span>', wait: 350 },
      { kind: 'cmd', html: 'spark --cached', wait: 800 },
      { kind: 'out', html: 'spark: 0 run, 147 cached, 0 failed <span class="t">in 10ms</span>', wait: 400 },
    ],
  },
  ward: {
    title: 'example output for a policy with violations',
    lines: [
      { kind: 'cmd', html: 'ward' },
      { kind: 'out', html: '<span class="p">[license]</span> some-crate 1.2.0 license `GPL-3.0` is not allowed', wait: 300 },
      { kind: 'out', html: '<span class="p">[ban]</span> openssl 0.10.0 is banned', wait: 150 },
      { kind: 'out', html: '<span class="p">[duplicate]</span> syn appears in versions 1.0.109, 2.0.100', wait: 150 },
      { kind: 'out', html: 'ward: 3 violations', wait: 250 },
      { kind: 'cmd', html: 'echo $?', wait: 700 },
      { kind: 'out', html: '1', wait: 200 },
    ],
  },
}

function Section({ eyebrow, title, children }: { eyebrow: string; title: React.ReactNode; children: React.ReactNode }) {
  return (
    <section className="section" style={{ paddingTop: 72, paddingBottom: 72 }}>
      <div className="wrap">
        <Reveal><div className="eyebrow">{eyebrow}</div><h2 className="h2">{title}</h2></Reveal>
        {children}
      </div>
    </section>
  )
}

function Cap({ t, d, n }: { t: string; d: string; n: number }) {
  const spot = useSpot()
  return (
    <Reveal delay={(n % 3) * 0.06}>
      <div className="card spot" {...spot} style={{ padding: 22, height: '100%' }}>
        <div className="mono" style={{ color: 'var(--rust-1)', fontSize: 12 }}>{String(n + 1).padStart(2, '0')}</div>
        <h3 style={{ margin: '8px 0 6px', fontSize: 18, fontWeight: 600, letterSpacing: '-0.015em' }}>{t}</h3>
        <p style={{ margin: 0, color: 'var(--ash)', fontSize: 14.5, lineHeight: 1.55 }}>{d}</p>
      </div>
    </Reveal>
  )
}

export function ToolPage({ id }: { id: string }) {
  const t = tool(id)
  const [tab, setTab] = useState<'usage' | 'config'>('usage')
  const script = SCRIPTS[t.id]
  return (
    <>
      <Nav current={t.id} />
      <header style={{ position: 'relative', paddingTop: 150, paddingBottom: 64, overflow: 'hidden' }}>
        <div className="aurora" style={{ opacity: 0.7 }}><i /><i /><i /></div>
        <div className="grid-lines" />
        <div className="wrap" style={{ position: 'relative' }}>
          <img src={logo(t.id)} alt={`${t.id} logo`} width={96} height={96} style={{ filter: 'drop-shadow(0 0 40px rgba(255,122,61,.45))' }} />
          <div className="eyebrow" style={{ marginTop: 28 }}>{t.role} · {t.badge}</div>
          <h1 style={{ fontSize: 'clamp(3rem, 8vw, 5.5rem)', lineHeight: 1, letterSpacing: '-0.045em', fontWeight: 700, margin: '14px 0 16px' }}>{t.id}</h1>
          <p className="lede" style={{ fontSize: '1.4rem', color: '#fff', maxWidth: 760 }}>{t.tagline}</p>
          <div style={{ display: 'flex', gap: 28, marginTop: 34, flexWrap: 'wrap', alignItems: 'center' }}>
            <div><div className="num grad">{t.headline.value}</div><div className="mono" style={{ color: 'var(--ash)', fontSize: 12, marginTop: 8, maxWidth: 380 }}>{t.headline.label}</div></div>
            <a className="btn btn-primary" href="#bench">See the benchmark</a>
            <a className="btn btn-ghost" href="#usage">Usage</a>
          </div>
        </div>
      </header>

      <Section eyebrow="What it does" title={<>What <span className="grad">{t.id}</span> can do</>}>
        <div style={{ display: 'grid', gap: 14, gridTemplateColumns: 'repeat(auto-fit, minmax(290px, 1fr))', marginTop: 28 }}>
          {t.can.map((c, i) => <Cap key={c.t} {...c} n={i} />)}
        </div>
      </Section>

      {t.id === 'ember' && <RulesTable />}
      {(t.id === 'ember' || t.id === 'forge') && (
        <Playground only={t.id === 'ember' ? 'lint' : 'fmt'} id="demo" />
      )}
      {script && (
        <Section eyebrow="In action" title="See it run">
          <div style={{ marginTop: 24 }}><Terminal script={script.lines} title={script.title} /></div>
        </Section>
      )}

      <Section eyebrow="Benchmark" title={<>{t.bench.title}</>}>
        <div id="bench" />
        <Reveal>
          <div className="card" style={{ padding: 24, marginTop: 24, maxWidth: 860 }}>
            <p style={{ color: 'var(--ash)', fontSize: 13.5, margin: '0 0 6px' }}>{t.bench.sub} Grey is the incumbent, orange is {t.id}.</p>
            <BarChart data={t.bench.rows} xDataKey="name" orientation="horizontal" aspectRatio="16 / 7" margin={{ top: 12, right: 24, bottom: 24, left: 175 }}>
              <Grid vertical horizontal={false} />
              <Bar dataKey="incumbent" fill="var(--chart-line-secondary)" lineCap={6} />
              <Bar dataKey="blaze" fill="var(--chart-line-primary)" lineCap={6} />
              <BarYAxis />
              <ChartTooltip />
            </BarChart>
            <div className="mono" style={{ display: 'flex', flexWrap: 'wrap', gap: 18, fontSize: 12.5, color: 'var(--ash)', marginTop: 8 }}>
              {t.bench.rows.map((r) => <span key={r.name}>{r.name}: <b style={{ color: '#fff' }}>{fmtT(r.incumbent || r.blaze)}</b></span>)}
            </div>
            <p style={{ color: 'var(--smoke)', fontSize: 13, margin: '14px 0 0' }}>{t.bench.note} Measured on {`${'arm64 Darwin, 8 cores'}`}, best of three; reproduce with <code>bench/run.py</code>.</p>
          </div>
        </Reveal>
      </Section>

      <Section eyebrow="Usage" title="Get going">
        <div id="usage" />
        <Reveal>
          <div className="win" style={{ marginTop: 24, maxWidth: 860 }}>
            <div className="win-bar"><i /><i /><i />
              <div className="tabs" style={{ marginLeft: 12 }}>
                <button className={tab === 'usage' ? 'on' : ''} onClick={() => setTab('usage')}>Commands</button>
                <button className={tab === 'config' ? 'on' : ''} onClick={() => setTab('config')}>blaze.toml</button>
              </div>
            </div>
            <pre className="term" style={{ margin: 0, minHeight: 130, overflowX: 'auto' }}>{tab === 'usage' ? t.usage : t.config}</pre>
          </div>
        </Reveal>
      </Section>

      <Section eyebrow="Under the hood" title="How it works">
        <div style={{ maxWidth: 760, marginTop: 20 }}>
          {t.how.map((p, i) => <Reveal key={i} delay={i * 0.06}><p style={{ color: 'var(--ash)', lineHeight: 1.7, fontSize: 16.5 }}>{p}</p></Reveal>)}
          <Reveal><div className="card" style={{ padding: 22, marginTop: 28, borderLeft: '3px solid var(--rust-1)', borderRadius: 12 }}>
            <b>Limitations</b><p style={{ color: 'var(--ash)', margin: '8px 0 0', lineHeight: 1.6, fontSize: 15 }}>{t.limits}</p>
          </div></Reveal>
        </div>
      </Section>

      <Section eyebrow="More tools" title="The rest of the toolchain">
        <div style={{ display: 'grid', gap: 16, gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))', marginTop: 28 }}>
          {TOOLS.filter((o) => o.id !== t.id).map((o, i) => <ToolCard key={o.id} t={o} n={i} />)}
        </div>
      </Section>
      <Footer />
    </>
  )
}
