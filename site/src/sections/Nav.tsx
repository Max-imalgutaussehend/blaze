import { REPO, logo } from '../lib/blaze'

export function Nav() {
  return (
    <nav className="nav">
      <a className="brand" href="#top"><img src={logo('blaze')} alt="" />blaze</a>
      <a className="l hide" href="#tools">Tools</a>
      <a className="l hide" href="#race">Speed</a>
      <a className="l hide" href="#playground">Playground</a>
      <a className="l hide" href="#benchmarks">Benchmarks</a>
      <a className="l" href="#start">Get started</a>
      <a className="btn btn-primary" style={{ padding: '8px 12px', fontSize: 13 }} href={REPO}>GitHub</a>
    </nav>
  )
}
