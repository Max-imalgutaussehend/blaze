import { useState } from 'react'
import { motion } from 'motion/react'
import { Embers } from './Embers'
import { INSTALL_CARGO, INSTALL_SH, REPO, logo } from '../lib/blaze'
import { useCopy } from '../lib/hooks'

export function Hero() {
  const [tab, setTab] = useState<'sh' | 'cargo'>('sh')
  const cmd = tab === 'sh' ? INSTALL_SH : INSTALL_CARGO
  const [done, copy] = useCopy(cmd)
  return (
    <header id="top" style={{ position: 'relative', paddingTop: 150, paddingBottom: 90, overflow: 'hidden' }}>
      <div className="aurora"><i /><i /><i /></div>
      <div className="grid-lines" />
      <img className="cog-bg" src={logo('cog')} alt="" />
      <Embers />
      <div className="wrap" style={{ position: 'relative', textAlign: 'center' }}>
        <motion.a href="#race" initial={{ opacity: 0, y: -10 }} animate={{ opacity: 1, y: 0 }} className="badge"
          style={{ padding: '6px 12px', borderRadius: 99, textDecoration: 'none', color: 'var(--ash)', boxShadow: '0 0 0 1px var(--hairline) inset' }}>
          <span style={{ color: 'var(--rust-1)' }}>●</span>&nbsp; v0.2.1 · early alpha — numbers measured, caveats included →
        </motion.a>
        <motion.h1 initial={{ opacity: 0, y: 24, filter: 'blur(10px)' }} animate={{ opacity: 1, y: 0, filter: 'blur(0px)' }} transition={{ duration: 0.9, ease: [0.2, 0.8, 0.2, 1] }}
          style={{ fontSize: 'clamp(2.8rem, 8vw, 6rem)', lineHeight: 1.0, letterSpacing: '-0.045em', fontWeight: 700, margin: '28px auto 22px', maxWidth: 980 }}>
          Rust tooling at the<br /><span className="grad">speed of thought.</span>
        </motion.h1>
        <motion.p initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ delay: 0.25, duration: 0.8 }} className="lede" style={{ margin: '0 auto 36px', fontSize: '1.2rem' }}>
          Lint, format, test and audit dependencies with one toolchain — built on a hand-written lexer, parallel workers and content-hash caches. Forged in Rust.
        </motion.p>
        <motion.div initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.4 }} style={{ display: 'flex', gap: 12, justifyContent: 'center', flexWrap: 'wrap' }}>
          <a className="btn btn-primary" href="#start">Get started</a>
          <a className="btn btn-ghost" href={REPO}>
            <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38v-1.33c-2.23.48-2.7-1.07-2.7-1.07-.36-.92-.89-1.17-.89-1.17-.73-.5.05-.49.05-.49.8.06 1.23.83 1.23.83.72 1.22 1.87.87 2.33.66.07-.52.28-.87.5-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82a7.6 7.6 0 0 1 4 0c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.28.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48v2.2c0 .21.15.46.55.38A8 8 0 0 0 16 8c0-4.42-3.58-8-8-8z" /></svg>
            Star on GitHub
          </a>
        </motion.div>
        <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ delay: 0.6 }} style={{ marginTop: 40 }}>
          <div className="tabs" style={{ marginBottom: 12 }}>
            <button className={tab === 'sh' ? 'on' : ''} onClick={() => setTab('sh')}>macOS &amp; Linux</button>
            <button className={tab === 'cargo' ? 'on' : ''} onClick={() => setTab('cargo')}>cargo</button>
          </div>
          <div style={{ display: 'flex', justifyContent: 'center' }}>
            <div className="card" style={{ display: 'inline-flex', alignItems: 'center', gap: 14, padding: '12px 14px 12px 18px', borderRadius: 12, maxWidth: '100%' }}>
              <code style={{ fontSize: 13.5, color: 'var(--mist)', overflowX: 'auto', whiteSpace: 'nowrap' }}><span style={{ color: 'var(--rust-1)' }}>$ </span>{cmd}</code>
              <button onClick={copy} className="btn btn-ghost" style={{ padding: '7px 11px', fontSize: 12 }}>{done ? 'Copied ✓' : 'Copy'}</button>
            </div>
          </div>
          <div className="mono" style={{ marginTop: 18, fontSize: 12, color: 'var(--smoke)' }}>
            v0.2.1 &nbsp;|&nbsp; MIT OR Apache-2.0 &nbsp;|&nbsp; macOS · Linux
          </div>
        </motion.div>
      </div>
    </header>
  )
}
