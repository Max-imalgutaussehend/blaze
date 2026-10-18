import { useEffect, useRef, useState } from 'react'

export function useInView<T extends Element>(once = true, margin = '-80px') {
  const ref = useRef<T>(null)
  const [seen, setSeen] = useState(false)
  useEffect(() => {
    const el = ref.current
    if (!el) return
    const io = new IntersectionObserver(([e]) => {
      if (e.isIntersecting) { setSeen(true); if (once) io.disconnect() } else if (!once) setSeen(false)
    }, { rootMargin: margin })
    io.observe(el)
    return () => io.disconnect()
  }, [once, margin])
  return [ref, seen] as const
}

export function useSpot() {
  return {
    onMouseMove: (e: React.MouseEvent<HTMLElement>) => {
      const r = e.currentTarget.getBoundingClientRect()
      e.currentTarget.style.setProperty('--mx', `${e.clientX - r.left}px`)
      e.currentTarget.style.setProperty('--my', `${e.clientY - r.top}px`)
    },
  }
}

export function useCopy(text: string) {
  const [done, setDone] = useState(false)
  return [done, () => { navigator.clipboard?.writeText(text).then(() => { setDone(true); setTimeout(() => setDone(false), 1400) }) }] as const
}
