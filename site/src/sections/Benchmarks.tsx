import { BarChart } from '@/components/charts/bar-chart'
import { Bar } from '@/components/charts/bar'
import { BarXAxis } from '@/components/charts/bar-x-axis'
import { BarYAxis } from '@/components/charts/bar-y-axis'
import { Grid } from '@/components/charts/grid'
import { ChartTooltip } from '@/components/charts/tooltip'
import { R, x } from '../lib/blaze'
import { Reveal } from './Reveal'

const speedups = [
  { scenario: 'format cold', speedup: +(R.fmt_rustfmt / R.fmt_forge_cold).toFixed(1) },
  { scenario: 'format repeat', speedup: +(R.fmt_rustfmt / R.fmt_forge_warm).toFixed(1) },
  { scenario: 'test cold', speedup: +(R.test_nextest / R.test_spark).toFixed(1) },
  { scenario: 'test cached', speedup: +(R.test_nextest / R.test_spark_cached).toFixed(1) },
  { scenario: 'advisories', speedup: +(R.advisories_cargo_deny / R.advisories_ward).toFixed(1) },
  { scenario: 'cold build 2nd', speedup: +(R.coldbuild_cargo_second / R.coldbuild_blaze_shared_second).toFixed(1) },
  { scenario: 'add x3', speedup: +(R.add_cargo_3 / R.add_blaze_3).toFixed(1) },
  { scenario: 'check no-op', speedup: +(R.check_cargo_noop / R.check_blaze_noop).toFixed(1) },
  { scenario: 'run no-op', speedup: +(R.run_cargo_noop / R.run_blaze_noop).toFixed(1) },
  { scenario: 'build no-op', speedup: +(R.build_cargo_noop / R.build_blaze_noop).toFixed(1) },
  { scenario: 'lint vs clippy', speedup: +(R.clippy_cold / R.lint_ember_crate).toFixed(1) },
]
const times = [
  { tool: 'Format', incumbent: R.fmt_rustfmt, blaze: R.fmt_forge_cold },
  { tool: 'Test', incumbent: R.test_nextest, blaze: R.test_spark },
  { tool: 'Lint (vs clean clippy)', incumbent: R.clippy_cold, blaze: R.lint_ember_crate },
]

export function Benchmarks() {
  return (
    <section id="benchmarks" className="section">
      <div className="wrap">
        <Reveal>
          <div className="eyebrow">Benchmarks</div>
          <h2 className="h2">Numbers you can <span className="grad">reproduce.</span></h2>
          <p className="lede">Drawn from <code>bench/results.json</code>, produced by <code>bench/run.py</code> on {R.machine}.</p>
        </Reveal>
        <div style={{ display: 'grid', gap: 18, gridTemplateColumns: 'repeat(auto-fit, minmax(420px, 1fr))', marginTop: 36 }}>
          <Reveal>
            <div className="card" style={{ padding: 24 }}>
              <h3 style={{ margin: 0, fontSize: 18 }}>Speedup vs the incumbent</h3>
              <p style={{ color: 'var(--ash)', fontSize: 13.5, margin: '4px 0 6px' }}>Higher is better · ×</p>
              <BarChart data={speedups} xDataKey="scenario" orientation="horizontal" aspectRatio="4 / 4.6" margin={{ top: 16, right: 20, bottom: 24, left: 120 }}>
                <Grid vertical horizontal={false} />
                <Bar dataKey="speedup" fill="var(--chart-line-primary)" lineCap={6} />
                <BarYAxis />
                <ChartTooltip />
              </BarChart>
            </div>
          </Reveal>
          <Reveal delay={0.1}>
            <div className="card" style={{ padding: 24 }}>
              <h3 style={{ margin: 0, fontSize: 18 }}>Wall time, cold</h3>
              <p style={{ color: 'var(--ash)', fontSize: 13.5, margin: '4px 0 6px' }}>Lower is better · seconds · grey = incumbent</p>
              <BarChart data={times} xDataKey="tool" aspectRatio="16 / 10" margin={{ top: 28, right: 12, bottom: 40, left: 12 }}>
                <Grid horizontal />
                <Bar dataKey="incumbent" fill="var(--chart-line-secondary)" lineCap={6} />
                <Bar dataKey="blaze" fill="var(--chart-line-primary)" lineCap={6} />
                <BarXAxis showAllLabels />
                <ChartTooltip />
              </BarChart>
            </div>
          </Reveal>
        </div>
        <Reveal delay={0.1}>
          <div className="card" style={{ marginTop: 18, padding: 28, borderLeft: '3px solid var(--rust-1)', borderRadius: 12 }}>
            <b>What these numbers do not say.</b>
            <p style={{ color: 'var(--ash)', margin: '8px 0 0', fontSize: 14.5, lineHeight: 1.6 }}>
              ember has 6 syntactic rules and no type information, so it is not a clippy replacement ({x(R.clippy_cold, R.lint_ember_crate)} compares speed, not coverage). forge prints with prettyplease, so its style is not rustfmt's, and rustfmt also follows <code>mod</code> children. Cached figures describe repeat runs. Cold formatting is {x(R.fmt_rustfmt, R.fmt_forge_cold)} and cold testing {x(R.test_nextest, R.test_spark)} faster; the largest gaps come from skipping work that did not change (no-op build, repeat runs, cached tests), where cargo still pays a fixed cost. ember scans {R.full_corpus_files.toLocaleString()} files ({R.full_corpus_mb} MB) in {R.lint_ember_full_corpus.toFixed(1)} s. Absolute times vary with load.
            </p>
          </div>
        </Reveal>
      </div>
    </section>
  )
}
