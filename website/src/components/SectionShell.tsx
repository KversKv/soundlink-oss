import { useEffect, useRef, type ReactNode } from 'react'

interface Props {
  id?: string
  className?: string
  children: ReactNode
}

// 分区外壳：统一纵向节奏 + 滚动进入动效（prefers-reduced-motion 下静态化）
export function SectionShell({ id, className = '', children }: Props) {
  const ref = useRef<HTMLElement>(null)

  useEffect(() => {
    const el = ref.current
    if (!el) return
    const targets = el.querySelectorAll('.reveal')
    if (targets.length === 0) return
    const io = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            entry.target.classList.add('in-view')
            io.unobserve(entry.target)
          }
        })
      },
      { threshold: 0.15 },
    )
    targets.forEach((t) => io.observe(t))
    return () => io.disconnect()
  }, [])

  return (
    <section id={id} ref={ref} className={`py-24 md:py-32 ${className}`}>
      <div className="mx-auto w-full max-w-6xl px-6">{children}</div>
    </section>
  )
}
