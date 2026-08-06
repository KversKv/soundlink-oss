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

        <div className="reveal relative mb-10 md:mb-0">
          <div className="overflow-hidden rounded-[12px] border border-border bg-surface-1">
            <img
              src={c.scenario.desktopShotSrc}
              alt={c.scenario.desktopShotAlt}
              width={1200}
              height={900}
              loading="lazy"
              className="block h-auto w-full"
            />
          </div>
          <div className="absolute -bottom-8 right-4 w-[34%] overflow-hidden rounded-[12px] border border-border bg-surface-1 shadow-2xl shadow-black/60 md:-right-6">
            <img
              src={c.scenario.phoneShotSrc}
              alt={c.scenario.phoneShotAlt}
              width={400}
              height={700}
              loading="lazy"
              className="block h-auto w-full"
            />
          </div>
        </div>
      </div>
    </SectionShell>
  )
}
