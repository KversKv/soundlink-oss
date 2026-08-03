import { ShieldCheck, Usb, SealCheck } from '@phosphor-icons/react'
import { SectionShell } from '../components/SectionShell'
import type { Content } from '../content/zh'

const ICONS = [Usb, ShieldCheck, SealCheck]

// 非对称 bento：1 大 + 2 小
export function Differentiators({ c }: { c: Content }) {
  const [first, ...rest] = c.differentiators.items
  const FirstIcon = ICONS[0]

  return (
    <SectionShell id="features">
      <h2 className="reveal text-2xl font-semibold tracking-tight md:text-3xl">{c.differentiators.title}</h2>
      <div className="mt-10 grid grid-cols-1 gap-5 md:grid-cols-2">
        <div className="reveal rounded-[12px] border border-border bg-surface-1 p-8 md:row-span-2">
          <FirstIcon size={28} className="text-accent" />
          <h3 className="mt-5 text-xl font-semibold">{first.title}</h3>
          <p className="mt-3 max-w-[65ch] leading-relaxed text-text-dim">{first.body}</p>
        </div>
        {rest.map((item, i) => {
          const Icon = ICONS[i + 1]
          return (
            <div
              key={item.title}
              className="reveal rounded-[12px] border border-border bg-surface-1 p-8"
              style={{ transitionDelay: `${(i + 1) * 80}ms` }}
            >
              <Icon size={24} className="text-accent" />
              <h3 className="mt-4 text-lg font-semibold">{item.title}</h3>
              <p className="mt-3 leading-relaxed text-text-dim">{item.body}</p>
            </div>
          )
        })}
      </div>
    </SectionShell>
  )
}
