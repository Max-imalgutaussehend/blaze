import { REPO, TOOLS, link, logo } from '../lib/blaze'

export function Nav({ current }: { current?: string }) {
  return (
    <nav className="nav">
      <a className="brand" href={link('')}><img src={logo('blaze')} alt="" />blaze</a>
      {TOOLS.map((t) => (
        <a key={t.id} className={`l ${current === t.id ? 'on' : ''}`} href={link(`${t.id}/`)}>{t.id}</a>
      ))}
      <a className="l hide" href={link('#benchmarks')}>Benchmarks</a>
      <a className="l hide" href={link('#start')}>Get started</a>
      <a className="btn btn-primary" style={{ padding: '8px 12px', fontSize: 13 }} href={REPO}>GitHub</a>
    </nav>
  )
}
