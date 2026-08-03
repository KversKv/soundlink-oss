import { SectionShell } from '../components/SectionShell'
import type { Content } from '../content/zh'

// 两列分组清单，诚实展示，不折叠
export function Limitations({ c }: { c: Content }) {
  return (
    <SectionShell className="bg-surface-1/40">
      <h2 className="reveal text-2xl font-semibold tracking-tight md:text-3xl">{c.limitations.title}</h2>
      <p className="reveal mt-4 max-w-[65ch] leading-relaxed text-text-dim">{c.limitations.intro}</p>
      <div className="mt-10 grid grid-cols-1 gap-5 md:grid-cols-2">
        {c.limitations.groups.map((g, gi) => (
          <div
            key={g.name}
            className="reveal rounded-[12px] border border-border bg-surface-1 p-6"
            style={{ transitionDelay: `${gi * 80}ms` }}
          >
            <h3 className="text-base font-semibold">{g.name}</h3>
            <ul className="mt-4 space-y-3 text-sm leading-relaxed text-text-dim">
              {g.items.map((item) => (
                <li key={item} className="flex gap-3">
                  <span aria-hidden="true" className="mt-[9px] h-1 w-1 shrink-0 rounded-full bg-text-dim" />
                  <span>{item}</span>
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>
    </SectionShell>
  )
}
