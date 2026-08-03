import { SectionShell } from '../components/SectionShell'
import type { Content } from '../content/zh'

// 三组规格卡，数字用 mono
export function Specs({ c }: { c: Content }) {
  return (
    <SectionShell id="specs">
      <h2 className="reveal text-2xl font-semibold tracking-tight md:text-3xl">{c.specs.title}</h2>
      <div className="mt-10 grid grid-cols-1 gap-5 md:grid-cols-3">
        {c.specs.groups.map((g, gi) => (
          <div
            key={g.name}
            className="reveal rounded-[12px] border border-border bg-surface-1 p-6"
            style={{ transitionDelay: `${gi * 80}ms` }}
          >
            <h3 className="text-base font-semibold">{g.name}</h3>
            <dl className="mt-4">
              {g.rows.map(([k, v], i) => (
                <div
                  key={k}
                  className={`flex items-baseline justify-between py-2.5 ${
                    i > 0 ? 'border-t border-border/60' : ''
                  }`}
                >
                  <dt className="text-sm text-text-dim">{k}</dt>
                  <dd className="font-mono text-sm text-text">{v}</dd>
                </div>
              ))}
            </dl>
          </div>
        ))}
      </div>
    </SectionShell>
  )
}
