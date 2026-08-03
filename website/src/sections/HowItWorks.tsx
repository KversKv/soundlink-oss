import { SectionShell } from '../components/SectionShell'
import type { Content } from '../content/zh'

// 三栏截图序列，栏间用连接线串联（动词作标题，不用「步骤 1/2/3」）
export function HowItWorks({ c }: { c: Content }) {
  return (
    <SectionShell id="how" className="bg-surface-1/40">
      <h2 className="reveal text-2xl font-semibold tracking-tight md:text-3xl">{c.how.title}</h2>
      <ol className="mt-10 grid grid-cols-1 gap-10 md:grid-cols-3 md:gap-6">
        {c.how.steps.map((s, i) => (
          <li key={s.verb} className="reveal relative" style={{ transitionDelay: `${i * 80}ms` }}>
            {i < c.how.steps.length - 1 && (
              <span
                aria-hidden="true"
                className="absolute left-full top-16 hidden h-px w-6 bg-border md:block"
              />
            )}
            <div className="overflow-hidden rounded-[12px] border border-border bg-surface-1">
              <img
                src={s.imgSrc}
                alt={s.imgAlt}
                width={1200}
                height={800}
                loading="lazy"
                className="block h-auto w-full"
              />
            </div>
            <h3 className="mt-5 text-lg font-semibold">{s.verb}</h3>
            <p className="mt-2 max-w-[65ch] text-sm leading-relaxed text-text-dim">{s.body}</p>
          </li>
        ))}
      </ol>
    </SectionShell>
  )
}
