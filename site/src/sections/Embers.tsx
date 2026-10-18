import { useEffect, useRef } from 'react'

/** Rising ember particles on a canvas. Skips animation when the user prefers reduced motion. */
export function Embers() {
  const ref = useRef<HTMLCanvasElement>(null)
  useEffect(() => {
    const c = ref.current!
    const ctx = c.getContext('2d')!
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) return
    let w = 0, h = 0, raf = 0
    const dpr = Math.min(devicePixelRatio || 1, 2)
    const resize = () => { w = c.clientWidth; h = c.clientHeight; c.width = w * dpr; c.height = h * dpr; ctx.setTransform(dpr, 0, 0, dpr, 0, 0) }
    resize(); addEventListener('resize', resize)
    const mk = (init = false) => ({ x: Math.random() * w, y: init ? Math.random() * h : h + 10, r: 0.6 + Math.random() * 1.8, v: 0.25 + Math.random() * 0.9, d: Math.random() * Math.PI * 2, a: 0.25 + Math.random() * 0.6 })
    const ps = Array.from({ length: 70 }, () => mk(true))
    const tick = () => {
      ctx.clearRect(0, 0, w, h)
      for (const p of ps) {
        p.y -= p.v; p.d += 0.015; p.x += Math.sin(p.d) * 0.4
        if (p.y < -10) Object.assign(p, mk())
        const fade = Math.min(1, p.y / (h * 0.5))
        const g = ctx.createRadialGradient(p.x, p.y, 0, p.x, p.y, p.r * 6)
        g.addColorStop(0, `rgba(255,160,90,${p.a * fade})`); g.addColorStop(1, 'rgba(255,90,40,0)')
        ctx.fillStyle = g; ctx.beginPath(); ctx.arc(p.x, p.y, p.r * 6, 0, 7); ctx.fill()
      }
      raf = requestAnimationFrame(tick)
    }
    tick()
    return () => { cancelAnimationFrame(raf); removeEventListener('resize', resize) }
  }, [])
  return <canvas ref={ref} style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', pointerEvents: 'none' }} />
}
