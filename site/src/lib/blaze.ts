import results from '../../../bench/results.json'

export const R = results
export const REPO = 'https://github.com/Max-imalgutaussehend/blaze'
export const INSTALL_SH = `curl -LsSf ${REPO}/releases/latest/download/install.sh | sh`
export const INSTALL_CARGO = `cargo install --git ${REPO} blaze`
export const logo = (n: string) => `${import.meta.env.BASE_URL}logos/${n}.svg`

export const x = (a: number, b: number) => {
  const r = a / b
  return r >= 10 ? `${Math.round(r)}×` : `${r.toFixed(1)}×`
}
export const fmtT = (s: number) => (s < 1 ? `${(s * 1000).toFixed(s < 0.01 ? 1 : 0)} ms` : `${s.toFixed(2)} s`)

export const TOOLS = [
  { id: 'ember', role: 'linter', vs: 'clippy', blurb: 'Token-level rules, no compile step. Lints a crate in single-digit milliseconds.', alias: 'lint' },
  { id: 'forge', role: 'formatter', vs: 'rustfmt', blurb: 'Comment-preserving, verified output and a content-hash cache.', alias: 'fmt' },
  { id: 'spark', role: 'test runner', vs: 'cargo-nextest', blurb: 'Process-per-test on one worker pool, with an opt-in result cache.', alias: 'test' },
  { id: 'ward', role: 'dependency policy', vs: 'cargo-deny', blurb: 'Licenses, bans, duplicates and sources — checked from a cached graph.', alias: 'deny' },
] as const
