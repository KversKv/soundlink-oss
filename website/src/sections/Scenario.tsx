import { Check, X } from '@phosphor-icons/react'
import { SectionShell } from '../components/SectionShell'
import type { Content } from '../content/zh'

// 全宽单栏 editorial，右侧/下方配场景图
export function Scenario({ c }: { c: Content }) {
  return (
    <SectionShell className="bg-surface-1/40">
      <div className="grid grid-cols-1 items-center gap-12 md:grid-cols-[1.1fr_1fr]">
        <div className="reveal max-w-[65ch]">
          <h2 className="text-2xl font-semibold tracking-tight md:text-3xl">{c.scenario.title}</h2>
          <p className="mt-5 leading-relaxed text-text-dim">{c.scenario.problem}</p>

          <div className="mt-8 grid grid-cols-1 gap-6 sm:grid-cols-2">
            <div className="rounded-[12px] border border-border bg-surface-1 p-5">
              <div className="text-sm font-medium text-accent">{c.scenario.fitsTitle}</div>
              <ul className="mt-3 space-y-2 text-sm">
                {c.scenario.fits.map((f) => (
                  <li key={f} className="flex items-start gap-2">
                    <Check size={16} className="mt-0.5 shrink-0 text-accent" weight="bold" />
                    <span>{f}</span>
                  </li>
                ))}
              </ul>
            </div>
            <div className="rounded-[12px] border border-border bg-surface-1 p-5">
              <div className="text-sm font-medium text-text-dim">{c.scenario.unfitTitle}</div>
              <ul className="mt-3 space-y-2 text-sm text-text-dim">
                {c.scenario.unfit.map((f) => (
                  <li key={f} className="flex items-start gap-2">
                    <X size={16} className="mt-0.5 shrink-0 text-[#e06c75]" weight="bold" />
                    <span>{f}</span>
                  </li>
                ))}
              </ul>
            </div>
          </div>
        </div>

        <div className="reveal">
          {/* TODO(A4): 替换为真实场景图 */}
          <img
            src="https://picsum.photos/seed/soundlink-scene/1600/1100"
            alt={c.scenario.bgAlt}
            width={1600}
            height={1100}
            loading="lazy"
            className="block h-auto w-full rounded-[12px] border border-border object-cover"
          />
        </div>
      </div>
    </SectionShell>
  )
}
