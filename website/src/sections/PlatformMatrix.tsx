import { SectionShell } from '../components/SectionShell'
import { StatusBadge } from '../components/StatusBadge'
import type { Content } from '../content/zh'

// 等宽单行图标状态栅格，<md 折叠为单列
export function PlatformMatrix({ c }: { c: Content }) {
  return (
    <SectionShell id="platforms" className="pt-16 md:pt-20">
      <h2 className="reveal text-2xl font-semibold tracking-tight md:text-3xl">{c.platforms.title}</h2>
      <div className="mt-8 grid grid-cols-1 gap-4 sm:grid-cols-2 md:grid-cols-5">
        {c.platforms.items.map((p, i) => (
          <div
            key={p.name}
            className="reveal rounded-[12px] border border-border bg-surface-1 p-5"
            style={{ transitionDelay: `${i * 60}ms` }}
          >
            <div className="text-base font-medium">{p.name}</div>
            <div className="mt-1 text-sm text-text-dim">{p.role}</div>
            <div className="mt-4">
              <StatusBadge status={p.status} labels={c.platforms.status} />
            </div>
          </div>
        ))}
      </div>
      <p className="reveal mt-6 max-w-[65ch] text-sm leading-relaxed text-text-dim">{c.platforms.note}</p>
    </SectionShell>
  )
}
